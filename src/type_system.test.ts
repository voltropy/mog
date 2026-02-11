import { describe, test, expect } from "bun:test"
import { Lexer } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"
import {
  i8, i16, i32, i64,
  u8, u16, u32, u64,
  f16, f32, f64, bf16,
  boolType,
  IntegerType,
  UnsignedType,
  FloatType,
  BoolType,
  TypeAliasType,
  ArrayType,
  MapType,
  sameType,
  canCoerceWithWidening,
  compatibleTypes,
} from "./types"

// ── helpers ──────────────────────────────────────────────────────────────────

function lex(source: string) {
  const lexer = new Lexer(source)
  return lexer.tokenize().filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
}

function parse(source: string) {
  return parseTokens(lex(source))
}

function analyzeSource(source: string) {
  const ast = parse(source)
  const analyzer = new SemanticAnalyzer()
  const errors = analyzer.analyze(ast)
  return { ast, errors }
}

function expectNoErrors(source: string) {
  const { errors } = analyzeSource(source)
  if (errors.length > 0) {
    throw new Error(`Expected no errors but got:\n${errors.map(e => e.message).join("\n")}`)
  }
}

function expectErrors(source: string, count?: number) {
  const { errors } = analyzeSource(source)
  if (count !== undefined) {
    expect(errors.length).toBe(count)
  } else {
    expect(errors.length).toBeGreaterThan(0)
  }
  return errors
}

// ── Lexer tests ──────────────────────────────────────────────────────────────

describe("Phase 1: Lexer - new type tokens", () => {
  test("lexes 'int' as TYPE token", () => {
    const tokens = lex("int")
    expect(tokens.length).toBe(1)
    expect(tokens[0].type).toBe("TYPE")
    expect(tokens[0].value).toBe("int")
  })

  test("lexes 'float' as TYPE token", () => {
    const tokens = lex("float")
    expect(tokens.length).toBe(1)
    expect(tokens[0].type).toBe("TYPE")
    expect(tokens[0].value).toBe("float")
  })

  test("lexes 'bool' as TYPE token", () => {
    const tokens = lex("bool")
    expect(tokens.length).toBe(1)
    expect(tokens[0].type).toBe("TYPE")
    expect(tokens[0].value).toBe("bool")
  })

  test("lexes 'bf16' as TYPE token", () => {
    const tokens = lex("bf16")
    expect(tokens.length).toBe(1)
    expect(tokens[0].type).toBe("TYPE")
    expect(tokens[0].value).toBe("bf16")
  })

  test("lexes 'as' as keyword token", () => {
    const tokens = lex("as")
    expect(tokens.length).toBe(1)
    expect(tokens[0].type).toBe("as")
    expect(tokens[0].value).toBe("as")
  })

  test("lexes 'type' as keyword token", () => {
    const tokens = lex("type")
    expect(tokens.length).toBe(1)
    expect(tokens[0].type).toBe("type_kw")
    expect(tokens[0].value).toBe("type")
  })

  test("lexes 'true' as keyword token", () => {
    const tokens = lex("true")
    expect(tokens.length).toBe(1)
    expect(tokens[0].type).toBe("true")
    expect(tokens[0].value).toBe("true")
  })

  test("lexes 'false' as keyword token", () => {
    const tokens = lex("false")
    expect(tokens.length).toBe(1)
    expect(tokens[0].type).toBe("false")
    expect(tokens[0].value).toBe("false")
  })

  test("int does not conflict with 'in' keyword", () => {
    const tokens = lex("in int")
    expect(tokens[0].type).toBe("in")
    expect(tokens[1].type).toBe("TYPE")
    expect(tokens[1].value).toBe("int")
  })

  test("existing types still lex correctly", () => {
    const tokens = lex("i32 u64 f64 ptr string")
    expect(tokens[0].type).toBe("TYPE")
    expect(tokens[0].value).toBe("i32")
    expect(tokens[1].type).toBe("TYPE")
    expect(tokens[1].value).toBe("u64")
    expect(tokens[2].type).toBe("TYPE")
    expect(tokens[2].value).toBe("f64")
    expect(tokens[3].type).toBe("TYPE")
    expect(tokens[3].value).toBe("ptr")
    expect(tokens[4].type).toBe("TYPE")
    expect(tokens[4].value).toBe("string")
  })
})

