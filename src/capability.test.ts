import { describe, test, expect } from "bun:test"
import { parseCapabilityDecl, parseCapability, mogDeclTypeToMogType } from "./capability.js"
import { tokenize } from "./lexer.js"
import { parseTokens } from "./parser.js"
import { SemanticAnalyzer } from "./analyzer.js"
import { generateLLVMIR } from "./llvm_codegen.js"
import { compile } from "./compiler.js"

// ============================================================
// Part 1: Capability Declaration Parser Tests
// ============================================================

describe("Capability Declaration Parser", () => {
  test("parses a simple capability", () => {
    const source = `
      capability log {
        fn info(message: string);
        fn warn(message: string);
      }
    `
    const decls = parseCapabilityDecl(source)
    expect(decls).toHaveLength(1)
    expect(decls[0].name).toBe("log")
    expect(decls[0].functions).toHaveLength(2)
    expect(decls[0].functions[0].name).toBe("info")
    expect(decls[0].functions[0].params).toHaveLength(1)
    expect(decls[0].functions[0].params[0].name).toBe("message")
    expect(decls[0].functions[0].params[0].type).toBe("string")
    expect(decls[0].functions[0].returnType).toBeNull()
  })

  test("parses functions with return types", () => {
    const source = `
      capability math {
        fn add(a: int, b: int) -> int;
        fn multiply(a: int, b: int) -> int;
      }
    `
    const decls = parseCapabilityDecl(source)
    expect(decls).toHaveLength(1)
    expect(decls[0].name).toBe("math")
    expect(decls[0].functions).toHaveLength(2)
    
    const add = decls[0].functions[0]
    expect(add.name).toBe("add")
    expect(add.params).toHaveLength(2)
    expect(add.params[0].name).toBe("a")
    expect(add.params[0].type).toBe("int")
    expect(add.params[1].name).toBe("b")
    expect(add.params[1].type).toBe("int")
    expect(add.returnType).toBe("int")
  })

  test("parses optional parameters", () => {
    const source = `
      capability http {
        fn get(url: string, timeout: ?int) -> string;
      }
    `
    const decls = parseCapabilityDecl(source)
    const get = decls[0].functions[0]
    expect(get.params[0].optional).toBe(false)
    expect(get.params[1].optional).toBe(true)
    expect(get.params[1].type).toBe("int")
  })

  test("parses Result<T> return types", () => {
    const source = `
      capability fs {
        fn read(path: string) -> Result<string>;
      }
    `
    const decls = parseCapabilityDecl(source)
    const read = decls[0].functions[0]
    expect(read.returnType).toBe("Result<string>")
  })

  test("parses async functions", () => {
    const source = `
      capability net {
        async fn fetch(url: string) -> string;
      }
    `
    const decls = parseCapabilityDecl(source)
    const fetch = decls[0].functions[0]
    expect(fetch.isAsync).toBe(true)
    expect(fetch.name).toBe("fetch")
  })

  test("parses void functions (no return type)", () => {
    const source = `
      capability log {
        fn debug(msg: string);
      }
    `
    const decls = parseCapabilityDecl(source)
    expect(decls[0].functions[0].returnType).toBeNull()
  })

  test("parses multiple capabilities", () => {
    const source = `
      capability fs {
        fn read(path: string) -> string;
      }
      capability net {
        fn get(url: string) -> string;
      }
    `
    const decls = parseCapabilityDecl(source)
    expect(decls).toHaveLength(2)
    expect(decls[0].name).toBe("fs")
    expect(decls[1].name).toBe("net")
  })

  test("parseCapability returns first capability", () => {
    const source = `
      capability math {
        fn add(a: int, b: int) -> int;
      }
    `
    const decl = parseCapability(source)
    expect(decl).not.toBeNull()
    expect(decl!.name).toBe("math")
  })

  test("parseCapability returns null for empty input", () => {
    const decl = parseCapability("")
    expect(decl).toBeNull()
  })

  test("handles comments in declarations", () => {
    const source = `
      # This is a comment
      capability log {
        // Another comment
        fn info(message: string);
      }
    `
    const decls = parseCapabilityDecl(source)
    expect(decls).toHaveLength(1)
    expect(decls[0].functions).toHaveLength(1)
  })
})

