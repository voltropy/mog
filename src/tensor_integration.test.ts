import { describe, test, expect } from "bun:test"
import { compile } from "./compiler"
import { spawnSync } from "child_process"
import { writeFileSync, mkdirSync, rmSync } from "fs"
import { join } from "path"
import { tmpdir } from "os"

async function compileAndRun(source: string, args: string[] = []): Promise<{ exitCode: number, stdout: string, stderr: string }> {
  const sourceDir = join(tmpdir(), `mog-tensor-test-${Date.now()}-${Math.random().toString(36).slice(2)}`)

  try {
    mkdirSync(sourceDir, { recursive: true })

    const sourceFile = join(sourceDir, "test.mog")
    const llFile = join(sourceDir, "test.ll")
    const oFile = join(sourceDir, "test.o")
    const runtimePath = join(process.cwd(), "build", "runtime.a")

    const llcPath = "/opt/homebrew/opt/llvm/bin/llc"

    writeFileSync(sourceFile, source)

    const result = await compile(source)
    if (result.errors && result.errors.length > 0) {
      throw new Error(`Compilation errors: ${result.errors.map((e: any) => e.message).join(", ")}`)
    }
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
    throw e
  } finally {
    rmSync(sourceDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })
  }
}

describe("Tensor Integration Tests", () => {
  describe("Tensor Creation", () => {
    test("create tensor with shape (zeros)", async () => {
      const source = `{
  t := tensor<f32>([2, 3]);
  tensor_print(t);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout).toContain("shape=[2, 3]")
      expect(stdout).toContain("0.0000")
    })

    test("create tensor with data", async () => {
      const source = `{
  t := tensor<f32>([2, 3], [1, 2, 3, 4, 5, 6]);
  tensor_print(t);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout).toContain("1.0000")
      expect(stdout).toContain("6.0000")
      expect(stdout).toContain("shape=[2, 3]")
    })

    test("create 1D tensor", async () => {
      const source = `{
  t := tensor<f32>([4], [10, 20, 30, 40]);
  tensor_print(t);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout).toContain("10.0000")
      expect(stdout).toContain("40.0000")
      expect(stdout).toContain("shape=[4]")
    })
  })

  describe("Elementwise Operations", () => {
    test("tensor addition", async () => {
      const source = `{
  a := tensor<f32>([3], [1, 2, 3]);
  b := tensor<f32>([3], [4, 5, 6]);
  c := a + b;
  tensor_print(c);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout).toContain("5.0000")
      expect(stdout).toContain("7.0000")
      expect(stdout).toContain("9.0000")
    })

    test("tensor subtraction", async () => {
      const source = `{
  a := tensor<f32>([3], [10, 20, 30]);
  b := tensor<f32>([3], [1, 2, 3]);
  c := a - b;
  tensor_print(c);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout).toContain("9.0000")
      expect(stdout).toContain("18.0000")
      expect(stdout).toContain("27.0000")
    })

    test("tensor elementwise multiply", async () => {
      const source = `{
  a := tensor<f32>([3], [2, 3, 4]);
  b := tensor<f32>([3], [5, 6, 7]);
  c := a * b;
  tensor_print(c);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout).toContain("10.0000")
      expect(stdout).toContain("18.0000")
      expect(stdout).toContain("28.0000")
    })
  })

  describe("Matrix Multiply", () => {
    test("matmul 2x3 * 3x2 = 2x2", async () => {
      const source = `{
  a := tensor<f32>([2, 3], [1, 2, 3, 4, 5, 6]);
  b := tensor<f32>([3, 2], [7, 8, 9, 10, 11, 12]);
  c := matmul(a, b);
  tensor_print(c);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      // [1,2,3]*[7,9,11; 8,10,12] = [1*7+2*9+3*11, 1*8+2*10+3*12; ...]
      // Row 0: 7+18+33=58, 8+20+36=64
      // Row 1: 28+45+66=139, 32+50+72=154
      expect(stdout).toContain("58.0000")
      expect(stdout).toContain("64.0000")
      expect(stdout).toContain("139.0000")
      expect(stdout).toContain("154.0000")
      expect(stdout).toContain("shape=[2, 2]")
    })

    test("matmul as method call", async () => {
      const source = `{
  a := tensor<f32>([2, 2], [1, 2, 3, 4]);
  b := tensor<f32>([2, 2], [5, 6, 7, 8]);
  c := a.matmul(b);
  tensor_print(c);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      // [1*5+2*7, 1*6+2*8] = [19, 22]
      // [3*5+4*7, 3*6+4*8] = [43, 50]
      expect(stdout).toContain("19.0000")
      expect(stdout).toContain("22.0000")
      expect(stdout).toContain("43.0000")
      expect(stdout).toContain("50.0000")
    })
  })

  describe("Reductions", () => {
    test("sum reduction", async () => {
      const source = `{
  t := tensor<f32>([4], [1, 2, 3, 4]);
  s := t.sum();
  println_f64(s);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toContain("10")
    })

    test("mean reduction", async () => {
      const source = `{
  t := tensor<f32>([4], [2, 4, 6, 8]);
  m := t.mean();
  println_f64(m);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toContain("5")
    })
  })

  describe("Tensor Print", () => {
    test("print 2D tensor", async () => {
      const source = `{
  t := tensor<f32>([2, 2], [1, 2, 3, 4]);
  tensor_print(t);
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout).toContain("tensor(")
      expect(stdout).toContain("1.0000")
      expect(stdout).toContain("4.0000")
      expect(stdout).toContain("shape=[2, 2]")
    })

    test("print via method call", async () => {
      const source = `{
  t := tensor<f32>([3], [10, 20, 30]);
  t.print();
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout).toContain("tensor(")
      expect(stdout).toContain("10.0000")
      expect(stdout).toContain("30.0000")
    })
  })
})
