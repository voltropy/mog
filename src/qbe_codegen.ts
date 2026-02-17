// QBE Backend Code Generator for Mog
// Generates QBE IL from analyzed AST, targeting the QBE compiler backend
// QBE base types: w (i32), l (i64), s (f32), d (f64). All pointers are l.

import type { ProgramNode, StatementNode, ExpressionNode } from "./analyzer.js"
import {
  isArrayType, isMapType, isFloatType, isTensorType,
  IntegerType, UnsignedType, FloatType, ArrayType, StructType, SOAType,
  ResultType, OptionalType, FunctionType, TensorType, BoolType, StringType,
  VoidType, PointerType, MapType, CustomType, TypeAliasType, FutureType,
  isStructType, isStringType, isPointerType, isBoolType, isResultType,
  isOptionalType, isFutureType, isFunctionType, isVoidType, isIntegerType,
  isUnsignedType, isSOAType
} from "./types.js"

// QBE base types
type QBEType = "w" | "l" | "s" | "d" | "void"

class QBECodeGen {
  // --- State ---
  private ir: string[] = []           // accumulated QBE IL lines
  private dataSection: string[] = []  // data definitions (strings, globals)
  private funcSection: string[] = []  // function definitions
  private currentFunc: string[] = []  // lines for the current function being generated
  private entryAllocs: string[] = []  // alloc instructions (must be in entry block)

  private regCounter = 0
  private labelCounter = 0
  private stringCounter = 0
  private stringNameMap: Map<string, string> = new Map()
  private stringByteLengths: Map<string, number> = new Map()
  private lambdaCounter = 0
  private lambdaFuncs: string[] = []  // lifted lambda function IL
  private wrapperFuncs: string[] = [] // function wrapper IL
  private functionWrappers: Set<string> = new Set()

  private variables: Map<string, string> = new Map()       // name -> stack slot register
  private variableTypes: Map<string, any> = new Map()
  private functionTypes: Map<string, any> = new Map()
  private functionParamInfo: Map<string, { name: string; paramType: any; defaultValue?: any }[]> = new Map()
  private structDefs: Map<string, { name: string; fieldType: any }[]> = new Map()

  private loopStack: { breakLabel: string; continueLabel: string }[] = []
  private blockTerminated = false

  private capabilities: Set<string> = new Set()
  private capabilityDecls: Map<string, any> = new Map()

  private isInAsyncFunction = false
  private asyncFunctions: Set<string> = new Set()
  private hasAsyncFunctions = false
  private runtimeAsyncFunctions: Set<string> = new Set(["async_read_line"])

  public packagePrefix = ""
  public importedPackages: Map<string, string> = new Map()

  // Tracks whether we're generating inside a function (vs top-level)
  private inFunction = false
  private currentFunctionName = ""
  private currentReturnType: QBEType = "l"

  // --- Configuration ---
  setCapabilities(caps: string[]): void {
    for (const cap of caps) this.capabilities.add(cap)
  }

  setCapabilityDecls(decls: Map<string, any>): void {
    this.capabilityDecls = decls
  }

  private isAsyncCapabilityFunc(capName: string, funcName: string): boolean {
    const decl = this.capabilityDecls.get(capName)
    if (!decl) return false
    const func = decl.functions?.find((f: any) => f.name === funcName)
    return func?.isAsync === true
  }

  // --- Register & Label Management ---
  private nextReg(): string {
    return `%v.${this.regCounter++}`
  }

  private nextLabel(): string {
    return `@L.${this.labelCounter++}`
  }

  private resetCounters(): void {
    this.regCounter = 0
    this.labelCounter = 0
  }

  // --- Emit Helpers ---
  private emit(line: string): void {
    this.currentFunc.push(line)
  }

  private emitData(line: string): void {
    this.dataSection.push(line)
  }

  private emitAlloc(line: string): void {
    this.entryAllocs.push(line)
  }

  // --- Type Mapping ---
  toQBEType(type: any): QBEType {
    if (!type) return "l"
    if (type instanceof IntegerType) return "l"
    if (type instanceof UnsignedType) return "l"
    if (type instanceof FloatType) {
      if (type.kind === "f32") return "s"
      return "d" // f64 and others
    }
    if (type instanceof BoolType) return "l"
    if (type instanceof StringType) return "l"  // pointer as l
    if (type instanceof PointerType) return "l"
    if (type instanceof ArrayType) return "l"   // pointer as l
    if (type instanceof MapType) return "l"
    if (type instanceof StructType) return "l"
    if (type instanceof SOAType) return "l"
    if (type instanceof CustomType) return "l"
    if (type instanceof FunctionType) return "l"
    if (type instanceof TensorType) return "l"
    if (type instanceof ResultType) return "l"
    if (type instanceof OptionalType) return "l"
    if (type instanceof FutureType) return this.toQBEType(type.innerType)
    if (type instanceof VoidType) return "void"
    if (type instanceof TypeAliasType) return this.toQBEType(type.aliasedType)
    return "l"
  }

  // Return the QBE type suffix for function return or "void"
  private retTypeStr(type: any): string {
    const t = this.toQBEType(type)
    return t === "void" ? "" : t
  }

  // --- String Escaping for QBE ---
  private escapeStringForQBE(str: string): { escaped: string; byteLength: number } {
    let result = ""
    let byteLength = 0
    for (const char of str) {
      const code = char.codePointAt(0)!
      if (char === "\\") {
        // QBE uses C-style escapes in byte strings
        result += "\\\\"
        byteLength += 1
      } else if (char === '"') {
        result += '\\"'
        byteLength += 1
      } else if (code === 10) { // newline
        result += "\\n"
        byteLength += 1
      } else if (code === 13) { // carriage return
        result += "\\r"
        byteLength += 1
      } else if (code === 9) { // tab
        result += "\\t"
        byteLength += 1
      } else if (code === 0) {
        result += "\\0"
        byteLength += 1
      } else if (code >= 32 && code <= 126) {
        result += char
        byteLength += 1
      } else {
        // Multi-byte UTF-8: emit as individual escaped bytes
        const bytes = new TextEncoder().encode(char)
        // Close current string, emit bytes, reopen string
        for (const b of bytes) {
          result += `", b ${b}, b "`
        }
        byteLength += bytes.length
      }
    }
    return { escaped: result, byteLength }
  }

  private getStringByteLength(str: string): number {
    const cached = this.stringByteLengths.get(str)
    if (cached !== undefined) return cached
    const byteLength = new TextEncoder().encode(str).length
    this.stringByteLengths.set(str, byteLength)
    return byteLength
  }

  // Get or create a string constant, returns the QBE global name like $str.0
  private getOrCreateString(value: string): string {
    const existing = this.stringNameMap.get(value)
    if (existing) return existing

    const name = `$str.${this.stringCounter++}`
    this.stringNameMap.set(value, name)
    const { escaped, byteLength } = this.escapeStringForQBE(value)
    this.stringByteLengths.set(value, byteLength)
    this.emitData(`data ${name} = { b "${escaped}", b 0 }`)
    return name
  }

  // --- Float Handling ---
  // Convert a JS number to QBE double literal format
  private floatToQBE(value: number): string {
    // QBE accepts d_<IEEE754hex> for double constants
    const buf = new ArrayBuffer(8)
    const view = new DataView(buf)
    view.setFloat64(0, value, false)
    const bits = view.getBigUint64(0, false)
    return `d_${bits.toString(16).padStart(16, "0")}`
  }

  private floatToSingleQBE(value: number): string {
    const buf = new ArrayBuffer(4)
    const view = new DataView(buf)
    view.setFloat32(0, value, false)
    const bits = view.getUint32(0, false)
    return `s_${bits.toString(16).padStart(8, "0")}`
  }

  // --- Float Detection Helpers ---
  isFloatOperand(expr: any): boolean {
    if (!expr) return false
    if (expr.type === "NumberLiteral") {
      if (expr.kind === "f64" || expr.kind === "f32") return true
      if (typeof expr.value === "number" && String(expr.value).includes(".")) return true
      return false
    }
    if (expr.type === "Identifier") {
      const vt = this.variableTypes.get(expr.name)
      if (vt instanceof FloatType) return true
      if (vt instanceof StructType || vt instanceof SOAType) return false
      return false
    }
    if (expr.type === "MemberExpression") {
      const objType = this.variableTypes.get(expr.object?.name)
      if (objType instanceof StructType) {
        const fields = this.structDefs.get(objType.name)
        const f = fields?.find((ff: any) => ff.name === expr.property)
        if (f && f.fieldType instanceof FloatType) return true
      }
      return false
    }
    if (expr.type === "IndexExpression") {
      const arrType = this.variableTypes.get(expr.object?.name)
      if (arrType instanceof ArrayType && arrType.elementType instanceof FloatType) return true
      return false
    }
    if (expr.type === "BinaryExpression" || expr.type === "UnaryExpression") {
      return this.isFloatOperand(expr.left || expr.argument)
    }
    if (expr.type === "CallExpression") {
      const callee = expr.callee
      if (callee?.type === "Identifier") {
        const mathFuncs = new Set(["sqrt", "sin", "cos", "tan", "asin", "acos", "atan2", "exp", "log", "log2", "floor", "ceil", "round", "fabs", "pow", "fmin", "fmax"])
        if (mathFuncs.has(callee.name)) return true
        const retType = this.functionTypes.get(callee.name)
        if (retType instanceof FloatType) return true
      }
      return false
    }
    if (expr.resultType instanceof FloatType) return true
    return false
  }

  isStringProducingExpression(expr: any): boolean {
    if (!expr) return false
    if (expr.type === "StringLiteral" || expr.type === "TemplateLiteral") return true
    if (expr.type === "Identifier") {
      const vt = this.variableTypes.get(expr.name)
      return vt instanceof StringType
    }
    if (expr.type === "CallExpression") {
      const name = expr.callee?.name
      if (name === "str" || name === "i64_to_string" || name === "u64_to_string" || name === "f64_to_string" || name === "input_string") return true
      if (expr.callee?.type === "MemberExpression") {
        const method = expr.callee.property
        const strMethods = new Set(["upper", "lower", "trim", "replace", "split", "char_at"])
        if (strMethods.has(method)) return true
      }
    }
    if (expr.resultType instanceof StringType) return true
    return false
  }

  isArrayExpression(expr: any): boolean {
    if (expr.type === "ArrayLiteral" || expr.type === "ArrayFill") return true
    if (expr.type === "Identifier") {
      const vt = this.variableTypes.get(expr.name)
      return vt instanceof ArrayType
    }
    if (expr.resultType instanceof ArrayType) return true
    return false
  }

  isMapExpression(expr: any): boolean {
    if (expr.type === "MapLiteral") return true
    if (expr.type === "Identifier") {
      const vt = this.variableTypes.get(expr.name)
      return vt instanceof MapType
    }
    if (expr.resultType instanceof MapType) return true
    return false
  }

  isStructExpression(expr: any): boolean {
    if (expr.type === "StructLiteral") return true
    if (expr.type === "Identifier") {
      const vt = this.variableTypes.get(expr.name)
      return vt instanceof StructType
    }
    if (expr.resultType instanceof StructType) return true
    return false
  }

  // Infer the expression type string for template literal interpolation
  inferExpressionType(expr: any): string {
    if (this.isStringProducingExpression(expr)) return "string"
    if (this.isFloatOperand(expr)) return "float"
    return "int"
  }

  // Get array element type
  getArrayElementType(expr: any): any {
    if (expr.type === "Identifier") {
      const vt = this.variableTypes.get(expr.name)
      if (vt instanceof ArrayType) return vt.elementType
    }
    if (expr.resultType instanceof ArrayType) return expr.resultType.elementType
    return null
  }

  // --- Async Detection ---
  private detectAsyncFunctions(ast: ProgramNode): void {
    this.detectAsyncInStatements(ast.statements)
  }

  private detectAsyncInStatements(stmts: any[]): void {
    for (const stmt of stmts) {
      if (stmt.type === "AsyncFunctionDeclaration") {
        this.asyncFunctions.add(stmt.name)
        this.hasAsyncFunctions = true
      }
      if (stmt.type === "FunctionDeclaration" && stmt.isAsync) {
        this.asyncFunctions.add(stmt.name)
        this.hasAsyncFunctions = true
      }
      if (stmt.type === "Block" && stmt.statements) {
        this.detectAsyncInStatements(stmt.statements)
      }
    }
  }

  // --- String Constant Pre-collection ---
  // Walk the AST to find all string literals and pre-register them
  collectStringConstants(ast: any): void {
    if (!ast) return
    if (Array.isArray(ast)) {
      for (const node of ast) this.collectStringConstants(node)
      return
    }
    if (typeof ast !== "object") return

    if (ast.type === "StringLiteral" && typeof ast.value === "string") {
      this.getOrCreateString(ast.value)
    }
    if (ast.type === "TemplateLiteral" && ast.parts) {
      for (const part of ast.parts) {
        if (typeof part === "string") {
          this.getOrCreateString(part)
        } else if (part.type === "string" || typeof part.value === "string") {
          this.getOrCreateString(part.value || part)
        }
      }
    }
    if (ast.type === "MapLiteral" && ast.entries) {
      for (const entry of ast.entries) {
        if (typeof entry.key === "string") {
          this.getOrCreateString(entry.key)
        }
      }
    }
    if (ast.type === "MemberExpression" && typeof ast.property === "string") {
      this.getOrCreateString(ast.property)
    }

    // Recurse into all object properties
    for (const key of Object.keys(ast)) {
      const val = ast[key]
      if (val && typeof val === "object") {
        this.collectStringConstants(val)
      }
    }
  }

  // Collect function declarations from AST
  private collectFunctionDeclarations(ast: ProgramNode): any[] {
    return ast.statements.filter((s: any) =>
      s.type === "FunctionDeclaration" || s.type === "AsyncFunctionDeclaration"
    )
  }

  // Recursively find all function declarations including nested ones
  private findFunctionDeclarationsRecursive(statements: any[]): any[] {
    const result: any[] = []
    for (const stmt of statements) {
      if (stmt.type === "FunctionDeclaration" || stmt.type === "AsyncFunctionDeclaration") {
        result.push(stmt)
      }
      if (stmt.type === "Block" && stmt.statements) {
        result.push(...this.findFunctionDeclarationsRecursive(stmt.statements))
      }
      if (stmt.body?.statements) {
        result.push(...this.findFunctionDeclarationsRecursive(stmt.body.statements))
      }
    }
    return result
  }

  // Pre-register function signatures (for forward references)
  private preRegisterFunctions(ast: ProgramNode): void {
    const allFuncs = this.findFunctionDeclarationsRecursive(ast.statements)
    for (const func of allFuncs) {
      const name = func.name === "main" ? "program_user" : (this.packagePrefix + func.name)
      const retType = func.returnType ? this.toQBEType(func.returnType) : "l"
      // Async functions return a pointer (MogFuture*)
      if (func.type === "AsyncFunctionDeclaration" || func.isAsync) {
        this.functionTypes.set(name, func.returnType || new IntegerType("i64"))
        this.asyncFunctions.add(name)
      } else {
        this.functionTypes.set(name, func.returnType)
      }
      // Store param info for named/default arg resolution
      if (func.params) {
        this.functionParamInfo.set(name, func.params.map((p: any) => ({
          name: p.name,
          paramType: p.paramType,
          defaultValue: p.defaultValue,
        })))
      }
    }
  }

  // Pre-register struct definitions
  private preRegisterStructs(ast: ProgramNode): void {
    for (const stmt of ast.statements) {
      if (stmt.type === "StructDeclaration") {
        const fields = stmt.fields.map((f: any) => ({
          name: f.name,
          fieldType: f.fieldType,
        }))
        this.structDefs.set(stmt.name, fields)
      }
    }
  }

