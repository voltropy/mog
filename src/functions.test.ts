import { describe, test, expect } from "bun:test"
import { Lexer } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"
import { compile } from "./compiler"
import { FunctionType, IntegerType, FloatType, BoolType, sameType, isFunctionType } from "./types"

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

// ============================================================
// FunctionType class tests
// ============================================================
describe("FunctionType", () => {
  test("creates function type with param types and return type", () => {
    const ft = new FunctionType(
      [new IntegerType("i64"), new IntegerType("i64")],
      new IntegerType("i64")
    )
    expect(ft.paramTypes.length).toBe(2)
    expect(ft.returnType).toBeInstanceOf(IntegerType)
    expect(ft.type).toBe("FunctionType")
  })

  test("toString formats correctly", () => {
    const ft = new FunctionType(
      [new IntegerType("i64")],
      new BoolType()
    )
    expect(ft.toString()).toBe("fn(i64) -> bool")
  })

  test("sameType compares function types", () => {
    const ft1 = new FunctionType(
      [new IntegerType("i64")],
      new IntegerType("i64")
    )
    const ft2 = new FunctionType(
      [new IntegerType("i64")],
      new IntegerType("i64")
    )
    const ft3 = new FunctionType(
      [new FloatType("f64")],
      new IntegerType("i64")
    )
    expect(sameType(ft1, ft2)).toBe(true)
    expect(sameType(ft1, ft3)).toBe(false)
  })

  test("isFunctionType type guard", () => {
    const ft = new FunctionType([], new IntegerType("i64"))
    expect(isFunctionType(ft)).toBe(true)
    expect(isFunctionType(new IntegerType("i64"))).toBe(false)
  })

  test("nested function types", () => {
    // fn(int) -> fn(int) -> int
    const inner = new FunctionType([new IntegerType("i64")], new IntegerType("i64"))
    const outer = new FunctionType([new IntegerType("i64")], inner)
    expect(outer.toString()).toBe("fn(i64) -> fn(i64) -> i64")
  })
})

// ============================================================
// Anonymous function expression parsing
// ============================================================
describe("anonymous functions", () => {
  test("parses anonymous function expression", () => {
    const ast = parse(`{
  fn add(a: i64, b: i64) -> i64 {
    return a + b;
  }
  add(1, 2);
}`)
    expect(ast.type).toBe("Program")
    // When wrapped in {}, statements are inside a Block
    const block = ast.statements[0] as any
    const stmts = block.type === "Block" ? block.statements : ast.statements
    expect(stmts.some((s: any) => s.type === "FunctionDeclaration")).toBe(true)
  })

  test("parses anonymous function as expression", () => {
    const ast = parse(`{
  f := fn(x: i64) -> i64 { x + 1 };
}`)
    expect(ast.type).toBe("Program")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("ExpressionStatement")
    expect(stmt.expression.type).toBe("AssignmentExpression")
    expect(stmt.expression.value.type).toBe("Lambda")
    expect(stmt.expression.value.params.length).toBe(1)
    expect(stmt.expression.value.params[0].name).toBe("x")
  })

  test("parses anonymous function with multiple params", () => {
    const ast = parse(`{
  f := fn(a: i64, b: i64) -> i64 { a + b };
}`)
    const stmt = ast.statements[0] as any
    const lambda = stmt.expression.value
    expect(lambda.type).toBe("Lambda")
    expect(lambda.params.length).toBe(2)
    expect(lambda.params[0].name).toBe("a")
    expect(lambda.params[1].name).toBe("b")
  })

  test("anonymous function passed as argument", () => {
    const ast = parse(`{
  fn apply(f: i64, x: i64) -> i64 {
    return f(x);
  }
  apply(fn(x: i64) -> i64 { x * 2 }, 5);
}`)
    expect(ast.type).toBe("Program")
    // When wrapped in {}, statements are inside a Block
    const block = ast.statements[0] as any
    const stmts = block.type === "Block" ? block.statements : ast.statements
    const callStmt = stmts.find((s: any) => s.type === "ExpressionStatement" && (s as any).expression.type === "CallExpression") as any
    expect(callStmt).toBeTruthy()
    const callExpr = callStmt.expression
    const args = callExpr.args || callExpr.arguments
    expect(args[0].type).toBe("Lambda")
  })
})