// ── Types.ts tests ───────────────────────────────────────────────────────────

describe("Phase 1: Types - bf16 and BoolType", () => {
  test("bf16 singleton exists and has correct properties", () => {
    expect(bf16).toBeInstanceOf(FloatType)
    expect(bf16.kind).toBe("bf16")
    expect(bf16.bits).toBe(16)
    expect(bf16.toString()).toBe("bf16")
  })

  test("BoolType has correct properties", () => {
    expect(boolType).toBeInstanceOf(BoolType)
    expect(boolType.bits).toBe(1)
    expect(boolType.toString()).toBe("bool")
  })

  test("sameType works for BoolType", () => {
    expect(sameType(boolType, new BoolType())).toBe(true)
    expect(sameType(boolType, i32)).toBe(false)
  })

  test("TypeAliasType resolves correctly", () => {
    const alias = new TypeAliasType("MyInt", i64)
    expect(alias.name).toBe("MyInt")
    expect(alias.aliasedType).toBe(i64)
    expect(alias.toString()).toBe("MyInt")
  })

  test("sameType resolves type aliases", () => {
    const alias1 = new TypeAliasType("A", i64)
    const alias2 = new TypeAliasType("B", i64)
    expect(sameType(alias1, alias2)).toBe(true)
    expect(sameType(alias1, i64)).toBe(true)
    expect(sameType(i64, alias1)).toBe(true)
  })

  test("compatibleTypes resolves type aliases", () => {
    const alias = new TypeAliasType("MyFloat", f64)
    expect(compatibleTypes(alias, f64)).toBe(true)
    expect(compatibleTypes(f64, alias)).toBe(true)
  })
})

describe("Phase 1: Types - integer widening", () => {
  test("canCoerceWithWidening allows i8 -> i16", () => {
    expect(canCoerceWithWidening(i8, i16)).toBe(true)
  })

  test("canCoerceWithWidening allows i8 -> i32", () => {
    expect(canCoerceWithWidening(i8, i32)).toBe(true)
  })

  test("canCoerceWithWidening allows i32 -> i64 (int)", () => {
    expect(canCoerceWithWidening(i32, i64)).toBe(true)
  })

  test("canCoerceWithWidening allows i8 -> i64", () => {
    expect(canCoerceWithWidening(i8, i64)).toBe(true)
  })

  test("canCoerceWithWidening allows i64 -> i32 (literal coercion)", () => {
    // For literal assignments, any same-family integer conversion is allowed
    expect(canCoerceWithWidening(i64, i32)).toBe(true)
  })

  test("canCoerceWithWidening allows u8 -> u16", () => {
    expect(canCoerceWithWidening(u8, u16)).toBe(true)
  })

  test("canCoerceWithWidening allows u16 -> u32", () => {
    expect(canCoerceWithWidening(u16, u32)).toBe(true)
  })

  test("canCoerceWithWidening allows u64 -> u8 (literal coercion)", () => {
    // For literal assignments, any same-family unsigned conversion is allowed
    expect(canCoerceWithWidening(u64, u8)).toBe(true)
  })

  test("canCoerceWithWidening allows f32 -> f64 (existing)", () => {
    expect(canCoerceWithWidening(f32, f64)).toBe(true)
  })

  test("canCoerceWithWidening allows f64 -> f32 (literal coercion)", () => {
    // For literal assignments, any same-family float conversion is allowed
    expect(canCoerceWithWidening(f64, f32)).toBe(true)
  })
})

// ── Parser tests ─────────────────────────────────────────────────────────────