  // ============================================================
  // EXPRESSION GENERATION (stubs - to be filled by later tasks)
  // ============================================================
  generateExpression(ir: string[], expr: any): string {
    if (!expr) return "0"

    switch (expr.type) {
      case "NumberLiteral":
        return this.generateNumberLiteral(expr)
      case "BooleanLiteral":
        return expr.value ? "1" : "0"
      case "NullLiteral":
      case "NoneLiteral":
        return "0"
      case "StringLiteral":
        return this.generateStringLiteral(expr)
      case "TemplateLiteral":
        return this.generateTemplateLiteral(ir, expr)
      case "Identifier":
        return this.generateIdentifier(ir, expr)
      case "BinaryExpression":
        return this.generateBinaryExpression(ir, expr)
      case "UnaryExpression":
        return this.generateUnaryExpression(ir, expr)
      case "CallExpression":
        return this.generateCallExpression(ir, expr)
      case "AssignmentExpression":
        return this.generateAssignmentExpression(ir, expr)
      case "ArrayLiteral":
        return this.generateArrayLiteral(ir, expr)
      case "ArrayFill":
        return this.generateArrayFill(ir, expr)
      case "MapLiteral":
        return this.generateMapLiteral(ir, expr)
      case "StructLiteral":
        return this.generateStructLiteral(ir, expr)
      case "SoAConstructor":
        return this.generateSoAConstructor(ir, expr)
      case "SoALiteral":
        return "0" // deprecated
      case "MemberExpression":
        return this.generateMemberExpression(ir, expr)
      case "IndexExpression":
        return this.generateIndexExpression(ir, expr)
      case "SliceExpression":
        return this.generateSliceExpression(ir, expr)
      case "MapExpression":
        return this.generateExpression(ir, expr.collection)
      case "CastExpression":
        return this.generateCastExpression(ir, expr)
      case "IfExpression":
        return this.generateIfExpression(ir, expr)
      case "MatchExpression":
        return this.generateMatchExpression(ir, expr)
      case "Lambda":
        return this.generateLambda(ir, expr)
      case "Ok":
        return this.generateOkExpression(ir, expr)
      case "Err":
        return this.generateErrExpression(ir, expr)
      case "Some":
        return this.generateSomeExpression(ir, expr)
      case "None":
        return this.generateNoneExpression(ir)
      case "PropagateExpression":
        return this.generatePropagateExpression(ir, expr)
      case "IsSome":
        return this.generateIsSomeExpression(ir, expr)
      case "IsNone":
        return this.generateIsNoneExpression(ir, expr)
      case "IsOk":
        return this.generateIsOkExpression(ir, expr)
      case "IsErr":
        return this.generateIsErrExpression(ir, expr)
      case "AwaitExpression":
        return this.generateAwaitExpression(ir, expr)
      case "SpawnExpression":
        return this.generateSpawnExpression(ir, expr)
      case "TensorConstruction":
        return this.generateTensorConstruction(ir, expr)
      default:
        return "0"
    }
  }

  // ============================================================
  // STATEMENT GENERATION (stubs - to be filled by later tasks)
  // ============================================================
  generateStatement(ir: string[], stmt: any): void {
    if (this.blockTerminated) return

    switch (stmt.type) {
      case "VariableDeclaration":
        this.generateVariableDeclaration(ir, stmt)
        break
      case "ExpressionStatement":
        this.generateExpression(ir, stmt.expression)
        break
      case "ReturnStatement":
      case "Return":
        this.generateReturn(ir, stmt)
        break
      case "Conditional":
        this.generateConditional(ir, stmt)
        break
      case "WhileLoop":
        this.generateWhileLoop(ir, stmt)
        break
      case "ForLoop":
        this.generateForLoop(ir, stmt)
        break
      case "ForEachLoop":
        this.generateForEachLoop(ir, stmt)
        break
      case "ForInRange":
        this.generateForInRange(ir, stmt)
        break
      case "ForInIndex":
        this.generateForInIndex(ir, stmt)
        break
      case "ForInMap":
        this.generateForInMap(ir, stmt)
        break
      case "Block":
        this.generateBlock(ir, stmt)
        break
      case "Break":
        this.generateBreak(ir)
        break
      case "Continue":
        this.generateContinue(ir)
        break
      case "FunctionDeclaration":
      case "AsyncFunctionDeclaration":
        // Functions are hoisted and generated separately
        break
      case "StructDeclaration":
        // Struct defs are pre-registered
        break
      case "TryCatch":
        this.generateTryCatch(ir, stmt)
        break
      case "PackageDeclaration":
      case "ImportDeclaration":
        // Module declarations handled at top level
        break
      default:
        break
    }
  }

  // ============================================================
  // STUB METHODS - Each will be implemented by its own pebble task
  // ============================================================

  // --- Literals (qbe-ec4) ---
  private generateNumberLiteral(expr: any): string {
    if (expr.kind === "f64") {
      return this.floatToQBE(expr.value)
    }
    if (expr.kind === "f32") {
      return this.floatToSingleQBE(expr.value)
    }
    // If value contains a decimal point but no explicit kind, treat as f64
    if (typeof expr.value === "number" && String(expr.value).includes(".")) {
      return this.floatToQBE(expr.value)
    }
    // Integer literal — QBE accepts immediates directly
    return String(expr.value)
  }

  private generateStringLiteral(expr: any): string {
    return this.getOrCreateString(expr.value)
  }
  private generateIdentifier(ir: string[], expr: any): string {
    const slot = this.variables.get(expr.name)
    if (slot) {
      const reg = this.nextReg()
      const vt = this.variableTypes.get(expr.name)
      if (vt instanceof FloatType) {
        ir.push(`  ${reg} =d loadd ${slot}`)
      } else {
        ir.push(`  ${reg} =l loadl ${slot}`)
      }
      return reg
    }
    // Function reference (e.g., passing a function as a value)
    if (this.functionTypes.has(expr.name)) {
      return `$${expr.name}`
    }
    return "0"
  }

  // --- Binary/Unary (qbe-394) ---
  private generateBinaryExpression(ir: string[], expr: any): string {
    const op = expr.operator
    const isFloat = this.isFloatOperand(expr.left) || this.isFloatOperand(expr.right)

    // --- String equality ---
    if (op === "==" || op === "!=") {
      const isStringLeft = this.isStringProducingExpression(expr.left)
      const isStringRight = this.isStringProducingExpression(expr.right)
      if (isStringLeft || isStringRight) {
        const left = this.generateExpression(ir, expr.left)
        const right = this.generateExpression(ir, expr.right)
        const r = this.nextReg()
        ir.push(`  ${r} =l call $string_eq(l ${left}, l ${right})`)
        if (op === "!=") {
          const r2 = this.nextReg()
          ir.push(`  ${r2} =l xor ${r}, 1`)
          return r2
        }
        return r
      }
    }

    // Generate operands
    let left = this.generateExpression(ir, expr.left)
    let right = this.generateExpression(ir, expr.right)

    // --- Float operations ---
    if (isFloat) {
      // Cast integer operands to double if needed
      if (!this.isFloatOperand(expr.left)) {
        const tmp = this.nextReg()
        ir.push(`  ${tmp} =d swtof ${left}`)
        left = tmp
      }
      if (!this.isFloatOperand(expr.right)) {
        const tmp = this.nextReg()
        ir.push(`  ${tmp} =d swtof ${right}`)
        right = tmp
      }

      // Float arithmetic
      if (op === "+") { const r = this.nextReg(); ir.push(`  ${r} =d add ${left}, ${right}`); return r }
      if (op === "-") { const r = this.nextReg(); ir.push(`  ${r} =d sub ${left}, ${right}`); return r }
      if (op === "*") { const r = this.nextReg(); ir.push(`  ${r} =d mul ${left}, ${right}`); return r }
      if (op === "/") { const r = this.nextReg(); ir.push(`  ${r} =d div ${left}, ${right}`); return r }

      // Float comparisons (return w, then extend to l)
      let cmpOp = ""
      if (op === "==") cmpOp = "ceqd"
      else if (op === "!=") cmpOp = "cned"
      else if (op === "<") cmpOp = "cltd"
      else if (op === ">") cmpOp = "cgtd"
      else if (op === "<=") cmpOp = "cled"
      else if (op === ">=") cmpOp = "cged"

      if (cmpOp) {
        const rw = this.nextReg()
        ir.push(`  ${rw} =w ${cmpOp} ${left}, ${right}`)
        const rl = this.nextReg()
        ir.push(`  ${rl} =l extsw ${rw}`)
        return rl
      }
    }

    // --- Integer arithmetic ---
    if (op === "+") { const r = this.nextReg(); ir.push(`  ${r} =l add ${left}, ${right}`); return r }
    if (op === "-") { const r = this.nextReg(); ir.push(`  ${r} =l sub ${left}, ${right}`); return r }
    if (op === "*") { const r = this.nextReg(); ir.push(`  ${r} =l mul ${left}, ${right}`); return r }
    if (op === "/") { const r = this.nextReg(); ir.push(`  ${r} =l sdiv ${left}, ${right}`); return r }
    if (op === "%") { const r = this.nextReg(); ir.push(`  ${r} =l srem ${left}, ${right}`); return r }

    // --- Integer comparisons (return w, extend to l) ---
    let intCmpOp = ""
    if (op === "==") intCmpOp = "ceql"
    else if (op === "!=") intCmpOp = "cnel"
    else if (op === "<") intCmpOp = "csltl"
    else if (op === ">") intCmpOp = "csgtl"
    else if (op === "<=") intCmpOp = "cslel"
    else if (op === ">=") intCmpOp = "csgel"

    if (intCmpOp) {
      const rw = this.nextReg()
      ir.push(`  ${rw} =w ${intCmpOp} ${left}, ${right}`)
      const rl = this.nextReg()
      ir.push(`  ${rl} =l extsw ${rw}`)
      return rl
    }

    // --- Logical operators ---
    if (op === "&&") {
      const lBool = this.nextReg()
      ir.push(`  ${lBool} =w cnel ${left}, 0`)
      const rBool = this.nextReg()
      ir.push(`  ${rBool} =w cnel ${right}, 0`)
      const rw = this.nextReg()
      ir.push(`  ${rw} =w and ${lBool}, ${rBool}`)
      const rl = this.nextReg()
      ir.push(`  ${rl} =l extsw ${rw}`)
      return rl
    }
    if (op === "||") {
      const lBool = this.nextReg()
      ir.push(`  ${lBool} =w cnel ${left}, 0`)
      const rBool = this.nextReg()
      ir.push(`  ${rBool} =w cnel ${right}, 0`)
      const rw = this.nextReg()
      ir.push(`  ${rw} =w or ${lBool}, ${rBool}`)
      const rl = this.nextReg()
      ir.push(`  ${rl} =l extsw ${rw}`)
      return rl
    }

    // --- Bitwise operators ---
    if (op === "&") { const r = this.nextReg(); ir.push(`  ${r} =l and ${left}, ${right}`); return r }
    if (op === "|") { const r = this.nextReg(); ir.push(`  ${r} =l or ${left}, ${right}`); return r }
    if (op === "^") { const r = this.nextReg(); ir.push(`  ${r} =l xor ${left}, ${right}`); return r }
    if (op === "<<") { const r = this.nextReg(); ir.push(`  ${r} =l shl ${left}, ${right}`); return r }
    if (op === ">>") { const r = this.nextReg(); ir.push(`  ${r} =l sar ${left}, ${right}`); return r }

    // Unknown operator - return 0
    return "0"
  }
  private generateUnaryExpression(ir: string[], expr: any): string {
    const op = expr.operator
    const operand = expr.argument || expr.operand
    const val = this.generateExpression(ir, operand)
    const isFloat = this.isFloatOperand(operand)

    if (op === "-") {
      if (isFloat) {
        const r = this.nextReg()
        ir.push(`  ${r} =d neg ${val}`)
        return r
      }
      const r = this.nextReg()
      ir.push(`  ${r} =l sub 0, ${val}`)
      return r
    }

    if (op === "!") {
      const r = this.nextReg()
      ir.push(`  ${r} =l xor ${val}, 1`)
      return r
    }

    if (op === "~") {
      const r = this.nextReg()
      ir.push(`  ${r} =l xor ${val}, -1`)
      return r
    }

    // Unknown operator
    return val
  }