// ============================================================
// Single-expression function bodies
// ============================================================
describe("single-expression function bodies", () => {
  test("function with single expression body gets implicit return", () => {
    const ast = parse(`{
  fn double(x: i64) -> i64 { x * 2 }
}`)
    const funcDecl = ast.statements.find((s: any) => s.type === "FunctionDeclaration") as any
    expect(funcDecl).toBeTruthy()
    expect(funcDecl.body.statements.length).toBe(1)
    // The single expression should be wrapped as a Return statement
    expect(funcDecl.body.statements[0].type).toBe("Return")
    expect(funcDecl.body.statements[0].value.type).toBe("BinaryExpression")
  })

  test("function with explicit return still works", () => {
    const ast = parse(`{
  fn double(x: i64) -> i64 {
    return x * 2;
  }
}`)
    const funcDecl = ast.statements.find((s: any) => s.type === "FunctionDeclaration") as any
    expect(funcDecl).toBeTruthy()
    expect(funcDecl.body.statements[0].type).toBe("Return")
  })

  test("lambda with single expression body", () => {
    const ast = parse(`{
  f := fn(x: i64) -> i64 { x + 1 };
}`)
    const stmt = ast.statements[0] as any
    const lambda = stmt.expression.value
    // Single expression body should be wrapped as implicit return
    expect(lambda.body.statements.length).toBe(1)
    expect(lambda.body.statements[0].type).toBe("Return")
  })

  test("function with multiple statements keeps them all", () => {
    const ast = parse(`{
  fn foo(x: i64) -> i64 {
    y := x + 1;
    return y * 2;
  }
}`)
    const funcDecl = ast.statements.find((s: any) => s.type === "FunctionDeclaration") as any
    expect(funcDecl.body.statements.length).toBe(2)
  })
})

// ============================================================
// Default parameter values
// ============================================================
describe("default parameter values", () => {
  test("parses function with default parameter", () => {
    const ast = parse(`{
  fn greet(name: i64, times: i64 = 1) -> i64 {
    return name + times;
  }
}`)
    const funcDecl = ast.statements.find((s: any) => s.type === "FunctionDeclaration") as any
    expect(funcDecl).toBeTruthy()
    expect(funcDecl.params.length).toBe(2)
    expect(funcDecl.params[0].defaultValue).toBeFalsy()
    expect(funcDecl.params[1].defaultValue).toBeTruthy()
    expect(funcDecl.params[1].defaultValue.type).toBe("NumberLiteral")
    expect(funcDecl.params[1].defaultValue.value).toBe("1")
  })

  test("parses multiple default parameters", () => {
    const ast = parse(`{
  fn config(x: i64, y: i64 = 10, z: i64 = 20) -> i64 {
    return x + y + z;
  }
}`)
    const funcDecl = ast.statements.find((s: any) => s.type === "FunctionDeclaration") as any
    expect(funcDecl.params[0].defaultValue).toBeFalsy()
    expect(funcDecl.params[1].defaultValue).toBeTruthy()
    expect(funcDecl.params[2].defaultValue).toBeTruthy()
  })

  test("analyzer validates default value types", () => {
    const { errors } = analyze(`{
  fn foo(x: i64, y: i64 = 10) -> i64 {
    return x + y;
  }
  foo(5);
}`)
    // Should compile without errors - default value 10 is compatible with i64
    expect(errors.length).toBe(0)
  })
})

// ============================================================
// Named arguments at call sites
// ============================================================
describe("named arguments", () => {
  test("parses named arguments in function call", () => {
    const ast = parse(`{
  fn add(a: i64, b: i64) -> i64 {
    return a + b;
  }
  add(a: 1, b: 2);
}`)
    const block = ast.statements[0] as any
    const stmts = block.type === "Block" ? block.statements : ast.statements
    const callStmt = stmts.find(
      (s: any) => s.type === "ExpressionStatement" && (s as any).expression?.type === "CallExpression"
    ) as any
    expect(callStmt).toBeTruthy()
    const callExpr = callStmt.expression
    expect(callExpr.namedArgs).toBeTruthy()
    expect(callExpr.namedArgs.length).toBe(2)
    expect(callExpr.namedArgs[0].name).toBe("a")
    expect(callExpr.namedArgs[1].name).toBe("b")
  })

  test("parses mixed positional and named arguments", () => {
    const ast = parse(`{
  fn config(x: i64, y: i64, z: i64) -> i64 {
    return x + y + z;
  }
  config(1, y: 2, z: 3);
}`)
    const block = ast.statements[0] as any
    const stmts = block.type === "Block" ? block.statements : ast.statements
    const callStmt = stmts.find(
      (s: any) => s.type === "ExpressionStatement" && (s as any).expression?.type === "CallExpression"
    ) as any
    expect(callStmt).toBeTruthy()
    const callExpr = callStmt.expression
    // First arg should be positional
    const args = callExpr.args || callExpr.arguments || []
    expect(args.length).toBe(1) // Just the positional arg
    expect(callExpr.namedArgs.length).toBe(2) // y and z are named
  })
})