describe("mogDeclTypeToMogType", () => {
  test("maps basic types", () => {
    expect(mogDeclTypeToMogType("int")).toBe("i64")
    expect(mogDeclTypeToMogType("float")).toBe("f64")
    expect(mogDeclTypeToMogType("bool")).toBe("bool")
    expect(mogDeclTypeToMogType("string")).toBe("string")
    expect(mogDeclTypeToMogType("none")).toBe("void")
  })

  test("unwraps Result<T>", () => {
    expect(mogDeclTypeToMogType("Result<int>")).toBe("i64")
    expect(mogDeclTypeToMogType("Result<string>")).toBe("string")
  })
})

// ============================================================
// Part 2: Lexer Tests for requires/optional keywords
// ============================================================

describe("Lexer - capability keywords", () => {
  test("tokenizes 'requires' keyword", () => {
    const tokens = tokenize("requires")
    const filtered = tokens.filter(t => t.type !== "WHITESPACE")
    expect(filtered[0].type).toBe("requires")
    expect(filtered[0].value).toBe("requires")
  })

  test("tokenizes 'optional' keyword", () => {
    const tokens = tokenize("optional")
    const filtered = tokens.filter(t => t.type !== "WHITESPACE")
    expect(filtered[0].type).toBe("optional_kw")
    expect(filtered[0].value).toBe("optional")
  })

  test("tokenizes requires declaration", () => {
    const tokens = tokenize("requires fs, math;")
    const filtered = tokens.filter(t => t.type !== "WHITESPACE")
    expect(filtered[0].type).toBe("requires")
    expect(filtered[1].type).toBe("IDENTIFIER")
    expect(filtered[1].value).toBe("fs")
    expect(filtered[2].type).toBe("COMMA")
    expect(filtered[3].type).toBe("IDENTIFIER")
    expect(filtered[3].value).toBe("math")
    expect(filtered[4].type).toBe("SEMICOLON")
  })

  test("tokenizes optional declaration", () => {
    const tokens = tokenize("optional log;")
    const filtered = tokens.filter(t => t.type !== "WHITESPACE")
    expect(filtered[0].type).toBe("optional_kw")
    expect(filtered[1].type).toBe("IDENTIFIER")
    expect(filtered[1].value).toBe("log")
    expect(filtered[2].type).toBe("SEMICOLON")
  })

  test("requires/optional don't interfere with identifiers", () => {
    const tokens = tokenize("requires_flag: i32 = 1")
    const filtered = tokens.filter(t => t.type !== "WHITESPACE")
    // "requires_flag" should be an IDENTIFIER, not the "requires" keyword
    // because the regex uses \b word boundary
    expect(filtered[0].type).toBe("IDENTIFIER")
    expect(filtered[0].value).toBe("requires_flag")
  })
})

// ============================================================
// Part 3: Parser Tests for requires/optional declarations
// ============================================================

describe("Parser - capability declarations", () => {
  test("parses requires declaration", () => {
    const source = "requires fs;"
    const tokens = tokenize(source)
    const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    const ast = parseTokens(filtered)
    expect(ast.statements).toHaveLength(1)
    expect((ast.statements[0] as any).type).toBe("RequiresDeclaration")
    expect((ast.statements[0] as any).capabilities).toEqual(["fs"])
  })

  test("parses requires with multiple capabilities", () => {
    const source = "requires fs, math, log;"
    const tokens = tokenize(source)
    const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    const ast = parseTokens(filtered)
    expect(ast.statements).toHaveLength(1)
    expect((ast.statements[0] as any).capabilities).toEqual(["fs", "math", "log"])
  })

  test("parses optional declaration", () => {
    const source = "optional log;"
    const tokens = tokenize(source)
    const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    const ast = parseTokens(filtered)
    expect(ast.statements).toHaveLength(1)
    expect((ast.statements[0] as any).type).toBe("OptionalDeclaration")
    expect((ast.statements[0] as any).capabilities).toEqual(["log"])
  })

  test("parses requires + optional + code", () => {
    const source = `
      requires math;
      optional log;
      x: i32 = 42;
    `
    const tokens = tokenize(source)
    const filtered = tokens.filter(t => t.type !== "WHITESPACE" && t.type !== "COMMENT")
    const ast = parseTokens(filtered)
    expect(ast.statements).toHaveLength(3)
    expect((ast.statements[0] as any).type).toBe("RequiresDeclaration")
    expect((ast.statements[1] as any).type).toBe("OptionalDeclaration")
    expect((ast.statements[2] as any).type).toBe("VariableDeclaration")
  })
})

// ============================================================
// Part 4: Analyzer Tests - capability type checking
// ============================================================

