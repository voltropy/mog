import type { ProgramNode, StatementNode, ExpressionNode } from "./analyzer.js"
import { isArrayType, isMapType, isFloatType, isTensorType, IntegerType, UnsignedType, FloatType, ArrayType, MapType, StructType, SOAType, ResultType, OptionalType, FunctionType, TensorType, TypeAliasType } from "./types.js"

type LLVMType = "i8" | "i16" | "i32" | "i64" | "i128" | "i256" | "half" | "float" | "double" | "fp128" | "void" | "ptr"

type LLVMValue = {
  name: string
  type: LLVMType
  isConstant?: boolean
}

type LLVMFunction = {
  name: string
  returnType: LLVMType
  params: LLVMValue[]
  body: string[]
}

type LLVMGlobal = {
  name: string
  type: LLVMType
  initValue?: string
}

type OptimizationOptions = {
  /** Enable release mode optimizations (bounds check elision) */
  releaseMode: boolean
  /** Enable vectorization hints for array operations */
  vectorization: boolean
  /** Inline threshold: functions with fewer basic blocks will be marked alwaysinline */
  inlineThreshold: number
}

const defaultOptimizationOptions: OptimizationOptions = {
  releaseMode: process.env.MOG_RELEASE === "1",
  vectorization: true,
  inlineThreshold: 3,
}

class LLVMIRGenerator {
  private functions: Map<string, LLVMFunction> = new Map()
  private globals: LLVMGlobal[] = []
  private currentFunction: LLVMFunction | null = null
  private blockCounter = 0
  private valueCounter = 0
  private variableTypes: Map<string, any> = new Map()
  private functionTypes: Map<string, any> = new Map()
  private loopStack: { breakLabel: string; continueLabel: string }[] = []
  private tryStack: { catchLabel: string; errorVar: string }[] = []
  private blockTerminated = false
  private opts: OptimizationOptions
  private currentFunctionBasicBlocks = 0
  // Plugin compilation mode
  private pluginMode = false
  private pluginName = "plugin"
  private pluginVersion = "0.1.0"
  // Track pub function declarations for plugin exports
  private pubFunctions: { name: string; llvmName: string; params: any[]; returnType: any }[] = []

  constructor(options: Partial<OptimizationOptions> = {}) {
    this.opts = { ...defaultOptimizationOptions, ...options }
  }

  setPluginMode(name: string, version: string): void {
    this.pluginMode = true
    this.pluginName = name
    this.pluginVersion = version
  }

  setCapabilities(caps: string[]): void {
    for (const cap of caps) {
      this.capabilities.add(cap)
    }
  }

  setCapabilityDecls(decls: Map<string, any>): void {
    this.capabilityDecls = decls
  }

  /** Check if a capability function is declared as async in its .mogdecl */
  private isAsyncCapabilityFunc(capName: string, funcName: string): boolean {
    const decl = this.capabilityDecls.get(capName)
    if (!decl) return false
    const func = decl.functions?.find((f: any) => f.name === funcName)
    return func?.isAsync === true
  }

  private resetValueCounter(start: number = 0): void {
    this.valueCounter = start
  }
  private labelCounter = 0
  private stringConstants: string[] = []
  private stringCounter = 0
  private stringNameMap: Map<string, string> = new Map()
  private stringByteLengths: Map<string, number> = new Map() // JS string value -> UTF-8 byte length
  private lambdaCounter = 0
  private lambdaIR: string[] = []
  // Closure support: wrappers for named functions used as values
  private functionWrappers: Set<string> = new Set()
  private wrapperIR: string[] = []
  // Track function parameter info for named arg / default arg resolution
  private functionParamInfo: Map<string, { name: string; paramType: any; defaultValue?: any }[]> = new Map()
  // Struct definitions: maps struct name -> ordered list of { name, type } fields
  private structDefs: Map<string, { name: string; fieldType: any }[]> = new Map()
  // Capability tracking for Host FFI
  private capabilities: Set<string> = new Set()
  private capabilityDecls: Map<string, any> = new Map() // CapabilityDecl objects from .mogdecl
  private capCallDeclared = false
  private capStringConstants: string[] = []
  // Async/coroutine state
  private isInAsyncFunction = false
  private coroHandle = ""
  private coroFuture = ""
  private coroId = ""
  private awaitCounter = 0
  // Track which functions are async (name -> true)
  private asyncFunctions: Set<string> = new Set()
  // Track whether the program has any async functions
  private hasAsyncFunctions = false
  // Runtime async builtins (C functions that return MogFuture* directly, not Mog coroutines).
  // These skip coro-resume since they have no coroutine handle — they manage their own suspension.
  private runtimeAsyncFunctions: Set<string> = new Set(["async_read_line"])
  // Package prefix for name mangling in multi-package builds
  private packagePrefix: string = ""
  // Map of import alias -> package prefix for resolving cross-package calls
  public importedPackages: Map<string, string> = new Map()
  private resetStringCounter(): void {
    this.stringCounter = 0
    this.stringNameMap.clear()
    this.stringByteLengths.clear()
  }

  /** Encode a JS string to LLVM IR escaped format with proper UTF-8 encoding.
   *  Returns { escaped: string, byteLength: number } */
  private escapeStringForLLVM(str: string): { escaped: string; byteLength: number } {
    let llvmEscaped = ""
    let byteLength = 0
    for (const char of str) {
      const code = char.codePointAt(0)!
      if (char === "\\") {
        llvmEscaped += "\\\\"
        byteLength += 1
      } else if (char === '"') {
        llvmEscaped += '\\"'
        byteLength += 1
      } else if (code === 10) {
        llvmEscaped += "\\0A"
        byteLength += 1
      } else if (code === 13) {
        llvmEscaped += "\\0D"
        byteLength += 1
      } else if (code === 9) {
        llvmEscaped += "\\09"
        byteLength += 1
      } else if (code >= 32 && code <= 126) {
        llvmEscaped += char
        byteLength += 1
      } else {
        // Encode to UTF-8 bytes and emit each as hex escape
        const bytes = new TextEncoder().encode(char)
        for (const b of bytes) {
          llvmEscaped += `\\${b.toString(16).padStart(2, "0").toUpperCase()}`
        }
        byteLength += bytes.length
      }
    }
    return { escaped: llvmEscaped, byteLength }
  }

  /** Get the UTF-8 byte length of a JS string (cached in stringByteLengths) */
  private getStringByteLength(str: string): number {
    const cached = this.stringByteLengths.get(str)
    if (cached !== undefined) return cached
    const byteLength = new TextEncoder().encode(str).length
    this.stringByteLengths.set(str, byteLength)
    return byteLength
  }

  generate(ast: ProgramNode): string {
    const ir: string[] = []

    ir.push("; Mog LLVM IR")
    ir.push("; Generated by Mog compiler")
    ir.push("")

    const platform = process.platform === "darwin" ? "aarch64-apple-darwin" :
                     process.platform === "win32" ? "x86_64-pc-windows-msvc" :
                     "x86_64-unknown-linux-gnu"
    ir.push(`target triple = "${platform}"`)
    ir.push("")

    this.setupDeclarations(ir)
    this.generatePrintDeclarations(ir)
    if (this.capabilities.size > 0) {
      this.generateCapabilityDeclarations(ir)
    }

    // Detect async functions in AST
    this.detectAsyncFunctions(ast)
    if (this.hasAsyncFunctions) {
      this.generateAsyncDeclarations(ir)
    }

    this.resetValueCounter(0)
    this.resetStringCounter()
    this.stringConstants = []  // Clear string constants

    // First: collect all string constants from AST
    this.collectStringConstants(ast)
    // Reset counter for code generation pass
    this.stringCounter = 0

    // Collect struct/SoA definitions from top-level statements
    for (const stmt of ast.statements) {
      if (stmt.type === "StructDeclaration") {
        const fields = (stmt as any).fields?.map((f: any) => ({
          name: f.name,
          fieldType: f.fieldType
        })) || []
        this.structDefs.set((stmt as any).name, fields)
      }
    }

    // Collect function declarations first to identify strings in functions
    const functionDeclarations = this.collectFunctionDeclarations(ast)

    // Check if user has defined a main() function
    const mainFunc = functionDeclarations.find((f: any) => f.name === "main")
    const hasMain = !!mainFunc

    // Insert string constants before function declarations
    ir.push("; String constants")
    for (const str of this.stringConstants) {
      ir.push(str)
    }
    ir.push("")

    ir.push("; Function declarations")
    // First pass: register all function types and signatures before generating code
    // This is critical because nested functions (e.g., make_adder inside main) need
    // to be callable before they're fully generated, since main is generated first.
    for (const funcDecl of functionDeclarations) {
      const funcName = funcDecl.name === "main" ? "program_user" : funcDecl.name
      this.functionTypes.set(funcDecl.name, funcDecl.returnType)
      this.functionParamInfo.set(funcName, funcDecl.params)
      // Pre-register function signature so call sites can type arguments correctly
      const llvmParams = (funcDecl.params || []).map((p: any) => ({
        name: p.name,
        type: this.toLLVMType(p.paramType)
      }))
      // Async functions return ptr (MogFuture*) instead of their declared return type
      const isAsync = funcDecl.type === "AsyncFunctionDeclaration" || this.asyncFunctions.has(funcDecl.name)
      const llvmReturnType = isAsync ? "ptr" as LLVMType : this.toLLVMType(funcDecl.returnType)
      this.functions.set(funcName, {
        name: funcName,
        returnType: llvmReturnType,
        params: llvmParams,
        body: [],
      })
    }
    for (const funcDecl of functionDeclarations) {
      // Rename user's main() to @program_user to avoid conflict
      if (funcDecl.name === "main") {
        funcDecl.name = "program_user"
      }
      this.generateFunctionDeclaration(ir, funcDecl)
    }
    ir.push("")

    // Emit lifted lambda functions
    if (this.lambdaIR.length > 0) {
      ir.push("; Lambda functions")
      ir.push(...this.lambdaIR)
      ir.push("")
    }

    // Emit function wrappers (for named functions used as values)
    if (this.wrapperIR.length > 0) {
      ir.push("; Function wrappers for closure compatibility")
      ir.push(...this.wrapperIR)
      ir.push("")
    }

    // Determine if main is async
    const mainIsAsync = mainFunc && (mainFunc.type === "AsyncFunctionDeclaration" || this.asyncFunctions.has("main"))

    if (this.pluginMode) {
      // Plugin protocol always needs mog_vm_set_global, even without capabilities
      if (this.capabilities.size === 0) {
        ir.push('declare void @mog_vm_set_global(ptr)')
      }

      // Plugin mode: generate @program for top-level init code, then plugin protocol functions
      this.resetValueCounter(10)

      ir.push("define internal void @program() {")
      ir.push("entry:")

      this.blockTerminated = false
      for (const statement of ast.statements) {
        this.generateStatement(ir, statement)
        if (this.blockTerminated) break
      }

      if (!this.blockTerminated) {
        ir.push("  ret void")
      }
      ir.push("}")

      // Collect pub functions from declarations for export
      for (const funcDecl of functionDeclarations) {
        if (funcDecl.isPublic) {
          const originalName = funcDecl.name === "program_user" ? "main" : funcDecl.name
          this.pubFunctions.push({
            name: originalName,
            llvmName: funcDecl.name,
            params: funcDecl.params || [],
            returnType: funcDecl.returnType,
          })
        }
      }

      this.generatePluginInfo(ir)
      this.generatePluginInit(ir)
      this.generatePluginExports(ir)
    } else if (hasMain) {
      // User defined main() - generate wrapper at @main
      ir.push("; Main entry point (calls user's main)")
      
      if (mainIsAsync) {
        // Async main - set up event loop, call program_user (returns MogFuture*), run loop
        ir.push("define i32 @main(i32 %argc, ptr %argv) {")
        ir.push("entry:")
        ir.push("  call void @gc_init()")
        ir.push("  call void @gc_push_frame()")
        if (this.capabilities.size > 0) {
          ir.push("")
          ir.push("  ; Initialize VM with capabilities (reuse host VM if available)")
          ir.push("  %existing_vm = call ptr @mog_vm_get_global()")
          ir.push("  %vm_is_null = icmp eq ptr %existing_vm, null")
          ir.push("  br i1 %vm_is_null, label %create_vm, label %have_vm")
          ir.push("")
          ir.push("create_vm:")
          ir.push("  %new_vm = call ptr @mog_vm_new()")
          ir.push("  call void @mog_vm_set_global(ptr %new_vm)")
          ir.push("  call void @mog_register_posix_host(ptr %new_vm)")
          ir.push("  br label %have_vm")
          ir.push("")
          ir.push("have_vm:")
          ir.push("  %vm = phi ptr [ %existing_vm, %entry ], [ %new_vm, %create_vm ]")
        }
        ir.push("")
        ir.push("  ; Create and set global event loop")
        ir.push("  %loop = call ptr @mog_loop_new()")
        ir.push("  call void @mog_loop_set_global(ptr %loop)")
        ir.push("")
        ir.push("  ; Start the async main - runs eagerly, returns a future")
        ir.push("  %future = call ptr @program_user()")
        ir.push("")
        ir.push("  ; Run the event loop until everything completes")
        ir.push("  ; (For purely synchronous completions, this returns immediately)")
        ir.push("  call void @mog_loop_run(ptr %loop)")
        ir.push("")
        ir.push("  ; Get the result from main's future")
        ir.push("  %result_i64 = call i64 @mog_future_get_result(ptr %future)")
        ir.push("  %truncated = trunc i64 %result_i64 to i32")
        ir.push("")
        ir.push("  ; Cleanup")
        ir.push("  call void @mog_loop_free(ptr %loop)")
        if (this.capabilities.size > 0) {
          ir.push("  call void @mog_vm_free(ptr %vm)")
        }
        ir.push("  call void @gc_pop_frame()")
        ir.push("  ret i32 %truncated")
        ir.push("}")
      } else {
        // Synchronous main - existing behavior
        const hasParams = mainFunc && mainFunc.params.length >= 2
        
        if (hasParams) {
          ir.push("define i32 @main(i32 %argc, ptr %argv) {")
          ir.push("entry:")
          ir.push("  call void @gc_init()")
          ir.push("  call void @gc_push_frame()")
          if (this.capabilities.size > 0) {
            ir.push("  ; Reuse host VM if available, create new one otherwise")
            ir.push("  %existing_vm = call ptr @mog_vm_get_global()")
            ir.push("  %vm_is_null = icmp eq ptr %existing_vm, null")
            ir.push("  br i1 %vm_is_null, label %create_vm, label %have_vm")
            ir.push("create_vm:")
            ir.push("  %new_vm = call ptr @mog_vm_new()")
            ir.push("  call void @mog_vm_set_global(ptr %new_vm)")
            ir.push("  call void @mog_register_posix_host(ptr %new_vm)")
            ir.push("  br label %have_vm")
            ir.push("have_vm:")
            ir.push("  %vm = phi ptr [ %existing_vm, %entry ], [ %new_vm, %create_vm ]")
          }
          
          // Build array of length argc with argv pointers
          ir.push("  %args_array = call ptr @array_alloc(i64 8, i64 1, i64 %argc)")
          
          // Loop through argv and store pointers in array
          ir.push("  %args_array_data = getelementptr ptr, ptr %args_array, i64 0")
          
          // Process first 10 arguments (to keep code size reasonable)
          for (let i = 0; i < 10; i++) {
            ir.push(`  ; Process argv[${i}]`)
            ir.push(`  %argv_gep_${i} = getelementptr ptr, ptr %argv, i64 ${i}`)
            ir.push(`  %argv_ptr_${i} = load ptr, ptr %argv_gep_${i}`)
            ir.push(`  %in_bounds_${i} = icmp ult i64 ${i}, %argc`)
            ir.push(`  br i1 %in_bounds_${i}, label %store_argv_${i}, label %argv_done_${i}`)
            ir.push(`store_argv_${i}:`)
            ir.push(`  %array_gep_${i} = getelementptr i64, ptr %args_array_data, i64 ${i}`)
            ir.push(`  %argv_int_${i} = ptrtoint ptr %argv_ptr_${i} to i64`)
            ir.push(`  store i64 %argv_int_${i}, ptr %array_gep_${i}`)
            ir.push(`  br label %argv_done_${i}`)
            ir.push(`argv_done_${i}:`)
          }
          ir.push(`  ; Skip remaining argv elements (10..)`)
          
          // Create CLI args table (argc + args array)
          ir.push("  %cli_table = call ptr @map_new(i64 2)")
          ir.push("  %argc_key = alloca [5 x i8]")
          ir.push("  store [5 x i8] c\"argc\\00\", ptr %argc_key")
          ir.push("  call void @map_set(ptr %cli_table, ptr %argc_key, i64 4, i64 %argc)")
          
          ir.push("  %args_key = alloca [5 x i8]")
          ir.push("  store [5 x i8] c\"args\\00\", ptr %args_key")
          ir.push("  %args_int = ptrtoint ptr %args_array to i64")
          ir.push("  call void @map_set(ptr %cli_table, ptr %args_key, i64 4, i64 %args_int)")
          
          ir.push("  %cli_table_int = ptrtoint ptr %cli_table to i64")
          
          // Call user's main with CLI args table
          ir.push("  %result = call i64 @program_user(i64 %argc, i64 %cli_table_int)")
        } else {
          ir.push("define i32 @main(i32 %argc, ptr %argv) {")
          ir.push("entry:")
          ir.push("  call void @gc_init()")
          ir.push("  call void @gc_push_frame()")
          if (this.capabilities.size > 0) {
            ir.push("  ; Reuse host VM if available, create new one otherwise")
            ir.push("  %existing_vm = call ptr @mog_vm_get_global()")
            ir.push("  %vm_is_null = icmp eq ptr %existing_vm, null")
            ir.push("  br i1 %vm_is_null, label %create_vm, label %have_vm")
            ir.push("create_vm:")
            ir.push("  %new_vm = call ptr @mog_vm_new()")
            ir.push("  call void @mog_vm_set_global(ptr %new_vm)")
            ir.push("  call void @mog_register_posix_host(ptr %new_vm)")
            ir.push("  br label %have_vm")
            ir.push("have_vm:")
            ir.push("  %vm = phi ptr [ %existing_vm, %entry ], [ %new_vm, %create_vm ]")
          }
          ir.push("  %result = call i64 @program_user()")
        }
        
        ir.push("  %truncated = trunc i64 %result to i32")
        if (this.capabilities.size > 0) {
          ir.push("  call void @mog_vm_free(ptr %vm)")
        }
        ir.push("  call void @gc_pop_frame()")
        ir.push("  ret i32 %truncated")
        ir.push("}")
      }
    } else {
      // No main() - use program() entry point
      this.resetValueCounter(10)
      
      ir.push("define void @program() {")
      ir.push("entry:")

      this.blockTerminated = false
      for (const statement of ast.statements) {
        this.generateStatement(ir, statement)
        if (this.blockTerminated) break
      }

      if (!this.blockTerminated) {
        ir.push("  ret void")
      }
      ir.push("}")

      this.setupMainFunction(ir)
    }

    this.generateRuntimeFunctions(ir)
    this.generateMathDeclarations(ir)
    this.generatePOSIXDeclarations(ir)

    return ir.join("\n")
  }

  private collectFunctionDeclarations(ast: ProgramNode): any[] {
    const functions: any[] = []
    const declarations = this.findFunctionDeclarationsRecursive(ast.statements)
    functions.push(...declarations)
    return functions
  }

  private detectAsyncFunctions(ast: ProgramNode): void {
    const stmts = ast.statements || []
    this.detectAsyncInStatements(stmts)
  }

  private detectAsyncInStatements(stmts: any[]): void {
    for (const stmt of stmts) {
      if (stmt.type === "AsyncFunctionDeclaration") {
        this.asyncFunctions.add(stmt.name)
        this.hasAsyncFunctions = true
      }
      if (stmt.body?.statements) {
        this.detectAsyncInStatements(stmt.body.statements)
      }
      if (stmt.type === "Block" && stmt.statements) {
        this.detectAsyncInStatements(stmt.statements)
      }
    }
  }

  private findFunctionDeclarationsRecursive(statements: any[]): any[] {
    const functions: any[] = []
    for (const stmt of statements) {
      if (stmt.type === "FunctionDeclaration") {
        functions.push(stmt)
        // Recursively find nested function declarations in the function body
        if (stmt.body?.statements) {
          functions.push(...this.findFunctionDeclarationsRecursive(stmt.body.statements))
        }
      } else if (stmt.type === "AsyncFunctionDeclaration") {
        // TODO: For now, async functions are compiled as regular functions (synchronous fallback).
        // Future implementation will use coroutine-based state machine transformation.
        functions.push(stmt)
        if (stmt.body?.statements) {
          functions.push(...this.findFunctionDeclarationsRecursive(stmt.body.statements))
        }
      } else if (stmt.type === "Block") {
        functions.push(...this.findFunctionDeclarationsRecursive(stmt.statements))
      }
    }
    return functions
  }

  private setupDeclarations(ir: string[]): void {
    ir.push("; Declare GC functions")
    ir.push("declare void @gc_init()")
    ir.push("declare ptr @gc_alloc(i64)")
    ir.push("declare ptr @gc_alloc_closure(i64)")
    ir.push("declare void @gc_collect()")
    ir.push("declare void @gc_push_frame()")
    ir.push("declare void @gc_pop_frame()")
    ir.push("declare void @gc_benchmark_stats()")
    ir.push("declare void @gc_reset_stats()")
    ir.push("")

    ir.push("; Declare array functions")
    ir.push("declare ptr @array_alloc(i64 %element_size, i64 %dimension_count, ptr %dimensions)")
    ir.push("declare i64 @array_get(ptr %array, i64 %index)")
    ir.push("declare void @array_set(ptr %array, i64 %index, i64 %value)")
    ir.push("declare float @array_get_f32(ptr %array, i64 %index)")
    ir.push("declare void @array_set_f32(ptr %array, i64 %index, float %value)")
    ir.push("declare double @array_get_f64(ptr %array, i64 %index)")
    ir.push("declare void @array_set_f64(ptr %array, i64 %index, double %value)")
    ir.push("declare i64 @array_length(ptr %array)")
    ir.push("declare ptr @array_slice(ptr %array, i64 %start, i64 %end)")
    ir.push("declare ptr @array_slice_step(ptr %array, i64 %start, i64 %end, i64 %step)")
    ir.push("declare float @dot_f32(ptr %a, ptr %b)")
    ir.push("declare double @dot_f64(ptr %a, ptr %b)")
    ir.push("declare ptr @matmul(ptr %a, ptr %b)")
    ir.push("declare ptr @matrix_add(ptr %a, ptr %b)")
    ir.push("declare void @array_push(ptr %array, i64 %value)")
    ir.push("declare i64 @array_pop(ptr %array)")
    ir.push("declare i64 @array_contains(ptr %array, i64 %value)")
    ir.push("declare void @array_sort(ptr %array)")
    ir.push("declare void @array_sort_with_comparator(ptr %array, ptr %fn, i64 %env)")
    ir.push("declare void @array_reverse(ptr %array)")
    ir.push("declare ptr @array_join(ptr %array, ptr %separator)")
    ir.push("declare ptr @array_filter(ptr %array, ptr %fn, i64 %env)")
    ir.push("declare ptr @array_map(ptr %array, ptr %fn, i64 %env)")
    ir.push("")

    ir.push("; Declare map functions")
    ir.push("declare ptr @map_new(i64 %initial_capacity)")
    ir.push("declare i64 @map_get(ptr %map, ptr %key, i64 %key_len)")
    ir.push("declare void @map_set(ptr %map, ptr %key, i64 %key_len, i64 %value)")
    ir.push("declare i64 @map_has(ptr %map, ptr %key, i64 %key_len)")
    ir.push("declare i64 @map_size(ptr %map)")
    ir.push("declare ptr @map_key_at(ptr %map, i64 %index)")
    ir.push("declare i64 @map_value_at(ptr %map, i64 %index)")
    ir.push("")

    ir.push("; Declare string functions")
    ir.push("declare i64 @string_length(ptr %str)")
    ir.push("declare ptr @string_concat(ptr %a, ptr %b)")
    ir.push("declare ptr @string_char_at(ptr %str, i64 %index)")
    ir.push("declare ptr @string_slice(ptr %str, i64 %start, i64 %end)")
    ir.push("declare ptr @i64_to_string(i64 %value)")
    ir.push("declare ptr @u64_to_string(i64 %value)")
    ir.push("declare ptr @f64_to_string(double %value)")
    ir.push("declare ptr @string_upper(ptr %str)")
    ir.push("declare ptr @string_lower(ptr %str)")
    ir.push("declare ptr @string_trim(ptr %str)")
    ir.push("declare ptr @string_split(ptr %str, ptr %delim)")
    ir.push("declare i64 @string_contains(ptr %str, ptr %sub)")
    ir.push("declare i64 @string_starts_with(ptr %str, ptr %prefix)")
    ir.push("declare i64 @string_ends_with(ptr %str, ptr %suffix)")
    ir.push("declare ptr @string_replace(ptr %str, ptr %old, ptr %new)")
    ir.push("declare ptr @int_from_string(ptr %str)")
    ir.push("declare ptr @float_from_string(ptr %str)")
    ir.push("")

    ir.push("; Declare CLI argument helper functions")
    ir.push("declare i64 @get_argc_value(ptr %cli_table)")
    ir.push("declare i64 @get_argv_value(ptr %cli_table, i64 %index)")
    ir.push("")

    ir.push("; Declare Result/Optional helper functions")
    ir.push("declare ptr @mog_result_ok(i64 %value)")
    ir.push("declare ptr @mog_result_err(ptr %message)")
    ir.push("declare i64 @mog_result_is_ok(ptr %result)")
    ir.push("declare i64 @mog_result_unwrap(ptr %result)")
    ir.push("declare ptr @mog_result_unwrap_err(ptr %result)")
    ir.push("declare ptr @mog_optional_some(i64 %value)")
    ir.push("declare ptr @mog_optional_none()")
    ir.push("declare i64 @mog_optional_is_some(ptr %optional)")
    ir.push("declare i64 @mog_optional_unwrap(ptr %optional)")
    ir.push("")

    ir.push("; Declare tensor functions")
    ir.push("declare ptr @tensor_create(i64 %ndim, ptr %shape, i64 %dtype)")
    ir.push("declare ptr @tensor_create_with_data(i64 %ndim, ptr %shape, ptr %data, i64 %dtype)")
    ir.push("declare float @tensor_get_f32(ptr %tensor, i64 %idx)")
    ir.push("declare void @tensor_set_f32(ptr %tensor, i64 %idx, float %val)")
    ir.push("declare i64 @tensor_shape_dim(ptr %tensor, i64 %dim)")
    ir.push("declare i64 @tensor_ndim(ptr %tensor)")
    ir.push("declare i64 @tensor_size(ptr %tensor)")
    ir.push("declare ptr @tensor_add(ptr %a, ptr %b)")
    ir.push("declare ptr @tensor_sub(ptr %a, ptr %b)")
    ir.push("declare ptr @tensor_mul(ptr %a, ptr %b)")
    ir.push("declare ptr @tensor_matmul(ptr %a, ptr %b)")
    ir.push("declare double @tensor_sum(ptr %tensor)")
    ir.push("declare double @tensor_mean(ptr %tensor)")
    ir.push("declare void @tensor_print(ptr %tensor)")
    ir.push("declare ptr @tensor_reshape(ptr %tensor, i64 %ndim, ptr %shape)")
    ir.push("declare ptr @tensor_relu(ptr %tensor)")
    ir.push("declare ptr @tensor_sigmoid(ptr %tensor)")
    ir.push("declare ptr @tensor_tanh_act(ptr %tensor)")
    ir.push("declare ptr @tensor_softmax(ptr %tensor, i64 %dim)")
    ir.push("declare ptr @tensor_transpose(ptr %tensor)")
    ir.push("declare ptr @tensor_zeros(ptr %shape, i64 %ndims)")
    ir.push("declare ptr @tensor_ones(ptr %shape, i64 %ndims)")
    ir.push("declare ptr @tensor_randn(ptr %shape, i64 %ndims)")
    ir.push("")

    ir.push("; Declare no_grad context management functions")
    ir.push("declare void @mog_no_grad_begin()")
    ir.push("declare void @mog_no_grad_end()")
    ir.push("declare i64 @mog_is_no_grad()")
    ir.push("")

    ir.push("; Cooperative interrupt flag (checked at every loop back-edge)")
    ir.push("@mog_interrupt_flag = external global i32")
    ir.push("declare void @exit(i32) noreturn")
    ir.push("")

    ir.push("; Declare terminal I/O and utility functions")
    ir.push("declare i64 @stdin_poll(i64 %timeout_ms)")
    ir.push("declare ptr @stdin_read_line()")
    ir.push("declare i64 @time_ms()")
    ir.push("declare i64 @string_eq(ptr %a, ptr %b)")
    ir.push("declare void @flush_stdout()")
    ir.push("declare i64 @parse_int(ptr %s)")
    ir.push("declare double @parse_float(ptr %s)")
    ir.push("")
  }

  private generateAsyncDeclarations(ir: string[]): void {
    ir.push("")
    ir.push("; LLVM Coroutine intrinsics")
    ir.push("declare token @llvm.coro.id(i32, ptr, ptr, ptr)")
    ir.push("declare i64 @llvm.coro.size.i64()")
    ir.push("declare ptr @llvm.coro.begin(token, ptr)")
    ir.push("declare token @llvm.coro.save(ptr)")
    ir.push("declare i8 @llvm.coro.suspend(token, i1)")
    ir.push("declare ptr @llvm.coro.free(token, ptr)")
    ir.push("declare i1 @llvm.coro.end(ptr, i1, token)")
    ir.push("declare void @llvm.coro.resume(ptr)")
    ir.push("declare void @llvm.coro.destroy(ptr)")
    ir.push("declare i1 @llvm.coro.done(ptr)")
    ir.push("")
    ir.push("; Async runtime functions")
    ir.push("declare ptr @mog_future_new()")
    ir.push("declare void @mog_future_complete(ptr, i64)")
    ir.push("declare i64 @mog_await(ptr, ptr)")
    ir.push("declare i32 @mog_future_is_ready(ptr)")
    ir.push("declare i64 @mog_future_get_result(ptr)")
    ir.push("declare void @mog_future_set_waiter(ptr, ptr)")
    ir.push("declare ptr @malloc(i64)")
    ir.push("declare void @free(ptr)")
    ir.push("declare void @mog_loop_schedule(ptr, ptr)")
    ir.push("declare ptr @mog_loop_get_global()")
    ir.push("declare void @mog_loop_add_timer(ptr, i64, ptr)")
    ir.push("declare ptr @mog_all(ptr, i32)")
    ir.push("declare ptr @mog_race(ptr, i32)")
    ir.push("declare ptr @mog_loop_new()")
    ir.push("declare void @mog_loop_set_global(ptr)")
    ir.push("declare void @mog_loop_run(ptr)")
    ir.push("declare void @mog_loop_free(ptr)")
    ir.push("declare ptr @mog_future_get_coro_handle(ptr)")
    ir.push("declare void @mog_coro_resume(ptr)")
    ir.push("declare void @mog_future_set_coro_frame(ptr, ptr)")
    ir.push("declare ptr @async_read_line()")
    ir.push("declare void @mog_loop_add_fd_watcher(ptr, i32, i32, ptr)")
    ir.push("")
  }

  private generateAwaitExpression(ir: string[], expr: any): string {
    const awaitIdx = this.awaitCounter++
    const innerExpr = expr.argument

    // Check if this is a capability call (e.g., process.sleep, fs.read_file)
    // that returns a handle (MogFuture*)
    let innerFuture: string
    const isCapabilityAwait = innerExpr?.type === "CallExpression" &&
      innerExpr?.callee?.type === "MemberExpression" &&
      this.capabilities.has(innerExpr?.callee?.object?.name)

    if (isCapabilityAwait) {
      const capName = innerExpr.callee.object.name
      const funcName = innerExpr.callee.property
      const capArgs = (innerExpr.args || innerExpr.arguments || []).map((a: any) => this.generateExpression(ir, a))

      if (this.isAsyncCapabilityFunc(capName, funcName)) {
        // This capability function is declared as `async fn` in its .mogdecl.
        // The host returns a MogFuture* (as i64 inside a MogHandle).
        const capResult = this.generateCapabilityCall(ir, capName, funcName, capArgs, innerExpr)
        const futurePtr = `%await_future_${awaitIdx}`
        ir.push(`  ${futurePtr} = inttoptr i64 ${capResult} to ptr`)
        innerFuture = futurePtr
      } else {
        // Synchronous capability call — wrap result in an immediately-completed future.
        const capResult = this.generateCapabilityCall(ir, capName, funcName, capArgs, innerExpr)
        const newFuture = `%await_syncfut_${awaitIdx}`
        ir.push(`  ${newFuture} = call ptr @mog_future_new()`)
        ir.push(`  call void @mog_future_complete(ptr ${newFuture}, i64 ${capResult})`)
        innerFuture = newFuture
      }
    } else {
      // Regular async function call - evaluates to a MogFuture* (ptr)
      const futureVal = this.generateExpression(ir, innerExpr)
      innerFuture = futureVal

      // Check if this is a runtime async builtin (like async_read_line) that returns
      // a future directly without being a coroutine — no need to resume a coro handle.
      const isRuntimeAsync = innerExpr?.type === "CallExpression" &&
        innerExpr?.callee?.type === "Identifier" &&
        this.runtimeAsyncFunctions.has(innerExpr?.callee?.name)

      if (!isRuntimeAsync) {
        // The called async function is suspended at its initial suspend point.
        // We need to resume it so it can start executing.
        // Extract the coroutine handle from the returned future and resume it.
        const childHandle = `%await_child_hdl_${awaitIdx}`
        ir.push(`  ${childHandle} = call ptr @mog_future_get_coro_handle(ptr ${innerFuture})`)
        // Resume the child coroutine (it will run until it completes or suspends at an await)
        ir.push(`  call void @mog_coro_resume(ptr ${childHandle})`)
      }
    }

    // Save coroutine state BEFORE the await (coro.save sets the resume index)
    const saveToken = `%await_save_${awaitIdx}`
    ir.push(`  ${saveToken} = call token @llvm.coro.save(ptr ${this.coroHandle})`)

    // Call mog_await — registers coro_handle as waiter. If the future is already
    // ready, mog_await enqueues it in the event loop's ready queue so that
    // coro.suspend below returns immediately upon resumption.
    const awaitResult = `%await_res_${awaitIdx}`
    ir.push(`  ${awaitResult} = call i64 @mog_await(ptr ${innerFuture}, ptr ${this.coroHandle})`)

    // Always suspend. The event loop will resume us immediately if the future
    // was already ready (mog_await enqueued it). This avoids the LLVM coroutine
    // pass miscompilation where skipping coro.suspend causes llvm.assume to
    // eliminate readiness checks in the resume function.
    const susResult = `%await_sus_${awaitIdx}`
    ir.push(`  ${susResult} = call i8 @llvm.coro.suspend(token ${saveToken}, i1 false)`)
    ir.push(`  switch i8 ${susResult}, label %coro.suspend [`)
    ir.push(`    i8 0, label %await${awaitIdx}.ready`)
    ir.push(`    i8 1, label %coro.cleanup`)
    ir.push(`  ]`)

    // Ready path - get the result
    ir.push(`await${awaitIdx}.ready:`)
    const rawResult = `%await_raw_${awaitIdx}`
    ir.push(`  ${rawResult} = call i64 @mog_future_get_result(ptr ${innerFuture})`)

    // Check if the result should be a pointer (e.g., a function returning a string pointer)
    const resultType = expr?.resultType
    const needsPtrConversion = resultType?.type === "PointerType" || resultType?.type === "StringType"

    if (needsPtrConversion) {
      const ptrResult = `%await_val_${awaitIdx}`
      ir.push(`  ${ptrResult} = inttoptr i64 ${rawResult} to ptr`)
      return ptrResult
    }

    return rawResult
  }

