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
    const ast = parse("{ 42; }")
    expect(ast.type).toBe("Program")
    expect(ast.statements.length).toBe(1)
    const stmt = ast.statements[0]
    expect(stmt.type).toBe("ExpressionStatement")
    expect((stmt as any).expression.type).toBe("NumberLiteral")
    expect((stmt as any).expression.value).toBe("42")
  })

  test("string literal", () => {
    const ast = parse('{ "hello"; }')
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("StringLiteral")
    expect(stmt.expression.value).toBe("hello")
  })

  test("f-string with simple interpolation", () => {
    const ast = parse('{ f"Hello, {name}!"; }')
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("TemplateLiteral")
    expect(stmt.expression.parts).toHaveLength(3)
    expect(stmt.expression.parts[0]).toBe("Hello, ")
    expect(stmt.expression.parts[1].type).toBe("Identifier")
    expect(stmt.expression.parts[1].name).toBe("name")
    expect(stmt.expression.parts[2]).toBe("!")
  })

  test("f-string with expression interpolation", () => {
    const ast = parse('{ f"Result: {x + y}"; }')
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("TemplateLiteral")
    expect(stmt.expression.parts).toHaveLength(2)
    expect(stmt.expression.parts[0]).toBe("Result: ")
    expect(stmt.expression.parts[1].type).toBe("BinaryExpression")
    expect(stmt.expression.parts[1].operator).toBe("+")
  })

  test("f-string with multiple interpolations", () => {
    const ast = parse('{ f"{a} and {b}"; }')
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("TemplateLiteral")
    expect(stmt.expression.parts).toHaveLength(3)
    expect(stmt.expression.parts[0].type).toBe("Identifier")
    expect(stmt.expression.parts[0].name).toBe("a")
    expect(stmt.expression.parts[1]).toBe(" and ")
    expect(stmt.expression.parts[2].type).toBe("Identifier")
    expect(stmt.expression.parts[2].name).toBe("b")
  })

  test("f-string with function call interpolation", () => {
    const ast = parse('{ f"Value: {getValue()}"; }')
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("TemplateLiteral")
    expect(stmt.expression.parts).toHaveLength(2)
    expect(stmt.expression.parts[1].type).toBe("CallExpression")
    expect(stmt.expression.parts[1].callee.name).toBe("getValue")
  })

  test("single-quoted f-string", () => {
    const ast = parse("{ f'Hello, {name}!'; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("TemplateLiteral")
    expect(stmt.expression.parts[0]).toBe("Hello, ")
    expect(stmt.expression.parts[1].type).toBe("Identifier")
  })
})

