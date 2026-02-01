import { tokenize } from "./lexer.js"
import { parseTokens } from "./parser.js"
import { SemanticAnalyzer } from "./analyzer.js"
import { generateLLVMIR } from "./llvm_codegen.js"
import { type ProgramNode } from "./analyzer.js"
import { compileRuntime, linkToExecutable } from "./linker.js"

export interface Compiler {
  compile(source: string): Promise<CompiledProgram>
}

export interface CompiledProgram {
  llvmIR: string
  errors: CompileError[]
  objectCode?: string
  executable?: Buffer
}

export interface CompileError {
  message: string
  line: number
  column: number
}

export async function compile(source: string): Promise<CompiledProgram> {
  const errors: CompileError[] = []

  try {
    const tokens = tokenize(source)
    const filteredTokens = tokens.filter((t) => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    const ast: ProgramNode = parseTokens(filteredTokens)

    const analyzer = new SemanticAnalyzer()
    const semanticErrors = analyzer.analyze(ast)

    if (semanticErrors.length > 0) {
      errors.push(
        ...semanticErrors.map((e) => ({
          message: e.message,
          line: e.position.start.line,
          column: e.position.start.column,
        })),
      )
      return { llvmIR: "", errors }
    }

    const llvmIR = generateLLVMIR(ast)

    return { llvmIR, errors: [] }
  } catch (error: unknown) {
    if (error instanceof Error) {
      let line = 0
      let column = 0
      const lineMatch = error.message.match(/at line (\d+)/)
      if (lineMatch) {
        line = parseInt(lineMatch[1], 10)
      }
      errors.push({
        message: error.message,
        line,
        column,
      })
    }
    return { llvmIR: "", errors }
  }
}

export class AlgolScriptCompiler implements Compiler {
  async compile(source: string): Promise<CompiledProgram> {
    return compile(source)
  }
}

export async function compileToFile(source: string, outputPath: string): Promise<void> {
  const result = await compile(source)

  if (result.errors.length > 0) {
    console.error("Compilation errors:")
    for (const error of result.errors) {
      console.error(`  Line ${error.line}, Col ${error.column}: ${error.message}`)
    }
    throw new Error("Compilation failed")
  }

  await Bun.write(outputPath, result.llvmIR)
  console.log(`LLVM IR written to ${outputPath}`)
}

export async function compileToExecutable(source: string, outputPath: string): Promise<void> {
  const llFile = outputPath.replace(/\.exe$/, "") + ".ll"
  await compileToFile(source, llFile)

  const llc = Bun.spawn(["llc", "-filetype=obj", llFile], { cwd: process.cwd() })
  await llc.exited

  const objFile = llFile.replace(".ll", ".o")
  const clang = Bun.spawn(["clang", objFile, "-o", outputPath, "-L./runtime", "-lalgol_runtime"], {
    cwd: process.cwd(),
  })
  await clang.exited

  console.log(`Executable written to ${outputPath}`)
}
