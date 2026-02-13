/**
 * Module resolver for the Mog language module system.
 * Implements Go-style package resolution with:
 * - Package = directory convention
 * - mog.mod file at project root
 * - Import paths relative to module root
 * - pub keyword for exported symbols
 * - Circular import detection
 */

import { existsSync, readFileSync, readdirSync } from "fs"
import * as path from "path"
import { tokenize } from "./lexer.js"
import { parseTokens } from "./parser.js"
import type { ProgramNode } from "./analyzer.js"

export interface MogModule {
  path: string           // e.g., "math"
  packageName: string    // e.g., "math"
  files: string[]        // absolute paths to .mog files
  ast: ProgramNode       // merged AST from all files in package
  exports: Map<string, ExportedSymbol>
}

export interface ExportedSymbol {
  name: string
  kind: "function" | "struct" | "type" | "variable"
  node: any  // AST node
}

export interface ModuleGraph {
  root: string           // module root path (where mog.mod lives)
  modulePath: string     // from mog.mod
  packages: Map<string, MogModule>
  mainPackage: MogModule
}

/**
 * Find the module root by walking up directories looking for mog.mod.
 */
export function findModuleRoot(startPath: string): string | null {
  let dir = path.resolve(startPath)
  while (true) {
    if (existsSync(path.join(dir, "mog.mod"))) {
      return dir
    }
    const parent = path.dirname(dir)
    if (parent === dir) return null // Hit filesystem root
    dir = parent
  }
}

/**
 * Parse a mog.mod file and return the module name.
 * Format: module <name>
 */
export function parseModFile(filePath: string): { module: string } {
  const content = readFileSync(filePath, "utf-8")
  const lines = content.trim().split("\n")
  for (const line of lines) {
    const trimmed = line.trim()
    if (trimmed.startsWith("module ")) {
      return { module: trimmed.slice(7).trim() }
    }
  }
  throw new Error(`Invalid mog.mod file: no module declaration found in ${filePath}`)
}

/**
 * Parse a source string into an AST, filtering whitespace and comments.
 */
function parseSource(source: string): ProgramNode {
  const tokens = tokenize(source)
  const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
  return parseTokens(filtered)
}

/**
 * Get the package name from an AST (from the first PackageDeclaration).
 * Returns null if no package declaration exists (implicit main).
 */
export function getPackageName(ast: ProgramNode): string | null {
  for (const stmt of ast.statements) {
    if ((stmt as any).type === "PackageDeclaration") {
      return (stmt as any).name
    }
  }
  return null
}

/**
 * Collect all import paths from an AST.
 */
export function collectImportPaths(ast: ProgramNode): string[] {
  const imports: string[] = []
  for (const stmt of ast.statements) {
    if ((stmt as any).type === "ImportDeclaration") {
      imports.push(...(stmt as any).paths)
    }
  }
  return imports
}

/**
 * Collect exported symbols (pub items) from an AST.
 */
export function collectExports(ast: ProgramNode): Map<string, ExportedSymbol> {
  const exports = new Map<string, ExportedSymbol>()
  for (const stmt of ast.statements) {
    const s = stmt as any
    if (s.isPublic) {
      if (s.type === "FunctionDeclaration" || s.type === "AsyncFunctionDeclaration") {
        exports.set(s.name, { name: s.name, kind: "function", node: s })
      } else if (s.type === "StructDeclaration" || s.type === "StructDefinition") {
        exports.set(s.name, { name: s.name, kind: "struct", node: s })
      } else if (s.type === "TypeAliasDeclaration") {
        exports.set(s.name, { name: s.name, kind: "type", node: s })
      }
    }
  }
  return exports
}

/**
 * Resolve the full module graph from a main entry file.
 * 
 * 1. Find module root (mog.mod)
 * 2. Parse the main file and find its imports
 * 3. For each import, find the package directory and parse all .mog files
 * 4. Recursively resolve imports of imported packages
 * 5. Detect circular imports
 * 6. Return topologically sorted ModuleGraph
 */
export function resolveImports(mainFile: string): ModuleGraph {
  const absMainFile = path.resolve(mainFile)
  const mainDir = path.dirname(absMainFile)
  
  const root = findModuleRoot(mainDir)
  if (!root) {
    throw new Error(`No mog.mod found for ${mainFile}`)
  }

  const modInfo = parseModFile(path.join(root, "mog.mod"))
  const packages = new Map<string, MogModule>()
  const resolving = new Set<string>() // For circular import detection

  function resolvePackage(importPath: string): MogModule {
    // Check if already resolved
    const existing = packages.get(importPath)
    if (existing) return existing

    // Check for circular imports
    if (resolving.has(importPath)) {
      throw new Error(`Circular import detected: ${importPath}`)
    }
    resolving.add(importPath)

    const pkgDir = path.join(root!, importPath)
    if (!existsSync(pkgDir)) {
      throw new Error(`Package not found: ${importPath} (looking in ${pkgDir})`)
    }

    // Collect all .mog files in the package directory
    const files = readdirSync(pkgDir)
      .filter(f => f.endsWith(".mog"))
      .map(f => path.join(pkgDir, f))

    if (files.length === 0) {
      throw new Error(`No .mog files found in package: ${importPath}`)
    }

    // Parse all files and merge statements
    const allStatements: any[] = []
    let pkgName = path.basename(importPath)

    for (const file of files) {
      const source = readFileSync(file, "utf-8")
      const ast = parseSource(source)
      
      // Extract package name from PackageDeclaration
      const declaredName = getPackageName(ast)
      if (declaredName) {
        pkgName = declaredName
      }

      allStatements.push(...ast.statements)
    }

    // Recursively resolve this package's imports
    for (const stmt of allStatements) {
      if ((stmt as any).type === "ImportDeclaration") {
        const paths = (stmt as any).paths as string[]
        for (const p of paths) {
          resolvePackage(p)
        }
      }
    }

    const mergedAst: ProgramNode = {
      type: "Program",
      statements: allStatements,
      scopeId: `package_${pkgName}`,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: { line: 1, column: 1, index: 0 },
      },
    }

    const exports = collectExports(mergedAst)

    const mod: MogModule = {
      path: importPath,
      packageName: pkgName,
      files,
      ast: mergedAst,
      exports,
    }

    packages.set(importPath, mod)
    resolving.delete(importPath)
    return mod
  }

  // Parse the main file
  const mainSource = readFileSync(absMainFile, "utf-8")
  const mainAst = parseSource(mainSource)
  const mainPkgName = getPackageName(mainAst) || "main"

  // Resolve all imports from the main file
  const mainImports = collectImportPaths(mainAst)
  for (const imp of mainImports) {
    resolvePackage(imp)
  }

  const mainModule: MogModule = {
    path: "",
    packageName: mainPkgName,
    files: [absMainFile],
    ast: mainAst,
    exports: collectExports(mainAst),
  }

  return {
    root,
    modulePath: modInfo.module,
    packages,
    mainPackage: mainModule,
  }
}
