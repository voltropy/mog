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
  const mogVmPath = __dirname + "/../runtime/mog_vm.c"
  const mogVmObjPath = tempDir + "/mog_vm.o"
  const mogAsyncPath = __dirname + "/../runtime/mog_async.c"
  const mogAsyncObjPath = tempDir + "/mog_async.o"
  const posixHostPath = __dirname + "/../runtime/posix_host.c"
  const posixHostObjPath = tempDir + "/posix_host.o"
  const mogHeaderPath = __dirname + "/../runtime"

  await $`clang -c ${shellPath(runtimePath)} -o ${shellPath(runtimeObjPath)}`

  const objFiles = [shellPath(runtimeObjPath)]

  // Compile mog_vm.c if it exists
  if (existsSync(mogVmPath)) {
    await $`clang -c -I${shellPath(mogHeaderPath)} ${shellPath(mogVmPath)} -o ${shellPath(mogVmObjPath)}`
    objFiles.push(shellPath(mogVmObjPath))
  }

  // Compile mog_async.c if it exists
  if (existsSync(mogAsyncPath)) {
    await $`clang -c -I${shellPath(mogHeaderPath)} ${shellPath(mogAsyncPath)} -o ${shellPath(mogAsyncObjPath)}`
    objFiles.push(shellPath(mogAsyncObjPath))
  }

  // Compile posix_host.c if it exists
  if (existsSync(posixHostPath)) {
    await $`clang -c -I${shellPath(mogHeaderPath)} ${shellPath(posixHostPath)} -o ${shellPath(posixHostObjPath)}`
    objFiles.push(shellPath(posixHostObjPath))
  }

  await $`ar rcs ${shellPath(runtimeLibPath)} ${objFiles}`

  return runtimeLibPath
}

export async function linkToExecutable(llvmIR: string, outputPath: string, runtimeLib: string): Promise<string> {
  if (!existsSync(tempDir)) {
    await $`mkdir -p ${tempDir}`
  }

  const tempLlvm = tempDir + "/temp.ll"
  const tempObj = tempDir + "/temp.o"

  await writeFileSync(tempLlvm, llvmIR)

  // Use -O1 so LLVM coroutine passes run (required for presplitcoroutine lowering).
  // -O1 does not break non-async code.
  await $`clang -O1 -c -x ir ${shellPath(tempLlvm)} -o ${tempObj}`
  await $`clang ${tempObj} ${shellPath(runtimeLib)} -o ${shellPath(outputPath)}`

  return outputPath
}
