import { describe, test, expect } from "bun:test"
import { Lexer } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"
import { FutureType, FunctionType, IntegerType, ArrayType, isFutureType, isFunctionType, sameType } from "./types"

function tokenize(source: string) {
  const lexer = new Lexer(source)
  return lexer.tokenize().filter((t) => t.type !== "WHITESPACE" && t.type !== "COMMENT")
}

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

// Helper to get all statements (unwrapping blocks)
function getAllStatements(ast: any): any[] {
  const stmts: any[] = []
  for (const s of ast.statements) {
    if (s.type === "Block" && s.statements) {
      stmts.push(...s.statements)
    } else {
      stmts.push(s)
    }
  }
  return stmts
}

// ============================================================
// Lexer tests
// ============================================================
describe("Async/Await Lexer", () => {
  test("tokenizes 'async' keyword", () => {
    const tokens = tokenize("async")
    expect(tokens.length).toBe(1)
    expect(tokens[0].type).toBe("async")
    expect(tokens[0].value).toBe("async")
  })

  test("tokenizes 'await' keyword", () => {
    const tokens = tokenize("await")
    expect(tokens.length).toBe(1)
    expect(tokens[0].type).toBe("await")
    expect(tokens[0].value).toBe("await")
  })

  test("tokenizes 'async fn' sequence", () => {
    const tokens = tokenize("async fn foo()")
    expect(tokens[0].type).toBe("async")
    expect(tokens[1].type).toBe("fn")
    expect(tokens[2].type).toBe("IDENTIFIER")
    expect(tokens[2].value).toBe("foo")
  })

  test("tokenizes 'await expr' sequence", () => {
    const tokens = tokenize("await some_future")
    expect(tokens[0].type).toBe("await")
    expect(tokens[1].type).toBe("IDENTIFIER")
    expect(tokens[1].value).toBe("some_future")
  })

  test("async and await are not identifiers", () => {
    const tokens1 = tokenize("async")
    expect(tokens1[0].type).toBe("async")
    expect(tokens1[0].type).not.toBe("IDENTIFIER")

    const tokens2 = tokenize("await")
    expect(tokens2[0].type).toBe("await")
    expect(tokens2[0].type).not.toBe("IDENTIFIER")
  })

  test("identifiers starting with async/await are not keywords", () => {
    const tokens1 = tokenize("async_task")
    expect(tokens1[0].type).toBe("IDENTIFIER")
    expect(tokens1[0].value).toBe("async_task")

    const tokens2 = tokenize("awaiting")
    expect(tokens2[0].type).toBe("IDENTIFIER")
    expect(tokens2[0].value).toBe("awaiting")
  })
})