// ============================================================
// Named argument validation at call sites
// ============================================================
describe("named argument validation", () => {
  test("accepts valid named arguments", () => {
    const { errors } = analyze(`{
  fn add(a: i64, b: i64) -> i64 {
    return a + b;
  }
  add(a: 1, b: 2);
}`)
    expect(errors.length).toBe(0)
  })

  test("accepts mixed positional and named arguments", () => {
    const { errors } = analyze(`{
  fn config(x: i64, y: i64, z: i64) -> i64 {
    return x + y + z;
  }
  config(1, y: 2, z: 3);
}`)
    expect(errors.length).toBe(0)
  })

  test("errors on unknown named argument", () => {
    const { errors } = analyze(`{
  fn add(a: i64, b: i64) -> i64 {
    return a + b;
  }
  add(a: 1, c: 2);
}`)
    expect(errors.length).toBeGreaterThanOrEqual(1)
    const unknownArgError = errors.find((e: any) => e.message.includes("Unknown named argument 'c'"))
    expect(unknownArgError).toBeTruthy()
  })

  test("errors on missing required argument", () => {
    const { errors } = analyze(`{
  fn add(a: i64, b: i64) -> i64 {
    return a + b;
  }
  add(a: 1);
}`)
    expect(errors.length).toBeGreaterThanOrEqual(1)
    const missingArgError = errors.find((e: any) => e.message.includes("Missing required argument 'b'"))
    expect(missingArgError).toBeTruthy()
  })

  test("does not error when default fills missing named arg", () => {
    const { errors } = analyze(`{
  fn greet(x: i64, y: i64 = 10) -> i64 {
    return x + y;
  }
  greet(x: 5);
}`)
    expect(errors.length).toBe(0)
  })

  test("positional args cover required params without naming them", () => {
    const { errors } = analyze(`{
  fn add(a: i64, b: i64) -> i64 {
    return a + b;
  }
  add(1, 2);
}`)
    expect(errors.length).toBe(0)
  })
})

// ============================================================
// Function type annotations
// ============================================================
describe("function type annotations", () => {
  test("parses function type in type alias", () => {
    const ast = parse(`{
  type Callback = fn(i64) -> i64;
}`)
    expect(ast.type).toBe("Program")
    const typeAlias = ast.statements.find((s: any) => s.type === "TypeAliasDeclaration") as any
    expect(typeAlias).toBeTruthy()
    expect(typeAlias.name).toBe("Callback")
    const aliasedType = typeAlias.aliasedType
    // The aliased type should be a TypeAliasType wrapping a FunctionType
    expect(aliasedType.aliasedType).toBeInstanceOf(FunctionType)
  })

  test("parses function type as parameter type", () => {
    const ast = parse(`{
  fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
  }
}`)
    const funcDecl = ast.statements.find((s: any) => s.type === "FunctionDeclaration") as any
    expect(funcDecl).toBeTruthy()
    expect(funcDecl.params[0].paramType).toBeInstanceOf(FunctionType)
    const fnType = funcDecl.params[0].paramType as FunctionType
    expect(fnType.paramTypes.length).toBe(1)
  })

  test("parses function type as return type", () => {
    const ast = parse(`{
  fn make_adder(n: i64) -> fn(i64) -> i64 {
    return fn(x: i64) -> i64 { x + n };
  }
}`)
    const funcDecl = ast.statements.find((s: any) => s.type === "FunctionDeclaration") as any
    expect(funcDecl).toBeTruthy()
    expect(funcDecl.returnType).toBeInstanceOf(FunctionType)
    const retType = funcDecl.returnType as FunctionType
    expect(retType.paramTypes.length).toBe(1)
  })

  test("parses function type in variable declaration", () => {
    const ast = parse(`{
  callback: fn(i64) -> i64 = fn(x: i64) -> i64 { x * 2 };
}`)
    const varDecl = ast.statements.find((s: any) => s.type === "VariableDeclaration") as any
    expect(varDecl).toBeTruthy()
    expect(varDecl.varType).toBeInstanceOf(FunctionType)
  })
})

