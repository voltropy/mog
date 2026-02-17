import { describe, test, expect } from "bun:test"
import { tokenize } from "./lexer.js"
import { parseTokens } from "./parser.js"
import { SemanticAnalyzer } from "./analyzer.js"
import { generateQBEIR } from "./qbe_codegen.js"
import { parseCapabilityDecl } from "./capability.js"
import { spawnSync } from "child_process"
import { writeFileSync, mkdirSync, rmSync, readFileSync, existsSync } from "fs"
import { join, resolve, dirname } from "path"
import { tmpdir } from "os"

const QBE_PATH = "/opt/homebrew/bin/qbe"
const QBE_TARGET = "arm64_apple"
const RUNTIME_PATH = join(process.cwd(), "build", "runtime.a")

/**
 * Compile a Mog source string through the QBE backend, assemble, link, and run.
 * Returns the captured stdout from the resulting binary.
 *
 * Pipeline:
 *   Mog source → tokenize → parse → analyze → generateQBEIR → QBE IL string
 *   → qbe (arm64_apple) → .s assembly → cc + runtime.a → binary → run → stdout
 */
async function compileAndRunQBE(mogSource: string): Promise<string> {
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
  const sourceDir = join(tmpdir(), `mog-qbe-test-${id}`)

  try {
    mkdirSync(sourceDir, { recursive: true })

    const ssaFile = join(sourceDir, "prog.ssa")
    const asmFile = join(sourceDir, "prog.s")
    const exeFile = join(sourceDir, "prog")

    // Step 1: Mog source → QBE IL
    const tokens = tokenize(mogSource)
    const filtered = tokens.filter(
      (t: any) => t.type !== "WHITESPACE" && t.type !== "NEWLINE" && t.type !== "COMMENT"
    )
    const ast = parseTokens(filtered)
    const analyzer = new SemanticAnalyzer()
    analyzer.analyze(ast)
    const qbeIL = generateQBEIR(ast)

    // Step 2: Write QBE IL to .ssa file
    writeFileSync(ssaFile, qbeIL)

    // Step 3: Run qbe to produce .s assembly (qbe writes to stdout)
    const qbeResult = spawnSync(QBE_PATH, ["-t", QBE_TARGET, ssaFile], {
      stdio: ["pipe", "pipe", "pipe"],
      timeout: 10000,
    })
    if (qbeResult.status !== 0) {
      const stderr = qbeResult.stderr?.toString() ?? ""
      throw new Error(
        `QBE compilation failed (exit ${qbeResult.status}):\n${stderr}\n\nGenerated QBE IL:\n${qbeIL}`
      )
    }
    writeFileSync(asmFile, qbeResult.stdout)

    // Step 4: Assemble and link with runtime
    const ccResult = spawnSync("cc", ["-o", exeFile, asmFile, RUNTIME_PATH, "-lm"], {
      stdio: ["pipe", "pipe", "pipe"],
      timeout: 10000,
    })
    if (ccResult.status !== 0) {
      const stderr = ccResult.stderr?.toString() ?? ""
      // Ignore non-fatal linker warnings (e.g., "no platform load command")
      if (!stderr.includes("no platform load command") || ccResult.status !== 0) {
        // Check if binary was actually produced despite warnings
        const { existsSync } = require("fs")
        if (!existsSync(exeFile)) {
          throw new Error(
            `cc linking failed (exit ${ccResult.status}):\n${stderr}\n\nGenerated QBE IL:\n${qbeIL}`
          )
        }
      }
    }

    // Step 5: Run the binary and capture stdout
    const runResult = spawnSync(exeFile, [], {
      stdio: ["pipe", "pipe", "pipe"],
      timeout: 5000,
    })
    if (runResult.status !== 0) {
      const stderr = runResult.stderr?.toString() ?? ""
      throw new Error(
        `Binary execution failed (exit ${runResult.status}):\nstderr: ${stderr}\nstdout: ${runResult.stdout?.toString() ?? ""}\n\nGenerated QBE IL:\n${qbeIL}`
      )
    }

    return runResult.stdout?.toString() ?? ""
  } finally {
    rmSync(sourceDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })
  }
}

