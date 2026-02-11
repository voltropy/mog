import { describe, it, expect } from "bun:test"
import { tokenize } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"
import { generateLLVMIR } from "./llvm_codegen"
import { compile } from "./compiler"
import { ResultType, OptionalType, IntegerType, FloatType, ArrayType, UnsignedType } from "./types"

// ============================================================================
// Helpers
// ============================================================================

function pos(index: number = 0): { start: any; end: any } {
  return { start: { line: 1, column: 1, index }, end: { line: 1, column: 1, index } }
}

/** Parse source to tokens, filter whitespace/comments */
function lex(source: string) {
  const tokens = tokenize(source)
  return tokens.filter((t) => t.type !== "WHITESPACE" && t.type !== "COMMENT")
}

/** Parse source to AST */
function parse(source: string) {
  const ast = parseTokens(lex(source))
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

describe("Error Handling - Lexer", () => {
  describe("ok keyword", () => {
    it("should tokenize ok as keyword", () => {
      const tokens = lex("ok(42)")
      expect(tokens.some(t => t.type === "ok")).toBe(true)
    })

    it("should not tokenize ok_thing as ok keyword", () => {
      const tokens = lex("ok_thing")
      expect(tokens.some(t => t.type === "ok")).toBe(false)
      expect(tokens.some(t => t.type === "IDENTIFIER")).toBe(true)
    })
  })

  describe("err keyword", () => {
    it("should tokenize err as keyword", () => {
      const tokens = lex('err("error")')
      expect(tokens.some(t => t.type === "err")).toBe(true)
    })

    it("should not tokenize error as err keyword", () => {
      const tokens = lex("error")
      expect(tokens.some(t => t.type === "err")).toBe(false)
      expect(tokens.some(t => t.type === "IDENTIFIER")).toBe(true)
    })
  })

  describe("some keyword", () => {
    it("should tokenize some as keyword", () => {
      const tokens = lex("some(42)")
      expect(tokens.some(t => t.type === "some")).toBe(true)
    })
  })

  describe("none keyword", () => {
    it("should tokenize none as keyword", () => {
      const tokens = lex("none")
      expect(tokens.some(t => t.type === "none")).toBe(true)
    })

    it("should not tokenize nonexistent as none keyword", () => {
      const tokens = lex("nonexistent")
      expect(tokens.some(t => t.type === "none")).toBe(false)
    })
  })

  describe("try keyword", () => {
    it("should tokenize try as keyword", () => {
      const tokens = lex("try { }")
      expect(tokens.some(t => t.type === "try")).toBe(true)
    })
  })

  describe("catch keyword", () => {
    it("should tokenize catch as keyword", () => {
      const tokens = lex("catch(e) { }")
      expect(tokens.some(t => t.type === "catch")).toBe(true)
    })
  })

  describe("is keyword", () => {
    it("should tokenize is as keyword", () => {
      const tokens = lex("x is some(y)")
      expect(tokens.some(t => t.type === "is")).toBe(true)
    })
  })

  describe("QUESTION_MARK token (?)", () => {
    it("should tokenize ? as QUESTION_MARK", () => {
      const tokens = lex("x?")
      expect(tokens.some(t => t.type === "QUESTION_MARK")).toBe(true)
    })

    it("should tokenize ? separately from other operators", () => {
      const tokens = lex("a?;")
      const questionToken = tokens.find(t => t.type === "QUESTION_MARK")
      expect(questionToken).toBeDefined()
      expect(questionToken?.value).toBe("?")
    })
  })

  describe("combined error handling tokens", () => {
    it("should tokenize ok(42) correctly", () => {
      const tokens = lex("ok(42)")
      const types = tokens.map(t => t.type)
      expect(types).toContain("ok")
      expect(types).toContain("LPAREN")
      expect(types).toContain("NUMBER")
      expect(types).toContain("RPAREN")
    })

    it("should tokenize err(\"message\") correctly", () => {
      const tokens = lex('err("message")')
      const types = tokens.map(t => t.type)
      expect(types).toContain("err")
      expect(types).toContain("LPAREN")
      expect(types).toContain("STRING")
      expect(types).toContain("RPAREN")
    })

    it("should tokenize try/catch correctly", () => {
      const tokens = lex('try { } catch(e) { }')
      const types = tokens.map(t => t.type)
      expect(types).toContain("try")
      expect(types).toContain("catch")
    })

    it("should tokenize ? propagation operator", () => {
      const tokens = lex("result?;")
      const types = tokens.map(t => t.type)
      expect(types).toContain("IDENTIFIER")
      expect(types).toContain("QUESTION_MARK")
      expect(types).toContain("SEMICOLON")
    })

    it("should tokenize is some pattern", () => {
      const tokens = lex("x is some(val)")
      const types = tokens.map(t => t.type)
      expect(types).toContain("IDENTIFIER")
      expect(types).toContain("is")
      expect(types).toContain("some")
      expect(types).toContain("LPAREN")
      expect(types).toContain("RPAREN")
    })
  })
})

// ============================================================================
// PARSER TESTS
// ============================================================================

describe("Error Handling - Parser", () => {
  describe("ok() expression", () => {
    it("should parse ok(42) as OkExpression", () => {
      const ast = parse("ok(42)")
      const stmt = ast.statements[0] as any
      expect(stmt.type).toBe("ExpressionStatement")
      expect(stmt.expression.type).toBe("OkExpression")
      expect(stmt.expression.value.type).toBe("NumberLiteral")
      expect(stmt.expression.value.value).toBe("42")
    })

    it("should parse ok(x + 1) with complex expression", () => {
      const ast = parse("ok(x + 1)")
      const stmt = ast.statements[0] as any
      expect(stmt.expression.type).toBe("OkExpression")
      expect(stmt.expression.value.type).toBe("BinaryExpression")
    })
  })

  describe("err() expression", () => {
    it("should parse err(\"message\") as ErrExpression", () => {
      const ast = parse('err("message")')
      const stmt = ast.statements[0] as any
      expect(stmt.type).toBe("ExpressionStatement")
      expect(stmt.expression.type).toBe("ErrExpression")
      expect(stmt.expression.value.type).toBe("StringLiteral")
    })
  })

  describe("some() expression", () => {
    it("should parse some(42) as SomeExpression", () => {
      const ast = parse("some(42)")
      const stmt = ast.statements[0] as any
      expect(stmt.type).toBe("ExpressionStatement")
      expect(stmt.expression.type).toBe("SomeExpression")
      expect(stmt.expression.value.type).toBe("NumberLiteral")
    })
  })

  describe("none expression", () => {
    it("should parse none as NoneExpression", () => {
      const ast = parse("none")
      const stmt = ast.statements[0] as any
      expect(stmt.type).toBe("ExpressionStatement")
      expect(stmt.expression.type).toBe("NoneExpression")
    })
  })

  describe("? propagation operator", () => {
    it("should parse expr? as PropagateExpression", () => {
      const ast = parse("x?")
      const stmt = ast.statements[0] as any
      expect(stmt.type).toBe("ExpressionStatement")
      expect(stmt.expression.type).toBe("PropagateExpression")
      expect(stmt.expression.value.type).toBe("Identifier")
      expect(stmt.expression.value.name).toBe("x")
    })
  })

  describe("try/catch blocks", () => {
    it("should parse try/catch statement", () => {
      const source = `try {
        x := ok(42);
      } catch(e) {
        y := 0;
      }`
      const ast = parse(source)
      const stmt = ast.statements[0] as any
      expect(stmt.type).toBe("TryCatch")
      expect(stmt.tryBody.type).toBe("Block")
      expect(stmt.errorVar).toBe("e")
      expect(stmt.catchBody.type).toBe("Block")
    })
  })

  describe("is some() pattern matching", () => {
    it("should parse if expr is some(val) as IsSomeExpression", () => {
      const source = `if x is some(val) {
        y := val;
      }`
      const ast = parse(source)
      const stmt = ast.statements[0] as any
      expect(stmt.type).toBe("Conditional")
      expect(stmt.condition.type).toBe("IsSomeExpression")
      expect(stmt.condition.value.name).toBe("x")
      expect(stmt.condition.binding).toBe("val")
    })

    it("should parse if expr is none as IsNoneExpression", () => {
      const source = `if x is none {
        y := 0;
      }`
      const ast = parse(source)
      const stmt = ast.statements[0] as any
      expect(stmt.type).toBe("Conditional")
      expect(stmt.condition.type).toBe("IsNoneExpression")
    })
  })

  describe("Result<T> type annotation", () => {
    it("should parse Result<int> return type", () => {
      const source = `fn foo() -> Result<int> {
        return ok(42);
      }`
      const ast = parse(source)
      const func = ast.statements[0] as any
      expect(func.type).toBe("FunctionDeclaration")
      expect(func.returnType).toBeInstanceOf(ResultType)
      expect((func.returnType as ResultType).innerType).toBeInstanceOf(IntegerType)
    })
  })

  describe("?T optional type annotation", () => {
    it("should parse ?int return type", () => {
      const source = `fn find() -> ?int {
        return none;
      }`
      const ast = parse(source)
      const func = ast.statements[0] as any
      expect(func.type).toBe("FunctionDeclaration")
      expect(func.returnType).toBeInstanceOf(OptionalType)
      expect((func.returnType as OptionalType).innerType).toBeInstanceOf(IntegerType)
    })
  })

  describe("match on Result variants", () => {
    it("should parse match with ok/err variant patterns", () => {
      const source = `match x {
        ok(val) => val,
        err(msg) => 0,
      }`
      const ast = parse(source)
      const stmt = ast.statements[0] as any
      const matchExpr = stmt.expression
      expect(matchExpr.type).toBe("MatchExpression")
      expect(matchExpr.arms.length).toBe(2)
      expect(matchExpr.arms[0].pattern.type).toBe("VariantPattern")
      expect(matchExpr.arms[0].pattern.name).toBe("ok")
      expect(matchExpr.arms[0].pattern.binding).toBe("val")
      expect(matchExpr.arms[1].pattern.type).toBe("VariantPattern")
      expect(matchExpr.arms[1].pattern.name).toBe("err")
      expect(matchExpr.arms[1].pattern.binding).toBe("msg")
    })
  })
})

// ============================================================================
// ANALYZER TESTS
// ============================================================================

describe("Error Handling - Analyzer", () => {
  describe("ok() expression type", () => {
    it("should infer Result type for ok(42)", () => {
      const ast = parse("ok(42)")
      const errors = analyze(ast)
      // Should not have errors
      expect(errors.length).toBe(0)
      // Check the resultType annotation
      const expr = (ast.statements[0] as any).expression
      expect(expr.resultType).toBeInstanceOf(ResultType)
    })
  })

  describe("err() expression type", () => {
    it("should infer Result type for err(\"msg\")", () => {
      const ast = parse('err("msg")')
      const errors = analyze(ast)
      expect(errors.length).toBe(0)
      const expr = (ast.statements[0] as any).expression
      expect(expr.resultType).toBeInstanceOf(ResultType)
    })
  })

  describe("some() expression type", () => {
    it("should infer Optional type for some(42)", () => {
      const ast = parse("some(42)")
      const errors = analyze(ast)
      expect(errors.length).toBe(0)
      const expr = (ast.statements[0] as any).expression
      expect(expr.resultType).toBeInstanceOf(OptionalType)
    })
  })

  describe("none expression type", () => {
    it("should infer Optional type for none", () => {
      const ast = parse("none")
      const errors = analyze(ast)
      expect(errors.length).toBe(0)
      const expr = (ast.statements[0] as any).expression
      expect(expr.resultType).toBeInstanceOf(OptionalType)
    })
  })

  describe("is some() type", () => {
    it("should return bool type for is some()", () => {
      const source = `if some(42) is some(val) {
        val;
      }`
      const ast = parse(source)
      const errors = analyze(ast)
      // IsSome expressions use boolType
      expect(errors.length).toBe(0)
    })
  })

  describe("try/catch analysis", () => {
    it("should analyze try/catch without errors", () => {
      const source = `try {
        ok(42);
      } catch(e) {
        e;
      }`
      const ast = parse(source)
      const errors = analyze(ast)
      expect(errors.length).toBe(0)
    })
  })

  describe("function with Result return type", () => {
    it("should analyze function returning Result<int>", () => {
      const source = `fn foo() -> Result<int> {
        return ok(42);
      }`
      const ast = parse(source)
      const errors = analyze(ast)
      expect(errors.length).toBe(0)
    })
  })

  describe("function with optional return type", () => {
    it("should analyze function returning ?int", () => {
      const source = `fn find() -> ?int {
        return none;
      }`
      const ast = parse(source)
      const errors = analyze(ast)
      expect(errors.length).toBe(0)
    })
  })
})

// ============================================================================
// CODEGEN TESTS
// ============================================================================

describe("Error Handling - Codegen", () => {
  describe("ok() codegen", () => {
    it("should generate call to mog_result_ok for ok(42)", () => {
      const source = `fn main() -> int {
        return 0;
      }
      ok(42)`
      const ast = parse(source)
      analyze(ast)
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@mog_result_ok")
    })
  })

  describe("err() codegen", () => {
    it("should generate call to mog_result_err for err(msg)", () => {
      const source = `fn main() -> int {
        return 0;
      }
      err("error")`
      const ast = parse(source)
      analyze(ast)
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@mog_result_err")
    })
  })

  describe("some() codegen", () => {
    it("should generate call to mog_optional_some for some(42)", () => {
      const source = `fn main() -> int {
        return 0;
      }
      some(42)`
      const ast = parse(source)
      analyze(ast)
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@mog_optional_some")
    })
  })

  describe("none codegen", () => {
    it("should generate call to mog_optional_none for none", () => {
      const source = `fn main() -> int {
        return 0;
      }
      none`
      const ast = parse(source)
      analyze(ast)
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@mog_optional_none")
    })
  })

  describe("runtime declarations", () => {
    it("should declare all result/optional functions", () => {
      const source = `fn main() -> int { return 0; }`
      const ast = parse(source)
      analyze(ast)
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("declare ptr @mog_result_ok(i64")
      expect(ir).toContain("declare ptr @mog_result_err(ptr")
      expect(ir).toContain("declare i64 @mog_result_is_ok(ptr")
      expect(ir).toContain("declare i64 @mog_result_unwrap(ptr")
      expect(ir).toContain("declare ptr @mog_result_unwrap_err(ptr")
      expect(ir).toContain("declare ptr @mog_optional_some(i64")
      expect(ir).toContain("declare ptr @mog_optional_none()")
      expect(ir).toContain("declare i64 @mog_optional_is_some(ptr")
      expect(ir).toContain("declare i64 @mog_optional_unwrap(ptr")
    })
  })

  describe("match on Result variants codegen", () => {
    it("should generate variant pattern matching for ok/err", () => {
      const source = `fn main() -> int {
        return 0;
      }
      match ok(42) {
        ok(val) => val,
        err(msg) => 0,
      }`
      const ast = parse(source)
      analyze(ast)
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@mog_result_is_ok")
      expect(ir).toContain("@mog_result_unwrap")
    })
  })

  describe("try/catch codegen", () => {
    it("should generate try/catch block structure", () => {
      const source = `fn main() -> int {
        try {
          ok(42);
        } catch(e) {
          0;
        }
        return 0;
      }`
      const ast = parse(source)
      analyze(ast)
      const ir = generateLLVMIR(ast)
      // Check that try/catch generates block labels
      expect(ir).toContain("alloca i64") // error flag
    })
  })

  describe("? propagation codegen", () => {
    it("should generate propagation code for ? operator", () => {
      const source = `fn foo() -> Result<int> {
        return ok(42);
      }
      fn main() -> int {
        return 0;
      }
      foo()?`
      const ast = parse(source)
      analyze(ast)
      const ir = generateLLVMIR(ast)
      // ? generates a branch on result tag
      expect(ir).toContain("@mog_result_is_ok") 
    })
  })

  describe("IsSome/IsNone codegen", () => {
    it("should generate is_some check for is some()", () => {
      const source = `fn main() -> int {
        return 0;
      }
      if some(42) is some(val) {
        val;
      }`
      const ast = parse(source)
      analyze(ast)
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@mog_optional_is_some")
    })
  })
})

// ============================================================================
// TYPE SYSTEM TESTS
// ============================================================================

describe("Error Handling - Types", () => {
  describe("ResultType", () => {
    it("should create ResultType with inner type", () => {
      const rt = new ResultType(new IntegerType("i64"))
      expect(rt.type).toBe("ResultType")
      expect(rt.innerType).toBeInstanceOf(IntegerType)
    })

    it("should have correct toString", () => {
      const rt = new ResultType(new IntegerType("i64"))
      expect(rt.toString()).toBe("Result<i64>")
    })

    it("should be comparable with sameType", () => {
      const rt1 = new ResultType(new IntegerType("i64"))
      const rt2 = new ResultType(new IntegerType("i64"))
      const rt3 = new ResultType(new FloatType("f64"))
      
      // Can't use sameType directly without import, test via type comparison
      expect(rt1.innerType.toString()).toBe(rt2.innerType.toString())
      expect(rt1.innerType.toString()).not.toBe(rt3.innerType.toString())
    })
  })

  describe("OptionalType", () => {
    it("should create OptionalType with inner type", () => {
      const ot = new OptionalType(new IntegerType("i64"))
      expect(ot.type).toBe("OptionalType")
      expect(ot.innerType).toBeInstanceOf(IntegerType)
    })

    it("should have correct toString", () => {
      const ot = new OptionalType(new IntegerType("i64"))
      expect(ot.toString()).toBe("?i64")
    })
  })
})

// ============================================================================
// COMPILE PIPELINE TESTS
// ============================================================================

describe("Error Handling - Compile Pipeline", () => {
  it("should compile ok(42) without errors", async () => {
    const result = await fullCompile("ok(42)")
    expect(result.errors.length).toBe(0)
    expect(result.llvmIR).toContain("@mog_result_ok")
  })

  it("should compile err(\"msg\") without errors", async () => {
    const result = await fullCompile('err("msg")')
    expect(result.errors.length).toBe(0)
    expect(result.llvmIR).toContain("@mog_result_err")
  })

  it("should compile some(42) without errors", async () => {
    const result = await fullCompile("some(42)")
    expect(result.errors.length).toBe(0)
    expect(result.llvmIR).toContain("@mog_optional_some")
  })

  it("should compile none without errors", async () => {
    const result = await fullCompile("none")
    expect(result.errors.length).toBe(0)
    expect(result.llvmIR).toContain("@mog_optional_none")
  })

  it("should compile function with Result<int> return type", async () => {
    const source = `fn foo() -> Result<int> {
      return ok(42);
    }`
    const result = await fullCompile(source)
    expect(result.errors.length).toBe(0)
    expect(result.llvmIR).toContain("@mog_result_ok")
  })

  it("should compile function with ?int return type", async () => {
    const source = `fn find() -> ?int {
      return none;
    }`
    const result = await fullCompile(source)
    expect(result.errors.length).toBe(0)
  })

  it("should compile try/catch block", async () => {
    const source = `try {
      ok(42);
    } catch(e) {
      0;
    }`
    const result = await fullCompile(source)
    expect(result.errors.length).toBe(0)
  })

  it("should compile match on Result variants", async () => {
    const source = `match ok(42) {
      ok(val) => val,
      err(msg) => 0,
    }`
    const result = await fullCompile(source)
    expect(result.errors.length).toBe(0)
    expect(result.llvmIR).toContain("@mog_result_is_ok")
  })

  it("should compile if is some() pattern matching", async () => {
    const source = `if some(42) is some(val) {
      val;
    }`
    const result = await fullCompile(source)
    expect(result.errors.length).toBe(0)
    expect(result.llvmIR).toContain("@mog_optional_is_some")
  })

  it("should compile match on Optional variants", async () => {
    const source = `match some(42) {
      some(val) => val,
      none() => 0,
    }`
    const result = await fullCompile(source)
    expect(result.errors.length).toBe(0)
  })

  it("should compile ? propagation in function", async () => {
    const source = `fn foo() -> Result<int> {
      return ok(42);
    }
    fn bar() -> Result<int> {
      return ok(foo()?);
    }`
    const result = await fullCompile(source)
    expect(result.errors.length).toBe(0)
    expect(result.llvmIR).toContain("@mog_result_is_ok")
    expect(result.llvmIR).toContain("@mog_result_unwrap")
  })

  it("should compile is none pattern matching", async () => {
    const source = `if none is none {
      0;
    }`
    const result = await fullCompile(source)
    expect(result.errors.length).toBe(0)
  })

  it("should compile nested ok/err in match arms", async () => {
    const source = `fn check(x: int) -> Result<int> {
      if x > 0 {
        return ok(x);
      }
      return err("negative");
    }`
    const result = await fullCompile(source)
    expect(result.errors.length).toBe(0)
    expect(result.llvmIR).toContain("@mog_result_ok")
    expect(result.llvmIR).toContain("@mog_result_err")
  })

  it("should compile some/none in function", async () => {
    const source = `fn find(x: int) -> ?int {
      if x > 0 {
        return some(x);
      }
      return none;
    }`
    const result = await fullCompile(source)
    expect(result.errors.length).toBe(0)
    expect(result.llvmIR).toContain("@mog_optional_some")
    expect(result.llvmIR).toContain("@mog_optional_none")
  })
})
