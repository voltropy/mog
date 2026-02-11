import { describe, test, expect } from "bun:test"
import { Lexer } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"
import { TensorType, FloatType, IntegerType, isTensorType, sameType } from "./types"

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

// ============================================================
// PHASE 10: TENSOR SYSTEM STUBS
// ============================================================

describe("Tensor System", () => {

  // ----------------------------------------------------------
  // 1. Lexer: tensor keyword
  // ----------------------------------------------------------
  describe("Lexer", () => {
    test("tokenizes tensor as keyword", () => {
      const tokens = lex("tensor")
      expect(tokens).toHaveLength(1)
      expect(tokens[0].type).toBe("tensor")
      expect(tokens[0].value).toBe("tensor")
    })

    test("tensor keyword before angle bracket", () => {
      const tokens = lex("tensor<f32>")
      expect(tokens[0].type).toBe("tensor")
      expect(tokens[1].type).toBe("LESS")
      expect(tokens[2].type).toBe("TYPE")
      expect(tokens[2].value).toBe("f32")
      expect(tokens[3].type).toBe("GREATER")
    })

    test("tensor does not match tensor_data identifier", () => {
      const tokens = lex("tensor_data")
      expect(tokens).toHaveLength(1)
      expect(tokens[0].type).toBe("IDENTIFIER")
      expect(tokens[0].value).toBe("tensor_data")
    })

    test("tensor in full declaration context", () => {
      const tokens = lex("t: tensor<f32> = tensor<f32>([2, 3]);")
      const tensorTokens = tokens.filter(t => t.type === "tensor")
      expect(tensorTokens).toHaveLength(2)
    })
  })

  // ----------------------------------------------------------
  // 2. Type system: TensorType
  // ----------------------------------------------------------
  describe("TensorType", () => {
    test("creates tensor type with dtype", () => {
      const t = new TensorType(new FloatType("f32"))
      expect(t.type).toBe("TensorType")
      expect(t.dtype).toBeInstanceOf(FloatType)
      expect((t.dtype as FloatType).kind).toBe("f32")
      expect(t.shape).toBeNull()
    })

    test("creates tensor type with shape", () => {
      const t = new TensorType(new FloatType("f64"), [2, 3])
      expect(t.shape).toEqual([2, 3])
      expect(t.toString()).toBe("tensor<f64>(2, 3)")
    })

    test("toString without shape", () => {
      const t = new TensorType(new FloatType("f32"))
      expect(t.toString()).toBe("tensor<f32>")
    })

    test("isTensorType guard", () => {
      const t = new TensorType(new FloatType("f32"))
      expect(isTensorType(t)).toBe(true)
      expect(isTensorType(new FloatType("f32"))).toBe(false)
    })

    test("sameType for tensor types", () => {
      const a = new TensorType(new FloatType("f32"))
      const b = new TensorType(new FloatType("f32"))
      const c = new TensorType(new FloatType("f64"))
      expect(sameType(a, b)).toBe(true)
      expect(sameType(a, c)).toBe(false)
    })

    test("sameType with shapes", () => {
      const a = new TensorType(new FloatType("f32"), [2, 3])
      const b = new TensorType(new FloatType("f32"), [2, 3])
      const c = new TensorType(new FloatType("f32"), [3, 2])
      expect(sameType(a, b)).toBe(true)
      expect(sameType(a, c)).toBe(false)
    })

    test("sameType null vs non-null shape", () => {
      const a = new TensorType(new FloatType("f32"))
      const b = new TensorType(new FloatType("f32"), [2, 3])
      expect(sameType(a, b)).toBe(false)
    })
  })

  // ----------------------------------------------------------
  // 3. Parser: tensor type annotations
  // ----------------------------------------------------------
  describe("Parser - Type Annotations", () => {
    test("parses tensor<f32> type annotation in variable declaration", () => {
      const ast = parse("t: tensor<f32> = tensor<f32>([2, 3]);")
      expect(ast.statements).toHaveLength(1)
      const decl = ast.statements[0] as any
      expect(decl.type).toBe("VariableDeclaration")
      expect(decl.varType).toBeInstanceOf(TensorType)
      expect((decl.varType as TensorType).dtype).toBeInstanceOf(FloatType)
      expect(((decl.varType as TensorType).dtype as FloatType).kind).toBe("f32")
    })

    test("parses tensor<f64> type annotation", () => {
      const ast = parse("x: tensor<f64> = tensor<f64>([4, 4]);")
      const decl = ast.statements[0] as any
      expect(decl.varType).toBeInstanceOf(TensorType)
      expect(((decl.varType as TensorType).dtype as FloatType).kind).toBe("f64")
    })

    test("parses tensor<i32> type annotation", () => {
      const ast = parse("x: tensor<i32> = tensor<i32>([3]);")
      const decl = ast.statements[0] as any
      expect(decl.varType).toBeInstanceOf(TensorType)
      expect((decl.varType as TensorType).dtype).toBeInstanceOf(IntegerType)
    })
  })

  // ----------------------------------------------------------
  // 4. Parser: tensor construction expressions
  // ----------------------------------------------------------
  describe("Parser - Construction Expressions", () => {
    test("parses tensor<f32>([2, 3]) construction", () => {
      const ast = parse("tensor<f32>([2, 3]);")
      const stmt = ast.statements[0] as any
      expect(stmt.type).toBe("ExpressionStatement")
      const expr = stmt.expression
      expect(expr.type).toBe("TensorConstruction")
      expect(expr.dtype).toBeInstanceOf(FloatType)
      expect((expr.dtype as FloatType).kind).toBe("f32")
      expect(expr.args).toHaveLength(1)
    })

    test("parses tensor construction with assignment", () => {
      const ast = parse("t: tensor<f32> = tensor<f32>([1]);")
      const decl = ast.statements[0] as any
      expect(decl.type).toBe("VariableDeclaration")
      const init = decl.value || decl.initializer
      expect(init.type).toBe("TensorConstruction")
    })

    test("parses tensor with method chain", () => {
      const ast = parse("tensor<f32>([2, 3]).transpose();")
      const stmt = ast.statements[0] as any
      const expr = stmt.expression
      expect(expr.type).toBe("CallExpression")
      expect(expr.callee.type).toBe("MemberExpression")
      expect(expr.callee.property).toBe("transpose")
    })

    test("parses tensor with matmul method", () => {
      const ast = parse("tensor<f32>([2, 3]).matmul(tensor<f32>([3, 2]));")
      const stmt = ast.statements[0] as any
      const expr = stmt.expression
      expect(expr.type).toBe("CallExpression")
      expect(expr.callee.property).toBe("matmul")
      // The argument should be another tensor construction
      expect(expr.args).toHaveLength(1)
    })

    test("parses tensor with .norm() method", () => {
      const ast = parse("tensor<f64>([4]).norm();")
      const stmt = ast.statements[0] as any
      const expr = stmt.expression
      expect(expr.type).toBe("CallExpression")
      expect(expr.callee.property).toBe("norm")
    })
  })

  // ----------------------------------------------------------
  // 5. Analyzer: tensor type checking
  // ----------------------------------------------------------
  describe("Analyzer - Type Checking", () => {
    test("tensor construction returns TensorType", () => {
      const { errors } = analyzeSource("t: tensor<f32> = tensor<f32>([2, 3]);")
      expect(errors).toHaveLength(0)
    })

    test("tensor variable is declared with correct type", () => {
      const { errors } = analyzeSource(`
        fn main() -> i64 {
          t: tensor<f32> = tensor<f32>([2, 3]);
          return 0;
        }
      `)
      expect(errors).toHaveLength(0)
    })

    test("tensor method .shape returns array type", () => {
      const { errors } = analyzeSource(`
        fn main() -> i64 {
          t: tensor<f32> = tensor<f32>([2, 3]);
          s := t.shape;
          return 0;
        }
      `)
      expect(errors).toHaveLength(0)
    })

    test("tensor method .norm() returns float", () => {
      const { errors } = analyzeSource(`
        fn main() -> i64 {
          t: tensor<f32> = tensor<f32>([2, 3]);
          n := t.norm();
          return 0;
        }
      `)
      expect(errors).toHaveLength(0)
    })

    test("tensor method .matmul() returns tensor", () => {
      const { errors } = analyzeSource(`
        fn main() -> i64 {
          a: tensor<f32> = tensor<f32>([2, 3]);
          b: tensor<f32> = tensor<f32>([3, 2]);
          c := a.matmul(b);
          return 0;
        }
      `)
      expect(errors).toHaveLength(0)
    })

    test("tensor method .transpose() returns tensor", () => {
      const { errors } = analyzeSource(`
        fn main() -> i64 {
          t: tensor<f32> = tensor<f32>([2, 3]);
          t2 := t.transpose();
          return 0;
        }
      `)
      expect(errors).toHaveLength(0)
    })

    test("tensor method .reshape() returns tensor", () => {
      const { errors } = analyzeSource(`
        fn main() -> i64 {
          t: tensor<f32> = tensor<f32>([2, 3]);
          t2 := t.reshape([6]);
          return 0;
        }
      `)
      expect(errors).toHaveLength(0)
    })

    test("tensor .sum() returns scalar float", () => {
      const { errors } = analyzeSource(`
        fn main() -> i64 {
          t: tensor<f64> = tensor<f64>([4]);
          s := t.sum();
          return 0;
        }
      `)
      expect(errors).toHaveLength(0)
    })

    test("tensor .mean() returns scalar float", () => {
      const { errors } = analyzeSource(`
        fn main() -> i64 {
          t: tensor<f32> = tensor<f32>([4]);
          m := t.mean();
          return 0;
        }
      `)
      expect(errors).toHaveLength(0)
    })

    test("tensor .argmax() returns integer", () => {
      const { errors } = analyzeSource(`
        fn main() -> i64 {
          t: tensor<f32> = tensor<f32>([4]);
          idx := t.argmax();
          return 0;
        }
      `)
      expect(errors).toHaveLength(0)
    })
  })
})
