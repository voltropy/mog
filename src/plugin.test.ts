import { describe, test, expect } from "bun:test"
import { compilePlugin } from "./compiler"
import { generateLLVMIR } from "./llvm_codegen"
import { generateQBEIR } from "./qbe_codegen"
import { tokenize } from "./lexer"
import { parseTokens } from "./parser"
import { SemanticAnalyzer } from "./analyzer"

describe("Plugin compilation", () => {
  // Helper to compile plugin source and get IR
  async function getPluginIR(source: string, name = "test_plugin") {
    const result = await compilePlugin(source, name, "1.0.0")
    return result
  }

  test("generates mog_plugin_info function", async () => {
    const result = await getPluginIR(
      `
      pub fn add(a: int, b: int) -> int {
        return a + b;
      }
    `,
      "math",
    )
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).toContain("define")
    expect(result.llvmIR).toContain("mog_plugin_info")
    expect(result.llvmIR).toContain("math")
  })

  test("generates mog_plugin_init function", async () => {
    const result = await getPluginIR(`
      pub fn greet() -> int {
        return 42;
      }
    `)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).toContain("mog_plugin_init")
    expect(result.llvmIR).toContain("gc_init")
    expect(result.llvmIR).toContain("mog_vm_set_global")
  })

  test("generates mog_plugin_exports function", async () => {
    const result = await getPluginIR(`
      pub fn foo() -> int { return 1; }
      pub fn bar(x: int) -> int { return x; }
    `)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).toContain("mog_plugin_exports")
    expect(result.llvmIR).toContain("mogp_foo")
    expect(result.llvmIR).toContain("mogp_bar")
  })

  test("does not generate @main in plugin mode", async () => {
    const result = await getPluginIR(`
      pub fn add(a: int, b: int) -> int {
        return a + b;
      }
    `)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).not.toContain("define i32 @main")
  })

  test("non-pub functions get internal linkage", async () => {
    const result = await getPluginIR(`
      fn helper(x: int) -> int { return x * x; }
      pub fn compute(a: int) -> int {
        return helper(a);
      }
    `)
    expect(result.errors).toHaveLength(0)
    // helper should be internal, compute should not
    expect(result.llvmIR).toContain("internal")
    expect(result.llvmIR).toContain("mogp_compute")
  })

  test("required capabilities listed in plugin info", async () => {
    const result = await getPluginIR(`
      requires process;
      pub fn get_time() -> int {
        return 0;
      }
    `)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).toContain("process")
    expect(result.llvmIR).toContain("mog_plugin_info")
  })

  test("empty plugin with no exports", async () => {
    const result = await getPluginIR(`
      fn internal_only() -> int { return 1; }
    `)
    expect(result.errors).toHaveLength(0)
    expect(result.llvmIR).toContain("mog_plugin_info")
    expect(result.llvmIR).toContain("mog_plugin_init")
    expect(result.llvmIR).toContain("mog_plugin_exports")
  })

  test("plugin with top-level init code", async () => {
    const result = await getPluginIR(`
      println("plugin loaded");
      pub fn value() -> int { return 42; }
    `)
    expect(result.errors).toHaveLength(0)
    // Top-level code goes in @program
    expect(result.llvmIR).toContain("program")
    // mog_plugin_init should call @program
    expect(result.llvmIR).toContain("mog_plugin_init")
  })
})

describe("QBE Plugin compilation", () => {
  // Helper to parse source into AST for QBE tests
  function parseSource(source: string) {
    const tokens = tokenize(source)
    const filtered = tokens.filter(
      (t) => t.type !== "WHITESPACE" && t.type !== "COMMENT",
    )
    const ast = parseTokens(filtered)
    const analyzer = new SemanticAnalyzer()
    analyzer.analyze(ast)
    return ast
  }

  test("generates plugin protocol functions in QBE IL", () => {
    const ast = parseSource(`
      pub fn add(a: int, b: int) -> int {
        return a + b;
      }
    `)

    const ir = generateQBEIR(ast, undefined, undefined, undefined, {
      pluginMode: true,
      pluginName: "test",
      pluginVersion: "1.0.0",
    })

    expect(ir).toContain("$mog_plugin_info")
    expect(ir).toContain("$mog_plugin_init")
    expect(ir).toContain("$mog_plugin_exports")
    expect(ir).toContain("$mogp_add")
  })

  test("QBE plugin does not generate main entry", () => {
    const ast = parseSource(`
      pub fn double_it(x: int) -> int {
        return x + x;
      }
    `)

    const ir = generateQBEIR(ast, undefined, undefined, undefined, {
      pluginMode: true,
      pluginName: "math",
    })

    // Should not have main entry point
    expect(ir).not.toContain("function w $main")
    // Should have plugin init
    expect(ir).toContain("$mog_plugin_init")
  })
})

describe("LLVM Plugin IR generation (direct)", () => {
  // Helper to parse source into AST for direct LLVM tests
  function parseSource(source: string) {
    const tokens = tokenize(source)
    const filtered = tokens.filter(
      (t) => t.type !== "WHITESPACE" && t.type !== "COMMENT",
    )
    const ast = parseTokens(filtered)
    const analyzer = new SemanticAnalyzer()
    analyzer.analyze(ast)
    return ast
  }

  test("generateLLVMIR with pluginOptions produces plugin functions", () => {
    const ast = parseSource(`
      pub fn multiply(a: int, b: int) -> int {
        return a * b;
      }
    `)

    const ir = generateLLVMIR(ast, undefined, undefined, undefined, {
      pluginMode: true,
      pluginName: "arithmetic",
      pluginVersion: "2.0.0",
    })

    expect(ir).toContain("mog_plugin_info")
    expect(ir).toContain("mog_plugin_init")
    expect(ir).toContain("mog_plugin_exports")
    expect(ir).toContain("mogp_multiply")
    expect(ir).toContain("arithmetic")
    expect(ir).not.toContain("define i32 @main")
  })

  test("generateLLVMIR without pluginOptions generates normal main", () => {
    const ast = parseSource(`
      fn add(a: int, b: int) -> int {
        return a + b;
      }
    `)

    const ir = generateLLVMIR(ast)

    expect(ir).toContain("@main")
    expect(ir).not.toContain("mog_plugin_info")
    expect(ir).not.toContain("mog_plugin_exports")
  })
})