  private generateSpawnExpression(ir: string[], expr: any): string {
    // spawn <async_call> — call the async function, which returns a MogFuture*,
    // then resume its coroutine eagerly. The future is returned as a ptr (can be ignored).
    const innerExpr = expr.argument
    const futureReg = this.generateExpression(ir, innerExpr)

    // The async function ran eagerly to its first suspension point.
    // The event loop will handle the rest. Return the future pointer.
    return futureReg
  }

  private generateStatement(ir: string[], statement: StatementNode): void {
    // Skip if current block is already terminated
    if (this.blockTerminated) {
      return
    }

    switch (statement.type) {
      case "VariableDeclaration": {
        const llvmType = this.toLLVMType(statement.varType)
        const reg = `%${statement.name}`
         let isPointerLike = statement.varType?.type === "ArrayType" || statement.varType?.type === "PointerType" 
          || statement.varType?.type === "StringType" || statement.varType?.type === "MapType"
          || statement.varType?.type === "StructType" || statement.varType?.type === "CustomType"
          || statement.varType?.type === "SOAType" || statement.varType?.type === "AOSType" || statement.varType?.type === "FunctionType"
          || statement.varType?.type === "TensorType"
        // Infer pointer type from value expression when varType is not explicit
        if (!isPointerLike && statement.value) {
          isPointerLike = this.isStringProducingExpression(statement.value)

        }
        // Infer tensor type from TensorConstruction value
        if (!isPointerLike && (statement.value as any)?.type === "TensorConstruction") {
          isPointerLike = true
          const tensorDtype = (statement.value as any).dtype
          this.variableTypes.set(statement.name, new TensorType(tensorDtype))
        }
        // Infer tensor type from value expression's resultType (e.g., tensor + tensor, matmul(a, b))
        if (!isPointerLike && (statement.value as any)?.resultType?.type === "TensorType") {
          isPointerLike = true
          this.variableTypes.set(statement.name, (statement.value as any).resultType)
        }
        // Infer closure/function type from call expression returning ptr (function type)
        if (!isPointerLike && statement.value) {
          const val = statement.value as any
          if (val.type === "CallExpression" && val.callee?.type === "Identifier") {
            const calledFunc = this.functions.get(val.callee.name)
            if (calledFunc && calledFunc.returnType === "ptr") {
              isPointerLike = true
            }
          }
        }
         // Infer closure type from lambda expression
        if (!isPointerLike && statement.value?.type === "Lambda") {
          isPointerLike = true
        }
        // Infer map type from MapLiteral value
        if (!isPointerLike && statement.value?.type === "MapLiteral") {
          isPointerLike = true
        }
        // Infer struct type from StructLiteral value
        if (!isPointerLike && statement.value?.type === "StructLiteral") {
          isPointerLike = true
        }
        // Infer array type from ArrayLiteral or ArrayFill value
        if (!isPointerLike && (statement.value?.type === "ArrayLiteral" || statement.value?.type === "ArrayFill")) {
          isPointerLike = true
        }
        // Infer pointer type from AwaitExpression whose resultType is pointer/string
        if (!isPointerLike && statement.value?.type === "AwaitExpression") {
          const awaitResultType = (statement.value as any).resultType
          if (awaitResultType?.type === "PointerType" || awaitResultType?.type === "StringType" ||
              awaitResultType?.type === "ArrayType" || awaitResultType?.type === "AOSType" ||
              awaitResultType?.type === "StructType" || awaitResultType?.type === "MapType") {
            isPointerLike = true
          }
        }
        const allocaType = isPointerLike ? "ptr" : llvmType
        ir.push(`  ${reg} = alloca ${allocaType}`)
        if (isPointerLike && !statement.varType) {
          // Check if this is a tensor construction — already registered in variableTypes above
          const isTensorValue = (statement.value as any)?.type === "TensorConstruction"
          // Check if this is a function/closure value — register as FunctionType
          const isFuncValue = statement.value?.type === "Lambda"
            || (statement.value?.type === "CallExpression" && statement.value.callee?.type === "Identifier"
                && this.functionTypes.get(statement.value.callee.name)?.type === "FunctionType")
            || (statement.value?.type === "Identifier" && this.functions.has(statement.value.name))
          if (isTensorValue) {
            // Already registered as TensorType above, skip
          } else if (isFuncValue) {
            // Register as FunctionType so indirect call handler kicks in
            const funcType = (statement.value as any)?.resultType || new FunctionType([], { type: "VoidType" } as any)
            this.variableTypes.set(statement.name, funcType)
           } else if (statement.value?.type === "MapLiteral") {
            // Register as MapType, preserving key/value types from resultType if available
            const mapResultType = (statement.value as any)?.resultType
            if (mapResultType?.type === "MapType" && mapResultType.keyType && mapResultType.valueType) {
              this.variableTypes.set(statement.name, mapResultType)
            } else {
              this.variableTypes.set(statement.name, { type: "MapType" } as any)
            }
          } else if (this.isStringProducingExpression(statement.value)) {
            // For inferred string types, register as StringType
            this.variableTypes.set(statement.name, { type: "StringType" } as any)
          } else {
            // For inferred pointer types, register as ArrayType(u8, []) so isStringType works
            this.variableTypes.set(statement.name, new ArrayType(new UnsignedType("u8"), []))
          }
        } else if (statement.varType?.type === "SOAType" && (statement.varType as any).structName && !(statement.varType instanceof SOAType)) {
          // Resolve raw SOAType annotation from parser to a real SOAType instance
          const soaStructName = (statement.varType as any).structName
          const soaCapacity = (statement.varType as any).capacity || 16
          const structFields = this.structDefs.get(soaStructName)
          if (structFields) {
            const structFieldMap = new Map<string, any>()
            for (const f of structFields) {
              structFieldMap.set(f.name, f.fieldType)
            }
            const resolvedStructType = new StructType(soaStructName, structFieldMap)
            const resolvedSoaType = new SOAType(resolvedStructType, soaCapacity)
            this.variableTypes.set(statement.name, resolvedSoaType)
            this.structDefs.set(statement.name, structFields)
          } else {
            this.variableTypes.set(statement.name, statement.varType)
          }
        } else {
          this.variableTypes.set(statement.name, statement.varType)
        }
        if (statement.value) {
          const value = this.generateExpression(ir, statement.value)
           let isPointerLike2 = statement.varType?.type === "ArrayType" || statement.varType?.type === "PointerType"
            || statement.varType?.type === "StringType" || statement.varType?.type === "MapType"
            || statement.varType?.type === "StructType" || statement.varType?.type === "CustomType"
            || statement.varType?.type === "SOAType" || statement.varType?.type === "AOSType" || statement.varType?.type === "FunctionType"
            || statement.varType?.type === "TensorType"
          if (!isPointerLike2 && isPointerLike) {
            isPointerLike2 = true
          }
          const storeType = isPointerLike2 ? "ptr" : llvmType
          // Capability calls return i64 (raw data from MogValue union).
          // If storing into a ptr variable, convert i64 -> ptr.
          // Also handle await wrapping a capability call (await cap.fn()).
          const exprVal = statement.value as any
          const isDirectCapCall = (exprVal?.type === "CallExpression" 
            && exprVal?.callee?.type === "MemberExpression"
            && this.capabilities.has(exprVal?.callee?.object?.name))
          // Note: AwaitExpression wrapping a cap call already converts to ptr in generateAwaitExpression,
          // so we only need inttoptr for direct (non-awaited) cap calls.
          if (storeType === "ptr" && isDirectCapCall) {
            const ptrConv = `%${this.valueCounter++}`
            ir.push(`  ${ptrConv} = inttoptr i64 ${value} to ptr`)
            ir.push(`  store ptr ${ptrConv}, ptr ${reg}`)
          } else {
            ir.push(`  store ${storeType} ${value}, ptr ${reg}`)
          }
        }
        break
      }
      case "Assignment": {
        const value = this.generateExpression(ir, statement.value)
        const varType = this.variableTypes.get(statement.name)
        const llvmType = this.toLLVMType(varType)
        ir.push(`  store ${llvmType} ${value}, ptr %${statement.name}`)
        break
      }
      case "ExpressionStatement":
        this.generateExpression(ir, statement.expression)
        break
      case "Block":
        this.blockTerminated = false
        for (const stmt of statement.statements) {
          this.generateStatement(ir, stmt)
          if (this.blockTerminated) break
        }
        break
      case "Return": {
        let value = null
        if (statement.value) {
          value = this.generateExpression(ir, statement.value)
        }
        if (this.isInAsyncFunction) {
          // In async functions, return completes the future and jumps to cleanup
          const retVal = value || "0"
          ir.push(`  call void @mog_future_complete(ptr ${this.coroFuture}, i64 ${retVal})`)
          ir.push(`  br label %coro.cleanup`)
          this.blockTerminated = true
        } else {
          ir.push("  call void @gc_pop_frame()")
          const returnType = this.currentFunction?.returnType || "i64"
          if (value && returnType !== "void") {
            ir.push(`  ret ${returnType} ${value}`)
          } else if (returnType === "void") {
            ir.push(`  ret void`)
          } else {
            ir.push(`  ret void`)
          }
          this.blockTerminated = true
        }
        break
      }
      case "Conditional":
        this.generateConditional(ir, statement)
        break
      case "FunctionDeclaration":
        break
      case "AsyncFunctionDeclaration":
        // TODO: Async functions are hoisted and compiled separately (like regular functions)
        break
      case "StructDeclaration": {
        // Register struct field definitions for codegen
        const fields = (statement as any).fields?.map((f: any) => ({
          name: f.name,
          fieldType: f.fieldType
        })) || []
        this.structDefs.set((statement as any).name, fields)
        break
      }
      case "WhileLoop":
        this.generateWhileLoop(ir, statement)
        break
      case "ForLoop":
        this.generateForLoop(ir, statement)
        break
      case "ForEachLoop":
        this.generateForEachLoop(ir, statement)
        break
      case "ForInRange":
        this.generateForInRange(ir, statement)
        break
      case "ForInIndex":
        this.generateForInIndex(ir, statement)
        break
      case "ForInMap":
        this.generateForInMap(ir, statement)
        break
      case "Break":
        this.generateBreak(ir)
        break
      case "Continue":
        this.generateContinue(ir)
        break
      case "TryCatch":
        this.generateTryCatch(ir, statement)
        break
      case "WithBlock":
        this.generateWithBlock(ir, statement)
        break
      default:
        break
    }
  }

  private generateExpression(ir: string[], expr: any): string {
    switch (expr.type) {
      case "AssignmentExpression":
        return this.generateAssignmentExpression(ir, expr)
      case "NumberLiteral": {
        // Handle float literals - convert to integer bit pattern for storage
        const val = String(expr.value)
        // Check if it's a float literal (has decimal point or exponent)
        const isFloat = val.includes(".") || val.toLowerCase().includes("e")
        if (isFloat) {
          const numVal = parseFloat(val)
          // Determine target float type from literalType annotation
          const floatKind = expr.literalType?.kind || "f64"
          // Convert to integer bit pattern (not hex float format)
          return this.floatToIntBits(numVal, floatKind)
        }
        // Integer literal - parse octal if needed
        let intVal: number
        if (val.startsWith("0") && val.length > 1) {
          intVal = parseInt(val, 8)
        } else {
          intVal = parseInt(val, 10)
        }
        return `${intVal}`
      }
      case "BooleanLiteral":
        return expr.value ? "1" : "0"
      case "StringLiteral": {
        const name = this.generateStringLiteral(ir, expr.value)
        const ptrReg = `%${this.valueCounter++}`
        ir.push(`  ${ptrReg} = getelementptr [${this.getStringByteLength(expr.value) + 1} x i8], ptr ${name}, i64 0, i64 0`)
        return ptrReg
      }
      case "TemplateLiteral":
        return this.generateTemplateLiteral(ir, expr)
      case "Identifier": {
        const name = expr.name

        // Math constants: PI and E as compile-time constant doubles
        if (name === "PI") {
          return "0x400921fb54442d18" // Math.PI as f64 bit pattern
        }
        if (name === "E") {
          return "0x4005bf0a8b145769" // Math.E as f64 bit pattern
        }

        // If name is a known function (not a variable), wrap it as a closure value
        if (this.functions.has(name) && !this.variableTypes.has(name)) {
          return this.getOrCreateFunctionWrapper(ir, name)
        }

        const ptrReg = `%${name}_local`
        const valReg = `%${this.valueCounter++}`

         const varType = this.variableTypes.get(name)
        const isPointerLike = varType?.type === "ArrayType" || varType?.type === "PointerType" || 
                              varType?.type === "StringType" || varType?.type === "MapType" ||
                              varType?.type === "StructType" || varType?.type === "CustomType" || 
                              varType?.type === "SOAType" || varType?.type === "AOSType" || varType?.type === "FunctionType" ||
                              varType?.type === "TensorType"

        // Determine LLVM type for loading
        let llvmLoadType = "i64"
        if (varType?.type === "FloatType") {
          llvmLoadType = this.toLLVMType(varType)
        }

        if (this.currentFunction && this.currentFunction.params.find((p) => p.name === name)) {
          if (isPointerLike) {
            const loaded = `%${this.valueCounter++}`
            ir.push(`  ${loaded} = load ptr, ptr ${ptrReg}`)
            return loaded
          }
          ir.push(`  ${valReg} = load ${llvmLoadType}, ptr ${ptrReg}`)
          return valReg
        }

        if (isPointerLike) {
          const loaded = `%${this.valueCounter++}`
          ir.push(`  ${loaded} = load ptr, ptr %${name}`)
          return loaded
        }

        ir.push(`  ${valReg} = load ${llvmLoadType}, ptr %${name}`)
        return valReg
      }
      case "BinaryExpression":
        return this.generateBinaryExpression(ir, expr)
      case "UnaryExpression":
        return this.generateUnaryExpression(ir, expr)
      case "CallExpression":
        return this.generateCallExpression(ir, expr)
      case "ArrayLiteral":
        return this.generateArrayLiteral(ir, expr)
      case "ArrayFill":
        return this.generateArrayFill(ir, expr)
      case "MapLiteral":
        return this.generateMapLiteral(ir, expr)
      case "StructLiteral":
        return this.generateStructLiteral(ir, expr)
      case "SoALiteral":
        return this.generateSoALiteral(ir, expr)
      case "SoAConstructor":
        return this.generateSoAConstructor(ir, expr)
      case "MemberExpression":
        return this.generateMemberExpression(ir, expr)
      case "IndexExpression":
        return this.generateIndexExpression(ir, expr)
      case "SliceExpression":
        return this.generateSliceExpression(ir, expr)
      case "MapExpression":
        return this.generateMapExpression(ir, expr)
      case "CastExpression":
        return this.generateCastExpression(ir, expr)
      case "IfExpression":
        return this.generateIfExpression(ir, expr)
      case "MatchExpression":
        return this.generateMatchExpression(ir, expr)
      case "Lambda":
        return this.generateLambda(ir, expr)
      case "OkExpression":
        return this.generateOkExpression(ir, expr)
      case "ErrExpression":
        return this.generateErrExpression(ir, expr)
      case "SomeExpression":
        return this.generateSomeExpression(ir, expr)
      case "NoneExpression":
        return this.generateNoneExpression(ir)
      case "PropagateExpression":
        return this.generatePropagateExpression(ir, expr)
      case "IsSomeExpression":
        return this.generateIsSomeExpression(ir, expr)
      case "IsNoneExpression":
        return this.generateIsNoneExpression(ir, expr)
      case "IsOkExpression":
        return this.generateIsOkExpression(ir, expr)
      case "IsErrExpression":
        return this.generateIsErrExpression(ir, expr)
      case "AwaitExpression":
        if (this.isInAsyncFunction) {
          return this.generateAwaitExpression(ir, expr)
        }
        // Fallback: non-async context, just evaluate synchronously
        return this.generateExpression(ir, expr.argument)
      case "SpawnExpression":
        return this.generateSpawnExpression(ir, expr)
      case "TensorConstruction":
        return this.generateTensorConstruction(ir, expr)
      default:
        return ""
    }
  }

  private generateStringLiteral(_ir: string[], value: string): string {
    // Look up the name from the map, or create a new one
    let name = this.stringNameMap.get(value)
    if (!name) {
      name = `@str${this.stringCounter++}`
      this.stringNameMap.set(value, name)
    }
    return name
  }

  private collectStringFromNode(node: any): void {
    if (!node) return

    if (node.type === "StringLiteral") {
      const name = `@str${this.stringCounter++}`
      this.stringNameMap.set(node.value, name)
      const { escaped: llvmEscaped, byteLength } = this.escapeStringForLLVM(node.value)
      this.stringByteLengths.set(node.value, byteLength)
      const strDef = `${name} = private unnamed_addr constant [${byteLength + 1} x i8] c"${llvmEscaped}\\00"`
      this.stringConstants.push(strDef)
      return
    }

    // Collect strings from template literals
    if (node.type === "TemplateLiteral") {
      for (const part of node.parts) {
        if (typeof part === "string") {
          // Check if this string part is already registered
          if (!this.stringNameMap.has(part)) {
            const name = `@str${this.stringCounter++}`
            this.stringNameMap.set(part, name)
            const { escaped: llvmEscaped, byteLength } = this.escapeStringForLLVM(part)
            this.stringByteLengths.set(part, byteLength)
            const strDef = `${name} = private unnamed_addr constant [${byteLength + 1} x i8] c"${llvmEscaped}\\00"`
            this.stringConstants.push(strDef)
          }
        } else {
          this.collectStringFromNode(part)
        }
      }
      return
    }

     // Collect map literal key strings
    if (node.type === "MapLiteral") {
      const entries = node.entries || node.columns || []
      for (const entry of entries) {
        const key = entry.key || entry.name
        if (key && typeof key === "string") {
          const keyName = `@map_key_${this.stringCounter++}`
          this.stringNameMap.set(`__map_key_${key}`, keyName)
          const { escaped: keyEscaped, byteLength: keyByteLen } = this.escapeStringForLLVM(key)
          this.stringByteLengths.set(key, keyByteLen)
          const strDef = `${keyName} = private constant [${keyByteLen + 1} x i8] c"${keyEscaped}\\00"`
          this.stringConstants.push(strDef)
        }
      }
      // Still recurse into values
      for (const entry of entries) {
        if (entry.value) this.collectStringFromNode(entry.value)
      }
      return
    }

    // Collect MemberExpression property names as string constants
    if (node.type === "MemberExpression") {
      const name = `@key_${node.property}_${this.stringCounter++}`
      const { escaped: llvmEscaped, byteLength } = this.escapeStringForLLVM(node.property)
      this.stringByteLengths.set(node.property, byteLength)
      const strDef = `${name} = private unnamed_addr constant [${byteLength + 1} x i8] c"${llvmEscaped}\\00"`
      this.stringConstants.push(strDef)
    }

    // Recursively collect from child nodes
    for (const key in node) {
      if (key === 'position' || key.startsWith('_')) continue
      const value = node[key]
      if (Array.isArray(value)) {
        value.forEach(child => this.collectStringFromNode(child))
      } else if (typeof value === 'object' && value !== null) {
        this.collectStringFromNode(value)
      }
    }
  }

  private collectStringConstants(ast: any): void {
    this.collectStringFromNode(ast)
  }

  private inferExpressionType(expr: any): string {
    // Infer the type of an expression for template literal conversion
    switch (expr.type) {
      case "StringLiteral":
        return "string"
      case "NumberLiteral": {
        const val = String(expr.value)
        if (val.includes(".") || val.toLowerCase().includes("e")) {
          return "f64"
        }
        return "i64"
      }
      case "Identifier": {
        const varType = this.variableTypes.get(expr.name)
        if (varType?.type === "FloatType") return varType.kind || "f64"
        if (varType?.type === "IntegerType") return varType.kind || "i64"
        if (varType?.type === "UnsignedType") return varType.kind || "u64"
        if (varType?.type === "StringType") return "string" // string type
        if (varType?.type === "ArrayType") return "string" // [u8] strings
        if (varType?.type === "PointerType") return "string" // ptr strings
        // Check if this is a string-typed variable by checking isStringType
        if (this.isStringType(expr)) return "string"
        return "i64" // default
      }
      case "BinaryExpression": {
        // For binary expressions, determine based on operands
        if (this.isFloatOperand(expr.left) || this.isFloatOperand(expr.right)) {
          return "f64"
        }
        return "i64"
      }
      case "CallExpression": {
        const funcName = expr.function?.name || expr.callee?.name
        if (funcName) {
          const func = this.functions.get(funcName)
          if (func) {
            if (func.returnType.startsWith("f")) return "f64"
            if (func.returnType === "ptr") return "string"
          }
        }
        return "i64"
      }
      case "TemplateLiteral":
        return "string"
      case "MemberExpression": {
        // Check if this is a struct field access to a string/ptr field
        const objType = this.variableTypes.get(expr.object?.name)
        const structName = objType?.name || objType?.structName
        const fields = structName ? this.structDefs.get(structName) : null
        if (fields) {
          const field = fields.find((f: any) => f.name === expr.property)
          if (field?.fieldType?.type === "FloatType") return field.fieldType.kind || "f64"
          if (field?.fieldType?.type === "PointerType") return "string"
          if (field?.fieldType?.type === "StringType") return "string"
        }
        return "i64"
      }
      case "IndexExpression": {
        // Check the element type of the array
        if (expr.object?.type === "Identifier") {
          const varType = this.variableTypes.get(expr.object.name)
          if (varType?.type === "ArrayType") {
            if (varType.elementType?.type === "StringType") return "string"
            if (varType.elementType?.type === "FloatType") return varType.elementType.kind || "f64"
          }
        }
        return "i64"
      }
      default:
        return "i64"
    }
  }

  private generateTemplateLiteral(ir: string[], expr: any): string {
    const parts = expr.parts as (string | any)[]
    if (parts.length === 0) {
      // Empty string
      const name = this.generateStringLiteral(ir, "")
      const ptrReg = `%${this.valueCounter++}`
      ir.push(`  ${ptrReg} = getelementptr [1 x i8], ptr ${name}, i64 0, i64 0`)
      return ptrReg
    }

    // Collect all string parts into registers
    const partRegs: string[] = []
    for (const part of parts) {
      if (typeof part === "string") {
        // String literal part
        const name = this.generateStringLiteral(ir, part)
        const ptrReg = `%${this.valueCounter++}`
        ir.push(`  ${ptrReg} = getelementptr [${this.getStringByteLength(part) + 1} x i8], ptr ${name}, i64 0, i64 0`)
        partRegs.push(ptrReg)
      } else {
        // Expression part - generate it and convert to string
        const exprReg = this.generateExpression(ir, part)
        const exprType = this.inferExpressionType(part)

        // Convert non-string expressions to strings
        if (exprType === "string") {
          partRegs.push(exprReg)
        } else if (exprType === "f64" || exprType === "f32" || exprType === "f16" || exprType === "f128") {
          // Float to string
          const convertReg = `%${this.valueCounter++}`
          ir.push(`  ${convertReg} = call ptr @f64_to_string(double ${exprReg})`)
          partRegs.push(convertReg)
        } else if (exprType.startsWith("u")) {
          // Unsigned to string
          const convertReg = `%${this.valueCounter++}`
          ir.push(`  ${convertReg} = call ptr @u64_to_string(i64 ${exprReg})`)
          partRegs.push(convertReg)
        } else {
          // Signed integer to string
          const convertReg = `%${this.valueCounter++}`
          ir.push(`  ${convertReg} = call ptr @i64_to_string(i64 ${exprReg})`)
          partRegs.push(convertReg)
        }
      }
    }

    // Concatenate all parts using string_concat
    let result = partRegs[0]
    for (let i = 1; i < partRegs.length; i++) {
      const concatResult = `%${this.valueCounter++}`
      ir.push(`  ${concatResult} = call ptr @string_concat(ptr ${result}, ptr ${partRegs[i]})`)
      result = concatResult
    }

    return result
  }

  private generateBinaryExpression(ir: string[], expr: any): string {
    const left = this.generateExpression(ir, expr.left)
    const right = this.generateExpression(ir, expr.right)
    const reg = `%${this.valueCounter++}`

    // Handle logical operators specially - they work on i1 but we store as i64
    if (expr.operator === "&&" || expr.operator === "||") {
      // Convert operands to i1 (compare with 0), do logical op, then extend to i64
      const leftBool = `%${this.valueCounter++}`
      const rightBool = `%${this.valueCounter++}`
      ir.push(`  ${leftBool} = icmp ne i64 ${left}, 0`)
      ir.push(`  ${rightBool} = icmp ne i64 ${right}, 0`)
      const boolReg = `%${this.valueCounter++}`
      const logicOp = expr.operator === "&&" ? "and" : "or"
      ir.push(`  ${boolReg} = ${logicOp} i1 ${leftBool}, ${rightBool}`)
      const extReg = `%${this.valueCounter++}`
      ir.push(`  ${extReg} = zext i1 ${boolReg} to i64`)
      return extReg
    }

    // Check for tensor operations - both operands are tensors
    if (this.isTensorType(expr.left) || this.isTensorType(expr.right)) {
      const tensorOps: Record<string, string> = {
        "+": "@tensor_add",
        "-": "@tensor_sub",
        "*": "@tensor_mul",
      }
      const tensorFunc = tensorOps[expr.operator]
      if (tensorFunc) {
        const resultReg = `%${this.valueCounter++}`
        ir.push(`  ${resultReg} = call ptr ${tensorFunc}(ptr ${left}, ptr ${right})`)
        return resultReg
      }
    }

    // Check for matrix operations - both operands are 2D arrays
    if (this.isMatrixOperation(expr.left, expr.right)) {
      if (expr.operator === "*") {
        return this.generateMatrixMultiplication(ir, left, right)
      } else if (expr.operator === "+") {
        return this.generateMatrixAddition(ir, left, right)
      }
    }

    // Check for vector operations - at least one operand is an array
    if (this.isVectorOperation(expr.left, expr.right)) {
      return this.generateVectorOperation(ir, expr, left, right)
    }

    // Handle pointer arithmetic: ptr + offset or ptr - offset
    if (this.isPointerArithmetic(expr, left, right)) {
      return this.generatePointerArithmetic(ir, expr, left, right)
    }

    // Handle string concatenation: string + string
    if (expr.operator === "+" && (this.isStringType(expr.left) || this.isStringType(expr.right))) {
      const concatReg = `%${this.valueCounter++}`
      ir.push(`  ${concatReg} = call ptr @string_concat(ptr ${left}, ptr ${right})`)
      return concatReg
    }

    const opMap: Record<string, string[]> = {
      "+": ["add i64", "fadd"],
      "-": ["sub i64", "fsub"],
      "*": ["mul i64", "fmul"],
      "/": ["sdiv i64", "fdiv"],
      "%": ["srem i64", "frem"],
      "|": ["or i64", "or"],
      "&": ["and i64", "and"],
      "^": ["xor i64", "xor"],
      "<<": ["shl i64", "shl"],
      ">>": ["ashr i64", "ashr"],
    }

    // Comparison operators return i1, need to extend to i64
    const cmpMap: Record<string, string[]> = {
      "<": ["icmp slt i64", "fcmp olt"],
      ">": ["icmp sgt i64", "fcmp ogt"],
      "<=": ["icmp sle i64", "fcmp ole"],
      ">=": ["icmp sge i64", "fcmp oge"],
      "=": ["icmp eq i64", "fcmp oeq"],
      "==": ["icmp eq i64", "fcmp oeq"],
      "!=": ["icmp ne i64", "fcmp one"],
    }

    // Determine if operation is float-based by checking operand types
    const isFloat = this.isFloatOperand(expr.left) || this.isFloatOperand(expr.right)

    if (cmpMap[expr.operator]) {
      // Handle string comparisons using string_eq runtime function
      const isStringCmp = this.isStringType(expr.left) || this.isStringType(expr.right)
      if (isStringCmp && (expr.operator === "==" || expr.operator === "=" || expr.operator === "!=")) {
        const eqReg = `%${this.valueCounter++}`
        ir.push(`  ${eqReg} = call i64 @string_eq(ptr ${left}, ptr ${right})`)
        if (expr.operator === "!=") {
          // string_eq returns 1 for equal, negate it
          const negReg = `%${this.valueCounter++}`
          ir.push(`  ${negReg} = icmp eq i64 ${eqReg}, 0`)
          const extReg = `%${this.valueCounter++}`
          ir.push(`  ${extReg} = zext i1 ${negReg} to i64`)
          return extReg
        }
        return eqReg
      }
      const [intOp, floatOp] = cmpMap[expr.operator]
      const floatType = isFloat ? this.getFloatTypeSize(expr) : "float"
      const cmpOp = isFloat ? `${floatOp} ${floatType} ${left}, ${right}` : `${intOp} ${left}, ${right}`
      const boolReg = `%${this.valueCounter++}`
      ir.push(`  ${boolReg} = ${cmpOp}`)
      const extReg = `%${this.valueCounter++}`
      ir.push(`  ${extReg} = zext i1 ${boolReg} to i64`)
      return extReg
    }

    const [intOp, floatOp] = opMap[expr.operator]
    if (isFloat) {
      const floatType = this.getFloatTypeSize(expr)
      ir.push(`  ${reg} = ${floatOp} ${floatType} ${left}, ${right}`)
    } else {
      ir.push(`  ${reg} = ${intOp} ${left}, ${right}`)
    }

    return reg
  }

  private isVectorOperation(leftExpr: any, rightExpr: any): boolean {
    // Check if at least one operand is an array type
    const leftArrayType = this.getExpressionArrayType(leftExpr)
    const rightArrayType = this.getExpressionArrayType(rightExpr)
    return leftArrayType !== null || rightArrayType !== null
  }

  private getExpressionArrayType(expr: any): any {
    if (!expr) return null

    // Check if the expression is an identifier with array type
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      if (varType?.type === "ArrayType") {
        return varType
      }
    }

    // Check if the expression itself has a resultType
    if (expr.resultType?.type === "ArrayType") {
      return expr.resultType
    }

