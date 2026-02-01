import { writeFileSync, existsSync } from "fs"
import { $ } from "bun"

const __dirname = import.meta.dirname

const runtimePath = __dirname + "/../runtime/runtime.c"
const tempDir = __dirname + "/../build"

// Helper to get clean path for shell commands
const shellPath = (p: string) => (p.startsWith("file://") ? p.replace("file://", "") : p)

export async function compileRuntime(): Promise<string> {
  if (!existsSync(tempDir)) {
    await $`mkdir -p ${tempDir}`
  }

  const runtimeLibPath = tempDir + "/runtime.a"
  const runtimeObjPath = tempDir + "/runtime.o"

  await $`clang -c ${shellPath(runtimePath)} -o ${shellPath(runtimeObjPath)}`
  await $`ar rcs ${shellPath(runtimeLibPath)} ${shellPath(runtimeObjPath)}`
  return runtimeLibPath
}

export async function linkToExecutable(llvmIR: string, outputPath: string, runtimeLib: string): Promise<string> {
  if (!existsSync(tempDir)) {
    await $`mkdir -p ${tempDir}`
  }

  const tempLlvm = tempDir + "/temp.ll"
  const tempObj = tempDir + "/temp.o"

  await writeFileSync(tempLlvm, llvmIR)

  await $`clang -c -x ir ${shellPath(tempLlvm)} -o ${tempObj}`
  await $`clang ${tempObj} ${shellPath(runtimeLib)} -o ${shellPath(outputPath)}`

  return outputPath
}