describe("Phase 1: Parser - int and float type annotations", () => {
  test("parses variable with int type", () => {
    const ast = parse("x: int := 42;")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("VariableDeclaration")
    expect(stmt.name).toBe("x")
    expect(stmt.varType).toBeInstanceOf(IntegerType)
    expect(stmt.varType.kind).toBe("i64")
  })

  test("parses variable with float type", () => {
    const ast = parse("y: float := 3.14;")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("VariableDeclaration")
    expect(stmt.name).toBe("y")
    expect(stmt.varType).toBeInstanceOf(FloatType)
    expect(stmt.varType.kind).toBe("f64")
  })

  test("parses variable with bf16 type", () => {
    const ast = parse("w: bf16 := 1.0;")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("VariableDeclaration")
    expect(stmt.name).toBe("w")
    expect(stmt.varType).toBeInstanceOf(FloatType)
    expect(stmt.varType.kind).toBe("bf16")
  })

  test("parses variable with bool type", () => {
    const ast = parse("flag: bool := true;")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("VariableDeclaration")
    expect(stmt.name).toBe("flag")
    expect(stmt.varType).toBeInstanceOf(BoolType)
  })

  test("int as function parameter type", () => {
    const ast = parse("fn add(a: int, b: int) -> int { return a; }")
    const fn = ast.statements[0] as any
    expect(fn.params[0].paramType).toBeInstanceOf(IntegerType)
    expect(fn.params[0].paramType.kind).toBe("i64")
    expect(fn.params[1].paramType).toBeInstanceOf(IntegerType)
    expect(fn.params[1].paramType.kind).toBe("i64")
    expect(fn.returnType).toBeInstanceOf(IntegerType)
    expect(fn.returnType.kind).toBe("i64")
  })

  test("float as function return type", () => {
    const ast = parse("fn pi() -> float { return 3.14; }")
    const fn = ast.statements[0] as any
    expect(fn.returnType).toBeInstanceOf(FloatType)
    expect(fn.returnType.kind).toBe("f64")
  })

  test("int in array type [int; 3]", () => {
    const ast = parse("arr: [int; 3] := [1, 2, 3];")
    const stmt = ast.statements[0] as any
    expect(stmt.varType).toBeInstanceOf(ArrayType)
    expect(stmt.varType.elementType).toBeInstanceOf(IntegerType)
    expect(stmt.varType.elementType.kind).toBe("i64")
    expect(stmt.varType.dimensions).toEqual([3])
  })

  test("float in dynamic array [float]", () => {
    const ast = parse("arr: [float] := [1.0, 2.0];")
    const stmt = ast.statements[0] as any
    expect(stmt.varType).toBeInstanceOf(ArrayType)
    expect(stmt.varType.elementType).toBeInstanceOf(FloatType)
    expect(stmt.varType.elementType.kind).toBe("f64")
  })

  test("bf16 in array type [bf16; 4]", () => {
    const ast = parse("arr: [bf16; 4] := [1.0, 2.0, 3.0, 4.0];")
    const stmt = ast.statements[0] as any
    expect(stmt.varType).toBeInstanceOf(ArrayType)
    expect(stmt.varType.elementType).toBeInstanceOf(FloatType)
    expect(stmt.varType.elementType.kind).toBe("bf16")
    expect(stmt.varType.dimensions).toEqual([4])
  })
})