    return null
  }

  private generateVectorOperation(ir: string[], expr: any, leftReg: string, rightReg: string): string {
    // Get element types for both operands
    const leftArrayType = this.getExpressionArrayType(expr.left)
    const rightArrayType = this.getExpressionArrayType(expr.right)

    // Determine which operand is the array and which is the scalar
    const isLeftArray = leftArrayType !== null
    const isRightArray = rightArrayType !== null

    if (!isLeftArray && !isRightArray) {
      throw new Error("Invalid vector operation: at least one operand must be an array")
    }

    // Use the array's element type
    const elementType = (leftArrayType || rightArrayType).elementType
    const isFloat = elementType?.type === "FloatType"
    const floatKind = isFloat ? elementType.kind : null

    // Get array length from the array operand
    const arrayReg = isLeftArray ? leftReg : rightReg
    const lenReg = `%${this.valueCounter++}`
    ir.push(`  ${lenReg} = call i64 @array_length(ptr ${arrayReg})`)

    // Allocate result array
    const dimensionsReg = `%${this.valueCounter++}`
    ir.push(`  ${dimensionsReg} = alloca [1 x i64]`)
    const dimensionsPtr = `%${this.valueCounter++}`
    ir.push(`  ${dimensionsPtr} = getelementptr [1 x i64], ptr ${dimensionsReg}, i64 0, i64 0`)
    ir.push(`  store i64 ${lenReg}, ptr ${dimensionsPtr}`)

    const elementSize = floatKind === "f64" ? 8 : floatKind === "f32" ? 4 : 8
    const resultReg = `%${this.valueCounter++}`
    ir.push(`  ${resultReg} = call ptr @array_alloc(i64 ${elementSize}, i64 1, ptr ${dimensionsReg})`)

    // Generate loop
    const loopStartLabel = `vec_loop_start_${this.labelCounter++}`
    const loopBodyLabel = `vec_loop_body_${this.labelCounter++}`
    const loopEndLabel = `vec_loop_end_${this.labelCounter++}`

    // Loop counter
    const counterReg = `%${this.valueCounter++}`
    ir.push(`  ${counterReg} = alloca i64`)
    ir.push(`  store i64 0, ptr ${counterReg}`)

    // Jump to loop start
    ir.push(`  br label %${loopStartLabel}`)

    // Loop start: check condition
    ir.push(`${loopStartLabel}:`)
    const currentCount = `%${this.valueCounter++}`
    ir.push(`  ${currentCount} = load i64, ptr ${counterReg}`)
    const cmpReg = `%${this.valueCounter++}`
    ir.push(`  ${cmpReg} = icmp slt i64 ${currentCount}, ${lenReg}`)
    ir.push(`  br i1 ${cmpReg}, label %${loopBodyLabel}, label %${loopEndLabel}`)

    // Loop body
    ir.push(`${loopBodyLabel}:`)

    // Get elements - one from array, one may be scalar
    let elem1Reg: string, elem2Reg: string

    if (floatKind === "f32") {
      if (isLeftArray) {
        elem1Reg = `%${this.valueCounter++}`
        ir.push(`  ${elem1Reg} = call float @array_get_f32(ptr ${leftReg}, i64 ${currentCount})`)
      } else {
        // Left is scalar - bitcast i64 to float (IEEE 754 representation)
        const tempReg = `%${this.valueCounter++}`
        ir.push(`  ${tempReg} = trunc i64 ${leftReg} to i32`)
        elem1Reg = `%${this.valueCounter++}`
        ir.push(`  ${elem1Reg} = bitcast i32 ${tempReg} to float`)
      }
      if (isRightArray) {
        elem2Reg = `%${this.valueCounter++}`
        ir.push(`  ${elem2Reg} = call float @array_get_f32(ptr ${rightReg}, i64 ${currentCount})`)
      } else {
        // Right is scalar - bitcast i64 to float (IEEE 754 representation)
        const tempReg = `%${this.valueCounter++}`
        ir.push(`  ${tempReg} = trunc i64 ${rightReg} to i32`)
        elem2Reg = `%${this.valueCounter++}`
        ir.push(`  ${elem2Reg} = bitcast i32 ${tempReg} to float`)
      }
    } else if (floatKind === "f64") {
      if (isLeftArray) {
        elem1Reg = `%${this.valueCounter++}`
        ir.push(`  ${elem1Reg} = call double @array_get_f64(ptr ${leftReg}, i64 ${currentCount})`)
      } else {
        // Left is scalar - convert i64 bit pattern to double
        // Store the i64 bit pattern to memory, then load as double
        const tempPtr = `%${this.valueCounter++}`
        ir.push(`  ${tempPtr} = alloca i64`)
        ir.push(`  store i64 ${leftReg}, ptr ${tempPtr}`)
        const doublePtr = `%${this.valueCounter++}`
        ir.push(`  ${doublePtr} = bitcast ptr ${tempPtr} to ptr`)
        elem1Reg = `%${this.valueCounter++}`
        ir.push(`  ${elem1Reg} = load double, ptr ${doublePtr}`)
      }
      if (isRightArray) {
        elem2Reg = `%${this.valueCounter++}`
        ir.push(`  ${elem2Reg} = call double @array_get_f64(ptr ${rightReg}, i64 ${currentCount})`)
      } else {
        // Right is scalar - convert i64 bit pattern to double
        const tempPtr = `%${this.valueCounter++}`
        ir.push(`  ${tempPtr} = alloca i64`)
        ir.push(`  store i64 ${rightReg}, ptr ${tempPtr}`)
        const doublePtr = `%${this.valueCounter++}`
        ir.push(`  ${doublePtr} = bitcast ptr ${tempPtr} to ptr`)
        elem2Reg = `%${this.valueCounter++}`
        ir.push(`  ${elem2Reg} = load double, ptr ${doublePtr}`)
      }
    } else {
      if (isLeftArray) {
        const e1 = `%${this.valueCounter++}`
        ir.push(`  ${e1} = call i64 @array_get(ptr ${leftReg}, i64 ${currentCount})`)
        elem1Reg = e1
      } else {
        elem1Reg = leftReg
      }
      if (isRightArray) {
        const e2 = `%${this.valueCounter++}`
        ir.push(`  ${e2} = call i64 @array_get(ptr ${rightReg}, i64 ${currentCount})`)
        elem2Reg = e2
      } else {
        elem2Reg = rightReg
      }
    }

    // Apply operation
    const resultElemReg = `%${this.valueCounter++}`
    const op = expr.operator

    if (floatKind === "f32") {
      const opMap: Record<string, string> = { "+": "fadd", "-": "fsub", "*": "fmul", "/": "fdiv" }
      const floatOp = opMap[op] || "fadd"
      ir.push(`  ${resultElemReg} = ${floatOp} float ${elem1Reg}, ${elem2Reg}`)
    } else if (floatKind === "f64") {
      const opMap: Record<string, string> = { "+": "fadd", "-": "fsub", "*": "fmul", "/": "fdiv" }
      const floatOp = opMap[op] || "fadd"
      ir.push(`  ${resultElemReg} = ${floatOp} double ${elem1Reg}, ${elem2Reg}`)
    } else {
      const opMap: Record<string, string> = { "+": "add i64", "-": "sub i64", "*": "mul i64", "/": "sdiv i64" }
      const intOp = opMap[op] || "add i64"
      ir.push(`  ${resultElemReg} = ${intOp} ${elem1Reg}, ${elem2Reg}`)
    }

    // Store result
    if (floatKind === "f32") {
      ir.push(`  call void @array_set_f32(ptr ${resultReg}, i64 ${currentCount}, float ${resultElemReg})`)
    } else if (floatKind === "f64") {
      ir.push(`  call void @array_set_f64(ptr ${resultReg}, i64 ${currentCount}, double ${resultElemReg})`)
    } else {
      ir.push(`  call void @array_set(ptr ${resultReg}, i64 ${currentCount}, i64 ${resultElemReg})`)
    }

    // Increment counter
    const nextCount = `%${this.valueCounter++}`
    ir.push(`  ${nextCount} = add i64 ${currentCount}, 1`)
    ir.push(`  store i64 ${nextCount}, ptr ${counterReg}`)

    // Jump back to start
    ir.push(`  br label %${loopStartLabel}`)

    // Loop end
    ir.push(`${loopEndLabel}:`)

    return resultReg
  }

  private isMatrixOperation(leftExpr: any, rightExpr: any): boolean {
    // Check if BOTH operands are 2D arrays (matrices)
    const leftArrayType = this.getExpressionArrayType(leftExpr)
    const rightArrayType = this.getExpressionArrayType(rightExpr)

    if (!leftArrayType || !rightArrayType) return false

    // Check if both are 2D (have 2 dimensions)
    const leftDims = leftExpr?.type === "Identifier" && leftExpr?.name
      ? this.variableTypes.get(leftExpr.name)?.dimensions?.length || leftArrayType.dimensions?.length
      : leftArrayType.dimensions?.length
    const rightDims = rightExpr?.type === "Identifier" && rightExpr?.name
      ? this.variableTypes.get(rightExpr.name)?.dimensions?.length || rightArrayType.dimensions?.length
      : rightArrayType.dimensions?.length

    // Treat as matrix if both have 2 dimensions
    return leftDims === 2 && rightDims === 2
  }

  private generateMatrixMultiplication(ir: string[], leftReg: string, rightReg: string): string {
    const resultReg = `%${this.valueCounter++}`
    ir.push(`  ${resultReg} = call ptr @matmul(ptr ${leftReg}, ptr ${rightReg})`)
    return resultReg
  }

  private generateMatrixAddition(ir: string[], leftReg: string, rightReg: string): string {
    const resultReg = `%${this.valueCounter++}`
    ir.push(`  ${resultReg} = call ptr @matrix_add(ptr ${leftReg}, ptr ${rightReg})`)
    return resultReg
  }

  private generateUnaryExpression(ir: string[], expr: any): string {
    const argument = expr.argument || expr.operand
    const value = this.generateExpression(ir, argument)
    const reg = `%${this.valueCounter++}`

    if (expr.operator === "-") {
      if (this.isFloatOperand(argument) || value.startsWith("0x")) {
        ir.push(`  ${reg} = fneg double ${value}`)
      } else {
        ir.push(`  ${reg} = sub i64 0, ${value}`)
      }
    } else if (expr.operator === "!") {
      ir.push(`  ${reg} = xor i64 ${value}, 1`)
    } else if (expr.operator === "~") {
      ir.push(`  ${reg} = xor i64 ${value}, -1`)
    }

    return reg
  }

  private generateCallExpression(ir: string[], expr: any): string {
    let positionalArgs = expr.args || expr.arguments || []
    const namedArgs: { name: string; value: any }[] = expr.namedArgs || []

    // Handle all([...]) and race([...]) builtins for structured concurrency
    if (expr.callee.type === "Identifier" && (expr.callee.name === "all" || expr.callee.name === "race")) {
      const arg = positionalArgs[0]
      if (arg && arg.type === "ArrayLiteral") {
        const elements = arg.elements || []

        if (this.isInAsyncFunction) {
          // Async context: use mog_all/mog_race combinators
          const count = elements.length
          if (count === 0) {
            return "0"
          }

          // Evaluate each future expression
          const futureRegs: string[] = []
          for (const elem of elements) {
            futureRegs.push(this.generateExpression(ir, elem))
          }

          // Store future pointers in a stack-allocated array
          const arrayReg = `%${this.valueCounter++}`
          ir.push(`  ${arrayReg} = alloca ptr, i32 ${count}`)
          for (let i = 0; i < count; i++) {
            const gep = `%${this.valueCounter++}`
            ir.push(`  ${gep} = getelementptr ptr, ptr ${arrayReg}, i32 ${i}`)
            ir.push(`  store ptr ${futureRegs[i]}, ptr ${gep}`)
          }

          // Call mog_all or mog_race
          const combFn = expr.callee.name === "all" ? "mog_all" : "mog_race"
          const resultFuture = `%${this.valueCounter++}`
          ir.push(`  ${resultFuture} = call ptr @${combFn}(ptr ${arrayReg}, i32 ${count})`)

          return resultFuture
        } else {
          // Synchronous fallback
          if (expr.callee.name === "race") {
            if (elements.length > 0) {
              return this.generateExpression(ir, elements[0])
            }
            return "0"
          } else {
            const results: string[] = []
            for (const elem of elements) {
              results.push(this.generateExpression(ir, elem))
            }
            return results.length > 0 ? results[results.length - 1] : "0"
          }
        }
      }
    }

    // If there are named args or default params, resolve the full argument list
    if (expr.callee.type === "Identifier") {
      const funcName = expr.callee.name
      const paramInfo = this.functionParamInfo.get(funcName)
      if (paramInfo && (namedArgs.length > 0 || paramInfo.some((p: any) => p.defaultValue))) {
        // Build the full resolved argument list
        const resolvedArgs: any[] = []
        for (let i = 0; i < paramInfo.length; i++) {
          const param = paramInfo[i]
          // Check if this param is covered by a positional arg
          if (i < positionalArgs.length) {
            resolvedArgs.push(positionalArgs[i])
          } else {
            // Check if it's in named args
            const namedArg = namedArgs.find((na: any) => na.name === param.name)
            if (namedArg) {
              resolvedArgs.push(namedArg.value)
            } else if (param.defaultValue) {
              // Use the default value expression
              resolvedArgs.push(param.defaultValue)
            }
            // If no value, skip (will be handled by LLVM with 0/default)
          }
        }
        positionalArgs = resolvedArgs
      }
    }

    const args = positionalArgs.map((arg: ExpressionNode) => this.generateExpression(ir, arg)).filter(Boolean)

    if (expr.callee.type === "Identifier") {
      const funcName = expr.callee.name

      // Handle print functions
      const printFunctions = ["print", "print_i64", "print_u64", "print_f64", "print_string", "print_buffer",
                              "println", "println_i64", "println_u64", "println_f64", "println_string"]
      if (printFunctions.includes(funcName)) {
        return this.generatePrintCall(ir, funcName, args, expr)
      }

      // Handle tensor_print function
      if (funcName === "tensor_print") {
        ir.push(`  call void @tensor_print(ptr ${args[0]})`)
        return "0"
      }

      // Handle tensor activation functions as free functions
      if (funcName === "relu" && args.length === 1 && this.isTensorType(positionalArgs[0])) {
        const reg = `%${this.valueCounter++}`
        ir.push(`  ${reg} = call ptr @tensor_relu(ptr ${args[0]})`)
        return reg
      }
      if (funcName === "sigmoid" && args.length === 1 && this.isTensorType(positionalArgs[0])) {
        const reg = `%${this.valueCounter++}`
        ir.push(`  ${reg} = call ptr @tensor_sigmoid(ptr ${args[0]})`)
        return reg
      }
      if (funcName === "tanh" && args.length === 1 && this.isTensorType(positionalArgs[0])) {
        const reg = `%${this.valueCounter++}`
        ir.push(`  ${reg} = call ptr @tensor_tanh_act(ptr ${args[0]})`)
        return reg
      }
      if (funcName === "softmax" && this.isTensorType(positionalArgs[0])) {
        const dimArg = args.length > 1 ? args[1] : "-1"
        const reg = `%${this.valueCounter++}`
        ir.push(`  ${reg} = call ptr @tensor_softmax(ptr ${args[0]}, i64 ${dimArg})`)
        return reg
      }

      // Handle input functions
      const inputFunctions = ["input_i64", "input_u64", "input_f64", "input_string"]
      if (inputFunctions.includes(funcName)) {
        return this.generateInputCall(ir, funcName)
      }

      // Handle dot product function
      if (funcName === "dot") {
        return this.generateDotCall(ir, expr, args)
      }

      // Handle matmul function
      if (funcName === "matmul") {
        return this.generateMatmulCall(ir, expr, args)
      }

      // Handle GC benchmark functions
      const gcBenchmarkFunctions = ["gc_reset_stats", "gc_benchmark_stats"]
      if (gcBenchmarkFunctions.includes(funcName)) {
        ir.push(`  call void @${funcName}()`)
        return "0"
      }

      // Handle math builtin functions
      const mathSingleArg: Record<string, string> = {
        sqrt: "@llvm.sqrt.f64",
        sin: "@llvm.sin.f64",
        cos: "@llvm.cos.f64",
        tan: "@tan",
        asin: "@asin",
        acos: "@acos",
        exp: "@llvm.exp.f64",
        log: "@llvm.log.f64",
        log2: "@llvm.log2.f64",
        floor: "@llvm.floor.f64",
        ceil: "@llvm.ceil.f64",
        round: "@llvm.round.f64",
        abs: "@llvm.fabs.f64",
      }
      const mathTwoArg: Record<string, string> = {
        pow: "@llvm.pow.f64",
        atan2: "@atan2",
        min: "@llvm.minnum.f64",
        max: "@llvm.maxnum.f64",
      }

      if (mathSingleArg[funcName]) {
        const argReg = args[0]
        const argExpr = positionalArgs[0]
        const doubleArg = this.toDoubleReg(ir, argReg, argExpr)
        const resultReg = `%${this.valueCounter++}`
        ir.push(`  ${resultReg} = call double ${mathSingleArg[funcName]}(double ${doubleArg})`)
        // Return double directly — f64 variables store as double, float ops consume double
        return resultReg
      }

      if (mathTwoArg[funcName]) {
        const argExpr0 = positionalArgs[0]
        const argExpr1 = positionalArgs[1]
        const doubleArg0 = this.toDoubleReg(ir, args[0], argExpr0)
        const doubleArg1 = this.toDoubleReg(ir, args[1], argExpr1)
        const resultReg = `%${this.valueCounter++}`
        ir.push(`  ${resultReg} = call double ${mathTwoArg[funcName]}(double ${doubleArg0}, double ${doubleArg1})`)
        // Return double directly — f64 variables store as double, float ops consume double
        return resultReg
      }

      // Handle str() conversion function
      if (funcName === "str") {
        const argReg = args[0]
        // Check if the argument is a float type
        const argExpr = positionalArgs[0]
        const isFloat = argExpr?.inferredType?.type === "FloatType" || 
                        argExpr?.type === "FloatLiteral" ||
                        (argExpr?.type === "Identifier" && this.variableTypes.get(argExpr.name)?.type === "FloatType")
        if (isFloat) {
          const doubleReg = this.toDoubleReg(ir, argReg, argExpr)
          const reg = `%${this.valueCounter++}`
          ir.push(`  ${reg} = call ptr @f64_to_string(double ${doubleReg})`)
          return reg
        } else {
          const reg = `%${this.valueCounter++}`
          ir.push(`  ${reg} = call ptr @i64_to_string(i64 ${argReg})`)
          return reg
        }
      }

      // Handle int_from_string() conversion
      if (funcName === "int_from_string") {
        const reg = `%${this.valueCounter++}`
        ir.push(`  ${reg} = call ptr @int_from_string(ptr ${args[0]})`)
        return reg
      }

      // Handle float_from_string() conversion
      if (funcName === "float_from_string") {
        const reg = `%${this.valueCounter++}`
        ir.push(`  ${reg} = call ptr @float_from_string(ptr ${args[0]})`)
        return reg
      }

      // Check if this is a variable holding a closure (not a direct function call)
      const callerVarType = this.variableTypes.get(funcName)
      if (callerVarType && (callerVarType.type === "FunctionType" || callerVarType instanceof FunctionType) && !this.functions.has(funcName)) {
        // This is an indirect call through a closure variable - delegate to the closure call path below
        // (falls through to the indirect call section at the end of this method)
      } else {

      const funcInfo = this.functions.get(funcName)

      // POSIX function signatures for proper argument typing
      const posixSignatures: Record<string, string[]> = {
        open: ["ptr", "i64"],  // variadic: may have mode
        read: ["i64", "ptr", "i64"],
        write: ["i64", "ptr", "i64"],
        pread: ["i64", "ptr", "i64", "i64"],
        pwrite: ["i64", "ptr", "i64", "i64"],
        lseek: ["i64", "i64", "i64"],
        close: ["i64"],
        fsync: ["i64"],
        fdatasync: ["i64"],
        stat: ["ptr", "ptr"],
        lstat: ["ptr", "ptr"],
        fstat: ["i64", "ptr"],
        access: ["ptr", "i64"],
        faccessat: ["i64", "ptr", "i64", "i64"],
        utimes: ["ptr", "ptr"],
        futimes: ["i64", "ptr"],
        utimensat: ["i64", "ptr", "ptr", "i64"],
        chmod: ["ptr", "i64"],
        fchmod: ["i64", "i64"],
        chown: ["ptr", "i64", "i64"],
        fchown: ["i64", "i64", "i64"],
        umask: ["i64"],
        truncate: ["ptr", "i64"],
        ftruncate: ["i64", "i64"],
        link: ["ptr", "ptr"],
        symlink: ["ptr", "ptr"],
        readlink: ["ptr", "ptr", "i64"],
        rename: ["ptr", "ptr"],
        unlink: ["ptr"],
        mkdir: ["ptr", "i64"],
        rmdir: ["ptr"],
        fcntl: ["i64", "i64"],  // variadic
        pathconf: ["ptr", "i64"],
        fpathconf: ["i64", "i64"],
        dup: ["i64"],
        dup2: ["i64", "i64"],
        creat: ["ptr", "i64"],
        mkfifo: ["ptr", "i64"],
        mknod: ["ptr", "i64", "i64"],  // variadic
        chdir: ["ptr"],
        fchdir: ["i64"],
        getcwd: ["ptr", "i64"],
        opendir: ["ptr"],
        readdir: ["ptr"],
        closedir: ["ptr"],
        rewinddir: ["ptr"],
      }

      const runtimeSignatures: Record<string, { params: string[]; ret: string }> = {
        map_new: { params: ["i64"], ret: "ptr" },
        map_get: { params: ["ptr", "ptr", "i64"], ret: "i64" },
        map_set: { params: ["ptr", "ptr", "i64", "i64"], ret: "void" },
    map_has: { params: ["ptr", "ptr", "i64"], ret: "i64" },
    map_size: { params: ["ptr"], ret: "i64" },
    map_key_at: { params: ["ptr", "i64"], ret: "ptr" },
        map_value_at: { params: ["ptr", "i64"], ret: "i64" },
        string_length: { params: ["ptr"], ret: "i64" },
        string_concat: { params: ["ptr", "ptr"], ret: "ptr" },
        string_upper: { params: ["ptr"], ret: "ptr" },
        string_lower: { params: ["ptr"], ret: "ptr" },
        string_trim: { params: ["ptr"], ret: "ptr" },
        string_split: { params: ["ptr", "ptr"], ret: "ptr" },
        string_contains: { params: ["ptr", "ptr"], ret: "i64" },
        string_starts_with: { params: ["ptr", "ptr"], ret: "i64" },
        string_ends_with: { params: ["ptr", "ptr"], ret: "i64" },
        string_replace: { params: ["ptr", "ptr", "ptr"], ret: "ptr" },
        int_from_string: { params: ["ptr"], ret: "ptr" },
        float_from_string: { params: ["ptr"], ret: "ptr" },
        i64_to_string: { params: ["i64"], ret: "ptr" },
        f64_to_string: { params: ["double"], ret: "ptr" },
        gc_alloc: { params: ["i64"], ret: "ptr" },
        stdin_poll: { params: ["i64"], ret: "i64" },
        stdin_read_line: { params: [], ret: "ptr" },
        time_ms: { params: [], ret: "i64" },
        string_eq: { params: ["ptr", "ptr"], ret: "i64" },
        flush_stdout: { params: [], ret: "void" },
        parse_int: { params: ["ptr"], ret: "i64" },
        parse_float: { params: ["ptr"], ret: "double" },
        async_read_line: { params: [], ret: "ptr" },
      }

      const sig = posixSignatures[funcName]
      const isVariadic = ["open", "fcntl", "mknod"].includes(funcName)

      // POSIX functions that return ptr instead of i64
      const posixReturnTypes: Record<string, string> = {
        opendir: "ptr",
        readdir: "ptr",
      }

      let typedArgs: string[]
      if (funcInfo && funcInfo.params) {
        typedArgs = args.map((arg: string, i: number) => {
          const paramType = funcInfo.params[i]?.type || "i64"
          return `${paramType} ${arg}`
        })
      } else if (sig) {
        // Use POSIX signature for typing
        typedArgs = args.map((arg: string, i: number) => {
          const paramType = sig[i] || "i64"
          return `${paramType} ${arg}`
        })
      } else if (runtimeSignatures[funcName]) {
        const sig = runtimeSignatures[funcName]
        typedArgs = args.map((arg: string, i: number) => {
          const paramType = sig.params[i] || "i64"
          return `${paramType} ${arg}`
        })
        const returnType = sig.ret
        const reg = returnType === "void" ? null : `%${this.valueCounter++}`
        if (reg) {
          ir.push(`  ${reg} = call ${returnType} @${funcName}(${typedArgs.join(", ")})`)
        } else {
          ir.push(`  call ${returnType} @${funcName}(${typedArgs.join(", ")})`)
        }
        return reg || ""
      } else if (isVariadic) {
        // For variadic functions without sig, type first 2 args, rest as i64
        typedArgs = args.map((arg: string, i: number) => {
          if (i === 0) return `ptr ${arg}`
          return `i64 ${arg}`
        })
      } else {
        typedArgs = args
      }

      const returnType = funcInfo?.returnType || posixReturnTypes[funcName] || "i64"
      if (returnType === "void") {
        ir.push(`  call ${returnType} @${funcName}(${typedArgs.join(", ")})`)
        return ""
      }
      const reg = `%${this.valueCounter++}`
      ir.push(`  ${reg} = call ${returnType} @${funcName}(${typedArgs.join(", ")})`)
      return reg
      } // end of else block for non-closure direct calls
    }

    // Handle capability function calls (MemberExpression like fs.read(...))
    if (expr.callee.type === "MemberExpression") {
      const memberExpr = expr.callee

      // Package-qualified function call: pkg.func(args)
      if (memberExpr.object?.type === "Identifier" && this.importedPackages.has(memberExpr.object.name)) {
        const pkgName = memberExpr.object.name
        const mangledName = `${pkgName}__${memberExpr.property}`
        const funcInfo = this.functions.get(mangledName)
        let typedArgs: string[]
        if (funcInfo && funcInfo.params) {
          typedArgs = args.map((arg: string, i: number) => {
            const paramType = funcInfo.params[i]?.type || "i64"
            return `${paramType} ${arg}`
          })
        } else {
          typedArgs = args.map((arg: string) => `i64 ${arg}`)
        }
        const reg = `%${this.valueCounter++}`
        const returnType = funcInfo?.returnType || "i64"
        ir.push(`  ${reg} = call ${returnType} @${mangledName}(${typedArgs.join(", ")})`)
        return reg
      }

      // Check if this is a string method call (e.g., s.upper(), s.lower())
      if (this.isStringType(memberExpr.object)) {
        const strReg = this.generateExpression(ir, memberExpr.object)
        const method = memberExpr.property
        const callArgs = expr.args || expr.arguments || []

        switch (method) {
          case "upper": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @string_upper(ptr ${strReg})`)
            return reg
          }
          case "lower": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @string_lower(ptr ${strReg})`)
            return reg
          }
          case "trim": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @string_trim(ptr ${strReg})`)
            return reg
          }
          case "split": {
            const delimReg = callArgs.length > 0 ? this.generateExpression(ir, callArgs[0]) : this.generateStringLiteral(ir, "")
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @string_split(ptr ${strReg}, ptr ${delimReg})`)
            return reg
          }
          case "contains": {
            const subReg = this.generateExpression(ir, callArgs[0])
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call i64 @string_contains(ptr ${strReg}, ptr ${subReg})`)
            return reg
          }
          case "starts_with": {
            const prefixReg = this.generateExpression(ir, callArgs[0])
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call i64 @string_starts_with(ptr ${strReg}, ptr ${prefixReg})`)
            return reg
          }
          case "ends_with": {
            const suffixReg = this.generateExpression(ir, callArgs[0])
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call i64 @string_ends_with(ptr ${strReg}, ptr ${suffixReg})`)
            return reg
          }
          case "replace": {
            const oldReg = this.generateExpression(ir, callArgs[0])
            const newReg = this.generateExpression(ir, callArgs[1])
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @string_replace(ptr ${strReg}, ptr ${oldReg}, ptr ${newReg})`)
            return reg
          }
          case "len": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call i64 @string_length(ptr ${strReg})`)
            return reg
          }
        }
      }

      // Check if this is an array method call (e.g., arr.push(5), arr.pop())
      if (this.isArrayType(memberExpr.object)) {
        const arrReg = this.generateExpression(ir, memberExpr.object)
        const method = memberExpr.property
        const callArgs = expr.args || expr.arguments || []

        switch (method) {
          case "push": {
            const valueReg = this.generateExpression(ir, callArgs[0])
            // Check if the value is a float register — bitcast to i64 for array storage
            const isFloatValue = this.isFloatProducingExpression(callArgs[0])
            // Check if value is a pointer type (struct, array, string, etc.) — ptrtoint to i64
            const isPtrValue = this.isPtrProducingExpression(callArgs[0])
            if (isFloatValue) {
              // Value is a double register, bitcast to i64 for storage
              const castReg = `%${this.valueCounter++}`
              ir.push(`  ${castReg} = bitcast double ${valueReg} to i64`)
              ir.push(`  call void @array_push(ptr ${arrReg}, i64 ${castReg})`)
            } else if (isPtrValue) {
              // Value is a pointer (struct, array, etc.), ptrtoint to i64 for storage
              const castReg = `%${this.valueCounter++}`
              ir.push(`  ${castReg} = ptrtoint ptr ${valueReg} to i64`)
              ir.push(`  call void @array_push(ptr ${arrReg}, i64 ${castReg})`)
            } else {
              // Value is already i64 (either integer or float bit pattern)
              ir.push(`  call void @array_push(ptr ${arrReg}, i64 ${valueReg})`)
            }
            return "0" // void return
          }
          case "pop": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call i64 @array_pop(ptr ${arrReg})`)
            return reg
          }
          case "contains": {
            const valueReg = this.generateExpression(ir, callArgs[0])
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call i64 @array_contains(ptr ${arrReg}, i64 ${valueReg})`)
            return reg
          }
          case "sort": {
            if (callArgs.length > 0) {
              // Sort with comparator closure
              const closureReg = this.generateExpression(ir, callArgs[0])
              const fnPtr = `%${this.valueCounter++}`
              ir.push(`  ${fnPtr} = load ptr, ptr ${closureReg}`)
              const envSlot = `%${this.valueCounter++}`
              ir.push(`  ${envSlot} = getelementptr i8, ptr ${closureReg}, i64 8`)
              const envPtr = `%${this.valueCounter++}`
              ir.push(`  ${envPtr} = load ptr, ptr ${envSlot}`)
              const envI64 = `%${this.valueCounter++}`
              ir.push(`  ${envI64} = ptrtoint ptr ${envPtr} to i64`)
              ir.push(`  call void @array_sort_with_comparator(ptr ${arrReg}, ptr ${fnPtr}, i64 ${envI64})`)
            } else {
              ir.push(`  call void @array_sort(ptr ${arrReg})`)
            }
            return "0" // void return
          }
          case "reverse": {
            ir.push(`  call void @array_reverse(ptr ${arrReg})`)
            return "0" // void return
          }
          case "join": {
            const sepReg = this.generateExpression(ir, callArgs[0])
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @array_join(ptr ${arrReg}, ptr ${sepReg})`)
            return reg
          }
          case "len": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call i64 @array_length(ptr ${arrReg})`)
            return reg
          }
          case "filter":
          case "map": {
            // The closure argument is a 16-byte struct: {fn_ptr, env_ptr}
            const closureReg = this.generateExpression(ir, callArgs[0])
            // Extract fn_ptr from closure[0]
            const fnPtr = `%${this.valueCounter++}`
            ir.push(`  ${fnPtr} = load ptr, ptr ${closureReg}`)
            // Extract env_ptr from closure[8]
            const envSlot = `%${this.valueCounter++}`
            ir.push(`  ${envSlot} = getelementptr i8, ptr ${closureReg}, i64 8`)
            const envPtr = `%${this.valueCounter++}`
            ir.push(`  ${envPtr} = load ptr, ptr ${envSlot}`)
            // Cast env_ptr to i64 for the C runtime call
            const envI64 = `%${this.valueCounter++}`
            ir.push(`  ${envI64} = ptrtoint ptr ${envPtr} to i64`)
            const reg = `%${this.valueCounter++}`
            const rtName = method === "filter" ? "array_filter" : "array_map"
            ir.push(`  ${reg} = call ptr @${rtName}(ptr ${arrReg}, ptr ${fnPtr}, i64 ${envI64})`)
            return reg
          }
        }
      }

      // Handle map method calls (e.g., m.has(key))
      if (this.isMapType(memberExpr.object)) {
        const mapReg = this.generateExpression(ir, memberExpr.object)
        const method = memberExpr.property
        const callArgs = expr.args || expr.arguments || []

        switch (method) {
          case "has": {
            const keyArg = callArgs[0]
            let keyPtr: string
            let keyLen: string

            if (keyArg.type === "StringLiteral") {
              const { escaped, byteLength } = this.escapeStringForLLVM(keyArg.value)
              keyPtr = `@key_str_${this.stringCounter++}`
              const strDef = `${keyPtr} = private unnamed_addr constant [${byteLength + 1} x i8] c"${escaped}\\00"`
              this.stringConstants.push(strDef)
              keyLen = String(byteLength)
            } else if (keyArg.type === "IntegerLiteral" || keyArg.type === "NumberLiteral") {
              const keyVal = this.generateExpression(ir, keyArg)
              keyPtr = `%${this.valueCounter++}`
              ir.push(`  ${keyPtr} = alloca i64`)
              ir.push(`  store i64 ${keyVal}, ptr ${keyPtr}`)
              keyLen = "8"
            } else {
              keyPtr = this.generateExpression(ir, keyArg)
              const lenReg = `%${this.valueCounter++}`
              ir.push(`  ${lenReg} = call i64 @string_length(ptr ${keyPtr})`)
              keyLen = lenReg
            }

            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call i64 @map_has(ptr ${mapReg}, ptr ${keyPtr}, i64 ${keyLen})`)
            return reg
          }
        }
      }

      // Handle tensor static constructors: tensor.zeros(), tensor.ones(), tensor.randn()
      if (memberExpr.object.type === "Identifier" && memberExpr.object.name === "tensor") {
        const method = memberExpr.property
        const callArgs = expr.args || expr.arguments || []
        if (method === "zeros" || method === "ones" || method === "randn") {
          const shapeArg = callArgs[0]
          if (shapeArg && shapeArg.type === "ArrayLiteral") {
            const shapeElements = shapeArg.elements || []
            const ndim = shapeElements.length
            const shapeAlloca = `%${this.valueCounter++}`
            ir.push(`  ${shapeAlloca} = alloca [${ndim} x i64]`)
            for (let i = 0; i < ndim; i++) {
              const dimVal = this.generateExpression(ir, shapeElements[i])
              const gepReg = `%${this.valueCounter++}`
              ir.push(`  ${gepReg} = getelementptr [${ndim} x i64], ptr ${shapeAlloca}, i64 0, i64 ${i}`)
              ir.push(`  store i64 ${dimVal}, ptr ${gepReg}`)
            }
            const funcName = `tensor_${method}`
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @${funcName}(ptr ${shapeAlloca}, i64 ${ndim})`)
            return reg
          }
        }
      }

      // Check if this is a tensor method call
      if (this.isTensorType(memberExpr.object)) {
        const tensorReg = this.generateExpression(ir, memberExpr.object)
        const method = memberExpr.property
        const callArgs = expr.args || expr.arguments || []

        switch (method) {
          case "sum": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call double @tensor_sum(ptr ${tensorReg})`)
            return reg
          }
          case "mean": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call double @tensor_mean(ptr ${tensorReg})`)
            return reg
          }
          case "matmul": {
            const otherReg = this.generateExpression(ir, callArgs[0])
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @tensor_matmul(ptr ${tensorReg}, ptr ${otherReg})`)
            return reg
          }
          case "reshape": {
            // reshape takes shape as array literal
            const shapeArg = callArgs[0]
            if (shapeArg && shapeArg.type === "ArrayLiteral") {
              const shapeElements = shapeArg.elements || []
              const ndim = shapeElements.length
              const shapeAlloca = `%${this.valueCounter++}`
              ir.push(`  ${shapeAlloca} = alloca [${ndim} x i64]`)
              for (let i = 0; i < ndim; i++) {
                const dimVal = this.generateExpression(ir, shapeElements[i])
                const gepReg = `%${this.valueCounter++}`
                ir.push(`  ${gepReg} = getelementptr [${ndim} x i64], ptr ${shapeAlloca}, i64 0, i64 ${i}`)
                ir.push(`  store i64 ${dimVal}, ptr ${gepReg}`)
              }
              const reg = `%${this.valueCounter++}`
              ir.push(`  ${reg} = call ptr @tensor_reshape(ptr ${tensorReg}, i64 ${ndim}, ptr ${shapeAlloca})`)
              return reg
            }
            return tensorReg
          }
          case "print": {
            ir.push(`  call void @tensor_print(ptr ${tensorReg})`)
            return "0"
          }
          case "shape": {
            // .shape(i) as method call returns dimension i
            if (callArgs.length > 0) {
              const dimReg = this.generateExpression(ir, callArgs[0])
              const reg = `%${this.valueCounter++}`
              ir.push(`  ${reg} = call i64 @tensor_shape_dim(ptr ${tensorReg}, i64 ${dimReg})`)
              return reg
            }
            return "0"
          }
          case "ndim": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call i64 @tensor_ndim(ptr ${tensorReg})`)
            return reg
          }
          case "size": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call i64 @tensor_size(ptr ${tensorReg})`)
            return reg
          }
          case "relu": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @tensor_relu(ptr ${tensorReg})`)
            return reg
          }
          case "sigmoid": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @tensor_sigmoid(ptr ${tensorReg})`)
            return reg
          }
          case "tanh": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @tensor_tanh_act(ptr ${tensorReg})`)
            return reg
          }
          case "softmax": {
            const dimArg = callArgs.length > 0 ? this.generateExpression(ir, callArgs[0]) : "-1"
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @tensor_softmax(ptr ${tensorReg}, i64 ${dimArg})`)
            return reg
          }
          case "transpose": {
            const reg = `%${this.valueCounter++}`
            ir.push(`  ${reg} = call ptr @tensor_transpose(ptr ${tensorReg})`)
            return reg
          }
        }
      }

      if (memberExpr.object.type === "Identifier" && this.capabilities.has(memberExpr.object.name)) {
        return this.generateCapabilityCall(ir, memberExpr.object.name, memberExpr.property, args, expr)
      }
    }

    // Handle indirect function call (calling a closure stored in a variable)
    if (expr.callee.type === "Identifier") {
      const funcPtrName = expr.callee.name

      // Determine the correct alloca name: parameters use %name_local, variables use %name
      const isParam = this.currentFunction && this.currentFunction.params.find((p) => p.name === funcPtrName)
      const allocaName = isParam ? `%${funcPtrName}_local` : `%${funcPtrName}`

      // Load the closure struct pointer from the variable
      const closureReg = `%${this.valueCounter++}`
      ir.push(`  ${closureReg} = load ptr, ptr ${allocaName}`)

      // Extract fn_ptr from closure[0]
      const fnPtrReg = `%${this.valueCounter++}`
      ir.push(`  ${fnPtrReg} = load ptr, ptr ${closureReg}`)

      // Extract env_ptr from closure[1] (offset 8)
      const envGep = `%${this.valueCounter++}`
      ir.push(`  ${envGep} = getelementptr i8, ptr ${closureReg}, i64 8`)
      const envReg = `%${this.valueCounter++}`
      ir.push(`  ${envReg} = load ptr, ptr ${envGep}`)

      // Build typed args with env as first arg
      // Try to get actual param types from FunctionType if available
      const varType = this.variableTypes.get(funcPtrName)
      let typedUserArgs: string[]
      if (varType && varType.paramTypes) {
        // We have a FunctionType with known param types
        typedUserArgs = args.map((arg: string, i: number) => {
          const paramType = varType.paramTypes[i]
          if (paramType) {
            const llvmType = this.toLLVMType(paramType)
            return `${llvmType} ${arg}`
          }
          return `i64 ${arg}`
        })
      } else {
        typedUserArgs = args.map((arg: string) => `i64 ${arg}`)
      }

      const allArgs = [`ptr ${envReg}`, ...typedUserArgs]

      // Determine return type
      let retType = "i64"
      if (varType && varType.returnType) {
        retType = this.toLLVMType(varType.returnType)
      }

      const resultReg = `%${this.valueCounter++}`
      ir.push(`  ${resultReg} = call ${retType} ${fnPtrReg}(${allArgs.join(", ")})`)
      return resultReg
    }

    return ""
  }

  private generateCapabilityCall(ir: string[], capName: string, funcName: string, args: string[], expr: any): string {
    // Create capability name string on stack
    const capStrAlloca = `%${this.valueCounter++}`
    ir.push(`  ${capStrAlloca} = alloca [${capName.length + 1} x i8]`)
    ir.push(`  store [${capName.length + 1} x i8] c"${capName}\\00", ptr ${capStrAlloca}`)

    // Create function name string on stack
    const funcStrAlloca = `%${this.valueCounter++}`
    ir.push(`  ${funcStrAlloca} = alloca [${funcName.length + 1} x i8]`)
    ir.push(`  store [${funcName.length + 1} x i8] c"${funcName}\\00", ptr ${funcStrAlloca}`)

    const nargs = args.length
    if (nargs > 0) {
      // Allocate MogValue array for arguments
      const argsArrayReg = `%${this.valueCounter++}`
      ir.push(`  ${argsArrayReg} = alloca %MogValue, i32 ${nargs}`)

      // Initialize each MogValue argument
      const argExprs = expr.args || expr.arguments || []
      for (let i = 0; i < nargs; i++) {
        const argExpr = argExprs[i]
        const argVal = args[i]

        // Determine tag from the expression type
        const isFloat = this.isFloatOperand(argExpr)
        const isString = argExpr && (argExpr.type === "StringLiteral" || argExpr.type === "TemplateLiteral")

        const gepTag = `%${this.valueCounter++}`
        ir.push(`  ${gepTag} = getelementptr %MogValue, ptr ${argsArrayReg}, i32 ${i}, i32 0`)

        const gepData = `%${this.valueCounter++}`
        ir.push(`  ${gepData} = getelementptr %MogValue, ptr ${argsArrayReg}, i32 ${i}, i32 1`)

        if (isString) {
          // MOG_STRING = 3
          ir.push(`  store i32 3, ptr ${gepTag}`)
          const ptrToInt = `%${this.valueCounter++}`
          ir.push(`  ${ptrToInt} = ptrtoint ptr ${argVal} to i64`)
          ir.push(`  store i64 ${ptrToInt}, ptr ${gepData}`)
        } else if (isFloat) {
          // MOG_FLOAT = 1
          ir.push(`  store i32 1, ptr ${gepTag}`)
          const bitcast = `%${this.valueCounter++}`
          ir.push(`  ${bitcast} = bitcast double ${argVal} to i64`)
          ir.push(`  store i64 ${bitcast}, ptr ${gepData}`)
        } else {
          // MOG_INT = 0 (default for integers)
          ir.push(`  store i32 0, ptr ${gepTag}`)
          ir.push(`  store i64 ${argVal}, ptr ${gepData}`)
        }
      }

      // Call mog_cap_call_out with explicit output pointer (avoids ARM64 ABI issues)
      const outAlloca = `%${this.valueCounter++}`
      ir.push(`  ${outAlloca} = alloca %MogValue, align 8`)
      ir.push(`  call void @mog_cap_call_out(ptr ${outAlloca}, ptr null, ptr ${capStrAlloca}, ptr ${funcStrAlloca}, ptr ${argsArrayReg}, i32 ${nargs})`)

      // Load the data field (at byte offset 8: skip tag i32 + 4 bytes padding)
      const dataGep = `%${this.valueCounter++}`
      ir.push(`  ${dataGep} = getelementptr inbounds %MogValue, ptr ${outAlloca}, i32 0, i32 1`)
      const extractReg = `%${this.valueCounter++}`
      ir.push(`  ${extractReg} = load i64, ptr ${dataGep}, align 8`)

      return extractReg
    } else {
      // No arguments - call with explicit output pointer
      const outAlloca = `%${this.valueCounter++}`
      ir.push(`  ${outAlloca} = alloca %MogValue, align 8`)
      ir.push(`  call void @mog_cap_call_out(ptr ${outAlloca}, ptr null, ptr ${capStrAlloca}, ptr ${funcStrAlloca}, ptr null, i32 0)`)

      // Load the data field (at byte offset 8: skip tag i32 + 4 bytes padding)
      const dataGep = `%${this.valueCounter++}`
      ir.push(`  ${dataGep} = getelementptr inbounds %MogValue, ptr ${outAlloca}, i32 0, i32 1`)
      const extractReg = `%${this.valueCounter++}`
      ir.push(`  ${extractReg} = load i64, ptr ${dataGep}, align 8`)

      return extractReg
    }
  }

   private generateCapabilityDeclarations(ir: string[]): void {
    ir.push("")
    ir.push("; Host FFI declarations")
    ir.push("%MogValue = type { i32, { { ptr, ptr } } }")  // tag(4) + pad(4) + union{handle{ptr,ptr}}(16) = 24 bytes
    // Use explicit output pointer to avoid ABI issues with large struct returns on ARM64.
    // mog_cap_call_out(MogValue *out, MogVM *vm, const char *cap, const char *func, MogValue *args, int nargs)
    ir.push("declare void @mog_cap_call_out(ptr, ptr, ptr, ptr, ptr, i32)")
    // VM lifecycle — needed to initialize capabilities before use
    ir.push("declare ptr @mog_vm_new()")
    ir.push("declare ptr @mog_vm_get_global()")
    ir.push("declare void @mog_vm_set_global(ptr)")
    ir.push("declare void @mog_register_posix_host(ptr)")
    ir.push("declare void @mog_vm_free(ptr)")
    ir.push("")
    this.capCallDeclared = true
  }

  private generateInputCall(ir: string[], funcName: string): string {
    const reg = `%${this.valueCounter++}`

    switch (funcName) {
      case "input_i64":
      case "input_u64":
        ir.push(`  ${reg} = call i64 @${funcName}()`)
        return reg
      case "input_f64":
        ir.push(`  ${reg} = call double @${funcName}()`)
        return reg
      case "input_string":
        ir.push(`  ${reg} = call ptr @${funcName}()`)
        return reg
      default:
        return ""
    }
  }

  private generateDotCall(ir: string[], expr: any, args: string[]): string {
    // Get the argument nodes to determine element type
    const arg0 = expr.args?.[0] || expr.arguments?.[0]

    // Determine element type from first argument
    let elementType = "f64" // default
    const argType = arg0?.resultType
    if (argType?.elementType?.type === "FloatType" && argType?.elementType?.precision === 32) {
      elementType = "f32"
    }

    const reg = `%${this.valueCounter++}`
    if (elementType === "f32") {
      ir.push(`  ${reg} = call float @dot_f32(ptr ${args[0]}, ptr ${args[1]})`)
    } else {
      ir.push(`  ${reg} = call double @dot_f64(ptr ${args[0]}, ptr ${args[1]})`)
    }
    return reg
  }

  private generateMatmulCall(ir: string[], expr: any, args: string[]): string {
    // Check if arguments are tensors (TensorType) - use tensor_matmul
    const arg0Expr = expr.args?.[0] || expr.arguments?.[0]
    if (arg0Expr && this.isTensorType(arg0Expr)) {
      const reg = `%${this.valueCounter++}`
      ir.push(`  ${reg} = call ptr @tensor_matmul(ptr ${args[0]}, ptr ${args[1]})`)
      return reg
    }
    // matmul takes two arrays and returns a new array (result of matrix multiplication)
    // Both arguments should be arrays
    const reg = `%${this.valueCounter++}`
    ir.push(`  ${reg} = call ptr @matmul(ptr ${args[0]}, ptr ${args[1]})`)
    return reg
  }

  private generateTensorConstruction(ir: string[], expr: any): string {
    const args = expr.args || []
    const dtypeNum = this.tensorDtypeToInt(expr.dtype)

    if (args.length === 0) {
      // tensor<f32>() - no args, return empty 0-dim tensor
      const shapeAlloca = `%${this.valueCounter++}`
      ir.push(`  ${shapeAlloca} = alloca i64`)
      ir.push(`  store i64 0, ptr ${shapeAlloca}`)
      const reg = `%${this.valueCounter++}`
      ir.push(`  ${reg} = call ptr @tensor_create(i64 0, ptr ${shapeAlloca}, i64 ${dtypeNum})`)
      return reg
    }

    // First arg should be shape (ArrayLiteral)
    const shapeArg = args[0]

    if (shapeArg.type === "ArrayLiteral") {
      const shapeElements = shapeArg.elements || []
      const ndim = shapeElements.length

      // Allocate shape array on stack
      const shapeAlloca = `%${this.valueCounter++}`
      ir.push(`  ${shapeAlloca} = alloca [${ndim} x i64]`)

      // Store each dimension value
      for (let i = 0; i < ndim; i++) {
        const dimVal = this.generateExpression(ir, shapeElements[i])
        const gepReg = `%${this.valueCounter++}`
        ir.push(`  ${gepReg} = getelementptr [${ndim} x i64], ptr ${shapeAlloca}, i64 0, i64 ${i}`)
        ir.push(`  store i64 ${dimVal}, ptr ${gepReg}`)
      }

      if (args.length >= 2 && args[1].type === "ArrayLiteral") {
        // tensor<f32>([2, 3], [1,2,3,4,5,6]) - with data
        const dataElements = args[1].elements || []
        const dataCount = dataElements.length

        // Allocate data array on stack as floats
        const dataAlloca = `%${this.valueCounter++}`
        ir.push(`  ${dataAlloca} = alloca [${dataCount} x float]`)

        for (let i = 0; i < dataCount; i++) {
          const elemVal = this.generateExpression(ir, dataElements[i])
          const floatVal = this.toFloatReg(ir, elemVal, dataElements[i])
          const gepReg = `%${this.valueCounter++}`
          ir.push(`  ${gepReg} = getelementptr [${dataCount} x float], ptr ${dataAlloca}, i64 0, i64 ${i}`)
          ir.push(`  store float ${floatVal}, ptr ${gepReg}`)
        }

        const reg = `%${this.valueCounter++}`
        ir.push(`  ${reg} = call ptr @tensor_create_with_data(i64 ${ndim}, ptr ${shapeAlloca}, ptr ${dataAlloca}, i64 ${dtypeNum})`)
        return reg
      } else {
        // tensor<f32>([3, 4]) - zeros
        const reg = `%${this.valueCounter++}`
        ir.push(`  ${reg} = call ptr @tensor_create(i64 ${ndim}, ptr ${shapeAlloca}, i64 ${dtypeNum})`)
        return reg
      }
    }

    // Fallback: evaluate the first arg as a general expression (it might be a variable holding a shape)
    const shapeReg = this.generateExpression(ir, shapeArg)
    const reg = `%${this.valueCounter++}`
    ir.push(`  ${reg} = call ptr @tensor_create(i64 1, ptr ${shapeReg}, i64 ${dtypeNum})`)
    return reg
  }

  private tensorDtypeToInt(dtype: any): number {
    if (!dtype) return 0
    const kind = dtype.kind || dtype.toString()
    switch (kind) {
      case "f32": return 0
      case "f64": return 1
      case "f16": return 2
      case "bf16": return 3
      case "i32": return 4
      case "i64": return 5
      default: return 0
    }
  }

  private toFloatReg(ir: string[], reg: string, expr: any): string {
    // If it's already a float literal or float expression, return as-is
    if (expr?.type === "FloatLiteral") {
      // FloatLiteral values come as i64 (bitcast from double), convert to float
      const doubleReg = `%${this.valueCounter++}`
      ir.push(`  ${doubleReg} = bitcast i64 ${reg} to double`)
      const floatReg = `%${this.valueCounter++}`
      ir.push(`  ${floatReg} = fptrunc double ${doubleReg} to float`)
      return floatReg
    }
    if (this.isFloatOperand(expr)) {
      // Already double, truncate to float
      const floatReg = `%${this.valueCounter++}`
      ir.push(`  ${floatReg} = fptrunc double ${reg} to float`)
      return floatReg
    }
    // Integer literal - convert to float
    const floatReg = `%${this.valueCounter++}`
    ir.push(`  ${floatReg} = sitofp i64 ${reg} to float`)
    return floatReg
  }

  private isTensorType(expr: any): boolean {
    if (!expr) return false

    // Check if it's an identifier with TensorType in variableTypes
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      if (varType?.type === "TensorType") return true
    }

    // Check if the expression has a resultType that's a tensor
    if (expr.resultType?.type === "TensorType") return true

    // Check if it's a TensorConstruction node
    if (expr.type === "TensorConstruction") return true

    return false
  }

  private inferTensorDtype(expr: any): any {
    if (!expr) return { kind: "f32", type: "FloatType" }
    // Direct tensor construction
    if (expr.type === "TensorConstruction") return expr.dtype
    // Binary expression - infer from operands
    if (expr.type === "BinaryExpression") {
      if (this.isTensorType(expr.left)) return this.inferTensorDtype(expr.left)
      if (this.isTensorType(expr.right)) return this.inferTensorDtype(expr.right)
    }
    // Identifier - get from variable type
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      if (varType?.type === "TensorType") return varType.dtype
    }
    // Call expression
    if (expr.type === "CallExpression") {
      if (expr.callee?.type === "MemberExpression" && this.isTensorType(expr.callee.object)) {
        return this.inferTensorDtype(expr.callee.object)
      }
      // matmul(a, b) - infer from first arg
      const args = expr.args || expr.arguments || []
      if (args.length > 0) return this.inferTensorDtype(args[0])
    }
    return { kind: "f32", type: "FloatType" }
  }

  private isFloatProducingExpression(expr: any): boolean {
    if (!expr) return false
    // IndexExpression on float arrays returns double directly from array_get_f64
    if (expr.type === "IndexExpression" && expr.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.name)
      if (varType?.type === "ArrayType" && varType.elementType?.type === "FloatType") return true
    }
    // Check resultType annotation from analyzer
    if (expr.resultType?.type === "FloatType") return true
    // Float literal: NumberLiteral with '.' in value
    if (expr.type === "NumberLiteral" && typeof expr.value === "string" && expr.value.includes(".")) return true
    // Cast to f64
    if (expr.type === "CastExpression" && (expr.targetType === "f64" || expr.targetType?.name === "f64")) return true
    // Binary expression where either operand is float-producing
    if (expr.type === "BinaryExpression") {
      if (this.isFloatProducingExpression(expr.left) || this.isFloatProducingExpression(expr.right)) return true
    }
    // Unary on float
    if (expr.type === "UnaryExpression") {
      if (this.isFloatProducingExpression(expr.operand)) return true
    }
    // Function call returning float
    if (expr.type === "CallExpression" && expr.callee?.type === "Identifier") {
      const retType = this.functionTypes.get(expr.callee.name)
      if (retType?.type === "FloatType") return true
      // Math builtins always return float
      const callee = expr.callee.name
      const mathBuiltins = new Set([
        "sqrt", "sin", "cos", "tan", "asin", "acos",
        "exp", "log", "log2", "floor", "ceil", "round", "abs",
        "pow", "atan2", "min", "max",
      ])
      if (callee && mathBuiltins.has(callee)) return true
      // Check user-defined functions that return float types
      if (callee) {
        const func = this.functions.get(callee)
        if (func && (func.returnType === "double" || func.returnType === "float" || func.returnType === "half" || func.returnType === "fp128")) {
          return true
        }
      }
    }
    // Variable known to be float
    if (expr.type === "Identifier") {
      const vt = this.variableTypes.get(expr.name)
      if (vt?.type === "FloatType") return true
    }
    // Member access on a struct with float field
    if (expr.type === "MemberExpression" && expr.object?.type === "Identifier") {
      const objType = this.variableTypes.get(expr.object.name)
      if (objType?.type === "StructType" && objType.fields) {
        const fieldType = objType.fields.get(expr.property)
        if (fieldType?.type === "FloatType") return true
      }
      // SOAType direct member access (e.g., particles.x where x is f64)
      if (objType?.type === "SOAType") {
        const fieldType = objType.structType?.fields?.get(expr.property)
        if (fieldType?.type === "FloatType") return true
      }
    }
    // SoA transposed access: datums[i].field where datums is SOAType
    if (expr.type === "MemberExpression" && expr.object?.type === "IndexExpression" && expr.object?.object?.type === "Identifier") {
      const objType = this.variableTypes.get(expr.object.object.name)
      if (objType?.type === "SOAType") {
        const fieldType = objType.structType?.fields?.get(expr.property)
        if (fieldType?.type === "FloatType") return true
      }
    }
    // Tensor method calls that return double: .sum(), .mean()
    if (expr.type === "CallExpression" && expr.callee?.type === "MemberExpression") {
      const method = expr.callee.property
      if (this.isTensorType(expr.callee.object)) {
        const floatMethods = ["sum", "mean", "norm", "dot"]
        if (floatMethods.includes(method)) return true
      }
    }
    return false
  }

  private isTensorProducingExpression(expr: any): boolean {
    if (!expr) return false
    // Direct tensor construction
    if (expr.type === "TensorConstruction") return true
    // Binary expression where at least one operand is a tensor
    if (expr.type === "BinaryExpression") {
      return this.isTensorType(expr.left) || this.isTensorType(expr.right)
    }
    // matmul() call
    if (expr.type === "CallExpression" && expr.callee?.type === "Identifier" && expr.callee.name === "matmul") {
      return true
    }
    // Free tensor functions: relu(), sigmoid(), tanh(), softmax()
    if (expr.type === "CallExpression" && expr.callee?.type === "Identifier") {
      const tensorFuncs = ["relu", "sigmoid", "tanh", "softmax"]
      if (tensorFuncs.includes(expr.callee.name)) return true
    }
    // Static constructors: tensor.zeros(), tensor.ones(), tensor.randn()
    if (expr.type === "CallExpression" && expr.callee?.type === "MemberExpression") {
      if (expr.callee.object?.name === "tensor") {
        const staticMethods = ["zeros", "ones", "randn"]
        if (staticMethods.includes(expr.callee.property)) return true
      }
    }
    // Tensor method calls that return tensors (matmul, reshape, etc.)
    if (expr.type === "CallExpression" && expr.callee?.type === "MemberExpression") {
      const method = expr.callee.property
      if (this.isTensorType(expr.callee.object)) {
        const tensorMethods = ["matmul", "reshape", "transpose", "view", "flatten", "squeeze", "unsqueeze", "relu", "sigmoid", "tanh", "softmax"]
        if (tensorMethods.includes(method)) return true
      }
    }
    return false
  }

  private generatePrintCall(ir: string[], funcName: string, args: string[], expr: any): string {
    // Handle println with no arguments (just newline)
    if (funcName === "println" && args.length === 0) {
      ir.push(`  call void @println()`)
      return ""
    }

    // Handle print_buffer specially (takes ptr and i64)
    if (funcName === "print_buffer") {
      if (args.length >= 2) {
        ir.push(`  call void @print_buffer(ptr ${args[0]}, i64 ${args[1]})`)
      }
      return ""
    }

    if (args.length === 0) {
      return ""
    }

    // Determine the actual function to call based on argument type
    const arg = expr.args?.[0] || expr.arguments?.[0]
    let actualFunc = funcName

    // Determine argument type - try resultType first, then check function calls, then variableTypes
    let argType = arg?.resultType?.type
    if (!argType && arg?.type === "Identifier") {
      const varType = this.variableTypes.get(arg.name)
      argType = varType?.type
    }
    if (!argType && arg?.type === "CallExpression" && arg?.callee?.type === "Identifier") {
      // Check if this is a call to a user-defined function and get its return type
      const funcName = arg.callee.name
      const funcType = this.functionTypes.get(funcName)
      if (funcType) {
        argType = funcType.type
      }
    }
    // String literals are strings
    if (!argType && arg?.type === "StringLiteral") {
      argType = "ArrayType"
    }
    // Template literals are also strings
    if (!argType && arg?.type === "TemplateLiteral") {
      argType = "ArrayType"
    }
    // Pointer types (ptr) are also strings/pointer data
    if (!argType && arg?.type === "Identifier") {
      const varType = this.variableTypes.get(arg.name)
      if (varType?.type === "PointerType" || varType?.type === "ArrayType") {
        argType = "PointerType"
      }
    }
    // MemberExpression on a struct - look up the field type
    if (!argType && arg?.type === "MemberExpression" && arg?.object?.type === "Identifier") {
      const varType = this.variableTypes.get(arg.object.name)
      const structName = varType?.name || varType?.structName
      const structFields = structName ? this.structDefs.get(structName) : null
      if (structFields) {
        const fieldDef = structFields.find((f: any) => f.name === arg.property)
        if (fieldDef?.fieldType?.type) {
          argType = fieldDef.fieldType.type
        }
      }
    }
    // SoA transposed access: datums[i].field - look up field type from SOAType
    if (!argType && arg?.type === "MemberExpression" && arg?.object?.type === "IndexExpression" && arg?.object?.object?.type === "Identifier") {
      const varType = this.variableTypes.get(arg.object.object.name)
      if (varType?.type === "SOAType") {
        const soaType = varType as SOAType
        const fieldType = soaType.structType.fields.get(arg.property)
        if (fieldType?.type) {
          argType = fieldType.type
        }
      }
    }
    // Check if it's a string method call result (e.g., s.upper(), s.lower(), s.trim())
    if (!argType && this.isStringType(arg)) {
      argType = "StringType"
    }
    // Check if it's a call to a string-producing function (e.g., str(), i64_to_string())
    if (!argType && this.isStringProducingExpression(arg)) {
      argType = "StringType"
    }
    argType = argType || "IntegerType"

    // Force ptr type for direct print_string/println_string calls
    if (actualFunc === "print_string" || actualFunc === "println_string") {
      ir.push(`  call void @${actualFunc}(ptr ${args[0]})`)
      return ""
    }

    // If generic print/println, determine specific version based on type
    if (funcName === "print" || funcName === "println") {
      if (argType === "FloatType") {
        actualFunc = funcName === "print" ? "print_f64" : "println_f64"
      } else if (argType === "UnsignedType") {
        actualFunc = funcName === "print" ? "print_u64" : "println_u64"
      } else if (argType === "ArrayType" || argType === "PointerType" || argType === "StringType") {
        actualFunc = funcName === "print" ? "print_string" : "println_string"
      } else {
        actualFunc = funcName === "print" ? "print_i64" : "println_i64"
      }
    }

    // Generate call with appropriate type
    // For explicitly-typed print functions, use the correct LLVM type
    const isExplicitFloat = actualFunc === "print_f64" || actualFunc === "println_f64"
    if (argType === "FloatType" || isExplicitFloat) {
      // If the arg is a hex constant (like PI/E), it's already a double literal
      // If it's a register from math builtins, it was bitcast back to i64 — need to convert
      let doubleVal = args[0]
      if (!args[0].startsWith("0x") && !this.isFloatOperand(arg)) {
        // i64 register — bitcast to double
        const bc = `%${this.valueCounter++}`
        ir.push(`  ${bc} = bitcast i64 ${args[0]} to double`)
        doubleVal = bc
      }
      ir.push(`  call void @${actualFunc}(double ${doubleVal})`)
    } else if (argType === "ArrayType" || argType === "PointerType" || argType === "StringType") {
      ir.push(`  call void @${actualFunc}(ptr ${args[0]})`)
    } else {
      ir.push(`  call void @${actualFunc}(i64 ${args[0]})`)
    }
    return ""
  }

  private generateAssignmentExpression(ir: string[], expr: any): string {
    const { name, target, value } = expr
    const valueReg = this.generateExpression(ir, value)
    
    if (target && target.type === "IndexExpression") {
      const obj = this.generateExpression(ir, target.object)
      const index = this.generateExpression(ir, target.index)

      // Check if this is a table assignment (table[key] = value)
      if (this.isMapType(target.object)) {
        // Table assignment: table[key] := value
        // The index is the key - could be string or integer
        const keyExpr = target.index
        let keyPtr: string
        let keyLen: string

        if (keyExpr.type === "StringLiteral") {
          // String key: use string literal directly
          const { escaped, byteLength } = this.escapeStringForLLVM(keyExpr.value)
          keyPtr = `@key_str_${this.stringCounter++}`
          const strDef = `${keyPtr} = private unnamed_addr constant [${byteLength + 1} x i8] c"${escaped}\\00"`
          this.stringConstants.push(strDef)
          keyLen = String(byteLength)
        } else if (keyExpr.type === "Identifier") {
          // Variable key - assume it's a string, use strlen to get length
          const keyVar = this.generateExpression(ir, keyExpr)
          keyPtr = keyVar
          const lenReg = `%${this.valueCounter++}`
          ir.push(`  ${lenReg} = call i64 @string_length(ptr ${keyVar})`)
          keyLen = lenReg
        } else if (keyExpr.type === "IntegerLiteral" || keyExpr.type === "NumberLiteral") {
          // Integer key: store to memory and pass as ptr
          const keyVal = keyExpr.value !== undefined ? keyExpr.value : index
          keyPtr = `%${this.valueCounter++}`
          ir.push(`  ${keyPtr} = alloca i64`)
          ir.push(`  store i64 ${keyVal}, ptr ${keyPtr}`)
          keyLen = "8" // i64 is 8 bytes
        } else {
          // Expression key: evaluate and use as string
          keyPtr = index
          const lenReg = `%${this.valueCounter++}`
          ir.push(`  ${lenReg} = call i64 @string_length(ptr ${index})`)
          keyLen = lenReg
        }

        ir.push(`  call void @map_set(ptr ${obj}, ptr ${keyPtr}, i64 ${keyLen}, i64 ${valueReg})`)
      } else {
        // Check if this is a raw pointer (from gc_alloc) vs an array object
        const objVar = target.object?.type === "Identifier" ? this.variableTypes.get(target.object.name) : null
        const isRawPointer = objVar?.type === "PointerType"
        
        if (isRawPointer) {
          // Raw pointer access: use getelementptr + store directly
          // Handle nested array access (pointer arithmetic on raw ptr)
          const isNestedIndex = target.object?.type === "IndexExpression"
          let ptrReg = obj
          if (isNestedIndex) {
            const r = `%${this.valueCounter++}`
            ir.push(`  ${r} = inttoptr i64 ${obj} to ptr`)
            ptrReg = r
          }
          
          // Calculate element address: ptr + index
          const elemPtr = `%${this.valueCounter++}`
          ir.push(`  ${elemPtr} = getelementptr i8, ptr ${ptrReg}, i64 ${index}`)
          
          // Store the value
          ir.push(`  store i64 ${valueReg}, ptr ${elemPtr}`)
        } else if (objVar instanceof TensorType) {
          // Handle tensor element assignment: t[i] = val
          let floatVal = valueReg
          if (!this.isFloatOperand(expr.value)) {
            floatVal = `%${this.valueCounter++}`
            ir.push(`  ${floatVal} = sitofp i64 ${valueReg} to float`)
          } else {
            // If it's a double, truncate to float
            floatVal = `%${this.valueCounter++}`
            ir.push(`  ${floatVal} = fptrunc double ${valueReg} to float`)
          }
          ir.push(`  call void @tensor_set_f32(ptr ${obj}, i64 ${index}, float ${floatVal})`)
        } else {
          // Array assignment: array[index] := value
          // Handle nested array access
          const isNestedIndex = target.object?.type === "IndexExpression"
          let arrayPtr = obj
          if (isNestedIndex) {
            const r = `%${this.valueCounter++}`
            ir.push(`  ${r} = inttoptr i64 ${obj} to ptr`)
            arrayPtr = r
          }

          // Determine element type to use appropriate setter
          const elementType = this.getArrayElementType(target.object)

          // Emit bounds check elision hint in release mode for AoS/SoA operations
          if (this.shouldElideBoundsCheck()) {
            ir.push(`  ; BOUNDS_CHECK_ELIDED: release mode optimization`)
          }

          if (elementType?.type === "FloatType") {
            const isValueFloat = this.isFloatOperand(value) || valueReg.startsWith("0x")
            if (elementType.kind === "f32") {
              if (isValueFloat) {
                // Value is already a float - use directly
                ir.push(`  call void @array_set_f32(ptr ${arrayPtr}, i64 ${index}, float ${valueReg})`)
              } else {
                // Integer value needs conversion
                const floatReg = `%${this.valueCounter++}`
                ir.push(`  ${floatReg} = sitofp i64 ${valueReg} to float`)
                ir.push(`  call void @array_set_f32(ptr ${arrayPtr}, i64 ${index}, float ${floatReg})`)
              }
            } else if (elementType.kind === "f64") {
              if (isValueFloat) {
                // Value is already a double - use directly
                ir.push(`  call void @array_set_f64(ptr ${arrayPtr}, i64 ${index}, double ${valueReg})`)
              } else {
                // Integer value needs conversion
                const doubleReg = `%${this.valueCounter++}`
                ir.push(`  ${doubleReg} = sitofp i64 ${valueReg} to double`)
                ir.push(`  call void @array_set_f64(ptr ${arrayPtr}, i64 ${index}, double ${doubleReg})`)
              }
            } else {
              ir.push(`  call void @array_set(ptr ${arrayPtr}, i64 ${index}, i64 ${valueReg})`)
            }
          } else {
            ir.push(`  call void @array_set(ptr ${arrayPtr}, i64 ${index}, i64 ${valueReg})`)
          }
        }
      }
    } else if (target && target.type === "MemberExpression") {
      // SoA transposed write: datums[i].field = val → load column, array_set
      if (target.object?.type === "IndexExpression" && target.object?.object?.type === "Identifier") {
        const soaVarName = target.object.object.name
        const soaVarType = this.variableTypes.get(soaVarName)
        if (soaVarType?.type === "SOAType") {
          const soaType = soaVarType as SOAType
          const structType = soaType.structType
          const fieldName = target.property
          const fieldNames = Array.from(structType.fields.keys())
          const fieldIndex = fieldNames.indexOf(fieldName)
          
          if (fieldIndex >= 0) {
            const fieldType = structType.fields.get(fieldName)
            const isFloat = fieldType?.type === "FloatType"
            
            // Generate the index expression
            const indexReg = this.generateExpression(ir, target.object.index)
            
            // Load the SoA storage pointer
            const isParam = this.currentFunction && this.currentFunction.params.find((p: any) => p.name === soaVarName)
            const loadSrc = isParam ? `%${soaVarName}_local` : `%${soaVarName}`
            const storageReg = `%${this.valueCounter++}`
            ir.push(`  ${storageReg} = load ptr, ptr ${loadSrc}`)
            
            // GEP to the field's column array pointer
            const colPtrReg = `%${this.valueCounter++}`
            ir.push(`  ${colPtrReg} = getelementptr i8, ptr ${storageReg}, i64 ${fieldIndex * 8}`)
            
            // Load the column array pointer
            const colArrayReg = `%${this.valueCounter++}`
            ir.push(`  ${colArrayReg} = load ptr, ptr ${colPtrReg}`)
            
            // Set the value in the column array
            if (isFloat) {
              const floatKind = (fieldType as any)?.kind || "f64"
              const isValueFloat = this.isFloatOperand(value) || valueReg.startsWith("0x")
              if (floatKind === "f32") {
                if (isValueFloat) {
                  ir.push(`  call void @array_set_f32(ptr ${colArrayReg}, i64 ${indexReg}, float ${valueReg})`)
                } else {
                  const floatConv = `%${this.valueCounter++}`
                  ir.push(`  ${floatConv} = sitofp i64 ${valueReg} to float`)
                  ir.push(`  call void @array_set_f32(ptr ${colArrayReg}, i64 ${indexReg}, float ${floatConv})`)
                }
              } else {
                if (isValueFloat) {
                  ir.push(`  call void @array_set_f64(ptr ${colArrayReg}, i64 ${indexReg}, double ${valueReg})`)
                } else {
                  const doubleConv = `%${this.valueCounter++}`
                  ir.push(`  ${doubleConv} = sitofp i64 ${valueReg} to double`)
                  ir.push(`  call void @array_set_f64(ptr ${colArrayReg}, i64 ${indexReg}, double ${doubleConv})`)
                }
              }
            } else {
              ir.push(`  call void @array_set(ptr ${colArrayReg}, i64 ${indexReg}, i64 ${valueReg})`)
            }
            
            return valueReg
          }
        }
      }
      
      // Check if target object is a struct - use GEP-based assignment
      if (target.object?.type === "Identifier") {
        const varType = this.variableTypes.get(target.object.name)
        const structName = varType?.name || varType?.structName
        const structFields = structName ? this.structDefs.get(structName) : null

        if (structFields) {
          const fieldIndex = structFields.findIndex((f: any) => f.name === target.property)
          if (fieldIndex >= 0) {
            const fieldDef = structFields[fieldIndex]
            const isFloat = fieldDef.fieldType?.type === "FloatType"
            const fieldLLVMType = fieldDef.fieldType ? this.toLLVMType(fieldDef.fieldType) : "i64"
            // Load struct pointer
            // For function parameters, the param is stored in %name_local (see generateFunctionDeclaration)
            const isParam = this.currentFunction && this.currentFunction.params.find((p) => p.name === target.object.name)
            const structAssignSrc = isParam ? `%${target.object.name}_local` : `%${target.object.name}`
            const structPtr = `%${this.valueCounter++}`
            ir.push(`  ${structPtr} = load ptr, ptr ${structAssignSrc}`)
            // GEP to field
            const fieldPtr = `%${this.valueCounter++}`
            ir.push(`  ${fieldPtr} = getelementptr i8, ptr ${structPtr}, i64 ${fieldIndex * 8}`)
            // Store with correct type
            if (isFloat) {
              ir.push(`  store double ${valueReg}, ptr ${fieldPtr}`)
            } else if (fieldLLVMType === "ptr") {
              ir.push(`  store ptr ${valueReg}, ptr ${fieldPtr}`)
            } else {
              ir.push(`  store i64 ${valueReg}, ptr ${fieldPtr}`)
            }
            return valueReg
          }
        }
      }

      // Fallback: Map member assignment (person.age := 31)
      const obj = this.generateExpression(ir, target.object)
      const keyName = `@key_${target.property}_${this.stringCounter++}`
      const { escaped, byteLength } = this.escapeStringForLLVM(target.property)
      const strDef = `${keyName} = private unnamed_addr constant [${byteLength + 1} x i8] c"${escaped}\\00"`
      this.stringConstants.push(strDef)
      ir.push(`  call void @map_set(ptr ${obj}, ptr ${keyName}, i64 ${byteLength}, i64 ${valueReg})`)
    } else {
      let varType = this.variableTypes.get(name)
      // If variable doesn't exist yet (walrus-style new variable), allocate it
      if (!varType && name) {
        // Check if this is a SoAConstructor expression
        const isSoAConstructor = value?.type === "SoAConstructor"
        // Determine type from value - function pointer, string, SoA, or general
        // Lambda expressions now return closure pointers (ptr), not @name
        // Also detect named functions used as values (Identifier that resolves to a known function)
        const isNamedFuncRef = value?.type === "Identifier" && this.functions.has(value.name) && !this.variableTypes.has(value.name)
        // Check types BEFORE closure detection to avoid misclassifying string/struct-returning functions
        const isStringExpr = this.isStringProducingExpression(value)
        const isStructLiteral = value?.type === "StructLiteral"
        // Check if calling a function that returns a struct type
        const isStructReturningCall = value?.type === "CallExpression" && value.callee?.type === "Identifier"
          && (() => {
            const retType = this.functionTypes.get(value.callee.name)
            return retType?.type === "StructType" || retType?.type === "CustomType"
          })()
        // Check if calling a function that returns an array type
        const isArrayReturningCall = value?.type === "CallExpression" && value.callee?.type === "Identifier"
          && (() => {
            const retType = this.functionTypes.get(value.callee.name)
            return retType?.type === "ArrayType" || retType?.type === "AOSType"
          })()
        // Check if the value is a function call that returns ptr (e.g., make_adder(5) returning a closure)
        // Exclude string-returning, struct-returning, and array-returning functions
        const isClosureCall = value?.type === "CallExpression" && value.callee?.type === "Identifier"
          && this.functions.get(value.callee.name)?.returnType === "ptr"
          && !isStringExpr && !isStructLiteral && !isStructReturningCall && !isArrayReturningCall
        const isFuncPtr = valueReg.startsWith("@") || value?.type === "Lambda" || isNamedFuncRef || isClosureCall
        const isTensorExpr = this.isTensorProducingExpression(value)
        const isFloatExpr = this.isFloatProducingExpression(value)
        const isMapExpr = value?.type === "MapLiteral"
        const isArrayExpr = value?.type === "ArrayLiteral" || value?.type === "ArrayFill"
        // AwaitExpression returning ptr (string, struct, array from capability calls)
        const isAwaitPtr = value?.type === "AwaitExpression" && (
          value.resultType?.type === "PointerType" || value.resultType?.type === "StringType" ||
          value.resultType?.type === "ArrayType" || value.resultType?.type === "StructType" ||
          value.resultType?.type === "AOSType" || value.resultType?.type === "MapType")
        // Check resultType annotation for ptr-producing expressions (e.g., .filter(), .map() returning arrays)
        const isResultTypePtr = !isArrayReturningCall && !isStructReturningCall && value?.resultType && (
          value.resultType.type === "ArrayType" || value.resultType.type === "AOSType" ||
          value.resultType.type === "StructType" || value.resultType.type === "MapType")
        const allocaType = (isFuncPtr || isStringExpr || isSoAConstructor || isTensorExpr || isMapExpr || isStructLiteral || isStructReturningCall || isArrayReturningCall || isArrayExpr || isAwaitPtr || isResultTypePtr) ? "ptr" : isFloatExpr ? "double" : "i64"
        ir.push(`  %${name} = alloca ${allocaType}`)
        if (isTensorExpr) {
          // Register as TensorType for later method/operator dispatch
          const tensorDtype = this.inferTensorDtype(value)
          this.variableTypes.set(name, new TensorType(tensorDtype))
        } else if (isSoAConstructor) {
          // Build SOAType from struct fields and register it
          const structFields = this.structDefs.get(value.structName)
          if (structFields) {
            const structFieldMap = new Map<string, any>()
            for (const f of structFields) {
              structFieldMap.set(f.name, f.fieldType)
            }
            const structType = new StructType(value.structName, structFieldMap)
            const soaType = new SOAType(structType, value.capacity || 16)
            this.variableTypes.set(name, soaType)
            // Also register in structDefs for field lookup
            this.structDefs.set(name, structFields)
          }
        } else if (isFuncPtr) {
          // Track as a function type for later loads
          // Try to extract actual FunctionType from Lambda or named function
          if (value?.type === "Lambda") {
            const paramTypes = (value.params || []).map((p: any) => p.paramType)
            const retType = value.returnType
            this.variableTypes.set(name, new FunctionType(paramTypes, retType))
          } else if (isNamedFuncRef && this.functionTypes.has(value.name)) {
            // Named function reference - get its FunctionType
            const funcInfo = this.functions.get(value.name)
            if (funcInfo) {
              const paramTypes = funcInfo.params.map((p: any) => {
                // Try to reverse-lookup param types from functionParamInfo
                const paramInfo = this.functionParamInfo.get(value.name)
                const pInfo = paramInfo?.find((pi: any) => pi.name === p.name)
                return pInfo?.paramType || new IntegerType("i64")
              })
              const retType = this.functionTypes.get(value.name) || new IntegerType("i64")
              this.variableTypes.set(name, new FunctionType(paramTypes, retType))
            } else {
              this.variableTypes.set(name, { type: "FunctionType" })
            }
          } else if (isClosureCall) {
            // Function call returning a closure — extract the FunctionType from the called function's return type
            const calledRetType = this.functionTypes.get(value.callee.name)
            if (calledRetType && calledRetType.type === "FunctionType") {
              this.variableTypes.set(name, calledRetType)
            } else {
              this.variableTypes.set(name, { type: "FunctionType" })
            }
          } else {
            this.variableTypes.set(name, { type: "FunctionType" })
          }
         } else if (isMapExpr) {
          // Track as map type for later map operations, preserving key/value types if available
          const mapResultType = value?.resultType
          if (mapResultType?.type === "MapType" && mapResultType.keyType && mapResultType.valueType) {
            this.variableTypes.set(name, mapResultType)
          } else {
            this.variableTypes.set(name, { type: "MapType" } as any)
          }
        } else if (isStructLiteral) {
          // Track as struct type for field access
          const structName = value.name || value.structName
          const structFields = this.structDefs.get(structName)
          if (structFields) {
            const fieldMap = new Map<string, any>()
            for (const f of structFields) {
              fieldMap.set(f.name, f.fieldType)
            }
            this.variableTypes.set(name, new StructType(structName, fieldMap))
            this.structDefs.set(name, structFields)
          } else {
            this.variableTypes.set(name, { type: "StructType", name: structName } as any)
          }
        } else if (isStructReturningCall) {
          // Track as struct type from function return type
          const retType = this.functionTypes.get(value.callee.name)
          if (retType?.type === "StructType" && retType.name) {
            const structFields = this.structDefs.get(retType.name)
            if (structFields) {
              const fieldMap = new Map<string, any>()
              for (const f of structFields) {
                fieldMap.set(f.name, f.fieldType)
              }
              this.variableTypes.set(name, new StructType(retType.name, fieldMap))
              this.structDefs.set(name, structFields)
            } else {
              this.variableTypes.set(name, retType)
            }
          } else {
            this.variableTypes.set(name, retType || { type: "StructType" } as any)
          }
        } else if (isArrayReturningCall) {
          // Track as array type from function return type
          const retType = this.functionTypes.get(value.callee.name)
          this.variableTypes.set(name, retType)
        } else if (isAwaitPtr) {
          // Track based on the await result type
          const awaitResType = value.resultType
          if (awaitResType?.type === "StringType" || awaitResType?.type === "PointerType") {
            this.variableTypes.set(name, { type: "StringType" } as any)
          } else if (awaitResType?.type === "ArrayType" || awaitResType?.type === "AOSType") {
            this.variableTypes.set(name, awaitResType)
          } else if (awaitResType?.type === "StructType") {
            this.variableTypes.set(name, awaitResType)
          } else {
            this.variableTypes.set(name, { type: "PointerType" } as any)
          }
        } else if (isResultTypePtr) {
          // Track from analyzer's resultType annotation (e.g., .filter() returning ArrayType)
          this.variableTypes.set(name, value.resultType)
        } else if (isArrayExpr) {
          // Track as array type from ArrayLiteral or ArrayFill
          this.variableTypes.set(name, new ArrayType(new IntegerType("i64"), []))
        } else if (isStringExpr) {
          // Track as string type for later string operations
          this.variableTypes.set(name, { type: "StringType" } as any)
        } else if (isFloatExpr) {
          // Track as float type for proper load/store
          this.variableTypes.set(name, new FloatType("f64"))
        } else {
          // Default: track as integer type so reassignment doesn't re-alloca
          this.variableTypes.set(name, new IntegerType("i64"))
        }
        varType = this.variableTypes.get(name)
      }
      const llvmType = this.toLLVMType(varType)
      ir.push(`  store ${llvmType} ${valueReg}, ptr %${name}`)
    }
    
    return valueReg
  }

  private generateArrayLiteral(ir: string[], expr: any): string {
    const size = expr.elements.length || 0
    const dimensionsReg = `%${this.valueCounter++}`
    ir.push(`  ${dimensionsReg} = alloca [1 x i64]`)
    const dimensionsPtr = `%${this.valueCounter++}`
    ir.push(`  ${dimensionsPtr} = getelementptr [1 x i64], ptr ${dimensionsReg}, i64 0, i64 0`)
    ir.push(`  store i64 ${size}, ptr ${dimensionsPtr}`)

    // Determine element type from the array literal's resultType
    const elementType = expr.resultType?.elementType
    let elementSize = 8 // default to 8 bytes for i64
    if (elementType?.type === "FloatType") {
      elementSize = elementType.kind === "f64" ? 8 : 4
    }

    const elementReg = `%${this.valueCounter++}`
    ir.push(`  ${elementReg} = call ptr @array_alloc(i64 ${elementSize}, i64 1, ptr ${dimensionsReg})`)

    for (let i = 0; i < expr.elements.length; i++) {
      const elemExpr = expr.elements[i]
      if (elementType?.type === "FloatType") {
        // Handle float elements directly
        const val = String(elemExpr.value)
        const numVal = parseFloat(val)
        if (elementType.kind === "f32") {
          // Generate float constant as decimal - ensure it has a decimal point
          const floatLit = val.includes('.') ? val : val + '.0'
          ir.push(`  call void @array_set_f32(ptr ${elementReg}, i64 ${i}, float ${floatLit})`)
        } else if (elementType.kind === "f64") {
          // Generate double constant as decimal - ensure it has a decimal point
          const doubleLit = val.includes('.') ? val : val + '.0'
          ir.push(`  call void @array_set_f64(ptr ${elementReg}, i64 ${i}, double ${doubleLit})`)
        } else {
          const value = this.generateExpression(ir, elemExpr)
          ir.push(`  call void @array_set(ptr ${elementReg}, i64 ${i}, i64 ${value})`)
        }
      } else {
        const value = this.generateExpression(ir, elemExpr)
        if (value) {
          // Check if value is a pointer (nested array) and convert to i64
          const valueToStore = value.includes('ptr') || value.startsWith('%') && !value.includes('i64')
            ? (() => { const r = `%${this.valueCounter++}`; ir.push(`  ${r} = ptrtoint ptr ${value} to i64`); return r; })()
            : value
          ir.push(`  call void @array_set(ptr ${elementReg}, i64 ${i}, i64 ${valueToStore})`)
        }
      }
    }

    return elementReg
  }

  private generateArrayFill(ir: string[], expr: any): string {
    // Generate array filled with repeated value: [value; count]
    const countExpr = expr.count
    const valueExpr = expr.value

    // Get element type from resultType
    const elementType = expr.resultType?.elementType
    let elementSize = 8 // default to 8 bytes for i64
    if (elementType?.type === "FloatType") {
      elementSize = elementType.kind === "f64" ? 8 : 4
    }

    // Determine count - try to get compile-time constant
    let countValue: number | null = null
    if (countExpr.type === "NumberLiteral" || countExpr.type === "IntegerLiteral") {
      countValue = parseInt(countExpr.value)
    }

    // Allocate array with the specified count
    let countReg: string
    if (countValue !== null) {
      countReg = `%${this.valueCounter++}`
      ir.push(`  ${countReg} = alloca [1 x i64]`)
      const countPtr = `%${this.valueCounter++}`
      ir.push(`  ${countPtr} = getelementptr [1 x i64], ptr ${countReg}, i64 0, i64 0`)
      ir.push(`  store i64 ${countValue}, ptr ${countPtr}`)
    } else {
      // Runtime count — generate expression FIRST to avoid register misordering
      const runtimeCount = this.generateExpression(ir, countExpr)
      countReg = `%${this.valueCounter++}`
      ir.push(`  ${countReg} = alloca [1 x i64]`)
      const countPtr = `%${this.valueCounter++}`
      ir.push(`  ${countPtr} = getelementptr [1 x i64], ptr ${countReg}, i64 0, i64 0`)
      ir.push(`  store i64 ${runtimeCount}, ptr ${countPtr}`)
    }

    const elementReg = `%${this.valueCounter++}`
    ir.push(`  ${elementReg} = call ptr @array_alloc(i64 ${elementSize}, i64 1, ptr ${countReg})`)

    // Fill the array with the value
    const value = this.generateExpression(ir, valueExpr)

    // Loop to fill array
    const loopStartLabel = this.nextLabel()
    const loopBodyLabel = this.nextLabel()
    const loopEndLabel = this.nextLabel()

    const iReg = `%${this.valueCounter++}`
    ir.push(`  ${iReg} = alloca i64`)
    ir.push(`  store i64 0, ptr ${iReg}`)
    ir.push(`  br label %${loopStartLabel}`)

    ir.push(`${loopStartLabel}:`)
    const currentI = `%${this.valueCounter++}`
    ir.push(`  ${currentI} = load i64, ptr ${iReg}`)
    if (countValue !== null) {
      const cmpResult = `%${this.valueCounter++}`
      ir.push(`  ${cmpResult} = icmp slt i64 ${currentI}, ${countValue}`)
      ir.push(`  br i1 ${cmpResult}, label %${loopBodyLabel}, label %${loopEndLabel}`)
    } else {
      const runtimeCount = this.generateExpression(ir, countExpr)
      const cmpResult = `%${this.valueCounter++}`
      ir.push(`  ${cmpResult} = icmp slt i64 ${currentI}, ${runtimeCount}`)
      ir.push(`  br i1 ${cmpResult}, label %${loopBodyLabel}, label %${loopEndLabel}`)
    }

    ir.push(`${loopBodyLabel}:`)

    // Store value at current index
    if (elementType?.type === "FloatType") {
      if (elementType.kind === "f32") {
        ir.push(`  call void @array_set_f32(ptr ${elementReg}, i64 ${currentI}, float ${value})`)
      } else {
        ir.push(`  call void @array_set_f64(ptr ${elementReg}, i64 ${currentI}, double ${value})`)
      }
    } else {
      ir.push(`  call void @array_set(ptr ${elementReg}, i64 ${currentI}, i64 ${value})`)
    }

    // Increment counter
    const nextI = `%${this.valueCounter++}`
    ir.push(`  ${nextI} = add i64 ${currentI}, 1`)
    ir.push(`  store i64 ${nextI}, ptr ${iReg}`)
    ir.push(`  br label %${loopStartLabel}`)

    ir.push(`${loopEndLabel}:`)

    return elementReg
  }

  private generateMapLiteral(ir: string[], expr: any): string {
    const entries = expr.entries || expr.columns || []

    // Regular map literal
    const tableReg = `%${this.valueCounter++}`
    const capacity = entries.length > 0 ? entries.length * 2 : 4
    ir.push(`  ${tableReg} = call ptr @map_new(i64 ${capacity})`)

     for (const entry of entries) {
      const key = entry.key || entry.name
      const value = entry.value || (entry.values && entry.values[0])
      if (!value) continue
      let valueReg = this.generateExpression(ir, value)
      // Float values from generateExpression are hex constants (e.g., 0x3ff8000000000000)
      // which LLVM treats as double-type constants. For map_set's i64 parameter, we need
      // to bitcast double -> i64 so LLVM accepts the constant.
      const valStr = String(value.value || "")
      const isFloatVal = (valStr.includes(".") || valStr.toLowerCase().includes("e"))
        || value.resultType?.type === "FloatType"
        || value.literalType?.type === "FloatType"
      if (isFloatVal) {
        const bitcastReg = `%${this.valueCounter++}`
        ir.push(`  ${bitcastReg} = bitcast double ${valueReg} to i64`)
        valueReg = bitcastReg
      }
      // Use pre-registered map key constant from collection pass
      const keyName = this.stringNameMap.get(`__map_key_${key}`) || `@map_key_${this.stringCounter++}`
      ir.push(`  call void @map_set(ptr ${tableReg}, ptr ${keyName}, i64 ${key.length}, i64 ${valueReg})`)
    }

    return tableReg
  }

  private generateStructLiteral(ir: string[], expr: any): string {
    // Look up struct definition to get field order and types
    const structName = expr.structName || ""
    const structFields = this.structDefs.get(structName)

    if (!structFields || structFields.length === 0) {
      // Fallback: allocate based on literal fields, detect types where possible
      const numFields = expr.fields?.length || 0
      const structReg = `%${this.valueCounter++}`
      ir.push(`  ${structReg} = call ptr @gc_alloc(i64 ${numFields * 8})`)
      for (let i = 0; i < numFields; i++) {
        const field = expr.fields[i]
        const valueReg = this.generateExpression(ir, field.value)
        if (valueReg) {
          const fieldPtr = `%${this.valueCounter++}`
          ir.push(`  ${fieldPtr} = getelementptr i8, ptr ${structReg}, i64 ${i * 8}`)
          // Detect float and pointer values to use correct store type
          const isFloat = field.value?.type === "FloatLiteral" || this.isFloatOperand(field.value)
          const isString = field.value?.type === "StringLiteral" || this.isStringProducingExpression(field.value)
          if (isFloat) {
            ir.push(`  store double ${valueReg}, ptr ${fieldPtr}`)
          } else if (isString) {
            ir.push(`  store ptr ${valueReg}, ptr ${fieldPtr}`)
          } else {
            ir.push(`  store i64 ${valueReg}, ptr ${fieldPtr}`)
          }
        }
      }
      return structReg
    }

    // Allocate struct: each field is 8 bytes
    const structReg = `%${this.valueCounter++}`
    ir.push(`  ${structReg} = call ptr @gc_alloc(i64 ${structFields.length * 8})`)

    // Store each field at its offset
    for (const field of expr.fields) {
      const fieldIndex = structFields.findIndex((f: any) => f.name === field.name)
      if (fieldIndex < 0) continue

      const fieldDef = structFields[fieldIndex]
      const valueReg = this.generateExpression(ir, field.value)
      if (!valueReg) continue

      const fieldPtr = `%${this.valueCounter++}`
      ir.push(`  ${fieldPtr} = getelementptr i8, ptr ${structReg}, i64 ${fieldIndex * 8}`)

      // Determine store type based on field type
      const isFloat = fieldDef.fieldType?.type === "FloatType"
      const fieldLLVMType = fieldDef.fieldType ? this.toLLVMType(fieldDef.fieldType) : "i64"
      if (isFloat) {
        ir.push(`  store double ${valueReg}, ptr ${fieldPtr}`)
      } else if (fieldLLVMType === "ptr") {
        ir.push(`  store ptr ${valueReg}, ptr ${fieldPtr}`)
      } else {
        ir.push(`  store i64 ${valueReg}, ptr ${fieldPtr}`)
      }
    }

    return structReg
  }

  private generateSoALiteral(_ir: string[], _expr: any): string {
    // SoA literals are no longer supported in the new design.
    // SoA containers are declared with: soa name: StructType[capacity]
    return "0"
  }

  private generateSoAConstructor(ir: string[], expr: any): string {
    const structName = expr.structName
    const capacity = expr.capacity || 16  // default capacity
    const structFields = this.structDefs.get(structName)

    if (!structFields || structFields.length === 0) {
      return "null"
    }

    const numFields = structFields.length

    // Allocate storage block: one ptr per field
    const storageReg = `%${this.valueCounter++}`
    ir.push(`  ${storageReg} = call ptr @gc_alloc(i64 ${numFields * 8})`)

    // Allocate one array per field
    for (let i = 0; i < numFields; i++) {
      const field = structFields[i]
      const elemSize = 8  // all fields are 8 bytes (i64 or f64)

      // Create dimensions array on stack
      const dimsAlloca = `%${this.valueCounter++}`
      ir.push(`  ${dimsAlloca} = alloca [1 x i64]`)
      const dimPtr = `%${this.valueCounter++}`
      ir.push(`  ${dimPtr} = getelementptr [1 x i64], ptr ${dimsAlloca}, i64 0, i64 0`)
      ir.push(`  store i64 ${capacity}, ptr ${dimPtr}`)

      const arrayReg = `%${this.valueCounter++}`
      ir.push(`  ${arrayReg} = call ptr @array_alloc(i64 ${elemSize}, i64 1, ptr ${dimPtr})`)

      // Store array pointer in storage block
      const fieldPtr = `%${this.valueCounter++}`
      ir.push(`  ${fieldPtr} = getelementptr i8, ptr ${storageReg}, i64 ${i * 8}`)
      ir.push(`  store ptr ${arrayReg}, ptr ${fieldPtr}`)
    }

    return storageReg
  }

  private generateMemberExpression(ir: string[], expr: any): string {
    // Package-qualified access: pkg.func or pkg.Type
    if (expr.object?.type === "Identifier" && this.importedPackages.has(expr.object.name)) {
      const pkgName = expr.object.name
      const mangledName = `${pkgName}__${expr.property}`
      // Return as identifier reference to the mangled name
      return `@${mangledName}`
    }

    // String property access: s.len
    if (expr.property === "len" && this.isStringType(expr.object)) {
      const strReg = this.generateExpression(ir, expr.object)
      const reg = `%${this.valueCounter++}`
      ir.push(`  ${reg} = call i64 @string_length(ptr ${strReg})`)
      return reg
    }

    // Array property access: arr.len
    if (expr.property === "len" && this.isArrayType(expr.object)) {
      const arrReg = this.generateExpression(ir, expr.object)
      const reg = `%${this.valueCounter++}`
      ir.push(`  ${reg} = call i64 @array_length(ptr ${arrReg})`)
      return reg
    }

    // Tensor property access: t.ndim, t.size
    if (this.isTensorType(expr.object)) {
      const tensorReg = this.generateExpression(ir, expr.object)
      if (expr.property === "ndim") {
        const reg = `%${this.valueCounter++}`
        ir.push(`  ${reg} = call i64 @tensor_ndim(ptr ${tensorReg})`)
        return reg
      }
      if (expr.property === "size") {
        const reg = `%${this.valueCounter++}`
        ir.push(`  ${reg} = call i64 @tensor_size(ptr ${tensorReg})`)
        return reg
      }
    }

    // SoA transposed access: datums[i].field → load column array, then array_get
    if (expr.object?.type === "IndexExpression" && expr.object?.object?.type === "Identifier") {
      const varName = expr.object.object.name
      const varType = this.variableTypes.get(varName)
      if (varType?.type === "SOAType") {
        const soaType = varType as SOAType
        const structType = soaType.structType
        const fieldName = expr.property
        const fieldNames = Array.from(structType.fields.keys())
        const fieldIndex = fieldNames.indexOf(fieldName)
        
        if (fieldIndex >= 0) {
          const fieldType = structType.fields.get(fieldName)
          const isFloat = fieldType?.type === "FloatType"
          
          // Generate the index expression
          const indexReg = this.generateExpression(ir, expr.object.index)
          
          // Load the SoA storage pointer
          const isParam = this.currentFunction && this.currentFunction.params.find((p: any) => p.name === varName)
          const loadSrc = isParam ? `%${varName}_local` : `%${varName}`
          const storageReg = `%${this.valueCounter++}`
          ir.push(`  ${storageReg} = load ptr, ptr ${loadSrc}`)
          
          // GEP to the field's column array pointer
          const colPtrReg = `%${this.valueCounter++}`
          ir.push(`  ${colPtrReg} = getelementptr i8, ptr ${storageReg}, i64 ${fieldIndex * 8}`)
          
          // Load the column array pointer
          const colArrayReg = `%${this.valueCounter++}`
          ir.push(`  ${colArrayReg} = load ptr, ptr ${colPtrReg}`)
          
          // Index into the column array
          const resultReg = `%${this.valueCounter++}`
          if (isFloat) {
            const floatKind = (fieldType as any)?.kind || "f64"
            if (floatKind === "f32") {
              ir.push(`  ${resultReg} = call float @array_get_f32(ptr ${colArrayReg}, i64 ${indexReg})`)
            } else {
              ir.push(`  ${resultReg} = call double @array_get_f64(ptr ${colArrayReg}, i64 ${indexReg})`)
            }
          } else {
            ir.push(`  ${resultReg} = call i64 @array_get(ptr ${colArrayReg}, i64 ${indexReg})`)
          }
          return resultReg
        }
      }
    }

    // Check if the object is a struct type - use GEP-based field access
    if (expr.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.name)
      const structName = varType?.name || varType?.structName
      const structFields = structName ? this.structDefs.get(structName) : null

      if (structFields) {
        const fieldIndex = structFields.findIndex((f: any) => f.name === expr.property)
        if (fieldIndex >= 0) {
          const fieldDef = structFields[fieldIndex]
          const isFloat = fieldDef.fieldType?.type === "FloatType"
          const fieldLLVMType = fieldDef.fieldType ? this.toLLVMType(fieldDef.fieldType) : "i64"
          // Load the struct pointer
          // For function parameters, the param is stored in %name_local (see generateFunctionDeclaration)
          // For local variables, the alloca is %name
          const isParam = this.currentFunction && this.currentFunction.params.find((p) => p.name === expr.object.name)
          const structLoadSrc = isParam ? `%${expr.object.name}_local` : `%${expr.object.name}`
          const structPtr = `%${this.valueCounter++}`
          ir.push(`  ${structPtr} = load ptr, ptr ${structLoadSrc}`)
          // GEP to field offset
          const fieldPtr = `%${this.valueCounter++}`
          ir.push(`  ${fieldPtr} = getelementptr i8, ptr ${structPtr}, i64 ${fieldIndex * 8}`)
          // Load field value with correct type
          const reg = `%${this.valueCounter++}`
          if (isFloat) {
            ir.push(`  ${reg} = load double, ptr ${fieldPtr}`)
          } else if (fieldLLVMType === "ptr") {
            ir.push(`  ${reg} = load ptr, ptr ${fieldPtr}`)
          } else {
            ir.push(`  ${reg} = load i64, ptr ${fieldPtr}`)
          }
          return reg
        }
      }
    }

    // Fallback: map-based member access
    const obj = this.generateExpression(ir, expr.object)
    const keyName = `@key_${expr.property}_${this.stringCounter++}`
    const { escaped, byteLength } = this.escapeStringForLLVM(expr.property)
    const strDef = `${keyName} = private unnamed_addr constant [${byteLength + 1} x i8] c"${escaped}\\00"`
    this.stringConstants.push(strDef)
    const reg = `%${this.valueCounter++}`
    ir.push(`  ${reg} = call i64 @map_get(ptr ${obj}, ptr ${keyName}, i64 ${byteLength})`)
    return reg
  }

  private generateIndexExpression(ir: string[], expr: any): string {
    const obj = this.generateExpression(ir, expr.object)
    const index = this.generateExpression(ir, expr.index)
    // Handle nested array access
    // If expr.object is an IndexExpression, array is an i64 (from array_get), convert to ptr
    // Otherwise, array is already a ptr (from alloca or variable load)
    const isNestedIndex = expr.object?.type === "IndexExpression"
    let arrayPtr = obj
    if (isNestedIndex) {
      const r = `%${this.valueCounter++}`
      ir.push(`  ${r} = inttoptr i64 ${obj} to ptr`)
      arrayPtr = r
    }

    // Check if this is a table access (table[key])
    if (this.isMapType(expr.object)) {
      // Table access: table[key] - key can be string or integer
      const keyExpr = expr.index
      let keyPtr: string
      let keyLen: string

      if (keyExpr.type === "StringLiteral") {
        // String key: use string literal directly
        const { escaped, byteLength } = this.escapeStringForLLVM(keyExpr.value)
        keyPtr = `@key_str_${this.stringCounter++}`
        const strDef = `${keyPtr} = private unnamed_addr constant [${byteLength + 1} x i8] c"${escaped}\\00"`
        this.stringConstants.push(strDef)
        keyLen = String(byteLength)
      } else if (keyExpr.type === "IntegerLiteral" || keyExpr.type === "NumberLiteral") {
        // Integer key: store to memory and pass as ptr
        const keyVal = keyExpr.value !== undefined ? keyExpr.value : index
        keyPtr = `%${this.valueCounter++}`
        ir.push(`  ${keyPtr} = alloca i64`)
        ir.push(`  store i64 ${keyVal}, ptr ${keyPtr}`)
        keyLen = "8" // i64 is 8 bytes
      } else {
        // Variable or expression key - assume string
        keyPtr = index
        const lenReg = `%${this.valueCounter++}`
        ir.push(`  ${lenReg} = call i64 @string_length(ptr ${index})`)
        keyLen = lenReg
      }

      const reg = `%${this.valueCounter++}`
      ir.push(`  ${reg} = call i64 @map_get(ptr ${arrayPtr}, ptr ${keyPtr}, i64 ${keyLen})`)
      return reg
    }

    // Check if this is a raw pointer (from gc_alloc)
    const objVar = expr.object?.type === "Identifier" ? this.variableTypes.get(expr.object.name) : null
    if (objVar?.type === "PointerType") {
      // Raw pointer access: use getelementptr + load directly
      const isNestedIndex = expr.object?.type === "IndexExpression"
      let ptrReg = arrayPtr
      if (isNestedIndex) {
        const r = `%${this.valueCounter++}`
        ir.push(`  ${r} = inttoptr i64 ${obj} to ptr`)
        ptrReg = r
      }
      
      // Calculate element address: ptr + index
      const elemPtr = `%${this.valueCounter++}`
      ir.push(`  ${elemPtr} = getelementptr i8, ptr ${ptrReg}, i64 ${index}`)
      
      // Load the value
      const valReg = `%${this.valueCounter++}`
      ir.push(`  ${valReg} = load i64, ptr ${elemPtr}`)
      return valReg
    }

    // Handle tensor element access: t[i] -> tensor_get_f32
    if (objVar instanceof TensorType) {
      const result = `%${this.valueCounter++}`
      ir.push(`  ${result} = call float @tensor_get_f32(ptr ${arrayPtr}, i64 ${index})`)
      return result
    }

    // Determine element type to use appropriate getter
    const elementType = this.getArrayElementType(expr.object)
    const reg = `%${this.valueCounter++}`

    // Check if this is a string type [u8]
    if (this.isStringType(expr.object)) {
      // String indexing - returns a new single-char string
      const resultReg = `%${this.valueCounter++}`
      // Emit bounds check elision hint in release mode
      if (this.shouldElideBoundsCheck()) {
        ir.push(`  ; BOUNDS_CHECK_ELIDED: release mode optimization`)
      }
      ir.push(`  ${resultReg} = call ptr @string_char_at(ptr ${arrayPtr}, i64 ${index})`)
      return resultReg
    }

    // Emit bounds check elision hint in release mode for AoS/SoA operations
    if (this.shouldElideBoundsCheck()) {
      ir.push(`  ; BOUNDS_CHECK_ELIDED: release mode optimization`)
    }

    if (elementType?.type === "StringType") {
      // String arrays: array_get returns i64 (pointer stored as i64), convert to ptr
      ir.push(`  ${reg} = call i64 @array_get(ptr ${arrayPtr}, i64 ${index})`)
      const ptrReg = `%${this.valueCounter++}`
      ir.push(`  ${ptrReg} = inttoptr i64 ${reg} to ptr`)
      return ptrReg
    } else if (elementType?.type === "FloatType") {
      if (elementType.kind === "f32") {
        const floatReg = `%${this.valueCounter++}`
        ir.push(`  ${floatReg} = call float @array_get_f32(ptr ${arrayPtr}, i64 ${index})`)
        // Convert float to i64 for language value representation
        const convReg = `%${this.valueCounter++}`
        ir.push(`  ${convReg} = fptoui float ${floatReg} to i64`)
        return convReg
      } else if (elementType.kind === "f64") {
        const doubleReg = `%${this.valueCounter++}`
        ir.push(`  ${doubleReg} = call double @array_get_f64(ptr ${arrayPtr}, i64 ${index})`)
        // Return the double register directly — the value is semantically a float
        return doubleReg
      } else {
        ir.push(`  ${reg} = call i64 @array_get(ptr ${arrayPtr}, i64 ${index})`)
      }
    } else {
      ir.push(`  ${reg} = call i64 @array_get(ptr ${arrayPtr}, i64 ${index})`)
    }
    return reg
  }

  private getArrayElementType(expr: any): any {
    if (!expr) return null

    // If it's an identifier, look up the variable type
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      if (varType?.type === "ArrayType") {
        return varType.elementType
      }
    }

    // If it's an index expression, recursively get element type
    if (expr.type === "IndexExpression") {
      return this.getArrayElementType(expr.object)
    }

    // Check if it's a MemberExpression on a SoA variable (returns the struct field type)
    if (expr.type === "MemberExpression" && expr.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.name)
      if (varType?.type === "SOAType") {
        const soaType = varType as SOAType
        const fieldName = expr.property?.name || expr.property
        const fieldType = soaType.structType.fields.get(fieldName)
        if (fieldType) {
          return fieldType
        }
      }
    }

    // Check if it's a SoA transposed access pattern: datums[i].field
    if (expr.type === "MemberExpression" && expr.object?.type === "IndexExpression" && expr.object?.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.object.name)
      if (varType?.type === "SOAType") {
        const soaType = varType as SOAType
        const fieldName = expr.property?.name || expr.property
        return soaType.structType.fields.get(fieldName) || null
      }
    }

    // Check if the expression itself has a resultType (from analyzer)
    if (expr.resultType?.type === "ArrayType") {
      return expr.resultType.elementType
    }

    return null
  }

  private getMapValueType(expr: any): any {
    if (!expr) return null

    // If it's an identifier, look up the variable type
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      if (varType?.type === "MapType" && (varType as MapType).valueType) {
        return (varType as MapType).valueType
      }
    }

    // Check if the expression itself has a resultType
    if (expr.resultType?.type === "MapType" && expr.resultType.valueType) {
      return expr.resultType.valueType
    }

    return null
  }

  private isStringProducingExpression(expr: any): boolean {
    if (!expr) return false
    // str() conversion and string-returning functions
    if (expr.type === "CallExpression" && expr.callee?.type === "Identifier") {
      const name = expr.callee.name
      if (name === "str" || name === "string_concat" || name === "string_upper" || 
          name === "string_lower" || name === "string_trim" || name === "string_replace" ||
          name === "string_char_at" || name === "string_slice" ||
          name === "i64_to_string" || name === "u64_to_string" || name === "f64_to_string") {
        return true
      }
      // Check user-defined functions that return string type
      const funcRetType = this.functionTypes.get(name)
      if (funcRetType?.type === "StringType" || funcRetType?.type === "PointerType") return true
    }
    // String method calls (e.g., s.upper(), s.trim())
    if (expr.type === "CallExpression" && expr.callee?.type === "MemberExpression") {
      const method = expr.callee.property
      if (["upper", "lower", "trim", "replace", "split", "join"].includes(method)) {
        return true
      }
    }
    // String literals and template literals
    if (expr.type === "StringLiteral" || expr.type === "TemplateLiteral") {
      return true
    }
    // IndexExpression on string arrays (e.g., parts[0] where parts: string[])
    if (expr.type === "IndexExpression" && expr.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.name)
      if (varType?.type === "ArrayType" && varType.elementType?.type === "StringType") return true
    }
    // Identifier that is a string variable
    if (expr.type === "Identifier") {
      const varType = this.variableTypes.get(expr.name)
      if (varType?.type === "StringType") return true
    }
    // Binary + on strings (string concatenation)
    if (expr.type === "BinaryExpression" && expr.operator === "+" &&
        (this.isStringType(expr.left) || this.isStringType(expr.right))) {
      return true
    }
    // Slice expression on strings
    if (expr.type === "SliceExpression" && this.isStringType(expr.object)) {
      return true
    }
    // Check analyzer result type
    if (expr.resultType?.type === "StringType") {
      return true
    }
    return false
  }

  private isStringType(expr: any): boolean {
    if (!expr) return false

    // Check if it's a string literal
    if (expr.type === "StringLiteral") {
      return true
    }

    // Check if it's a template literal (f-string)
    if (expr.type === "TemplateLiteral") {
      return true
    }

    // If it's an identifier, look up the variable type
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      if (varType?.type === "StringType") {
        return true
      }
      if (varType?.type === "ArrayType") {
        const arrType = varType as ArrayType
        return arrType.elementType?.type === "UnsignedType" &&
               (arrType.elementType as any)?.kind === "u8" &&
               arrType.dimensions.length === 0
      }
    }

    // Check if it's a method call that returns a string (e.g., s.upper().lower())
    if (expr.type === "CallExpression" && expr.callee?.type === "MemberExpression") {
      const method = expr.callee.property
      const stringMethods = ["upper", "lower", "trim", "replace", "split"]
      if (stringMethods.includes(method) && method !== "split") {
        return this.isStringType(expr.callee.object)
      }
    }

    // Check if it's a call to a string-producing function (e.g., str(), i64_to_string())
    if (expr.type === "CallExpression" && this.isStringProducingExpression(expr)) {
      return true
    }

    // Check if it's a binary + expression with string operands (string concatenation)
    if (expr.type === "BinaryExpression" && expr.operator === "+" &&
        (this.isStringType(expr.left) || this.isStringType(expr.right))) {
      return true
    }

    // Check analyzer result type
    if (expr.resultType?.type === "StringType") {
      return true
    }

    return false
  }

  private isArrayType(expr: any): boolean {
    if (!expr) return false

    // If it's an identifier, look up the variable type
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      if (varType?.type === "ArrayType" || varType?.type === "AOSType") {
        // Exclude string types ([u8])
        const arrType = varType as ArrayType
        if (arrType.elementType?.type === "UnsignedType" &&
            (arrType.elementType as any)?.kind === "u8" &&
            arrType.dimensions.length === 0) {
          return false
        }
        return true
      }
    }

    return false
  }

  private isPtrProducingExpression(expr: any): boolean {
    if (!expr) return false
    // Identifier — check variableTypes
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      if (varType) {
        const t = varType.type
        if (t === "ArrayType" || t === "AOSType" || t === "StructType" || 
            t === "CustomType" || t === "PointerType" || t === "StringType" ||
            t === "MapType" || t === "SOAType" || t === "FunctionType" || t === "TensorType") {
          return true
        }
      }
    }
    // StructLiteral always produces a ptr (gc_alloc)
    if (expr.type === "StructLiteral") return true
    // CallExpression returning ptr
    if (expr.type === "CallExpression" && expr.callee?.type === "Identifier") {
      const func = this.functions.get(expr.callee.name)
      if (func && func.returnType === "ptr") return true
    }
    // resultType annotation from analyzer
    if (expr.resultType) {
      const rt = expr.resultType.type
      if (rt === "StructType" || rt === "ArrayType" || rt === "AOSType" || rt === "CustomType" ||
          rt === "PointerType" || rt === "StringType" || rt === "MapType") {
        return true
      }
    }
    return false
  }

  private isMapType(expr: any): boolean {
    if (!expr) return false

    // If it's an identifier, look up the variable type
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      if (varType?.type === "MapType") {
        return true
      }
    }

    // Check if the expression itself has a resultType
    if (expr.resultType?.type === "MapType") {
      return true
    }

    return false
  }

  private generateSliceExpression(ir: string[], expr: any): string {
    const array = this.generateExpression(ir, expr.object)
    const start = this.generateExpression(ir, expr.start)
    const end = this.generateExpression(ir, expr.end)

    const isNestedIndex = expr.object?.type === "IndexExpression"
    let arrayPtr = array
    if (isNestedIndex) {
      const r = `%${this.valueCounter++}`
      ir.push(`  ${r} = inttoptr i64 ${array} to ptr`)
      arrayPtr = r
    }

    // Check if this is a string type [u8]
    if (this.isStringType(expr.object)) {
      const resultReg = `%${this.valueCounter++}`
      // String slicing with step not supported yet, use basic slice
      ir.push(`  ${resultReg} = call ptr @string_slice(ptr ${arrayPtr}, i64 ${start}, i64 ${end})`)
      return resultReg
    }

    const reg = `%${this.valueCounter++}`

    // Handle step parameter if provided
    if (expr.step) {
      const step = this.generateExpression(ir, expr.step)
      ir.push(`  ${reg} = call ptr @array_slice_step(ptr ${arrayPtr}, i64 ${start}, i64 ${end}, i64 ${step})`)
    } else {
      ir.push(`  ${reg} = call ptr @array_slice(ptr ${arrayPtr}, i64 ${start}, i64 ${end})`)
    }
    return reg
  }

  private generateMapExpression(ir: string[], expr: any): string {
    const collection = this.generateExpression(ir, expr.collection)
    const func = expr.function

    return collection
  }

  private generateCastExpression(ir: string[], expr: any): string {
    const value = this.generateExpression(ir, expr.value)
    const sourceType = expr.sourceType ? this.toLLVMType(expr.sourceType) : this.toLLVMType(expr.value?.resultType)
    const targetType = this.toLLVMType(expr.targetType)
    const isSourceSigned = expr.sourceType?.type === "IntegerType"
    const isTargetSigned = expr.targetType?.type === "IntegerType"
    const reg = `%${this.valueCounter++}`

    const sourceIsInt = sourceType.startsWith("i") || sourceType === "ptr"
    const targetIsInt = targetType.startsWith("i") || targetType === "ptr"
    const floatTypes = new Set(["half", "float", "double", "fp128"])
    const sourceIsFloat = floatTypes.has(sourceType)
    const targetIsFloat = floatTypes.has(targetType)

    if (sourceType === "ptr" && targetIsInt) {
      ir.push(`  ${reg} = ptrtoint ptr ${value} to ${targetType}`)
      return reg
    }

    if (targetType === "ptr" && sourceIsInt) {
      ir.push(`  ${reg} = inttoptr ${sourceType} ${value} to ptr`)
      return reg
    }

    if (sourceIsInt && targetIsInt) {
      const sourceBits = this.getIntBits(sourceType)
      const targetBits = this.getIntBits(targetType)

      if (targetBits > sourceBits) {
        const op = isSourceSigned ? "sext" : "zext"
        ir.push(`  ${reg} = ${op} ${sourceType} ${value} to ${targetType}`)
      } else if (targetBits < sourceBits) {
        ir.push(`  ${reg} = trunc ${sourceType} ${value} to ${targetType}`)
      } else {
        return value
      }
    } else if (sourceIsInt && targetIsFloat) {
      const op = isSourceSigned ? "sitofp" : "uitofp"
      ir.push(`  ${reg} = ${op} ${sourceType} ${value} to ${targetType}`)
    } else if (sourceIsFloat && targetIsInt) {
      const op = isTargetSigned ? "fptosi" : "fptoui"
      ir.push(`  ${reg} = ${op} ${sourceType} ${value} to ${targetType}`)
    } else if (sourceIsFloat && targetIsFloat) {
      const sourceBits = this.getFloatBits(sourceType)
      const targetBits = this.getFloatBits(targetType)

      if (targetBits > sourceBits) {
        ir.push(`  ${reg} = fpext ${sourceType} ${value} to ${targetType}`)
      } else if (targetBits < sourceBits) {
        ir.push(`  ${reg} = fptrunc ${sourceType} ${value} to ${targetType}`)
      } else {
        return value
      }
    }

    return reg
  }

  private generateBlock(ir: string[], statement: any): void {
    for (const stmt of statement.statements) {
      this.generateStatement(ir, stmt)
      if (this.blockTerminated) break
    }
  }

  private generateConditional(ir: string[], statement: any): void {
    const condValue = this.generateExpression(ir, statement.test || statement.condition)
    // Convert i64 condition to i1
    const condBool = `%${this.valueCounter++}`
    ir.push(`  ${condBool} = icmp ne i64 ${condValue}, 0`)
    const trueLabel = this.nextLabel()
    const falseLabel = this.nextLabel()
    const endLabel = this.nextLabel()

    ir.push(`  br i1 ${condBool}, label %${trueLabel}, label %${falseLabel}`)
    ir.push("")

    // Save blockTerminated state - branches with terminators shouldn't affect parent block
    const prevTerminated = this.blockTerminated
    this.blockTerminated = false

    ir.push(`${trueLabel}:`)
    const trueBranch = statement.trueBranch || statement.consequent
    const trueHadTerminator = this.generateBlockWithTerminator(ir, trueBranch)
    if (!trueHadTerminator) {
      ir.push(`  br label %${endLabel}`)
      ir.push("")
    }

    ir.push(`${falseLabel}:`)
    const falseBranch = statement.falseBranch || statement.alternate
    let falseHadTerminator = false
    if (falseBranch) {
      falseHadTerminator = this.generateBlockWithTerminator(ir, falseBranch)
      if (!falseHadTerminator) {
        ir.push(`  br label %${endLabel}`)
        ir.push("")
      }
    } else {
      ir.push(`  br label %${endLabel}`)
      ir.push("")
    }

    // End label is needed if either branch doesn't have a terminator
    // (because those branches will branch to the end label)
    const needsEndLabel = !trueHadTerminator || !falseHadTerminator
    if (needsEndLabel) {
      ir.push(`${endLabel}:`)
    }

    // Restore blockTerminated state unless both branches terminate
    if (!(trueHadTerminator && falseHadTerminator)) {
      this.blockTerminated = prevTerminated
    }
  }

  private generateBlockWithTerminator(ir: string[], statement: any): boolean {
    if (statement.type === "Block") {
      let hadTerminator = false
      for (const stmt of statement.statements) {
        this.generateStatement(ir, stmt)
        if (this.blockTerminated) {
          hadTerminator = true
          break
        }
      }
      return hadTerminator
    } else {
      if (statement.type === "Return" || statement.type === "Break" || statement.type === "Continue") {
        this.generateStatement(ir, statement)
        return true
      }
      this.generateStatement(ir, statement)
      return false
    }
  }

  /**
   * Emit a cooperative interrupt check before a loop back-edge.
   * Generates: load @mog_interrupt_flag, test, and conditional branch to
   * an abort block that returns MOG_INTERRUPT_CODE (-99).
   * The `backEdgeLabel` is where normal execution continues;
   * `functionExitLabel` is used for early return on interrupt.
   */
  private emitInterruptCheck(ir: string[], backEdgeLabel: string): void {
    const flagLoad = `%__intflag_${this.valueCounter++}`
    const flagTest = `%__inttest_${this.valueCounter++}`
    const interruptLabel = this.nextLabel() // label for abort path
    const continueLabel = this.nextLabel()  // label for normal back-edge

    ir.push(`  ; Cooperative interrupt check`)
    ir.push(`  ${flagLoad} = load volatile i32, ptr @mog_interrupt_flag`)
    ir.push(`  ${flagTest} = icmp ne i32 ${flagLoad}, 0`)
    ir.push(`  br i1 ${flagTest}, label %${interruptLabel}, label %${continueLabel}`)
    ir.push("")

    // Abort path: return MOG_INTERRUPT_CODE (-99)
    ir.push(`${interruptLabel}:`)
    if (this.isInAsyncFunction) {
      // In async functions, we complete the future with the error code and return
      ir.push(`  call void @mog_future_complete(ptr ${this.coroFuture}, i64 -99)`)
      // Branch to coroutine cleanup — we need to end the coroutine
      ir.push(`  br label %coro.cleanup`)
    } else if (!this.currentFunction || this.currentFunction.returnType === "void") {
      // In void functions (including top-level program()), we can't return an error code.
      // Call a helper that prints a message and exits, since there's no return value to signal.
      ir.push(`  call void @exit(i32 99)`)
      ir.push(`  unreachable`)
    } else {
      const retType = this.currentFunction?.returnType || "i64"
      if (retType === "double") {
        ir.push(`  ret double -9.9e1`)
      } else if (retType === "ptr") {
        ir.push(`  ret ptr null`)
      } else {
        ir.push(`  ret i64 -99`)
      }
    }
    ir.push("")

    // Normal path: continue to the back-edge
    ir.push(`${continueLabel}:`)
    ir.push(`  br label %${backEdgeLabel}`)
  }

  private generateWhileLoop(ir: string[], statement: any): void {
    const startLabel = this.nextLabel()
    const bodyLabel = this.nextLabel()
    const endLabel = this.nextLabel()

    // Push loop context for break/continue
    this.loopStack.push({ breakLabel: endLabel, continueLabel: startLabel })

    ir.push(`  br label %${startLabel}`)
    ir.push("")

    ir.push(`${startLabel}:`)
    const condValue = this.generateExpression(ir, statement.test || statement.condition)
    // Convert i64 condition to i1
    const condBool = `%${this.valueCounter++}`
    ir.push(`  ${condBool} = icmp ne i64 ${condValue}, 0`)
    ir.push(`  br i1 ${condBool}, label %${bodyLabel}, label %${endLabel}`)
    ir.push("")

    ir.push(`${bodyLabel}:`)
    this.generateStatement(ir, statement.body)
    if (!this.blockTerminated) {
      this.emitInterruptCheck(ir, startLabel)
    }
    ir.push("")

    ir.push(`${endLabel}:`)

    // Pop loop context
    this.loopStack.pop()
  }

  private generateForLoop(ir: string[], statement: any): void {
    const headerLabel = this.nextLabel()
    const bodyLabel = this.nextLabel()
    const incLabel = this.nextLabel()
    const endLabel = this.nextLabel()

    // Push loop context for break/continue
    this.loopStack.push({ breakLabel: endLabel, continueLabel: incLabel })

    const { variable, start, end, body } = statement

    const reg = `%${variable}`
    ir.push(`  ${reg} = alloca i64`)
    const startValue = this.generateExpression(ir, start)
    ir.push(`  store i64 ${startValue}, ptr ${reg}`)

    ir.push(`  br label %${headerLabel}`)
    ir.push("")

    ir.push(`${headerLabel}:`)
    const varValue = this.generateExpression(ir, { type: "Identifier", name: variable })
    const endValue = this.generateExpression(ir, end)
    const cond = `%${this.valueCounter++}`
    ir.push(`  ${cond} = icmp sle i64 ${varValue}, ${endValue}`)
    ir.push(`  br i1 ${cond}, label %${bodyLabel}, label %${endLabel}`)
    ir.push("")

    ir.push(`${bodyLabel}:`)
    this.generateStatement(ir, body)
    if (!this.blockTerminated) {
      ir.push(`  br label %${incLabel}`)
      ir.push("")
    }

    ir.push(`${incLabel}:`)
    const currentValue = this.generateExpression(ir, { type: "Identifier", name: variable })
    const incValue = `%${this.valueCounter++}`
    ir.push(`  ${incValue} = add i64 ${currentValue}, 1`)
    ir.push(`  store i64 ${incValue}, ptr %${variable}`)
    this.emitInterruptCheck(ir, headerLabel)
    ir.push("")

    ir.push(`${endLabel}:`)

    // Emit vectorization metadata for hot loop optimization
    // This hints to LLVM that this loop should be vectorized for SIMD operations
    if (this.opts.vectorization) {
      this.emitVectorizationMetadata(ir, headerLabel, endLabel)
    }

    // Pop loop context
    this.loopStack.pop()
  }

  private generateForEachLoop(ir: string[], statement: any): void {
    const startLabel = this.nextLabel()
    const bodyLabel = this.nextLabel()
    const incLabel = this.nextLabel()
    const endLabel = this.nextLabel()

    // Push loop context for break/continue
    this.loopStack.push({ breakLabel: endLabel, continueLabel: incLabel })
    const prevTerminated = this.blockTerminated
    this.blockTerminated = false

    const { variable, array, body } = statement

    // Generate array and get its length
    const arrayPtr = this.generateExpression(ir, array)

    // Determine element type from array type info
    const elemType = this.getArrayElementType(array)
    const isFloatElem = elemType?.type === "FloatType"
    const allocaType = isFloatElem ? "double" : "i64"

    // Use unique names for the index register to avoid conflicts with multiple loops
    const loopId = this.valueCounter++
    const indexReg = `%${variable}_idx_${loopId}`
    const valueReg = `%${variable}`

    // Allocate index variable (always unique per loop)
    ir.push(`  ${indexReg} = alloca i64`)
    ir.push(`  store i64 0, ptr ${indexReg}`)

    // Only allocate loop variable if not already declared in this function
    const existingVarType = this.variableTypes.get(variable)
    if (!existingVarType) {
      ir.push(`  ${valueReg} = alloca ${allocaType}`)
    }
    if (isFloatElem) {
      this.variableTypes.set(variable, new FloatType("f64"))
    } else {
      this.variableTypes.set(variable, { type: "IntegerType", kind: "i64" })
    }

    // Get array length
    const lengthReg = `%${this.valueCounter++}`
    ir.push(`  ${lengthReg} = call i64 @array_length(ptr ${arrayPtr})`)

    ir.push(`  br label %${startLabel}`)
    ir.push("")

    // Header: check if index < length
    ir.push(`${startLabel}:`)
    const currentIndex = `%${this.valueCounter++}`
    ir.push(`  ${currentIndex} = load i64, ptr ${indexReg}`)
    const cond = `%${this.valueCounter++}`
    ir.push(`  ${cond} = icmp slt i64 ${currentIndex}, ${lengthReg}`)
    ir.push(`  br i1 ${cond}, label %${bodyLabel}, label %${endLabel}`)
    ir.push("")

    // Body: get element and execute
    ir.push(`${bodyLabel}:`)
    const elemValue = `%${this.valueCounter++}`
    ir.push(`  ${elemValue} = call i64 @array_get(ptr ${arrayPtr}, i64 ${currentIndex})`)
    if (isFloatElem) {
      // Bitcast i64 to double for f64 array elements
      const floatVal = `%${this.valueCounter++}`
      ir.push(`  ${floatVal} = bitcast i64 ${elemValue} to double`)
      ir.push(`  store double ${floatVal}, ptr ${valueReg}`)
    } else {
      ir.push(`  store i64 ${elemValue}, ptr ${valueReg}`)
    }
    this.generateStatement(ir, body)
    ir.push(`  br label %${incLabel}`)
    ir.push("")

    // Increment
    ir.push(`${incLabel}:`)
    const nextIndex = `%${this.valueCounter++}`
    ir.push(`  ${nextIndex} = add i64 ${currentIndex}, 1`)
    ir.push(`  store i64 ${nextIndex}, ptr ${indexReg}`)
    this.emitInterruptCheck(ir, startLabel)
    ir.push("")

    ir.push(`${endLabel}:`)

    // Emit vectorization metadata for array iteration optimization
    // This enables SIMD vectorization for SoA column operations
    if (this.opts.vectorization) {
      this.emitVectorizationMetadata(ir, startLabel, endLabel)
    }

    // Pop loop context
    this.loopStack.pop()
    this.blockTerminated = prevTerminated
  }

  private generateForInRange(ir: string[], statement: any): void {
    const headerLabel = this.nextLabel()
    const bodyLabel = this.nextLabel()
    const incLabel = this.nextLabel()
    const endLabel = this.nextLabel()

    // Push loop context for break/continue
    this.loopStack.push({ breakLabel: endLabel, continueLabel: incLabel })

    const { variable, start, end, body } = statement

    const reg = `%${variable}`
    // Only allocate if not already declared in this function
    if (!this.variableTypes.has(variable)) {
      ir.push(`  ${reg} = alloca i64`)
    }
    this.variableTypes.set(variable, { type: "IntegerType", kind: "i64" })
    const startValue = this.generateExpression(ir, start)
    ir.push(`  store i64 ${startValue}, ptr ${reg}`)

    ir.push(`  br label %${headerLabel}`)
    ir.push("")

    ir.push(`${headerLabel}:`)
    const varValue = `%${this.valueCounter++}`
    ir.push(`  ${varValue} = load i64, ptr ${reg}`)
    const endValue = this.generateExpression(ir, end)
    const cond = `%${this.valueCounter++}`
    ir.push(`  ${cond} = icmp slt i64 ${varValue}, ${endValue}`)
    ir.push(`  br i1 ${cond}, label %${bodyLabel}, label %${endLabel}`)
    ir.push("")

    ir.push(`${bodyLabel}:`)
    this.blockTerminated = false
    this.generateStatement(ir, body)
    if (!this.blockTerminated) {
      ir.push(`  br label %${incLabel}`)
      ir.push("")
    }

    ir.push(`${incLabel}:`)
    const currentValue = `%${this.valueCounter++}`
    ir.push(`  ${currentValue} = load i64, ptr ${reg}`)
    const incValue = `%${this.valueCounter++}`
    ir.push(`  ${incValue} = add i64 ${currentValue}, 1`)
    ir.push(`  store i64 ${incValue}, ptr %${variable}`)
    this.emitInterruptCheck(ir, headerLabel)
    ir.push("")

    ir.push(`${endLabel}:`)
    this.blockTerminated = false

    // Pop loop context
    this.loopStack.pop()
  }

  private generateForInIndex(ir: string[], statement: any): void {
    const startLabel = this.nextLabel()
    const bodyLabel = this.nextLabel()
    const incLabel = this.nextLabel()
    const endLabel = this.nextLabel()

    // Push loop context for break/continue
    this.loopStack.push({ breakLabel: endLabel, continueLabel: incLabel })
    const prevTerminated = this.blockTerminated
    this.blockTerminated = false

    const { indexVariable, valueVariable, iterable, body } = statement

    // Generate array and get its length
    const arrayPtr = this.generateExpression(ir, iterable)
    const indexReg = `%${indexVariable}`
    const valueReg = `%${valueVariable}`

    // Determine element type from array type info
    const elemType = this.getArrayElementType(iterable)
    const isFloatElem = elemType?.type === "FloatType"
    const isStructElem = elemType?.type === "StructType"
    const allocaType = isFloatElem ? "double" : (isStructElem ? "ptr" : "i64")

    // Allocate index variable and value variable (only if not already declared)
    if (!this.variableTypes.has(indexVariable)) {
      ir.push(`  ${indexReg} = alloca i64`)
    }
    ir.push(`  store i64 0, ptr ${indexReg}`)
    if (!this.variableTypes.has(valueVariable)) {
      ir.push(`  ${valueReg} = alloca ${allocaType}`)
    }
    this.variableTypes.set(indexVariable, { type: "IntegerType", kind: "i64" })
    if (isFloatElem) {
      this.variableTypes.set(valueVariable, new FloatType("f64"))
    } else if (isStructElem) {
      this.variableTypes.set(valueVariable, elemType)
    } else {
      this.variableTypes.set(valueVariable, { type: "IntegerType", kind: "i64" })
    }

    // Get array length
    const lengthReg = `%${this.valueCounter++}`
    ir.push(`  ${lengthReg} = call i64 @array_length(ptr ${arrayPtr})`)

    ir.push(`  br label %${startLabel}`)
    ir.push("")

    // Header: check if index < length
    ir.push(`${startLabel}:`)
    const currentIndex = `%${this.valueCounter++}`
    ir.push(`  ${currentIndex} = load i64, ptr ${indexReg}`)
    const cond = `%${this.valueCounter++}`
    ir.push(`  ${cond} = icmp slt i64 ${currentIndex}, ${lengthReg}`)
    ir.push(`  br i1 ${cond}, label %${bodyLabel}, label %${endLabel}`)
    ir.push("")

    // Body: get element and execute
    ir.push(`${bodyLabel}:`)
    const elemValue = `%${this.valueCounter++}`
    ir.push(`  ${elemValue} = call i64 @array_get(ptr ${arrayPtr}, i64 ${currentIndex})`)
    if (isFloatElem) {
      // Bitcast i64 to double for f64 array elements
      const floatVal = `%${this.valueCounter++}`
      ir.push(`  ${floatVal} = bitcast i64 ${elemValue} to double`)
      ir.push(`  store double ${floatVal}, ptr ${valueReg}`)
    } else if (isStructElem) {
      // Convert i64 (stored pointer) back to ptr for struct elements
      const ptrVal = `%${this.valueCounter++}`
      ir.push(`  ${ptrVal} = inttoptr i64 ${elemValue} to ptr`)
      ir.push(`  store ptr ${ptrVal}, ptr ${valueReg}`)
    } else {
      ir.push(`  store i64 ${elemValue}, ptr ${valueReg}`)
    }
    this.generateStatement(ir, body)
    if (!this.blockTerminated) {
      ir.push(`  br label %${incLabel}`)
      ir.push("")
    }

    // Increment
    ir.push(`${incLabel}:`)
    const nextIndex = `%${this.valueCounter++}`
    ir.push(`  ${nextIndex} = add i64 ${currentIndex}, 1`)
    ir.push(`  store i64 ${nextIndex}, ptr ${indexReg}`)
    this.emitInterruptCheck(ir, startLabel)
    ir.push("")

    ir.push(`${endLabel}:`)

    // Pop loop context
    this.loopStack.pop()
    this.blockTerminated = prevTerminated
  }

  private generateForInMap(ir: string[], statement: any): void {
    const startLabel = this.nextLabel()
    const bodyLabel = this.nextLabel()
    const incLabel = this.nextLabel()
    const endLabel = this.nextLabel()

    // Push loop context for break/continue
    this.loopStack.push({ breakLabel: endLabel, continueLabel: incLabel })
    const prevTerminated = this.blockTerminated
    this.blockTerminated = false

    const { keyVariable, valueVariable, map: mapExpr, body } = statement

    // Generate map pointer and get its size
    const mapPtr = this.generateExpression(ir, mapExpr)
    const keyReg = `%${keyVariable}`
    const valueReg = `%${valueVariable}`
    const idxReg = `%__map_idx_${this.valueCounter++}`

    // Determine map value type for float-aware handling
    const mapValueType = this.getMapValueType(mapExpr)
    const isFloatValue = mapValueType?.type === "FloatType"
    const valueAllocaType = isFloatValue ? "double" : "i64"

    // Allocate loop index, key variable, and value variable
    ir.push(`  ${idxReg} = alloca i64`)
    ir.push(`  store i64 0, ptr ${idxReg}`)
    ir.push(`  ${keyReg} = alloca i64`)
    ir.push(`  ${valueReg} = alloca ${valueAllocaType}`)
    this.variableTypes.set(keyVariable, { type: "PointerType" })
    if (isFloatValue) {
      this.variableTypes.set(valueVariable, new FloatType("f64"))
    } else {
      this.variableTypes.set(valueVariable, { type: "IntegerType", kind: "i64" })
    }

    // Get map size
    const sizeReg = `%${this.valueCounter++}`
    ir.push(`  ${sizeReg} = call i64 @map_size(ptr ${mapPtr})`)

    ir.push(`  br label %${startLabel}`)
    ir.push("")

    // Header: check if index < size
    ir.push(`${startLabel}:`)
    const currentIdx = `%${this.valueCounter++}`
    ir.push(`  ${currentIdx} = load i64, ptr ${idxReg}`)
    const cond = `%${this.valueCounter++}`
    ir.push(`  ${cond} = icmp slt i64 ${currentIdx}, ${sizeReg}`)
    ir.push(`  br i1 ${cond}, label %${bodyLabel}, label %${endLabel}`)
    ir.push("")

    // Body: get key and value at current index
    ir.push(`${bodyLabel}:`)
    const keyPtr = `%${this.valueCounter++}`
    ir.push(`  ${keyPtr} = call ptr @map_key_at(ptr ${mapPtr}, i64 ${currentIdx})`)
    // Store key pointer as i64 (ptrtoint)
    const keyAsInt = `%${this.valueCounter++}`
    ir.push(`  ${keyAsInt} = ptrtoint ptr ${keyPtr} to i64`)
    ir.push(`  store i64 ${keyAsInt}, ptr ${keyReg}`)

    const valResult = `%${this.valueCounter++}`
    ir.push(`  ${valResult} = call i64 @map_value_at(ptr ${mapPtr}, i64 ${currentIdx})`)
    if (isFloatValue) {
      const floatVal = `%${this.valueCounter++}`
      ir.push(`  ${floatVal} = bitcast i64 ${valResult} to double`)
      ir.push(`  store double ${floatVal}, ptr ${valueReg}`)
    } else {
      ir.push(`  store i64 ${valResult}, ptr ${valueReg}`)
    }

    this.generateStatement(ir, body)
    if (!this.blockTerminated) {
      ir.push(`  br label %${incLabel}`)
      ir.push("")
    }

    // Increment
    ir.push(`${incLabel}:`)
    const nextIdx = `%${this.valueCounter++}`
    ir.push(`  ${nextIdx} = add i64 ${currentIdx}, 1`)
    ir.push(`  store i64 ${nextIdx}, ptr ${idxReg}`)
    this.emitInterruptCheck(ir, startLabel)
    ir.push("")

    ir.push(`${endLabel}:`)

    // Pop loop context
    this.loopStack.pop()
    this.blockTerminated = prevTerminated
  }

  private generateIfExpression(ir: string[], expr: any): string {
    const condValue = this.generateExpression(ir, expr.condition)
    // Convert i64 condition to i1
    const condBool = `%${this.valueCounter++}`
    ir.push(`  ${condBool} = icmp ne i64 ${condValue}, 0`)

    const trueLabel = this.nextLabel()
    const falseLabel = this.nextLabel()
    const endLabel = this.nextLabel()

    // Allocate a place to store the result
    const resultReg = `%ifexpr_result_${this.valueCounter++}`
    ir.push(`  ${resultReg} = alloca i64`)

    ir.push(`  br i1 ${condBool}, label %${trueLabel}, label %${falseLabel}`)
    ir.push("")

    // True branch
    ir.push(`${trueLabel}:`)
    const trueValue = this.generateBlockExpression(ir, expr.trueBranch)
    ir.push(`  store i64 ${trueValue}, ptr ${resultReg}`)
    ir.push(`  br label %${endLabel}`)
    ir.push("")

    // False branch
    ir.push(`${falseLabel}:`)
    if (expr.falseBranch) {
      let falseValue: string
      if (expr.falseBranch.type === "IfExpression") {
        falseValue = this.generateIfExpression(ir, expr.falseBranch)
      } else {
        falseValue = this.generateBlockExpression(ir, expr.falseBranch)
      }
      ir.push(`  store i64 ${falseValue}, ptr ${resultReg}`)
    } else {
      ir.push(`  store i64 0, ptr ${resultReg}`)
    }
    ir.push(`  br label %${endLabel}`)
    ir.push("")

    ir.push(`${endLabel}:`)
    const result = `%${this.valueCounter++}`
    ir.push(`  ${result} = load i64, ptr ${resultReg}`)
    return result
  }

  private generateBlockExpression(ir: string[], block: any): string {
    // Generate all statements in a block, returning the value of the last expression statement
    if (block.type === "Block") {
      let lastValue = "0"
      for (const stmt of block.statements) {
        if (stmt.type === "ExpressionStatement") {
          lastValue = this.generateExpression(ir, stmt.expression)
        } else {
          this.generateStatement(ir, stmt)
          // Reset to default after non-expression statements (e.g., for loops, if blocks)
          // so the block value reflects only the last expression, not an earlier one
          lastValue = "0"
        }
      }
      return lastValue
    }
    // Single expression
    return this.generateExpression(ir, block)
  }

  private generateLambda(ir: string[], expr: any): string {
    // Lift the lambda to a top-level function with a unique name
    const lambdaName = `__lambda_${this.lambdaCounter++}`
    const params = expr.params || []
    const returnType = expr.returnType
    const body = expr.body
    const capturedVars: string[] = expr.capturedVars || []
    const capturedVarTypes: Record<string, any> = expr.capturedVarTypes || {}

    // Save current state (including async state to prevent coroutine-style returns in non-async lambdas)
    const savedFunction = this.currentFunction
    const savedBlockTerminated = this.blockTerminated
    const savedValueCounter = this.valueCounter
    const savedVariableTypes = new Map(this.variableTypes)
    const savedIsInAsyncFunction = this.isInAsyncFunction
    const savedCoroHandle = this.coroHandle
    const savedCoroFuture = this.coroFuture
    const savedCoroId = this.coroId
    const savedAwaitCounter = this.awaitCounter

    // Lambdas are NOT async (even when defined inside async functions)
    this.isInAsyncFunction = false
    this.coroHandle = ""
    this.coroFuture = ""
    this.coroId = ""
    this.awaitCounter = 0

    // Generate the lambda function IR into lambdaIR buffer
    const lambdaIrBuf: string[] = []

    this.resetValueCounter(10)

    // Build param list WITH env as first param
    const llvmParams: LLVMValue[] = [{ name: "__env", type: "ptr" as LLVMType }]
    for (const p of params) {
      const llvmType = this.toLLVMType(p.paramType)
      llvmParams.push({ name: p.name, type: llvmType })
    }

    const llvmReturnType = this.toLLVMType(returnType)

    const func: LLVMFunction = {
      name: lambdaName,
      returnType: llvmReturnType,
      params: llvmParams,
      body: [],
    }

    this.functions.set(lambdaName, func)
    this.functionTypes.set(lambdaName, returnType)
    this.currentFunction = func

    const paramStr = llvmParams.map((p: LLVMValue) => `${p.type} %${p.name}`).join(", ")

    lambdaIrBuf.push(`define ${llvmReturnType} @${lambdaName}(${paramStr}) nounwind {`)
    lambdaIrBuf.push("entry:")
    lambdaIrBuf.push("  call void @gc_push_frame()")

    // Copy user params to local allocas (skip __env)
    for (const param of llvmParams) {
      if (param.name === "__env") continue
      const localReg = `%${param.name}_local`
      lambdaIrBuf.push(`  ${localReg} = alloca ${param.type}`)
      lambdaIrBuf.push(`  store ${param.type} %${param.name}, ptr ${localReg}`)
      const paramType = params.find((p: any) => p.name === param.name)?.paramType
      if (paramType) {
        this.variableTypes.set(param.name, paramType)
      }
    }

    // Load captured variables from env struct
    for (let i = 0; i < capturedVars.length; i++) {
      const varName = capturedVars[i]
      const offset = i * 8
      const capturedType = capturedVarTypes[varName]
      const isFloat = capturedType?.type === "FloatType"
      const isPtr = capturedType?.type === "ArrayType" || capturedType?.type === "PointerType" ||
                    capturedType?.type === "StructType" || capturedType?.type === "CustomType" ||
                    capturedType?.type === "SOAType" || capturedType?.type === "AOSType" || capturedType?.type === "FunctionType" ||
                    capturedType?.type === "MapType" || capturedType?.type === "StringType" ||
                    capturedType?.type === "TensorType"
      const llvmType = isFloat ? "double" : (isPtr ? "ptr" : "i64")

      const gepReg = `%__env_gep_${i}`
      lambdaIrBuf.push(`  ${gepReg} = getelementptr i8, ptr %__env, i64 ${offset}`)
      const loadReg = `%__env_load_${i}`
      lambdaIrBuf.push(`  ${loadReg} = load ${llvmType}, ptr ${gepReg}`)
      // Create a local alloca so the rest of the body can use %varName as before
      lambdaIrBuf.push(`  %${varName} = alloca ${llvmType}`)
      lambdaIrBuf.push(`  store ${llvmType} ${loadReg}, ptr %${varName}`)
      // Register the variable type so codegen works correctly
      if (capturedType) {
        this.variableTypes.set(varName, capturedType)
      }
    }
    lambdaIrBuf.push("")

    this.blockTerminated = false
    for (const stmt of body.statements) {
      this.generateStatement(lambdaIrBuf, stmt)
      if (this.blockTerminated) break
    }

    let hasReturn = false
    for (const stmt of body.statements) {
      if (stmt.type === "Return") {
        hasReturn = true
        break
      }
    }

    if (!hasReturn) {
      lambdaIrBuf.push("  call void @gc_pop_frame()")
      if (llvmReturnType !== "void") {
        lambdaIrBuf.push(`  ret ${llvmReturnType} 0`)
      } else {
        lambdaIrBuf.push("  ret void")
      }
    }

    lambdaIrBuf.push("}")
    lambdaIrBuf.push("")

    // Add to lambdaIR for later emission
    this.lambdaIR.push(...lambdaIrBuf)

    // Restore state (including async state)
    this.currentFunction = savedFunction
    this.blockTerminated = savedBlockTerminated
    this.valueCounter = savedValueCounter
    this.variableTypes = savedVariableTypes
    this.isInAsyncFunction = savedIsInAsyncFunction
    this.coroHandle = savedCoroHandle
    this.coroFuture = savedCoroFuture
    this.coroId = savedCoroId
    this.awaitCounter = savedAwaitCounter

    // Create closure struct in the CALLER's IR context
    if (capturedVars.length > 0) {
      // Allocate environment with gc_alloc_closure
      const envReg = `%${this.valueCounter++}`
      ir.push(`  ${envReg} = call ptr @gc_alloc_closure(i64 ${capturedVars.length})`)

      // Store captured values into environment
      for (let i = 0; i < capturedVars.length; i++) {
        const varName = capturedVars[i]
        const capturedType = capturedVarTypes[varName]
        const isFloat = capturedType?.type === "FloatType"
        const isPtr = capturedType?.type === "ArrayType" || capturedType?.type === "PointerType" ||
                      capturedType?.type === "StructType" || capturedType?.type === "CustomType" ||
                      capturedType?.type === "SOAType" || capturedType?.type === "AOSType" || capturedType?.type === "FunctionType" ||
                      capturedType?.type === "MapType" || capturedType?.type === "StringType" ||
                      capturedType?.type === "TensorType"
        const llvmType = isFloat ? "double" : (isPtr ? "ptr" : "i64")

        // Determine the correct alloca name: parameters use %name_local, variables use %name
        const isParamInCaller = this.currentFunction && this.currentFunction.params.find((p) => p.name === varName)
        const loadFrom = isParamInCaller ? `%${varName}_local` : `%${varName}`

        const varReg = `%${this.valueCounter++}`
        ir.push(`  ${varReg} = load ${llvmType}, ptr ${loadFrom}`)

        const slotPtr = `%${this.valueCounter++}`
        ir.push(`  ${slotPtr} = getelementptr i8, ptr ${envReg}, i64 ${i * 8}`)

        if (isFloat) {
          // Bitcast double to i64 for uniform 8-byte storage
          const intReg = `%${this.valueCounter++}`
          ir.push(`  ${intReg} = bitcast double ${varReg} to i64`)
          ir.push(`  store i64 ${intReg}, ptr ${slotPtr}`)
        } else {
          ir.push(`  store ${llvmType} ${varReg}, ptr ${slotPtr}`)
        }
      }

      // Allocate closure pair (fn_ptr + env_ptr)
      const closureReg = `%${this.valueCounter++}`
      ir.push(`  ${closureReg} = call ptr @gc_alloc(i64 16)`)
      ir.push(`  store ptr @${lambdaName}, ptr ${closureReg}`)
      const envSlot = `%${this.valueCounter++}`
      ir.push(`  ${envSlot} = getelementptr i8, ptr ${closureReg}, i64 8`)
      ir.push(`  store ptr ${envReg}, ptr ${envSlot}`)

      return closureReg
    } else {
      // No captures — still create closure pair but with null env
      const closureReg = `%${this.valueCounter++}`
      ir.push(`  ${closureReg} = call ptr @gc_alloc(i64 16)`)
      ir.push(`  store ptr @${lambdaName}, ptr ${closureReg}`)
      const envSlot = `%${this.valueCounter++}`
      ir.push(`  ${envSlot} = getelementptr i8, ptr ${closureReg}, i64 8`)
      ir.push(`  store ptr null, ptr ${envSlot}`)

      return closureReg
    }
  }

  /**
   * Generate a wrapper function for a named function so it can be used as a closure value.
   * Named functions don't have the `ptr %__env` first parameter, so we generate a thin
   * wrapper that accepts __env (and ignores it) and forwards the call.
   */
  private getOrCreateFunctionWrapper(ir: string[], funcName: string): string {
    const wrapperName = `__wrap_${funcName}`
    
    if (!this.functionWrappers.has(funcName)) {
      this.functionWrappers.add(funcName)
      
      const funcInfo = this.functions.get(funcName)
      if (funcInfo) {
        const wrapperParams: string[] = ["ptr %__env"]
        const forwardArgs: string[] = []
        
        for (let i = 0; i < funcInfo.params.length; i++) {
          const p = funcInfo.params[i]
          wrapperParams.push(`${p.type} %__p${i}`)
          forwardArgs.push(`${p.type} %__p${i}`)
        }
        
        const retType = funcInfo.returnType
        
        this.wrapperIR.push(`define ${retType} @${wrapperName}(${wrapperParams.join(", ")}) nounwind {`)
        this.wrapperIR.push("entry:")
        if (retType === "void") {
          this.wrapperIR.push(`  call void @${funcName}(${forwardArgs.join(", ")})`)
          this.wrapperIR.push("  ret void")
        } else {
          this.wrapperIR.push(`  %__result = call ${retType} @${funcName}(${forwardArgs.join(", ")})`)
          this.wrapperIR.push(`  ret ${retType} %__result`)
        }
        this.wrapperIR.push("}")
        this.wrapperIR.push("")
      }
    }
    
    // Create a closure pair with the wrapper and null env
    const closureReg = `%${this.valueCounter++}`
    ir.push(`  ${closureReg} = call ptr @gc_alloc(i64 16)`)
    ir.push(`  store ptr @${wrapperName}, ptr ${closureReg}`)
    const envSlot = `%${this.valueCounter++}`
    ir.push(`  ${envSlot} = getelementptr i8, ptr ${closureReg}, i64 8`)
    ir.push(`  store ptr null, ptr ${envSlot}`)
    
    return closureReg
  }

  private generateMatchExpression(ir: string[], expr: any): string {
    const subjectValue = this.generateExpression(ir, expr.subject)

    // Determine match result type by inspecting arm bodies
    // Default to i64, use ptr for string-producing arms, double for float-producing arms
    const arms = expr.arms as any[]
    let matchResultType: string = "i64"
    for (const arm of arms) {
      const bodyExpr = arm.body
      if (bodyExpr?.type === "StringLiteral" || bodyExpr?.resultType?.type === "StringType") {
        matchResultType = "ptr"
        break
      }
      if (this.isFloatProducingExpression(bodyExpr)) {
        matchResultType = "double"
        break
      }
    }

    // Allocate result
    const resultReg = `%match_result_${this.valueCounter++}`
    ir.push(`  ${resultReg} = alloca ${matchResultType}`)

    const endLabel = this.nextLabel()

    for (let i = 0; i < arms.length; i++) {
      const arm = arms[i]
      const isLast = i === arms.length - 1

      if (arm.pattern.type === "WildcardPattern") {
        // Wildcard always matches - generate arm body and jump to end
        const armValue = this.generateBlockExpression(ir, arm.body)
        ir.push(`  store ${matchResultType} ${armValue || "0"}, ptr ${resultReg}`)
        ir.push(`  br label %${endLabel}`)
        ir.push("")
        break
      }

      const matchBodyLabel = this.nextLabel()
      const nextArmLabel = isLast ? endLabel : this.nextLabel()

      if (arm.pattern.type === "LiteralPattern") {
        // Generate the pattern value
        const patternValue = this.generateExpression(ir, arm.pattern.value)
        const cmpResult = `%${this.valueCounter++}`
        // Use fcmp for float subjects, icmp for integers
        if (this.isFloatProducingExpression(expr.subject)) {
          ir.push(`  ${cmpResult} = fcmp oeq double ${subjectValue}, ${patternValue}`)
        } else {
          ir.push(`  ${cmpResult} = icmp eq i64 ${subjectValue}, ${patternValue}`)
        }
        ir.push(`  br i1 ${cmpResult}, label %${matchBodyLabel}, label %${nextArmLabel}`)
        ir.push("")
      } else if (arm.pattern.type === "VariantPattern") {
        // Handle ok(val), err(msg), some(val) patterns on Result/Optional
        const variantName = arm.pattern.name
        const subjectPtr = `%match_ptr_${this.valueCounter++}`
        ir.push(`  ${subjectPtr} = inttoptr i64 ${subjectValue} to ptr`)

        if (variantName === "ok" || variantName === "some") {
          // Check tag == 0 for ok, tag == 1 for some (is_some)
          const checkFn = variantName === "ok" ? "@mog_result_is_ok" : "@mog_optional_is_some"
          const isMatch = `%${this.valueCounter++}`
          ir.push(`  ${isMatch} = call i64 ${checkFn}(ptr ${subjectPtr})`)
          const cmpResult = `%${this.valueCounter++}`
          ir.push(`  ${cmpResult} = icmp ne i64 ${isMatch}, 0`)
          ir.push(`  br i1 ${cmpResult}, label %${matchBodyLabel}, label %${nextArmLabel}`)
          ir.push("")

          // Arm body - bind the unwrapped value if there's a binding
          ir.push(`${matchBodyLabel}:`)
          if (arm.pattern.binding) {
            const unwrapFn = variantName === "ok" ? "@mog_result_unwrap" : "@mog_optional_unwrap"
            const unwrapped = `%${this.valueCounter++}`
            ir.push(`  ${unwrapped} = call i64 ${unwrapFn}(ptr ${subjectPtr})`)
            // Determine the inner type from the subject's resultType (Result<T> or Optional<T>)
            const subjectResultType = expr.subject?.resultType
            const innerType = subjectResultType?.innerType
            const isInnerPtr = innerType && (innerType.type === "ArrayType" || innerType.type === "AOSType" ||
              innerType.type === "StructType" || innerType.type === "StringType" || innerType.type === "PointerType" ||
              innerType.type === "MapType")
            const bindingReg = `%${arm.pattern.binding}`
            if (isInnerPtr) {
              ir.push(`  ${bindingReg} = alloca ptr`)
              const asPtr = `%${this.valueCounter++}`
              ir.push(`  ${asPtr} = inttoptr i64 ${unwrapped} to ptr`)
              ir.push(`  store ptr ${asPtr}, ptr ${bindingReg}`)
            } else {
              ir.push(`  ${bindingReg} = alloca i64`)
              ir.push(`  store i64 ${unwrapped}, ptr ${bindingReg}`)
            }
            // Set the variable type to the inner type for proper method dispatch
            if (innerType) {
              this.variableTypes.set(arm.pattern.binding, innerType)
            } else {
              this.variableTypes.set(arm.pattern.binding, new IntegerType("i64"))
            }
          }
        } else if (variantName === "err") {
          // Check tag == 1 for err (is_ok == 0)
          const isOk = `%${this.valueCounter++}`
          ir.push(`  ${isOk} = call i64 @mog_result_is_ok(ptr ${subjectPtr})`)
          const cmpResult = `%${this.valueCounter++}`
          ir.push(`  ${cmpResult} = icmp eq i64 ${isOk}, 0`)
          ir.push(`  br i1 ${cmpResult}, label %${matchBodyLabel}, label %${nextArmLabel}`)
          ir.push("")

          // Arm body - bind the error message
          ir.push(`${matchBodyLabel}:`)
          if (arm.pattern.binding) {
            const errMsg = `%${this.valueCounter++}`
            ir.push(`  ${errMsg} = call ptr @mog_result_unwrap_err(ptr ${subjectPtr})`)
            const errInt = `%${this.valueCounter++}`
            ir.push(`  ${errInt} = ptrtoint ptr ${errMsg} to i64`)
            const bindingReg = `%${arm.pattern.binding}`
            ir.push(`  ${bindingReg} = alloca i64`)
            ir.push(`  store i64 ${errInt}, ptr ${bindingReg}`)
          }
        } else if (variantName === "none") {
          // Check is_some == 0
          const isSome = `%${this.valueCounter++}`
          ir.push(`  ${isSome} = call i64 @mog_optional_is_some(ptr ${subjectPtr})`)
          const cmpResult = `%${this.valueCounter++}`
          ir.push(`  ${cmpResult} = icmp eq i64 ${isSome}, 0`)
          ir.push(`  br i1 ${cmpResult}, label %${matchBodyLabel}, label %${nextArmLabel}`)
          ir.push("")
          ir.push(`${matchBodyLabel}:`)
        } else {
          // Unknown variant, skip
          ir.push(`  br label %${matchBodyLabel}`)
          ir.push(`${matchBodyLabel}:`)
        }

        const armValue = this.generateBlockExpression(ir, arm.body)
        ir.push(`  store ${matchResultType} ${armValue || "0"}, ptr ${resultReg}`)
        ir.push(`  br label %${endLabel}`)
        ir.push("")

        if (!isLast) {
          ir.push(`${nextArmLabel}:`)
        }
        continue
      }

      // Arm body
      ir.push(`${matchBodyLabel}:`)
      const armValue = this.generateBlockExpression(ir, arm.body)
      ir.push(`  store ${matchResultType} ${armValue || "0"}, ptr ${resultReg}`)
      ir.push(`  br label %${endLabel}`)
      ir.push("")

      // If not the last arm and not wildcard, emit the next arm label
      if (!isLast) {
        ir.push(`${nextArmLabel}:`)
      }
    }

    ir.push(`${endLabel}:`)
    const result = `%${this.valueCounter++}`
    ir.push(`  ${result} = load ${matchResultType}, ptr ${resultReg}`)
    return result
  }

  private generateBreak(ir: string[]): void {
    if (this.loopStack.length === 0) {
      // Error: break outside of loop - this should be caught by analyzer
      return
    }
    const loopContext = this.loopStack[this.loopStack.length - 1]
    ir.push(`  br label %${loopContext.breakLabel}`)
    this.blockTerminated = true
  }

  private generateContinue(ir: string[]): void {
    if (this.loopStack.length === 0) {
      // Error: continue outside of loop - this should be caught by analyzer
      return
    }
    const loopContext = this.loopStack[this.loopStack.length - 1]
    ir.push(`  br label %${loopContext.continueLabel}`)
    this.blockTerminated = true
  }

  private generateFunctionDeclaration(ir: string[], statement: any): void {
    const { name, params, returnType, body } = statement
    const isAsync = statement.type === "AsyncFunctionDeclaration" || this.asyncFunctions.has(name)

    if (isAsync) {
      this.generateAsyncFunctionDeclaration(ir, statement)
      return
    }

    this.resetValueCounter(10)
    this.currentFunctionBasicBlocks = 0

    const llvmParams = params.map((p: any) => {
      const llvmType = this.toLLVMType(p.paramType)
      return { name: p.name, type: llvmType }
    })

    const llvmReturnType = this.toLLVMType(returnType)

    const func: LLVMFunction = {
      name,
      returnType: llvmReturnType,
      params: llvmParams,
      body: [],
    }

    this.functions.set(name, func)
    this.functionTypes.set(name, returnType)
    this.currentFunction = func

    // Save and reset per-function variable types to prevent leaking across functions
    const savedVariableTypes = new Map(this.variableTypes)

    const paramStr = llvmParams.map((p: LLVMValue) => `${p.type} %${p.name}`).join(", ")

    // Determine function attributes based on optimization settings
    // Small functions (few params, simple body) get alwaysinline for hot path optimization
    const funcAttributes = this.determineFunctionAttributes(statement)

    // Build function definition with proper LLVM syntax:
    // define [internal] <returntype> @<name>(<params>) [fn-attributes] {
    // Note: function attributes like nounwind, alwaysinline go after params, not before return type
    const attrSuffix = funcAttributes.trim()

    // In plugin mode, non-pub functions get internal linkage (hidden in .dylib)
    const linkage = (this.pluginMode && !statement.isPublic) ? "internal " : ""

    ir.push(`define ${linkage}${llvmReturnType} @${name}(${paramStr})${attrSuffix} {`)
    ir.push("entry:")
    ir.push("  call void @gc_push_frame()")

    for (const param of llvmParams) {
      const paramReg = `%${param.name}`
      const localReg = `%${param.name}_local`
      ir.push(`  ${localReg} = alloca ${param.type}`)
      ir.push(`  store ${param.type} ${paramReg}, ptr ${localReg}`)
      // Track parameter type for later loads
      const paramType = params.find((p: any) => p.name === param.name)?.paramType
      if (paramType) {
        this.variableTypes.set(param.name, paramType)
      }
    }
    ir.push("")

    this.blockTerminated = false
    for (const stmt of body.statements) {
      this.generateStatement(ir, stmt)
      if (this.blockTerminated) break
    }

    let hasReturn = false
    for (const stmt of body.statements) {
      if (stmt.type === "Return") {
        hasReturn = true
        break
      }
    }

    if (!hasReturn) {
      ir.push("  call void @gc_pop_frame()")
      if (llvmReturnType !== "void") {
        ir.push(`  ret ${llvmReturnType} 0`)
      } else {
        ir.push("  ret void")
      }
    }


    ir.push("}")
    ir.push("")

    // Restore variable types from outer scope
    this.variableTypes = savedVariableTypes
    this.currentFunction = null
  }

  private generateAsyncFunctionDeclaration(ir: string[], statement: any): void {
    const { name, params, returnType, body } = statement

    this.resetValueCounter(10)
    this.currentFunctionBasicBlocks = 0
    this.awaitCounter = 0

    const llvmParams = params.map((p: any) => {
      const llvmType = this.toLLVMType(p.paramType)
      return { name: p.name, type: llvmType }
    })

    // Async functions return ptr (MogFuture*)
    const func: LLVMFunction = {
      name,
      returnType: "ptr",
      params: llvmParams,
      body: [],
    }

    this.functions.set(name, func)
    this.functionTypes.set(name, returnType)
    this.currentFunction = func

    const paramStr = llvmParams.map((p: LLVMValue) => `${p.type} %${p.name}`).join(", ")

    // Async functions use presplitcoroutine attribute (required for LLVM coroutine passes)
    // In plugin mode, non-pub async functions get internal linkage
    const asyncLinkage = (this.pluginMode && !statement.isPublic) ? "internal " : ""
    ir.push(`define ${asyncLinkage}ptr @${name}(${paramStr}) presplitcoroutine {`)
    ir.push("entry:")

    // Coroutine setup
    ir.push("  %coro.id = call token @llvm.coro.id(i32 0, ptr null, ptr null, ptr null)")
    ir.push("  %coro.size = call i64 @llvm.coro.size.i64()")
    ir.push("  %coro.mem = call ptr @malloc(i64 %coro.size)")
    ir.push("  %coro.hdl = call ptr @llvm.coro.begin(token %coro.id, ptr %coro.mem)")
    ir.push("")

    // Create the future that represents this async function's result
    ir.push("  %coro.future = call ptr @mog_future_new()")
    ir.push("")

    // No initial suspend - the function runs eagerly. If it completes without
    // hitting an await, the future is completed inline and returned directly.
    // Set up GC frame and local variables
    ir.push("  call void @gc_push_frame()")

    // Save async state
    const prevIsAsync = this.isInAsyncFunction
    const prevCoroHandle = this.coroHandle
    const prevCoroFuture = this.coroFuture
    const prevCoroId = this.coroId
    const prevAwaitCounter = this.awaitCounter

    this.isInAsyncFunction = true
    this.coroHandle = "%coro.hdl"
    this.coroFuture = "%coro.future"
    this.coroId = "%coro.id"
    this.awaitCounter = 0

    // Allocate local variables for parameters
    for (const param of llvmParams) {
      const paramReg = `%${param.name}`
      const localReg = `%${param.name}_local`
      ir.push(`  ${localReg} = alloca ${param.type}`)
      ir.push(`  store ${param.type} ${paramReg}, ptr ${localReg}`)
      const paramType = params.find((p: any) => p.name === param.name)?.paramType
      if (paramType) {
        this.variableTypes.set(param.name, paramType)
      }
    }
    ir.push("")

    // Generate function body
    this.blockTerminated = false
    for (const stmt of body.statements) {
      this.generateStatement(ir, stmt)
      if (this.blockTerminated) break
    }

    // If no explicit return, complete the future with 0
    if (!this.blockTerminated) {
      ir.push(`  call void @mog_future_complete(ptr %coro.future, i64 0)`)
      ir.push(`  br label %coro.cleanup`)
    }

    // Cleanup label - coroutine is done (destroy path)
    // IMPORTANT: We do NOT free the coroutine frame here because LLVM's
    // coroutine pass spills the future pointer into the frame. If we free
    // the frame and then try to return the future pointer, we get a
    // use-after-free. Instead, we transfer frame ownership to the future
    // via mog_future_set_coro_frame, and the frame is freed when the
    // future is freed.
    ir.push("")
    ir.push("coro.cleanup:")
    ir.push("  call void @gc_pop_frame()")
    ir.push("  %coro.mem.cleanup = call ptr @llvm.coro.free(token %coro.id, ptr %coro.hdl)")
    ir.push("  call void @mog_future_set_coro_frame(ptr %coro.future, ptr %coro.mem.cleanup)")
    ir.push("  br label %coro.suspend")

    // Suspend label - shared exit point for both suspend and cleanup paths.
    // Both the switch default case (suspend) and cleanup branch here.
    // coro.end marks where the coroutine returns control to the caller;
    // in the resume function this becomes ret void.
    ir.push("")
    ir.push("coro.suspend:")
    ir.push(`  %coro.end.unused = call i1 @llvm.coro.end(ptr ${this.coroHandle}, i1 false, token none)`)
    ir.push("  ret ptr %coro.future")

    ir.push("}")
    ir.push("")

    // Restore state
    this.isInAsyncFunction = prevIsAsync
    this.coroHandle = prevCoroHandle
    this.coroFuture = prevCoroFuture
    this.coroId = prevCoroId
    this.awaitCounter = prevAwaitCounter
    this.currentFunction = null
  }

  /**
   * Determine LLVM function attributes for optimization
   * Small functions get alwaysinline for hot path optimization
   */
  private determineFunctionAttributes(statement: any): string {
    const attrs: string[] = []
    const body = statement.body

    // Async functions use presplitcoroutine - don't add other attributes that conflict
    const isAsync = statement.type === "AsyncFunctionDeclaration" || this.asyncFunctions.has(statement.name)
    if (isAsync) {
      return " presplitcoroutine"
    }

    // Count basic blocks by checking for control flow statements
    let basicBlockCount = 1 // entry block
    if (body?.statements) {
      for (const stmt of body.statements) {
        if (stmt.type === "Conditional" || stmt.type === "WhileLoop" ||
            stmt.type === "ForLoop" || stmt.type === "ForEachLoop") {
          basicBlockCount++
        }
      }
    }

    // Small functions: mark as alwaysinline for hot path optimization
    if (basicBlockCount <= this.opts.inlineThreshold) {
      attrs.push("alwaysinline")
    }

    // Inline functions with small struct operations for hot path optimization
    // This enables efficient struct field access and copy operations
    if (this.hasSmallStructOperations(statement)) {
      if (!attrs.includes("alwaysinline")) {
        attrs.push("alwaysinline")
      }
    }

    // Add nounwind for functions that don't unwind (no exceptions in Mog)
    attrs.push("nounwind")

    // Return attributes as suffix (with leading space if any attributes present)
    return attrs.length > 0 ? ` ${attrs.join(" ")}` : ""
  }

  /**
   * Emit LLVM metadata for loop vectorization
   * This hints to the LLVM optimizer to vectorize array operations
   */
  private emitVectorizationMetadata(ir: string[], loopStart: string, loopEnd: string): void {
    if (!this.opts.vectorization) return

    // Add a metadata node to mark this loop as vectorizable
    // In full LLVM IR, this would be: !0 = !{!"llvm.loop.vectorize.enable", i1 1}
    // For now, we add a comment hint that the optimizer can use
    ir.push(`  ; VECTORIZE_HINT: loop ${loopStart} -> ${loopEnd}`)
  }

  /**
   * Check if bounds checks can be elided (release mode only)
   * In release mode, skip bounds checks for performance
   */
  private shouldElideBoundsCheck(): boolean {
    return this.opts.releaseMode
  }

  /**
   * Determine if a function operates on small structs that should be inlined
   * This enables hot path optimization for struct field access and copy operations
   */
  private hasSmallStructOperations(statement: any): boolean {
    // Check if function body contains struct operations
    // Small struct operations benefit from inlining
    const body = statement.body
    if (!body?.statements) return false

    for (const stmt of body.statements) {
      // Check for struct-related operations
      if (this.containsStructOperations(stmt)) {
        return true
      }
    }
    return false
  }

  /**
   * Recursively check if a statement contains struct operations
   */
  private containsStructOperations(stmt: any): boolean {
    if (!stmt) return false

    // Check for struct field access or assignment
    if (stmt.type === "MemberExpression" ||
        (stmt.type === "AssignmentStatement" &&
         stmt.target?.type === "MemberExpression")) {
      // This could be a struct field operation - check if it uses inline hint
      return true
    }

    // Recursively check nested statements
    if (stmt.body?.statements) {
      for (const s of stmt.body.statements) {
        if (this.containsStructOperations(s)) return true
      }
    }

    return false
  }

  /**
   * Emit inline hint for small struct operations
   * Marks struct copy/field access operations for LLVM inlining
   */
  private emitStructInlineHint(ir: string[], structName: string, operation: string): void {
    // Emit a comment hint that LLVM can use for inlining decisions
    ir.push(`  ; STRUCT_INLINE_HINT: ${structName} ${operation}`)
  }

  private generateReturn(ir: string[], statement: any): void {
    if (statement.value) {
      const reg = this.generateExpression(ir, statement.value)
      const returnType = this.currentFunction?.returnType || "i64"
      ir.push(`  ret ${returnType} ${reg}`)
    } else {
      ir.push("  ret void")
    }
  }

  // ============================================================
  // Plugin protocol: three exported functions for .dylib plugins
  // ============================================================

  private generatePluginInfo(ir: string[]): void {
    ir.push("")
    ir.push("; Plugin info constants")

    // Plugin name string constant
    const nameEsc = this.escapeStringForLLVM(this.pluginName)
    const nameLen = nameEsc.byteLength + 1 // +1 for null terminator
    ir.push(`@.plugin_name = private unnamed_addr constant [${nameLen} x i8] c"${nameEsc.escaped}\\00"`)

    // Plugin version string constant
    const verEsc = this.escapeStringForLLVM(this.pluginVersion)
    const verLen = verEsc.byteLength + 1
    ir.push(`@.plugin_version = private unnamed_addr constant [${verLen} x i8] c"${verEsc.escaped}\\00"`)

    // Required capabilities: null-terminated array of i8* pointers
    const caps = Array.from(this.capabilities)
    for (let i = 0; i < caps.length; i++) {
      const capEsc = this.escapeStringForLLVM(caps[i])
      const capLen = capEsc.byteLength + 1
      ir.push(`@.plugin_cap_${i} = private unnamed_addr constant [${capLen} x i8] c"${capEsc.escaped}\\00"`)
    }
    // Array of pointers to cap strings, null-terminated
    const capPtrs = caps.map((_c, i) => {
      const capEsc = this.escapeStringForLLVM(caps[i])
      const capLen = capEsc.byteLength + 1
      return `ptr @.plugin_cap_${i}`
    })
    capPtrs.push("ptr null") // null terminator
    ir.push(`@.plugin_required_caps = private unnamed_addr constant [${capPtrs.length} x ptr] [${capPtrs.join(", ")}]`)

    // Export names: null-terminated array
    const exports = this.pubFunctions
    for (let i = 0; i < exports.length; i++) {
      const expNameEsc = this.escapeStringForLLVM(exports[i].name)
      const expNameLen = expNameEsc.byteLength + 1
      ir.push(`@.plugin_export_name_${i} = private unnamed_addr constant [${expNameLen} x i8] c"${expNameEsc.escaped}\\00"`)
    }
    const expNamePtrs = exports.map((_e, i) => `ptr @.plugin_export_name_${i}`)
    expNamePtrs.push("ptr null") // null terminator
    ir.push(`@.plugin_export_names = private unnamed_addr constant [${expNamePtrs.length} x ptr] [${expNamePtrs.join(", ")}]`)

    // MogPluginInfo struct: { ptr name, ptr version, ptr required_caps, i64 num_exports, ptr export_names }
    ir.push(`@.plugin_info = private unnamed_addr constant { ptr, ptr, ptr, i64, ptr } {`)
    ir.push(`  ptr @.plugin_name,`)
    ir.push(`  ptr @.plugin_version,`)
    ir.push(`  ptr @.plugin_required_caps,`)
    ir.push(`  i64 ${exports.length},`)
    ir.push(`  ptr @.plugin_export_names`)
    ir.push(`}`)

    // The @mog_plugin_info function
    ir.push("")
    ir.push("; Plugin info query function")
    ir.push("define ptr @mog_plugin_info() {")
    ir.push("entry:")
    ir.push("  ret ptr @.plugin_info")
    ir.push("}")
  }

  private generatePluginInit(ir: string[]): void {
    ir.push("")
    ir.push("; Plugin initialization function")
    ir.push("define i32 @mog_plugin_init(ptr %vm) {")
    ir.push("entry:")
    ir.push("  call void @gc_init()")
    ir.push("  call void @gc_push_frame()")
    ir.push("  call void @mog_vm_set_global(ptr %vm)")

    // If async functions exist, create event loop
    if (this.hasAsyncFunctions) {
      ir.push("")
      ir.push("  ; Set up async event loop")
      ir.push("  %loop = call ptr @mog_loop_new()")
      ir.push("  call void @mog_loop_set_global(ptr %loop)")
    }

    ir.push("")
    ir.push("  ; Run top-level statements")
    ir.push("  call void @program()")

    ir.push("")
    ir.push("  call void @gc_pop_frame()")
    ir.push("  ret i32 0")
    ir.push("}")
  }

  private generatePluginExports(ir: string[]): void {
    const exports = this.pubFunctions

    ir.push("")
    ir.push("; Plugin export table: array of { ptr name, ptr func_ptr } pairs")

    // Emit name constants for each export (reuse the ones from plugin_info)
    // Emit the export table as a global constant array of { ptr, ptr } structs
    if (exports.length > 0) {
      const entries = exports.map((_exp, i) => {
        const exportedName = `mogp_${exports[i].name}`
        return `{ ptr, ptr } { ptr @.plugin_export_name_${i}, ptr @${exportedName} }`
      })
      ir.push(`@.plugin_export_table = private unnamed_addr constant [${exports.length} x { ptr, ptr }] [${entries.join(", ")}]`)
    } else {
      ir.push(`@.plugin_export_table = private unnamed_addr constant [0 x { ptr, ptr }] zeroinitializer`)
    }

    // The @mog_plugin_exports function
    ir.push("")
    ir.push("; Plugin exports query function")
    ir.push("define ptr @mog_plugin_exports(ptr %count_out) {")
    ir.push("entry:")
    ir.push(`  store i64 ${exports.length}, ptr %count_out`)
    ir.push("  ret ptr @.plugin_export_table")
    ir.push("}")

    // Generate mogp_ wrapper aliases for pub functions
    // These are thin wrappers that forward to the actual function, ensuring
    // a stable exported symbol name regardless of internal mangling
    ir.push("")
    ir.push("; Plugin export wrappers (mogp_ prefix)")
    for (const exp of exports) {
      const wrapperName = `mogp_${exp.name}`
      const llvmRetType = this.toLLVMType(exp.returnType)
      const llvmParams = (exp.params || []).map((p: any) => {
        const t = this.toLLVMType(p.paramType)
        return `${t} %${p.name}`
      })
      const paramStr = llvmParams.join(", ")
      const callArgs = (exp.params || []).map((p: any) => {
        const t = this.toLLVMType(p.paramType)
        return `${t} %${p.name}`
      }).join(", ")

      ir.push(`define ${llvmRetType} @${wrapperName}(${paramStr}) {`)
      ir.push("entry:")
      if (llvmRetType === "void") {
        ir.push(`  call void @${exp.llvmName}(${callArgs})`)
        ir.push("  ret void")
      } else {
        ir.push(`  %result = call ${llvmRetType} @${exp.llvmName}(${callArgs})`)
        ir.push(`  ret ${llvmRetType} %result`)
      }
      ir.push("}")
    }
  }

  private setupMainFunction(ir: string[]): void {
    ir.push("")
    ir.push("; Main entry point")
    ir.push("define i32 @main() {")
    ir.push("entry:")
    ir.push("  call void @gc_init()")
    ir.push("  call void @gc_push_frame()")
    ir.push("  call void @program()")
    ir.push("  call void @gc_pop_frame()")
    ir.push("  ret i32 0")
    ir.push("}")
  }

  private generateRuntimeFunctions(ir: string[]): void {
    ir.push("")
    ir.push("; Runtime function definitions (linked from runtime library)")
  }

  private generateMathDeclarations(ir: string[]): void {
    ir.push("")
    ir.push("; Math function declarations (libm)")
    // Single-arg: double -> double
    ir.push("declare double @llvm.sqrt.f64(double)")
    ir.push("declare double @llvm.sin.f64(double)")
    ir.push("declare double @llvm.cos.f64(double)")
    ir.push("declare double @tan(double)")
    ir.push("declare double @asin(double)")
    ir.push("declare double @acos(double)")
    ir.push("declare double @atan2(double, double)")
    ir.push("declare double @llvm.exp.f64(double)")
    ir.push("declare double @llvm.log.f64(double)")
    ir.push("declare double @llvm.log2.f64(double)")
    ir.push("declare double @llvm.floor.f64(double)")
    ir.push("declare double @llvm.ceil.f64(double)")
    ir.push("declare double @llvm.round.f64(double)")
    ir.push("declare double @llvm.fabs.f64(double)")
    // Two-arg: (double, double) -> double
    ir.push("declare double @llvm.pow.f64(double, double)")
    ir.push("declare double @llvm.minnum.f64(double, double)")
    ir.push("declare double @llvm.maxnum.f64(double, double)")
    ir.push("")
  }

  private generatePOSIXDeclarations(ir: string[]): void {
    ir.push("")
    ir.push("; POSIX function declarations")

    ir.push("declare i64 @open(ptr, i64, ...)")
    ir.push("declare i64 @read(i64, ptr, i64)")
    ir.push("declare i64 @write(i64, ptr, i64)")
    ir.push("declare i64 @close(i64)")
    ir.push("declare i64 @access(ptr, i64)")
    ir.push("declare i64 @faccessat(i64, ptr, i64, i64)")
    ir.push("declare i64 @chmod(ptr, i64)")
    ir.push("declare i64 @fchmod(i64, i64)")
    ir.push("declare i64 @chown(ptr, i64, i64)")
    ir.push("declare i64 @fchown(i64, i64, i64)")
    ir.push("declare i64 @lseek(i64, i64, i64)")
    ir.push("declare i64 @fsync(i64)")
    ir.push("declare i64 @fdatasync(i64)")
    ir.push("declare i64 @ftruncate(i64, i64)")
    ir.push("declare i64 @stat(ptr, ptr)")
    ir.push("declare i64 @lstat(ptr, ptr)")
    ir.push("declare i64 @fstat(i64, ptr)")
    ir.push("declare i64 @link(ptr, ptr)")
    ir.push("declare i64 @symlink(ptr, ptr)")
    ir.push("declare i64 @readlink(ptr, ptr, i64)")
    ir.push("declare i64 @unlink(ptr)")
    ir.push("declare i64 @rename(ptr, ptr)")
    ir.push("declare ptr @opendir(ptr)")
    ir.push("declare ptr @readdir(ptr)")
    ir.push("declare void @rewinddir(ptr)")
    ir.push("declare void @closedir(ptr)")
    ir.push("declare i64 @mkdir(ptr, i64)")
    ir.push("declare i64 @rmdir(ptr)")
    ir.push("declare i64 @chdir(ptr)")
    ir.push("declare i64 @fchdir(i64)")
    ir.push("declare i64 @getcwd(ptr, i64)")
    ir.push("declare i64 @dup(i64)")
    ir.push("declare i64 @dup2(i64, i64)")
    ir.push("declare i64 @fcntl(i64, i64, ...)")
    ir.push("declare i64 @pathconf(ptr, i64)")
    ir.push("declare i64 @fpathconf(i64, i64)")
    ir.push("declare i64 @creat(ptr, i64)")
    ir.push("declare i64 @mkfifo(ptr, i64)")
    ir.push("declare i64 @mknod(ptr, i64, i64)")
    ir.push("declare i64 @utimes(ptr, ptr)")
    ir.push("declare i64 @futimes(i64, ptr)")
    ir.push("declare i64 @utimensat(i64, ptr, ptr, i64)")
    ir.push("declare i64 @pread(i64, ptr, i64, i64)")
    ir.push("declare i64 @pwrite(i64, ptr, i64, i64)")
    ir.push("declare i64 @truncate(ptr, i64)")
    ir.push("")
  }

  private generatePrintDeclarations(ir: string[]): void {
    ir.push("")
    ir.push("; Print function declarations")
    ir.push("declare void @print_i64(i64)")
    ir.push("declare void @print_u64(i64)")
    ir.push("declare void @print_f64(double)")
    ir.push("declare void @print_string(ptr)")
    ir.push("declare void @print_buffer(ptr, i64)")
    ir.push("declare void @println()")
    ir.push("declare void @println_i64(i64)")
    ir.push("declare void @println_u64(i64)")
    ir.push("declare void @println_f64(double)")
    ir.push("declare void @println_string(ptr)")
    ir.push("")
    ir.push("; Input function declarations")
    ir.push("declare i64 @input_i64()")
    ir.push("declare i64 @input_u64()")
    ir.push("declare double @input_f64()")
    ir.push("declare ptr @input_string()")
    ir.push("")
  }

  private nextLabel(): string {
    return `label${this.labelCounter++}`
  }

  private toLLVMType(type: any): LLVMType {
    if (!type) return "i64"

    switch (type.type) {
      case "IntegerType":
        return type.kind as LLVMType
      case "UnsignedType": {
        // LLVM integers are signless — u8 maps to i8, u16 to i16, etc.
        const bits = type.kind.slice(1)  // "u64" -> "64"
        return `i${bits}` as LLVMType
      }
      case "FloatType":
        // LLVM float type mapping: f8/f16->half, f32->float, f64->double, f128->fp128
        // f256 is not natively supported by LLVM - we treat it as fp128 (or could use software emulation)
        switch (type.kind) {
          case "f8":
          case "f16":
            return "half"
          case "f32":
            return "float"
          case "f64":
            return "double"
          case "f128":
          case "f256":
            return "fp128"
          default:
            return "float"
        }
      case "ArrayType":
        return "ptr"
      case "MapType":
        return "ptr"
      case "StructType":
        // Structs are heap-allocated, represented as pointers
        return "ptr"
      case "AOSType":
        // AoS is an array of structs, represented as pointer
        return "ptr"
      case "SOAType":
        // SoA is heap-allocated, represented as pointer
        return "ptr"
      case "CustomType":
        // Custom types (structs) are heap-allocated, represented as pointers
        return "ptr"
      case "PointerType":
        return "ptr"
      case "StringType":
        return "ptr"
      case "FunctionType":
        // Function types are function pointers
        return "ptr"
      case "ResultType":
        // Result types are represented as structs (tag + value)
        return "i64"
      case "OptionalType":
        // Optional types are represented as structs (tag + value)
        return "i64"
      case "FutureType":
        // TODO: Future types use synchronous fallback for now - represented as their inner type
        return this.toLLVMType(type.innerType)
      case "TensorType":
        // Tensors are heap-allocated GC objects, represented as pointers
        return "ptr"
      case "VoidType":
        return "void"
      case "TypeAliasType":
        return this.toLLVMType(type.aliasedType)
 default:
        return "i64"
    }
  }

