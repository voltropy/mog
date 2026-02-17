import { describe, test, expect } from "bun:test"
import { tokenize } from "./lexer.js"
import { parseTokens } from "./parser.js"
import { SemanticAnalyzer } from "./analyzer.js"
import { generateQBEIR } from "./qbe_codegen.js"
import { spawnSync } from "child_process"
import { writeFileSync, mkdirSync, rmSync } from "fs"
import { join } from "path"
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
})