describe("Phase 1: Parser - boolean literals", () => {
  test("parses true literal", () => {
    const ast = parse("{ true; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("BooleanLiteral")
    expect(stmt.expression.value).toBe(true)
  })

  test("parses false literal", () => {
    const ast = parse("{ false; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("BooleanLiteral")
    expect(stmt.expression.value).toBe(false)
  })

  test("bool variable with true value", () => {
    const ast = parse("flag: bool := true;")
    const stmt = ast.statements[0] as any
    expect(stmt.name).toBe("flag")
    expect(stmt.varType).toBeInstanceOf(BoolType)
    expect(stmt.value.type).toBe("BooleanLiteral")
    expect(stmt.value.value).toBe(true)
  })

  test("bool variable with false value", () => {
    const ast = parse("flag: bool := false;")
    const stmt = ast.statements[0] as any
    expect(stmt.value.type).toBe("BooleanLiteral")
    expect(stmt.value.value).toBe(false)
  })
})

describe("Phase 1: Parser - as cast syntax", () => {
  test("simple as cast: x as i32", () => {
    const ast = parse("{ x as i32; }")
    const stmt = ast.statements[0] as any
    const expr = stmt.expression
    expect(expr.type).toBe("CastExpression")
    expect(expr.targetType).toBeInstanceOf(IntegerType)
    expect(expr.targetType.kind).toBe("i32")
    expect(expr.value.type).toBe("Identifier")
    expect(expr.value.name).toBe("x")
  })

  test("as cast to float", () => {
    const ast = parse("{ y as float; }")
    const stmt = ast.statements[0] as any
    const expr = stmt.expression
    expect(expr.type).toBe("CastExpression")
    expect(expr.targetType).toBeInstanceOf(FloatType)
    expect(expr.targetType.kind).toBe("f64")
  })

  test("as cast to int", () => {
    const ast = parse("{ z as int; }")
    const stmt = ast.statements[0] as any
    const expr = stmt.expression
    expect(expr.type).toBe("CastExpression")
    expect(expr.targetType).toBeInstanceOf(IntegerType)
    expect(expr.targetType.kind).toBe("i64")
  })

  test("as cast to bf16", () => {
    const ast = parse("{ val as bf16; }")
    const stmt = ast.statements[0] as any
    const expr = stmt.expression
    expect(expr.type).toBe("CastExpression")
    expect(expr.targetType).toBeInstanceOf(FloatType)
    expect(expr.targetType.kind).toBe("bf16")
  })

  test("chained as casts", () => {
    const ast = parse("{ x as f32 as f64; }")
    const stmt = ast.statements[0] as any
    const outer = stmt.expression
    expect(outer.type).toBe("CastExpression")
    expect(outer.targetType).toBeInstanceOf(FloatType)
    expect(outer.targetType.kind).toBe("f64")
    const inner = outer.value
    expect(inner.type).toBe("CastExpression")
    expect(inner.targetType).toBeInstanceOf(FloatType)
    expect(inner.targetType.kind).toBe("f32")
  })

  test("old cast<T>(expr) syntax still works", () => {
    const ast = parse("{ cast<i32>(x); }")
    const stmt = ast.statements[0] as any
    const expr = stmt.expression
    expect(expr.type).toBe("CastExpression")
    expect(expr.targetType).toBeInstanceOf(IntegerType)
    expect(expr.targetType.kind).toBe("i32")
  })
})

describe("Phase 1: Parser - type aliases", () => {
  test("simple type alias", () => {
    const ast = parse("type MyInt = i64;")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("TypeAliasDeclaration")
    expect(stmt.name).toBe("MyInt")
    expect(stmt.aliasedType).toBeInstanceOf(TypeAliasType)
    expect(stmt.aliasedType.aliasedType).toBeInstanceOf(IntegerType)
    expect(stmt.aliasedType.aliasedType.kind).toBe("i64")
  })

  test("type alias with int", () => {
    const ast = parse("type Count = int;")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("TypeAliasDeclaration")
    expect(stmt.name).toBe("Count")
    expect(stmt.aliasedType.aliasedType).toBeInstanceOf(IntegerType)
    expect(stmt.aliasedType.aliasedType.kind).toBe("i64")
  })

  test("type alias with float", () => {
    const ast = parse("type Score = float;")
    const stmt = ast.statements[0] as any
    expect(stmt.aliasedType.aliasedType).toBeInstanceOf(FloatType)
    expect(stmt.aliasedType.aliasedType.kind).toBe("f64")
  })

  test("type alias with map type", () => {
    const ast = parse("type Config = {string: string};")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("TypeAliasDeclaration")
    expect(stmt.name).toBe("Config")
    expect(stmt.aliasedType).toBeInstanceOf(TypeAliasType)
    expect(stmt.aliasedType.aliasedType).toBeInstanceOf(MapType)
  })
})

describe("Phase 1: Parser - := binding vs = reassignment", () => {
  test(":= for initial binding", () => {
    const ast = parse("x: i32 := 10;")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("VariableDeclaration")
    expect(stmt.name).toBe("x")
    expect(stmt.varType).toBeInstanceOf(IntegerType)
  })

  test("= for reassignment in expression context", () => {
    const ast = parse("{ x = 20; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("AssignmentExpression")
    expect(stmt.expression.name).toBe("x")
  })

  test(":= still works for reassignment", () => {
    const ast = parse("{ x := 30; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("AssignmentExpression")
    expect(stmt.expression.name).toBe("x")
  })

  test("= for variable declaration still works", () => {
    const ast = parse("x: i32 = 10;")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("VariableDeclaration")
    expect(stmt.name).toBe("x")
  })

  test("array index assignment with =", () => {
    const ast = parse("{ arr[0] = 42; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("AssignmentExpression")
    expect(stmt.expression.target.type).toBe("IndexExpression")
  })
})

// ── Analyzer tests ───────────────────────────────────────────────────────────

describe("Phase 1: Analyzer - int and float types", () => {
  test("int variable declaration accepted", () => {
    expectNoErrors("x: int := 42;")
  })

  test("float variable declaration accepted", () => {
    expectNoErrors("y: float := 3.14;")
  })

  test("bf16 variable declaration accepted", () => {
    expectNoErrors("w: bf16 := 1.0;")
  })

  test("function with int parameters", () => {
    expectNoErrors("fn add(a: int, b: int) -> int { return a; }")
  })

  test("function with float return type", () => {
    expectNoErrors("fn pi() -> float { return 3.14; }")
  })
})

describe("Phase 1: Analyzer - bool type", () => {
  test("bool variable with true", () => {
    expectNoErrors("flag: bool := true;")
  })

  test("bool variable with false", () => {
    expectNoErrors("flag: bool := false;")
  })
})

describe("Phase 1: Analyzer - implicit integer widening", () => {
  test("i8 literal widens to i16", () => {
    expectNoErrors("x: i16 := 5;")
  })

  test("i8 literal widens to i32", () => {
    expectNoErrors("x: i32 := 5;")
  })

  test("i32 literal widens to i64 (int)", () => {
    expectNoErrors("x: int := 42;")
  })

  test("f32 literal widens to f64 (float)", () => {
    expectNoErrors("x: float := 3.14;")
  })

  test("f32 literal widens to f64 (existing behavior)", () => {
    expectNoErrors("x: f64 := 3.14;")
  })
})

describe("Phase 1: Analyzer - as cast expressions", () => {
  test("as cast from i64 to i32 produces warning", () => {
    const errors = expectErrors(`
      x: i64 := 100;
      y: i32 := cast<i32>(x);
    `)
    // Should warn about lossy cast
    expect(errors.some(e => e.message.includes("Lossy cast"))).toBe(true)
  })
})

describe("Phase 1: Analyzer - type aliases", () => {
  test("type alias resolves in variable declarations", () => {
    expectNoErrors(`
      type Count = i64;
      x: Count := 42;
    `)
  })

  test("type alias with map type resolves", () => {
    // Map alias parses correctly
    const ast = parse("type Config = {string: string};")
    expect(ast.statements[0]).toBeDefined()
    expect((ast.statements[0] as any).type).toBe("TypeAliasDeclaration")
  })
})

// ── Integration tests ────────────────────────────────────────────────────────

describe("Phase 1: Integration - mixed features", () => {
  test("int type with := binding", () => {
    const ast = parse("count: int := 0;")
    const stmt = ast.statements[0] as any
    expect(stmt.type).toBe("VariableDeclaration")
    expect(stmt.varType).toBeInstanceOf(IntegerType)
    expect(stmt.varType.kind).toBe("i64")
  })

  test("float with as cast", () => {
    const ast = parse("{ x as float; }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("CastExpression")
    expect(stmt.expression.targetType).toBeInstanceOf(FloatType)
    expect(stmt.expression.targetType.kind).toBe("f64")
  })

  test("existing i32/f64 syntax unchanged", () => {
    expectNoErrors("x: i32 := 42;")
    expectNoErrors("y: f64 := 3.14;")
  })

  test("existing cast<T>() syntax unchanged", () => {
    const ast = parse("{ cast<f32>(x); }")
    const stmt = ast.statements[0] as any
    expect(stmt.expression.type).toBe("CastExpression")
    expect(stmt.expression.targetType).toBeInstanceOf(FloatType)
    expect(stmt.expression.targetType.kind).toBe("f32")
  })

  test("struct declarations still work with new keywords", () => {
    expectNoErrors(`
      struct Point {
        x: float;
        y: float;
      }
    `)
  })
})
