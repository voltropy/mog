import { describe, test, expect } from "bun:test"
import { Lexer } from "./lexer"
import { parseTokens } from "./parser"

function parse(source: string) {
  const lexer = new Lexer(source)
  const tokens = lexer.tokenize()
  const filteredTokens = tokens.filter((t) => t.type !== "WHITESPACE" && t.type !== "COMMENT")
  return parseTokens(filteredTokens)
}

describe("parser literals", () => {
  test("number literal", () => {
    const ast = parse("BEGIN 42; END")
    expect(ast.type).toBe("Program")
    expect(ast.statements.length).toBe(1)
    const stmt = ast.statements[0]
    expect(stmt.type).toBe("ExpressionStatement")
    expect((stmt as any).expression.type).toBe("NumberLiteral")
    expect((stmt as any).expression.value).toBe(42)
  })

  test("string literal", () => {
    const ast = parse('BEGIN "hello"; END')
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("StringLiteral")
    expect(stmt.expression.value).toBe("hello")
  })
})

describe("parser identifiers", () => {
  test("single identifier", () => {
    const ast = parse("BEGIN x; END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("Identifier")
    expect(stmt.expression.name).toBe("x")
  })

  test("multi-character identifier", () => {
    const ast = parse("BEGIN myVariable; END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.name).toBe("myVariable")
  })
})

describe("parser binary expressions", () => {
  test("addition", () => {
    const ast = parse("BEGIN 1 + 2; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.type).toBe("BinaryExpression")
    expect(expr.operator).toBe("+")
    expect(expr.left.value).toBe(1)
    expect(expr.right.value).toBe(2)
  })

  test("subtraction", () => {
    const ast = parse("BEGIN 5 - 3; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("-")
  })

  test("multiplication", () => {
    const ast = parse("BEGIN 2 * 3; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("*")
  })

  test("division", () => {
    const ast = parse("BEGIN 10 / 2; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("/")
  })

  test("modulo", () => {
    const ast = parse("BEGIN 7 % 3; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("%")
  })
})

describe("parser logical expressions", () => {
  test("bitwise and", () => {
    const ast = parse("BEGIN 1 & 2; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("&")
  })

  test("bitwise or", () => {
    const ast = parse("BEGIN 1 | 2; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("|")
  })
})

describe("parser unary expressions", () => {
  test("negation", () => {
    const ast = parse("BEGIN -x; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.type).toBe("UnaryExpression")
    expect(expr.operator).toBe("-")
    expect(expr.argument.type).toBe("Identifier")
    expect(expr.argument.name).toBe("x")
  })

  test("logical not", () => {
    const ast = parse("BEGIN not 1; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.type).toBe("UnaryExpression")
    expect(expr.operator).toBe("not")
  })
})

describe("parser grouped expressions", () => {
  test("parenthesized expression", () => {
    const ast = parse("BEGIN (1 + 2) * 3; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.type).toBe("BinaryExpression")
    expect(expr.operator).toBe("*")
    expect(expr.left.type).toBe("BinaryExpression")
    expect(expr.left.operator).toBe("+")
  })
})

describe("parser operator precedence", () => {
  test("multiplication before addition", () => {
    const ast = parse("BEGIN 1 + 2 * 3; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("+")
    expect(expr.right.type).toBe("BinaryExpression")
    expect(expr.right.operator).toBe("*")
  })

  test("parentheses override precedence", () => {
    const ast = parse("BEGIN (1 + 2) * 3; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("*")
    expect(expr.left.type).toBe("BinaryExpression")
    expect(expr.left.operator).toBe("+")
  })

  test("left associativity", () => {
    const ast = parse("BEGIN 1 - 2 - 3; END")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("-")
    expect(expr.left.type).toBe("BinaryExpression")
  })
})

describe("parser function calls", () => {
  test("call without arguments", () => {
    const ast = parse("BEGIN foo(); END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("CallExpression")
    expect(stmt.expression.callee.name).toBe("foo")
    expect(stmt.expression.args.length).toBe(0)
  })

  test("call with one argument", () => {
    const ast = parse("BEGIN foo(42); END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.args.length).toBe(1)
    expect(stmt.expression.args[0].type).toBe("NumberLiteral")
  })

  test("call with multiple arguments", () => {
    const ast = parse("BEGIN foo(1, 2, 3); END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.args.length).toBe(3)
  })

  test("call with expression argument", () => {
    const ast = parse("BEGIN foo(a + b); END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.args[0].type).toBe("BinaryExpression")
  })

  test("chained function call", () => {
    const ast = parse("BEGIN foo(bar()); END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.args[0].type).toBe("CallExpression")
  })
})

describe("parser array literals", () => {
  test("empty array", () => {
    const ast = parse("BEGIN []; END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("ArrayLiteral")
    expect(stmt.expression.elements.length).toBe(0)
  })

  test("array with numbers", () => {
    const ast = parse("BEGIN [1, 2, 3]; END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.elements.length).toBe(3)
  })

  test("nested array", () => {
    const ast = parse("BEGIN [[1, 2], [3, 4]]; END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.elements.length).toBe(2)
    expect(stmt.expression.elements[0].type).toBe("ArrayLiteral")
  })
})

describe("parser table literals", () => {
  test("empty table", () => {
    const ast = parse("BEGIN {}; END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("TableLiteral")
    expect(stmt.expression.columns.length).toBe(0)
  })

  test("table with columns", () => {
    const ast = parse("BEGIN { a: 1, b: 2 }; END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.columns.length).toBe(2)
    expect(stmt.expression.columns[0].name).toBe("a")
    expect(stmt.expression.columns[1].name).toBe("b")
  })
})

describe("parser if statements", () => {
  test("if without else", () => {
    const ast = parse("BEGIN IF (x > y) THEN x := x + 1; FI END")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("Conditional")
    expect(stmt.condition.type).toBe("BinaryExpression")
    expect(stmt.falseBranch).toBeNull()
  })

  test("if with else", () => {
    const ast = parse("BEGIN IF (x > y) THEN x := 1; ELSE x := -1; FI END")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("Conditional")
    expect(stmt.falseBranch).not.toBeNull()
  })
})

describe("parser while loops", () => {
  test("simple while loop", () => {
    const ast = parse("BEGIN WHILE (x < 10) DO x := x + 1; OD END")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("WhileLoop")
    expect(stmt.test.type).toBe("BinaryExpression")
    expect(stmt.body.type).toBe("Block")
  })
})

describe("parser blocks", () => {
  test("empty block", () => {
    const ast = parse("BEGIN END")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("Block")
    expect(stmt.statements.length).toBe(0)
  })

  test("block with multiple statements", () => {
    const ast = parse("BEGIN a: i32 = 1; a := a + 1; 42; END")
    const stmt = ast.statements[0] as any
    expect(stmt.statements.length).toBe(3)
  })
})

describe("parser complex nested structures", () => {
  test("nested blocks with conditionals", () => {
    const ast = parse("BEGIN IF (x > 0) THEN IF (y > 0) THEN 1; FI FI END")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("Conditional")
    expect(stmt.trueBranch.statements[0].type).toBe("Conditional")
  })

  test("expression with nested function calls", () => {
    const ast = parse("BEGIN foo(bar(1, 2), 3); END")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.args.length).toBe(2)
    expect(stmt.expression.args[0].type).toBe("CallExpression")
  })
})