  // --- Calls (qbe-38e) ---
  private generateCallExpression(ir: string[], expr: any): string {
    // --- 1. Direct identifier calls ---
    if (expr.callee?.type === "Identifier") {
      const name = expr.callee.name

      // Generate all argument registers first
      const argRegs: string[] = []
      for (const arg of (expr.args || [])) {
        argRegs.push(this.generateExpression(ir, arg))
      }

      // --- Print functions ---
      const printFuncs = new Set([
        "print", "println", "print_string", "println_string",
        "print_i64", "println_i64", "print_f64", "println_f64",
        "print_u64", "println_u64", "print_buffer",
      ])
      if (printFuncs.has(name)) {
        return this.generatePrintCall(ir, name, argRegs, expr)
      }

      // --- Math builtins (single-arg) ---
      const mathSingle = new Set([
        "sqrt", "sin", "cos", "tan", "asin", "acos",
        "exp", "log", "log2", "floor", "ceil", "round", "abs",
      ])
      if (mathSingle.has(name)) {
        const r = this.nextReg()
        const qbeName = name === "abs" ? "fabs" : name
        ir.push(`  ${r} =d call $${qbeName}(d ${argRegs[0]})`)
        return r
      }

      // --- Math builtins (two-arg) ---
      const mathDouble = new Map([
        ["pow", "pow"], ["atan2", "atan2"],
        ["min", "fmin"], ["max", "fmax"],
        ["fmin", "fmin"], ["fmax", "fmax"],
      ])
      if (mathDouble.has(name)) {
        const r = this.nextReg()
        const qbeName = mathDouble.get(name)!
        ir.push(`  ${r} =d call $${qbeName}(d ${argRegs[0]}, d ${argRegs[1]})`)
        return r
      }

      // --- Conversion functions ---
      if (name === "str" || name === "i64_to_string") {
        const r = this.nextReg()
        ir.push(`  ${r} =l call $i64_to_string(l ${argRegs[0]})`)
        return r
      }
      if (name === "u64_to_string") {
        const r = this.nextReg()
        ir.push(`  ${r} =l call $u64_to_string(l ${argRegs[0]})`)
        return r
      }
      if (name === "f64_to_string") {
        const r = this.nextReg()
        ir.push(`  ${r} =l call $f64_to_string(d ${argRegs[0]})`)
        return r
      }
      if (name === "int_from_string") {
        const r = this.nextReg()
        ir.push(`  ${r} =l call $int_from_string(l ${argRegs[0]})`)
        return r
      }
      if (name === "float_from_string") {
        const r = this.nextReg()
        ir.push(`  ${r} =d call $float_from_string(l ${argRegs[0]})`)
        return r
      }

      // --- Input functions ---
      if (name === "input_i64" || name === "input_u64") {
        const r = this.nextReg()
        ir.push(`  ${r} =l call $${name}()`)
        return r
      }
      if (name === "input_f64") {
        const r = this.nextReg()
        ir.push(`  ${r} =d call $input_f64()`)
        return r
      }
      if (name === "input_string") {
        const r = this.nextReg()
        ir.push(`  ${r} =l call $input_string()`)
        return r
      }

      // --- tensor_print ---
      if (name === "tensor_print") {
        ir.push(`  call $tensor_print(l ${argRegs[0]})`)
        return "0"
      }

      // --- GC functions ---
      if (name === "gc_collect" || name === "gc_benchmark_stats" || name === "gc_reset_stats") {
        ir.push(`  call $${name}()`)
        return "0"
      }

      // --- General function calls ---
      // Resolve named arguments and default parameters if present
      let finalArgRegs = argRegs
      if (expr.namedArgs && this.functionParamInfo.has(name)) {
        const paramInfo = this.functionParamInfo.get(name)!
        finalArgRegs = this.resolveNamedArgs(ir, name, expr.namedArgs, argRegs, paramInfo, expr)
      }

      const retType = this.functionTypes.get(name)
      const qbeRet = this.toQBEType(retType)

      // Build argument list
      const argList = finalArgRegs.map((reg) => `l ${reg}`).join(", ")

      if (qbeRet === "void") {
        ir.push(`  call $${name}(${argList})`)
        return "0"
      }

      const r = this.nextReg()
      // Use the correct QBE type for the return
      const retStr = qbeRet === "d" || qbeRet === "s" ? qbeRet : "l"
      if (retStr === "d" || retStr === "s") {
        // Float return - also need float-typed args for params that are float
        const typedArgs = this.buildTypedArgList(name, finalArgRegs)
        ir.push(`  ${r} =${retStr} call $${name}(${typedArgs})`)
      } else {
        ir.push(`  ${r} =l call $${name}(${argList})`)
      }
      return r
    }

    // --- 2. Member expression calls ---
    if (expr.callee?.type === "MemberExpression") {
      const obj = expr.callee.object
      const method = expr.callee.property
      const objName = obj?.name || ""

      // Generate object register
      const objReg = this.generateExpression(ir, obj)

      // Generate argument registers
      const argRegs: string[] = []
      for (const arg of (expr.args || [])) {
        argRegs.push(this.generateExpression(ir, arg))
      }

      // --- Package-qualified calls ---
      if (this.importedPackages.has(objName)) {
        const pkg = this.importedPackages.get(objName)!
        const qualifiedName = `${pkg}__${method}`

        // Build args: all arg registers
        const allArgs = argRegs.map((reg) => `l ${reg}`).join(", ")

        const retType = this.functionTypes.get(qualifiedName)
        const qbeRet = this.toQBEType(retType)

        if (qbeRet === "void") {
          ir.push(`  call $${qualifiedName}(${allArgs})`)
          return "0"
        }

        const r = this.nextReg()
        const retStr = qbeRet === "d" || qbeRet === "s" ? qbeRet : "l"
        ir.push(`  ${r} =${retStr} call $${qualifiedName}(${allArgs})`)
        return r
      }

      // --- Capability calls ---
      if (this.capabilities.has(objName)) {
        return this.generateCapabilityCall(ir, objName, method, argRegs, expr)
      }

      // --- String methods ---
      const stringMethods = new Map<string, { runtime: string; returnsString: boolean; extraArgs: boolean }>([
        ["upper", { runtime: "string_upper", returnsString: true, extraArgs: false }],
        ["lower", { runtime: "string_lower", returnsString: true, extraArgs: false }],
        ["trim", { runtime: "string_trim", returnsString: true, extraArgs: false }],
        ["split", { runtime: "string_split", returnsString: false, extraArgs: true }],
        ["contains", { runtime: "string_contains", returnsString: false, extraArgs: true }],
        ["starts_with", { runtime: "string_starts_with", returnsString: false, extraArgs: true }],
        ["ends_with", { runtime: "string_ends_with", returnsString: false, extraArgs: true }],
        ["replace", { runtime: "string_replace", returnsString: true, extraArgs: true }],
        ["len", { runtime: "string_len", returnsString: false, extraArgs: false }],
        ["index_of", { runtime: "string_index_of", returnsString: false, extraArgs: true }],
      ])
      if (this.isStringProducingExpression(obj) || this.variableTypes.get(objName) instanceof StringType) {
        const strMethod = stringMethods.get(method)
        if (strMethod) {
          const r = this.nextReg()
          const args = strMethod.extraArgs
            ? `l ${objReg}, ${argRegs.map((a) => `l ${a}`).join(", ")}`
            : `l ${objReg}`
          ir.push(`  ${r} =l call $${strMethod.runtime}(${args})`)
          return r
        }
      }

      // --- Array methods ---
      const objType = this.variableTypes.get(objName)
      if (objType instanceof ArrayType || this.isArrayExpression(obj)) {
        if (method === "push") {
          ir.push(`  call $array_push(l ${objReg}, l ${argRegs[0]})`)
          return "0"
        }
        if (method === "pop") {
          const r = this.nextReg()
          ir.push(`  ${r} =l call $array_pop(l ${objReg})`)
          return r
        }
        if (method === "contains") {
          const r = this.nextReg()
          ir.push(`  ${r} =l call $array_contains(l ${objReg}, l ${argRegs[0]})`)
          return r
        }
        if (method === "sort") {
          ir.push(`  call $array_sort(l ${objReg})`)
          return "0"
        }
        if (method === "reverse") {
          ir.push(`  call $array_reverse(l ${objReg})`)
          return "0"
        }
        if (method === "len") {
          const r = this.nextReg()
          ir.push(`  ${r} =l call $array_len(l ${objReg})`)
          return r
        }
      }

      // --- Tensor methods ---
      if (objType instanceof TensorType || isTensorType(objType)) {
        if (method === "sum") {
          const r = this.nextReg()
          ir.push(`  ${r} =d call $tensor_sum(l ${objReg})`)
          return r
        }
        if (method === "mean") {
          const r = this.nextReg()
          ir.push(`  ${r} =d call $tensor_mean(l ${objReg})`)
          return r
        }
        if (method === "matmul") {
          const r = this.nextReg()
          ir.push(`  ${r} =l call $tensor_matmul(l ${objReg}, l ${argRegs[0]})`)
          return r
        }
        if (method === "reshape") {
          const r = this.nextReg()
          ir.push(`  ${r} =l call $tensor_reshape(l ${objReg}, l ${argRegs[0]})`)
          return r
        }
        if (method === "print") {
          ir.push(`  call $tensor_print(l ${objReg})`)
          return "0"
        }
        if (method === "shape") {
          const r = this.nextReg()
          ir.push(`  ${r} =l call $tensor_shape(l ${objReg})`)
          return r
        }
        if (method === "ndim") {
          const r = this.nextReg()
          ir.push(`  ${r} =l call $tensor_ndim(l ${objReg})`)
          return r
        }
        if (method === "size") {
          const r = this.nextReg()
          ir.push(`  ${r} =l call $tensor_size(l ${objReg})`)
          return r
        }
      }

      // --- Fallback: generic method call on object ---
      const allArgs = [objReg, ...argRegs].map((reg) => `l ${reg}`).join(", ")
      const r = this.nextReg()
      ir.push(`  ${r} =l call $${method}(${allArgs})`)
      return r
    }

    // --- 3. Indirect calls (closures) ---
    // Variable holds a closure struct: {fn_ptr, env_ptr}
    const closureReg = this.generateExpression(ir, expr.callee)

    // Load fn_ptr from offset 0
    const fnPtr = this.nextReg()
    ir.push(`  ${fnPtr} =l loadl ${closureReg}`)

    // Load env_ptr from offset 8
    const envOff = this.nextReg()
    ir.push(`  ${envOff} =l add ${closureReg}, 8`)
    const envPtr = this.nextReg()
    ir.push(`  ${envPtr} =l loadl ${envOff}`)

    // Generate argument registers
    const argRegs: string[] = []
    for (const arg of (expr.args || [])) {
      argRegs.push(this.generateExpression(ir, arg))
    }

    // Build call: fn_ptr(env, arg1, arg2, ...)
    const callArgs = [`l ${envPtr}`, ...argRegs.map((a) => `l ${a}`)].join(", ")
    const r = this.nextReg()
    ir.push(`  ${r} =l call ${fnPtr}(${callArgs})`)
    return r
  }

  /**
   * Resolve named arguments against parameter info, filling defaults.
   */
  private resolveNamedArgs(
    ir: string[],
    funcName: string,
    namedArgs: { name: string; value: any }[],
    positionalRegs: string[],
    paramInfo: { name: string; paramType: any; defaultValue?: any }[],
    expr: any,
  ): string[] {
    const result: string[] = new Array(paramInfo.length).fill("0")
    const namedMap = new Map<string, any>()
    for (const na of namedArgs) {
      namedMap.set(na.name, na.value)
    }

    // Fill positional args first
    for (let i = 0; i < positionalRegs.length && i < paramInfo.length; i++) {
      result[i] = positionalRegs[i]
    }

    // Override with named args
    for (let i = 0; i < paramInfo.length; i++) {
      const namedVal = namedMap.get(paramInfo[i].name)
      if (namedVal) {
        result[i] = this.generateExpression(ir, namedVal)
      } else if (result[i] === "0" && paramInfo[i].defaultValue) {
        result[i] = this.generateExpression(ir, paramInfo[i].defaultValue)
      }
    }

    return result
  }

  /**
   * Build a typed argument list for calls to functions with known param types.
   */
  private buildTypedArgList(funcName: string, argRegs: string[]): string {
    const paramInfo = this.functionParamInfo.get(funcName)
    if (!paramInfo) {
      return argRegs.map((reg) => `l ${reg}`).join(", ")
    }
    return argRegs.map((reg, i) => {
      if (i < paramInfo.length) {
        const pt = this.toQBEType(paramInfo[i].paramType)
        return `${pt === "d" || pt === "s" ? pt : "l"} ${reg}`
      }
      return `l ${reg}`
    }).join(", ")
  }

  // --- Assignments (qbe-3ae) ---
  private generateVariableDeclaration(ir: string[], stmt: any): void {
    // Allocate stack space for the variable
    const slot = this.nextReg()
    this.emitAlloc(`  ${slot} =l alloc8 8`)
    this.variables.set(stmt.name, slot)

    // Track type if annotation exists
    if (stmt.typeAnnotation) {
      this.variableTypes.set(stmt.name, stmt.typeAnnotation)
    } else if (stmt.varType) {
      this.variableTypes.set(stmt.name, stmt.varType)
    }

    // Generate initializer or default to 0
    if (stmt.initializer) {
      const val = this.generateExpression(ir, stmt.initializer)
      const vt = this.variableTypes.get(stmt.name)
      if (vt instanceof FloatType) {
        ir.push(`  stored ${val}, ${slot}`)
      } else {
        ir.push(`  storel ${val}, ${slot}`)
      }
    } else {
      ir.push(`  storel 0, ${slot}`)
    }
  }
  private generateAssignmentExpression(ir: string[], expr: any): string {
    // Case A: Declaration (:= operator or isDeclaration flag, or new variable with no operator)
    const declName = expr.name || expr.target?.name
    const isDeclAssignment = expr.isDeclaration ||
      (declName && !expr.operator && !this.variables.has(declName) && !expr.target)
    if (isDeclAssignment) {
      const name = declName
      if (!name) return "0"

      const slot = this.nextReg()
      this.emitAlloc(`  ${slot} =l alloc8 8`)
      this.variables.set(name, slot)

      // Infer and track type from initializer
      if (expr.value) {
        if (expr.value.type === "StructLiteral") {
          const sn = expr.value.structName || expr.value.name
          if (sn) this.variableTypes.set(name, new StructType(sn))
        } else if (expr.value.type === "ArrayLiteral" || expr.value.type === "ArrayFill") {
          this.variableTypes.set(name, new ArrayType(new IntegerType("i64"), []))
        } else if (expr.value.type === "MapLiteral") {
          this.variableTypes.set(name, new MapType(new StringType(), new IntegerType("i64")))
        }
      }

      const val = this.generateExpression(ir, expr.value)
      ir.push(`  storel ${val}, ${slot}`)
      return val
    }

    // Case D: Index assignment (arr[i] = val)
    if (expr.target?.type === "IndexExpression") {
      const val = this.generateExpression(ir, expr.value)
      const obj = this.generateExpression(ir, expr.target.object)
      const idx = this.generateExpression(ir, expr.target.index)

      // Check if this is a map
      const objType = this.variableTypes.get(expr.target.object?.name)
      if (objType instanceof MapType) {
        // Map set: $map_set(map, key, value)
        ir.push(`  call $map_set(l ${obj}, l ${idx}, l ${val})`)
        return val
      }

      // Array index assignment: data = *(obj + 16), ptr = data + idx * 8
      const dataPtr = this.nextReg()
      ir.push(`  ${dataPtr} =l add ${obj}, 16`)
      const data = this.nextReg()
      ir.push(`  ${data} =l loadl ${dataPtr}`)
      const off = this.nextReg()
      ir.push(`  ${off} =l mul ${idx}, 8`)
      const elemPtr = this.nextReg()
      ir.push(`  ${elemPtr} =l add ${data}, ${off}`)
      ir.push(`  storel ${val}, ${elemPtr}`)
      return val
    }

    // Case E: Member assignment (s.field = val)
    if (expr.target?.type === "MemberExpression") {
      const val = this.generateExpression(ir, expr.value)
      const structPtr = this.generateExpression(ir, expr.target.object)
      const fieldName = expr.target.property

      // Look up field offset from structDefs
      const objType = this.variableTypes.get(expr.target.object?.name)
      if (objType instanceof StructType) {
        const fields = this.structDefs.get(objType.name)
        if (fields) {
          const fieldIndex = fields.findIndex((f: any) => f.name === fieldName)
          if (fieldIndex >= 0) {
            const offset = fieldIndex * 8
            if (offset === 0) {
              ir.push(`  storel ${val}, ${structPtr}`)
            } else {
              const off = this.nextReg()
              ir.push(`  ${off} =l add ${structPtr}, ${offset}`)
              ir.push(`  storel ${val}, ${off}`)
            }
            return val
          }
        }
      }
      return val
    }

    // Case B & C: Simple or compound assignment to a named variable
    const name = expr.name || expr.target?.name
    if (!name) return "0"

    const slot = this.variables.get(name)
    if (!slot) return "0"

    if (expr.operator === "=" || !expr.operator) {
      // Case B: Simple reassignment (operator may be "=" or absent)
      const val = this.generateExpression(ir, expr.value)
      const vt = this.variableTypes.get(name)
      if (vt instanceof FloatType) {
        ir.push(`  stored ${val}, ${slot}`)
      } else {
        ir.push(`  storel ${val}, ${slot}`)
      }
      return val
    }

    // Case C: Compound assignment (+=, -=, *=, /=, %=)
    const vt = this.variableTypes.get(name)
    const isFloat = vt instanceof FloatType
    const cur = this.nextReg()
    if (isFloat) {
      ir.push(`  ${cur} =d loadd ${slot}`)
    } else {
      ir.push(`  ${cur} =l loadl ${slot}`)
    }
    const rhs = this.generateExpression(ir, expr.value)
    const result = this.nextReg()

    const baseOp = expr.operator.slice(0, -1) // remove trailing '='
    if (isFloat) {
      const opMap: Record<string, string> = { "+": "add", "-": "sub", "*": "mul", "/": "div" }
      const qbeOp = opMap[baseOp] || "add"
      ir.push(`  ${result} =d ${qbeOp} ${cur}, ${rhs}`)
      ir.push(`  stored ${result}, ${slot}`)
    } else {
      const opMap: Record<string, string> = { "+": "add", "-": "sub", "*": "mul", "/": "div", "%": "rem" }
      const qbeOp = opMap[baseOp] || "add"
      ir.push(`  ${result} =l ${qbeOp} ${cur}, ${rhs}`)
      ir.push(`  storel ${result}, ${slot}`)
    }
    return result
  }

  // --- Control Flow (qbe-e91) ---