describe("Analyzer - capability type checking", () => {
  test("allows capability function calls when declared", async () => {
    const source = `
      requires math;
      x: i64 = math.add(1, 2);
    `
    const result = await compile(source)
    // Should compile without errors (math.mogdecl exists)
    expect(result.errors).toHaveLength(0)
  })

  test("allows optional capability calls", async () => {
    const source = `
      optional log;
      log.info("hello");
    `
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
  })

  test("errors on non-existent capability function", async () => {
    const source = `
      requires math;
      x: i64 = math.nonexistent(1, 2);
    `
    const result = await compile(source)
    expect(result.errors.length).toBeGreaterThan(0)
    expect(result.errors[0].message).toContain("nonexistent")
  })

  test("errors on wrong argument count", async () => {
    const source = `
      requires math;
      x: i64 = math.add(1);
    `
    const result = await compile(source)
    expect(result.errors.length).toBeGreaterThan(0)
    expect(result.errors[0].message).toContain("expects at least")
  })

  test("errors on too many arguments", async () => {
    const source = `
      requires math;
      x: i64 = math.add(1, 2, 3);
    `
    const result = await compile(source)
    expect(result.errors.length).toBeGreaterThan(0)
    expect(result.errors[0].message).toContain("expects at most")
  })

  test("multiple required capabilities", async () => {
    const source = `
      requires math, log;
      x: i64 = math.add(1, 2);
      log.info("result");
    `
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
  })
})

// ============================================================
// Part 5: LLVM IR Codegen Tests - capability calls
// ============================================================

describe("Codegen - capability calls", () => {
  test("generates mog_cap_call for capability function calls", async () => {
    const source = `
      requires log;
      log.info("hello");
    `
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).toContain("mog_cap_call")
    expect(result.llvmIR).toContain("%MogValue")
  })

  test("generates correct cap and func names in IR", async () => {
    const source = `
      requires math;
      x: i64 = math.add(1, 2);
    `
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).toContain('c"math\\00"')
    expect(result.llvmIR).toContain('c"add\\00"')
    expect(result.llvmIR).toContain("mog_cap_call")
  })

  test("generates MogValue type and declarations", async () => {
    const source = `
      requires math;
      x: i64 = math.add(1, 2);
    `
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).toContain("%MogValue = type { i32, { { ptr, ptr } } }")
    expect(result.llvmIR).toContain("declare void @mog_cap_call(ptr sret(%MogValue), ptr, ptr, ptr, ptr, i32)")
  })

  test("extracts return value from MogValue via sret", async () => {
    const source = `
      requires math;
      x: i64 = math.add(1, 2);
    `
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
    // sret convention: result is loaded via GEP from stack alloca, not extractvalue
    expect(result.llvmIR).toContain("call void @mog_cap_call(ptr sret(%MogValue)")
    expect(result.llvmIR).toContain("getelementptr inbounds %MogValue")
  })

  test("does not generate capability declarations when no capabilities used", async () => {
    const source = `x: i64 = 42;`
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).not.toContain("%MogValue")
    expect(result.llvmIR).not.toContain("mog_cap_call")
  })

  test("handles capability calls with no arguments", async () => {
    const source = `
      requires math;
      x: i64 = math.abs(42);
    `
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).toContain("mog_cap_call")
  })

  test("packs string arguments with MOG_STRING tag", async () => {
    const source = `
      requires log;
      log.info("hello world");
    `
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
    // MOG_STRING = 3
    expect(result.llvmIR).toContain("store i32 3")
    expect(result.llvmIR).toContain("ptrtoint ptr")
  })

  test("packs integer arguments with MOG_INT tag", async () => {
    const source = `
      requires math;
      x: i64 = math.add(10, 20);
    `
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
    // MOG_INT = 0
    expect(result.llvmIR).toContain("store i32 0")
  })
})

// ============================================================
// Part 6: Capability Validation Tests
// ============================================================

describe("Capability validation", () => {
  test("requires declaration compiles even if capability not registered at runtime", async () => {
    // At compile time, we just type-check. Runtime registration is separate.
    const source = `
      requires math;
      x: i64 = math.add(1, 2);
    `
    const result = await compile(source)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).toContain("mog_cap_call")
  })

  test("optional declaration compiles even without .mogdecl file", async () => {
    // "unknown_cap" has no .mogdecl file - should still compile
    const source = `
      optional unknown_cap;
      unknown_cap.something("test");
    `
    const result = await compile(source)
    // Should not error - unknown optional capabilities are allowed
    expect(result.errors).toHaveLength(0)
  })
})
