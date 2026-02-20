import { tokenize } from "./lexer.js"
import { parseTokens } from "./parser.js"
import { SemanticAnalyzer } from "./analyzer.js"
import { generateLLVMIR, generateModuleLLVMIR } from "./llvm_codegen.js"
import { type ProgramNode } from "./analyzer.js"
import { compileRuntime, linkToExecutable } from "./linker.js"
import { parseCapabilityDecl } from "./capability.js"
import { existsSync, readFileSync, readdirSync, writeFileSync, unlinkSync, mkdtempSync } from "fs"
import * as path from "path"
import { tmpdir } from "os"
import { spawnSync } from "child_process"

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

    const llvmIR = generateLLVMIR(ast, undefined, allCaps, capabilityDecls)

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

/**
 * Compile a multi-package module from an entry file.
 * Resolves imports, collects all packages, and produces combined LLVM IR.
 */
export async function compileModule(entryFile: string): Promise<CompiledProgram> {
  const errors: CompileError[] = []

  try {
    // Find module root (directory containing mog.mod)
    const moduleRoot = findModuleRoot(path.dirname(path.resolve(entryFile)))
    if (!moduleRoot) {
      // No mog.mod found — fall back to single-file compilation
      const source = readFileSync(entryFile, "utf-8")
      return compile(source)
    }

    const modFile = readFileSync(path.join(moduleRoot, "mog.mod"), "utf-8")
    const modulePath = parseModFile(modFile)

    // Parse the entry file first
    const entrySource = readFileSync(entryFile, "utf-8")
    const entryAst = parseSource(entrySource)

    // Collect imports from the entry file
    const imports = collectImports(entryAst)
    
    // Resolve all packages
    const packages: { packageName: string; ast: ProgramNode; exports: Map<string, { name: string; kind: string }> }[] = []
    const resolved = new Set<string>()
    const resolving = new Set<string>() // For circular import detection

    // Recursively resolve packages
    const resolvePackage = (importPath: string): void => {
      if (resolved.has(importPath)) return
      if (resolving.has(importPath)) {
        errors.push({
          message: `Circular import detected: ${importPath}`,
          line: 0,
          column: 0,
        })
        return
      }
      resolving.add(importPath)

      const pkgDir = path.join(moduleRoot, importPath)
      if (!existsSync(pkgDir)) {
        errors.push({
          message: `Package not found: ${importPath} (looking in ${pkgDir})`,
          line: 0,
          column: 0,
        })
        return
      }

      // Collect all .mog files in the package directory
      const files = readdirSync(pkgDir).filter(f => f.endsWith(".mog"))
      if (files.length === 0) {
        errors.push({
          message: `No .mog files found in package: ${importPath}`,
          line: 0,
          column: 0,
        })
        return
      }

      // Parse and merge all files in the package
      const allStatements: any[] = []
      let pkgName = path.basename(importPath)

      for (const file of files) {
        const source = readFileSync(path.join(pkgDir, file), "utf-8")
        const ast = parseSource(source)
        
        // Extract package name from first PackageDeclaration
        for (const stmt of ast.statements) {
          if ((stmt as any).type === "PackageDeclaration") {
            pkgName = (stmt as any).name
            break
          }
        }

        allStatements.push(...ast.statements)
      }

      // Recursively resolve this package's imports
      const pkgImports = collectImportsFromStatements(allStatements)
      for (const imp of pkgImports) {
        resolvePackage(imp)
      }

      // Collect exports (pub items)
      const exports = collectExports(allStatements)

      const mergedAst: ProgramNode = {
        type: "Program",
        statements: allStatements,
        scopeId: `package_${pkgName}`,
        position: {
          start: { line: 1, column: 1, index: 0 },
          end: { line: 1, column: 1, index: 0 },
        },
      }

      packages.push({ packageName: pkgName, ast: mergedAst, exports })
      resolving.delete(importPath)
      resolved.add(importPath)
    }

    // Resolve all imports from the entry file
    for (const imp of imports) {
      resolvePackage(imp)
    }

    if (errors.length > 0) {
      return { llvmIR: "", errors }
    }

    // Add the main package
    const mainPkgName = getPackageName(entryAst) || "main"
    const mainExports = collectExports(entryAst.statements as any[])
    packages.push({ packageName: mainPkgName, ast: entryAst, exports: mainExports })

    // Run semantic analysis on each package
    // For the main package, register imported package exports as available symbols
    for (const pkg of packages) {
      const analyzer = new SemanticAnalyzer()
      
      // Register imported packages' exported symbols
      if (pkg.packageName === mainPkgName) {
        for (const otherPkg of packages) {
          if (otherPkg.packageName !== mainPkgName) {
            // Register the package name as a variable so pkg.func works
            // The actual resolution of cross-package calls happens in codegen via name mangling
            for (const [name, info] of otherPkg.exports) {
              // We don't register individual exports here — the codegen handles mangling
              void name
              void info
            }
          }
        }
      }

      const semanticErrors = analyzer.analyze(pkg.ast)
      // Filter out "undeclared variable" errors for imported package names
      const importedPkgNames = new Set(packages.filter(p => p.packageName !== mainPkgName).map(p => p.packageName))
      const filteredErrors = semanticErrors.filter(e => {
        // Allow undefined variable references that match imported package names
        const undefinedMatch = e.message.match(/^Undefined variable '(\w+)'$/)
        if (undefinedMatch) {
          const varName = undefinedMatch[1]
          if (importedPkgNames.has(varName)) return false
        }
        return true
      })

      if (filteredErrors.length > 0) {
        errors.push(
          ...filteredErrors.map(e => ({
            message: `[${pkg.packageName}] ${e.message}`,
            line: e.position.start.line,
            column: e.position.start.column,
          }))
        )
      }
    }

    if (errors.length > 0) {
      return { llvmIR: "", errors }
    }

    // Generate combined LLVM IR
    const llvmIR = generateModuleLLVMIR(packages)

    return { llvmIR, errors: [] }
  } catch (error: unknown) {
    if (error instanceof Error) {
      errors.push({
        message: error.message,
        line: 0,
        column: 0,
      })
    }
    return { llvmIR: "", errors }
  }
}

