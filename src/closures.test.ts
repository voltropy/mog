import { describe, test, expect } from "bun:test"
import { Lexer } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"
import { compile } from "./compiler"
import { FunctionType, IntegerType, isFunctionType } from "./types"
import { spawnSync } from "child_process"
import { writeFileSync, mkdirSync, rmSync } from "fs"
import { join } from "path"
import { tmpdir } from "os"

function parse(source: string) {
  const lexer = new Lexer(source)
  const tokens = lexer.tokenize()
  const filteredTokens = tokens.filter((t) => t.type !== "WHITESPACE" && t.type !== "COMMENT")
  return parseTokens(filteredTokens)
}

function analyze(source: string) {
  const ast = parse(source)
  const analyzer = new SemanticAnalyzer()
  const errors = analyzer.analyze(ast)
  return { ast, errors }
}

async function compileAndRun(source: string): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const sourceDir = join(tmpdir(), `mog-closure-test-${Date.now()}-${Math.random().toString(36).slice(2)}`)

  try {
    mkdirSync(sourceDir, { recursive: true })

    const llFile = join(sourceDir, "test.ll")
    const oFile = join(sourceDir, "test.o")
    const runtimePath = join(process.cwd(), "build", "runtime.a")
    const llcPath = "/opt/homebrew/opt/llvm/bin/llc"

    const result = await compile(source)
    if (result.errors.length > 0) {
      throw new Error(`Compilation errors: ${result.errors.map((e) => e.message).join(", ")}`)
    }
    await Bun.write(llFile, result.llvmIR)

    const llcProcess = spawnSync(llcPath, ["-filetype=obj", llFile, "-o", oFile], { stdio: "pipe" })
    if (llcProcess.status !== 0) {
      throw new Error(`llc compilation failed: ${llcProcess.stderr}`)
    }

    const exeFile = join(sourceDir, "test")
    const clangResult = spawnSync("clang", [oFile, runtimePath, "-o", exeFile], { stdio: "pipe" })
    if (clangResult.status !== 0 && !clangResult.stderr.toString().includes("no platform load command")) {
      throw new Error(`clang linking failed: ${clangResult.stderr}`)
    }

    const runResult = spawnSync(exeFile, [], { stdio: "pipe" })

    return {
      exitCode: runResult.status ?? 0,
      stdout: runResult.stdout ? runResult.stdout.toString() : "",
      stderr: runResult.stderr ? runResult.stderr.toString() : "",
    }
  } finally {
    rmSync(sourceDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })
  }
}

// ============================================================
// Parser tests — Lambda AST structure
// ============================================================
describe("Closure Parser", () => {
  test("parses unnamed fn with params", () => {
    const ast = parse(`{
      f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x * 2; };
    }`)
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("VariableDeclaration")
    expect(stmt.value.type).toBe("Lambda")
    expect(stmt.value.params.length).toBe(1)
    expect(stmt.value.params[0].name).toBe("x")
  })

  test("parses unnamed fn with no params", () => {
    const ast = parse(`{
      f: fn() -> i64 = fn() -> i64 { return 42; };
    }`)
    const stmt = ast.statements[0] as any
    expect(stmt.value.type).toBe("Lambda")
    expect(stmt.value.params.length).toBe(0)
  })

  test("parses unnamed fn with multiple params", () => {
    const ast = parse(`{
      f: fn(i64, i64) -> i64 = fn(a: i64, b: i64) -> i64 { return a + b; };
    }`)
    const stmt = ast.statements[0] as any
    expect(stmt.value.type).toBe("Lambda")
    expect(stmt.value.params.length).toBe(2)
  })
})