  private generateConditional(ir: string[], stmt: any): void {
    const lid = this.labelCounter++
    const thenLabel = `@then.${lid}`
    const elseLabel = `@else.${lid}`
    const endLabel = `@endif.${lid}`

    // Generate condition and convert to w for jnz
    const cond = this.generateExpression(ir, stmt.condition)
    const condW = this.nextReg()
    ir.push(`  ${condW} =w cnel ${cond}, 0`)
    ir.push(`  jnz ${condW}, ${thenLabel}, ${elseLabel}`)

    // Then block
    ir.push(`${thenLabel}`)
    this.blockTerminated = false
    const trueBranch = stmt.trueBranch || stmt.consequent
    if (trueBranch?.body) {
      for (const s of trueBranch.body) {
        this.generateStatement(ir, s)
      }
    } else if (trueBranch?.statements) {
      for (const s of trueBranch.statements) {
        this.generateStatement(ir, s)
      }
    }
    if (!this.blockTerminated) {
      ir.push(`  jmp ${endLabel}`)
    }

    // Else block
    ir.push(`${elseLabel}`)
    this.blockTerminated = false
    const falseBranch = stmt.falseBranch || stmt.alternate
    if (falseBranch) {
      if (falseBranch.type === "Conditional") {
        // else-if chain: recurse
        this.generateConditional(ir, falseBranch)
      } else if (falseBranch.body) {
        for (const s of falseBranch.body) {
          this.generateStatement(ir, s)
        }
      } else if (falseBranch.statements) {
        for (const s of falseBranch.statements) {
          this.generateStatement(ir, s)
        }
      }
    }
    if (!this.blockTerminated) {
      ir.push(`  jmp ${endLabel}`)
    }

    // End label
    ir.push(`${endLabel}`)
    this.blockTerminated = false
  }

  private generateWhileLoop(ir: string[], stmt: any): void {
    const lid = this.labelCounter++
    const condLabel = `@while.cond.${lid}`
    const bodyLabel = `@while.body.${lid}`
    const endLabel = `@while.end.${lid}`
    const backLabel = `@while.back.${lid}`

    // Save and set loop labels for break/continue
    this.loopStack.push({ breakLabel: endLabel, continueLabel: condLabel })

    // Jump to condition check
    ir.push(`  jmp ${condLabel}`)

    // Condition block
    ir.push(`${condLabel}`)
    const cond = this.generateExpression(ir, stmt.test || stmt.condition)
    const condW = this.nextReg()
    ir.push(`  ${condW} =w cnel ${cond}, 0`)
    ir.push(`  jnz ${condW}, ${bodyLabel}, ${endLabel}`)

    // Body block
    ir.push(`${bodyLabel}`)
    this.blockTerminated = false
    const whileBody = stmt.body?.body || stmt.body?.statements
    if (whileBody) {
      for (const s of whileBody) {
        this.generateStatement(ir, s)
      }
    }

    // Interrupt check before back-edge
    if (!this.blockTerminated) {
      this.emitInterruptCheck(ir, condLabel, endLabel, backLabel)
    }

    // End block
    ir.push(`${endLabel}`)
    this.blockTerminated = false

    // Restore loop labels
    this.loopStack.pop()
  }

  private generateForLoop(ir: string[], stmt: any): void {
    const lid = this.labelCounter++
    const condLabel = `@for.cond.${lid}`
    const bodyLabel = `@for.body.${lid}`
    const stepLabel = `@for.step.${lid}`
    const endLabel = `@for.end.${lid}`
    const backLabel = `@for.back.${lid}`

    // Allocate counter variable
    const counterSlot = this.nextReg()
    this.emitAlloc(`  ${counterSlot} =l alloc8 8`)
    const varName = stmt.variable?.name || stmt.variable
    this.variables.set(varName, counterSlot)

    // Generate start value and store
    const startVal = this.generateExpression(ir, stmt.start)
    ir.push(`  storel ${startVal}, ${counterSlot}`)

    // Generate end value
    const endVal = this.generateExpression(ir, stmt.end)
    const endSlot = this.nextReg()
    this.emitAlloc(`  ${endSlot} =l alloc8 8`)
    ir.push(`  storel ${endVal}, ${endSlot}`)

    // Save and set loop labels
    this.loopStack.push({ breakLabel: endLabel, continueLabel: stepLabel })

    // Jump to condition
    ir.push(`  jmp ${condLabel}`)

    // Condition: counter < end
    ir.push(`${condLabel}`)
    const curVal = this.nextReg()
    ir.push(`  ${curVal} =l loadl ${counterSlot}`)
    const endCur = this.nextReg()
    ir.push(`  ${endCur} =l loadl ${endSlot}`)
    const cmp = this.nextReg()
    ir.push(`  ${cmp} =w csltl ${curVal}, ${endCur}`)
    ir.push(`  jnz ${cmp}, ${bodyLabel}, ${endLabel}`)

    // Body
    ir.push(`${bodyLabel}`)
    this.blockTerminated = false
    const forBody = stmt.body?.body || stmt.body?.statements
    if (forBody) {
      for (const s of forBody) {
        this.generateStatement(ir, s)
      }
    }
    if (!this.blockTerminated) {
      ir.push(`  jmp ${stepLabel}`)
    }

    // Step: increment counter
    ir.push(`${stepLabel}`)
    const stepCur = this.nextReg()
    ir.push(`  ${stepCur} =l loadl ${counterSlot}`)
    const stepVal = stmt.step ? this.generateExpression(ir, stmt.step) : "1"
    const nextVal = this.nextReg()
    ir.push(`  ${nextVal} =l add ${stepCur}, ${stepVal}`)
    ir.push(`  storel ${nextVal}, ${counterSlot}`)

    // Interrupt check before back-edge
    this.emitInterruptCheck(ir, condLabel, endLabel, backLabel)

    // End
    ir.push(`${endLabel}`)
    this.blockTerminated = false
    this.loopStack.pop()
  }

  private generateForEachLoop(ir: string[], stmt: any): void {
    const lid = this.labelCounter++
    const condLabel = `@foreach.cond.${lid}`
    const bodyLabel = `@foreach.body.${lid}`
    const stepLabel = `@foreach.step.${lid}`
    const endLabel = `@foreach.end.${lid}`
    const backLabel = `@foreach.back.${lid}`

    // Generate iterable (array pointer) - parser uses 'array' field
    const arr = this.generateExpression(ir, stmt.array || stmt.iterable)

    // Load length from offset 0
    const lenReg = this.nextReg()
    ir.push(`  ${lenReg} =l loadl ${arr}`)

    // Load data pointer from offset 16
    const dataPtrSlot = this.nextReg()
    ir.push(`  ${dataPtrSlot} =l add ${arr}, 16`)
    const dataPtr = this.nextReg()
    ir.push(`  ${dataPtr} =l loadl ${dataPtrSlot}`)

    // Allocate index counter
    const idxSlot = this.nextReg()
    this.emitAlloc(`  ${idxSlot} =l alloc8 8`)
    ir.push(`  storel 0, ${idxSlot}`)

    // Allocate variable slot for loop element
    const varName = stmt.variable?.name || stmt.variable
    const elemSlot = this.nextReg()
    this.emitAlloc(`  ${elemSlot} =l alloc8 8`)
    this.variables.set(varName, elemSlot)

    // Save and set loop labels
    this.loopStack.push({ breakLabel: endLabel, continueLabel: stepLabel })

    ir.push(`  jmp ${condLabel}`)

    // Condition: index < length
    ir.push(`${condLabel}`)
    const idx = this.nextReg()
    ir.push(`  ${idx} =l loadl ${idxSlot}`)
    const cmp = this.nextReg()
    ir.push(`  ${cmp} =w csltl ${idx}, ${lenReg}`)
    ir.push(`  jnz ${cmp}, ${bodyLabel}, ${endLabel}`)

    // Body: load element at data_ptr + index*8
    ir.push(`${bodyLabel}`)
    this.blockTerminated = false
    const offset = this.nextReg()
    ir.push(`  ${offset} =l mul ${idx}, 8`)
    const elemAddr = this.nextReg()
    ir.push(`  ${elemAddr} =l add ${dataPtr}, ${offset}`)
    const elemVal = this.nextReg()
    ir.push(`  ${elemVal} =l loadl ${elemAddr}`)
    ir.push(`  storel ${elemVal}, ${elemSlot}`)

    const feBody = stmt.body?.body || stmt.body?.statements
    if (feBody) {
      for (const s of feBody) {
        this.generateStatement(ir, s)
      }
    }
    if (!this.blockTerminated) {
      ir.push(`  jmp ${stepLabel}`)
    }

    // Step: increment index
    ir.push(`${stepLabel}`)
    const curIdx = this.nextReg()
    ir.push(`  ${curIdx} =l loadl ${idxSlot}`)
    const nextIdx = this.nextReg()
    ir.push(`  ${nextIdx} =l add ${curIdx}, 1`)
    ir.push(`  storel ${nextIdx}, ${idxSlot}`)

    // Interrupt check
    this.emitInterruptCheck(ir, condLabel, endLabel, backLabel)

    // End
    ir.push(`${endLabel}`)
    this.blockTerminated = false
    this.loopStack.pop()
  }

  private generateForInRange(ir: string[], stmt: any): void {
    // ForInRange is structurally identical to ForLoop with step=1
    this.generateForLoop(ir, { ...stmt, step: stmt.step || { type: "NumberLiteral", value: 1 } })
  }

  private generateForInIndex(ir: string[], stmt: any): void {
    const lid = this.labelCounter++
    const condLabel = `@forindex.cond.${lid}`
    const bodyLabel = `@forindex.body.${lid}`
    const stepLabel = `@forindex.step.${lid}`
    const endLabel = `@forindex.end.${lid}`
    const backLabel = `@forindex.back.${lid}`

    // Generate iterable (array pointer)
    const arr = this.generateExpression(ir, stmt.iterable)

    // Load length from offset 0
    const lenReg = this.nextReg()
    ir.push(`  ${lenReg} =l loadl ${arr}`)

    // Load data pointer from offset 16
    const dataPtrSlot = this.nextReg()
    ir.push(`  ${dataPtrSlot} =l add ${arr}, 16`)
    const dataPtr = this.nextReg()
    ir.push(`  ${dataPtr} =l loadl ${dataPtrSlot}`)

    // Allocate index variable
    const indexVarName = stmt.indexVariable?.name || stmt.indexVariable || stmt.indexVar?.name || stmt.indexVar
    const idxSlot = this.nextReg()
    this.emitAlloc(`  ${idxSlot} =l alloc8 8`)
    ir.push(`  storel 0, ${idxSlot}`)
    this.variables.set(indexVarName, idxSlot)

    // Allocate element variable
    const valVarName = stmt.valueVariable?.name || stmt.valueVariable || stmt.variable?.name || stmt.variable
    const elemSlot = this.nextReg()
    this.emitAlloc(`  ${elemSlot} =l alloc8 8`)
    this.variables.set(valVarName, elemSlot)

    // Save and set loop labels
    this.loopStack.push({ breakLabel: endLabel, continueLabel: stepLabel })

    ir.push(`  jmp ${condLabel}`)

    // Condition: index < length
    ir.push(`${condLabel}`)
    const idx = this.nextReg()
    ir.push(`  ${idx} =l loadl ${idxSlot}`)
    const cmp = this.nextReg()
    ir.push(`  ${cmp} =w csltl ${idx}, ${lenReg}`)
    ir.push(`  jnz ${cmp}, ${bodyLabel}, ${endLabel}`)

    // Body: load element
    ir.push(`${bodyLabel}`)
    this.blockTerminated = false
    const offset = this.nextReg()
    ir.push(`  ${offset} =l mul ${idx}, 8`)
    const elemAddr = this.nextReg()
    ir.push(`  ${elemAddr} =l add ${dataPtr}, ${offset}`)
    const elemVal = this.nextReg()
    ir.push(`  ${elemVal} =l loadl ${elemAddr}`)
    ir.push(`  storel ${elemVal}, ${elemSlot}`)

    const fiBody = stmt.body?.body || stmt.body?.statements
    if (fiBody) {
      for (const s of fiBody) {
        this.generateStatement(ir, s)
      }
    }
    if (!this.blockTerminated) {
      ir.push(`  jmp ${stepLabel}`)
    }

    // Step: increment index
    ir.push(`${stepLabel}`)
    const curIdx = this.nextReg()
    ir.push(`  ${curIdx} =l loadl ${idxSlot}`)
    const nextIdx = this.nextReg()
    ir.push(`  ${nextIdx} =l add ${curIdx}, 1`)
    ir.push(`  storel ${nextIdx}, ${idxSlot}`)

    // Interrupt check
    this.emitInterruptCheck(ir, condLabel, endLabel, backLabel)

    // End
    ir.push(`${endLabel}`)
    this.blockTerminated = false
    this.loopStack.pop()
  }

  private generateForInMap(ir: string[], stmt: any): void {
    const lid = this.labelCounter++
    const condLabel = `@formap.cond.${lid}`
    const bodyLabel = `@formap.body.${lid}`
    const stepLabel = `@formap.step.${lid}`
    const endLabel = `@formap.end.${lid}`
    const backLabel = `@formap.back.${lid}`

    // Generate map expression - parser may use 'iterable' or 'map'
    const mapVal = this.generateExpression(ir, stmt.iterable || stmt.map)

    // Get iterator: $map_iter_new(map) -> iterator ptr
    const iter = this.nextReg()
    ir.push(`  ${iter} =l call $map_iter_new(l ${mapVal})`)

    // Allocate key and value variable slots
    const keyVarName = stmt.indexVariable?.name || stmt.indexVariable || stmt.keyVar?.name || stmt.keyVar
    const fmValVarName = stmt.valueVariable?.name || stmt.valueVariable || stmt.valueVar?.name || stmt.valueVar
    const keySlot = this.nextReg()
    this.emitAlloc(`  ${keySlot} =l alloc8 8`)
    this.variables.set(keyVarName, keySlot)
    const valSlot = this.nextReg()
    this.emitAlloc(`  ${valSlot} =l alloc8 8`)
    this.variables.set(fmValVarName, valSlot)

    // Save and set loop labels
    this.loopStack.push({ breakLabel: endLabel, continueLabel: stepLabel })

    ir.push(`  jmp ${condLabel}`)

    // Condition: $map_iter_next(iter, &key, &value) returns 0 when done
    ir.push(`${condLabel}`)
    const hasNext = this.nextReg()
    ir.push(`  ${hasNext} =l call $map_iter_next(l ${iter}, l ${keySlot}, l ${valSlot})`)
    const hasNextW = this.nextReg()
    ir.push(`  ${hasNextW} =w cnel ${hasNext}, 0`)
    ir.push(`  jnz ${hasNextW}, ${bodyLabel}, ${endLabel}`)

    // Body
    ir.push(`${bodyLabel}`)
    this.blockTerminated = false
    const fmBody = stmt.body?.body || stmt.body?.statements
    if (fmBody) {
      for (const s of fmBody) {
        this.generateStatement(ir, s)
      }
    }
    if (!this.blockTerminated) {
      ir.push(`  jmp ${stepLabel}`)
    }

    // Step (just jump back to condition for maps)
    ir.push(`${stepLabel}`)
    this.emitInterruptCheck(ir, condLabel, endLabel, backLabel)

    // End
    ir.push(`${endLabel}`)
    this.blockTerminated = false
    this.loopStack.pop()
  }

  private generateBlock(ir: string[], stmt: any): void {
    const blockBody = stmt.body || stmt.statements
    if (blockBody) {
      for (const s of blockBody) {
        this.generateStatement(ir, s)
      }
    }
  }

  private generateBreak(ir: string[]): void {
    const top = this.loopStack[this.loopStack.length - 1]
    if (top) {
      ir.push(`  jmp ${top.breakLabel}`)
      this.blockTerminated = true
    }
  }

  private generateContinue(ir: string[]): void {
    const top = this.loopStack[this.loopStack.length - 1]
    if (top) {
      ir.push(`  jmp ${top.continueLabel}`)
      this.blockTerminated = true
    }
  }

  private emitInterruptCheck(ir: string[], backEdgeLabel: string, exitLabel: string, resumeLabel: string): void {
    const iflag = this.nextReg()
    ir.push(`  ${iflag} =l loadl $mog_interrupt_flag`)
    const iflagW = this.nextReg()
    ir.push(`  ${iflagW} =w cnel ${iflag}, 0`)
    ir.push(`  jnz ${iflagW}, ${exitLabel}, ${resumeLabel}`)
    ir.push(`${resumeLabel}`)
    ir.push(`  jmp ${backEdgeLabel}`)
  }

