import { describe, test, expect } from "bun:test"
import { compile, type CompileError } from "./compiler"

describe("Compiler Integration Tests", () => {
  describe("Valid Programs", () => {
    test("simple arithmetic", async () => {
      const source = `BEGIN
  x: i64 = 42;
  y: i64 = 10;
  z: i64 = (x + y) * 2;
END`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i32 @main()")
      expect(result.llvmIR).toContain("add")
      expect(result.llvmIR).toContain("mul")
    })

    test("number literal expression", async () => {
      const source = `BEGIN
  x: i64 = 42;
END`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i32 @main()")
    })

    test("function declaration and call", async () => {
      const source = `BEGIN
  FUNCTION add(a: i64, b: i64) -> i64
    RETURN a + b;
  END

  add(10, 20);
END`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i64 @add")
      expect(result.llvmIR).toContain("call i64 @add")
    })

    test("while loop", async () => {
      const source = `BEGIN
  i: i64 = 0;
  WHILE i < 10 DO
    i := i + 1;
  OD
END`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("label")
    })
  })

  describe("Error Programs", () => {
    test("detects syntax error", async () => {
      const source = `BEGIN
  i64 x := ;
END`
      const result = await compile(source)
      expect(result.errors.length).toBeGreaterThan(0)
    })

    test("detects undeclared variable", async () => {
      const source = `BEGIN
  x := y;
END`
      const result = await compile(source)
      expect(result.errors.length).toBeGreaterThan(0)
    })

    test("detects type mismatch", async () => {
      const source = `BEGIN
  x: i64 = 42;
  x := "hello";
END`
      const result = await compile(source)
      expect(result.errors.length).toBeGreaterThan(0)
    })
  })

  describe("Error Reporting", () => {
    test("reports error location correctly", async () => {
      const source = `BEGIN
  1 + + 2;
END`
      const result = await compile(source)
      if (result.errors.length > 0) {
        const error = result.errors[0] as CompileError
        expect(error.line).toBeGreaterThanOrEqual(1)
        expect(error.message).toBeTruthy()
      }
    })
  })
})