// ============================================================
// Analyzer tests — capture analysis
// ============================================================
describe("Closure Capture Analysis", () => {
  test("lambda with no captures has empty capturedVars", () => {
    const { ast, errors } = analyze(`{
      f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x * 2; };
    }`)
    expect(errors.length).toBe(0)
    const varDecl = ast.statements[0] as any
    const lambda = varDecl.value
    expect(lambda.capturedVars).toBeDefined()
    expect(lambda.capturedVars.length).toBe(0)
  })

  test("lambda capturing one variable", () => {
    const { ast, errors } = analyze(`{
      n: i64 = 10;
      f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + n; };
    }`)
    expect(errors.length).toBe(0)
    const block = ast.statements[0] as any
    const varDecl = block.statements[1]
    const lambda = varDecl.value
    expect(lambda.capturedVars).toContain("n")
    expect(lambda.capturedVars.length).toBe(1)
  })

  test("lambda capturing multiple variables", () => {
    const { ast, errors } = analyze(`{
      a: i64 = 1;
      b: i64 = 2;
      f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + a + b; };
    }`)
    expect(errors.length).toBe(0)
    const block = ast.statements[0] as any
    const varDecl = block.statements[2]
    const lambda = varDecl.value
    expect(lambda.capturedVars).toContain("a")
    expect(lambda.capturedVars).toContain("b")
    expect(lambda.capturedVars.length).toBe(2)
  })

  test("lambda does NOT capture its own params", () => {
    const { ast, errors } = analyze(`{
      f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x * 2; };
    }`)
    expect(errors.length).toBe(0)
    const lambda = (ast.statements[0] as any).value
    expect(lambda.capturedVars).not.toContain("x")
  })

  test("lambda does NOT capture locally declared variables", () => {
    const { ast, errors } = analyze(`{
      f: fn(i64) -> i64 = fn(x: i64) -> i64 {
        y: i64 = x * 2;
        return y;
      };
    }`)
    expect(errors.length).toBe(0)
    const lambda = (ast.statements[0] as any).value
    expect(lambda.capturedVars).not.toContain("y")
  })
})