// ============================================================
// Closures capturing variables
// ============================================================
describe("closures", () => {
  test("lambda captures outer scope variables - analyzer accepts", () => {
    const { errors } = analyze(`{
  n: i64 = 5;
  f := fn(x: i64) -> i64 { x + n };
}`)
    // Closure should be accepted - n is in the enclosing scope
    expect(errors.length).toBe(0)
  })

  test("function returning a lambda - analyzer accepts", () => {
    const { errors } = analyze(`{
  fn make_adder(n: i64) -> fn(i64) -> i64 {
    return fn(x: i64) -> i64 { x + n };
  }
}`)
    expect(errors.length).toBe(0)
  })

  test("nested closures - analyzer accepts", () => {
    const { errors } = analyze(`{
  a: i64 = 1;
  f := fn(x: i64) -> i64 {
    return x + a;
  };
}`)
    expect(errors.length).toBe(0)
  })
})

// ============================================================
// Higher-order functions
// ============================================================
describe("higher-order functions", () => {
  test("function taking function parameter", () => {
    const ast = parse(`{
  fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
  }
}`)
    const funcDecl = ast.statements.find((s: any) => s.type === "FunctionDeclaration") as any
    expect(funcDecl.params[0].paramType).toBeInstanceOf(FunctionType)
    expect(funcDecl.params[1].paramType).toBeInstanceOf(IntegerType)
  })

  test("function returning a function", () => {
    const ast = parse(`{
  fn make_multiplier(factor: i64) -> fn(i64) -> i64 {
    return fn(x: i64) -> i64 { x * factor };
  }
}`)
    const funcDecl = ast.statements.find((s: any) => s.type === "FunctionDeclaration") as any
    expect(funcDecl.returnType).toBeInstanceOf(FunctionType)
    // The body should contain a return with a Lambda
    expect(funcDecl.body.statements[0].type).toBe("Return")
    expect(funcDecl.body.statements[0].value.type).toBe("Lambda")
  })

  test("analyzer accepts higher-order function patterns", () => {
    const { errors } = analyze(`{
  fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return f(x);
  }
  fn double(x: i64) -> i64 { x * 2 }
}`)
    expect(errors.length).toBe(0)
  })
})

// ============================================================
// Compilation integration tests
// ============================================================
describe("function improvements compilation", () => {
  test("compiles basic function with single expression body", async () => {
    const source = `{
  fn double(x: i64) -> i64 { x * 2 }
  double(5);
}`
    const result = await compile(source)
    expect(result.errors).toEqual([])
    expect(result.llvmIR).toContain("define i64 @double")
    expect(result.llvmIR).toContain("mul")
  })

  test("compiles function with default parameters", async () => {
    const source = `{
  fn add(a: i64, b: i64 = 10) -> i64 {
    return a + b;
  }
  add(5);
}`
    const result = await compile(source)
    expect(result.errors).toEqual([])
    expect(result.llvmIR).toContain("define i64 @add")
  })

  test("compiles anonymous function expression", async () => {
    const source = `{
  fn apply(f: i64, x: i64) -> i64 {
    return x;
  }
  apply(fn(x: i64) -> i64 { x + 1 }, 5);
}`
    const result = await compile(source)
    expect(result.errors).toEqual([])
    // Should contain the lifted lambda function
    expect(result.llvmIR).toContain("__lambda_")
  })

  test("compiles function returning void (no return type)", async () => {
    const source = `{
  fn greet(x: i64) {
    print_i64(x);
  }
  greet(42);
}`
    const result = await compile(source)
    expect(result.errors).toEqual([])
    expect(result.llvmIR).toContain("define void @greet")
  })

  test("compiles function type alias", async () => {
    const source = `{
  type Transformer = fn(i64) -> i64;
  fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    return x;
  }
}`
    const result = await compile(source)
    expect(result.errors).toEqual([])
  })

  test("compiles named arguments", async () => {
    const source = `{
  fn config(x: i64, y: i64 = 10, z: i64 = 20) -> i64 {
    return x + y + z;
  }
  config(1, y: 5, z: 15);
}`
    const result = await compile(source)
    expect(result.errors).toEqual([])
    expect(result.llvmIR).toContain("define i64 @config")
  })
})

// ============================================================
// Void function (no return type arrow)
// ============================================================
describe("void functions", () => {
  test("parses function without return type as void", () => {
    const ast = parse(`{
  fn greet(name: i64) {
    print_i64(name);
  }
}`)
    const funcDecl = ast.statements.find((s: any) => s.type === "FunctionDeclaration") as any
    expect(funcDecl).toBeTruthy()
    expect(funcDecl.returnType.type).toBe("VoidType")
  })
})