private getIntBits(type: LLVMType): number {
    const match = type.match(/i(\d+)/)
    return match ? parseInt(match[1]) : 64
  }

  private getFloatBits(type: LLVMType): number {
    switch (type) {
      case "half": return 16
      case "float": return 32
      case "double": return 64
      case "fp128": return 128
      default: return 32
    }
  }

  private isFloatType(llvmType: LLVMType): boolean {
    return llvmType === "half" || llvmType === "float" || llvmType === "double" || llvmType === "fp128"
  }

  /**
   * Convert a value to a double register for math builtins.
   * Handles: f64 variable loads (already double), hex float constants (already double),
   * i64 registers (need bitcast), and integer constants.
   */
  private toDoubleReg(ir: string[], reg: string, expr: any): string {
    // Hex constants like 0x400921... are LLVM double literals — use directly
    if (reg.startsWith("0x")) {
      return reg
    }
    // If the expression is a float operand AND produced a named register,
    // it was loaded as `double` — use directly
    if (this.isFloatOperand(expr) && reg.startsWith("%")) {
      return reg
    }
    // Otherwise it's an i64 register or integer constant — bitcast to double
    const doubleReg = `%${this.valueCounter++}`
    ir.push(`  ${doubleReg} = bitcast i64 ${reg} to double`)
    return doubleReg
  }

  private isFloatOperand(expr: any): boolean {
    if (!expr) return false

    // Check literal type annotation
    if (expr.literalType?.type === "FloatType") return true

    // Check if it's a float literal value
    if (typeof expr.value === "number" && !Number.isInteger(expr.value)) return true
    if (typeof expr.value === "string") {
      const val = expr.value.toLowerCase()
      if (val.includes(".") || val.includes("e")) return true
    }

    // Check if it's an identifier with float type
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      return varType?.type === "FloatType"
    }

    // Check if it's a MemberExpression on a struct with float field
    if (expr.type === "MemberExpression" && expr.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.name)
      if (varType?.type === "StructType" || varType?.type === "SOAType" || varType?.type === "CustomType") {
        const fieldName = expr.property?.name || expr.property
        // For SOAType, check the backing struct's fields directly
        if (varType?.type === "SOAType") {
          const soaType = varType as SOAType
          const fieldType = soaType.structType.fields.get(fieldName)
          if (fieldType?.type === "FloatType") return true
        }
        const structInfo = this.structDefs.get(expr.object.name) || this.structDefs.get(varType.name)
        if (structInfo) {
          const field = structInfo.find((f: any) => f.name === fieldName)
          if (field?.fieldType?.type === "FloatType") return true
        }
      }
    }

    // Check if it's a SoA transposed access: datums[i].field
    if (expr.type === "MemberExpression" && expr.object?.type === "IndexExpression" && expr.object?.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.object.name)
      if (varType?.type === "SOAType") {
        const soaType = varType as SOAType
        const fieldName = expr.property?.name || expr.property
        const fieldType = soaType.structType.fields.get(fieldName)
        if (fieldType?.type === "FloatType") return true
      }
    }

    // Check if it's an IndexExpression on a float array (e.g., values[i] where values: f64[])
    if (expr.type === "IndexExpression" && expr.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.name)
      if (varType?.type === "ArrayType" && varType.elementType?.type === "FloatType") return true
    }

    // For unary expressions, check the operand recursively
    if (expr.type === "UnaryExpression") {
      const arg = expr.argument || expr.operand
      return this.isFloatOperand(arg)
    }

    // For binary expressions, check left operand recursively
    if (expr.type === "BinaryExpression") {
      return this.isFloatOperand(expr.left) || this.isFloatOperand(expr.right)
    }

    // Check if it's a call to a function that returns a float type
    if (expr.type === "CallExpression") {
      const callee = expr.callee?.name || expr.function?.name
      const mathBuiltins = new Set([
        "sqrt", "sin", "cos", "tan", "asin", "acos",
        "exp", "log", "log2", "floor", "ceil", "round", "abs",
        "pow", "atan2", "min", "max",
      ])
      if (callee && mathBuiltins.has(callee)) return true
      // Check user-defined functions that return float types
      if (callee) {
        const func = this.functions.get(callee)
        if (func && (func.returnType === "double" || func.returnType === "float" || func.returnType === "half" || func.returnType === "fp128")) {
          return true
        }
      }
    }

    return false
  }

  private getFloatTypeSize(expr: any): "half" | "float" | "double" | "fp128" {
    if (!expr) return "float"

    const floatKindToLLVM = (kind: string): "half" | "float" | "double" | "fp128" => {
      switch (kind) {
        case "f8":
        case "f16":
          return "half"
        case "f32":
          return "float"
        case "f64":
          return "double"
        case "f128":
        case "f256":
          return "fp128"
        default:
          return "float"
      }
    }

    // Check literal type annotation
    if (expr.literalType?.type === "FloatType") {
      return floatKindToLLVM(expr.literalType.kind)
    }

    // Check if it's an identifier with float type
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      if (varType?.type === "FloatType") {
        return floatKindToLLVM(varType.kind)
      }
    }

    // Check if it's an IndexExpression on a float array (e.g., values[i] where values: f64[])
    if (expr.type === "IndexExpression" && expr.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.name)
      if (varType?.type === "ArrayType" && varType.elementType?.type === "FloatType") {
        return floatKindToLLVM(varType.elementType.kind)
      }
    }

    // For binary expressions, check operands recursively and use widest type
    if (expr.type === "BinaryExpression") {
      const leftSize = this.getFloatTypeSize(expr.left)
      const rightSize = this.getFloatTypeSize(expr.right)
      // Priority: fp128 > double > float > half
      if (leftSize === "fp128" || rightSize === "fp128") return "fp128"
      if (leftSize === "double" || rightSize === "double") return "double"
      if (leftSize === "float" || rightSize === "float") return "float"
      return "half"
    }

    // For function calls, check the function's return type
    if (expr.type === "CallExpression") {
      const funcName = expr.function?.name || expr.callee?.name
      if (funcName) {
        const func = this.functions.get(funcName)
        if (func) {
          switch (func.returnType) {
            case "half": return "half"
            case "float": return "float"
            case "double": return "double"
            case "fp128": return "fp128"
          }
        }
      }
    }

    // For MemberExpression on structs, check the field type
    if (expr.type === "MemberExpression" && expr.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.name)
      if (varType?.type === "StructType" || varType?.type === "SOAType" || varType?.type === "CustomType") {
        const fieldName = expr.property?.name || expr.property
        const structInfo = this.structDefs.get(expr.object.name) || this.structDefs.get(varType.name)
        if (structInfo) {
          const field = structInfo.find((f: any) => f.name === fieldName)
          if (field?.fieldType?.type === "FloatType") {
            return floatKindToLLVM(field.fieldType.kind)
          }
        }
      }
    }

    // SoA transposed access: particles[i].field where particles is SOAType
    if (expr.type === "MemberExpression" && expr.object?.type === "IndexExpression" && expr.object?.object?.type === "Identifier") {
      const varType = this.variableTypes.get(expr.object.object.name)
      if (varType?.type === "SOAType") {
        const soaType = varType as SOAType
        const fieldName = expr.property?.name || expr.property
        const fieldType = soaType.structType?.fields?.get(fieldName)
        if (fieldType?.type === "FloatType") {
          return floatKindToLLVM((fieldType as any).kind)
        }
        // Also check via structDefs
        const structInfo = this.structDefs.get(expr.object.object.name) || this.structDefs.get(varType.name)
        if (structInfo) {
          const field = structInfo.find((f: any) => f.name === fieldName)
          if (field?.fieldType?.type === "FloatType") {
            return floatKindToLLVM(field.fieldType.kind)
          }
        }
      }
    }

    return "float" // default to float
  }

  private floatToHex(value: number, kind: string): string {
    // Convert float value to IEEE 754 hex representation based on type
    switch (kind) {
      case "f8":
      case "f16": {
        // Half precision (16-bit)
        const buffer = new ArrayBuffer(2)
        // Use Float16Array if available, otherwise convert via Float32
        if (typeof Float16Array !== 'undefined') {
          new Float16Array(buffer)[0] = value
        } else {
          // Convert float32 to half precision manually
          const f32Buffer = new ArrayBuffer(4)
          new DataView(f32Buffer).setFloat32(0, value, false)
          const f32Bits = new DataView(f32Buffer).getUint32(0, false)
          // Extract sign, exponent, mantissa
          const sign = (f32Bits >> 31) & 0x1
          const exponent = (f32Bits >> 23) & 0xFF
          const mantissa = f32Bits & 0x7FFFFF
          // Convert to half precision
          let hSign = sign
          let hExponent: number
          let hMantissa: number
          if (exponent === 0) {
            hExponent = 0
            hMantissa = 0
          } else if (exponent === 0xFF) {
            hExponent = 0x1F
            hMantissa = mantissa ? 0x200 : 0
          } else {
            const newExp = exponent - 127 + 15
            if (newExp >= 31) {
              hExponent = 0x1F
              hMantissa = 0
            } else if (newExp <= 0) {
              hExponent = 0
              hMantissa = (mantissa | 0x800000) >> (1 - newExp)
            } else {
              hExponent = newExp
              hMantissa = mantissa >> 13
            }
          }
          const hBits = (hSign << 15) | (hExponent << 10) | hMantissa
          return `0x${hBits.toString(16).padStart(4, '0')}`
        }
        const bits = new DataView(buffer).getUint16(0, false)
        return `0x${bits.toString(16).padStart(4, '0')}`
      }
      case "f32": {
        // Single precision (32-bit)
        const buffer = new ArrayBuffer(4)
        new DataView(buffer).setFloat32(0, value, false)
        const bits = new DataView(buffer).getUint32(0, false)
        return `0x${bits.toString(16).padStart(8, '0')}`
      }
      case "f64":
      default: {
        // Double precision (64-bit)
        const buffer = new ArrayBuffer(8)
        new DataView(buffer).setFloat64(0, value, false)
        const bits = new DataView(buffer).getBigUint64(0, false)
        return `0x${bits.toString(16).padStart(16, '0')}`
      }
      case "f128":
      case "f256": {
        // Quad precision (128-bit) - use fp128 format
        // For now, store as double and zero-pad the high bits
        const buffer = new ArrayBuffer(8)
        new DataView(buffer).setFloat64(0, value, false)
        const lowBits = new DataView(buffer).getBigUint64(0, false)
        return `0x0000000000000000${lowBits.toString(16).padStart(16, '0')}`
      }
    }
  }

  private floatToIntBits(value: number, kind: string): string {
    // Convert float value to LLVM hex float format (e.g., 0x40091eb851eb851f for 3.14).
    // LLVM interprets 0x... as IEEE 754 hex bits for float types (double, float, etc.)
    // and also accepts them for i64 in most contexts.
    switch (kind) {
      case "f32": {
        const buffer = new ArrayBuffer(4)
        new DataView(buffer).setFloat32(0, value, false)
        const bits = new DataView(buffer).getUint32(0, false)
        return `0x${bits.toString(16).padStart(8, '0')}`
      }
      case "f64":
      default: {
        const buffer = new ArrayBuffer(8)
        new DataView(buffer).setFloat64(0, value, false)
        const bits = new DataView(buffer).getBigUint64(0, false)
        return `0x${bits.toString(16).padStart(16, '0')}`
      }
    }
  }

  private isPointerArithmetic(expr: any, leftReg: string, rightReg: string): boolean {
    // Check if this is pointer arithmetic: ptr + offset or ptr - offset
    // This is a heuristic - if left looks like a pointer register (contains "ptr" in the name)
    // and the operator is + or -, treat it as pointer arithmetic
    return (expr.operator === "+" || expr.operator === "-") &&
           (leftReg.startsWith("%ptr") || this.isPointerExpression(expr.left))
  }

  private isPointerExpression(expr: any): boolean {
    if (!expr) return false
    if (expr.type === "Identifier" && expr.name) {
      const varType = this.variableTypes.get(expr.name)
      return varType?.type === "PointerType" || varType?.type === "Pointer"
    }
    return false
  }

  private generatePointerArithmetic(ir: string[], expr: any, leftReg: string, rightReg: string): string {
    const resultReg = `%${this.valueCounter++}`
    // LLVM uses getelementptr for pointer arithmetic
    // gep i8, ptr %left, i64 %right gives ptr = left + right bytes
    if (expr.operator === "+") {
      ir.push(`  ${resultReg} = getelementptr i8, ptr ${leftReg}, i64 ${rightReg}`)
    } else { // operator === "-"
      // Negate the offset for subtraction
      const negReg = `%${this.valueCounter++}`
      ir.push(`  ${negReg} = sub i64 0, ${rightReg}`)
      ir.push(`  ${resultReg} = getelementptr i8, ptr ${leftReg}, i64 ${negReg}`)
    }
    return resultReg
  }

  // ============================================================================
  // Result/Optional Error Handling Code Generation
  // ============================================================================

  private generateOkExpression(ir: string[], expr: any): string {
    let value = this.generateExpression(ir, expr.value)
    // If the inner value is a pointer type, convert to i64 first
    if (this.isPtrProducingExpression(expr.value)) {
      const asInt = `%${this.valueCounter++}`
      ir.push(`  ${asInt} = ptrtoint ptr ${value} to i64`)
      value = asInt
    }
    const resultPtr = `%${this.valueCounter++}`
    ir.push(`  ${resultPtr} = call ptr @mog_result_ok(i64 ${value})`)
    const resultInt = `%${this.valueCounter++}`
    ir.push(`  ${resultInt} = ptrtoint ptr ${resultPtr} to i64`)
    return resultInt
  }

  private generateErrExpression(ir: string[], expr: any): string {
    const value = this.generateExpression(ir, expr.value)
    // The value may be a ptr (string literal GEP) or i64 (variable)
    let valuePtr: string
    if (expr.value.type === "StringLiteral" || expr.value.type === "TemplateLiteral") {
      valuePtr = value // already a ptr from GEP
    } else {
      valuePtr = `%${this.valueCounter++}`
      ir.push(`  ${valuePtr} = inttoptr i64 ${value} to ptr`)
    }
    const resultPtr = `%${this.valueCounter++}`
    ir.push(`  ${resultPtr} = call ptr @mog_result_err(ptr ${valuePtr})`)
    const resultInt = `%${this.valueCounter++}`
    ir.push(`  ${resultInt} = ptrtoint ptr ${resultPtr} to i64`)
    return resultInt
  }

  private generateSomeExpression(ir: string[], expr: any): string {
    const value = this.generateExpression(ir, expr.value)
    const optPtr = `%${this.valueCounter++}`
    ir.push(`  ${optPtr} = call ptr @mog_optional_some(i64 ${value})`)
    const optInt = `%${this.valueCounter++}`
    ir.push(`  ${optInt} = ptrtoint ptr ${optPtr} to i64`)
    return optInt
  }

  private generateNoneExpression(ir: string[]): string {
    const optPtr = `%${this.valueCounter++}`
    ir.push(`  ${optPtr} = call ptr @mog_optional_none()`)
    const optInt = `%${this.valueCounter++}`
    ir.push(`  ${optInt} = ptrtoint ptr ${optPtr} to i64`)
    return optInt
  }

  private generatePropagateExpression(ir: string[], expr: any): string {
    const value = this.generateExpression(ir, expr.value)
    const valuePtr = `%${this.valueCounter++}`
    ir.push(`  ${valuePtr} = inttoptr i64 ${value} to ptr`)

    // Determine if this is Result or Optional based on type annotation
    const resultType = (expr.value as any).resultType
    const isOptional = resultType instanceof OptionalType

    const isOkLabel = this.nextLabel()
    const propagateLabel = this.nextLabel()

    const tryCtx = this.tryStack.length > 0 ? this.tryStack[this.tryStack.length - 1] : null

    if (isOptional) {
      const isSome = `%${this.valueCounter++}`
      ir.push(`  ${isSome} = call i64 @mog_optional_is_some(ptr ${valuePtr})`)
      const cmp = `%${this.valueCounter++}`
      ir.push(`  ${cmp} = icmp ne i64 ${isSome}, 0`)
      ir.push(`  br i1 ${cmp}, label %${isOkLabel}, label %${propagateLabel}`)

      ir.push(`${propagateLabel}:`)
      if (tryCtx) {
        // Inside try block: create a "none" error string and branch to catch
        let noneStrName = this.stringNameMap.get("none")
        if (!noneStrName) {
          noneStrName = `@str${this.stringCounter++}`
          this.stringNameMap.set("none", noneStrName)
          this.stringByteLengths.set("none", 4)
          this.stringConstants.push(`${noneStrName} = private unnamed_addr constant [5 x i8] c"none\\00"`)
        }
        const noneStr = `%${this.valueCounter++}`
        ir.push(`  ${noneStr} = call ptr @mog_string_new(ptr ${noneStrName}, i64 4)`)
        const noneInt = `%${this.valueCounter++}`
        ir.push(`  ${noneInt} = ptrtoint ptr ${noneStr} to i64`)
        ir.push(`  store i64 ${noneInt}, ptr ${tryCtx.errorVar}`)
        ir.push(`  br label %${tryCtx.catchLabel}`)
      } else {
        // Return the none value from the current function
        ir.push(`  call void @gc_pop_frame()`)
        ir.push(`  ret i64 ${value}`)
      }

      ir.push(`${isOkLabel}:`)
      const unwrapped = `%${this.valueCounter++}`
      ir.push(`  ${unwrapped} = call i64 @mog_optional_unwrap(ptr ${valuePtr})`)
      return unwrapped
    } else {
      // Assume Result type
      const isOk = `%${this.valueCounter++}`
      ir.push(`  ${isOk} = call i64 @mog_result_is_ok(ptr ${valuePtr})`)
      const cmp = `%${this.valueCounter++}`
      ir.push(`  ${cmp} = icmp ne i64 ${isOk}, 0`)
      ir.push(`  br i1 ${cmp}, label %${isOkLabel}, label %${propagateLabel}`)

      ir.push(`${propagateLabel}:`)
      if (tryCtx) {
        // Inside try block: extract error string from Result and branch to catch
        const errPtr = `%${this.valueCounter++}`
        ir.push(`  ${errPtr} = call ptr @mog_result_unwrap_err(ptr ${valuePtr})`)
        const errInt = `%${this.valueCounter++}`
        ir.push(`  ${errInt} = ptrtoint ptr ${errPtr} to i64`)
        ir.push(`  store i64 ${errInt}, ptr ${tryCtx.errorVar}`)
        ir.push(`  br label %${tryCtx.catchLabel}`)
      } else {
        // Return the error Result from the current function
        ir.push(`  call void @gc_pop_frame()`)
        ir.push(`  ret i64 ${value}`)
      }

      ir.push(`${isOkLabel}:`)
      const unwrapped = `%${this.valueCounter++}`
      ir.push(`  ${unwrapped} = call i64 @mog_result_unwrap(ptr ${valuePtr})`)
      return unwrapped
    }
  }

  private generateIsSomeExpression(ir: string[], expr: any): string {
    const value = this.generateExpression(ir, expr.value)
    const valuePtr = `%${this.valueCounter++}`
    ir.push(`  ${valuePtr} = inttoptr i64 ${value} to ptr`)
    const isSome = `%${this.valueCounter++}`
    ir.push(`  ${isSome} = call i64 @mog_optional_is_some(ptr ${valuePtr})`)

    // If there's a binding, unwrap the value and store it
    if (expr.binding) {
      const bindingReg = `%${expr.binding}`
      ir.push(`  ${bindingReg} = alloca i64`)
      const unwrapped = `%${this.valueCounter++}`
      ir.push(`  ${unwrapped} = call i64 @mog_optional_unwrap(ptr ${valuePtr})`)
      ir.push(`  store i64 ${unwrapped}, ptr ${bindingReg}`)
      this.variableTypes.set(expr.binding, new IntegerType("i64"))
    }
    return isSome
  }

  private generateIsNoneExpression(ir: string[], expr: any): string {
    const value = this.generateExpression(ir, expr.value)
    const valuePtr = `%${this.valueCounter++}`
    ir.push(`  ${valuePtr} = inttoptr i64 ${value} to ptr`)
    const isSome = `%${this.valueCounter++}`
    ir.push(`  ${isSome} = call i64 @mog_optional_is_some(ptr ${valuePtr})`)
    const isNone = `%${this.valueCounter++}`
    ir.push(`  ${isNone} = xor i64 ${isSome}, 1`)
    return isNone
  }

  private generateIsOkExpression(ir: string[], expr: any): string {
    const value = this.generateExpression(ir, expr.value)
    const valuePtr = `%${this.valueCounter++}`
    ir.push(`  ${valuePtr} = inttoptr i64 ${value} to ptr`)
    const isOk = `%${this.valueCounter++}`
    ir.push(`  ${isOk} = call i64 @mog_result_is_ok(ptr ${valuePtr})`)

    if (expr.binding) {
      const bindingReg = `%${expr.binding}`
      ir.push(`  ${bindingReg} = alloca i64`)
      const unwrapped = `%${this.valueCounter++}`
      ir.push(`  ${unwrapped} = call i64 @mog_result_unwrap(ptr ${valuePtr})`)
      ir.push(`  store i64 ${unwrapped}, ptr ${bindingReg}`)
      this.variableTypes.set(expr.binding, new IntegerType("i64"))
    }
    return isOk
  }

  private generateIsErrExpression(ir: string[], expr: any): string {
    const value = this.generateExpression(ir, expr.value)
    const valuePtr = `%${this.valueCounter++}`
    ir.push(`  ${valuePtr} = inttoptr i64 ${value} to ptr`)
    const isOk = `%${this.valueCounter++}`
    ir.push(`  ${isOk} = call i64 @mog_result_is_ok(ptr ${valuePtr})`)
    const isErr = `%${this.valueCounter++}`
    ir.push(`  ${isErr} = xor i64 ${isOk}, 1`)

    if (expr.binding) {
      const bindingReg = `%${expr.binding}`
      ir.push(`  ${bindingReg} = alloca i64`)
      const errMsg = `%${this.valueCounter++}`
      ir.push(`  ${errMsg} = call ptr @mog_result_unwrap_err(ptr ${valuePtr})`)
      const errInt = `%${this.valueCounter++}`
      ir.push(`  ${errInt} = ptrtoint ptr ${errMsg} to i64`)
      ir.push(`  store i64 ${errInt}, ptr ${bindingReg}`)
    }
    return isErr
  }

  private generateTryCatch(ir: string[], statement: any): void {
    // TryCatch implementation:
    // The try body runs. If a ? propagation occurs, it stores the error
    // and jumps to the catch block.
    // We use a global-like error flag to track if we're in an error state.

    // For simplicity, we implement try/catch as:
    // 1. Allocate an error result variable
    // 2. Run try body statements
    // 3. Jump to end
    // 4. Catch block: the error variable gets bound
    // Note: With the ? operator, the error propagation generates a ret instruction,
    // so we need a different approach for try/catch.

    // Actually, the way to implement try/catch with ? is:
    // Inside a try block, instead of returning from the function on error,
    // we jump to the catch block.

    // Simple implementation: generate try body as a block, catch as a block.
    // The ? operator inside try should be handled at a higher level.
    // For now, let's emit the try body normally and the catch body as a fallback.

    // Generate a flag for error tracking
    const errorFlag = `%try_err_flag_${this.valueCounter++}`
    const errorValue = `%try_err_val_${this.valueCounter++}`
    ir.push(`  ${errorFlag} = alloca i64`)
    ir.push(`  store i64 0, ptr ${errorFlag}`)
    ir.push(`  ${errorValue} = alloca i64`)
    ir.push(`  store i64 0, ptr ${errorValue}`)

    const tryEndLabel = this.nextLabel()
    const catchLabel = this.nextLabel()
    const endLabel = this.nextLabel()

    // Push try context so ? operator branches to catch instead of returning
    this.tryStack.push({ catchLabel, errorVar: errorValue })

    // Generate try body
    for (const stmt of statement.tryBody.statements) {
      this.generateStatement(ir, stmt)
      if (this.blockTerminated) {
        this.blockTerminated = false
        break
      }
    }

    this.tryStack.pop()
    ir.push(`  br label %${tryEndLabel}`)

    // Try completed successfully - skip catch
    ir.push(`${tryEndLabel}:`)
    ir.push(`  br label %${endLabel}`)

    // Catch block
    ir.push(`${catchLabel}:`)
    // Bind error variable
    const errVarReg = `%${statement.errorVar}`
    ir.push(`  ${errVarReg} = alloca i64`)
    const errLoad = `%${this.valueCounter++}`
    ir.push(`  ${errLoad} = load i64, ptr ${errorValue}`)
    ir.push(`  store i64 ${errLoad}, ptr ${errVarReg}`)
    this.variableTypes.set(statement.errorVar, new IntegerType("i64"))

    for (const stmt of statement.catchBody.statements) {
      this.generateStatement(ir, stmt)
      if (this.blockTerminated) break
    }
    if (!this.blockTerminated) {
      ir.push(`  br label %${endLabel}`)
    }
    this.blockTerminated = false

    ir.push(`${endLabel}:`)
  }

  private generateWithBlock(ir: string[], statement: any): void {
    // For `with no_grad() { ... }`:
    // 1. Evaluate the context expression (e.g., call no_grad())
    // 2. Call mog_no_grad_begin() to set the flag
    // 3. Generate body
    // 4. Call mog_no_grad_end() to clear the flag

    // Check if context is a no_grad() call
    const ctx = statement.context
    const isNoGrad =
      ctx.type === "CallExpression" &&
      ((ctx.callee?.name === "no_grad") ||
       (ctx.callee?.type === "Identifier" && ctx.callee?.name === "no_grad"))

    if (isNoGrad) {
      ir.push("  call void @mog_no_grad_begin()")
    } else {
      // Generic with block: evaluate the context expression
      this.generateExpression(ir, ctx)
    }

    // Generate body
    for (const stmt of statement.body.statements) {
      this.generateStatement(ir, stmt)
      if (this.blockTerminated) break
    }

    if (isNoGrad) {
      if (!this.blockTerminated) {
        ir.push("  call void @mog_no_grad_end()")
      }
    }
    this.blockTerminated = false
  }
}

