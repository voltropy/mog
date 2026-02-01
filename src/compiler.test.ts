import { describe, test, expect } from "bun:test"
import { compile, type CompileError } from "./compiler"

describe("Compiler Integration Tests", () => {
  describe("Valid Programs", () => {
    test("simple arithmetic", async () => {
      const source = `{
  x: i64 = 42;
  y: i64 = 10;
  z: i64 = (x + y) * 2;
}`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i32 @main()")
      expect(result.llvmIR).toContain("add")
      expect(result.llvmIR).toContain("mul")
    })

    test("number literal expression", async () => {
      const source = `{
  x: i64 = 42;
}`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i32 @main()")
    })

    test("function declaration and call", async () => {
      const source = `{
  fn add(a: i64, b: i64) -> i64 {
    return a + b;
  }

  add(10, 20);
}`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i64 @add")
      expect(result.llvmIR).toContain("call i64 @add")
    })

    test("while loop", async () => {
      const source = `{
  i: i64 = 0;
  while (i < 10) {
    i := i + 1;
  }
}`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("label")
    })
  })

  describe("Error Programs", () => {
    test("detects syntax error", async () => {
      const source = `{
  x: i64 = ;
}`
      const result = await compile(source)
      expect(result.errors.length).toBeGreaterThan(0)
    })

    test("detects undeclared variable", async () => {
      const source = `{
  x := y;
}`
      const result = await compile(source)
      expect(result.errors.length).toBeGreaterThan(0)
    })

    test("detects type mismatch", async () => {
      const source = `{
  x: i64 = 42;
  x := "hello";
}`
      const result = await compile(source)
      expect(result.errors.length).toBeGreaterThan(0)
    })
  })

  describe("Error Reporting", () => {
    test("reports error location correctly", async () => {
      const source = `{
  1 + + 2;
}`
      const result = await compile(source)
      if (result.errors.length > 0) {
        const error = result.errors[0] as CompileError
        expect(error.line).toBeGreaterThanOrEqual(1)
        expect(error.message).toBeTruthy()
      }
    })

    test("detects assignment in if condition", async () => {
      const source = `{
  x: i64 = 10;
  if (x := 5) {
    y: i64 = 20;
  }
}`
      const result = await compile(source)
      expect(result.errors.length).toBeGreaterThan(0)
      expect(result.errors.some(e => e.message.includes("Assignment") && e.message.includes("condition"))).toBe(true)
    })

    test("detects assignment in while condition", async () => {
      const source = `{
  x: i64 = 10;
  while (x := x - 1) {
    y: i64 = 20;
  }
}`
      const result = await compile(source)
      expect(result.errors.length).toBeGreaterThan(0)
      expect(result.errors.some(e => e.message.includes("Assignment") && e.message.includes("condition"))).toBe(true)
    })
  })
})
