import { describe, test, expect } from "bun:test"
import { generateLLVMIR } from "./llvm_codegen"
import { i64, f64, voidType, array, table } from "./types"

describe("LLVM IR Generator", () => {
  describe("Program Structure", () => {
    test("generates proper LLVM IR header", () => {
      const ast = {
        type: "Program",
        statements: [],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain('target triple = "x86_64-unknown-linux-gnu"')
    })

    test("generates external function declarations", () => {
      const ast = {
        type: "Program",
        statements: [],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("declare ptr @llm_call")
      expect(ir).toContain("declare void @gc_init")
      expect(ir).toContain("declare ptr @array_alloc")
      expect(ir).toContain("declare ptr @table_new")
    })
  })

  describe("Variable Declarations", () => {
    test("generates alloca and store for integer variable", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "x",
            varType: i64,
            value: {
              type: "NumberLiteral",
              value: "42",
              literalType: i64,
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("%x = alloca i64")
      expect(ir).toContain("store i64 42, ptr %x")
    })

    test("generates uninitialized variable", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "y",
            varType: i64,
            value: null,
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("%y = alloca i64")
    })
  })

  describe("Assignments", () => {
    test("generates store for integer assignment", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "x",
            varType: i64,
            value: { type: "NumberLiteral", value: "0", literalType: i64 },
          },
          {
            type: "Assignment",
            name: "x",
            value: {
              type: "NumberLiteral",
              value: "100",
              literalType: i64,
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("%x = alloca i64")
      expect(ir).toContain("store i64 0, ptr %x")
    })
  })

  describe("Binary Expressions", () => {
    test("generates add operation", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "BinaryExpression",
              operator: "+",
              left: { type: "NumberLiteral", value: "5", literalType: i64 },
              right: { type: "NumberLiteral", value: "3", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("add i64")
    })

    test("generates subtract operation", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "BinaryExpression",
              operator: "-",
              left: { type: "NumberLiteral", value: "10", literalType: i64 },
              right: { type: "NumberLiteral", value: "4", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("sub i64")
    })

    test("generates multiply operation", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "BinaryExpression",
              operator: "*",
              left: { type: "NumberLiteral", value: "3", literalType: i64 },
              right: { type: "NumberLiteral", value: "7", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("mul i64")
    })

    test("generates divide operation", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "BinaryExpression",
              operator: "/",
              left: { type: "NumberLiteral", value: "20", literalType: i64 },
              right: { type: "NumberLiteral", value: "4", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("sdiv i64")
    })

    test("generates modulo operation", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "BinaryExpression",
              operator: "%",
              left: { type: "NumberLiteral", value: "17", literalType: i64 },
              right: { type: "NumberLiteral", value: "5", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("srem i64")
    })

    test("generates less than comparison", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "BinaryExpression",
              operator: "<",
              left: { type: "NumberLiteral", value: "5", literalType: i64 },
              right: { type: "NumberLiteral", value: "10", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("icmp slt i64")
    })

    test("generates greater than comparison", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "BinaryExpression",
              operator: ">",
              left: { type: "NumberLiteral", value: "10", literalType: i64 },
              right: { type: "NumberLiteral", value: "5", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("icmp sgt i64")
    })

    test("generates equality comparison", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "BinaryExpression",
              operator: "==",
              left: { type: "NumberLiteral", value: "5", literalType: i64 },
              right: { type: "NumberLiteral", value: "5", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("icmp eq i64")
    })

    test("generates inequality comparison", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "BinaryExpression",
              operator: "!=",
              left: { type: "NumberLiteral", value: "5", literalType: i64 },
              right: { type: "NumberLiteral", value: "10", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("icmp ne i64")
    })
  })

  describe("Unary Expressions", () => {
    test("generates negation operation", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "UnaryExpression",
              operator: "-",
              operand: { type: "NumberLiteral", value: "5", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("sub i64 0")
    })

    test("generates logical NOT", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "UnaryExpression",
              operator: "!",
              operand: { type: "NumberLiteral", value: "1", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("xor i64")
    })
  })

  describe("Function Calls", () => {
    test("generates function call without arguments", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "CallExpression",
              callee: { type: "Identifier", name: "foo" },
              arguments: [],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("call i64 @foo()")
    })

    test("generates function call with integer arguments", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "CallExpression",
              callee: { type: "Identifier", name: "bar" },
              arguments: [
                { type: "NumberLiteral", value: "1", literalType: i64 },
                { type: "NumberLiteral", value: "2", literalType: i64 },
              ],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("call i64 @bar(1, 2)")
    })

    test("generates function call with variable arguments", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "x",
            varType: i64,
            value: { type: "NumberLiteral", value: "5", literalType: i64 },
          },
          {
            type: "ExpressionStatement",
            expression: {
              type: "CallExpression",
              callee: { type: "Identifier", name: "baz" },
              arguments: [{ type: "Identifier", name: "x" }],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("%x = alloca i64")
      expect(ir).toContain("call i64 @baz")
    })
  })

  describe("Array Literals", () => {
    test("generates array literal with integers", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "arr",
            varType: array(i64, 1),
            value: {
              type: "ArrayLiteral",
              elements: [
                { type: "NumberLiteral", value: "1", literalType: i64 },
                { type: "NumberLiteral", value: "2", literalType: i64 },
                { type: "NumberLiteral", value: "3", literalType: i64 },
              ],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("%arr = alloca i64")
      expect(ir).toContain("call ptr @array_alloc(i64 %size, i64 %dim_count, i64 %dimensions)")
      expect(ir).toContain("call void @array_set(ptr %0, i64 0, i64 1)")
      expect(ir).toContain("call void @array_set(ptr %0, i64 1, i64 2)")
      expect(ir).toContain("call void @array_set(ptr %0, i64 2, i64 3)")
    })

    test("generates empty array literal", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "empty",
            varType: array(i64, 1),
            value: {
              type: "ArrayLiteral",
              elements: [],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("%empty = alloca i64")
      expect(ir).toContain("call ptr @array_alloc(i64 %size, i64 %dim_count, i64 %dimensions)")
    })
  })

  describe("Conditionals", () => {
    test("generates if statement without else", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "x",
            varType: i64,
            value: { type: "NumberLiteral", value: "10", literalType: i64 },
          },
          {
            type: "Conditional",
            condition: {
              type: "BinaryExpression",
              operator: ">",
              left: { type: "Identifier", name: "x" },
              right: { type: "NumberLiteral", value: "5", literalType: i64 },
            },
            consequent: {
              type: "Block",
              statements: [
                {
                  type: "Assignment",
                  name: "x",
                  value: { type: "NumberLiteral", value: "0", literalType: i64 },
                },
              ],
            },
            alternate: null,
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("br i1")
      expect(ir).toContain("label0:")
      expect(ir).toContain("label1:")
    })

    test("generates if-else statement", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "Conditional",
            condition: {
              type: "BinaryExpression",
              operator: ">",
              left: { type: "NumberLiteral", value: "10", literalType: i64 },
              right: { type: "NumberLiteral", value: "5", literalType: i64 },
            },
            consequent: {
              type: "Block",
              statements: [],
            },
            alternate: {
              type: "Block",
              statements: [],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("br i1")
      expect(ir).toMatch(/label[0-9]+:/)
    })
  })

  describe("While Loops", () => {
    test("generates while loop structure", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "i",
            varType: i64,
            value: { type: "NumberLiteral", value: "0", literalType: i64 },
          },
          {
            type: "WhileLoop",
            condition: {
              type: "BinaryExpression",
              operator: "<",
              left: { type: "Identifier", name: "i" },
              right: { type: "NumberLiteral", value: "10", literalType: i64 },
            },
            body: {
              type: "Block",
              statements: [
                {
                  type: "Assignment",
                  name: "i",
                  value: {
                    type: "BinaryExpression",
                    operator: "+",
                    left: { type: "Identifier", name: "i" },
                    right: { type: "NumberLiteral", value: "1", literalType: i64 },
                  },
                },
              ],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("br label")
      expect(ir).toContain("label0:")
      expect(ir).toContain("label1:")
      expect(ir).toContain("label2:")
    })
  })

  describe("For Loops", () => {
    test("generates for loop structure", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ForLoop",
            variable: "i",
            start: { type: "NumberLiteral", value: "0", literalType: i64 },
            end: { type: "NumberLiteral", value: "10", literalType: i64 },
            step: { type: "NumberLiteral", value: "1", literalType: i64 },
            body: {
              type: "Block",
              statements: [],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("%i = alloca i64")
      expect(ir).toContain("store i64 0, ptr %i")
      expect(ir).toContain("br label")
      let labelCount = (ir.match(/label[0-9]+:/g) || []).length
      expect(labelCount).toBeGreaterThanOrEqual(4)
    })
  })

  describe("Function Declarations", () => {
    test("generates function with parameters", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "FunctionDeclaration",
            name: "add",
            params: [
              { name: "a", paramType: i64 },
              { name: "b", paramType: i64 },
            ],
            returnType: i64,
            body: {
              type: "Block",
              statements: [
                {
                  type: "Return",
                  value: {
                    type: "BinaryExpression",
                    operator: "+",
                    left: { type: "Identifier", name: "a" },
                    right: { type: "Identifier", name: "b" },
                  },
                },
              ],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("define i64 @add(i64 %a, i64 %b)")
      expect(ir).toContain("add i64")
      expect(ir).toContain("ret i64")
    })

    test("generates void function with default i64 return", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "FunctionDeclaration",
            name: "greet",
            params: [],
            returnType: voidType,
            body: {
              type: "Block",
              statements: [
                {
                  type: "ExpressionStatement",
                  expression: {
                    type: "CallExpression",
                    callee: { type: "Identifier", name: "printHello" },
                    arguments: [],
                  },
                },
              ],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("define i64 @greet()")
      expect(ir).toContain("ret i64 0")
    })

    test("generates function without parameters", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "FunctionDeclaration",
            name: "main",
            params: [],
            returnType: i64,
            body: {
              type: "Block",
              statements: [
                {
                  type: "Return",
                  value: { type: "NumberLiteral", value: "0", literalType: i64 },
                },
              ],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("define i64 @main()")
    })
  })

  describe("Return Statements", () => {
    test("generates return with integer value", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "FunctionDeclaration",
            name: "getOne",
            params: [],
            returnType: i64,
            body: {
              type: "Block",
              statements: [
                {
                  type: "Return",
                  value: { type: "NumberLiteral", value: "1", literalType: i64 },
                },
              ],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("define i64 @getOne()")
      expect(ir).toContain("ret i64 1")
    })

    test("generates return with expression", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "FunctionDeclaration",
            name: "square",
            params: [{ name: "x", paramType: i64 }],
            returnType: i64,
            body: {
              type: "Block",
              statements: [
                {
                  type: "Return",
                  value: {
                    type: "BinaryExpression",
                    operator: "*",
                    left: { type: "Identifier", name: "x" },
                    right: { type: "Identifier", name: "x" },
                  },
                },
              ],
            },
          },
],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("define i64 @square(i64 %x)")
      expect(ir).toMatch(/%x_local = alloca i64/)
      expect(ir).toContain("mul i64")
      expect(ir).toMatch(/ret i64/)
    })
  })

  describe("Array Indexing", () => {
    test("generates array index access", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "arr",
            varType: array(i64, 1),
            value: {
              type: "ArrayLiteral",
              elements: [{ type: "NumberLiteral", value: "10", literalType: i64 }],
            },
          },
          {
            type: "ExpressionStatement",
            expression: {
              type: "IndexExpression",
              object: { type: "Identifier", name: "arr" },
              index: { type: "NumberLiteral", value: "1", literalType: i64 },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("array_get")
    })
  })

  describe("Table Member Expressions", () => {
    test("generates table member access", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "MemberExpression",
              object: { type: "Identifier", name: "obj" },
              property: { type: "Identifier", name: "x" },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("table_get")
    })

    test("generates table member assignment", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "VariableDeclaration",
            name: "obj",
            varType: table(String, i64),
            value: null,
          },
          {
            type: "Assignment",
            name: "obj",
            value: {
              type: "MemberExpression",
              object: { type: "Identifier", name: "obj" },
              property: { type: "Identifier", name: "x" },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("table_set")
    })
  })

  describe("Complex Expressions", () => {
    test("handles nested binary expressions", () => {
      const ast = {
        type: "Program",
        statements: [
          {
            type: "ExpressionStatement",
            expression: {
              type: "BinaryExpression",
              operator: "+",
              left: {
                type: "BinaryExpression",
                operator: "*",
                left: { type: "NumberLiteral", value: "2", literalType: i64 },
                right: { type: "NumberLiteral", value: "3", literalType: i64 },
              },
              right: {
                type: "BinaryExpression",
                operator: "/",
                left: { type: "NumberLiteral", value: "10", literalType: i64 },
                right: { type: "NumberLiteral", value: "5", literalType: i64 },
              },
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("mul i64")
      expect(ir).toContain("sdiv i64")
      expect(ir).toContain("add i64")
    })
  })
})
