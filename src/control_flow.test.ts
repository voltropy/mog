import { describe, it, expect } from "bun:test"
import { tokenize } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"
import { generateLLVMIR } from "./llvm_codegen"
import { compile } from "./compiler"
import { i64, ArrayType, IntegerType } from "./types"

// ============================================================================
// Helpers
// ============================================================================

function pos(index: number = 0): { start: any; end: any } {
  return { start: { line: 1, column: 1, index }, end: { line: 1, column: 1, index } }
}

function program(...statements: any[]): any {
  return { type: "Program", statements, scopeId: "global", position: pos() }
}

function block(...statements: any[]): any {
  return { type: "Block", statements, scopeId: "block", position: pos() }
}

function varDecl(name: string, type: any, value: any): any {
  return { type: "VariableDeclaration", name, varType: type, value, position: pos() }
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

function exprStmt(expression: any): any {
  return { type: "ExpressionStatement", expression, position: pos() }
}

function funcDecl(name: string, params: any[], returnType: any, body: any): any {
  return { type: "FunctionDeclaration", name, params, returnType, body, position: pos() }
}

function breakStmt(): any {
  return { type: "Break", position: pos() }
}

function continueStmt(): any {
  return { type: "Continue", position: pos() }
}

/** Parse source to tokens, filter whitespace/comments */
function lex(source: string) {
  const tokens = tokenize(source)
  return tokens.filter((t) => t.type !== "WHITESPACE" && t.type !== "COMMENT")
}

/** Parse source to AST, returning the inner block's statements for convenience */
function parse(source: string) {
  const ast = parseTokens(lex(source))
  // The top-level { } wraps everything in a Block - unwrap for easier access
  if (ast.statements.length === 1 && (ast.statements[0] as any).type === "Block") {
    return { ...ast, statements: (ast.statements[0] as any).statements }
  }
  return ast
}

/** Analyze AST, return errors */
function analyze(ast: any) {
  const analyzer = new SemanticAnalyzer()
  return analyzer.analyze(ast)
}

/** Full compile pipeline, return result */
async function fullCompile(source: string) {
  return await compile(source)
}

// ============================================================================
// LEXER TESTS
// ============================================================================

describe("Control Flow - Lexer", () => {
  describe("RANGE token (..)", () => {
    it("should tokenize .. as RANGE", () => {
      const tokens = lex("0..10")
      expect(tokens.some(t => t.type === "RANGE")).toBe(true)
      const rangeToken = tokens.find(t => t.type === "RANGE")
      expect(rangeToken?.value).toBe("..")
    })

    it("should tokenize .. separately from single DOT", () => {
      const tokens = lex("a.b")
      expect(tokens.some(t => t.type === "DOT")).toBe(true)
      expect(tokens.some(t => t.type === "RANGE")).toBe(false)
    })

    it("should tokenize for i in 0..10 correctly", () => {
      const tokens = lex("for i in 0..10 { }")
      const types = tokens.map(t => t.type)
      expect(types).toContain("for")
      expect(types).toContain("in")
      expect(types).toContain("RANGE")
    })

    it("should not confuse .. with two dots in member access", () => {
      const tokens = lex("a..b")
      const rangeTokens = tokens.filter(t => t.type === "RANGE")
      expect(rangeTokens.length).toBe(1)
    })
  })

  describe("FAT_ARROW token (=>)", () => {
    it("should tokenize => as FAT_ARROW", () => {
      const tokens = lex("x => y")
      const fatArrow = tokens.find(t => t.type === "FAT_ARROW")
      expect(fatArrow).toBeDefined()
      expect(fatArrow?.value).toBe("=>")
    })

    it("should not confuse => with = followed by >", () => {
      const tokens = lex("=>")
      expect(tokens.filter(t => t.type === "FAT_ARROW").length).toBe(1)
      expect(tokens.filter(t => t.type === "ASSIGN").length).toBe(0)
      expect(tokens.filter(t => t.type === "GREATER").length).toBe(0)
    })
  })

  describe("match keyword", () => {
    it("should tokenize match as keyword", () => {
      const tokens = lex("match x { }")
      expect(tokens.some(t => t.type === "match")).toBe(true)
    })

    it("should not tokenize match_thing as match keyword", () => {
      const tokens = lex("match_thing")
      expect(tokens.some(t => t.type === "match")).toBe(false)
      expect(tokens.some(t => t.type === "IDENTIFIER")).toBe(true)
    })
  })

  describe("UNDERSCORE token", () => {
    it("should tokenize _ as UNDERSCORE", () => {
      const tokens = lex("_")
      expect(tokens.some(t => t.type === "UNDERSCORE")).toBe(true)
    })

    it("should tokenize _x as IDENTIFIER, not UNDERSCORE", () => {
      const tokens = lex("_x")
      expect(tokens.some(t => t.type === "IDENTIFIER")).toBe(true)
      expect(tokens.some(t => t.type === "UNDERSCORE")).toBe(false)
    })
  })
})

// ============================================================================
// PARSER TESTS
// ============================================================================

describe("Control Flow - Parser", () => {
  describe("for..in range loops", () => {
    it("should parse for i in 0..10 as ForInRange", () => {
      const ast = parse("{ for i in 0..10 { i; } }")
      const forStmt = ast.statements[0]
      expect(forStmt.type).toBe("ForInRange")
      expect(forStmt.variable).toBe("i")
      expect(forStmt.start.type).toBe("NumberLiteral")
      expect(forStmt.start.value).toBe("0")
      expect(forStmt.end.type).toBe("NumberLiteral")
      expect(forStmt.end.value).toBe("10")
      expect(forStmt.body.type).toBe("Block")
    })

    it("should parse range with expressions", () => {
      const ast = parse("{ x: i64 = 5; for i in x..20 { i; } }")
      const forStmt = ast.statements[1]
      expect(forStmt.type).toBe("ForInRange")
      expect(forStmt.start.type).toBe("Identifier")
      expect(forStmt.start.name).toBe("x")
    })
  })

  describe("for item in array (without type annotation)", () => {
    it("should parse for item in arr as ForEachLoop with null varType", () => {
      const ast = parse("{ arr: i64[] = [1, 2, 3]; for item in arr { item; } }")
      const forStmt = ast.statements[1]
      expect(forStmt.type).toBe("ForEachLoop")
      expect(forStmt.variable).toBe("item")
      expect(forStmt.varType).toBeNull()
      expect(forStmt.array.type).toBe("Identifier")
      expect(forStmt.array.name).toBe("arr")
    })
  })

  describe("for i, item in array (index + value)", () => {
    it("should parse for i, item in arr as ForInIndex", () => {
      const ast = parse("{ arr: i64[] = [1, 2, 3]; for i, item in arr { i; } }")
      const forStmt = ast.statements[1]
      expect(forStmt.type).toBe("ForInIndex")
      expect(forStmt.indexVariable).toBe("i")
      expect(forStmt.valueVariable).toBe("item")
      expect(forStmt.iterable.type).toBe("Identifier")
      expect(forStmt.iterable.name).toBe("arr")
    })
  })

  describe("if-expression", () => {
    it("should parse if as expression in variable initializer", () => {
      const ast = parse("{ x: i64 = 5; result: i64 = if x > 0 { 1; } else { 0; }; }")
      // The variable declaration with an if-expression
      const decl = ast.statements[1]
      expect(decl.type).toBe("VariableDeclaration")
      expect(decl.name).toBe("result")
      expect(decl.value.type).toBe("IfExpression")
      expect(decl.value.condition.type).toBe("BinaryExpression")
      expect(decl.value.trueBranch.type).toBe("Block")
      expect(decl.value.falseBranch.type).toBe("Block")
    })

    it("should parse if-else if-else expression chain", () => {
      const ast = parse("{ x: i64 = 5; r: i64 = if x > 10 { 1; } else if x > 0 { 2; } else { 3; }; }")
      const decl = ast.statements[1]
      expect(decl.value.type).toBe("IfExpression")
      expect(decl.value.falseBranch.type).toBe("IfExpression")
      expect(decl.value.falseBranch.falseBranch.type).toBe("Block")
    })
  })

  describe("match expression", () => {
    it("should parse match with literal patterns", () => {
      const ast = parse("{ x: i64 = 1; r: i64 = match x { 1 => 10, 2 => 20, _ => 0 }; }")
      const decl = ast.statements[1]
      expect(decl.value.type).toBe("MatchExpression")
      expect(decl.value.subject.name).toBe("x")
      expect(decl.value.arms.length).toBe(3)
      expect(decl.value.arms[0].pattern.type).toBe("LiteralPattern")
      expect(decl.value.arms[1].pattern.type).toBe("LiteralPattern")
      expect(decl.value.arms[2].pattern.type).toBe("WildcardPattern")
    })

    it("should parse match with block bodies", () => {
      const ast = parse("{ x: i64 = 1; r: i64 = match x { 1 => { 10; }, _ => { 0; } }; }")
      const decl = ast.statements[1]
      expect(decl.value.type).toBe("MatchExpression")
      expect(decl.value.arms.length).toBe(2)
    })

    it("should parse match without trailing comma", () => {
      const ast = parse("{ x: i64 = 1; r: i64 = match x { 1 => 10, _ => 0 }; }")
      const decl = ast.statements[1]
      expect(decl.value.type).toBe("MatchExpression")
      expect(decl.value.arms.length).toBe(2)
    })
  })
})

// ============================================================================
// ANALYZER TESTS
// ============================================================================

describe("Control Flow - Analyzer", () => {
  describe("for..in range", () => {
    it("should accept for-in range with integer bounds", () => {
      const ast = program(
        funcDecl("main", [], i64, block(
          { type: "ForInRange", variable: "i", start: num(0, i64), end: num(10, i64), body: block(), position: pos() },
        ))
      )
      const errors = analyze(ast)
      expect(errors).toHaveLength(0)
    })

    it("should declare loop variable in scope", () => {
      // Build a for-in range that uses the loop variable in the body
      const ast = program(
        funcDecl("main", [], i64, block(
          { type: "ForInRange", variable: "i", start: num(0, i64), end: num(10, i64),
            body: block(exprStmt(ident("i"))), position: pos() },
        ))
      )
      const errors = analyze(ast)
      expect(errors).toHaveLength(0)
    })
  })

  describe("for-in index", () => {
    it("should accept for i, item in array", () => {
      const arrType = new ArrayType(new IntegerType("i64"), [])
      const ast = program(
        funcDecl("main", [], i64, block(
          varDecl("arr", arrType, { type: "ArrayLiteral", elements: [num(1, i64)], position: pos() }),
          { type: "ForInIndex", indexVariable: "i", valueVariable: "item",
            iterable: ident("arr"), body: block(exprStmt(ident("i")), exprStmt(ident("item"))), position: pos() },
        ))
      )
      const errors = analyze(ast)
      expect(errors).toHaveLength(0)
    })
  })

  describe("if-expression", () => {
    it("should accept if-expression with integer condition", () => {
      const ast = program(
        funcDecl("main", [], i64, block(
          varDecl("x", i64, num(5, i64)),
          varDecl("r", i64, {
            type: "IfExpression",
            condition: binary(ident("x"), ">", num(0, i64)),
            trueBranch: block(exprStmt(num(1, i64))),
            falseBranch: block(exprStmt(num(0, i64))),
            position: pos(),
          }),
        ))
      )
      const errors = analyze(ast)
      expect(errors).toHaveLength(0)
    })
  })

  describe("match expression", () => {
    it("should accept match with literal patterns", () => {
      const ast = program(
        funcDecl("main", [], i64, block(
          varDecl("x", i64, num(1, i64)),
          varDecl("r", i64, {
            type: "MatchExpression",
            subject: ident("x"),
            arms: [
              { pattern: { type: "LiteralPattern", value: num(1, i64) }, body: num(10, i64) },
              { pattern: { type: "WildcardPattern" }, body: num(0, i64) },
            ],
            position: pos(),
          }),
        ))
      )
      const errors = analyze(ast)
      expect(errors).toHaveLength(0)
    })
  })

  describe("break/continue in for..in loops", () => {
    it("should allow break inside for-in range", () => {
      const ast = program(
        funcDecl("main", [], i64, block(
          { type: "ForInRange", variable: "i", start: num(0, i64), end: num(10, i64),
            body: block(breakStmt()), position: pos() },
        ))
      )
      const errors = analyze(ast)
      expect(errors).toHaveLength(0)
    })

    it("should allow continue inside for-in range", () => {
      const ast = program(
        funcDecl("main", [], i64, block(
          { type: "ForInRange", variable: "i", start: num(0, i64), end: num(10, i64),
            body: block(continueStmt()), position: pos() },
        ))
      )
      const errors = analyze(ast)
      expect(errors).toHaveLength(0)
    })

    it("should allow break inside for-in index", () => {
      const arrType = new ArrayType(new IntegerType("i64"), [])
      const ast = program(
        funcDecl("main", [], i64, block(
          varDecl("arr", arrType, { type: "ArrayLiteral", elements: [num(1, i64)], position: pos() }),
          { type: "ForInIndex", indexVariable: "i", valueVariable: "item",
            iterable: ident("arr"), body: block(breakStmt()), position: pos() },
        ))
      )
      const errors = analyze(ast)
      expect(errors).toHaveLength(0)
    })

    it("should reject break outside of for-in range", () => {
      const ast = program(
        funcDecl("main", [], i64, block(
          breakStmt(),
        ))
      )
      const errors = analyze(ast)
      expect(errors.length).toBeGreaterThan(0)
      expect(errors[0].message).toContain("break")
    })
  })
})

// ============================================================================
// CODEGEN TESTS
// ============================================================================

describe("Control Flow - Codegen", () => {
  describe("for..in range codegen", () => {
    it("should generate LLVM IR for for i in 0..10", async () => {
      const source = `{
  fn test() -> i64 {
    sum: i64 = 0;
    for i in 0..10 {
      sum := sum + i;
    }
    return sum;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("alloca i64")  // loop variable allocation
      expect(result.llvmIR).toContain("icmp slt")    // i < end (exclusive range)
      expect(result.llvmIR).toContain("add i64")     // increment
      expect(result.llvmIR).toContain("br label")    // loop branch
    })

    it("should generate range loop with variable bounds", async () => {
      const source = `{
  fn test(n: i64) -> i64 {
    sum: i64 = 0;
    for i in 0..n {
      sum := sum + i;
    }
    return sum;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("icmp slt")
    })
  })

  describe("for..in array codegen (without type annotation)", () => {
    it("should generate LLVM IR for for item in array", async () => {
      const source = `{
  fn test() -> i64 {
    arr: i64[] = [10, 20, 30];
    sum: i64 = 0;
    for item in arr {
      sum := sum + item;
    }
    return sum;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("@array_length")
      expect(result.llvmIR).toContain("@array_get")
      expect(result.llvmIR).toContain("icmp slt")
    })
  })

  describe("for i, item in array codegen", () => {
    it("should generate LLVM IR for for i, item in array", async () => {
      const source = `{
  fn test() -> i64 {
    arr: i64[] = [10, 20, 30];
    sum: i64 = 0;
    for idx, val in arr {
      sum := sum + val + idx;
    }
    return sum;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("@array_length")
      expect(result.llvmIR).toContain("@array_get")
      expect(result.llvmIR).toContain("alloca i64")
    })
  })

  describe("if-expression codegen", () => {
    it("should generate LLVM IR for simple if-expression", async () => {
      const source = `{
  fn test(x: i64) -> i64 {
    result: i64 = if x > 0 { 1; } else { 0; };
    return result;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("icmp ne")      // condition check
      expect(result.llvmIR).toContain("br i1")        // conditional branch
      expect(result.llvmIR).toContain("alloca i64")   // result storage
    })

    it("should generate LLVM IR for if-else if-else expression", async () => {
      const source = `{
  fn classify(x: i64) -> i64 {
    result: i64 = if x > 10 { 3; } else if x > 5 { 2; } else { 1; };
    return result;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
      // Should have multiple conditional branches for the chain
      const branchCount = (result.llvmIR.match(/br i1/g) || []).length
      expect(branchCount).toBeGreaterThanOrEqual(2)
    })
  })

  describe("match expression codegen", () => {
    it("should generate LLVM IR for match with literals", async () => {
      const source = `{
  fn test(x: i64) -> i64 {
    result: i64 = match x {
      1 => 10,
      2 => 20,
      _ => 0
    };
    return result;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("icmp eq")      // pattern comparison
      expect(result.llvmIR).toContain("br i1")        // conditional branch per arm
    })

    it("should generate LLVM IR for match with wildcard only", async () => {
      const source = `{
  fn test(x: i64) -> i64 {
    result: i64 = match x {
      _ => 42
    };
    return result;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
    })

    it("should generate match with multiple literal arms", async () => {
      const source = `{
  fn test(x: i64) -> i64 {
    result: i64 = match x {
      0 => 100,
      1 => 200,
      2 => 300,
      _ => 0
    };
    return result;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
      // Should have icmp eq for each literal pattern (3)
      const eqCount = (result.llvmIR.match(/icmp eq/g) || []).length
      expect(eqCount).toBeGreaterThanOrEqual(3)
    })
  })

  describe("break/continue in for..in range codegen", () => {
    it("should generate break in for-in range", async () => {
      const source = `{
  fn test() -> i64 {
    sum: i64 = 0;
    for i in 0..100 {
      if (i > 5) {
        break;
      }
      sum := sum + i;
    }
    return sum;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
      // break should branch to the end label
      expect(result.llvmIR).toContain("br label")
    })

    it("should generate continue in for-in range", async () => {
      const source = `{
  fn test() -> i64 {
    sum: i64 = 0;
    for i in 0..10 {
      if (i == 3) {
        continue;
      }
      sum := sum + i;
    }
    return sum;
  }
}`
      const result = await fullCompile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("br label")
    })
  })
})

// ============================================================================
// END-TO-END / INTEGRATION TESTS
// ============================================================================

describe("Control Flow - End to End", () => {
  it("should compile for-in range loop end-to-end", async () => {
    const source = `{
  fn sum_range(n: i64) -> i64 {
    total: i64 = 0;
    for i in 0..n {
      total := total + i;
    }
    return total;
  }
  sum_range(5);
}`
    const result = await fullCompile(source)
    expect(result.errors).toEqual([])
    expect(result.llvmIR).toContain("define i64 @sum_range")
    expect(result.llvmIR).toContain("call i64 @sum_range")
  })

  it("should compile for-in index loop end-to-end", async () => {
    const source = `{
  fn sum_indexed(arr: i64[]) -> i64 {
    total: i64 = 0;
    for idx, val in arr {
      total := total + val;
    }
    return total;
  }
}`
    const result = await fullCompile(source)
    expect(result.errors).toEqual([])
    expect(result.llvmIR).toContain("define i64 @sum_indexed")
  })

  it("should compile if-expression end-to-end", async () => {
    const source = `{
  fn abs(x: i64) -> i64 {
    result: i64 = if x > 0 { x; } else { 0 - x; };
    return result;
  }
  abs(5);
}`
    const result = await fullCompile(source)
    expect(result.errors).toEqual([])
    expect(result.llvmIR).toContain("define i64 @abs")
  })

  it("should compile match expression end-to-end", async () => {
    const source = `{
  fn to_string_code(x: i64) -> i64 {
    code: i64 = match x {
      0 => 48,
      1 => 49,
      2 => 50,
      _ => 63
    };
    return code;
  }
  to_string_code(1);
}`
    const result = await fullCompile(source)
    expect(result.errors).toEqual([])
    expect(result.llvmIR).toContain("define i64 @to_string_code")
    expect(result.llvmIR).toContain("call i64 @to_string_code")
  })

  it("should compile nested for-in range with break", async () => {
    const source = `{
  fn find_first_gt(arr: i64[], threshold: i64) -> i64 {
    result: i64 = 0;
    for item: i64 in arr {
      if (item > threshold) {
        result := item;
        break;
      }
    }
    return result;
  }
}`
    const result = await fullCompile(source)
    expect(result.errors).toEqual([])
    expect(result.llvmIR).toContain("define i64 @find_first_gt")
  })
})
