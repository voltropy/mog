import { describe, test, expect } from "bun:test"
import { generateLLVMIR } from "./llvm_codegen"
import { i64, array, FunctionType } from "./types"

/**
 * Tests for Phase 7: Array Methods
 * 
 * Methods tested:
 *   .len       — array length (property access)
 *   .push(val) — append element
 *   .pop()     — remove and return last element
 *   .contains(val) — returns bool (i64 0/1)
 *   .sort()    — sort in place
 *   .reverse() — reverse in place
 */

// Helper to create a program AST with an array variable and a method call statement
function makeArrayMethodProgram(methodName: string, methodArgs: any[] = []): any {
  return {
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
      {
        type: "ExpressionStatement",
        expression: {
          type: "CallExpression",
          callee: {
            type: "MemberExpression",
            object: { type: "Identifier", name: "arr" },
            property: methodName,
          },
          args: methodArgs,
          arguments: methodArgs,
        },
      },
    ],
  }
}

// Helper to create an AST that assigns the result of a method call to a variable
function makeArrayMethodAssignProgram(methodName: string, methodArgs: any[] = []): any {
  return {
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
      {
        type: "VariableDeclaration",
        name: "result",
        varType: i64,
        value: {
          type: "CallExpression",
          callee: {
            type: "MemberExpression",
            object: { type: "Identifier", name: "arr" },
            property: methodName,
          },
          args: methodArgs,
          arguments: methodArgs,
        },
      },
    ],
  }
}