// ============================================================
// Integration tests — compile & run
// ============================================================
describe("Closure Integration", () => {
  test("non-capturing lambda: fn(x) -> x * 2", async () => {
    const source = `{
  fn main() -> i64 {
    f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x * 2; };
    return f(21);
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(42)
  })

  test("capturing one variable", async () => {
    const source = `{
  fn main() -> i64 {
    n: i64 = 10;
    f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + n; };
    return f(5);
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(15)
  })

  test("capturing multiple variables", async () => {
    const source = `{
  fn main() -> i64 {
    a: i64 = 1;
    b: i64 = 2;
    f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + a + b; };
    return f(10);
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(13)
  })

  test("closure preserves captured value (value capture, not reference)", async () => {
    const source = `{
  fn main() -> i64 {
    n: i64 = 10;
    f: fn() -> i64 = fn() -> i64 { return n; };
    n := 20;
    return f();
  }
}`
    const { exitCode } = await compileAndRun(source)
    // Value capture: f captures the VALUE of n at creation time (10), not a reference
    expect(exitCode).toBe(10)
  })

  test("make_adder pattern: returning closure from function", async () => {
    const source = `{
  fn make_adder(n: i64) -> fn(i64) -> i64 {
    return fn(x: i64) -> i64 { return x + n; };
  }

  fn main() -> i64 {
    add10: fn(i64) -> i64 = make_adder(10);
    return add10(5);
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(15)
  })

  test("named function called directly still works", async () => {
    const source = `{
  fn add(a: i64, b: i64) -> i64 {
    return a + b;
  }

  fn main() -> i64 {
    return add(30, 12);
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(42)
  })

  test("non-capturing lambda with no params", async () => {
    const source = `{
  fn main() -> i64 {
    f: fn() -> i64 = fn() -> i64 { return 42; };
    return f();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(42)
  })

  test("closure used with print", async () => {
    const source = `{
  fn main() -> i64 {
    n: i64 = 5;
    f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + n; };
    print(f(10));
    return 0;
  }
}`
    const { exitCode, stdout } = await compileAndRun(source)
    expect(exitCode).toBe(0)
    expect(stdout.trim()).toBe("15")
  })

  test("multiple closures with different captures", async () => {
    const source = `{
  fn main() -> i64 {
    a: i64 = 10;
    b: i64 = 20;
    f: fn() -> i64 = fn() -> i64 { return a; };
    g: fn() -> i64 = fn() -> i64 { return b; };
    return f() + g();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(30)
  })

  test("named function as value with wrapper", async () => {
    const source = `{
  fn add(a: i64, b: i64) -> i64 {
    return a + b;
  }

  fn main() -> i64 {
    f: fn(i64, i64) -> i64 = add;
    return f(3, 4);
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(7)
  })

  test("higher order: passing closure as argument", async () => {
    const source = `{
  fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
  }

  fn main() -> i64 {
    n: i64 = 10;
    adder: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + n; };
    return apply(adder, 5);
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(15)
  })
})

// ============================================================
// Integration tests — array.filter() and array.map()
// ============================================================
describe("Array filter and map integration", () => {
  test("array.filter with inline lambda (no captures)", async () => {
    const source = `{
  fn main() -> i64 {
    arr: [i64] = [1, 2, 3, 4, 5, 6];
    evens: [i64] = arr.filter(fn(x: i64) -> i64 { return (x % 2) == 0; });
    return evens.len();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(3)  // [2, 4, 6] -> length 3
  })

  test("array.map with inline lambda (no captures)", async () => {
    const source = `{
  fn main() -> i64 {
    arr: [i64] = [1, 2, 3];
    doubled: [i64] = arr.map(fn(x: i64) -> i64 { return x * 2; });
    return doubled.pop();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(6)  // last element: 3*2 = 6
  })

  test("array.filter with closure capturing variable", async () => {
    const source = `{
  fn main() -> i64 {
    threshold: i64 = 3;
    arr: [i64] = [1, 2, 3, 4, 5];
    big: [i64] = arr.filter(fn(x: i64) -> i64 { return x > threshold; });
    return big.len();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(2)  // [4, 5] -> length 2
  })

  test("array.map with closure capturing variable", async () => {
    const source = `{
  fn main() -> i64 {
    offset: i64 = 100;
    arr: [i64] = [1, 2, 3];
    shifted: [i64] = arr.map(fn(x: i64) -> i64 { return x + offset; });
    return shifted.pop();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(103)  // last element: 3+100 = 103
  })

  test("array.filter then array.map chained", async () => {
    const source = `{
  fn main() -> i64 {
    arr: [i64] = [1, 2, 3, 4, 5, 6];
    result: [i64] = arr.filter(fn(x: i64) -> i64 { return (x % 2) == 0; });
    doubled: [i64] = result.map(fn(x: i64) -> i64 { return x * 2; });
    return doubled.len();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(3)  // filter -> [2,4,6], map -> [4,8,12], len -> 3
  })

  test("array.filter returns empty array when nothing matches", async () => {
    const source = `{
  fn main() -> i64 {
    arr: [i64] = [1, 2, 3];
    result: [i64] = arr.filter(fn(x: i64) -> i64 { return x > 100; });
    return result.len();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(0)  // nothing passes -> length 0
  })

  test("array.map preserves array length", async () => {
    const source = `{
  fn main() -> i64 {
    arr: [i64] = [10, 20, 30, 40];
    mapped: [i64] = arr.map(fn(x: i64) -> i64 { return x + 1; });
    return mapped.len();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(4)  // same length as input
  })

  test("array.filter with named closure variable", async () => {
    const source = `{
  fn main() -> i64 {
    arr: [i64] = [1, 2, 3, 4, 5];
    pred: fn(i64) -> i64 = fn(x: i64) -> i64 { return x >= 3; };
    result: [i64] = arr.filter(pred);
    return result.len();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(3)  // [3, 4, 5] -> length 3
  })

  test("array.map with named closure variable", async () => {
    const source = `{
  fn main() -> i64 {
    arr: [i64] = [5, 10, 15];
    neg: fn(i64) -> i64 = fn(x: i64) -> i64 { return 0 - x; };
    result: [i64] = arr.map(neg);
    return result.len();
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(3)  // same length
  })
})

describe("array.sort with comparator", () => {
  test("sort descending with closure comparator", async () => {
    const source = `{
  fn main() -> i64 {
    arr: [i64] = [3, 1, 4, 1, 5, 9, 2, 6];
    arr.sort(fn(a: i64, b: i64) -> i64 { return b - a; });
    return arr[0];
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(9)  // largest element first after descending sort
  })

  test("sort ascending with closure comparator", async () => {
    const source = `{
  fn main() -> i64 {
    arr: [i64] = [5, 3, 8, 1, 2];
    arr.sort(fn(a: i64, b: i64) -> i64 { return a - b; });
    return arr[0];
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(1)  // smallest element first after ascending sort
  })

  // TODO: Enable once captured variables work in 2-arg lambdas passed to sort
  // test("sort with comparator capturing a variable", async () => {
  //   const source = `{
  //   fn main() -> i64 {
  //     direction: i64 = 0 - 1;
  //     arr: [i64] = [5, 3, 8, 1, 2];
  //     arr.sort(fn(a: i64, b: i64) -> i64 { return (a - b) * direction; });
  //     return arr[0];
  //   }
  // }`
  //   const { exitCode } = await compileAndRun(source)
  //   expect(exitCode).toBe(8)
  // })

  test("no-arg sort still works (ascending default)", async () => {
    const source = `{
  fn main() -> i64 {
    arr: [i64] = [5, 3, 8, 1, 2];
    arr.sort();
    return arr[0];
  }
}`
    const { exitCode } = await compileAndRun(source)
    expect(exitCode).toBe(1)  // smallest element first after default ascending sort
  })
})
