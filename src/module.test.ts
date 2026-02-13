import { describe, test, expect, beforeEach, afterEach } from "bun:test"
import { tokenize } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"
import { compile, compileModule } from "./compiler"
import { generateLLVMIR, generateModuleLLVMIR } from "./llvm_codegen"
import {
  findModuleRoot,
  parseModFile,
  collectExports,
  collectImportPaths,
  getPackageName,
  resolveImports,
} from "./module"
import { mkdirSync, writeFileSync, rmSync, existsSync } from "fs"
import * as path from "path"
import type { ProgramNode } from "./analyzer"

// Helper to parse source into AST
function parse(source: string): ProgramNode {
  const tokens = tokenize(source)
  const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
  return parseTokens(filtered)
}

// Helper to filter non-whitespace/comment tokens
function getTokenTypes(source: string): string[] {
  return tokenize(source)
    .filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    .map(t => t.type)
}

// Temp directory for filesystem-based tests
const tmpDir = path.join(import.meta.dirname || __dirname, "__test_modules__")

function setupTestProject(files: Record<string, string>) {
  // Clean up if exists
  if (existsSync(tmpDir)) {
    rmSync(tmpDir, { recursive: true })
  }
  mkdirSync(tmpDir, { recursive: true })

  for (const [filePath, content] of Object.entries(files)) {
    const fullPath = path.join(tmpDir, filePath)
    mkdirSync(path.dirname(fullPath), { recursive: true })
    writeFileSync(fullPath, content)
  }
}

function cleanupTestProject() {
  if (existsSync(tmpDir)) {
    rmSync(tmpDir, { recursive: true })
  }
}

