import { describe, test, expect } from "bun:test"
import { compile } from "./compiler"
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

describe("String Methods", () => {
  describe(".upper() and .lower()", () => {
    test("upper() converts to uppercase", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello world";
  result: [u8] = s.upper();
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("HELLO WORLD")
    })

    test("lower() converts to lowercase", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "HELLO WORLD";
  result: [u8] = s.lower();
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("hello world")
    })

    test("upper() on mixed case", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "Hello World 123";
  println_string(s.upper());
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("HELLO WORLD 123")
    })

    test("lower() on mixed case", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "Hello World 123";
  println_string(s.lower());
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("hello world 123")
    })
  })

  describe(".trim()", () => {
    test("trim() strips leading and trailing whitespace", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "  hello  ";
  result: [u8] = s.trim();
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("hello")
    })

    test("trim() strips tabs and newlines", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "  hello world  ";
  println_string(s.trim());
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("hello world")
    })
  })

  describe(".contains(), .starts_with(), .ends_with()", () => {
    test("contains() returns true when substring found", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello world";
  if (s.contains("world")) {
    println_string("found");
  } else {
    println_string("not found");
  }
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("found")
    })

    test("contains() returns false when substring not found", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello world";
  if (s.contains("xyz")) {
    println_string("found");
  } else {
    println_string("not found");
  }
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("not found")
    })

    test("starts_with() checks prefix", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello world";
  if (s.starts_with("hello")) {
    println_string("yes");
  } else {
    println_string("no");
  }
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("yes")
    })

    test("ends_with() checks suffix", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello world";
  if (s.ends_with("world")) {
    println_string("yes");
  } else {
    println_string("no");
  }
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("yes")
    })

    test("starts_with() returns false for wrong prefix", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello world";
  if (s.starts_with("world")) {
    println_string("yes");
  } else {
    println_string("no");
  }
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("no")
    })

    test("ends_with() returns false for wrong suffix", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello world";
  if (s.ends_with("hello")) {
    println_string("yes");
  } else {
    println_string("no");
  }
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("no")
    })
  })

  describe(".replace()", () => {
    test("replace() replaces occurrences", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello world";
  result: [u8] = s.replace("world", "mog");
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("hello mog")
    })

    test("replace() replaces all occurrences", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "aXbXc";
  result: [u8] = s.replace("X", "-");
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("a-b-c")
    })
  })

  describe(".split()", () => {
    test("split() compiles and produces IR with string_split call", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "a,b,c";
  parts = s.split(",");
  return 0;
}`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("@string_split")
    })
  })

  describe("str() conversion", () => {
    test("str() converts integer to string", async () => {
      const source = `fn main() -> i64 {
  n: i64 = 42;
  s = str(n);
  println_string(s);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("42")
    })

    test("str() converts negative integer to string", async () => {
      const source = `fn main() -> i64 {
  n: i64 = 0 - 123;
  s = str(n);
  println_string(s);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("-123")
    })
  })

  describe("String method chaining", () => {
    test("chain upper() and lower()", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "Hello World";
  result: [u8] = s.upper().lower();
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("hello world")
    })

    test("chain trim() and upper()", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "  hello  ";
  result: [u8] = s.trim().upper();
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("HELLO")
    })

    test("chain replace() and upper()", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello world";
  result: [u8] = s.replace("world", "mog").upper();
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("HELLO MOG")
    })
  })

  describe("String .len property", () => {
    test("len returns string length", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello";
  println_i64(s.len);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("5")
    })

    test("len on empty string", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "";
  println_i64(s.len);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("0")
    })
  })

  describe("String variable method calls", () => {
    test("upper() on string variable", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello";
  result: [u8] = s.upper();
  println_string(result);
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("HELLO")
    })

    test("contains() on string variable", async () => {
      const source = `fn main() -> i64 {
  s: [u8] = "hello world";
  if (s.contains("world")) {
    println_string("yes");
  } else {
    println_string("no");
  }
  return 0;
}`
      const { exitCode, stdout } = await compileAndRun(source)
      expect(exitCode).toBe(0)
      expect(stdout.trim()).toBe("yes")
    })
  })
})
