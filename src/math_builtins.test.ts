import { describe, test, expect } from "bun:test"
import { SemanticAnalyzer } from "./analyzer"
import { generateLLVMIR } from "./llvm_codegen"
import { FloatType, IntegerType } from "./types"

// --- AST helpers ---

function pos() {
  return { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 1, index: 0 } }
}

function program(...statements: any[]): any {
  return { type: "Program", statements, position: pos() }
}

function ident(name: string): any {
  return { type: "Identifier", name, position: pos() }
}

function num(value: any, type: any = null): any {
  return { type: "NumberLiteral", value: String(value), literalType: type, position: pos() }
}

function call(callee: any, ...args: any[]): any {
  return { type: "CallExpression", callee, args, position: pos() }
}

function varDecl(name: string, type: any, value: any): any {
  return { type: "VariableDeclaration", name, varType: type, value, position: pos() }
}

function exprStmt(expr: any): any {
  return { type: "ExpressionStatement", expression: expr, position: pos() }
}

const f64 = new FloatType("f64")
const i64 = new IntegerType("i64")

// --- Analyzer Tests ---

describe("Math Builtins - Analyzer", () => {
  describe("math functions are recognized as builtins", () => {
    const singleArgFunctions = ["abs", "sqrt", "sin", "cos", "tan", "asin", "acos",
      "exp", "log", "log2", "floor", "ceil", "round"]

    for (const funcName of singleArgFunctions) {
      test(`${funcName}() is a known builtin`, () => {
        const ast = program(
          varDecl("result", f64, call(ident(funcName), num("1.0", f64)))
        )
        const analyzer = new SemanticAnalyzer()
        const errors = analyzer.analyze(ast)
        // Should not have "Undefined variable" error for the function name
        const undefinedErrors = errors.filter(e => e.message.includes(`Undefined variable '${funcName}'`))
        expect(undefinedErrors).toEqual([])
      })
    }

    const twoArgFunctions = ["pow", "atan2", "min", "max"]

    for (const funcName of twoArgFunctions) {
      test(`${funcName}() is a known builtin`, () => {
        const ast = program(
          varDecl("result", f64, call(ident(funcName), num("1.0", f64), num("2.0", f64)))
        )
        const analyzer = new SemanticAnalyzer()
        const errors = analyzer.analyze(ast)
        const undefinedErrors = errors.filter(e => e.message.includes(`Undefined variable '${funcName}'`))
        expect(undefinedErrors).toEqual([])
      })
    }
  })

  describe("PI and E constants are recognized", () => {
    test("PI is a known constant", () => {
      const ast = program(
        varDecl("x", f64, ident("PI"))
      )
      const analyzer = new SemanticAnalyzer()
      const errors = analyzer.analyze(ast)
      const undefinedErrors = errors.filter(e => e.message.includes("Undefined variable 'PI'"))
      expect(undefinedErrors).toEqual([])
    })

    test("E is a known constant", () => {
      const ast = program(
        varDecl("x", f64, ident("E"))
      )
      const analyzer = new SemanticAnalyzer()
      const errors = analyzer.analyze(ast)
      const undefinedErrors = errors.filter(e => e.message.includes("Undefined variable 'E'"))
      expect(undefinedErrors).toEqual([])
    })
  })

  describe("math functions return f64", () => {
    test("sqrt returns f64 type", () => {
      const ast = program(
        varDecl("result", f64, call(ident("sqrt"), num("4.0", f64)))
      )
      const analyzer = new SemanticAnalyzer()
      const errors = analyzer.analyze(ast)
      expect(errors).toEqual([])
    })

    test("pow returns f64 type", () => {
      const ast = program(
        varDecl("result", f64, call(ident("pow"), num("2.0", f64), num("3.0", f64)))
      )
      const analyzer = new SemanticAnalyzer()
      const errors = analyzer.analyze(ast)
      expect(errors).toEqual([])
    })
  })
})

// --- LLVM Codegen Tests ---

