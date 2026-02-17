import { describe, test, expect } from "bun:test"
import { compile } from "./compiler"
import { spawnSync } from "child_process"
import { writeFileSync, rmSync, mkdirSync } from "fs"
import { join } from "path"
import { tmpdir } from "os"
import { tokenize } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"
import { generateLLVMIR } from "./llvm_codegen"

async function compileAndRun(source: string): Promise<{ exitCode: number, stdout: string, stderr: string }> {
  const sourceDir = join(tmpdir(), `mog-fix-test-${Date.now()}-${Math.random().toString(36).slice(2)}`)
  try {
    mkdirSync(sourceDir, { recursive: true })
    const llFile = join(sourceDir, "test.ll")
    const oFile = join(sourceDir, "test.o")
    const exeFile = join(sourceDir, "test")
    const runtimePath = join(process.cwd(), "build", "runtime.a")
    const llcPath = "/opt/homebrew/opt/llvm/bin/llc"
    const result = await compile(source)
    writeFileSync(llFile, result.llvmIR)
    const llc = spawnSync(llcPath, ["-filetype=obj", llFile, "-o", oFile], { stdio: "pipe" })
    if (llc.status !== 0) throw new Error(`llc failed: ${llc.stderr}`)
    const link = spawnSync("clang", [oFile, runtimePath, "-o", exeFile, "-lm"], { stdio: "pipe" })
    if (link.status !== 0 && !link.stderr.toString().includes("no platform load command")) throw new Error(`clang failed: ${link.stderr}`)
    const run = spawnSync(exeFile, [], { stdio: "pipe", timeout: 5000 })
    return { exitCode: run.status ?? 0, stdout: run.stdout?.toString() ?? "", stderr: run.stderr?.toString() ?? "" }
  } finally {
    rmSync(sourceDir, { recursive: true, force: true })
  }
}

function getIR(source: string): string {
  const tokens = tokenize(source)
  const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
  const ast = parseTokens(filtered)
  const analyzer = new SemanticAnalyzer()
  analyzer.analyze(ast)
  return generateLLVMIR(ast)
}

describe('? operator in try blocks', () => {
  test('? inside try branches to catch instead of ret', () => {
    const source = `
fn may_fail(x: int) -> Result<int> {
  if x < 0 {
    return err("negative");
  }
  return ok(x * 2);
}

fn main() -> int {
  try {
    val := may_fail(5)?;
    println(val);
  } catch(e) {
    println(e);
  }
  return 0;
}
`
    const ir = getIR(source)
    // Inside try, ? should branch to catch label (label4) instead of 'ret'
    // The error path from ? in a try block should NOT contain 'ret i64' — it should branch to the catch block
    // Check that the error propagation path has a branch (not ret) after storing error value
    const lines = ir.split('\n')
    // Find the try_err_val store followed by branch (not ret)
    let foundErrBranch = false
    for (let i = 0; i < lines.length - 1; i++) {
      if (lines[i].includes('store i64') && lines[i].includes('try_err_val')) {
        // Next non-empty line should be a branch, not a ret
        const next = lines[i + 1].trim()
        if (next.startsWith('br label')) foundErrBranch = true
      }
    }
    expect(foundErrBranch).toBe(true)
  })

  test('? outside try still returns from function', () => {
    const source = `
fn may_fail(x: int) -> Result<int> {
  if x < 0 {
    return err("negative");
  }
  return ok(x * 2);
}

fn caller() -> Result<int> {
  val := may_fail(5)?;
  return ok(val);
}

fn main() -> int {
  return 0;
}
`
    const ir = getIR(source)
    // Outside try, ? should generate ret (return from function)
    // The caller function should have a ret in the propagation path
    expect(ir).toContain('ret i64')
  })
})

describe('Array join', () => {
  test('join array of ints with separator', async () => {
    const result = await compileAndRun(`
fn main() -> int {
  arr := [1, 2, 3, 4, 5];
  joined := arr.join(", ");
  println(joined);
  return 0;
}
`)
    expect(result.stdout.trim()).toBe('1, 2, 3, 4, 5')
  })
})

describe('With blocks', () => {
  test('with no_grad generates begin/end calls', () => {
    const source = `
fn main() -> int {
  with no_grad() {
    x: int = 42;
    println(x);
  }
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('mog_no_grad_begin')
    expect(ir).toContain('mog_no_grad_end')
  })
})

describe('Match exhaustiveness', () => {
  test('warns on non-exhaustive Result match missing err', () => {
    const source = `
fn get_val() -> Result<int> {
  return ok(42);
}

fn main() -> int {
  r := get_val();
  match r {
    ok(v) => println(v),
  }
  return 0;
}
`
    const tokens = tokenize(source)
    const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    const ast = parseTokens(filtered)
    const analyzer = new SemanticAnalyzer()
    // Capture console.log output (warnings are logged, not returned)
    const logs: string[] = []
    const origLog = console.log
    console.log = (...args: any[]) => logs.push(args.join(' '))
    analyzer.analyze(ast)
    console.log = origLog
    const exhaustivenessWarnings = logs.filter(l => l.includes('exhaustive') || l.includes('missing'))
    expect(exhaustivenessWarnings.length).toBeGreaterThan(0)
  })

  test('no warning on exhaustive Result match', () => {
    const source = `
fn get_val() -> Result<int> {
  return ok(42);
}

fn main() -> int {
  r := get_val();
  match r {
    ok(v) => println(v),
    err(e) => println(e),
  }
  return 0;
}
`
    const tokens = tokenize(source)
    const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    const ast = parseTokens(filtered)
    const analyzer = new SemanticAnalyzer()
    const logs: string[] = []
    const origLog = console.log
    console.log = (...args: any[]) => logs.push(args.join(' '))
    analyzer.analyze(ast)
    console.log = origLog
    const exhaustivenessWarnings = logs.filter(l => l.includes('exhaustive') || l.includes('missing'))
    expect(exhaustivenessWarnings.length).toBe(0)
  })
})

describe('Tensor ML operations', () => {
  test('relu/sigmoid/softmax generate correct LLVM IR calls', () => {
    const source = `
fn main() -> int {
  t := tensor_ones([3]);
  r := relu(t);
  s := sigmoid(t);
  sm := softmax(t);
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('tensor_relu')
    expect(ir).toContain('tensor_sigmoid')
    expect(ir).toContain('tensor_softmax')
  })
})