/**
 * Compile Mog source through QBE backend with capability support (process, fs, etc.)
 * Uses the POSIX host from the runtime for built-in capabilities.
 */
async function compileAndRunQBEWithCaps(mogSource: string, timeoutMs = 10000): Promise<{ stdout: string; exitCode: number }> {
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
  const sourceDir = join(tmpdir(), `mog-qbe-cap-${id}`)

  try {
    mkdirSync(sourceDir, { recursive: true })

    const ssaFile = join(sourceDir, "prog.ssa")
    const asmFile = join(sourceDir, "prog.s")
    const exeFile = join(sourceDir, "prog")

    // Parse and analyze with capability declarations
    const tokens = tokenize(mogSource)
    const filtered = tokens.filter(
      (t: any) => t.type !== "WHITESPACE" && t.type !== "NEWLINE" && t.type !== "COMMENT"
    )
    const ast = parseTokens(filtered)

    // Extract capabilities from AST
    const requiredCaps: string[] = []
    const extractCaps = (stmts: any[]) => {
      for (const stmt of stmts) {
        if (stmt.type === "RequiresDeclaration") requiredCaps.push(...stmt.capabilities)
        if (stmt.type === "Block" && stmt.statements) extractCaps(stmt.statements)
      }
    }
    extractCaps(ast.statements)

    // Load .mogdecl files
    const capabilityDecls = new Map<string, any>()
    const capsDir = resolve(process.cwd(), "capabilities")
    for (const capName of requiredCaps) {
      const declPath = join(capsDir, `${capName}.mogdecl`)
      if (existsSync(declPath)) {
        const declSource = readFileSync(declPath, "utf-8")
        const decls = parseCapabilityDecl(declSource)
        const decl = decls.find((d: any) => d.name === capName)
        if (decl) capabilityDecls.set(capName, decl)
      }
    }

    const analyzer = new SemanticAnalyzer()
    analyzer.setCapabilityDecls(capabilityDecls)
    analyzer.analyze(ast)

    const qbeIL = generateQBEIR(ast, undefined, requiredCaps, capabilityDecls)
    writeFileSync(ssaFile, qbeIL)

    // QBE → assembly
    const qbeResult = spawnSync(QBE_PATH, ["-t", QBE_TARGET, ssaFile], {
      stdio: ["pipe", "pipe", "pipe"],
      timeout: 10000,
    })
    if (qbeResult.status !== 0) {
      throw new Error(`QBE failed:\n${qbeResult.stderr?.toString()}\n\nQBE IL:\n${qbeIL}`)
    }
    writeFileSync(asmFile, qbeResult.stdout)

    // Link with runtime (includes posix_host.o)
    const ccResult = spawnSync("cc", ["-o", exeFile, asmFile, RUNTIME_PATH, "-lm"], {
      stdio: ["pipe", "pipe", "pipe"],
      timeout: 10000,
    })
    if (ccResult.status !== 0) {
      if (!existsSync(exeFile)) {
        throw new Error(`cc failed:\n${ccResult.stderr?.toString()}\n\nQBE IL:\n${qbeIL}`)
      }
    }

    const runResult = spawnSync(exeFile, [], {
      stdio: ["pipe", "pipe", "pipe"],
      timeout: timeoutMs,
    })

    return {
      stdout: runResult.stdout?.toString() ?? "",
      exitCode: runResult.status ?? -1,
    }
  } finally {
    rmSync(sourceDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })
  }
}

/**
 * Compile Mog source through QBE backend with a custom C host file for capabilities.
 */
