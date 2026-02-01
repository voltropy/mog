import { compile } from "./compiler.js"

const source = `
BEGIN
  i32 x := 42;
  i32 y := 10;
  i32 z := (x + y) * 2;
END
`

console.log("Testing compilation...")
const result = await compile(source)
console.log("\nErrors:", result.errors)
console.log("Number of errors:", result.errors.length)
if (result.errors.length > 0) {
  result.errors.forEach((e, i) => {
    console.log(`  Error ${i + 1}: [${e.line}:${e.column}] ${e.message}`)
  })
  console.log("\nLLVM IR (partial):", result.llvmIR ? result.llvmIR.substring(0, 500) : "No IR generated")
} else {
  console.log("\nCOMPILATION SUCCESSFUL!")
  console.log("\nLLVM IR:")
  console.log(result.llvmIR)
}