  // --- Functions (qbe-329) ---
  private generateFunctionDeclaration(ir: string[], stmt: any): void {
    // Determine function name: "main" is renamed to "program_user"
    const funcName = stmt.name === "main"
      ? "program_user"
      : (this.packagePrefix + stmt.name)

    // Save outer state
    const outerVars = this.variables
    const outerVarTypes = this.variableTypes
    const outerFunc = this.currentFunc
    const outerAllocs = this.entryAllocs
    const outerInFunction = this.inFunction
    const outerFunctionName = this.currentFunctionName
    const outerReturnType = this.currentReturnType
    const outerBlockTerminated = this.blockTerminated
    const outerRegCounter = this.regCounter
    const outerLabelCounter = this.labelCounter

    // Reset for new function scope
    this.variables = new Map()
    this.variableTypes = new Map()
    this.currentFunc = []
    this.entryAllocs = []
    this.inFunction = true
    this.currentFunctionName = funcName
    this.blockTerminated = false
    this.regCounter = 0
    this.labelCounter = 0

    // Determine return type
    const retQBE = stmt.returnType ? this.toQBEType(stmt.returnType) : "l"
    this.currentReturnType = retQBE

    // Build parameter list
    const params: { name: string; qbeType: string }[] = []
    for (const p of (stmt.params || [])) {
      const pt = p.paramType || p.typeAnnotation
      const qbeT = pt ? this.toQBEType(pt) : "l"
      params.push({ name: p.name, qbeType: qbeT === "d" || qbeT === "s" ? qbeT : "l" })
    }

    // Allocate stack slots for parameters and store them
    for (const p of params) {
      const slot = this.nextReg()
      this.emitAlloc(`  ${slot} =l alloc8 8`)
      this.variables.set(p.name, slot)
      // Track parameter types
      const paramDef = (stmt.params || []).find((pp: any) => pp.name === p.name)
      if (paramDef) {
        const pt = paramDef.paramType || paramDef.typeAnnotation
        if (pt) this.variableTypes.set(p.name, pt)
      }
      if (p.qbeType === "d") {
        this.currentFunc.push(`  stored %${p.name}, ${slot}`)
      } else {
        this.currentFunc.push(`  storel %${p.name}, ${slot}`)
      }
    }

    // Generate body statements
    const body = stmt.body
    if (body) {
      const stmts = body.body || body.statements || []
      for (const s of stmts) {
        this.generateStatement(this.currentFunc, s)
      }
    }

    // Build the function signature
    const exportPrefix = stmt.isPublic ? "export " : ""
    const retTypeSig = retQBE === "void" ? "" : ` ${retQBE}`
    const paramSig = params.map(p => `${p.qbeType} %${p.name}`).join(", ")

    // Assemble the function
    const funcLines: string[] = []
    funcLines.push(`${exportPrefix}function${retTypeSig} $${funcName}(${paramSig}) {`)
    funcLines.push(`@start`)

    // GC frame push
    funcLines.push(`  call $gc_push_frame()`)

    // Entry allocs (stack slots)
    for (const alloc of this.entryAllocs) {
      funcLines.push(alloc)
    }

    // Body
    for (const line of this.currentFunc) {
      funcLines.push(line)
    }

    // If body doesn't end with return, add default return
    if (!this.blockTerminated) {
      funcLines.push(`  call $gc_pop_frame()`)
      if (retQBE === "void") {
        funcLines.push(`  ret`)
      } else {
        funcLines.push(`  ret 0`)
      }
    }

    funcLines.push(`}`)
    funcLines.push("")

    // Append to ir (which is this.funcSection from the caller)
    for (const line of funcLines) {
      ir.push(line)
    }

    // Restore outer state
    this.variables = outerVars
    this.variableTypes = outerVarTypes
    this.currentFunc = outerFunc
    this.entryAllocs = outerAllocs
    this.inFunction = outerInFunction
    this.currentFunctionName = outerFunctionName
    this.currentReturnType = outerReturnType
    this.blockTerminated = outerBlockTerminated
    this.regCounter = outerRegCounter
    this.labelCounter = outerLabelCounter
  }
  private generateAsyncFunctionDeclaration(ir: string[], stmt: any): void {
    // Determine function name: "main" is renamed to "program_user"
    const funcName = stmt.name === "main"
      ? "program_user"
      : (this.packagePrefix + stmt.name)

    // Save outer state
    const outerVars = this.variables
    const outerVarTypes = this.variableTypes
    const outerFunc = this.currentFunc
    const outerAllocs = this.entryAllocs
    const outerInFunction = this.inFunction
    const outerFunctionName = this.currentFunctionName
    const outerReturnType = this.currentReturnType
    const outerBlockTerminated = this.blockTerminated
    const outerRegCounter = this.regCounter
    const outerLabelCounter = this.labelCounter
    const outerIsInAsync = this.isInAsyncFunction

    // Reset for new function scope
    this.variables = new Map()
    this.variableTypes = new Map()
    this.currentFunc = []
    this.entryAllocs = []
    this.inFunction = true
    this.isInAsyncFunction = true
    this.currentFunctionName = funcName
    this.blockTerminated = false
    this.regCounter = 0
    this.labelCounter = 0

    // Async functions always return a future pointer (l)
    this.currentReturnType = "l"

    // Build parameter list
    const params: { name: string; qbeType: string }[] = []
    for (const p of (stmt.params || [])) {
      const pt = p.paramType || p.typeAnnotation
      const qbeT = pt ? this.toQBEType(pt) : "l"
      params.push({ name: p.name, qbeType: qbeT === "d" || qbeT === "s" ? qbeT : "l" })
    }

    // Allocate a future at the start
    this.currentFunc.push(`  %future =l call $mog_future_new()`)

    // Allocate stack slots for parameters and store them
    for (const p of params) {
      const slot = this.nextReg()
      this.emitAlloc(`  ${slot} =l alloc8 8`)
      this.variables.set(p.name, slot)
      // Track parameter types
      const paramDef = (stmt.params || []).find((pp: any) => pp.name === p.name)
      if (paramDef) {
        const pt = paramDef.paramType || paramDef.typeAnnotation
        if (pt) this.variableTypes.set(p.name, pt)
      }
      if (p.qbeType === "d") {
        this.currentFunc.push(`  stored %${p.name}, ${slot}`)
      } else {
        this.currentFunc.push(`  storel %${p.name}, ${slot}`)
      }
    }

    // Generate body statements
    const body = stmt.body
    if (body) {
      const stmts = body.body || body.statements || []
      for (const s of stmts) {
        this.generateStatement(this.currentFunc, s)
      }
    }

    // Build the function signature — async functions always return l (future pointer)
    const exportPrefix = stmt.isPublic ? "export " : ""
    const paramSig = params.map(p => `${p.qbeType} %${p.name}`).join(", ")

    // Assemble the function
    const funcLines: string[] = []
    funcLines.push(`${exportPrefix}function l $${funcName}(${paramSig}) {`)
    funcLines.push(`@start`)

    // GC frame push
    funcLines.push(`  call $gc_push_frame()`)

    // Entry allocs (stack slots)
    for (const alloc of this.entryAllocs) {
      funcLines.push(alloc)
    }

    // Body
    for (const line of this.currentFunc) {
      funcLines.push(line)
    }

    // If body doesn't end with return, complete future with 0 and return it
    if (!this.blockTerminated) {
      funcLines.push(`  call $mog_future_complete(l %future, l 0)`)
      funcLines.push(`  call $gc_pop_frame()`)
      funcLines.push(`  ret %future`)
    }

    funcLines.push(`}`)
    funcLines.push("")

    // Append to ir
    for (const line of funcLines) {
      ir.push(line)
    }

    // Restore outer state
    this.variables = outerVars
    this.variableTypes = outerVarTypes
    this.currentFunc = outerFunc
    this.entryAllocs = outerAllocs
    this.inFunction = outerInFunction
    this.currentFunctionName = outerFunctionName
    this.currentReturnType = outerReturnType
    this.blockTerminated = outerBlockTerminated
    this.regCounter = outerRegCounter
    this.labelCounter = outerLabelCounter
    this.isInAsyncFunction = outerIsInAsync
  }

  private generateReturn(ir: string[], stmt: any): void {
    if (this.isInAsyncFunction) {
      // In async functions, complete the future and return the future pointer
      if (stmt.value) {
        const val = this.generateExpression(ir, stmt.value)
        ir.push(`  call $mog_future_complete(l %future, l ${val})`)
      } else {
        ir.push(`  call $mog_future_complete(l %future, l 0)`)
      }
      ir.push(`  call $gc_pop_frame()`)
      ir.push(`  ret %future`)
    } else {
      if (stmt.value) {
        const val = this.generateExpression(ir, stmt.value)
        ir.push(`  ret ${val}`)
      } else {
        ir.push(`  ret`)
      }
    }
    this.blockTerminated = true
  }

  // --- Data Structures (qbe-879, qbe-f78) ---
  private generateArrayLiteral(ir: string[], expr: any): string {
    const elements: any[] = expr.elements || []
    const count = elements.length

    // Allocate header: {i64 len, i64 cap, ptr data} = 24 bytes
    const hdr = this.nextReg()
    ir.push(`  ${hdr} =l call $gc_alloc(l 24)`)

    // Allocate data: count * 8 bytes (at least 8 to avoid zero-size alloc)
    const dataSize = Math.max(count * 8, 8)
    const data = this.nextReg()
    ir.push(`  ${data} =l call $gc_alloc(l ${dataSize})`)

    // Store len at offset 0
    ir.push(`  storel ${count}, ${hdr}`)

    // Store cap at offset 8
    const capPtr = this.nextReg()
    ir.push(`  ${capPtr} =l add ${hdr}, 8`)
    ir.push(`  storel ${count}, ${capPtr}`)

    // Store data pointer at offset 16
    const dataPtr = this.nextReg()
    ir.push(`  ${dataPtr} =l add ${hdr}, 16`)
    ir.push(`  storel ${data}, ${dataPtr}`)

    // Store each element into the data buffer
    for (let i = 0; i < count; i++) {
      const val = this.generateExpression(ir, elements[i])
      if (i === 0) {
        ir.push(`  storel ${val}, ${data}`)
      } else {
        const off = this.nextReg()
        ir.push(`  ${off} =l add ${data}, ${i * 8}`)
        ir.push(`  storel ${val}, ${off}`)
      }
    }

    return hdr
  }

  private generateArrayFill(ir: string[], expr: any): string {
    // [value; count] syntax
    const countReg = this.generateExpression(ir, expr.count)

    // Allocate header: {i64 len, i64 cap, ptr data} = 24 bytes
    const hdr = this.nextReg()
    ir.push(`  ${hdr} =l call $gc_alloc(l 24)`)

    // Allocate data: count * 8 bytes
    const dataSize = this.nextReg()
    ir.push(`  ${dataSize} =l mul ${countReg}, 8`)
    const data = this.nextReg()
    ir.push(`  ${data} =l call $gc_alloc(l ${dataSize})`)

    // Store len at offset 0
    ir.push(`  storel ${countReg}, ${hdr}`)

    // Store cap at offset 8
    const capPtr = this.nextReg()
    ir.push(`  ${capPtr} =l add ${hdr}, 8`)
    ir.push(`  storel ${countReg}, ${capPtr}`)

    // Store data pointer at offset 16
    const dataPtr = this.nextReg()
    ir.push(`  ${dataPtr} =l add ${hdr}, 16`)
    ir.push(`  storel ${data}, ${dataPtr}`)

    // Generate the fill value
    const val = this.generateExpression(ir, expr.value)

    // Loop to fill using a stack-allocated counter
    // Allocate counter in entry block
    const ctrAddr = this.nextReg()
    this.emitAlloc(`  ${ctrAddr} =l alloc8 8`)
    ir.push(`  storel 0, ${ctrAddr}`)

    const headLabel = this.nextLabel()
    const bodyLabel = this.nextLabel()
    const endLabel = this.nextLabel()

    ir.push(`  jmp ${headLabel}`)
    ir.push(`${headLabel}`)
    const ctr = this.nextReg()
    ir.push(`  ${ctr} =l loadl ${ctrAddr}`)
    const cmp = this.nextReg()
    ir.push(`  ${cmp} =w csltl ${ctr}, ${countReg}`)
    ir.push(`  jnz ${cmp}, ${bodyLabel}, ${endLabel}`)
    ir.push(`${bodyLabel}`)
    const off = this.nextReg()
    ir.push(`  ${off} =l mul ${ctr}, 8`)
    const ptr = this.nextReg()
    ir.push(`  ${ptr} =l add ${data}, ${off}`)
    ir.push(`  storel ${val}, ${ptr}`)
    const next = this.nextReg()
    ir.push(`  ${next} =l add ${ctr}, 1`)
    ir.push(`  storel ${next}, ${ctrAddr}`)
    ir.push(`  jmp ${headLabel}`)
    ir.push(`${endLabel}`)

    return hdr
  }

  private generateMapLiteral(ir: string[], expr: any): string {
    const entries: any[] = expr.entries || []

    // Create a new map
    const mapReg = this.nextReg()
    ir.push(`  ${mapReg} =l call $map_new()`)

    // For each entry, generate key and value, then call map_set
    for (const entry of entries) {
      const key = this.generateExpression(ir, entry.key)
      const val = this.generateExpression(ir, entry.value)
      ir.push(`  call $map_set(l ${mapReg}, l ${key}, l ${val})`)
    }

    return mapReg
  }
  private generateStructLiteral(ir: string[], expr: any): string {
    const fields: { name: string; value: any }[] = expr.fields || []
    const structName = expr.structName || expr.name

    // Look up struct definition for field ordering
    const structDef = structName ? this.structDefs.get(structName) : null
    const fieldCount = structDef ? structDef.length : fields.length

    // Allocate space: fieldCount * 8 bytes
    const allocSize = Math.max(fieldCount * 8, 8)
    const structPtr = this.nextReg()
    ir.push(`  ${structPtr} =l call $gc_alloc(l ${allocSize})`)

    // Store each field at its offset
    if (structDef) {
      // Use struct definition ordering
      for (let i = 0; i < structDef.length; i++) {
        const defField = structDef[i]
        const litField = fields.find(f => f.name === defField.name)
        const val = litField
          ? this.generateExpression(ir, litField.value)
          : "0"
        if (i === 0) {
          ir.push(`  storel ${val}, ${structPtr}`)
        } else {
          const off = this.nextReg()
          ir.push(`  ${off} =l add ${structPtr}, ${i * 8}`)
          ir.push(`  storel ${val}, ${off}`)
        }
      }
    } else {
      // No struct definition, use literal order
      for (let i = 0; i < fields.length; i++) {
        const val = this.generateExpression(ir, fields[i].value)
        if (i === 0) {
          ir.push(`  storel ${val}, ${structPtr}`)
        } else {
          const off = this.nextReg()
          ir.push(`  ${off} =l add ${structPtr}, ${i * 8}`)
          ir.push(`  storel ${val}, ${off}`)
        }
      }
    }

    return structPtr
  }
  private generateSoAConstructor(ir: string[], expr: any): string {
    const structName = expr.name
    const fields = this.structDefs.get(structName)
    const fieldCount = fields ? fields.length : 0

    // Generate count expression
    const count = this.generateExpression(ir, expr.count)

    // Allocate header: (fieldCount + 1) * 8 bytes — stores count followed by field array pointers
    const headerSize = (fieldCount + 1) * 8
    const header = this.nextReg()
    ir.push(`  ${header} =l call $gc_alloc(l ${headerSize})`)

    // Store count at offset 0
    ir.push(`  storel ${count}, ${header}`)

    // For each field, allocate an array of (count * 8) bytes and store pointer in header
    for (let i = 0; i < fieldCount; i++) {
      // Compute data size: count * 8
      const dataSize = this.nextReg()
      ir.push(`  ${dataSize} =l mul ${count}, 8`)
      // Allocate the field array
      const fieldArr = this.nextReg()
      ir.push(`  ${fieldArr} =l call $gc_alloc(l ${dataSize})`)
      // Store field array pointer at offset (i + 1) * 8 in header
      const offset = (i + 1) * 8
      if (offset === 0) {
        ir.push(`  storel ${fieldArr}, ${header}`)
      } else {
        const ptr = this.nextReg()
        ir.push(`  ${ptr} =l add ${header}, ${offset}`)
        ir.push(`  storel ${fieldArr}, ${ptr}`)
      }
    }

    return header
  }