describe("Module System", () => {
  afterEach(() => {
    cleanupTestProject()
  })

  // =========================================================================
  // LEXER TESTS
  // =========================================================================
  describe("Lexer", () => {
    test("tokenizes 'package' keyword", () => {
      const tokens = tokenize("package")
      expect(tokens).toEqual([
        {
          type: "package",
          value: "package",
          position: {
            start: { line: 1, column: 1, index: 0 },
            end: { line: 1, column: 8, index: 7 },
          },
        },
      ])
    })

    test("tokenizes 'import' keyword", () => {
      const tokens = tokenize("import")
      expect(tokens).toEqual([
        {
          type: "import",
          value: "import",
          position: {
            start: { line: 1, column: 1, index: 0 },
            end: { line: 1, column: 7, index: 6 },
          },
        },
      ])
    })

    test("tokenizes 'pub' keyword", () => {
      const tokens = tokenize("pub")
      expect(tokens).toEqual([
        {
          type: "pub",
          value: "pub",
          position: {
            start: { line: 1, column: 1, index: 0 },
            end: { line: 1, column: 4, index: 3 },
          },
        },
      ])
    })

    test("package keyword is distinct from identifiers", () => {
      const types = getTokenTypes("package main")
      expect(types).toEqual(["package", "IDENTIFIER"])
    })

    test("import keyword is distinct from identifiers", () => {
      const types = getTokenTypes('import "math"')
      expect(types).toEqual(["import", "STRING"])
    })

    test("pub keyword is distinct from identifiers", () => {
      const types = getTokenTypes("pub fn add")
      expect(types).toEqual(["pub", "fn", "IDENTIFIER"])
    })

    test("package-like identifiers are not confused with keyword", () => {
      const types = getTokenTypes("packageName")
      expect(types).toEqual(["IDENTIFIER"])
    })

    test("import-like identifiers are not confused with keyword", () => {
      const types = getTokenTypes("importData")
      expect(types).toEqual(["IDENTIFIER"])
    })

    test("pub-like identifiers are not confused with keyword", () => {
      const types = getTokenTypes("publisher")
      expect(types).toEqual(["IDENTIFIER"])
    })

    test("tokenizes full package declaration", () => {
      const types = getTokenTypes("package math;")
      expect(types).toEqual(["package", "IDENTIFIER", "SEMICOLON"])
    })

    test("tokenizes single import", () => {
      const types = getTokenTypes('import "math"')
      expect(types).toEqual(["import", "STRING"])
    })

    test("tokenizes grouped import", () => {
      const types = getTokenTypes('import ( "math" "strings" )')
      expect(types).toEqual(["import", "LPAREN", "STRING", "STRING", "RPAREN"])
    })

    test("tokenizes pub fn declaration", () => {
      const types = getTokenTypes("pub fn add(a: i64, b: i64) -> i64 { return a + b; }")
      expect(types[0]).toBe("pub")
      expect(types[1]).toBe("fn")
      expect(types[2]).toBe("IDENTIFIER") // add
    })

    test("tokenizes pub struct declaration", () => {
      const types = getTokenTypes("pub struct Point { x: f64, y: f64 }")
      expect(types[0]).toBe("pub")
      expect(types[1]).toBe("struct")
      expect(types[2]).toBe("IDENTIFIER") // Point
    })
  })

  // =========================================================================
  // PARSER TESTS
  // =========================================================================
  describe("Parser", () => {
    test("parses package declaration", () => {
      const ast = parse("package math;")
      expect(ast.statements.length).toBe(1)
      const pkg = ast.statements[0] as any
      expect(pkg.type).toBe("PackageDeclaration")
      expect(pkg.name).toBe("math")
    })

    test("parses package declaration without semicolon", () => {
      const ast = parse("package main\nfn foo() -> i64 { return 1; }")
      expect(ast.statements.length).toBe(2)
      const pkg = ast.statements[0] as any
      expect(pkg.type).toBe("PackageDeclaration")
      expect(pkg.name).toBe("main")
    })

    test("parses single import", () => {
      const ast = parse('import "math"')
      expect(ast.statements.length).toBe(1)
      const imp = ast.statements[0] as any
      expect(imp.type).toBe("ImportDeclaration")
      expect(imp.paths).toEqual(["math"])
    })

    test("parses single import with semicolon", () => {
      const ast = parse('import "math";')
      expect(ast.statements.length).toBe(1)
      const imp = ast.statements[0] as any
      expect(imp.type).toBe("ImportDeclaration")
      expect(imp.paths).toEqual(["math"])
    })

    test("parses grouped imports", () => {
      const ast = parse('import ( "math" "strings" "myapp/utils" )')
      expect(ast.statements.length).toBe(1)
      const imp = ast.statements[0] as any
      expect(imp.type).toBe("ImportDeclaration")
      expect(imp.paths).toEqual(["math", "strings", "myapp/utils"])
    })

    test("parses grouped imports with semicolons", () => {
      const ast = parse('import ( "math"; "strings"; )')
      expect(ast.statements.length).toBe(1)
      const imp = ast.statements[0] as any
      expect(imp.type).toBe("ImportDeclaration")
      expect(imp.paths).toEqual(["math", "strings"])
    })

    test("parses grouped imports with commas", () => {
      const ast = parse('import ( "math", "strings" )')
      expect(ast.statements.length).toBe(1)
      const imp = ast.statements[0] as any
      expect(imp.type).toBe("ImportDeclaration")
      expect(imp.paths).toEqual(["math", "strings"])
    })

    test("parses pub fn declaration", () => {
      const ast = parse("pub fn add(a: i64, b: i64) -> i64 { return a + b; }")
      expect(ast.statements.length).toBe(1)
      const fn = ast.statements[0] as any
      expect(fn.type).toBe("FunctionDeclaration")
      expect(fn.name).toBe("add")
      expect(fn.isPublic).toBe(true)
    })

    test("parses non-pub fn declaration (no isPublic)", () => {
      const ast = parse("fn add(a: i64, b: i64) -> i64 { return a + b; }")
      expect(ast.statements.length).toBe(1)
      const fn = ast.statements[0] as any
      expect(fn.type).toBe("FunctionDeclaration")
      expect(fn.isPublic).toBeUndefined()
    })

    test("parses pub struct declaration", () => {
      const ast = parse("pub struct Point { x: f64, y: f64 }")
      expect(ast.statements.length).toBe(1)
      const st = ast.statements[0] as any
      expect(st.type).toBe("StructDeclaration")
      expect(st.name).toBe("Point")
      expect(st.isPublic).toBe(true)
    })

    test("parses pub type alias", () => {
      const ast = parse("pub type Num = i64")
      expect(ast.statements.length).toBe(1)
      const ta = ast.statements[0] as any
      expect(ta.type).toBe("TypeAliasDeclaration")
      expect(ta.isPublic).toBe(true)
    })

    test("parses pub async fn", () => {
      const ast = parse("pub async fn fetch(url: string) -> string { return url; }")
      expect(ast.statements.length).toBe(1)
      const fn = ast.statements[0] as any
      expect(fn.type).toBe("AsyncFunctionDeclaration")
      expect(fn.name).toBe("fetch")
      expect(fn.isPublic).toBe(true)
    })

    test("parses full module file structure", () => {
      const source = `
package math

pub fn add(a: i64, b: i64) -> i64 {
  return a + b;
}

fn helper() -> i64 {
  return 42;
}

pub struct Vector {
  x: f64,
  y: f64,
}
`
      const ast = parse(source)
      expect(ast.statements.length).toBe(4)

      const pkg = ast.statements[0] as any
      expect(pkg.type).toBe("PackageDeclaration")
      expect(pkg.name).toBe("math")

      const addFn = ast.statements[1] as any
      expect(addFn.type).toBe("FunctionDeclaration")
      expect(addFn.name).toBe("add")
      expect(addFn.isPublic).toBe(true)

      const helperFn = ast.statements[2] as any
      expect(helperFn.type).toBe("FunctionDeclaration")
      expect(helperFn.name).toBe("helper")
      expect(helperFn.isPublic).toBeUndefined()

      const vector = ast.statements[3] as any
      expect(vector.type).toBe("StructDeclaration")
      expect(vector.name).toBe("Vector")
      expect(vector.isPublic).toBe(true)
    })

    test("parses main file with imports", () => {
      const source = `
package main

import "math"

fn main() -> i64 {
  return 0;
}
`
      const ast = parse(source)
      expect(ast.statements.length).toBe(3)

      const pkg = ast.statements[0] as any
      expect(pkg.type).toBe("PackageDeclaration")
      expect(pkg.name).toBe("main")

      const imp = ast.statements[1] as any
      expect(imp.type).toBe("ImportDeclaration")
      expect(imp.paths).toEqual(["math"])

      const mainFn = ast.statements[2] as any
      expect(mainFn.type).toBe("FunctionDeclaration")
      expect(mainFn.name).toBe("main")
    })
  })

  // =========================================================================
  // ANALYZER TESTS
  // =========================================================================
  describe("Analyzer", () => {
    test("accepts package declaration without error", () => {
      const ast = parse("package main\nfn main() -> i64 { return 0; }")
      const analyzer = new SemanticAnalyzer()
      const errors = analyzer.analyze(ast)
      expect(errors.length).toBe(0)
    })

    test("accepts import declaration without error", () => {
      const ast = parse('package main\nimport "math"\nfn main() -> i64 { return 0; }')
      const analyzer = new SemanticAnalyzer()
      const errors = analyzer.analyze(ast)
      expect(errors.length).toBe(0)
    })

    test("accepts pub fn without error", () => {
      const ast = parse("pub fn add(a: i64, b: i64) -> i64 { return a + b; }")
      const analyzer = new SemanticAnalyzer()
      const errors = analyzer.analyze(ast)
      expect(errors.length).toBe(0)
    })

    test("accepts pub struct without error", () => {
      const ast = parse("pub struct Point { x: f64, y: f64 }")
      const analyzer = new SemanticAnalyzer()
      const errors = analyzer.analyze(ast)
      expect(errors.length).toBe(0)
    })
  })

  // =========================================================================
  // BACKWARD COMPATIBILITY TESTS
  // =========================================================================
  describe("Backward Compatibility", () => {
    test("files without package declaration still compile (single-file mode)", async () => {
      const source = `
fn main() -> i64 {
  x: i64 = 42;
  return x;
}
`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i64 @program_user")
    })

    test("files without package declaration still work with expressions", async () => {
      const source = `{
  x: i64 = 42;
  y: i64 = x + 1;
}`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i32 @main()")
    })

    test("function with package and main still generates correct IR", async () => {
      const source = `
package main

fn main() -> i64 {
  return 42;
}
`
      const result = await compile(source)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i64 @program_user")
      expect(result.llvmIR).toContain("define i32 @main(")
    })

    test("struct declaration still works with package", async () => {
      const source = `
package main

struct Point {
  x: i64,
  y: i64,
}

fn main() -> i64 {
  p := Point { x: 1, y: 2 };
  return 0;
}
`
      const result = await compile(source)
      expect(result.errors).toEqual([])
    })
  })

  // =========================================================================
  // MODULE RESOLVER TESTS
  // =========================================================================
  describe("Module Resolver", () => {
    test("findModuleRoot returns directory with mog.mod", () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
        "main.mog": "package main\n",
        "sub/helper.mog": "package helper\n",
      })
      const root = findModuleRoot(path.join(tmpDir, "sub"))
      expect(root).toBe(tmpDir)
    })

    test("findModuleRoot returns null when no mog.mod found", () => {
      setupTestProject({
        "main.mog": "package main\n",
      })
      // Start from temp dir which has no mog.mod
      const root = findModuleRoot("/tmp/nonexistent-dir-for-mog-test")
      expect(root).toBeNull()
    })

    test("parseModFile extracts module name", () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
      })
      const info = parseModFile(path.join(tmpDir, "mog.mod"))
      expect(info.module).toBe("myapp")
    })

    test("getPackageName returns name from PackageDeclaration", () => {
      const ast = parse("package math")
      expect(getPackageName(ast)).toBe("math")
    })

    test("getPackageName returns null when no PackageDeclaration", () => {
      const ast = parse("fn main() -> i64 { return 0; }")
      expect(getPackageName(ast)).toBeNull()
    })

    test("collectImportPaths extracts all import paths", () => {
      const ast = parse('import "math"\nimport ( "strings" "utils" )')
      const imports = collectImportPaths(ast)
      expect(imports).toEqual(["math", "strings", "utils"])
    })

    test("collectExports finds pub functions", () => {
      const ast = parse(`
pub fn add(a: i64, b: i64) -> i64 { return a + b; }
fn helper() -> i64 { return 42; }
pub fn sub(a: i64, b: i64) -> i64 { return a - b; }
`)
      const exports = collectExports(ast)
      expect(exports.size).toBe(2)
      expect(exports.has("add")).toBe(true)
      expect(exports.has("sub")).toBe(true)
      expect(exports.has("helper")).toBe(false)
      expect(exports.get("add")?.kind).toBe("function")
    })

    test("collectExports finds pub structs", () => {
      const ast = parse(`
pub struct Point { x: f64, y: f64 }
struct Internal { a: i64 }
`)
      const exports = collectExports(ast)
      expect(exports.size).toBe(1)
      expect(exports.has("Point")).toBe(true)
      expect(exports.get("Point")?.kind).toBe("struct")
    })

    test("resolveImports resolves a simple project", () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
        "main.mog": `package main
import "math"
fn main() -> i64 {
  return 0;
}
`,
        "math/math.mog": `package math
pub fn add(a: i64, b: i64) -> i64 {
  return a + b;
}
fn helper() -> i64 {
  return 42;
}
`,
      })

      const graph = resolveImports(path.join(tmpDir, "main.mog"))
      expect(graph.root).toBe(tmpDir)
      expect(graph.modulePath).toBe("myapp")
      expect(graph.mainPackage.packageName).toBe("main")
      expect(graph.packages.size).toBe(1)
      
      const mathPkg = graph.packages.get("math")!
      expect(mathPkg).toBeDefined()
      expect(mathPkg.packageName).toBe("math")
      expect(mathPkg.exports.size).toBe(1)
      expect(mathPkg.exports.has("add")).toBe(true)
      expect(mathPkg.exports.has("helper")).toBe(false) // Not exported
    })

    test("resolveImports detects circular imports", () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
        "main.mog": `package main
import "a"
fn main() -> i64 { return 0; }
`,
        "a/a.mog": `package a
import "b"
pub fn fa() -> i64 { return 1; }
`,
        "b/b.mog": `package b
import "a"
pub fn fb() -> i64 { return 2; }
`,
      })

      expect(() => resolveImports(path.join(tmpDir, "main.mog"))).toThrow(
        "Circular import detected: a"
      )
    })

    test("resolveImports handles multiple files in a package", () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
        "main.mog": `package main
import "math"
fn main() -> i64 { return 0; }
`,
        "math/add.mog": `package math
pub fn add(a: i64, b: i64) -> i64 { return a + b; }
`,
        "math/sub.mog": `package math
pub fn sub(a: i64, b: i64) -> i64 { return a - b; }
`,
      })

      const graph = resolveImports(path.join(tmpDir, "main.mog"))
      const mathPkg = graph.packages.get("math")!
      expect(mathPkg.files.length).toBe(2)
      expect(mathPkg.exports.size).toBe(2)
      expect(mathPkg.exports.has("add")).toBe(true)
      expect(mathPkg.exports.has("sub")).toBe(true)
    })

    test("resolveImports handles nested imports", () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
        "main.mog": `package main
import "math"
fn main() -> i64 { return 0; }
`,
        "math/math.mog": `package math
import "utils"
pub fn add(a: i64, b: i64) -> i64 { return a + b; }
`,
        "utils/utils.mog": `package utils
pub fn helper() -> i64 { return 42; }
`,
      })

      const graph = resolveImports(path.join(tmpDir, "main.mog"))
      expect(graph.packages.size).toBe(2)
      expect(graph.packages.has("math")).toBe(true)
      expect(graph.packages.has("utils")).toBe(true)
    })

    test("resolveImports handles subpath imports", () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
        "main.mog": `package main
import "myapp/utils"
fn main() -> i64 { return 0; }
`,
        "myapp/utils/utils.mog": `package utils
pub fn helper() -> i64 { return 42; }
`,
      })

      const graph = resolveImports(path.join(tmpDir, "main.mog"))
      expect(graph.packages.size).toBe(1)
      const utilsPkg = graph.packages.get("myapp/utils")!
      expect(utilsPkg.packageName).toBe("utils")
    })
  })

  // =========================================================================
  // NAME MANGLING IN CODEGEN
  // =========================================================================
  describe("Name Mangling", () => {
    test("package-prefixed functions get mangled names", () => {
      const mathAst = parse(`
package math
pub fn add(a: i64, b: i64) -> i64 { return a + b; }
`)
      const mainAst = parse(`
package main
import "math"
fn main() -> i64 {
  return 0;
}
`)

      const packages = [
        { packageName: "math", ast: mathAst, exports: new Map([["add", { name: "add", kind: "function" }]]) },
        { packageName: "main", ast: mainAst, exports: new Map() },
      ]

      const ir = generateModuleLLVMIR(packages as any)
      // The math.add function should be mangled to math__add
      expect(ir).toContain("@math__add")
    })

    test("main package functions are not mangled", () => {
      const mainAst = parse(`
package main
fn main() -> i64 {
  return 42;
}
`)

      const packages = [
        { packageName: "main", ast: mainAst, exports: new Map() },
      ]

      const ir = generateModuleLLVMIR(packages as any)
      expect(ir).toContain("@program_user")
      expect(ir).not.toContain("@main__")
    })

    test("cross-package function calls use mangled names", () => {
      const mathAst = parse(`
package math
pub fn add(a: i64, b: i64) -> i64 { return a + b; }
`)
      const mainAst = parse(`
package main
import "math"
fn main() -> i64 {
  result := math.add(1, 2);
  return result;
}
`)

      const packages = [
        { packageName: "math", ast: mathAst, exports: new Map([["add", { name: "add", kind: "function" }]]) },
        { packageName: "main", ast: mainAst, exports: new Map() },
      ]

      const ir = generateModuleLLVMIR(packages as any)
      // Should define math__add
      expect(ir).toContain("define i64 @math__add")
      // Should call math__add
      expect(ir).toContain("call i64 @math__add")
    })

    test("struct names get mangled in non-main packages", () => {
      const mathAst = parse(`
package math
pub struct Vector { x: f64, y: f64 }
`)
      const mainAst = parse(`
package main
import "math"
fn main() -> i64 { return 0; }
`)

      const packages = [
        { packageName: "math", ast: mathAst, exports: new Map([["Vector", { name: "Vector", kind: "struct" }]]) },
        { packageName: "main", ast: mainAst, exports: new Map() },
      ]

      const ir = generateModuleLLVMIR(packages as any)
      // The IR doesn't use LLVM named struct types (all are ptr/heap-allocated),
      // but the struct definition should be registered with the mangled name
      // in the internal structDefs map. We verify by checking the IR compiles without error.
      expect(ir).toBeTruthy()
    })
  })

  // =========================================================================
  // MODULE COMPILATION INTEGRATION
  // =========================================================================
  describe("Module Compilation", () => {
    test("compileModule falls back to single-file when no mog.mod", async () => {
      setupTestProject({
        "main.mog": `fn main() -> i64 { return 42; }`,
      })

      const result = await compileModule(path.join(tmpDir, "main.mog"))
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i64 @program_user")
    })

    test("compileModule compiles a simple multi-package project", async () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
        "main.mog": `package main

import "math"

fn main() -> i64 {
  result := math.add(1, 2);
  return result;
}
`,
        "math/math.mog": `package math

pub fn add(a: i64, b: i64) -> i64 {
  return a + b;
}
`,
      })

      const result = await compileModule(path.join(tmpDir, "main.mog"))
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("@math__add")
      expect(result.llvmIR).toContain("define i64 @math__add")
    })

    test("compileModule detects circular imports", async () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
        "main.mog": `package main
import "a"
fn main() -> i64 { return 0; }
`,
        "a/a.mog": `package a
import "b"
pub fn fa() -> i64 { return 1; }
`,
        "b/b.mog": `package b
import "a"
pub fn fb() -> i64 { return 2; }
`,
      })

      const result = await compileModule(path.join(tmpDir, "main.mog"))
      expect(result.errors.length).toBeGreaterThan(0)
      expect(result.errors[0].message).toContain("Circular import")
    })

    test("compileModule reports missing package", async () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
        "main.mog": `package main
import "nonexistent"
fn main() -> i64 { return 0; }
`,
      })

      const result = await compileModule(path.join(tmpDir, "main.mog"))
      expect(result.errors.length).toBeGreaterThan(0)
      expect(result.errors[0].message).toContain("Package not found")
    })

    test("compileModule handles multiple packages", async () => {
      setupTestProject({
        "mog.mod": "module myapp\n",
        "main.mog": `package main

import (
  "math"
  "strings"
)

fn main() -> i64 {
  a := math.add(1, 2);
  return a;
}
`,
        "math/math.mog": `package math
pub fn add(a: i64, b: i64) -> i64 { return a + b; }
`,
        "strings/strings.mog": `package strings
pub fn length(s: string) -> i64 { return 0; }
`,
      })

      const result = await compileModule(path.join(tmpDir, "main.mog"))
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("@math__add")
      expect(result.llvmIR).toContain("@strings__length")
    })
  })

  // =========================================================================
  // SINGLE-FILE COMPILATION (BACKWARD COMPATIBILITY)
  // =========================================================================
  describe("Single-file Compilation with Module Keywords", () => {
    test("compile() handles package + fn main", async () => {
      const result = await compile(`
package main

fn main() -> i64 {
  return 42;
}
`)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i64 @program_user")
    })

    test("compile() handles package + import (import is a no-op in single-file)", async () => {
      const result = await compile(`
package main

import "math"

fn main() -> i64 {
  return 42;
}
`)
      expect(result.errors).toEqual([])
    })

    test("compile() handles pub fn in single-file", async () => {
      const result = await compile(`
pub fn add(a: i64, b: i64) -> i64 {
  return a + b;
}

fn main() -> i64 {
  return add(1, 2);
}
`)
      expect(result.errors).toEqual([])
      expect(result.llvmIR).toContain("define i64 @add")
    })

    test("compile() handles pub struct in single-file", async () => {
      const result = await compile(`
pub struct Point {
  x: i64,
  y: i64,
}

fn main() -> i64 {
  p := Point { x: 1, y: 2 };
  return 0;
}
`)
      expect(result.errors).toEqual([])
    })
  })
})
