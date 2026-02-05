import { generateLLVMIR } from "./src/llvm_codegen.ts"

// Test function for small inline optimization
const ast = {
  type: "Program",
  statements: [
    {
      type: "FunctionDeclaration",
      name: "add",
      params: [
        { name: "a", paramType: { type: "IntegerType", kind: "i64" } },
        { name: "b", paramType: { type: "IntegerType", kind: "i64" } }
      ],
      returnType: { type: "IntegerType", kind: "i64" },
      body: {
        type: "Block",
        statements: [
          {
            type: "Return",
            value: {
              type: "BinaryExpression",
              operator: "+",
              left: { type: "Identifier", name: "a" },
              right: { type: "Identifier", name: "b" }
            }
          }
        ]
      }
    },
    {
      type: "ForLoop",
      variable: "i",
      start: { type: "IntegerLiteral", value: 0 },
      end: { type: "IntegerLiteral", value: 100 },
      body: {
        type: "Block",
        statements: []
      }
    }
  ]
}

// Test with release mode and vectorization enabled
const ir = generateLLVMIR(ast, {
  releaseMode: true,
  vectorization: true,
  inlineThreshold: 3
})

console.log(ir)
