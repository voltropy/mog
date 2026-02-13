import { compile, compileModule } from "./compiler.js"
import { readFileSync, existsSync } from "fs"
import { compileRuntime, linkToExecutable } from "./linker.js"
import { basename, dirname, resolve, join } from "path"

const filePath = process.argv[2]
if (!filePath) {
  console.log("Usage: bun run src/index.ts <source.mog>")
  process.exit(1)
}

console.log("\nMog Compiler")
console.log("===================")

// Detect module mode: walk up from file to find mog.mod
function findModRoot(startDir: string): string | null {
  let dir = startDir
  while (true) {
    if (existsSync(join(dir, "mog.mod"))) return dir
    const parent = dirname(dir)
    if (parent === dir) return null
    dir = parent
  }
}

const absPath = resolve(filePath)
const modRoot = findModRoot(dirname(absPath))

let result
if (modRoot) {
  console.log(`Module root: ${modRoot}`)
  result = await compileModule(absPath)
} else {
  const source = readFileSync(filePath, "utf-8")
  result = await compile(source)
}

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
const outputPath = process.cwd() + "/" + basename(filePath, ".mog")
await linkToExecutable(result.llvmIR, outputPath, runtimeLib)

console.log(`\\n✓ Executable created: ${outputPath}`)
console.log(`\\nRun with: ${outputPath}`)