describe("Math Builtins - LLVM Codegen", () => {
  describe("math function declarations are emitted", () => {
    test("generates all math declare statements", () => {
      const ast = { type: "Program", statements: [] }
      const ir = generateLLVMIR(ast)
      // LLVM intrinsics
      expect(ir).toContain("declare double @llvm.sqrt.f64(double)")
      expect(ir).toContain("declare double @llvm.sin.f64(double)")
      expect(ir).toContain("declare double @llvm.cos.f64(double)")
      expect(ir).toContain("declare double @llvm.exp.f64(double)")
      expect(ir).toContain("declare double @llvm.log.f64(double)")
      expect(ir).toContain("declare double @llvm.log2.f64(double)")
      expect(ir).toContain("declare double @llvm.floor.f64(double)")
      expect(ir).toContain("declare double @llvm.ceil.f64(double)")
      expect(ir).toContain("declare double @llvm.round.f64(double)")
      expect(ir).toContain("declare double @llvm.fabs.f64(double)")
      expect(ir).toContain("declare double @llvm.pow.f64(double, double)")
      expect(ir).toContain("declare double @llvm.minnum.f64(double, double)")
      expect(ir).toContain("declare double @llvm.maxnum.f64(double, double)")
      // libm functions
      expect(ir).toContain("declare double @tan(double)")
      expect(ir).toContain("declare double @asin(double)")
      expect(ir).toContain("declare double @acos(double)")
      expect(ir).toContain("declare double @atan2(double, double)")
    })
  })

  describe("single-arg math calls generate correct IR", () => {
    const singleArgTests: [string, string][] = [
      ["sqrt", "@llvm.sqrt.f64"],
      ["sin", "@llvm.sin.f64"],
      ["cos", "@llvm.cos.f64"],
      ["tan", "@tan"],
      ["asin", "@asin"],
      ["acos", "@acos"],
      ["exp", "@llvm.exp.f64"],
      ["log", "@llvm.log.f64"],
      ["log2", "@llvm.log2.f64"],
      ["floor", "@llvm.floor.f64"],
      ["ceil", "@llvm.ceil.f64"],
      ["round", "@llvm.round.f64"],
      ["abs", "@llvm.fabs.f64"],
    ]

    for (const [funcName, llvmName] of singleArgTests) {
      test(`${funcName}(x) calls ${llvmName}`, () => {
        const ast = {
          type: "Program",
          statements: [
            {
              type: "VariableDeclaration",
              name: "x",
              varType: f64,
              value: { type: "NumberLiteral", value: "1.0", literalType: f64 },
            },
            {
              type: "VariableDeclaration",
              name: "result",
              varType: f64,
              value: {
                type: "CallExpression",
                callee: { type: "Identifier", name: funcName },
                args: [{ type: "Identifier", name: "x" }],
              },
            },
          ],
        }
        const ir = generateLLVMIR(ast)
        // Should have bitcast i64 -> double, call, bitcast double -> i64
        expect(ir).toContain(`bitcast i64`)
        expect(ir).toContain(`to double`)
        expect(ir).toContain(`call double ${llvmName}(double`)
        expect(ir).toContain(`bitcast double`)
        expect(ir).toContain(`to i64`)
      })
    }
  })

  describe("two-arg math calls generate correct IR", () => {
    const twoArgTests: [string, string][] = [
      ["pow", "@llvm.pow.f64"],
      ["atan2", "@atan2"],
      ["min", "@llvm.minnum.f64"],
      ["max", "@llvm.maxnum.f64"],
    ]

    for (const [funcName, llvmName] of twoArgTests) {
      test(`${funcName}(a, b) calls ${llvmName}`, () => {
        const ast = {
          type: "Program",
          statements: [
            {
              type: "VariableDeclaration",
              name: "a",
              varType: f64,
              value: { type: "NumberLiteral", value: "2.0", literalType: f64 },
            },
            {
              type: "VariableDeclaration",
              name: "b",
              varType: f64,
              value: { type: "NumberLiteral", value: "3.0", literalType: f64 },
            },
            {
              type: "VariableDeclaration",
              name: "result",
              varType: f64,
              value: {
                type: "CallExpression",
                callee: { type: "Identifier", name: funcName },
                args: [
                  { type: "Identifier", name: "a" },
                  { type: "Identifier", name: "b" },
                ],
              },
            },
          ],
        }
        const ir = generateLLVMIR(ast)
        expect(ir).toContain(`call double ${llvmName}(double`)
      })
    }
  })

  describe("PI and E constants", () => {
    test("PI generates correct f64 bit pattern", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "pi_val",
            varType: f64,
            value: { type: "Identifier", name: "PI" },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      // PI = 3.141592653589793 -> 0x400921fb54442d18
      expect(ir).toContain("0x400921fb54442d18")
    })

    test("E generates correct f64 bit pattern", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "e_val",
            varType: f64,
            value: { type: "Identifier", name: "E" },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      // E = 2.718281828459045 -> 0x4005bf0a8b145769
      expect(ir).toContain("0x4005bf0a8b145769")
    })

    test("PI can be used as argument to math function", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "result",
            varType: f64,
            value: {
              type: "CallExpression",
              callee: { type: "Identifier", name: "sin" },
              args: [{ type: "Identifier", name: "PI" }],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("call double @llvm.sin.f64(double")
      expect(ir).toContain("0x400921fb54442d18")
    })
  })

  describe("math section header in IR", () => {
    test("IR contains math declarations section", () => {
      const ast = { type: "Program", statements: [] }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("; Math function declarations (libm)")
    })
  })
})
