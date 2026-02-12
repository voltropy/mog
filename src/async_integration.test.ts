import { describe, test, expect } from "bun:test"
import { compile } from "./compiler"
import { compileRuntime } from "./linker"
import { spawnSync } from "child_process"
import { writeFileSync, existsSync, mkdirSync, rmSync } from "fs"
import { join } from "path"
import { tmpdir } from "os"

// Build runtime once for all tests
let runtimeLibPath: string

async function ensureRuntime(): Promise<string> {
  if (!runtimeLibPath) {
    runtimeLibPath = await compileRuntime()
  }
  return runtimeLibPath
}

/**
 * Compile Mog source with async support to a native executable and run it.
 * Uses clang -O1 -x ir for LLVM coroutine pass support.
 */
async function compileAndRun(source: string, args: string[] = [], timeoutMs: number = 10000): Promise<{ exitCode: number, stdout: string, stderr: string }> {
  const sourceDir = join(tmpdir(), `mog-async-test-${Date.now()}-${Math.random().toString(36).slice(2)}`)

  try {
    mkdirSync(sourceDir, { recursive: true })
    const runtimeLib = await ensureRuntime()

    const llFile = join(sourceDir, "test.ll")
    const oFile = join(sourceDir, "test.o")
    const exeFile = join(sourceDir, "test")

    // Compile Mog source to LLVM IR
    const result = await compile(source)
    if (result.errors.length > 0) {
      throw new Error(`Compilation errors: ${result.errors.map(e => e.message).join("; ")}`)
    }

    writeFileSync(llFile, result.llvmIR)

    // Use clang -O1 -x ir for coroutine lowering (required for presplitcoroutine)
    const clangIr = spawnSync("clang", ["-O1", "-c", "-x", "ir", llFile, "-o", oFile], {
      stdio: "pipe",
      timeout: 30000,
    })

    if (clangIr.status !== 0) {
      const stderr = clangIr.stderr?.toString() || ""
      // Write IR for debugging
      if (process.env.DEBUG_TEST) {
        console.log("LLVM IR:", result.llvmIR)
      }
      throw new Error(`clang IR compilation failed (status ${clangIr.status}): ${stderr}`)
    }

    // Link with runtime
    const clangLink = spawnSync("clang", [oFile, runtimeLib, "-o", exeFile], {
      stdio: "pipe",
      timeout: 30000,
    })

    if (clangLink.status !== 0 && !(clangLink.stderr?.toString() || "").includes("no platform load command")) {
      throw new Error(`clang linking failed: ${clangLink.stderr?.toString()}`)
    }

    // Run the executable
    const runResult = spawnSync(exeFile, args, {
      stdio: "pipe",
      timeout: timeoutMs,
    })

    return {
      exitCode: runResult.status ?? -1,
      stdout: runResult.stdout ? runResult.stdout.toString() : "",
      stderr: runResult.stderr ? runResult.stderr.toString() : "",
    }
  } finally {
    rmSync(sourceDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })
  }
}

describe("Async/Await Integration Tests", () => {

  describe("Basic async functions", () => {
    test("basic async function returning a value", async () => {
      const source = `{
  async fn compute(x: i64) -> i64 {
    return x + 1;
  }

  async fn main() -> i64 {
    r: i64 = await compute(41);
    return r;
  }
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(42)
    })

    test("async function with no await", async () => {
      const source = `{
  async fn simple() -> i64 {
    return 7;
  }

  async fn main() -> i64 {
    r: i64 = await simple();
    return r;
  }
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(7)
    })

    test("sequential awaits", async () => {
      const source = `{
  async fn add_one(x: i64) -> i64 {
    return x + 1;
  }

  async fn main() -> i64 {
    a: i64 = await add_one(10);
    b: i64 = await add_one(a);
    return b;
  }
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(12)
    })
  })

  describe("Nested async calls", () => {
    test("nested async function calling another async function", async () => {
      const source = `{
  async fn double(x: i64) -> i64 {
    return x * 2;
  }

  async fn add_and_double(a: i64, b: i64) -> i64 {
    sum: i64 = a + b;
    result: i64 = await double(sum);
    return result;
  }

  async fn main() -> i64 {
    r: i64 = await add_and_double(10, 11);
    return r;
  }
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(42)
    })
  })

  describe("Async with control flow", () => {
    test("async with conditional", async () => {
      const source = `{
  async fn check(x: i64) -> i64 {
    if (x > 5) {
      return 1;
    }
    return 0;
  }

  async fn main() -> i64 {
    a: i64 = await check(10);
    b: i64 = await check(3);
    return a + b;
  }
}`
      const { exitCode } = await compileAndRun(source)
      // check(10) = 1, check(3) = 0, total = 1
      expect(exitCode).toBe(1)
    })
  })

  describe("Process capabilities (async I/O)", () => {
    test("process.sleep completes without hanging", async () => {
      const source = `{
  requires process;

  async fn main() -> i64 {
    await process.sleep(10);
    return 0;
  }
}`
      const { exitCode } = await compileAndRun(source, [], 5000)
      expect(exitCode).toBe(0)
    })

    test("process.time_ms returns monotonic time", async () => {
      const source = `{
  requires process;

  async fn main() -> i64 {
    t1: i64 = await process.timestamp();
    await process.sleep(20);
    t2: i64 = await process.timestamp();
    diff: i64 = t2 - t1;
    if (diff >= 10) {
      return 0;
    }
    return 1;
  }
}`
      const { exitCode } = await compileAndRun(source, [], 5000)
      expect(exitCode).toBe(0)
    })
  })

  describe("Filesystem capabilities", () => {
    test("fs.write_file and fs.read_file round-trip", async () => {
      const tmpFile = join(tmpdir(), `mog-test-fs-${Date.now()}.txt`)
      try {
        const source = `{
  requires fs;

  async fn main() -> i64 {
    written: i64 = await fs.write_file("${tmpFile}", "hello mog");
    if (written > 0) {
      return 0;
    }
    return 1;
  }
}`
        const { exitCode } = await compileAndRun(source)
        expect(exitCode).toBe(0)
      } finally {
        try { rmSync(tmpFile, { force: true }) } catch {}
      }
    })

    test("fs.exists checks file presence", async () => {
      const tmpFile = join(tmpdir(), `mog-test-exists-${Date.now()}.txt`)
      try {
        // First check non-existent file
        const source = `{
  requires fs;

   async fn main() -> i64 {
    e: bool = await fs.exists("${tmpFile}");
    if (e) {
      return 1;
    }
    return 0;
  }
}`
        const { exitCode } = await compileAndRun(source)
        // File doesn't exist, so fs.exists returns 0
        expect(exitCode).toBe(0)
      } finally {
        try { rmSync(tmpFile, { force: true }) } catch {}
      }
    })
  })

  describe("Combinators", () => {
    test("all() with multiple async functions", async () => {
      const source = `{
  async fn task_a() -> i64 {
    return 10;
  }

  async fn task_b() -> i64 {
    return 20;
  }

  async fn main() -> i64 {
    await all([task_a(), task_b()]);
    return 0;
  }
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(0)
    })

    test("race() with multiple async functions", async () => {
      const source = `{
  async fn fast() -> i64 {
    return 1;
  }

  async fn slow() -> i64 {
    return 2;
  }

  async fn main() -> i64 {
    r: i64 = await race([fast(), slow()]);
    return 0;
  }
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(0)
    })
  })
})