async function compileAndRunQBEWithHost(mogSource: string, hostSource: string, timeoutMs = 10000): Promise<{ stdout: string; exitCode: number }> {
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
  const sourceDir = join(tmpdir(), `mog-qbe-host-${id}`)

  try {
    mkdirSync(sourceDir, { recursive: true })

    const ssaFile = join(sourceDir, "prog.ssa")
    const asmFile = join(sourceDir, "prog.s")
    const hostFile = join(sourceDir, "host.c")
    const hostObjFile = join(sourceDir, "host.o")
    const exeFile = join(sourceDir, "prog")
    const runtimeIncDir = resolve(process.cwd(), "runtime")

    // Parse and analyze with capability declarations
    const tokens = tokenize(mogSource)
    const filtered = tokens.filter(
      (t: any) => t.type !== "WHITESPACE" && t.type !== "NEWLINE" && t.type !== "COMMENT"
    )
    const ast = parseTokens(filtered)

    // Extract capabilities
    const requiredCaps: string[] = []
    const extractCaps = (stmts: any[]) => {
      for (const stmt of stmts) {
        if (stmt.type === "RequiresDeclaration") requiredCaps.push(...stmt.capabilities)
        if (stmt.type === "Block" && stmt.statements) extractCaps(stmt.statements)
      }
    }
    extractCaps(ast.statements)

    // Load .mogdecl files
    const capabilityDecls = new Map<string, any>()
    const capsDir = resolve(process.cwd(), "capabilities")
    for (const capName of requiredCaps) {
      const declPath = join(capsDir, `${capName}.mogdecl`)
      if (existsSync(declPath)) {
        const declSource = readFileSync(declPath, "utf-8")
        const decls = parseCapabilityDecl(declSource)
        const decl = decls.find((d: any) => d.name === capName)
        if (decl) capabilityDecls.set(capName, decl)
      }
    }

    const analyzer = new SemanticAnalyzer()
    analyzer.setCapabilityDecls(capabilityDecls)
    analyzer.analyze(ast)

    const qbeIL = generateQBEIR(ast, undefined, requiredCaps, capabilityDecls)
    writeFileSync(ssaFile, qbeIL)

    // QBE → assembly
    const qbeResult = spawnSync(QBE_PATH, ["-t", QBE_TARGET, ssaFile], {
      stdio: ["pipe", "pipe", "pipe"],
      timeout: 10000,
    })
    if (qbeResult.status !== 0) {
      throw new Error(`QBE failed:\n${qbeResult.stderr?.toString()}\n\nQBE IL:\n${qbeIL}`)
    }
    writeFileSync(asmFile, qbeResult.stdout)

    // Compile host C file
    writeFileSync(hostFile, hostSource)
    const ccHost = spawnSync("cc", ["-c", "-I", runtimeIncDir, hostFile, "-o", hostObjFile], {
      stdio: ["pipe", "pipe", "pipe"],
      timeout: 10000,
    })
    if (ccHost.status !== 0) {
      throw new Error(`Host compilation failed:\n${ccHost.stderr?.toString()}`)
    }

    // Link: assembly + host + runtime
    const ccResult = spawnSync("cc", ["-o", exeFile, asmFile, hostObjFile, RUNTIME_PATH, "-lm"], {
      stdio: ["pipe", "pipe", "pipe"],
      timeout: 10000,
    })
    if (ccResult.status !== 0) {
      if (!existsSync(exeFile)) {
        throw new Error(`cc linking failed:\n${ccResult.stderr?.toString()}\n\nQBE IL:\n${qbeIL}`)
      }
    }

    const runResult = spawnSync(exeFile, [], {
      stdio: ["pipe", "pipe", "pipe"],
      timeout: timeoutMs,
    })

    return {
      stdout: runResult.stdout?.toString() ?? "",
      exitCode: runResult.status ?? -1,
    }
  } finally {
    rmSync(sourceDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })
  }
}

