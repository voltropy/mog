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
import { generateQBEIR } from "./qbe_codegen"

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

describe('Math builtins', () => {
  test('single-arg math builtins generate correct LLVM IR calls', () => {
    const source = `
fn main() -> int {
  x := 1.0;
  a := sqrt(x);
  b := sin(x);
  c := cos(x);
  d := tan(x);
  e := asin(x);
  f := acos(x);
  g := exp(x);
  h := log(x);
  i := log2(x);
  j := floor(x);
  k := ceil(x);
  l := round(x);
  m := abs(x);
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('@llvm.sqrt.f64')
    expect(ir).toContain('@llvm.sin.f64')
    expect(ir).toContain('@llvm.cos.f64')
    expect(ir).toContain('@tan')
    expect(ir).toContain('@asin')
    expect(ir).toContain('@acos')
    expect(ir).toContain('@llvm.exp.f64')
    expect(ir).toContain('@llvm.log.f64')
    expect(ir).toContain('@llvm.log2.f64')
    expect(ir).toContain('@llvm.floor.f64')
    expect(ir).toContain('@llvm.ceil.f64')
    expect(ir).toContain('@llvm.round.f64')
    expect(ir).toContain('@llvm.fabs.f64')
  })

  test('two-arg math builtins generate correct LLVM IR calls', () => {
    const source = `
fn main() -> int {
  x := 2.0;
  y := 3.0;
  a := pow(x, y);
  b := atan2(x, y);
  c := min(x, y);
  d := max(x, y);
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('@llvm.pow.f64')
    expect(ir).toContain('@atan2')
    expect(ir).toContain('@llvm.minnum.f64')
    expect(ir).toContain('@llvm.maxnum.f64')
  })

  test('math library function declarations are present', () => {
    const source = `
fn main() -> int {
  x := 1.0;
  a := tan(x);
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('declare double @tan(double)')
    expect(ir).toContain('declare double @asin(double)')
    expect(ir).toContain('declare double @acos(double)')
    expect(ir).toContain('declare double @atan2(double, double)')
  })
})

describe('println(str()) dispatches to println_string', () => {
  test('str() on float generates println_string not println_i64', () => {
    const source = `
fn main() -> int {
  x: float = 3.14;
  println(str(x));
  return 0;
}
`
    const ir = getIR(source)
    // str() returns a string (ptr), so println should dispatch to println_string
    expect(ir).toContain('println_string')
    expect(ir).not.toContain('call void @println_i64')
  })

  test('str() on int generates println_string', () => {
    const source = `
fn main() -> int {
  x := 42;
  println(str(x));
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('println_string')
  })
})

describe('String slice', () => {
  test('string slice generates string_slice call in IR', () => {
    const source = `
fn main() -> int {
  s := "hello world";
  sub := s[0:5];
  println(sub);
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('@string_slice')
  })

  test('string slice end-to-end', async () => {
    const source = `
fn main() -> int {
  s := "hello world";
  sub := s[0:5];
  println(sub);
  return 0;
}
`
    const result = await compileAndRun(source)
    expect(result.exitCode).toBe(0)
    expect(result.stdout.trim()).toBe("hello")
  })

  test('string slice with variables', async () => {
    const source = `
fn main() -> int {
  s := "abcdef";
  start := 2;
  end := 5;
  println(s[start:end]);
  return 0;
}
`
    const result = await compileAndRun(source)
    expect(result.exitCode).toBe(0)
    expect(result.stdout.trim()).toBe("cde")
  })
})

describe('String concatenation with +', () => {
  test('string + string generates string_concat call in IR', () => {
    const source = `
fn main() -> int {
  a := "hello";
  b := " world";
  c := a + b;
  println(c);
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('@string_concat')
  })

  test('string + string end-to-end', async () => {
    const source = `
fn main() -> int {
  a := "hello";
  b := " world";
  c := a + b;
  println(c);
  return 0;
}
`
    const result = await compileAndRun(source)
    expect(result.exitCode).toBe(0)
    expect(result.stdout.trim()).toBe("hello world")
  })

  test('string literal + literal', async () => {
    const source = `
fn main() -> int {
  c := "foo" + "bar";
  println(c);
  return 0;
}
`
    const result = await compileAndRun(source)
    expect(result.exitCode).toBe(0)
    expect(result.stdout.trim()).toBe("foobar")
  })

  test('chained string concatenation', async () => {
    const source = `
fn main() -> int {
  a := "a" + "b" + "c";
  println(a);
  return 0;
}
`
    const result = await compileAndRun(source)
    expect(result.exitCode).toBe(0)
    expect(result.stdout.trim()).toBe("abc")
  })
})

describe('parse_int and parse_float', () => {
  test('parse_int generates correct IR', () => {
    const source = `
fn main() -> int {
  s := "42";
  n := parse_int(s);
  println(n);
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('@parse_int')
  })

  test('parse_int end-to-end', async () => {
    const source = `
fn main() -> int {
  s := "123";
  n := parse_int(s);
  println(n);
  return 0;
}
`
    const result = await compileAndRun(source)
    expect(result.exitCode).toBe(0)
    expect(result.stdout.trim()).toBe("123")
  })

  test('parse_float generates correct IR', () => {
    const source = `
fn main() -> int {
  s := "3.14";
  f := parse_float(s);
  println(f);
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('@parse_float')
  })

  test('parse_float end-to-end', async () => {
    const source = `
fn main() -> int {
  s := "3.14";
  f := parse_float(s);
  println(f);
  return 0;
}
`
    const result = await compileAndRun(source)
    expect(result.exitCode).toBe(0)
    expect(result.stdout.trim()).toContain("3.14")
  })

  test('int_from_string returns Result', () => {
    const source = `
fn main() -> int {
  r := int_from_string("456");
  println(r);
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('@int_from_string')
  })

  test('float_from_string returns Result', () => {
    const src = `
fn main() -> int {
  r := parse_float("3.14");
  println(r);
  return 0;
}
`
    const ir = getIR(src)
    expect(ir).toContain('@float_from_string')
  })
})

describe('Tensor element access', () => {
  function getQBEIR(source: string): string {
    const tokens = tokenize(source)
    const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    const ast = parseTokens(filtered)
    const analyzer = new SemanticAnalyzer()
    analyzer.analyze(ast)
    return generateQBEIR(ast)
  }

  test('tensor element read generates tensor_get_f32 in LLVM IR', () => {
    const source = `
fn main() -> int {
  t := tensor<f32>([3], [1.0, 2.0, 3.0]);
  val := t[0];
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('@tensor_get_f32')
  })

  test('tensor element write generates tensor_set_f32 in LLVM IR', () => {
    const source = `
fn main() -> int {
  t := tensor<f32>([3], [1.0, 2.0, 3.0]);
  t[1] = 42.0;
  return 0;
}
`
    const ir = getIR(source)
    expect(ir).toContain('@tensor_set_f32')
  })

  test('tensor element read generates inline access in QBE IR', () => {
    const source = `
fn main() -> int {
  t := tensor<f32>([3], [1.0, 2.0, 3.0]);
  val := t[0];
  return 0;
}
`
    const ir = getQBEIR(source)
    expect(ir).toContain('$tensor_create')
    expect(ir).toContain('loadl')
    expect(ir).toContain('mul 0, 8')
  })

  test('tensor element write generates inline store in QBE IR', () => {
    const source = `
fn main() -> int {
  t := tensor<f32>([3], [1.0, 2.0, 3.0]);
  t[1] = 42.0;
  return 0;
}
`
    const ir = getQBEIR(source)
    expect(ir).toContain('$tensor_create')
    expect(ir).toContain('storel 42.0')
    expect(ir).toContain('mul 1, 8')
  })

  test('analyzer accepts tensor indexing without errors', () => {
    const source = `
fn main() -> int {
  t := tensor<f32>([3], [1.0, 2.0, 3.0]);
  val := t[0];
  return 0;
}
`
    const tokens = tokenize(source)
    const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    const ast = parseTokens(filtered)
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.length).toBe(0)
  })
})
