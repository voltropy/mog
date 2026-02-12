import { tokenize } from "./lexer.js"
import { parseTokens } from "./parser.js"
import { SemanticAnalyzer } from "./analyzer.js"
import { generateLLVMIR } from "./llvm_codegen.js"
import { type ProgramNode } from "./analyzer.js"
import { compileRuntime, linkToExecutable } from "./linker.js"
import { parseCapabilityDecl } from "./capability.js"
import { existsSync, readFileSync } from "fs"
import * as path from "path"

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

    // Extract capability declarations from AST (may be top-level or inside a Block)
    const requiredCaps: string[] = []
    const optionalCaps: string[] = []
    const extractCaps = (statements: any[]) => {
      for (const stmt of statements) {
        if ((stmt as any).type === "RequiresDeclaration") {
          requiredCaps.push(...(stmt as any).capabilities)
        }
        if ((stmt as any).type === "OptionalDeclaration") {
          optionalCaps.push(...(stmt as any).capabilities)
        }
        if ((stmt as any).type === "Block" && (stmt as any).statements) {
          extractCaps((stmt as any).statements)
        }
      }
    }
    extractCaps(ast.statements)

    // Load capability declarations from .mogdecl files
    const capabilityDecls = new Map<string, any>()
    const allCaps = [...requiredCaps, ...optionalCaps]
    const __dirname = import.meta.dirname || path.dirname(new URL(import.meta.url).pathname)
    const searchPaths = [
      path.resolve(__dirname, "../capabilities"),
      path.resolve(process.cwd(), "capabilities"),
    ]
    for (const capName of allCaps) {
      for (const searchPath of searchPaths) {
        const declPath = path.join(searchPath, `${capName}.mogdecl`)
        if (existsSync(declPath)) {
          const declSource = readFileSync(declPath, "utf-8")
          const decls = parseCapabilityDecl(declSource)
          const decl = decls.find(d => d.name === capName)
          if (decl) {
            capabilityDecls.set(capName, decl)
          }
          break
        }
      }
    }

    const analyzer = new SemanticAnalyzer()
    analyzer.setCapabilityDecls(capabilityDecls)
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

    const llvmIR = generateLLVMIR(ast, undefined, allCaps)

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

export class MogCompiler implements Compiler {
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
  const clang = Bun.spawn(["clang", objFile, "-o", outputPath, "-L./runtime", "-lmog_runtime"], {
    cwd: process.cwd(),
  })
  await clang.exited

  console.log(`Executable written to ${outputPath}`)
}