// Helper functions for module compilation

function findModuleRoot(startDir: string): string | null {
  let dir = startDir
  while (true) {
    if (existsSync(path.join(dir, "mog.mod"))) {
      return dir
    }
    const parent = path.dirname(dir)
    if (parent === dir) return null // Hit filesystem root
    dir = parent
  }
}

function parseModFile(content: string): string {
  // Format: module <name>
  const lines = content.trim().split("\n")
  for (const line of lines) {
    const trimmed = line.trim()
    if (trimmed.startsWith("module ")) {
      return trimmed.slice(7).trim()
    }
  }
  return ""
}

function parseSource(source: string): ProgramNode {
  const tokens = tokenize(source)
  const filteredTokens = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
  return parseTokens(filteredTokens)
}

function getPackageName(ast: ProgramNode): string | null {
  for (const stmt of ast.statements) {
    if ((stmt as any).type === "PackageDeclaration") {
      return (stmt as any).name
    }
  }
  return null
}

function collectImports(ast: ProgramNode): string[] {
  return collectImportsFromStatements(ast.statements as any[])
}

function collectImportsFromStatements(statements: any[]): string[] {
  const imports: string[] = []
  for (const stmt of statements) {
    if (stmt.type === "ImportDeclaration") {
      imports.push(...stmt.paths)
    }
  }
  return imports
}