  // --- Member/Index (qbe-b0e) ---
  private generateMemberExpression(ir: string[], expr: any): string {
    const obj = this.generateExpression(ir, expr.object)
    const property = expr.property

    // Special case: .len for arrays — load from offset 0 of array header
    if (property === "len" || property === "length") {
      const objType = expr.object?.name ? this.variableTypes.get(expr.object.name) : null
      if (objType instanceof ArrayType || this.isArrayExpression(expr.object)) {
        const val = this.nextReg()
        ir.push(`  ${val} =l loadl ${obj}`)
        return val
      }
      // For strings, call string_len
      if (objType instanceof StringType || this.isStringProducingExpression(expr.object)) {
        const val = this.nextReg()
        ir.push(`  ${val} =l call $string_len(l ${obj})`)
        return val
      }
    }

    // Special case: .cap for arrays — load from offset 8
    if (property === "cap" || property === "capacity") {
      const objType = expr.object?.name ? this.variableTypes.get(expr.object.name) : null
      if (objType instanceof ArrayType || this.isArrayExpression(expr.object)) {
        const capPtr = this.nextReg()
        ir.push(`  ${capPtr} =l add ${obj}, 8`)
        const val = this.nextReg()
        ir.push(`  ${val} =l loadl ${capPtr}`)
        return val
      }
    }

    // Struct field access — look up field index from structDefs
    const objType = expr.object?.name ? this.variableTypes.get(expr.object.name) : null
    if (objType instanceof StructType) {
      const fields = this.structDefs.get(objType.name)
      if (fields) {
        const fieldIndex = fields.findIndex((f: any) => f.name === property)
        if (fieldIndex >= 0) {
          const offset = fieldIndex * 8
          let loadPtr = obj
          if (offset > 0) {
            loadPtr = this.nextReg()
            ir.push(`  ${loadPtr} =l add ${obj}, ${offset}`)
          }
          const fieldDef = fields[fieldIndex]
          const val = this.nextReg()
          if (fieldDef.fieldType instanceof FloatType) {
            ir.push(`  ${val} =d loadd ${loadPtr}`)
          } else {
            ir.push(`  ${val} =l loadl ${loadPtr}`)
          }
          return val
        }
      }
    }

    // Fallback: try to find struct type from the struct expression itself
    if (expr.object?.type === "StructLiteral") {
      const structName = expr.object.structName || expr.object.name
      if (structName) {
        const fields = this.structDefs.get(structName)
        if (fields) {
          const fieldIndex = fields.findIndex((f: any) => f.name === property)
          if (fieldIndex >= 0) {
            const offset = fieldIndex * 8
            let loadPtr = obj
            if (offset > 0) {
              loadPtr = this.nextReg()
              ir.push(`  ${loadPtr} =l add ${obj}, ${offset}`)
            }
            const val = this.nextReg()
            ir.push(`  ${val} =l loadl ${loadPtr}`)
            return val
          }
        }
      }
    }

    // Generic fallback: use property name string for runtime lookup
    // This handles cases where we don't have static type info
    const propStr = this.getOrCreateString(property)
    const val = this.nextReg()
    ir.push(`  ${val} =l call $struct_get_field(l ${obj}, l ${propStr})`)
    return val
  }
  private generateIndexExpression(ir: string[], expr: any): string {
    const obj = this.generateExpression(ir, expr.object)
    const idx = this.generateExpression(ir, expr.index)

    // Check if this is a map (string-keyed index)
    const objType = expr.object?.name ? this.variableTypes.get(expr.object.name) : null
    if (objType instanceof MapType || this.isMapExpression(expr.object) ||
        this.isStringProducingExpression(expr.index)) {
      // Map get: $map_get(map, key)
      const val = this.nextReg()
      ir.push(`  ${val} =l call $map_get(l ${obj}, l ${idx})`)
      return val
    }

    // Array index: load data ptr from offset 16, compute element ptr
    const dataPtr = this.nextReg()
    ir.push(`  ${dataPtr} =l add ${obj}, 16`)
    const data = this.nextReg()
    ir.push(`  ${data} =l loadl ${dataPtr}`)
    const off = this.nextReg()
    ir.push(`  ${off} =l mul ${idx}, 8`)
    const elemPtr = this.nextReg()
    ir.push(`  ${elemPtr} =l add ${data}, ${off}`)

    // Check element type for float arrays
    if (objType instanceof ArrayType && objType.elementType instanceof FloatType) {
      const val = this.nextReg()
      ir.push(`  ${val} =d loadd ${elemPtr}`)
      return val
    }

    const val = this.nextReg()
    ir.push(`  ${val} =l loadl ${elemPtr}`)
    return val
  }
  private generateSliceExpression(ir: string[], expr: any): string {
    // Generate the source array
    const obj = this.generateExpression(ir, expr.object)

    // Load length from offset 0
    const srcLen = this.nextReg()
    ir.push(`  ${srcLen} =l loadl ${obj}`)

    // Load data pointer from offset 16
    const dataPtrOff = this.nextReg()
    ir.push(`  ${dataPtrOff} =l add ${obj}, 16`)
    const srcData = this.nextReg()
    ir.push(`  ${srcData} =l loadl ${dataPtrOff}`)

    // Compute start (default 0)
    const start = expr.start
      ? this.generateExpression(ir, expr.start)
      : "0"

    // Compute end (default length)
    const end = expr.end
      ? this.generateExpression(ir, expr.end)
      : srcLen

    // Compute new length: end - start
    const newLen = this.nextReg()
    ir.push(`  ${newLen} =l sub ${end}, ${start}`)

    // Allocate new array header (24 bytes: len, cap, data ptr)
    const newHdr = this.nextReg()
    ir.push(`  ${newHdr} =l call $gc_alloc(l 24)`)

    // Allocate new data buffer: newLen * 8
    const newDataSize = this.nextReg()
    ir.push(`  ${newDataSize} =l mul ${newLen}, 8`)
    const newData = this.nextReg()
    ir.push(`  ${newData} =l call $gc_alloc(l ${newDataSize})`)

    // Store len at offset 0
    ir.push(`  storel ${newLen}, ${newHdr}`)

    // Store cap at offset 8
    const newCapPtr = this.nextReg()
    ir.push(`  ${newCapPtr} =l add ${newHdr}, 8`)
    ir.push(`  storel ${newLen}, ${newCapPtr}`)

    // Store data pointer at offset 16
    const newDataPtr = this.nextReg()
    ir.push(`  ${newDataPtr} =l add ${newHdr}, 16`)
    ir.push(`  storel ${newData}, ${newDataPtr}`)

    // Copy elements from start to end using a loop
    const idxSlot = this.nextReg()
    this.emitAlloc(`  ${idxSlot} =l alloc8 8`)
    ir.push(`  storel 0, ${idxSlot}`)

    const loopHead = this.nextLabel()
    const loopBody = this.nextLabel()
    const loopEnd = this.nextLabel()

    ir.push(`  jmp ${loopHead}`)

    // Loop header: check idx < newLen
    ir.push(`${loopHead}`)
    const idx = this.nextReg()
    ir.push(`  ${idx} =l loadl ${idxSlot}`)
    const cmp = this.nextReg()
    ir.push(`  ${cmp} =w csltl ${idx}, ${newLen}`)
    ir.push(`  jnz ${cmp}, ${loopBody}, ${loopEnd}`)

    // Loop body: copy one element
    ir.push(`${loopBody}`)
    // Source index = start + idx
    const srcIdx = this.nextReg()
    ir.push(`  ${srcIdx} =l add ${start}, ${idx}`)
    const srcOff = this.nextReg()
    ir.push(`  ${srcOff} =l mul ${srcIdx}, 8`)
    const srcElem = this.nextReg()
    ir.push(`  ${srcElem} =l add ${srcData}, ${srcOff}`)
    const val = this.nextReg()
    ir.push(`  ${val} =l loadl ${srcElem}`)

    // Destination offset = idx * 8
    const dstOff = this.nextReg()
    ir.push(`  ${dstOff} =l mul ${idx}, 8`)
    const dstElem = this.nextReg()
    ir.push(`  ${dstElem} =l add ${newData}, ${dstOff}`)
    ir.push(`  storel ${val}, ${dstElem}`)

    // Increment idx
    const nextIdx = this.nextReg()
    ir.push(`  ${nextIdx} =l add ${idx}, 1`)
    ir.push(`  storel ${nextIdx}, ${idxSlot}`)
    ir.push(`  jmp ${loopHead}`)

    ir.push(`${loopEnd}`)
    this.blockTerminated = false

    return newHdr
  }

  // --- Closures (qbe-896) ---
  private generateLambda(ir: string[], expr: any): string {
    const lambdaId = this.lambdaCounter++
    const lambdaName = `lambda.${lambdaId}`

    // Determine captures: variables referenced in the lambda body that exist in outer scope
    const captures: string[] = expr.captures || []
    // If no explicit captures, try to detect them from the body
    const detectedCaptures: string[] = []
    if (captures.length === 0) {
      const paramNames = new Set<string>((expr.params || []).map((p: any) => p.name))
      this.findCapturedVars(expr.body, paramNames, detectedCaptures)
    }
    const allCaptures = captures.length > 0 ? captures : detectedCaptures

    // Build lambda parameters: env_ptr as first param, then user params
    const params: { name: string; qbeType: string }[] = [
      { name: "env", qbeType: "l" }
    ]
    for (const p of (expr.params || [])) {
      const pt = p.paramType || p.typeAnnotation
      const qbeT = pt ? this.toQBEType(pt) : "l"
      params.push({ name: p.name, qbeType: qbeT === "d" || qbeT === "s" ? qbeT : "l" })
    }

    // Determine return type
    const retQBE = expr.returnType ? this.toQBEType(expr.returnType) : "l"

    // Save outer state
    const outerVars = this.variables
    const outerVarTypes = this.variableTypes
    const outerFunc = this.currentFunc
    const outerAllocs = this.entryAllocs
    const outerInFunction = this.inFunction
    const outerFunctionName = this.currentFunctionName
    const outerReturnType = this.currentReturnType
    const outerBlockTerminated = this.blockTerminated
    const outerRegCounter = this.regCounter
    const outerLabelCounter = this.labelCounter

    // Reset for lambda function scope
    this.variables = new Map()
    this.variableTypes = new Map()
    this.currentFunc = []
    this.entryAllocs = []
    this.inFunction = true
    this.currentFunctionName = lambdaName
    this.blockTerminated = false
    this.regCounter = 0
    this.labelCounter = 0
    this.currentReturnType = retQBE

    // Allocate stack slots for env and user params
    for (const p of params) {
      const slot = this.nextReg()
      this.emitAlloc(`  ${slot} =l alloc8 8`)
      this.variables.set(p.name, slot)
      if (p.qbeType === "d") {
        this.currentFunc.push(`  stored %${p.name}, ${slot}`)
      } else {
        this.currentFunc.push(`  storel %${p.name}, ${slot}`)
      }
    }

    // Restore captured variables from env pointer
    for (let i = 0; i < allCaptures.length; i++) {
      const capName = allCaptures[i]
      const slot = this.nextReg()
      this.emitAlloc(`  ${slot} =l alloc8 8`)
      this.variables.set(capName, slot)

      // Copy type info from outer scope
      const outerType = outerVarTypes.get(capName)
      if (outerType) this.variableTypes.set(capName, outerType)

      // Load from env: env_ptr + i*8
      const envSlot = this.variables.get("env")!
      const envPtr = this.nextReg()
      this.currentFunc.push(`  ${envPtr} =l loadl ${envSlot}`)
      if (i === 0) {
        const capVal = this.nextReg()
        this.currentFunc.push(`  ${capVal} =l loadl ${envPtr}`)
        this.currentFunc.push(`  storel ${capVal}, ${slot}`)
      } else {
        const capOff = this.nextReg()
        this.currentFunc.push(`  ${capOff} =l add ${envPtr}, ${i * 8}`)
        const capVal = this.nextReg()
        this.currentFunc.push(`  ${capVal} =l loadl ${capOff}`)
        this.currentFunc.push(`  storel ${capVal}, ${slot}`)
      }
    }

    // Copy param type info
    for (const p of (expr.params || [])) {
      const pt = p.paramType || p.typeAnnotation
      if (pt) this.variableTypes.set(p.name, pt)
    }

    // Generate body
    const body = expr.body
    if (body) {
      if (body.body || body.statements) {
        // Block body
        const stmts = body.body || body.statements || []
        for (const s of stmts) {
          this.generateStatement(this.currentFunc, s)
        }
      } else {
        // Expression body — treat as a return
        const val = this.generateExpression(this.currentFunc, body)
        this.currentFunc.push(`  ret ${val}`)
        this.blockTerminated = true
      }
    }

    // Assemble the lambda function
    const paramSig = params.map(p => `${p.qbeType} %${p.name}`).join(", ")
    const retTypeSig = retQBE === "void" ? "" : ` ${retQBE}`
    const funcLines: string[] = []
    funcLines.push(`function${retTypeSig} $${lambdaName}(${paramSig}) {`)
    funcLines.push(`@start`)

    for (const alloc of this.entryAllocs) {
      funcLines.push(alloc)
    }
    for (const line of this.currentFunc) {
      funcLines.push(line)
    }
    if (!this.blockTerminated) {
      if (retQBE === "void") {
        funcLines.push(`  ret`)
      } else {
        funcLines.push(`  ret 0`)
      }
    }
    funcLines.push(`}`)
    funcLines.push("")

    // Save lambda function to lambdaFuncs array
    this.lambdaFuncs.push(...funcLines)

    // Restore outer state
    this.variables = outerVars
    this.variableTypes = outerVarTypes
    this.currentFunc = outerFunc
    this.entryAllocs = outerAllocs
    this.inFunction = outerInFunction
    this.currentFunctionName = outerFunctionName
    this.currentReturnType = outerReturnType
    this.blockTerminated = outerBlockTerminated
    this.regCounter = outerRegCounter
    this.labelCounter = outerLabelCounter

    // Now generate the closure pair in the outer function context
    if (allCaptures.length > 0) {
      // Allocate environment: captures.length * 8 bytes
      const envSize = allCaptures.length * 8
      const envReg = this.nextReg()
      ir.push(`  ${envReg} =l call $gc_alloc(l ${envSize})`)

      // Store captured values into environment
      for (let i = 0; i < allCaptures.length; i++) {
        const capName = allCaptures[i]
        const outerSlot = this.variables.get(capName)
        if (outerSlot) {
          const capVal = this.nextReg()
          ir.push(`  ${capVal} =l loadl ${outerSlot}`)
          if (i === 0) {
            ir.push(`  storel ${capVal}, ${envReg}`)
          } else {
            const off = this.nextReg()
            ir.push(`  ${off} =l add ${envReg}, ${i * 8}`)
            ir.push(`  storel ${capVal}, ${off}`)
          }
        }
      }

      // Create closure pair: {fn_ptr, env_ptr} = 16 bytes
      const closureReg = this.nextReg()
      ir.push(`  ${closureReg} =l call $gc_alloc(l 16)`)
      ir.push(`  storel $${lambdaName}, ${closureReg}`)
      const envOff = this.nextReg()
      ir.push(`  ${envOff} =l add ${closureReg}, 8`)
      ir.push(`  storel ${envReg}, ${envOff}`)

      return closureReg
    } else {
      // No captures: env is 0
      const closureReg = this.nextReg()
      ir.push(`  ${closureReg} =l call $gc_alloc(l 16)`)
      ir.push(`  storel $${lambdaName}, ${closureReg}`)
      const envOff = this.nextReg()
      ir.push(`  ${envOff} =l add ${closureReg}, 8`)
      ir.push(`  storel 0, ${envOff}`)

      return closureReg
    }
  }

