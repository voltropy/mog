import { compile } from "./src/compiler.js"

const source = `BEGIN\ni32 x := 42;\nEND\n`

console.log("Source:", JSON.stringify(source))
console.log("Compiling...")

const result = await compile(source)

console.log("Errors:", result.errors)
console.log("Error count:", result.errors.length)

if (result.errors.length === 0) {
  console.log("\nSuccess! LLVM IR:")
  console.log(result.llvmIR.substring(0, 200))
}