describe("QBE Integration Tests", () => {
  test("print integer literal: println_i64(42)", async () => {
    const output = await compileAndRunQBE(`println_i64(42)`)
    expect(output).toBe("42\n")
  })

  test("arithmetic: println_i64(3 + 4)", async () => {
    const output = await compileAndRunQBE(`println_i64(3 + 4)`)
    expect(output).toBe("7\n")
  })

  test("variable declaration: x := 10; println_i64(x)", async () => {
    const output = await compileAndRunQBE(`x := 10\nprintln_i64(x)`)
    expect(output).toBe("10\n")
  })

  test("if/else: if true { println_i64(1) } else { println_i64(0) }", async () => {
    const output = await compileAndRunQBE(
      `if true { println_i64(1) } else { println_i64(0) }`
    )
    expect(output).toBe("1\n")
  })

  test("while loop: count to 5", async () => {
    const output = await compileAndRunQBE(
      `x := 0\nwhile x < 5 {\n  x = x + 1\n}\nprintln_i64(x)`
    )
    expect(output).toBe("5\n")
  })

  test("function: fn add(a, b) -> int", async () => {
    const output = await compileAndRunQBE(
      `fn add(a: int, b: int) -> int {\n  return a + b;\n}\nprintln_i64(add(3, 4))`
    )
    expect(output).toBe("7\n")
  })

  test("string literal: println(\"hello\")", async () => {
    const output = await compileAndRunQBE(`println("hello")`)
    expect(output).toBe("hello\n")
  })

  test("boolean comparison: println_i64(1 == 1)", async () => {
    const output = await compileAndRunQBE(`println_i64(1 == 1)`)
    expect(output).toBe("1\n")
  })
})

describe("QBE Async Integration Tests", () => {
  test("simple async function returning a value", async () => {
    const output = await compileAndRunQBE(`
async fn compute() -> int {
  return 42;
}

async fn main() -> int {
  x := await compute()
  println_i64(x)
  return 0;
}
`)
    expect(output).toBe("42\n")
  })

  test("async function with arithmetic", async () => {
    const output = await compileAndRunQBE(`
async fn add_async(a: int, b: int) -> int {
  return a + b;
}

async fn main() -> int {
  result := await add_async(10, 32)
  println_i64(result)
  return 0;
}
`)
    expect(output).toBe("42\n")
  })

  test("multiple sequential awaits", async () => {
    const output = await compileAndRunQBE(`
async fn double(x: int) -> int {
  return x * 2;
}

async fn main() -> int {
  a := await double(5)
  b := await double(a)
  println_i64(b)
  return 0;
}
`)
    expect(output).toBe("20\n")
  })

  test("async with sync code between awaits", async () => {
    const output = await compileAndRunQBE(`
async fn identity(x: int) -> int {
  return x;
}

async fn main() -> int {
  a := await identity(10)
  b := a + 5
  c := await identity(b)
  println_i64(c)
  return 0;
}
`)
    expect(output).toBe("15\n")
  })

  test("async function with no internal await", async () => {
    const output = await compileAndRunQBE(`
async fn simple() -> int {
  return 7;
}

async fn main() -> int {
  r := await simple()
  println_i64(r)
  return 0;
}
`)
    expect(output).toBe("7\n")
  })

  test("nested async: async function calling another async function", async () => {
    const output = await compileAndRunQBE(`
async fn double(x: int) -> int {
  return x * 2;
}

async fn add_and_double(a: int, b: int) -> int {
  sum := a + b
  result := await double(sum)
  return result;
}

async fn main() -> int {
  r := await add_and_double(10, 11)
  println_i64(r)
  return 0;
}
`)
    expect(output).toBe("42\n")
  })

  test("async with conditional", async () => {
    const output = await compileAndRunQBE(`
async fn check(x: int) -> int {
  if x > 5 {
    return 1;
  }
  return 0;
}

async fn main() -> int {
  a := await check(10)
  b := await check(3)
  result := a + b
  println_i64(result)
  return 0;
}
`)
    expect(output).toBe("1\n")
  })

  test("async with while loop containing await", async () => {
    const output = await compileAndRunQBE(`
async fn increment(x: int) -> int {
  return x + 1;
}

async fn main() -> int {
  i := 0
  while i < 5 {
    i = await increment(i)
  }
  println_i64(i)
  return 0;
}
`)
    expect(output).toBe("5\n")
  })

  test("recursive async function", async () => {
    const output = await compileAndRunQBE(`
async fn async_factorial(n: int) -> int {
  if n <= 1 {
    return 1;
  }
  sub := await async_factorial(n - 1)
  return n * sub;
}

async fn main() -> int {
  r := await async_factorial(5)
  println_i64(r)
  return 0;
}
`)
    expect(output).toBe("120\n")
  })

  test("spawn: fire-and-forget async function", async () => {
    const output = await compileAndRunQBE(`
async fn compute(x: int) -> int {
  return x * x;
}

async fn main() -> int {
  f := spawn compute(7)
  r := await f
  println_i64(r)
  return 0;
}
`)
    expect(output).toBe("49\n")
  })

  test("three-level nested async chain", async () => {
    const output = await compileAndRunQBE(`
async fn add_one(x: int) -> int {
  return x + 1;
}

async fn add_two(x: int) -> int {
  r := await add_one(x)
  r2 := await add_one(r)
  return r2;
}

async fn add_four(x: int) -> int {
  r := await add_two(x)
  r2 := await add_two(r)
  return r2;
}

async fn main() -> int {
  r := await add_four(10)
  println_i64(r)
  return 0;
}
`)
    expect(output).toBe("14\n")
  })

  test("async pipeline: fetch → transform → validate", async () => {
    const output = await compileAndRunQBE(`
async fn fetch_value() -> int {
  return 10;
}

async fn transform(x: int) -> int {
  return x * 3;
}

async fn validate(x: int) -> int {
  if x > 20 {
    return 1;
  }
  return 0;
}

async fn main() -> int {
  v := await fetch_value()
  t := await transform(v)
  valid := await validate(t)
  println_i64(valid)
  return 0;
}
`)
    expect(output).toBe("1\n")
  })
})