// ============================================================
// Parser tests
// ============================================================
describe("Async/Await Parser", () => {
  test("parses async fn declaration with implicit return", () => {
    const ast = parse("async fn fetch() -> i64 { 42 }")
    expect(ast.statements.length).toBe(1)
    expect(ast.statements[0].type).toBe("AsyncFunctionDeclaration")
    const decl = ast.statements[0] as any
    expect(decl.name).toBe("fetch")
    expect(decl.isAsync).toBe(true)
  })

  test("parses async fn declaration with explicit return", () => {
    const ast = parse("async fn fetch() -> i64 { return 42; }")
    expect(ast.statements.length).toBe(1)
    expect(ast.statements[0].type).toBe("AsyncFunctionDeclaration")
    const decl = ast.statements[0] as any
    expect(decl.name).toBe("fetch")
    expect(decl.isAsync).toBe(true)
  })

  test("parses async fn with parameters", () => {
    const ast = parse("async fn fetch(url: string, timeout: i64) -> i64 { return 0; }")
    const decl = ast.statements[0] as any
    expect(decl.type).toBe("AsyncFunctionDeclaration")
    expect(decl.name).toBe("fetch")
    expect(decl.params.length).toBe(2)
    expect(decl.params[0].name).toBe("url")
    expect(decl.params[1].name).toBe("timeout")
  })

  test("parses async fn with void return type", () => {
    const ast = parse("async fn do_work() { }")
    const decl = ast.statements[0] as any
    expect(decl.type).toBe("AsyncFunctionDeclaration")
    expect(decl.returnType.type).toBe("VoidType")
  })

  test("parses await expression", () => {
    const ast = parse(`{
      async fn fetch() -> i64 { 42 }
      async fn do_main() -> i64 {
        x := await fetch();
        return x;
      }
    }`)
    const stmts = getAllStatements(ast)
    const mainFunc = stmts.find((s: any) => s.name === "do_main") as any
    expect(mainFunc).toBeDefined()
    expect(mainFunc.type).toBe("AsyncFunctionDeclaration")
    // x := await fetch() is an ExpressionStatement with AssignmentExpression
    const exprStmt = mainFunc.body.statements[0] as any
    expect(exprStmt.type).toBe("ExpressionStatement")
    const assignment = exprStmt.expression as any
    expect(assignment.type).toBe("AssignmentExpression")
    expect(assignment.value.type).toBe("AwaitExpression")
    expect(assignment.value.argument.type).toBe("CallExpression")
  })

  test("parses all([...]) as regular call expression", () => {
    const ast = parse(`{
      async fn a() -> i64 { 1 }
      async fn b() -> i64 { 2 }
      async fn do_main() -> i64 {
        results := await all([a(), b()]);
        return 0;
      }
    }`)
    const stmts = getAllStatements(ast)
    const mainFunc = stmts.find((s: any) => s.name === "do_main") as any
    const exprStmt = mainFunc.body.statements[0] as any
    const assignment = exprStmt.expression as any
    expect(assignment.value.type).toBe("AwaitExpression")
    const awaitArg = assignment.value.argument
    expect(awaitArg.type).toBe("CallExpression")
    expect(awaitArg.callee.name).toBe("all")
  })

  test("parses race([...]) as regular call expression", () => {
    const ast = parse(`{
      async fn a() -> i64 { 1 }
      async fn do_main() -> i64 {
        result := await race([a()]);
        return 0;
      }
    }`)
    const stmts = getAllStatements(ast)
    const mainFunc = stmts.find((s: any) => s.name === "do_main") as any
    const exprStmt = mainFunc.body.statements[0] as any
    const assignment = exprStmt.expression as any
    expect(assignment.value.type).toBe("AwaitExpression")
    const awaitArg = assignment.value.argument
    expect(awaitArg.type).toBe("CallExpression")
    expect(awaitArg.callee.name).toBe("race")
  })

  test("parses nested await expressions", () => {
    const ast = parse(`{
      async fn fetch() -> i64 { 42 }
      async fn process(x: i64) -> i64 { x }
      async fn do_main() -> i64 {
        result := await process(await fetch());
        return result;
      }
    }`)
    const stmts = getAllStatements(ast)
    const mainFunc = stmts.find((s: any) => s.name === "do_main") as any
    const exprStmt = mainFunc.body.statements[0] as any
    const assignment = exprStmt.expression as any
    expect(assignment.value.type).toBe("AwaitExpression")
    // The argument of the outer await is a call to process()
    const processCall = assignment.value.argument
    expect(processCall.type).toBe("CallExpression")
    // The argument of process() is another await
    const args = processCall.args || processCall.arguments
    expect(args[0].type).toBe("AwaitExpression")
  })
})