export function generateLLVMIR(
  ast: ProgramNode,
  options?: Partial<OptimizationOptions>,
  capabilities?: string[],
  capabilityDecls?: Map<string, any>,
  pluginOptions?: { pluginMode: boolean; pluginName?: string; pluginVersion?: string },
): string {
  const generator = new LLVMIRGenerator(options)
  if (capabilities && capabilities.length > 0) {
    generator.setCapabilities(capabilities)
  }
  if (capabilityDecls && capabilityDecls.size > 0) {
    generator.setCapabilityDecls(capabilityDecls)
  }
  if (pluginOptions?.pluginMode) {
    generator.setPluginMode(
      pluginOptions.pluginName ?? "plugin",
      pluginOptions.pluginVersion ?? "0.1.0",
    )
  }
  return generator.generate(ast)
}

export function generateModuleLLVMIR(
  packages: { packageName: string; ast: ProgramNode; exports: Map<string, { name: string; kind: string }> }[],
  options?: Partial<OptimizationOptions>,
  capabilities?: string[],
  capabilityDecls?: Map<string, any>,
): string {
  const generator = new LLVMIRGenerator(options)
  if (capabilities && capabilities.length > 0) {
    generator.setCapabilities(capabilities)
  }
  if (capabilityDecls && capabilityDecls.size > 0) {
    generator.setCapabilityDecls(capabilityDecls)
  }

  // For multi-package compilation, we merge all package ASTs into a single AST
  // with package-prefixed function names for non-main packages
  const mergedStatements: any[] = []
  const importedPkgs = new Map<string, string>()

  for (const pkg of packages) {
    if (pkg.packageName === "main") {
      // Main package statements go through directly
      mergedStatements.push(...pkg.ast.statements)
    } else {
      // Non-main package: prefix function and struct names
      for (const stmt of pkg.ast.statements) {
        if (stmt.type === "PackageDeclaration" || stmt.type === "ImportDeclaration") {
          continue // Skip package/import declarations
        }
        if (stmt.type === "FunctionDeclaration" || stmt.type === "AsyncFunctionDeclaration") {
          const funcStmt = stmt as any
          const mangledName = `${pkg.packageName}__${funcStmt.name}`
          mergedStatements.push({ ...funcStmt, name: mangledName })
        } else if (stmt.type === "StructDeclaration" || stmt.type === "StructDefinition") {
          const structStmt = stmt as any
          const mangledName = `${pkg.packageName}__${structStmt.name}`
          mergedStatements.push({ ...structStmt, name: mangledName })
        } else {
          mergedStatements.push(stmt)
        }
      }
      importedPkgs.set(pkg.packageName, pkg.packageName)
    }
  }

  // Set imported packages on generator for resolving qualified member access
  generator.importedPackages = importedPkgs

  const mergedAst: ProgramNode = {
    type: "Program",
    statements: mergedStatements,
    scopeId: "module",
    position: packages[0]?.ast.position || {
      start: { line: 1, column: 1, index: 0 },
      end: { line: 1, column: 1, index: 0 },
    },
  }

  return generator.generate(mergedAst)
}

export { LLVMIRGenerator, type OptimizationOptions }