describe("Array Methods - LLVM IR Generation", () => {
  describe(".push(value)", () => {
    test("generates call to array_push", () => {
      const ast = makeArrayMethodProgram("push", [
        { type: "NumberLiteral", value: "42", literalType: i64 },
      ])
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@array_push")
      expect(ir).toContain("call void @array_push(ptr")
    })
  })

  describe(".pop()", () => {
    test("generates call to array_pop", () => {
      const ast = makeArrayMethodAssignProgram("pop")
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@array_pop")
      expect(ir).toContain("call i64 @array_pop(ptr")
    })
  })

  describe(".contains(value)", () => {
    test("generates call to array_contains", () => {
      const ast = makeArrayMethodAssignProgram("contains", [
        { type: "NumberLiteral", value: "2", literalType: i64 },
      ])
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@array_contains")
      expect(ir).toContain("call i64 @array_contains(ptr")
    })
  })

  describe(".sort()", () => {
    test("generates call to array_sort (no args)", () => {
      const ast = makeArrayMethodProgram("sort")
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@array_sort")
      expect(ir).toContain("call void @array_sort(ptr")
      expect(ir).not.toContain("call void @array_sort_with_comparator(")
    })

    test("generates call to array_sort_with_comparator when given closure", () => {
      const closureType = new FunctionType([i64, i64], i64)
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
                { type: "NumberLiteral", value: "3", literalType: i64 },
                { type: "NumberLiteral", value: "1", literalType: i64 },
                { type: "NumberLiteral", value: "2", literalType: i64 },
              ],
            },
          },
          {
            type: "VariableDeclaration",
            name: "cmp",
            varType: closureType,
            value: {
              type: "Lambda",
              params: [{ name: "a", paramType: i64 }, { name: "b", paramType: i64 }],
              returnType: i64,
              body: {
                type: "Block",
                statements: [
                  {
                    type: "ReturnStatement",
                    value: {
                      type: "BinaryExpression",
                      operator: "-",
                      left: { type: "Identifier", name: "a" },
                      right: { type: "Identifier", name: "b" },
                    },
                  },
                ],
              },
              capturedVars: [],
              capturedVarTypes: {},
            },
          },
          {
            type: "ExpressionStatement",
            expression: {
              type: "CallExpression",
              callee: {
                type: "MemberExpression",
                object: { type: "Identifier", name: "arr" },
                property: "sort",
              },
              args: [{ type: "Identifier", name: "cmp" }],
              arguments: [{ type: "Identifier", name: "cmp" }],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@array_sort_with_comparator")
      expect(ir).toContain("call void @array_sort_with_comparator(ptr")
    })
  })

  describe(".reverse()", () => {
    test("generates call to array_reverse", () => {
      const ast = makeArrayMethodProgram("reverse")
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@array_reverse")
      expect(ir).toContain("call void @array_reverse(ptr")
    })
  })

  describe(".len (property access)", () => {
    test("generates call to array_length for .len property", () => {
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
              ],
            },
          },
          {
            type: "VariableDeclaration",
            name: "length",
            varType: i64,
            value: {
              type: "MemberExpression",
              object: { type: "Identifier", name: "arr" },
              property: "len",
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@array_length")
      expect(ir).toContain("call i64 @array_length(ptr")
    })
  })

  describe(".len (via method call syntax)", () => {
    test("generates call to array_length for .len() call", () => {
      const ast = makeArrayMethodAssignProgram("len")
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@array_length")
      expect(ir).toContain("call i64 @array_length(ptr")
    })
  })

  describe(".filter(closure)", () => {
    test("generates call to array_filter with closure fn and env", () => {
      const closureType = new FunctionType([i64], i64)
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
          {
            type: "VariableDeclaration",
            name: "pred",
            varType: closureType,
            value: {
              type: "Lambda",
              params: [{ name: "x", paramType: i64 }],
              returnType: i64,
              body: {
                type: "Block",
                statements: [
                  {
                    type: "ReturnStatement",
                    value: { type: "NumberLiteral", value: "1", literalType: i64 },
                  },
                ],
              },
              capturedVars: [],
              capturedVarTypes: {},
            },
          },
          {
            type: "VariableDeclaration",
            name: "result",
            varType: array(i64, 1),
            value: {
              type: "CallExpression",
              callee: {
                type: "MemberExpression",
                object: { type: "Identifier", name: "arr" },
                property: "filter",
              },
              args: [{ type: "Identifier", name: "pred" }],
              arguments: [{ type: "Identifier", name: "pred" }],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@array_filter")
      expect(ir).toContain("call ptr @array_filter(ptr")
    })
  })

  describe(".map(closure)", () => {
    test("generates call to array_map with closure fn and env", () => {
      const closureType = new FunctionType([i64], i64)
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
          {
            type: "VariableDeclaration",
            name: "transform",
            varType: closureType,
            value: {
              type: "Lambda",
              params: [{ name: "x", paramType: i64 }],
              returnType: i64,
              body: {
                type: "Block",
                statements: [
                  {
                    type: "ReturnStatement",
                    value: {
                      type: "BinaryExpression",
                      operator: "*",
                      left: { type: "Identifier", name: "x" },
                      right: { type: "NumberLiteral", value: "2", literalType: i64 },
                    },
                  },
                ],
              },
              capturedVars: [],
              capturedVarTypes: {},
            },
          },
          {
            type: "VariableDeclaration",
            name: "result",
            varType: array(i64, 1),
            value: {
              type: "CallExpression",
              callee: {
                type: "MemberExpression",
                object: { type: "Identifier", name: "arr" },
                property: "map",
              },
              args: [{ type: "Identifier", name: "transform" }],
              arguments: [{ type: "Identifier", name: "transform" }],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("@array_map")
      expect(ir).toContain("call ptr @array_map(ptr")
    })
  })

  describe("LLVM IR declarations", () => {
    test("declares all array method runtime functions", () => {
      const ast = { type: "Program", statements: [] }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("declare void @array_push(ptr %array, i64 %value)")
      expect(ir).toContain("declare i64 @array_pop(ptr %array)")
      expect(ir).toContain("declare i64 @array_contains(ptr %array, i64 %value)")
      expect(ir).toContain("declare void @array_sort(ptr %array)")
      expect(ir).toContain("declare void @array_sort_with_comparator(ptr %array, ptr %fn, i64 %env)")
      expect(ir).toContain("declare void @array_reverse(ptr %array)")
      expect(ir).toContain("declare ptr @array_filter(ptr %array, ptr %fn, i64 %env)")
      expect(ir).toContain("declare ptr @array_map(ptr %array, ptr %fn, i64 %env)")
    })
  })

  describe("chained operations", () => {
    test("push then pop generates both calls", () => {
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
              ],
            },
          },
          {
            type: "ExpressionStatement",
            expression: {
              type: "CallExpression",
              callee: {
                type: "MemberExpression",
                object: { type: "Identifier", name: "arr" },
                property: "push",
              },
              args: [{ type: "NumberLiteral", value: "99", literalType: i64 }],
              arguments: [{ type: "NumberLiteral", value: "99", literalType: i64 }],
            },
          },
          {
            type: "VariableDeclaration",
            name: "last",
            varType: i64,
            value: {
              type: "CallExpression",
              callee: {
                type: "MemberExpression",
                object: { type: "Identifier", name: "arr" },
                property: "pop",
              },
              args: [],
              arguments: [],
            },
          },
        ],
      }
      const ir = generateLLVMIR(ast)
      expect(ir).toContain("call void @array_push(ptr")
      expect(ir).toContain("call i64 @array_pop(ptr")
    })
  })
})
