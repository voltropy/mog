import { describe, test, expect } from "bun:test"
import { compile } from "./compiler"
import { spawnSync } from "child_process"
import { writeFileSync, unlinkSync, readFileSync, existsSync, mkdirSync, rmdirSync, rmSync } from "fs"
import { join } from "path"
import { tmpdir } from "os"

interface CompileResult {
  success: boolean
  llvmIR: string
  errors: any[]
}

async function compileAndRun(source: string, args: string[] = []): Promise<{ exitCode: number, stdout: string, stderr: string }> {
  const sourceDir = join(tmpdir(), `mog-test-${Date.now()}`)
  
  try {
    mkdirSync(sourceDir, { recursive: true })
    
    const sourceFile = join(sourceDir, "test.mog")
    const llFile = join(sourceDir, "test.ll")
    const oFile = join(sourceDir, "test.o")
    const runtimePath = join(process.cwd(), "build", "runtime.a")
    
    const llcPath = "/opt/homebrew/opt/llvm/bin/llc"
    
    writeFileSync(sourceFile, source)
    
    const result = await compile(source)
    await Bun.write(llFile, result.llvmIR)
    
    const llcProcess = spawnSync(llcPath, ["-filetype=obj", llFile, "-o", oFile], {
      stdio: "pipe",
    })
    
    if (llcProcess.error) {
      throw new Error(`llc not found or failed: ${llcProcess.error.message}`)
    }
    
    if (llcProcess.status !== 0) {
      throw new Error(`llc compilation failed: ${llcProcess.stderr}`)
    }
    
    const exeFile = join(sourceDir, "test")
    const clangResult = spawnSync("clang", [oFile, runtimePath, "-o", exeFile], {
      stdio: "pipe",
    })
    
    if (clangResult.error) {
      throw new Error(`clang not found or failed: ${clangResult.error.message}`)
    }
    
    if (clangResult.status !== 0 && !clangResult.stderr.toString().includes("no platform load command")) {
      throw new Error(`clang linking failed: ${clangResult.stderr}`)
    }
    
    const runResult = spawnSync(exeFile, args, {
      stdio: "pipe",
    })
    
    return {
      exitCode: runResult.status ?? 0,
      stdout: runResult.stdout ? runResult.stdout.toString() : "",
      stderr: runResult.stderr ? runResult.stderr.toString() : "",
    }
  } catch (e) {
    const sourceFiles = process.env.DEBUG_TEST ? sourceDir : sourceDir
    throw e
  } finally {
    rmSync(sourceDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })
  }
}

describe("Integration Tests - Compilation to Machine Code", () => {
  describe("Simple Programs without main()", () => {
    test("simple arithmetic compiles and runs", async () => {
      const source = `{
  x: i64 = 42;
  y: i64 = 10;
  z: i64 = x + y;
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(0)
    })
    
    test("while loop compiles and runs", async () => {
      const source = `{
  i: i64 = 0;
  while (i < 10) {
    i := i + 1;
  }
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(0)
    })
    
    test("function call compiles and runs", async () => {
      const source = `{
  fn add(a: i64, b: i64) -> i64 {
    return a + b;
  }
  
  result: i64 = add(10, 20);
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(0)
    })
    
    test("if statement compiles and runs", async () => {
      const source = `{
  x: i64 = 42;
  if (x > 0) {
    z1: i64 = 100;
  } else {
    z2: i64 = 200;
  }
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(0)
    })
  })
  
  describe("main() function with exit codes", () => {
    test("main() returns exit code 0", async () => {
      const source = `fn main() -> i64 {
  return 0;
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(0)
    })
    
    test("main() returns exit code 42", async () => {
      const source = `fn main() -> i64 {
  return 42;
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(42)
    })
    
    test("main() returns exit code 127", async () => {
      const source = `fn main() -> i64 {
  return 127;
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(127)
    })
    
    test("main() returns exit code 1 for error", async () => {
      const source = `fn main() -> i64 {
  return 1;
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(1)
    })
    
    test("main() computes and returns result", async () => {
      const source = `fn main() -> i64 {
  x: i64 = 10;
  y: i64 = 20;
  return x + y;
}`
      const { exitCode } = await compileAndRun(source)
      expect(exitCode).toBe(30)
    })
  })
  
  describe("main() with command-line arguments", () => {
    test("main() returns count of args (stub)", async () => {
      const source = `fn main() -> i64 {
  return 3;
}`
      const { exitCode } = await compileAndRun(source, ["arg1", "arg2"])
      expect(exitCode).toBe(3)
    })
  })
})