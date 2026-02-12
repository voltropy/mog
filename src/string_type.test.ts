import { describe, test, expect } from "bun:test"
import { compile } from "./compiler"
import { tokenize } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"
import {
  StringType,
  PointerType,
  ArrayType,
  UnsignedType,
  isStringType,
  sameType,
  compatibleTypes,
  stringType,
} from "./types"
import { spawnSync } from "child_process"
import { writeFileSync, mkdirSync, rmSync } from "fs"
import { join } from "path"
import { tmpdir } from "os"

async function compileAndRun(source: string, args: string[] = []): Promise<{ exitCode: number, stdout: string, stderr: string }> {
  const sourceDir = join(tmpdir(), `mog-test-${Date.now()}-${Math.random().toString(36).slice(2)}`)
  
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
      throw new Error(`Compilation errors: ${JSON.stringify(result.errors)}`)
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

describe("String Type", () => {

  describe("Type system", () => {
    test("StringType class exists and has correct type field", () => {
      const st = new StringType()
      expect(st.type).toBe("StringType")
      expect(st.toString()).toBe("string")
    })

    test("isStringType helper works", () => {
      expect(isStringType(new StringType())).toBe(true)
      expect(isStringType(new PointerType())).toBe(false)
      expect(isStringType(new ArrayType(new UnsignedType("u8"), []))).toBe(false)
    })

    test("stringType constant is StringType", () => {
      expect(stringType).toBeInstanceOf(StringType)
    })

    test("sameType for StringType", () => {
      expect(sameType(new StringType(), new StringType())).toBe(true)
      expect(sameType(new StringType(), new PointerType())).toBe(false)
    })

    test("StringType is compatible with PointerType (bidirectional)", () => {
      expect(compatibleTypes(new StringType(), new PointerType())).toBe(true)
      expect(compatibleTypes(new PointerType(), new StringType())).toBe(true)
    })
  })

  describe("Lexer", () => {
    test("'string' is tokenized as TYPE", () => {
      const tokens = tokenize("name: string")
      const typeToken = tokens.find(t => t.value === "string")
      expect(typeToken).toBeDefined()
      expect(typeToken!.type).toBe("TYPE")
    })
  })

  describe("Parser", () => {
    test("string type annotation is parsed", () => {
      const tokens = tokenize(`fn main() -> i64 { name: string = "Alice"; return 0; }`).filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
      const ast = parseTokens(tokens)
      expect(ast).toBeDefined()
      const mainFn = (ast as any).statements[0]
      const varDecl = mainFn.body.statements[0]
      expect(varDecl.type).toBe("VariableDeclaration")
      expect(varDecl.varType).toBeDefined()
      expect(varDecl.varType.type).toBe("StringType")
    })
  })

  describe("Analyzer", () => {
    test("string literal infers as StringType", async () => {
      const source = `fn main() -> i64 { name := "Alice"; return 0; }`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
    })

    test("f-string infers as StringType", async () => {
      const source = `fn main() -> i64 { name := "Alice"; greeting := f"Hello, {name}!"; return 0; }`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
    })

    test("string methods work on StringType variables", async () => {
      const source = `fn main() -> i64 {
  name: string = "Alice";
  upper := name.upper();
  lower := name.lower();
  trimmed := name.trim();
  return 0;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
    })

    test("str() builtin compiles without errors", async () => {
      const source = `fn main() -> i64 {
  s := str(42);
  return 0;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
    })
  })

  describe("Backward compatibility", () => {
    test("ptr variables can still hold strings", async () => {
      const source = `fn main() -> i64 {
  s: ptr = "hello";
  println_string(s);
  return 0;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
    })

    test("[u8] variables still work for strings", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello";
  println_string(s);
  return 0;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
    })
  })

  describe("Integration", () => {
    test("string type annotation with print", async () => {
      const source = `fn main() -> i64 {
  name: string = "Alice";
  println_string(name);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("Alice")
    })

    test("string methods on string-typed variable", async () => {
      const source = `fn main() -> i64 {
  name: string = "hello";
  result: string = name.upper();
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("HELLO")
    })

    test("f-string returns string type", async () => {
      const source = `fn main() -> i64 {
  name: string = "World";
  greeting := f"Hello, {name}!";
  println_string(greeting);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("Hello, World!")
    })

    test("string .len works", async () => {
      const source = `fn main() -> i64 {
  name: string = "Alice";
  len := name.len;
  println_i64(len);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("5")
    })

    test("str(42) converts int to string", async () => {
      const source = `fn main() -> i64 {
  s := str(42);
  println_string(s);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("42")
    })

    test("str(3.14) converts float to string", async () => {
      const source = `fn main() -> i64 {
  x: f64 = 3.14;
  s := str(x);
  println_string(s);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toContain("3.14")
    })

    test("inferred string type with methods", async () => {
      const source = `fn main() -> i64 {
  name := "hello world";
  result := name.upper();
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("HELLO WORLD")
    })
  })
})