describe("QBE Async Combinators", () => {
  test("all() with multiple async functions", async () => {
    const output = await compileAndRunQBE(`
async fn task_a() -> int {
  return 10;
}

async fn task_b() -> int {
  return 20;
}

async fn main() -> int {
  await all([task_a(), task_b()])
  println("all done")
  return 0;
}
`)
    expect(output).toBe("all done\n")
  })

  test("race() with multiple async functions", async () => {
    const output = await compileAndRunQBE(`
async fn fast() -> int {
  return 1;
}

async fn slow() -> int {
  return 2;
}

async fn main() -> int {
  r := await race([fast(), slow()])
  println("race done")
  return 0;
}
`)
    expect(output).toBe("race done\n")
  })
})

describe("QBE Async Capabilities", () => {
  test("process.sleep completes without hanging", async () => {
    const { stdout, exitCode } = await compileAndRunQBEWithCaps(`
requires process

async fn main() -> int {
  await process.sleep(10)
  println("slept")
  return 0;
}
`, 5000)
    expect(exitCode).toBe(0)
    expect(stdout).toBe("slept\n")
  })

  test("process.timestamp returns monotonic time", async () => {
    const { stdout, exitCode } = await compileAndRunQBEWithCaps(`
requires process

async fn main() -> int {
  t1 := await process.timestamp()
  await process.sleep(20)
  t2 := await process.timestamp()
  diff := t2 - t1
  if diff >= 10 {
    println("ok")
  } else {
    println("fail")
  }
  return 0;
}
`, 5000)
    expect(exitCode).toBe(0)
    expect(stdout).toBe("ok\n")
  })

  test("sync capability: process.cwd returns a value", async () => {
    const { exitCode } = await compileAndRunQBEWithCaps(`
requires process

async fn main() -> int {
  d := await process.cwd()
  println("got cwd")
  return 0;
}
`)
    expect(exitCode).toBe(0)
  })

  test("fs.write_file and fs.exists round-trip", async () => {
    const tmpFile = join(tmpdir(), `mog-qbe-test-fs-${Date.now()}.txt`)
    try {
      const { stdout, exitCode } = await compileAndRunQBEWithCaps(`
requires fs

async fn main() -> int {
  written := await fs.write_file("${tmpFile}", "hello qbe")
  if written > 0 {
    println("written")
  }
  return 0;
}
`)
      expect(exitCode).toBe(0)
      expect(stdout).toBe("written\n")
    } finally {
      try { rmSync(tmpFile, { force: true }) } catch {}
    }
  })
})

