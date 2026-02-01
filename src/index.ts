import { compile } from "./compiler.js"
import { readFileSync } from "fs"
import { compileRuntime, linkToExecutable } from "./linker.js"
import { basename } from "path"

const filePath = process.argv[2]
if (!filePath) {
  console.log("Usage: bun run src/index.ts <source.algol>")
  process.exit(1)
}

const source = readFileSync(filePath, "utf-8")

console.log("\nAlgolScript Compiler")
console.log("===================")

const result = await compile(source)

if (result.errors.length > 0) {
  console.log("\nErrors:")
  for (const error of result.errors) {
    console.log(`  [${error.line}:${error.column}] ${error.message}`)
  }
  process.exit(1)
}

console.log("\\nCompiling runtime...")
const runtimeLib = await compileRuntime()

console.log("Linking to executable...")
const outputPath = process.cwd() + "/" + basename(filePath, ".algol")
await linkToExecutable(result.llvmIR, outputPath, runtimeLib)

console.log(`\\n✓ Executable created: ${outputPath}`)
console.log(`\\nRun with: ${outputPath}`)
