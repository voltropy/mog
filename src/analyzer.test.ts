import { describe, test, expect } from "bun:test"
import { SemanticAnalyzer, type SemanticError, type Position } from "./analyzer"
import {
  i8,
  i16,
  i32,
  i64,
  u8,
  f32,
  f64,
  IntegerType,
  UnsignedType,
  FloatType,
  VoidType,
  ArrayType,
  array,
} from "./types"

function pos(index: number = 0): { start: any; end: any } {
  return { start: { line: 1, column: 1, index }, end: { line: 1, column: 1, index } }
}

function program(...statements: any[]): any {
  return { type: "Program", statements, position: pos() }
}

function varDecl(name: string, type: any, value: any): any {
  return { type: "VariableDeclaration", name, varType: type, value, position: pos() }
}

function assign(name: string, value: any): any {
  return { type: "Assignment", name, value, position: pos() }
}

function ident(name: string): any {
  return { type: "Identifier", name, position: pos() }
}

function num(value: any, type: any = null): any {
  return { type: "NumberLiteral", value: String(value), literalType: type, position: pos() }
}

function binary(left: any, op: string, right: any): any {
  return { type: "BinaryExpression", left, operator: op, right, position: pos() }
}

function block(...statements: any[]): any {
  return { type: "Block", statements, position: pos() }
}

function funcDecl(name: string, params: any[], returnType: any, body: any): any {
  return { type: "FunctionDeclaration", name, params, returnType, body, position: pos() }
}

function param(name: string, type: any): any {
  return { name, paramType: type }
}

function ret(value: any = null): any {
  return { type: "Return", value, position: pos() }
}

function call(callee: any, ...args: any[]): any {
  return { type: "CallExpression", callee, args, position: pos() }
}

function index(arr: any, idx: any): any {
  return { type: "IndexExpression", object: arr, index: idx, position: pos() }
}

function conditional(condition: any, trueBranch: any, falseBranch: any = null): any {
  return {
    type: "Conditional",
    condition,
    trueBranch: { type: "Block", statements: Array.isArray(trueBranch) ? trueBranch : [trueBranch], position: pos() },
    falseBranch: falseBranch
      ? { type: "Block", statements: Array.isArray(falseBranch) ? falseBranch : [falseBranch], position: pos() }
      : null,
    position: pos(),
  }
}