  // Helper: find variables referenced in a lambda body that exist in outer scope
  private findCapturedVars(node: any, paramNames: Set<string>, captures: string[]): void {
    if (!node) return
    if (Array.isArray(node)) {
      for (const item of node) this.findCapturedVars(item, paramNames, captures)
      return
    }
    if (typeof node !== "object") return

    if (node.type === "Identifier" && typeof node.name === "string") {
      if (!paramNames.has(node.name) &&
          this.variables.has(node.name) &&
          !captures.includes(node.name)) {
        captures.push(node.name)
      }
    }

    for (const key of Object.keys(node)) {
      if (key === "type" || key === "position") continue
      const val = node[key]
      if (val && typeof val === "object") {
        this.findCapturedVars(val, paramNames, captures)
      }
    }
  }

  // --- Templates (qbe-dc2) ---
  private generateTemplateLiteral(ir: string[], expr: any): string {
    const parts: any[] = expr.parts
    if (!parts || parts.length === 0) {
      // Empty template literal: return empty string
      const name = this.getOrCreateString("")
      return name
    }

    let result: string | null = null

    for (const part of parts) {
      let partReg: string

      if (typeof part === "string") {
        // Raw string part from parser
        partReg = this.getOrCreateString(part)
      } else if (part.type === "StringLiteral") {
        // StringLiteral node
        partReg = this.getOrCreateString(part.value)
      } else {
        // Expression that needs evaluation and possibly type conversion
        const val = this.generateExpression(ir, part)
        const exprType = this.inferExpressionType(part)

        if (exprType === "string") {
          partReg = val
        } else if (exprType === "float") {
          const converted = this.nextReg()
          ir.push(`  ${converted} =l call $f64_to_string(d ${val})`)
          partReg = converted
        } else {
          // int (default)
          const converted = this.nextReg()
          ir.push(`  ${converted} =l call $i64_to_string(l ${val})`)
          partReg = converted
        }
      }

      if (result === null) {
        result = partReg
      } else {
        const cat = this.nextReg()
        ir.push(`  ${cat} =l call $string_concat(l ${result}, l ${partReg})`)
        result = cat
      }
    }

    return result!
  }

  // --- Match (qbe-b1b) ---
  private generateMatchExpression(ir: string[], expr: any): string {
    const mid = this.labelCounter++
    const endLabel = `@match.end.${mid}`

    // Result slot allocated in entry block
    const resultSlot = this.nextReg()
    this.emitAlloc(`  ${resultSlot} =l alloc8 8`)

    // Generate the match value once
    const matchVal = this.generateExpression(ir, expr.value)

    const arms: any[] = expr.arms || []
    const armLabels: string[] = []
    const bodyLabels: string[] = []

    // Pre-generate labels for each arm
    for (let i = 0; i < arms.length; i++) {
      armLabels.push(`@match.arm.${mid}.${i}`)
      bodyLabels.push(`@match.body.${mid}.${i}`)
    }
    const fallLabel = `@match.fall.${mid}`

    // Jump to first arm check
    ir.push(`  jmp ${armLabels[0] || fallLabel}`)

    for (let i = 0; i < arms.length; i++) {
      const arm = arms[i]
      const pat = arm.pattern
      const nextArm = i + 1 < arms.length ? armLabels[i + 1] : fallLabel

      // Arm check block
      ir.push(`${armLabels[i]}`)
      this.blockTerminated = false

      let condReg: string

      if (pat.type === "Identifier" && pat.name === "_") {
        // Wildcard - always matches
        condReg = "1"
      } else if (pat.type === "IntegerLiteral" || pat.type === "NumberLiteral") {
        const patVal = String(pat.value)
        const cmpW = this.nextReg()
        ir.push(`  ${cmpW} =w ceql ${matchVal}, ${patVal}`)
        condReg = this.nextReg()
        ir.push(`  ${condReg} =l extsw ${cmpW}`)
      } else if (pat.type === "StringLiteral") {
        const strName = this.getOrCreateString(pat.value)
        condReg = this.nextReg()
        ir.push(`  ${condReg} =l call $string_eq(l ${matchVal}, l ${strName})`)
      } else if (pat.type === "RangePattern") {
        const startVal = String(pat.start.value ?? pat.start)
        const endVal = String(pat.end.value ?? pat.end)
        const ge = this.nextReg()
        ir.push(`  ${ge} =w csgel ${matchVal}, ${startVal}`)
        const le = this.nextReg()
        ir.push(`  ${le} =w cslel ${matchVal}, ${endVal}`)
        const both = this.nextReg()
        ir.push(`  ${both} =w and ${ge}, ${le}`)
        condReg = this.nextReg()
        ir.push(`  ${condReg} =l extsw ${both}`)
      } else if (pat.type === "IsOk" || pat.type === "IsSome") {
        const tag = this.nextReg()
        ir.push(`  ${tag} =l loadl ${matchVal}`)
        const cmpW = this.nextReg()
        ir.push(`  ${cmpW} =w ceql ${tag}, 0`)
        condReg = this.nextReg()
        ir.push(`  ${condReg} =l extsw ${cmpW}`)
      } else if (pat.type === "IsErr" || pat.type === "IsNone") {
        const tag = this.nextReg()
        ir.push(`  ${tag} =l loadl ${matchVal}`)
        const cmpW = this.nextReg()
        ir.push(`  ${cmpW} =w ceql ${tag}, 1`)
        condReg = this.nextReg()
        ir.push(`  ${condReg} =l extsw ${cmpW}`)
      } else if (pat.type === "BooleanLiteral") {
        const patVal = pat.value ? "1" : "0"
        const cmpW = this.nextReg()
        ir.push(`  ${cmpW} =w ceql ${matchVal}, ${patVal}`)
        condReg = this.nextReg()
        ir.push(`  ${condReg} =l extsw ${cmpW}`)
      } else {
        condReg = "0"
      }

      // Optional guard
      if (arm.guard) {
        const guardVal = this.generateExpression(ir, arm.guard)
        const combined = this.nextReg()
        ir.push(`  ${combined} =l and ${condReg}, ${guardVal}`)
        condReg = combined
      }

      // Branch: if matched jump to body, else next arm
      if (condReg === "1") {
        ir.push(`  jmp ${bodyLabels[i]}`)
      } else {
        const condW = this.nextReg()
        ir.push(`  ${condW} =w cnel ${condReg}, 0`)
        ir.push(`  jnz ${condW}, ${bodyLabels[i]}, ${nextArm}`)
      }

      // Body block
      ir.push(`${bodyLabels[i]}`)
      this.blockTerminated = false

      // If pattern binds a variable (e.g., is Ok(x)), extract payload and bind
      if ((pat.type === "IsOk" || pat.type === "IsSome" || pat.type === "IsErr") && pat.binding) {
        const bindName = typeof pat.binding === "string" ? pat.binding : pat.binding.name
        if (bindName && bindName !== "_") {
          const payOff = this.nextReg()
          ir.push(`  ${payOff} =l add ${matchVal}, 8`)
          const payVal = this.nextReg()
          ir.push(`  ${payVal} =l loadl ${payOff}`)
          const slot = this.nextReg()
          this.emitAlloc(`  ${slot} =l alloc8 8`)
          ir.push(`  storel ${payVal}, ${slot}`)
          this.variables.set(bindName, slot)
        }
      }

      // Generate body expression
      let bodyVal: string
      const body = arm.body
      if (body && body.body && Array.isArray(body.body)) {
        const stmts = body.body
        bodyVal = "0"
        for (let j = 0; j < stmts.length; j++) {
          const s = stmts[j]
          if (j === stmts.length - 1 && s.type !== "VariableDeclaration" && s.type !== "ReturnStatement") {
            if (s.expression) {
              bodyVal = this.generateExpression(ir, s.expression)
            } else {
              bodyVal = this.generateExpression(ir, s)
            }
          } else {
            this.generateStatement(ir, s)
          }
        }
      } else {
        bodyVal = this.generateExpression(ir, body)
      }

      ir.push(`  storel ${bodyVal}, ${resultSlot}`)
      ir.push(`  jmp ${endLabel}`)
    }

    // Fallthrough: store 0
    ir.push(`${fallLabel}`)
    this.blockTerminated = false
    ir.push(`  storel 0, ${resultSlot}`)
    ir.push(`  jmp ${endLabel}`)

    // End block: load result
    ir.push(`${endLabel}`)
    this.blockTerminated = false
    const result = this.nextReg()
    ir.push(`  ${result} =l loadl ${resultSlot}`)
    return result
  }

  // --- Result/Optional (qbe-765) ---
  private generateOkExpression(ir: string[], expr: any): string {
    const val = this.generateExpression(ir, expr.value)
    const ptr = this.nextReg()
    ir.push(`  ${ptr} =l call $gc_alloc(l 16)`)
    ir.push(`  storel 0, ${ptr}`)  // tag = Ok (0)
    const payOff = this.nextReg()
    ir.push(`  ${payOff} =l add ${ptr}, 8`)
    ir.push(`  storel ${val}, ${payOff}`)  // payload
    return ptr
  }

  private generateErrExpression(ir: string[], expr: any): string {
    const val = this.generateExpression(ir, expr.value)
    const ptr = this.nextReg()
    ir.push(`  ${ptr} =l call $gc_alloc(l 16)`)
    ir.push(`  storel 1, ${ptr}`)  // tag = Err (1)
    const payOff = this.nextReg()
    ir.push(`  ${payOff} =l add ${ptr}, 8`)
    ir.push(`  storel ${val}, ${payOff}`)  // payload
    return ptr
  }

  private generateSomeExpression(ir: string[], expr: any): string {
    const val = this.generateExpression(ir, expr.value)
    const ptr = this.nextReg()
    ir.push(`  ${ptr} =l call $gc_alloc(l 16)`)
    ir.push(`  storel 0, ${ptr}`)  // tag = Some (0)
    const payOff = this.nextReg()
    ir.push(`  ${payOff} =l add ${ptr}, 8`)
    ir.push(`  storel ${val}, ${payOff}`)  // payload
    return ptr
  }

  private generateNoneExpression(ir: string[]): string {
    const ptr = this.nextReg()
    ir.push(`  ${ptr} =l call $gc_alloc(l 16)`)
    ir.push(`  storel 1, ${ptr}`)  // tag = None (1)
    const payOff = this.nextReg()
    ir.push(`  ${payOff} =l add ${ptr}, 8`)
    ir.push(`  storel 0, ${payOff}`)  // payload = 0
    return ptr
  }

  private generatePropagateExpression(ir: string[], expr: any): string {
    // ? operator: if Err/None, return early propagating; if Ok/Some, extract payload
    const val = this.generateExpression(ir, expr.value || expr.expression)
    const pid = this.labelCounter++
    const okLabel = `@prop.ok.${pid}`
    const errLabel = `@prop.err.${pid}`

    // Load tag
    const tag = this.nextReg()
    ir.push(`  ${tag} =l loadl ${val}`)
    const isErr = this.nextReg()
    ir.push(`  ${isErr} =w ceql ${tag}, 1`)
    ir.push(`  jnz ${isErr}, ${errLabel}, ${okLabel}`)

    // Error path: propagate by returning the tagged union as-is
    ir.push(`${errLabel}`)
    this.blockTerminated = false
    ir.push(`  ret ${val}`)

    // Ok path: extract payload
    ir.push(`${okLabel}`)
    this.blockTerminated = false
    const payOff = this.nextReg()
    ir.push(`  ${payOff} =l add ${val}, 8`)
    const payload = this.nextReg()
    ir.push(`  ${payload} =l loadl ${payOff}`)
    return payload
  }

  private generateIsSomeExpression(ir: string[], expr: any): string {
    const val = this.generateExpression(ir, expr.value || expr.expression)
    const tag = this.nextReg()
    ir.push(`  ${tag} =l loadl ${val}`)
    const cmpW = this.nextReg()
    ir.push(`  ${cmpW} =w ceql ${tag}, 0`)
    const result = this.nextReg()
    ir.push(`  ${result} =l extsw ${cmpW}`)
    return result
  }

  private generateIsNoneExpression(ir: string[], expr: any): string {
    const val = this.generateExpression(ir, expr.value || expr.expression)
    const tag = this.nextReg()
    ir.push(`  ${tag} =l loadl ${val}`)
    const cmpW = this.nextReg()
    ir.push(`  ${cmpW} =w ceql ${tag}, 1`)
    const result = this.nextReg()
    ir.push(`  ${result} =l extsw ${cmpW}`)
    return result
  }

  private generateIsOkExpression(ir: string[], expr: any): string {
    const val = this.generateExpression(ir, expr.value || expr.expression)
    const tag = this.nextReg()
    ir.push(`  ${tag} =l loadl ${val}`)
    const cmpW = this.nextReg()
    ir.push(`  ${cmpW} =w ceql ${tag}, 0`)
    const result = this.nextReg()
    ir.push(`  ${result} =l extsw ${cmpW}`)
    return result
  }

  private generateIsErrExpression(ir: string[], expr: any): string {
    const val = this.generateExpression(ir, expr.value || expr.expression)
    const tag = this.nextReg()
    ir.push(`  ${tag} =l loadl ${val}`)
    const cmpW = this.nextReg()
    ir.push(`  ${cmpW} =w ceql ${tag}, 1`)
    const result = this.nextReg()
    ir.push(`  ${result} =l extsw ${cmpW}`)
    return result
  }

  // --- Capabilities (qbe-d89) ---
  private generateCapabilityCall(ir: string[], capName: string, funcName: string, args: string[], expr: any): string {
    // Build args buffer: allocate n*8 bytes, store each arg at offset i*8
    const argCount = args.length
    const bufSize = Math.max(argCount * 8, 8)
    const argsBuf = this.nextReg()
    ir.push(`  ${argsBuf} =l call $gc_alloc(l ${bufSize})`)

    for (let i = 0; i < argCount; i++) {
      if (i === 0) {
        ir.push(`  storel ${args[i]}, ${argsBuf}`)
      } else {
        const off = this.nextReg()
        ir.push(`  ${off} =l add ${argsBuf}, ${i * 8}`)
        ir.push(`  storel ${args[i]}, ${off}`)
      }
    }

    // Create string constants for cap name and func name
    const capStr = this.getOrCreateString(capName)
    const funcStr = this.getOrCreateString(funcName)

    // Call the capability dispatcher
    const result = this.nextReg()
    ir.push(`  ${result} =l call $mog_cap_call_out(l ${capStr}, l ${funcStr}, l ${argsBuf}, l ${argCount})`)
    return result
  }