function collectExports(statements: any[]): Map<string, { name: string; kind: string }> {
  const exports = new Map<string, { name: string; kind: string }>()
  for (const stmt of statements) {
    if (stmt.isPublic) {
      if (stmt.type === "FunctionDeclaration" || stmt.type === "AsyncFunctionDeclaration") {
        exports.set(stmt.name, { name: stmt.name, kind: "function" })
      } else if (stmt.type === "StructDeclaration" || stmt.type === "StructDefinition") {
        exports.set(stmt.name, { name: stmt.name, kind: "struct" })
      } else if (stmt.type === "TypeAliasDeclaration") {
        exports.set(stmt.name, { name: stmt.name, kind: "type" })
      }
    }
  }
  return exports
}

/**
 * Compile source in plugin mode, producing LLVM IR suitable for a shared library.
 * Uses the same tokenize -> filter -> parse -> analyze pipeline as compile(),
 * but passes pluginOptions to generateLLVMIR so the codegen emits plugin metadata
 * and uses appropriate linkage.
 */
export async function compilePlugin(
  source: string,
  pluginName: string,
  pluginVersion?: string,
): Promise<{ llvmIR: string; errors: CompileError[] }> {
  const errors: CompileError[] = []

  try {
    const tokens = tokenize(source)
    const filteredTokens = tokens.filter((t) => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    const ast: ProgramNode = parseTokens(filteredTokens)

    // Extract capability declarations from AST (same as compile)
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

    // Load capability declarations from .mogdecl files (same as compile)
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

    const llvmIR = generateLLVMIR(ast, undefined, allCaps, capabilityDecls, {
      pluginMode: true,
      pluginName: pluginName,
      pluginVersion: pluginVersion || '0.1.0'
    })

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

/**
 * Compile source to a shared library (.dylib on macOS) suitable for plugin loading.
 * Runs the full plugin compilation pipeline, then invokes llc + clang to produce
 * a position-independent shared library.
 */
export async function compilePluginToSharedLib(
  source: string,
  pluginName: string,
  outputPath: string,
  pluginVersion?: string,
): Promise<CompiledProgram> {
  const result = await compilePlugin(source, pluginName, pluginVersion)

  if (result.errors.length > 0) {
    return { llvmIR: result.llvmIR, errors: result.errors }
  }

  const tempDir = mkdtempSync(path.join(tmpdir(), "mog-plugin-"))
  const llFile = path.join(tempDir, `${pluginName}.ll`)
  const oFile = path.join(tempDir, `${pluginName}.o`)

  try {
    writeFileSync(llFile, result.llvmIR)

    // Compile LLVM IR to object file with PIC relocation for shared library
    const llcPath = "/opt/homebrew/opt/llvm/bin/llc"
    const llcResult = spawnSync(llcPath, ["-filetype=obj", "-relocation-model=pic", "-O1", llFile, "-o", oFile], {
      stdio: "pipe",
    })

    if (llcResult.status !== 0) {
      const stderr = llcResult.stderr?.toString() || "unknown llc error"
      return {
        llvmIR: result.llvmIR,
        errors: [{ message: `llc failed: ${stderr}`, line: 0, column: 0 }],
      }
    }

    // Link to shared library
    // On macOS use -dynamiclib; link against the runtime archive and system libs
    // Look for runtime.a relative to project root (parent of src/)
    const projectRoot = path.resolve(import.meta.dir, "..")
    const runtimePath = path.resolve(projectRoot, "build", "runtime.a")
    const clangArgs = [
      "-shared", "-dynamiclib",
      "-o", outputPath,
      oFile,
      ...(existsSync(runtimePath) ? [runtimePath] : []),
      "-lSystem", "-lm",
    ]

    const clangResult = spawnSync("clang", clangArgs, {
      stdio: "pipe",
      cwd: process.cwd(),
    })

    if (clangResult.status !== 0) {
      const stderr = clangResult.stderr?.toString() || "unknown clang error"
      return {
        llvmIR: result.llvmIR,
        errors: [{ message: `clang linking failed: ${stderr}`, line: 0, column: 0 }],
      }
    }

    return { llvmIR: result.llvmIR, errors: [] }
  } finally {
    // Clean up temp files
    try { unlinkSync(llFile) } catch {}
    try { unlinkSync(oFile) } catch {}
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