describe("parser identifiers", () => {
  test("single identifier", () => {
    const ast = parse("{ x; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("Identifier")
    expect(stmt.expression.name).toBe("x")
  })

  test("multi-character identifier", () => {
    const ast = parse("{ myVariable; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.name).toBe("myVariable")
  })
})

describe("parser binary expressions", () => {
  test("addition", () => {
    const ast = parse("{ 1 + 2; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.type).toBe("BinaryExpression")
    expect(expr.operator).toBe("+")
    expect(expr.left.value).toBe("1")
    expect(expr.right.value).toBe("2")
  })

  test("subtraction", () => {
    const ast = parse("{ 5 - 3; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("-")
  })

  test("multiplication", () => {
    const ast = parse("{ 2 * 3; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("*")
  })

  test("division", () => {
    const ast = parse("{ 10 / 2; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("/")
  })

  test("modulo", () => {
    const ast = parse("{ 7 % 3; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("%")
  })
})

describe("parser logical expressions", () => {
  test("bitwise and", () => {
    const ast = parse("{ 1 & 2; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("&")
  })

  test("bitwise or", () => {
    const ast = parse("{ 1 | 2; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("|")
  })
})

describe("parser unary expressions", () => {
  test("negation", () => {
    const ast = parse("{ -x; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.type).toBe("UnaryExpression")
    expect(expr.operator).toBe("-")
    expect(expr.argument.type).toBe("Identifier")
    expect(expr.argument.name).toBe("x")
  })

  test("logical not", () => {
    const ast = parse("{ not 1; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.type).toBe("UnaryExpression")
    expect(expr.operator).toBe("not")
  })
})

describe("parser grouped expressions", () => {
  test("parenthesized expression", () => {
    const ast = parse("{ (1 + 2) * 3; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.type).toBe("BinaryExpression")
    expect(expr.operator).toBe("*")
    expect(expr.left.type).toBe("BinaryExpression")
    expect(expr.left.operator).toBe("+")
  })
})

describe("parser operator precedence", () => {
  test("multiplication before addition", () => {
    const ast = parse("{ 1 + 2 * 3; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("+")
    expect(expr.right.type).toBe("BinaryExpression")
    expect(expr.right.operator).toBe("*")
  })

  test("parentheses override precedence", () => {
    const ast = parse("{ (1 + 2) * 3; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("*")
    expect(expr.left.type).toBe("BinaryExpression")
    expect(expr.left.operator).toBe("+")
  })

  test("left associativity", () => {
    const ast = parse("{ 1 - 2 - 3; }")
    const expr = (ast.statements[0] as any).expression
    expect(expr.operator).toBe("-")
    expect(expr.left.type).toBe("BinaryExpression")
  })
})

describe("parser function calls", () => {
  test("call without arguments", () => {
    const ast = parse("{ foo(); }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("CallExpression")
    expect(stmt.expression.callee.name).toBe("foo")
    expect(stmt.expression.args.length).toBe(0)
  })

  test("call with one argument", () => {
    const ast = parse("{ foo(42); }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.args.length).toBe(1)
    expect(stmt.expression.args[0].type).toBe("NumberLiteral")
  })

  test("call with multiple arguments", () => {
    const ast = parse("{ foo(1, 2, 3); }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.args.length).toBe(3)
  })

  test("call with expression argument", () => {
    const ast = parse("{ foo(a + b); }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.args[0].type).toBe("BinaryExpression")
  })

  test("chained function call", () => {
    const ast = parse("{ foo(bar()); }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.args[0].type).toBe("CallExpression")
  })
})

describe("parser array literals", () => {
  test("empty array", () => {
    const ast = parse("{ []; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("ArrayLiteral")
    expect(stmt.expression.elements.length).toBe(0)
  })

  test("array with numbers", () => {
    const ast = parse("{ [1, 2, 3]; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.elements.length).toBe(3)
  })

  test("nested array", () => {
    const ast = parse("{ [[1, 2], [3, 4]]; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.elements.length).toBe(2)
    expect(stmt.expression.elements[0].type).toBe("ArrayLiteral")
  })
})

describe("parser map literals", () => {
  test("empty map", () => {
    const ast = parse("{ m = {}; }")
    const stmt = ast.statements[0] as any
    // = is now assignment, so m = {} is AssignmentExpression
    expect(stmt.expression.type).toBe("AssignmentExpression")
    expect(stmt.expression.value.type).toBe("MapLiteral")
    expect(stmt.expression.value.entries.length).toBe(0)
  })

  test("map with entries", () => {
    const ast = parse("{ m = { a: 1, b: 2 }; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("AssignmentExpression")
    expect(stmt.expression.value.type).toBe("MapLiteral")
    expect(stmt.expression.value.entries.length).toBe(2)
    expect(stmt.expression.value.entries[0].key).toBe("a")
    expect(stmt.expression.value.entries[1].key).toBe("b")
  })
})

describe("parser if statements", () => {
  test("if without else", () => {
    const ast = parse("{ if (x > y) { x := x + 1; } }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("Conditional")
    expect(stmt.condition.type).toBe("BinaryExpression")
    expect(stmt.falseBranch).toBeNull()
  })

  test("if with else", () => {
    const ast = parse("{ if (x > y) { x := 1; } else { x := -1; } }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("Conditional")
    expect(stmt.falseBranch).not.toBeNull()
  })
})

describe("parser while loops", () => {
  test("simple while loop", () => {
    const ast = parse("{ while (x < 10) { x := x + 1; } }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("WhileLoop")
    expect(stmt.test.type).toBe("BinaryExpression")
    expect(stmt.body.type).toBe("Block")
  })
})

describe("parser blocks", () => {
  test("empty block", () => {
    const ast = parse("{ }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("Block")
    expect(stmt.statements.length).toBe(0)
  })

  test("block with multiple statements", () => {
    const ast = parse("{ a: i32 = 1; a := a + 1; 42; }")
    const stmt = ast.statements[0] as any
    expect(stmt.statements.length).toBe(3)
  })
})

describe("parser complex nested structures", () => {
  test("nested blocks with conditionals", () => {
    const ast = parse("{ if (x > 0) { if (y > 0) { 1; } } }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("Conditional")
    expect(stmt.trueBranch.statements[0].type).toBe("Conditional")
  })

  test("expression with nested function calls", () => {
    const ast = parse("{ foo(bar(1, 2), 3); }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.args.length).toBe(2)
    expect(stmt.expression.args[0].type).toBe("CallExpression")
  })
})

describe("parser Map operations", () => {
  test("empty Map literal", () => {
    const ast = parse("{ m = {}; }")
    const stmt = ast.statements[0] as any
    // = is now assignment, so m = {} is AssignmentExpression
    expect(stmt.expression.type).toBe("AssignmentExpression")
    expect(stmt.expression.value.type).toBe("MapLiteral")
    expect(stmt.expression.value.entries.length).toBe(0)
  })

  test("Map literal with entries", () => {
    const ast = parse('{ m = { host: "localhost", port: 8080 }; }')
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("AssignmentExpression")
    expect(stmt.expression.value.type).toBe("MapLiteral")
    expect(stmt.expression.value.entries.length).toBe(2)
    expect(stmt.expression.value.entries[0].key).toBe("host")
    expect(stmt.expression.value.entries[1].key).toBe("port")
  })

  test("Map dot access", () => {
    const ast = parse("{ x := config.host; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.value.type).toBe("MemberExpression")
    expect(stmt.expression.value.object.name).toBe("config")
    expect(stmt.expression.value.property).toBe("host")
  })

  test("Map bracket access with string", () => {
    const ast = parse('{ x := config["port"]; }')
    const stmt = ast.statements[0] as any
    expect(stmt.expression.value.type).toBe("IndexExpression")
    expect(stmt.expression.value.object.name).toBe("config")
  })

  test("Map bracket access with variable", () => {
    const ast = parse("{ key := \"host\"; x := config[key]; }")
    const block = ast.statements[0] as any
    const stmts = block.statements as any[]
    const indexStmt = stmts[1]
    expect(indexStmt.expression.value.type).toBe("IndexExpression")
  })

  test("Map assignment via dot access", () => {
    const ast = parse("{ config.host := \"127.0.0.1\"; }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("ExpressionStatement")
    expect(stmt.expression.type).toBe("AssignmentExpression")
    expect(stmt.expression.target.type).toBe("MemberExpression")
  })

  test("Map assignment via bracket access", () => {
    const ast = parse('{ config["port"] := 3000; }')
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("ExpressionStatement")
    expect(stmt.expression.type).toBe("AssignmentExpression")
    expect(stmt.expression.target.type).toBe("IndexExpression")
  })
})

describe("parser struct definitions", () => {
  test("simple struct definition", () => {
    const ast = parse("struct Point { x: f64, y: f64 }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("StructDeclaration")
    expect(stmt.name).toBe("Point")
    expect(stmt.fields.length).toBe(2)
    expect(stmt.fields[0].name).toBe("x")
    expect(stmt.fields[0].fieldType.type).toBe("FloatType")
    expect(stmt.fields[0].fieldType.kind).toBe("f64")
    expect(stmt.fields[1].name).toBe("y")
    expect(stmt.fields[1].fieldType.type).toBe("FloatType")
    expect(stmt.fields[1].fieldType.kind).toBe("f64")
  })

  test("struct with multiple field types", () => {
    const ast = parse(`struct Particle {
      x: f64,
      y: f64,
      mass: f64,
      id: i32
    }`)
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("StructDeclaration")
    expect(stmt.name).toBe("Particle")
    expect(stmt.fields.length).toBe(4)
    expect(stmt.fields[0].fieldType.type).toBe("FloatType")
    expect(stmt.fields[0].fieldType.kind).toBe("f64")
    expect(stmt.fields[3].fieldType.type).toBe("IntegerType")
    expect(stmt.fields[3].fieldType.kind).toBe("i32")
  })

  test("struct definition with i64 field", () => {
    const ast = parse(`struct Counter {
      value: i64,
      name: string
    }`)
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("StructDeclaration")
    expect(stmt.fields[0].fieldType.type).toBe("IntegerType")
    expect(stmt.fields[0].fieldType.kind).toBe("i64")
  })
})

describe("parser struct literals", () => {
  test("struct literal with type annotation", () => {
    const ast = parse("{ p: Point := { x: 1.0, y: 2.0 }; }")
    const stmt = ast.statements[0] as any
    expect(stmt.declaredType.name).toBe("Point")
    expect(stmt.initializer.type).toBe("MapLiteral")
    expect(stmt.initializer.entries.length).toBe(2)
  })

  test("struct literal field values", () => {
    const ast = parse("{ p: Point := { x: 1.5, y: 2.5 }; }")
    const stmt = ast.statements[0] as any
    const lit = stmt.initializer as any
    expect(lit.entries[0].key).toBe("x")
    expect(lit.entries[0].value.value).toBe("1.5")
    expect(lit.entries[1].key).toBe("y")
    expect(lit.entries[1].value.value).toBe("2.5")
  })

  test("struct literal with expression values", () => {
    const ast = parse("{ p: Point := { x: 1.0 + 2.0, y: 3.0 * 4.0 }; }")
    const stmt = ast.statements[0] as any
    const lit = stmt.initializer as any
    expect(lit.entries[0].value.type).toBe("BinaryExpression")
    expect(lit.entries[1].value.type).toBe("BinaryExpression")
  })
})

describe("parser struct field access", () => {
  test("struct field read access", () => {
    const ast = parse("{ x := p.x; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.value.type).toBe("MemberExpression")
    expect(stmt.expression.value.object.name).toBe("p")
    expect(stmt.expression.value.property).toBe("x")
  })

  test("struct field assignment", () => {
    const ast = parse("{ p.x := 5.0; }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("ExpressionStatement")
    expect(stmt.expression.target.type).toBe("MemberExpression")
    expect(stmt.expression.target.property).toBe("x")
  })

  test("nested struct field access", () => {
    const ast = parse("{ x := outer.inner.value; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.value.type).toBe("MemberExpression")
    expect(stmt.expression.value.object.type).toBe("MemberExpression")
  })
})

describe("parser AoS (Array of Structs)", () => {
  test("AoS type annotation", () => {
    const ast = parse("{ particles: [Particle] := []; }")
    const stmt = ast.statements[0] as any
    expect(stmt.declaredType.type).toBe("AOSType")
    expect(stmt.declaredType.elementType).toBe("Particle")
  })

  test("AoS with fixed capacity", () => {
    const ast = parse("{ particles: [Particle; 100] := []; }")
    const stmt = ast.statements[0] as any
    expect(stmt.declaredType.type).toBe("AOSType")
    expect(stmt.declaredType.elementType).toBe("Particle")
    expect(stmt.declaredType.capacity).toBe(100)
  })

  test("AoS literal with struct elements", () => {
    const ast = parse("{ particles: [Particle] := [{ x: 0.0, y: 0.0 }, { x: 1.0, y: 1.0 }]; }")
    const stmt = ast.statements[0] as any
    expect(stmt.initializer.type).toBe("ArrayLiteral")
    expect(stmt.initializer.elements.length).toBe(2)
  })

  test("AoS index then field access", () => {
    const ast = parse("{ x := particles[0].x; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.value.type).toBe("MemberExpression")
    expect(stmt.expression.value.object.type).toBe("IndexExpression")
  })

  test("AoS element field assignment", () => {
    const ast = parse("{ particles[0].x := 5.0; }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("ExpressionStatement")
    expect(stmt.expression.target.type).toBe("MemberExpression")
  })

  test("AoS full element assignment", () => {
    const ast = parse("{ particles[0] := { x: 2.0, y: 2.0 }; }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("ExpressionStatement")
    expect(stmt.expression.target.type).toBe("IndexExpression")
    expect(stmt.expression.value.type).toBe("MapLiteral")
  })
})

describe("parser SoA (Struct of Arrays) - new design", () => {
  test("SoA declaration with capacity", () => {
    const ast = parse(`soa datums: Datum[100]`)
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("SoADeclaration")
    expect(stmt.name).toBe("datums")
    expect(stmt.structName).toBe("Datum")
    expect(stmt.capacity).toBe(100)
  })

  test("SoA declaration without capacity (dynamic)", () => {
    const ast = parse(`soa items: Item[]`)
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("SoADeclaration")
    expect(stmt.name).toBe("items")
    expect(stmt.structName).toBe("Item")
    expect(stmt.capacity).toBeNull()
  })

  test("SoA index-then-field access parses as MemberExpression(IndexExpression)", () => {
    const ast = parse("{ x := datums[0].id; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.value.type).toBe("MemberExpression")
    expect(stmt.expression.value.object.type).toBe("IndexExpression")
    expect(stmt.expression.value.object.object.type).toBe("Identifier")
    expect(stmt.expression.value.object.object.name).toBe("datums")
    expect(stmt.expression.value.property).toBe("id")
  })

  test("SoA field assignment parses correctly", () => {
    const ast = parse("{ datums[0].id := 42; }")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("ExpressionStatement")
    expect(stmt.expression.target.type).toBe("MemberExpression")
    expect(stmt.expression.target.object.type).toBe("IndexExpression")
    expect(stmt.expression.target.property).toBe("id")
  })
})