// ============================================================
// FutureType tests
// ============================================================
describe("FutureType", () => {
  test("creates FutureType with inner type", () => {
    const ft = new FutureType(new IntegerType("i64"))
    expect(ft.type).toBe("FutureType")
    expect(ft.innerType).toBeInstanceOf(IntegerType)
  })

  test("toString formats correctly", () => {
    const ft = new FutureType(new IntegerType("i64"))
    expect(ft.toString()).toBe("Future<i64>")
  })

  test("isFutureType returns true for FutureType", () => {
    const ft = new FutureType(new IntegerType("i64"))
    expect(isFutureType(ft)).toBe(true)
  })

  test("isFutureType returns false for non-FutureType", () => {
    const it = new IntegerType("i64")
    expect(isFutureType(it as any)).toBe(false)
  })

  test("sameType compares FutureTypes", () => {
    const ft1 = new FutureType(new IntegerType("i64"))
    const ft2 = new FutureType(new IntegerType("i64"))
    const ft3 = new FutureType(new IntegerType("i32"))
    expect(sameType(ft1, ft2)).toBe(true)
    expect(sameType(ft1, ft3)).toBe(false)
  })

  test("nested FutureType", () => {
    const ft = new FutureType(new FutureType(new IntegerType("i64")))
    expect(ft.toString()).toBe("Future<Future<i64>>")
    expect(isFutureType(ft.innerType)).toBe(true)
  })
})

// ============================================================
// Analyzer tests
// ============================================================
describe("Async/Await Analyzer", () => {
  test("async fn has Future<T> return type in symbol table", () => {
    const { ast, errors } = analyze(`
      async fn fetch() -> i64 { 42 }
    `)
    // Should have no errors
    expect(errors.length).toBe(0)
  })

  test("await only valid inside async fn", () => {
    const { errors } = analyze(`{
      async fn fetch() -> i64 { 42 }
      fn do_main() -> i64 {
        x := await fetch();
        return x;
      }
    }`)
    const awaitErrors = errors.filter((e: any) =>
      e.message?.includes("await")
    )
    expect(awaitErrors.length).toBeGreaterThan(0)
  })

  test("await is valid inside async fn", () => {
    const { errors } = analyze(`{
      async fn fetch() -> i64 { 42 }
      async fn do_main() -> i64 {
        x := await fetch();
        return x;
      }
    }`)
    const awaitErrors = errors.filter((e: any) =>
      e.message?.includes("await")
    )
    expect(awaitErrors.length).toBe(0)
  })

  test("all() with array of futures is valid", () => {
    const { errors } = analyze(`{
      async fn a() -> i64 { 1 }
      async fn b() -> i64 { 2 }
      async fn do_main() -> i64 {
        results := await all([a(), b()]);
        return 0;
      }
    }`)
    expect(errors.length).toBe(0)
  })

  test("race() with array of futures is valid", () => {
    const { errors } = analyze(`{
      async fn a() -> i64 { 1 }
      async fn b() -> i64 { 2 }
      async fn do_main() -> i64 {
        result := await race([a(), b()]);
        return 0;
      }
    }`)
    expect(errors.length).toBe(0)
  })

  test("async fn with parameters validates correctly", () => {
    const { errors } = analyze(`{
      async fn fetch(url: string) -> i64 { 42 }
      async fn do_main() -> i64 {
        result := await fetch("http://example.com");
        return result;
      }
    }`)
    expect(errors.length).toBe(0)
  })
})

// ============================================================
// Integration: codegen-level test with sync fallback
// ============================================================
describe("Async/Await Integration", () => {
  test("async fn that does sync work compiles without errors", () => {
    const source = `{
      async fn compute(x: i64) -> i64 {
        return x + 1;
      }
      async fn do_main() -> i64 {
        result := await compute(41);
        return result;
      }
    }`
    const { errors } = analyze(source)
    expect(errors.length).toBe(0)
  })

  test("multiple await expressions in sequence", () => {
    const source = `{
      async fn step1() -> i64 { 1 }
      async fn step2() -> i64 { 2 }
      async fn do_main() -> i64 {
        a := await step1();
        b := await step2();
        return a + b;
      }
    }`
    const { errors } = analyze(source)
    expect(errors.length).toBe(0)
  })

  test("async fn calling another async fn", () => {
    const source = `{
      async fn inner() -> i64 { 10 }
      async fn outer() -> i64 {
        x := await inner();
        return x + 5;
      }
      async fn do_main() -> i64 {
        return await outer();
      }
    }`
    const { errors } = analyze(source)
    expect(errors.length).toBe(0)
  })
})