describe("Semantic Analyzer - Variable Scope", () => {
  test("allows variable in same scope", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(42, new IntegerType("i32"))),
      varDecl("y", new IntegerType("i32"), binary(ident("x"), "+", num(5, new IntegerType("i32")))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("block scoping isolates variables", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(10, new IntegerType("i32"))),
      block(varDecl("x", new IntegerType("i32"), num(20, new IntegerType("i32")))),
      varDecl("x", new IntegerType("i32"), num(30, new IntegerType("i32"))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("lexical scoping allows outer scope access", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(10, new IntegerType("i32"))),
      block(varDecl("y", new IntegerType("i32"), binary(ident("x"), "+", num(5, new IntegerType("i32"))))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("inner scope shadows outer scope variable", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(10, new IntegerType("i32"))),
      block(
        varDecl("x", new IntegerType("i32"), num(20, new IntegerType("i32"))),
        varDecl("y", new IntegerType("i32"), ident("x")),
      ),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("variable not accessible after block scope ends", () => {
    const ast = program(
      block(varDecl("x", new IntegerType("i32"), num(10, new IntegerType("i32")))),
      varDecl("y", new IntegerType("i32"), ident("x")),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Undefined variable 'x'"))).toBe(true)
  })

  test("nested block scopes work correctly", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(1, new IntegerType("i32"))),
      block(
        varDecl("y", new IntegerType("i32"), num(2, new IntegerType("i32"))),
        block(varDecl("z", new IntegerType("i32"), binary(ident("x"), "+", ident("y")))),
      ),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("function parameters are in function scope", () => {
    const ast = program(
      funcDecl(
        "foo",
        [param("x", new IntegerType("i32"))],
        new IntegerType("i32"),
        block(
          varDecl("y", new IntegerType("i32"), binary(ident("x"), "+", num(5, new IntegerType("i32")))),
          ret(ident("y")),
        ),
      ),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("function parameters are not accessible outside function", () => {
    const ast = program(
      funcDecl("foo", [param("x", new IntegerType("i32"))], new IntegerType("i32"), block(ret(ident("x")))),
      assign("y", ident("x")),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Undefined variable 'x'"))).toBe(true)
  })
})

describe("Semantic Analyzer - Type Checking: Binary Operations", () => {
  test("allows addition of same integer types", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(5, new IntegerType("i32"))),
      varDecl("y", new IntegerType("i32"), binary(ident("x"), "+", num(10, new IntegerType("i32")))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("allows addition of same float types", () => {
    const ast = program(
      varDecl("x", new FloatType("f32"), num(5.0, new FloatType("f32"))),
      varDecl("y", new FloatType("f32"), binary(ident("x"), "+", num(10.0, new FloatType("f32")))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("rejects addition of different numeric types", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(5, new IntegerType("i32"))),
      varDecl("y", new FloatType("f32"), binary(ident("x"), "+", num(10.0, new FloatType("f32")))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Operator '+' requires same types"))).toBe(true)
  })

  test("allows comparison of same numeric types", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(5, new IntegerType("i32"))),
      conditional(
        binary(ident("x"), ">", num(10, new IntegerType("i32"))),
        assign("x", num(20, new IntegerType("i32"))),
      ),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("allows comparison with coercion from int to float", () => {
    const ast = program(
      varDecl("x", new FloatType("f32"), num(5.0, new FloatType("f32"))),
      varDecl("y", new IntegerType("i32"), num(3, new IntegerType("i32"))),
      conditional(binary(ident("x"), ">", ident("y")), assign("x", num(10.0, new FloatType("f32")))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("allows logical operators with numeric types", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(5, new IntegerType("i32"))),
      varDecl("y", new IntegerType("i32"), num(10, new IntegerType("i32"))),
      conditional(binary(ident("x"), "AND", ident("y")), assign("x", num(0, new IntegerType("i32")))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })
})

describe("Semantic Analyzer - Type Checking: Function Calls", () => {
  test("detects undefined function calls", () => {
    const ast = program({ type: "ExpressionStatement", expression: call(ident("foo")), position: pos() })
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Undefined function 'foo'"))).toBe(true)
  })

  test("allows calls to declared functions", () => {
    const ast = program(funcDecl("foo", [], new IntegerType("i32"), block(ret(num(42, new IntegerType("i32"))))), {
      type: "ExpressionStatement",
      expression: call(ident("foo")),
      position: pos(),
    })
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("rejects indexing into non-array type", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(5, new IntegerType("i32"))),
      varDecl("y", new IntegerType("i32"), index(ident("x"), num(0, new IntegerType("i32")))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Cannot index into non-array type"))).toBe(true)
  })
})

describe("Semantic Analyzer - Type Checking: Assignments", () => {
  test("allows assignment of compatible same type", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(5, new IntegerType("i32"))),
      assign("x", num(10, new IntegerType("i32"))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("allows assignment with type inference", () => {
    const ast = program(varDecl("x", null, num(5, null)), assign("x", num(10, new IntegerType("i64"))))
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("rejects assignment of incompatible type to declared variable", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(5, new IntegerType("i32"))),
      assign("x", num(10.0, new FloatType("f32"))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Type mismatch: cannot assign"))).toBe(true)
  })

  test("rejects assignment of incompatible type to inferred variable", () => {
    const ast = program(varDecl("x", null, num(5, null)), assign("x", num(10.0, new FloatType("f32"))))
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Type mismatch: cannot assign"))).toBe(true)
  })

  test("rejects assignment to undefined variable", () => {
    const ast = program(assign("x", num(10, new IntegerType("i32"))))
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Undefined variable 'x'"))).toBe(true)
  })

  test("rejects assignment to function", () => {
    const ast = program(
      funcDecl("foo", [], new IntegerType("i32"), block(ret(num(42, new IntegerType("i32"))))),
      assign("foo", num(10, new IntegerType("i32"))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Cannot assign to function"))).toBe(true)
  })
})

describe("Semantic Analyzer - Undeclared Variable Detection", () => {
  test("detects undeclared variable in expression", () => {
    const ast = program(assign("x", binary(ident("y"), "+", num(5, new IntegerType("i32")))))
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Undefined variable 'y'"))).toBe(true)
  })

  test("detects undeclared variable in assignment", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(10, new IntegerType("i32"))),
      assign("y", binary(ident("x"), "+", ident("z"))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Undefined variable 'z'"))).toBe(true)
  })

  test("detects undeclared variable in condition", () => {
    const ast = program(
      conditional(
        binary(ident("unknown"), ">", num(0, new IntegerType("i32"))),
        varDecl("x", new IntegerType("i32"), num(1, new IntegerType("i32"))),
      ),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Undefined variable 'unknown'"))).toBe(true)
  })
})

describe("Semantic Analyzer - Redeclaration Detection", () => {
  test("allows shadowing in nested blocks", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(10, new IntegerType("i32"))),
      block(varDecl("x", new IntegerType("i32"), num(20, new IntegerType("i32")))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("variables in different blocks are independent", () => {
    const ast = program(
      block(varDecl("x", new IntegerType("i32"), num(10, new IntegerType("i32")))),
      block(varDecl("x", new IntegerType("i32"), num(20, new IntegerType("i32")))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("function parameters don't conflict with outer variables", () => {
    const ast = program(
      varDecl("x", new IntegerType("i32"), num(10, new IntegerType("i32"))),
      funcDecl("foo", [param("x", new IntegerType("i32"))], new IntegerType("i32"), block(ret(ident("x")))),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })
})

describe("Semantic Analyzer - Return Type Validation", () => {
  test("detects return statement outside function", () => {
    const ast = program(ret(num(42, new IntegerType("i32"))))
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.some((e: SemanticError) => e.message.includes("Return statement outside function"))).toBe(true)
  })

  test("allows return inside function", () => {
    const ast = program(funcDecl("foo", [], new IntegerType("i32"), block(ret(num(42, new IntegerType("i32"))))))
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("allows return without value for void functions", () => {
    const ast = program(funcDecl("foo", [], new VoidType(), block(ret())))
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })
})

describe("Semantic Analyzer - Function Parameter Validation", () => {
  test("allows valid function parameter declarations", () => {
    const ast = program(
      funcDecl(
        "foo",
        [param("x", new IntegerType("i32")), param("y", new FloatType("f32"))],
        new IntegerType("i32"),
        block(ret(ident("x"))),
      ),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("allows using function parameters in function body", () => {
    const ast = program(
      funcDecl(
        "foo",
        [param("x", new IntegerType("i32")), param("y", new IntegerType("i32"))],
        new IntegerType("i32"),
        block(ret(binary(ident("x"), "+", ident("y")))),
      ),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("allows array type parameters", () => {
    const ast = program(
      funcDecl(
        "sum",
        [param("arr", new ArrayType(new IntegerType("i32"), [1]))],
        new IntegerType("i32"),
        block(ret(num(0, new IntegerType("i32")))),
      ),
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })
})

describe("Semantic Analyzer - Other Validations", () => {
  test("empty program produces no errors", () => {
    const ast = program()
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toEqual([])
  })

  test("multiple independent errors are collected", () => {
    const ast = program(assign("x", ident("undefined_var_1")), assign("y", ident("undefined_var_2")))
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.length).toBeGreaterThan(1)
  })
})