  // --- Tensor (qbe-945) ---
  private generateTensorConstruction(ir: string[], expr: any): string {
    // Generate the data expression (should be an array)
    const data = this.generateExpression(ir, expr.data)

    if (expr.shape) {
      // N-dimensional tensor: tensor_from_array(data, ndim, shape_ptr)
      const shape = this.generateExpression(ir, expr.shape)

      // Load ndim from shape array's length (offset 0)
      const ndim = this.nextReg()
      ir.push(`  ${ndim} =l loadl ${shape}`)

      // Load shape data pointer (offset 16)
      const shapeDpOff = this.nextReg()
      ir.push(`  ${shapeDpOff} =l add ${shape}, 16`)
      const shapePtr = this.nextReg()
      ir.push(`  ${shapePtr} =l loadl ${shapeDpOff}`)

      const result = this.nextReg()
      ir.push(`  ${result} =l call $tensor_from_array(l ${data}, l ${ndim}, l ${shapePtr})`)
      return result
    } else {
      // Simple 1D tensor: tensor_create(data_ptr, len)
      // Load data pointer from offset 16
      const dpOff = this.nextReg()
      ir.push(`  ${dpOff} =l add ${data}, 16`)
      const dataPtr = this.nextReg()
      ir.push(`  ${dataPtr} =l loadl ${dpOff}`)

      // Load length from offset 0
      const len = this.nextReg()
      ir.push(`  ${len} =l loadl ${data}`)

      const result = this.nextReg()
      ir.push(`  ${result} =l call $tensor_create(l ${dataPtr}, l ${len})`)
      return result
    }
  }

  // --- Cast (needs float support) ---
  private generateCastExpression(ir: string[], expr: any): string {
    const val = this.generateExpression(ir, expr.value || expr.expression)
    const targetType = expr.targetType || expr.typeAnnotation

    if (!targetType) return val

    const typeName = typeof targetType === "string" ? targetType : targetType.name || targetType.kind

    if (typeName === "float" || typeName === "f64" || typeName === "double") {
      // int -> float: swtof (signed word to float/double)
      const result = this.nextReg()
      ir.push(`  ${result} =d swtof ${val}`)
      return result
    } else if (typeName === "int" || typeName === "i64" || typeName === "i32") {
      // float -> int: dtosi (double to signed integer)
      const result = this.nextReg()
      ir.push(`  ${result} =l dtosi ${val}`)
      return result
    } else if (typeName === "string") {
      // int -> string via runtime
      const result = this.nextReg()
      ir.push(`  ${result} =l call $int_to_string(l ${val})`)
      return result
    } else if (typeName === "bool") {
      // any -> bool: compare != 0
      const cmpW = this.nextReg()
      ir.push(`  ${cmpW} =w cnel ${val}, 0`)
      const result = this.nextReg()
      ir.push(`  ${result} =l extsw ${cmpW}`)
      return result
    }

    // Unknown cast — pass through
    return val
  }

  // --- If Expression ---
  private generateIfExpression(ir: string[], expr: any): string {
    const lid = this.labelCounter++
    const thenLabel = `@ife.then.${lid}`
    const elseLabel = `@ife.else.${lid}`
    const endLabel = `@ife.end.${lid}`

    // Result slot
    const resultSlot = this.nextReg()
    this.emitAlloc(`  ${resultSlot} =l alloc8 8`)

    // Condition
    const cond = this.generateExpression(ir, expr.condition)
    const condW = this.nextReg()
    ir.push(`  ${condW} =w cnel ${cond}, 0`)
    ir.push(`  jnz ${condW}, ${thenLabel}, ${elseLabel}`)

    // Then branch
    ir.push(`${thenLabel}`)
    this.blockTerminated = false
    const thenVal = this.generateExpression(ir, expr.consequent)
    ir.push(`  storel ${thenVal}, ${resultSlot}`)
    ir.push(`  jmp ${endLabel}`)

    // Else branch
    ir.push(`${elseLabel}`)
    this.blockTerminated = false
    const elseVal = expr.alternate ? this.generateExpression(ir, expr.alternate) : "0"
    ir.push(`  storel ${elseVal}, ${resultSlot}`)
    ir.push(`  jmp ${endLabel}`)

    // End: load result
    ir.push(`${endLabel}`)
    this.blockTerminated = false
    const result = this.nextReg()
    ir.push(`  ${result} =l loadl ${resultSlot}`)
    return result
  }

  // --- Async (stubs for non-coroutine generation) ---
  private generateAwaitExpression(ir: string[], expr: any): string {
    // Generate the argument — should be an expression returning a future pointer
    const futurePtr = this.generateExpression(ir, expr.argument)
    // Wait for the future (passing 0 for coro_handle since we're not doing real suspension)
    ir.push(`  call $mog_await(l ${futurePtr}, l 0)`)
    // Get the result from the future
    const result = this.nextReg()
    ir.push(`  ${result} =l call $mog_future_get_result(l ${futurePtr})`)
    return result
  }

  private generateSpawnExpression(ir: string[], expr: any): string {
    // Generate the expression (typically a function call that returns a future)
    const futurePtr = this.generateExpression(ir, expr.expression)
    // Schedule the future on the event loop (fire-and-forget)
    ir.push(`  call $mog_loop_enqueue_ready(l 0, l ${futurePtr})`)
    return futurePtr
  }

  // --- Try/Catch ---
  private generateTryCatch(ir: string[], stmt: any): void {
    // For now, just generate the try body — no exception handling in QBE yet
    const tryBody = stmt.body || stmt.tryBlock
    if (tryBody) {
      const stmts = tryBody.body || tryBody.statements || (Array.isArray(tryBody) ? tryBody : [tryBody])
      for (const s of stmts) {
        this.generateStatement(ir, s)
      }
    }
  }

  // ============================================================
  // PRINT CALL GENERATION (shared utility, used by calls task)
  // ============================================================
  generatePrintCall(ir: string[], funcName: string, args: string[], expr: any): string {
    // Determine argument type and dispatch to appropriate runtime print function
    const argExpr = expr.args?.[0]
    const argReg = args[0] || "0"

    if (funcName === "print" || funcName === "println") {
      const suffix = funcName === "println" ? "ln" : ""
      // Determine type of argument
      if (argExpr) {
        if (this.isStringProducingExpression(argExpr)) {
          ir.push(`  call $print${suffix}_string(l ${argReg})`)
          return "0"
        }
        if (this.isFloatOperand(argExpr)) {
          ir.push(`  call $print${suffix}_f64(d ${argReg})`)
          return "0"
        }
        // Check variable type
        if (argExpr.type === "Identifier") {
          const vt = this.variableTypes.get(argExpr.name)
          if (vt instanceof UnsignedType) {
            ir.push(`  call $print${suffix}_u64(l ${argReg})`)
            return "0"
          }
        }
      }
      // Default: integer
      ir.push(`  call $print${suffix}_i64(l ${argReg})`)
      return "0"
    }

    // Specific print variants
    if (funcName === "print_string" || funcName === "println_string") {
      ir.push(`  call $${funcName}(l ${argReg})`)
    } else if (funcName === "print_i64" || funcName === "println_i64") {
      ir.push(`  call $${funcName}(l ${argReg})`)
    } else if (funcName === "print_f64" || funcName === "println_f64") {
      ir.push(`  call $${funcName}(d ${argReg})`)
    } else if (funcName === "print_u64" || funcName === "println_u64") {
      ir.push(`  call $${funcName}(l ${argReg})`)
    } else if (funcName === "print_buffer") {
      ir.push(`  call $print_buffer(l ${args[0]}, l ${args[1]})`)
    } else {
      ir.push(`  call $${funcName}(l ${argReg})`)
    }
    return "0"
  }

  // ============================================================
  // TOP-LEVEL GENERATE (qbe-1ff will flesh this out)
  // ============================================================
  generate(ast: ProgramNode): string {
    // Pre-registration passes
    this.preRegisterStructs(ast)
    this.preRegisterFunctions(ast)
    this.detectAsyncFunctions(ast)

    // Collect all string constants
    this.collectStringConstants(ast.statements)

    // Generate all function declarations first
    const allFuncs = this.findFunctionDeclarationsRecursive(ast.statements)
    for (const func of allFuncs) {
      if (func.type === "AsyncFunctionDeclaration" || func.isAsync) {
        this.generateAsyncFunctionDeclaration(this.funcSection, func)
      } else {
        this.generateFunctionDeclaration(this.funcSection, func)
      }
    }

    // Generate top-level statements as program() or program_user()
    const hasMain = allFuncs.some((f: any) => f.name === "main")
    const topLevelStmts = ast.statements.filter((s: any) =>
      s.type !== "FunctionDeclaration" &&
      s.type !== "AsyncFunctionDeclaration" &&
      s.type !== "StructDeclaration" &&
      s.type !== "PackageDeclaration" &&
      s.type !== "ImportDeclaration"
    )

    if (!hasMain && topLevelStmts.length > 0) {
      // Generate a $program function from top-level statements
      this.generateProgramFunction(topLevelStmts)
    }

    // Generate main entry point
    this.generateMainEntry(hasMain, topLevelStmts.length > 0)

    // Assemble the final output
    const output: string[] = []
    output.push("# Mog QBE IL")
    output.push("# Generated by Mog compiler")
    output.push("")

    // Data section (string constants)
    if (this.dataSection.length > 0) {
      output.push(...this.dataSection)
      output.push("")
    }

    // Lambda functions
    if (this.lambdaFuncs.length > 0) {
      output.push(...this.lambdaFuncs)
      output.push("")
    }

    // Function wrappers
    if (this.wrapperFuncs.length > 0) {
      output.push(...this.wrapperFuncs)
      output.push("")
    }

    // User functions
    if (this.funcSection.length > 0) {
      output.push(...this.funcSection)
      output.push("")
    }

    return output.join("\n")
  }

  // Generate the $program function for top-level code
  private generateProgramFunction(stmts: any[]): void {
    const ir = this.funcSection
    this.resetCounters()
    this.entryAllocs = []
    this.currentFunc = []
    this.inFunction = true
    this.currentFunctionName = "program"
    this.currentReturnType = "l"

    for (const stmt of stmts) {
      this.generateStatement(this.currentFunc, stmt)
    }

    // Emit the function
    ir.push(`export function l $program() {`)
    ir.push(`@start`)
    ir.push(`  call $gc_push_frame()`)
    // Entry allocs
    for (const alloc of this.entryAllocs) {
      ir.push(alloc)
    }
    // Body
    for (const line of this.currentFunc) {
      ir.push(line)
    }
    if (!this.blockTerminated) {
      ir.push(`  call $gc_pop_frame()`)
      ir.push(`  ret 0`)
    }
    ir.push(`}`)
    ir.push("")

    this.inFunction = false
    this.blockTerminated = false
  }

  // Generate the main() entry point
  private generateMainEntry(hasMain: boolean, hasTopLevel: boolean): void {
    const ir = this.funcSection

    if (hasMain) {
      if (this.hasAsyncFunctions && this.asyncFunctions.has("program_user")) {
        // Async main with event loop
        ir.push(`export function w $main() {`)
        ir.push(`@start`)
        ir.push(`  call $gc_init()`)
        if (this.capabilities.size > 0) {
          const vmReg = this.nextReg()
          ir.push(`  ${vmReg} =l call $mog_vm_new()`)
          // TODO: register capabilities
        }
        const loopReg = this.nextReg()
        ir.push(`  ${loopReg} =l call $mog_loop_new()`)
        const futureReg = this.nextReg()
        ir.push(`  ${futureReg} =l call $program_user()`)
        ir.push(`  call $mog_loop_run(l ${loopReg})`)
        const resultReg = this.nextReg()
        ir.push(`  ${resultReg} =l call $mog_future_get_result(l ${futureReg})`)
        ir.push(`  call $mog_loop_destroy(l ${loopReg})`)
        ir.push(`  call $gc_pop_frame()`)
        const retReg = this.nextReg()
        ir.push(`  ${retReg} =w copy 0`)
        ir.push(`  ret ${retReg}`)
        ir.push(`}`)
      } else {
        // Sync main
        ir.push(`export function w $main() {`)
        ir.push(`@start`)
        ir.push(`  call $gc_init()`)
        ir.push(`  call $gc_push_frame()`)
        if (this.capabilities.size > 0) {
          const vmReg = this.nextReg()
          ir.push(`  ${vmReg} =l call $mog_vm_new()`)
        }
        ir.push(`  call $program_user()`)
        ir.push(`  call $gc_pop_frame()`)
        const retReg = this.nextReg()
        ir.push(`  ${retReg} =w copy 0`)
        ir.push(`  ret ${retReg}`)
        ir.push(`}`)
      }
    } else if (hasTopLevel) {
      ir.push(`export function w $main() {`)
      ir.push(`@start`)
      ir.push(`  call $gc_init()`)
      ir.push(`  call $gc_push_frame()`)
      ir.push(`  call $program()`)
      ir.push(`  call $gc_pop_frame()`)
      const retReg = this.nextReg()
      ir.push(`  ${retReg} =w copy 0`)
      ir.push(`  ret ${retReg}`)
      ir.push(`}`)
    }
    ir.push("")
  }
}

// ============================================================
// EXPORTED API
// ============================================================

export function generateQBEIR(
  ast: any,
  moduleName?: string,
  capabilities?: string[],
  capabilityDecls?: Map<string, any>
): string {
  const gen = new QBECodeGen()
  if (capabilities) gen.setCapabilities(capabilities)
  if (capabilityDecls) gen.setCapabilityDecls(capabilityDecls)
  return gen.generate(ast)
}

export function generateModuleQBEIR(
  astOrPackages: any | Map<string, any>,
  packageNameOrCapabilities?: string | string[],
  capabilitiesOrCapDecls?: string[] | Map<string, any>,
  capabilityDecls?: Map<string, any>
): string {
  // --- Single-package form: generateModuleQBEIR(ast, packageName, capabilities?, capabilityDecls?) ---
  if (typeof packageNameOrCapabilities === "string") {
    const ast = astOrPackages
    const packageName = packageNameOrCapabilities
    const caps = capabilitiesOrCapDecls as string[] | undefined
    const capDecls = capabilityDecls

    const gen = new QBECodeGen()
    if (caps) gen.setCapabilities(caps)
    if (capDecls) gen.setCapabilityDecls(capDecls)

    // Set package prefix so all function names get mangled as pkgName__funcName
    gen.packagePrefix = `${packageName}__`

    // Extract imported package names from ImportDeclaration nodes in the AST
    for (const stmt of (ast.statements || [])) {
      if (stmt.type === "ImportDeclaration" && stmt.paths) {
        for (const path of stmt.paths) {
          gen.importedPackages.set(path, path)
        }
      }
    }

    return gen.generate(ast)
  }

  // --- Multi-package form: generateModuleQBEIR(packages, capabilities?, capabilityDecls?) ---
  const packages = astOrPackages as Map<string, any>
  const capabilities = packageNameOrCapabilities as string[] | undefined
  const capDecls = capabilitiesOrCapDecls as Map<string, any> | undefined

  // Merge all packages into a single AST, mangling non-main package names
  const mainAST = packages.get("main")
  if (!mainAST) throw new Error("No main package found")

  const mergedBody: any[] = []

  for (const [pkgName, pkgAST] of packages) {
    if (pkgName === "main") continue
    for (const stmt of pkgAST.statements) {
      if (stmt.type === "PackageDeclaration" || stmt.type === "ImportDeclaration") continue
      // Mangle names with package prefix
      if (stmt.type === "FunctionDeclaration" || stmt.type === "AsyncFunctionDeclaration") {
        mergedBody.push({ ...stmt, name: `${pkgName}__${stmt.name}` })
      } else if (stmt.type === "StructDeclaration") {
        mergedBody.push({ ...stmt, name: `${pkgName}__${stmt.name}` })
      } else {
        mergedBody.push(stmt)
      }
    }
  }

  mergedBody.push(...mainAST.statements)
  const mergedAST = { ...mainAST, statements: mergedBody }

  const gen = new QBECodeGen()
  if (capabilities) gen.setCapabilities(capabilities)
  if (capDecls) gen.setCapabilityDecls(capDecls)
  // Set imported packages for qualified access
  for (const [pkgName] of packages) {
    if (pkgName !== "main") {
      gen.importedPackages.set(pkgName, pkgName)
    }
  }
  return gen.generate(mergedAST)
}

export { QBECodeGen }