describe("QBE Custom Host Async (env.delay_square)", () => {
  const HOST_C = `
#include "mog.h"
#include "mog_async.h"
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

static MogValue host_env_delay_square(MogVM *vm, MogArgs *args) {
    (void)vm;
    int64_t value = mog_arg_int(args, 0);
    int64_t delay_ms = mog_arg_int(args, 1);
    int64_t result = value * value;

    MogEventLoop *loop = mog_loop_get_global();
    if (loop) {
        MogFuture *future = mog_future_new();
        mog_loop_add_timer_with_value(loop, (uint64_t)delay_ms, future, result);
        return mog_int((int64_t)(intptr_t)future);
    } else {
        return mog_int(result);
    }
}

static MogValue host_env_get_name(MogVM *vm, MogArgs *args) {
    (void)vm; (void)args;
    return mog_string("TestHost");
}

static MogValue host_env_log(MogVM *vm, MogArgs *args) {
    (void)vm;
    const char *message = mog_arg_string(args, 0);
    printf("%s\\n", message);
    return mog_none();
}

static const MogCapEntry env_functions[] = {
    { "delay_square", host_env_delay_square },
    { "get_name",     host_env_get_name },
    { "log",          host_env_log },
    { NULL, NULL }
};

__attribute__((constructor))
static void setup_mog_vm(void) {
    srand((unsigned int)time(NULL));
    MogVM *vm = mog_vm_new();
    mog_register_capability(vm, "env", env_functions);
    mog_vm_set_global(vm);
}
`

  test("basic async host function: delay_square(3, 10)", async () => {
    const { stdout, exitCode } = await compileAndRunQBEWithHost(`
requires env

async fn main() -> int {
  r := await env.delay_square(3, 10)
  println_i64(r)
  return 0;
}
`, HOST_C, 5000)
    expect(exitCode).toBe(0)
    expect(stdout).toBe("9\n")
  })

  test("multiple async host calls", async () => {
    const { stdout, exitCode } = await compileAndRunQBEWithHost(`
requires env

async fn main() -> int {
  a := await env.delay_square(2, 10)
  b := await env.delay_square(3, 10)
  c := a + b
  println_i64(c)
  return 0;
}
`, HOST_C, 5000)
    expect(exitCode).toBe(0)
    expect(stdout).toBe("13\n")
  })

  test("sync host call: env.get_name", async () => {
    const { stdout, exitCode } = await compileAndRunQBEWithHost(`
requires env

async fn main() -> int {
  name := await env.get_name()
  println(name)
  return 0;
}
`, HOST_C, 5000)
    expect(exitCode).toBe(0)
    expect(stdout).toBe("TestHost\n")
  })

  test("mixed sync and async host calls", async () => {
    const { stdout, exitCode } = await compileAndRunQBEWithHost(`
requires env

async fn main() -> int {
  name := await env.get_name()
  println(name)
  r := await env.delay_square(5, 10)
  println_i64(r)
  return 0;
}
`, HOST_C, 5000)
    expect(exitCode).toBe(0)
    expect(stdout).toBe("TestHost\n25\n")
  })

  test("async host call after user async function", async () => {
    const { stdout, exitCode } = await compileAndRunQBEWithHost(`
requires env

async fn compute(x: int) -> int {
  return x + 1;
}

async fn main() -> int {
  a := await compute(5)
  println_i64(a)
  b := await env.delay_square(a, 10)
  println_i64(b)
  return 0;
}
`, HOST_C, 5000)
    expect(exitCode).toBe(0)
    expect(stdout).toBe("6\n36\n")
  })
})
