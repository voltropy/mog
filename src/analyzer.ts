import type { Type } from "./types.js"
import {
  isIntegerType,
  isUnsignedType,
  isFloatType,
  isBoolType,
  isTypeAliasType,
  isFunctionType,
  isTensorType,
  isResultType,
  isOptionalType,
  resolveTypeAlias,
  isArrayType,
  isMapType,
  isTableType,
  isPointerType,
  isSOAType,
  isVoidType,
  isNumericType,
  sameType,
  compatibleTypes,
  getCommonType,
  canCoerceWithWidening,
  IntegerType,
  UnsignedType,
  FloatType,
  BoolType,
  TypeAliasType,
  ArrayType,
  MapType,
  PointerType,
  VoidType,
  StructType,
  SOAType,
  FunctionType,
  TensorType,
  ResultType,
  OptionalType,
  FutureType,
  isFutureType,
  isStringType,
  StringType,
  boolType,
} from "./types.js"

interface Position {
  line: number
  column: number
  index: number
}

interface ASTNode {
  type: string
  position: {
    start: Position
    end: Position
  }
}

interface ProgramNode extends ASTNode {
  type: "Program"
  statements: StatementNode[]
  scopeId: string
}

type StatementNode =
  | VariableDeclarationNode
  | AssignmentNode
  | AssignmentExpressionNode
  | ExpressionStatementNode
  | BlockNode
  | ReturnNode
  | ConditionalNode
  | FunctionDeclarationNode
  | AsyncFunctionDeclarationNode
  | WhileLoopNode
  | ForLoopNode
  | ForEachLoopNode
  | ForInRangeNode
  | ForInIndexNode
  | ForInMapNode
  | BreakNode
  | ContinueNode
  | StructDefinitionNode
  | StructDeclarationNode
  | RequiresDeclarationNode
  | OptionalDeclarationNode
  | TryCatchNode
  | WithBlockNode
  | PackageDeclarationNode
  | ImportDeclarationNode

interface PackageDeclarationNode extends ASTNode {
  type: "PackageDeclaration"
  name: string
}

interface ImportDeclarationNode extends ASTNode {
  type: "ImportDeclaration"
  paths: string[]
}

interface StructDefinitionNode extends ASTNode {
  type: "StructDefinition"
  name: string
  fields: { name: string; fieldType: Type }[]
  isPublic?: boolean
}

interface StructDeclarationNode extends ASTNode {
  type: "StructDeclaration"
  name: string
  fields: { name: string; fieldType: Type }[]
  isPublic?: boolean
}

interface VariableDeclarationNode extends ASTNode {
  type: "VariableDeclaration"
  name: string
  varType: Type | null
  value: ExpressionNode | null
}

interface AssignmentNode extends ASTNode {
  type: "Assignment"
  name: string
  value: ExpressionNode
}

interface ExpressionStatementNode extends ASTNode {
  type: "ExpressionStatement"
  expression: ExpressionNode
}

interface BlockNode extends ASTNode {
  type: "Block"
  statements: StatementNode[]
  scopeId: string
}

interface ReturnNode extends ASTNode {
  type: "Return"
  value: ExpressionNode | null
}

interface ConditionalNode extends ASTNode {
  type: "Conditional"
  condition: ExpressionNode
  trueBranch: BlockNode
  falseBranch: BlockNode | null
}

interface WhileLoopNode extends ASTNode {
  type: "WhileLoop"
  test: ExpressionNode
  body: BlockNode
}

interface ForLoopNode extends ASTNode {
  type: "ForLoop"
  variable: string
  start: ExpressionNode
  end: ExpressionNode
  body: BlockNode
}

interface ForEachLoopNode extends ASTNode {
  type: "ForEachLoop"
  variable: string
  varType: Type
  array: ExpressionNode
  body: BlockNode
}

interface ForInRangeNode extends ASTNode {
  type: "ForInRange"
  variable: string
  start: ExpressionNode
  end: ExpressionNode
  body: BlockNode
}

interface ForInIndexNode extends ASTNode {
  type: "ForInIndex"
  indexVariable: string
  valueVariable: string
  iterable: ExpressionNode
  body: BlockNode
}

interface ForInMapNode extends ASTNode {
  type: "ForInMap"
  keyVariable: string
  valueVariable: string
  map: ExpressionNode
  body: BlockNode
}

interface BreakNode extends ASTNode {
  type: "Break"
}

interface ContinueNode extends ASTNode {
  type: "Continue"
}

interface FunctionDeclarationNode extends ASTNode {
  type: "FunctionDeclaration"
  name: string
  params: FunctionParam[]
  returnType: Type
  body: BlockNode
  isPublic?: boolean
}

interface AsyncFunctionDeclarationNode extends ASTNode {
  type: "AsyncFunctionDeclaration"
  name: string
  params: FunctionParam[]
  returnType: Type
  body: BlockNode
  isAsync: true
  isPublic?: boolean
}

interface AwaitExpressionNode extends ASTNode {
  type: "AwaitExpression"
  argument: ExpressionNode
}

interface SpawnExpressionNode extends ASTNode {
  type: "SpawnExpression"
  argument: ExpressionNode
}

interface FunctionParam {
  name: string
  paramType: Type
  defaultValue?: ExpressionNode | null
}

type ExpressionNode =
  | IdentifierNode
  | NumberLiteralNode
  | StringLiteralNode
  | TemplateLiteralNode
  | ArrayLiteralNode
  | ArrayFillNode
  | MapLiteralNode
  | StructLiteralNode
  | SoALiteralNode
  | BinaryExpressionNode
  | UnaryExpressionNode
  | AssignmentExpressionNode
  | ConditionalExpressionNode
  | CallExpressionNode
  | MemberExpressionNode
  | IndexExpressionNode
  | SliceExpressionNode
  | LLMExpressionNode
  | LambdaNode
  | BlockExpressionNode
  | CastExpressionNode
  | IfExpressionNode
  | MatchExpressionNode
  | OkExpressionNode
  | ErrExpressionNode
  | SomeExpressionNode
  | NoneExpressionNode
  | PropagateExpressionNode
  | IsSomeExpressionNode
  | IsNoneExpressionNode
  | IsOkExpressionNode
  | IsErrExpressionNode
  | AwaitExpressionNode
  | SpawnExpressionNode

interface IdentifierNode extends ASTNode {
  type: "Identifier"
  name: string
}

interface NumberLiteralNode extends ASTNode {
  type: "NumberLiteral"
  value: string | number
  literalType: Type | null
}

interface StringLiteralNode extends ASTNode {
  type: "StringLiteral"
  value: string
}

interface TemplateLiteralNode extends ASTNode {
  type: "TemplateLiteral"
  parts: (string | ExpressionNode)[]
}

interface ArrayLiteralNode extends ASTNode {
  type: "ArrayLiteral"
  elements: ExpressionNode[]
}

interface ArrayFillNode extends ASTNode {
  type: "ArrayFill"
  value: ExpressionNode
  count: ExpressionNode
}

interface MapLiteralNode extends ASTNode {
  type: "MapLiteral"
  entries: { key: string; value: ExpressionNode }[]
}

interface StructLiteralNode extends ASTNode {
  type: "StructLiteral"
  structName: string | null
  fields: { name: string; value: ExpressionNode }[]
}

interface SoALiteralNode extends ASTNode {
  type: "SoALiteral"
  columns: { name: string; values: ExpressionNode[] }[]
}

interface BinaryExpressionNode extends ASTNode {
  type: "BinaryExpression"
  operator: string
  left: ExpressionNode
  right: ExpressionNode
}

interface UnaryExpressionNode extends ASTNode {
  type: "UnaryExpression"
  operator: string
  operand: ExpressionNode
}

interface AssignmentExpressionNode extends ASTNode {
  type: "AssignmentExpression"
  name?: string
  target?: ExpressionNode
  value: ExpressionNode
}

interface ConditionalExpressionNode extends ASTNode {
  type: "Conditional"
  condition: ExpressionNode
  trueBranch: ExpressionNode
  falseBranch: ExpressionNode
}

interface CallExpressionNode extends ASTNode {
  type: "CallExpression"
  callee: ExpressionNode
  arguments: ExpressionNode[]
}

interface MemberExpressionNode extends ASTNode {
  type: "MemberExpression"
  object: ExpressionNode
  property: string
}

interface IndexExpressionNode extends ASTNode {
  type: "IndexExpression"
  object: ExpressionNode
  index: ExpressionNode
}

interface SliceExpressionNode extends ASTNode {
  type: "SliceExpression"
  object: ExpressionNode
  start: ExpressionNode
  end: ExpressionNode
  step: ExpressionNode | null
}

interface LLMExpressionNode extends ASTNode {
  type: "LLMExpression"
  prompt: ExpressionNode
  modelSize: ExpressionNode
  reasoningEffort: ExpressionNode
  context: ExpressionNode
  returnType: Type
}

interface LambdaNode extends ASTNode {
  type: "Lambda"
  params: FunctionParam[]
  returnType: Type
  body: BlockNode
}

interface BlockExpressionNode extends ASTNode {
  type: "BlockExpression"
  block: BlockNode
}

interface CastExpressionNode extends ASTNode {
  type: "CastExpression"
  targetType: Type
  value: ExpressionNode
  sourceType?: Type
}

interface IfExpressionNode extends ASTNode {
  type: "IfExpression"
  condition: ExpressionNode
  trueBranch: BlockNode
  falseBranch: BlockNode | IfExpressionNode
}

interface MatchArm {
  pattern: MatchPattern
  body: ExpressionNode
}

type MatchPattern =
  | { type: "LiteralPattern"; value: ExpressionNode }
  | { type: "WildcardPattern" }
  | { type: "VariantPattern"; name: string; binding: string | null }

interface MatchExpressionNode extends ASTNode {
  type: "MatchExpression"
  subject: ExpressionNode
  arms: MatchArm[]
}

interface RequiresDeclarationNode extends ASTNode {
  type: "RequiresDeclaration"
  capabilities: string[]
}

interface OptionalDeclarationNode extends ASTNode {
  type: "OptionalDeclaration"
  capabilities: string[]
}

interface TryCatchNode extends ASTNode {
  type: "TryCatch"
  tryBody: BlockNode
  errorVar: string
  catchBody: BlockNode
}

interface WithBlockNode extends ASTNode {
  type: "WithBlock"
  context: ExpressionNode
  body: BlockNode
}

interface OkExpressionNode extends ASTNode {
  type: "OkExpression"
  value: ExpressionNode
}

interface ErrExpressionNode extends ASTNode {
  type: "ErrExpression"
  value: ExpressionNode
}

interface SomeExpressionNode extends ASTNode {
  type: "SomeExpression"
  value: ExpressionNode
}

interface NoneExpressionNode extends ASTNode {
  type: "NoneExpression"
}

interface PropagateExpressionNode extends ASTNode {
  type: "PropagateExpression"
  value: ExpressionNode
}

interface IsSomeExpressionNode extends ASTNode {
  type: "IsSomeExpression"
  value: ExpressionNode
  binding: string
}

interface IsNoneExpressionNode extends ASTNode {
  type: "IsNoneExpression"
  value: ExpressionNode
}

interface IsOkExpressionNode extends ASTNode {
  type: "IsOkExpression"
  value: ExpressionNode
  binding: string
}

interface IsErrExpressionNode extends ASTNode {
  type: "IsErrExpression"
  value: ExpressionNode
  binding: string
}

interface SemanticError {
  message: string
  position: {
    start: Position
    end: Position
  }
}

interface SymbolInfo {
  name: string
  symbolType: "variable" | "function" | "parameter" | "type"
  declaredType: Type | null
  inferredType: Type | null
  depth: number
}

class SymbolTable {
  private stack: Map<string, SymbolInfo>[] = []
  private depth: number = 0

  constructor() {
    this.pushScope()
  }

  pushScope(): void {
    this.stack.push(new Map())
    this.depth++
  }

  popScope(): void {
    if (this.stack.length > 1) {
      this.stack.pop()
      this.depth--
    }
  }

  declare(name: string, symbolType: "variable" | "function" | "parameter" | "type", declaredType: Type | null = null): void {
    const currentScope = this.stack[this.stack.length - 1]
    if (currentScope.has(name)) {
      return
    }
    currentScope.set(name, {
      name,
      symbolType,
      declaredType,
      inferredType: null,
      depth: this.depth,
    })
  }

  lookup(name: string): SymbolInfo | null {
    for (let i = this.stack.length - 1; i >= 0; i--) {
      const symbol = this.stack[i].get(name)
      if (symbol) {
        return symbol
      }
    }
    return null
  }

  setCurrentType(name: string, type: Type): void {
    for (let i = this.stack.length - 1; i >= 0; i--) {
      const symbol = this.stack[i].get(name)
      if (symbol) {
        symbol.inferredType = type
        return
      }
    }
  }

  getCurrentDepth(): number {
    return this.depth
  }
}

class SemanticAnalyzer {
  private symbolTable: SymbolTable
  private errors: SemanticError[] = []
  private warnings: SemanticError[] = []
  private currentFunction: string | null = null
  private loopDepth: number = 0
  // Capability tracking
  private requiredCapabilities: string[] = []
  private optionalCapabilities: string[] = []
  private capabilityDecls: Map<string, any> = new Map()
  // Closure capture analysis
  private currentCapturedVars: Set<string> | null = null
  private lambdaScopeDepth: number = 0
  // Async function tracking
  private inAsyncFunction: boolean = false

  constructor() {
    this.symbolTable = new SymbolTable()
  }

  setCapabilityDecls(decls: Map<string, any>): void {
    this.capabilityDecls = decls
  }

  analyze(program: ProgramNode): SemanticError[] {
    this.symbolTable = new SymbolTable()
    this.errors = []
    this.warnings = []
    this.requiredCapabilities = []
    this.optionalCapabilities = []
    this.declarePOSIXBuiltins()
    this.visitProgram(program)
    // Print warnings if any
    if (this.warnings.length > 0) {
      for (const warning of this.warnings) {
        console.log(`Warning: ${warning.message} at line ${warning.position.start.line}`)
      }
    }
    return this.errors
  }

  private declarePOSIXBuiltins(): void {
    const i64Type = new IntegerType("i64")
    const u64Type = new UnsignedType("u64")
    const f64Type = new FloatType("f64")
    const voidType = new VoidType()

    // POSIX filesystem functions
    const posixFunctions: Record<string, { params: { name: string; type: string }[]; returnType: Type }> = {
      open: { params: [{ name: "path", type: "i64" }, { name: "flags", type: "i64" }], returnType: i64Type },
      read: { params: [{ name: "fd", type: "i64" }, { name: "buf", type: "i64" }, { name: "count", type: "i64" }], returnType: i64Type },
      write: { params: [{ name: "fd", type: "i64" }, { name: "buf", type: "i64" }, { name: "count", type: "i64" }], returnType: i64Type },
      pread: { params: [{ name: "fd", type: "i64" }, { name: "buf", type: "i64" }, { name: "count", type: "i64" }, { name: "offset", type: "i64" }], returnType: i64Type },
      pwrite: { params: [{ name: "fd", type: "i64" }, { name: "buf", type: "i64" }, { name: "count", type: "i64" }, { name: "offset", type: "i64" }], returnType: i64Type },
      lseek: { params: [{ name: "fd", type: "i64" }, { name: "offset", type: "i64" }, { name: "whence", type: "i64" }], returnType: i64Type },
      close: { params: [{ name: "fd", type: "i64" }], returnType: i64Type },
      fsync: { params: [{ name: "fd", type: "i64" }], returnType: i64Type },
      fdatasync: { params: [{ name: "fd", type: "i64" }], returnType: i64Type },
      stat: { params: [{ name: "path", type: "i64" }, { name: "buf", type: "i64" }], returnType: i64Type },
      lstat: { params: [{ name: "path", type: "i64" }, { name: "buf", type: "i64" }], returnType: i64Type },
      fstat: { params: [{ name: "fd", type: "i64" }, { name: "buf", type: "i64" }], returnType: i64Type },
      access: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      faccessat: { params: [{ name: "dirfd", type: "i64" }, { name: "path", type: "i64" }, { name: "mode", type: "i64" }, { name: "flags", type: "i64" }], returnType: i64Type },
      utimes: { params: [{ name: "path", type: "i64" }, { name: "times", type: "i64" }], returnType: i64Type },
      futimes: { params: [{ name: "fd", type: "i64" }, { name: "times", type: "i64" }], returnType: i64Type },
      utimensat: { params: [{ name: "dirfd", type: "i64" }, { name: "path", type: "i64" }, { name: "times", type: "i64" }, { name: "flags", type: "i64" }], returnType: i64Type },
      chmod: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      fchmod: { params: [{ name: "fd", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      chown: { params: [{ name: "path", type: "i64" }, { name: "owner", type: "i64" }, { name: "group", type: "i64" }], returnType: i64Type },
      fchown: { params: [{ name: "fd", type: "i64" }, { name: "owner", type: "i64" }, { name: "group", type: "i64" }], returnType: i64Type },
      umask: { params: [{ name: "mask", type: "i64" }], returnType: i64Type },
      truncate: { params: [{ name: "path", type: "i64" }, { name: "length", type: "i64" }], returnType: i64Type },
      ftruncate: { params: [{ name: "fd", type: "i64" }, { name: "length", type: "i64" }], returnType: i64Type },
      link: { params: [{ name: "oldpath", type: "i64" }, { name: "newpath", type: "i64" }], returnType: i64Type },
      symlink: { params: [{ name: "target", type: "i64" }, { name: "linkpath", type: "i64" }], returnType: i64Type },
      readlink: { params: [{ name: "path", type: "i64" }, { name: "buf", type: "i64" }, { name: "bufsiz", type: "i64" }], returnType: i64Type },
      rename: { params: [{ name: "oldpath", type: "i64" }, { name: "newpath", type: "i64" }], returnType: i64Type },
      unlink: { params: [{ name: "path", type: "i64" }], returnType: i64Type },
      mkdir: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      rmdir: { params: [{ name: "path", type: "i64" }], returnType: i64Type },
      fcntl: { params: [{ name: "fd", type: "i64" }, { name: "cmd", type: "i64" }], returnType: i64Type },
      pathconf: { params: [{ name: "path", type: "i64" }, { name: "name", type: "i64" }], returnType: i64Type },
      fpathconf: { params: [{ name: "fd", type: "i64" }, { name: "name", type: "i64" }], returnType: i64Type },
      dup: { params: [{ name: "fd", type: "i64" }], returnType: i64Type },
      dup2: { params: [{ name: "fd", type: "i64" }, { name: "fd2", type: "i64" }], returnType: i64Type },
      creat: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      mkfifo: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      mknod: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }, { name: "dev", type: "i64" }], returnType: i64Type },
      opendir: { params: [{ name: "path", type: "i64" }], returnType: i64Type },
      readdir: { params: [{ name: "dirp", type: "i64" }], returnType: i64Type },
      closedir: { params: [{ name: "dirp", type: "i64" }], returnType: i64Type },
      rewinddir: { params: [{ name: "dirp", type: "i64" }], returnType: voidType },
      chdir: { params: [{ name: "path", type: "i64" }], returnType: i64Type },
      fchdir: { params: [{ name: "fd", type: "i64" }], returnType: i64Type },
      getcwd: { params: [{ name: "buf", type: "i64" }, { name: "size", type: "i64" }], returnType: i64Type },
    }

    for (const [name, func] of Object.entries(posixFunctions)) {
      this.symbolTable.declare(name, "function", func.returnType)
    }

    // Print/Output functions
    const printFunctions: Record<string, { params: { name: string; type: Type }[]; returnType: Type }> = {
      print: { params: [{ name: "value", type: i64Type }], returnType: voidType },
      print_i64: { params: [{ name: "value", type: i64Type }], returnType: voidType },
      print_u64: { params: [{ name: "value", type: u64Type }], returnType: voidType },
      print_f64: { params: [{ name: "value", type: f64Type }], returnType: voidType },
      print_string: { params: [{ name: "value", type: new StringType() }], returnType: voidType },
      println: { params: [], returnType: voidType },
      println_i64: { params: [{ name: "value", type: i64Type }], returnType: voidType },
      println_u64: { params: [{ name: "value", type: u64Type }], returnType: voidType },
      println_f64: { params: [{ name: "value", type: f64Type }], returnType: voidType },
      println_string: { params: [{ name: "value", type: new StringType() }], returnType: voidType },
    }

    for (const [name, func] of Object.entries(printFunctions)) {
      this.symbolTable.declare(name, "function", func.returnType)
    }

    // Input functions
    const inputFunctions: Record<string, { params: { name: string; type: Type }[]; returnType: Type }> = {
      input_i64: { params: [], returnType: i64Type },
      input_u64: { params: [], returnType: u64Type },
      input_f64: { params: [], returnType: f64Type },
      input_string: { params: [], returnType: i64Type },  // returns pointer
    }

    for (const [name, func] of Object.entries(inputFunctions)) {
      this.symbolTable.declare(name, "function", func.returnType)
    }

    // Table/Runtime functions
    const ptrType = new PointerType()

    const tableFunctions: Record<string, { params: { name: string; type: Type }[]; returnType: Type }> = {
      table_new: { params: [{ name: "capacity", type: i64Type }], returnType: ptrType },
      table_get: { params: [{ name: "table", type: ptrType }, { name: "key", type: i64Type }, { name: "key_len", type: i64Type }], returnType: i64Type },
      table_set: { params: [{ name: "table", type: ptrType }, { name: "key", type: i64Type }, { name: "key_len", type: i64Type }, { name: "value", type: i64Type }], returnType: voidType },
      map_has: { params: [{ name: "map", type: ptrType }, { name: "key", type: ptrType }, { name: "key_len", type: i64Type }], returnType: i64Type },
      map_size: { params: [{ name: "map", type: ptrType }], returnType: i64Type },
      map_key_at: { params: [{ name: "map", type: ptrType }, { name: "index", type: i64Type }], returnType: ptrType },
      map_value_at: { params: [{ name: "map", type: ptrType }, { name: "index", type: i64Type }], returnType: i64Type },
      gc_alloc: { params: [{ name: "size", type: i64Type }], returnType: ptrType },
    }

    for (const [name, func] of Object.entries(tableFunctions)) {
      this.symbolTable.declare(name, "function", func.returnType)
    }

    // String functions
    const stringFunctions: Record<string, { params: { name: string; type: Type }[]; returnType: Type }> = {
      string_length: { params: [{ name: "str", type: ptrType }], returnType: u64Type },
      string_concat: { params: [{ name: "a", type: ptrType }, { name: "b", type: ptrType }], returnType: ptrType },
      string_upper: { params: [{ name: "str", type: ptrType }], returnType: ptrType },
      string_lower: { params: [{ name: "str", type: ptrType }], returnType: ptrType },
      string_trim: { params: [{ name: "str", type: ptrType }], returnType: ptrType },
      string_split: { params: [{ name: "str", type: ptrType }, { name: "delim", type: ptrType }], returnType: ptrType },
      string_contains: { params: [{ name: "str", type: ptrType }, { name: "sub", type: ptrType }], returnType: boolType },
      string_starts_with: { params: [{ name: "str", type: ptrType }, { name: "prefix", type: ptrType }], returnType: boolType },
      string_ends_with: { params: [{ name: "str", type: ptrType }, { name: "suffix", type: ptrType }], returnType: boolType },
      string_replace: { params: [{ name: "str", type: ptrType }, { name: "old", type: ptrType }, { name: "new", type: ptrType }], returnType: ptrType },
      int_from_string: { params: [{ name: "str", type: ptrType }], returnType: ptrType },
      float_from_string: { params: [{ name: "str", type: ptrType }], returnType: ptrType },
      str: { params: [{ name: "value", type: i64Type }], returnType: new StringType() },
    }

    for (const [name, func] of Object.entries(stringFunctions)) {
      this.symbolTable.declare(name, "function", func.returnType)
    }

    // Terminal I/O and utility functions
    const termioFunctions: Record<string, { params: { name: string; type: Type }[]; returnType: Type }> = {
      stdin_poll: { params: [{ name: "timeout_ms", type: i64Type }], returnType: i64Type },
      stdin_read_line: { params: [], returnType: ptrType },
      time_ms: { params: [], returnType: i64Type },
      string_eq: { params: [{ name: "a", type: ptrType }, { name: "b", type: ptrType }], returnType: i64Type },
      flush_stdout: { params: [], returnType: voidType },
      parse_int: { params: [{ name: "s", type: ptrType }], returnType: i64Type },
      parse_float: { params: [{ name: "s", type: ptrType }], returnType: new FloatType("f64") },
      async_read_line: { params: [], returnType: new FutureType(ptrType) },
    }

    for (const [name, func] of Object.entries(termioFunctions)) {
      this.symbolTable.declare(name, "function", func.returnType)
    }

    // Math builtin functions (available without import)
    const mathFunctions: Record<string, { params: { name: string; type: Type }[]; returnType: Type }> = {
      // Single-arg: float -> float
      abs: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      sqrt: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      sin: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      cos: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      tan: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      asin: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      acos: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      exp: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      log: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      log2: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      floor: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      ceil: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      round: { params: [{ name: "x", type: f64Type }], returnType: f64Type },
      // Two-arg: (float, float) -> float
      pow: { params: [{ name: "x", type: f64Type }, { name: "n", type: f64Type }], returnType: f64Type },
      atan2: { params: [{ name: "y", type: f64Type }, { name: "x", type: f64Type }], returnType: f64Type },
      min: { params: [{ name: "a", type: f64Type }, { name: "b", type: f64Type }], returnType: f64Type },
      max: { params: [{ name: "a", type: f64Type }, { name: "b", type: f64Type }], returnType: f64Type },
    }

    for (const [name, func] of Object.entries(mathFunctions)) {
      this.symbolTable.declare(name, "function", func.returnType)
    }

    // Math constants (available without import)
    this.symbolTable.declare("PI", "variable", f64Type)
    this.symbolTable.declare("E", "variable", f64Type)

    // Tensor builtin functions
    this.symbolTable.declare("matmul", "function", new TensorType(new FloatType("f32")))
    this.symbolTable.declare("tensor_print", "function", voidType)

  }

  private emitError(message: string, position: { start: Position; end: Position }): void {
    this.errors.push({ message, position })
  }

  private emitWarning(message: string, position: { start: Position; end: Position }): void {
    this.warnings.push({ message, position })
  }

  private visitProgram(node: ProgramNode): void {
    const program = node as ProgramNode

    for (const stmt of program.statements) {
      this.visitStatement(stmt)
    }
  }

  private visitStatement(node: StatementNode): void {
    switch (node.type) {
      case "VariableDeclaration":
        this.visitVariableDeclaration(node as VariableDeclarationNode)
        break
      case "Assignment":
        this.visitAssignment(node as AssignmentNode)
        break
      case "AssignmentExpression":
        this.visitAssignment(node as any)
        break
      case "ExpressionStatement":
        this.visitExpressionStatement(node as ExpressionStatementNode)
        break
      case "Block":
        this.visitBlock(node as BlockNode)
        break
      case "Return":
        this.visitReturn(node as ReturnNode)
        break
      case "Conditional":
        this.visitConditional(node as ConditionalNode)
        break
      case "WhileLoop":
        this.visitWhileLoop(node as WhileLoopNode)
        break
      case "ForLoop":
        this.visitForLoop(node as ForLoopNode)
        break
      case "ForEachLoop":
        this.visitForEachLoop(node as ForEachLoopNode)
        break
      case "ForInRange":
        this.visitForInRange(node as any)
        break
      case "ForInIndex":
        this.visitForInIndex(node as any)
        break
      case "ForInMap":
        this.visitForInMap(node as any)
        break
      case "Break":
        if (this.loopDepth === 0) {
          this.emitError("break statement can only be used inside a loop", node.position)
        }
        break
      case "Continue":
        if (this.loopDepth === 0) {
          this.emitError("continue statement can only be used inside a loop", node.position)
        }
        break
      case "FunctionDeclaration":
        this.visitFunctionDeclaration(node as FunctionDeclarationNode)
        break

      case "StructDefinition":
        this.visitStructDefinition(node as StructDefinitionNode)
        break
      case "StructDeclaration":
        this.visitStructDeclaration(node as StructDeclarationNode)
        break
      case "RequiresDeclaration":
        this.visitRequiresDeclaration(node as any)
        break
      case "OptionalDeclaration":
        this.visitOptionalDeclaration(node as any)
        break
      case "TypeAliasDeclaration":
        this.visitTypeAliasDeclaration(node as any)
        break
      case "TryCatch":
        this.visitTryCatch(node as TryCatchNode)
        break
      case "AsyncFunctionDeclaration":
        this.visitAsyncFunctionDeclaration(node as AsyncFunctionDeclarationNode)
        break
      case "PackageDeclaration":
        // Package declarations are recorded by the module resolver, nothing to do here
        break
      case "ImportDeclaration":
        // Import declarations are resolved by the module resolver, nothing to do here
        break
    }
  }

  private visitRequiresDeclaration(node: any): void {
    for (const cap of node.capabilities) {
      this.requiredCapabilities.push(cap)
      // Register capability name in symbol table so it can be used as an object
      this.symbolTable.declare(cap, "variable", new PointerType())
    }
  }

  private visitTypeAliasDeclaration(node: any): void {
    const aliasType = node.aliasedType as TypeAliasType
    this.symbolTable.declare(node.name, "type", aliasType.aliasedType)
  }

  private visitOptionalDeclaration(node: any): void {
    for (const cap of node.capabilities) {
      this.optionalCapabilities.push(cap)
      this.symbolTable.declare(cap, "variable", new PointerType())
    }
  }

  private visitVariableDeclaration(node: VariableDeclarationNode): void {
    let declaredType = node.varType
    const valueType = node.value ? this.visitExpression(node.value) : null

    // Resolve CustomType to actual type from symbol table (for struct/SoA types)
    if (declaredType?.type === "CustomType") {
      const resolved = this.symbolTable.lookup((declaredType as any).name)
      if (resolved?.declaredType) {
        declaredType = resolved.declaredType
      }
    }

    // Resolve SOAType annotation from parser (has structName string, not resolved type)
    if (declaredType && (declaredType as any).type === "SOAType" && (declaredType as any).structName) {
      const resolved = this.symbolTable.lookup((declaredType as any).structName)
      if (resolved?.declaredType instanceof StructType) {
        declaredType = new SOAType(resolved.declaredType, (declaredType as any).capacity)
      }
    }

    if (valueType) {
      if (declaredType) {
        // Allow literal coercion (float widening, array literal element coercion)
        const isLiteral = node.value?.type === "NumberLiteral" || node.value?.type === "ArrayLiteral"
        // Allow SoA/Struct literal assignment (MapLiteral → SOAType/StructType, SoAConstructor → SOAType)
        const isSoAOrStructAssign = (declaredType.type === "SOAType" || declaredType.type === "StructType") && (valueType.type === "MapType" || valueType.type === "SOAType" || node.value?.type === "MapLiteral" || node.value?.type === "StructLiteral" || node.value?.type === "SoAConstructor")
        const typeCheck = isLiteral ? canCoerceWithWidening : compatibleTypes
        if (!isSoAOrStructAssign && !typeCheck(valueType, declaredType)) {
          this.emitError(
            `Type mismatch: cannot assign ${valueType.toString()} to ${declaredType.toString()}`,
            node.position,
          )
        }
        this.symbolTable.declare(node.name, "variable", declaredType)
        this.symbolTable.setCurrentType(node.name, declaredType)
      } else {
        this.symbolTable.declare(node.name, "variable", valueType)
        this.symbolTable.setCurrentType(node.name, valueType)
      }
    } else {
      if (declaredType) {
        this.symbolTable.declare(node.name, "variable", declaredType)
      } else {
        this.emitError(`Cannot infer type for variable '${node.name}'`, node.position)
      }
    }
  }

  private visitAssignment(node: AssignmentNode): void {
    let symbol = this.symbolTable.lookup(node.name)

    if (!symbol) {
      this.emitError(`Undefined variable '${node.name}'`, node.position)
      this.visitExpression(node.value)
      return
    }

    if (symbol.symbolType !== "variable") {
      this.emitError(`Cannot assign to ${symbol.symbolType} '${node.name}'`, node.position)
    }

    const valueType = this.visitExpression(node.value)

    if (valueType && symbol.declaredType) {
      if (!compatibleTypes(valueType, symbol.declaredType)) {
        this.emitError(
          `Type mismatch: cannot assign ${valueType.toString()} to ${symbol.declaredType.toString()}`,
          node.position,
        )
      }
    } else if (valueType && symbol.inferredType) {
      if (!compatibleTypes(valueType, symbol.inferredType)) {
        this.emitError(
          `Type mismatch: cannot assign ${valueType.toString()} to ${symbol.inferredType.toString()}`,
          node.position,
        )
      }
    }
  }

  private visitExpressionStatement(node: ExpressionStatementNode): void {
    this.visitExpression(node.expression)
  }

  private visitAssignmentExpression(node: AssignmentExpressionNode): Type | null {
    const valueType = this.visitExpression(node.value)

    if (node.name !== undefined) {
      let symbol = this.symbolTable.lookup(node.name)

      if (!symbol) {
        // Walrus-style assignment: auto-declare the variable with inferred type
        this.symbolTable.declare(node.name, "variable", valueType)
        if (valueType) {
          this.symbolTable.setCurrentType(node.name, valueType)
        }
        return valueType
      }

      if (symbol.symbolType !== "variable") {
        this.emitError(`Cannot assign to ${symbol.symbolType} '${node.name}'`, node.position)
        return null
      }

      if (valueType && symbol.declaredType) {
        if (!compatibleTypes(valueType, symbol.declaredType)) {
          this.emitError(
            `Type mismatch: cannot assign ${valueType.toString()} to ${symbol.declaredType.toString()}`,
            node.position,
          )
        }
      } else if (valueType && symbol.inferredType) {
        if (!compatibleTypes(valueType, symbol.inferredType)) {
          this.emitError(
            `Type mismatch: cannot assign ${valueType.toString()} to ${symbol.inferredType.toString()}`,
            node.position,
          )
        }
      }
    } else if (node.target !== undefined) {
      const target = node.target as any
      const objectValue = this.visitExpression(target.object)
      
      // Handle IndexExpression (arrays)
      if (target.type === "IndexExpression") {
        const indexValue = this.visitExpression(target.index)

        let targetType: Type | null = null
        if (objectValue && isArrayType(objectValue)) {
          targetType = objectValue.elementType
        }

        if (targetType && valueType && !sameType(targetType, valueType)) {
          this.emitError(
            `Type mismatch: cannot assign ${valueType.toString()} to array element of type ${targetType.toString()} (requires explicit cast)`,
            node.position,
          )
        }

        return targetType
      }
      
      // Handle MemberExpression (tables)
      if (node.target.type === "MemberExpression") {
        // For tables, we just need to validate the value type
        // The object should be a pointer (table)
        return valueType
      }
    }

    return valueType
  }

  private visitBlock(node: BlockNode): void {
    this.symbolTable.pushScope()

    for (const stmt of node.statements) {
      this.visitStatement(stmt)
    }

    this.symbolTable.popScope()
  }

  private visitReturn(node: ReturnNode): void {
    if (!this.currentFunction) {
      this.emitError(`Return statement outside function`, node.position)
    }

    if (node.value) {
      this.visitExpression(node.value)
    }
  }

  private visitConditional(node: ConditionalNode): void {
    const conditionType = this.visitExpression(node.condition)

    if (node.condition.type === "AssignmentExpression") {
      this.emitError(
        "Assignment (:=) cannot be used as a condition. Use == for comparison.",
        node.condition.position,
      )
    }

    if (conditionType) {
      if (!isIntegerType(conditionType) && !isUnsignedType(conditionType) && !isBoolType(conditionType)) {
        this.emitError(
          `Condition must be integer or unsigned type, got ${conditionType.toString()}`,
          node.condition.position,
        )
      }
    }

    // For is-patterns (IsSome, IsOk, etc.), declare the binding variable in the true branch scope
    const condNode = node.condition as any
    if (condNode.type === "IsSomeExpression" || condNode.type === "IsOkExpression") {
      this.symbolTable.pushScope()
      this.symbolTable.declare(condNode.binding, "variable", new IntegerType("i64"))
      for (const stmt of node.trueBranch.statements) {
        this.visitStatement(stmt)
      }
      this.symbolTable.popScope()
    } else if (condNode.type === "IsErrExpression") {
      this.symbolTable.pushScope()
      this.symbolTable.declare(condNode.binding, "variable", new ArrayType(new UnsignedType("u8"), []))
      for (const stmt of node.trueBranch.statements) {
        this.visitStatement(stmt)
      }
      this.symbolTable.popScope()
    } else {
      this.visitBlock(node.trueBranch)
    }

    if (node.falseBranch) {
      this.visitBlock(node.falseBranch)
    }
  }

  private visitWhileLoop(node: WhileLoopNode): void {
    const conditionType = this.visitExpression(node.test)

    if (node.test.type === "AssignmentExpression") {
      this.emitError(
        "Assignment (:=) cannot be used as a condition. Use == for comparison.",
        node.test.position,
      )
    }

    if (conditionType) {
      if (!isIntegerType(conditionType) && !isUnsignedType(conditionType)) {
        this.emitError(
          `While loop condition must be integer or unsigned type, got ${conditionType.toString()}`,
          node.test.position,
        )
      }
    }

    this.loopDepth++
    this.visitBlock(node.body)
    this.loopDepth--
  }

  private visitForLoop(node: ForLoopNode): void {
    const startType = this.visitExpression(node.start)
    const endType = this.visitExpression(node.end)

    if (startType && endType) {
      if (!isIntegerType(startType) || !isIntegerType(endType)) {
        this.emitError(`For loop bounds must be integer types`, node.position)
      }
    }

    this.symbolTable.pushScope()
    this.symbolTable.declare(node.variable, "variable", startType || new IntegerType("i64"))
    this.loopDepth++
    this.visitBlock(node.body)
    this.loopDepth--
    this.symbolTable.popScope()
  }

  private visitForEachLoop(node: ForEachLoopNode): void {
    const arrayType = this.visitExpression(node.array)

    if (arrayType && !isArrayType(arrayType)) {
      this.emitError(`For-each loop requires array type, got ${arrayType.toString()}`, node.position)
    }

    this.symbolTable.pushScope()
    // If varType is provided, use it; otherwise infer from array element type
    let elemType: Type = node.varType
    if (!elemType && arrayType && isArrayType(arrayType)) {
      elemType = (arrayType as ArrayType).elementType
    }
    if (!elemType) {
      elemType = new IntegerType("i64")
    }
    this.symbolTable.declare(node.variable, "variable", elemType)
    this.symbolTable.setCurrentType(node.variable, elemType)
    this.loopDepth++
    this.visitBlock(node.body)
    this.loopDepth--
    this.symbolTable.popScope()
  }

  private visitForInRange(node: any): void {
    const startType = this.visitExpression(node.start)
    const endType = this.visitExpression(node.end)

    if (startType && !isIntegerType(startType) && !isUnsignedType(startType)) {
      this.emitError(`For-in range start must be integer type, got ${startType.toString()}`, node.position)
    }
    if (endType && !isIntegerType(endType) && !isUnsignedType(endType)) {
      this.emitError(`For-in range end must be integer type, got ${endType.toString()}`, node.position)
    }

    this.symbolTable.pushScope()
    this.symbolTable.declare(node.variable, "variable", startType || new IntegerType("i64"))
    this.loopDepth++
    this.visitBlock(node.body)
    this.loopDepth--
    this.symbolTable.popScope()
  }

  private visitForInIndex(node: any): void {
    const iterableType = this.visitExpression(node.iterable)

    // If the iterable is a map, rewrite to ForInMap node for codegen
    if (iterableType && isMapType(iterableType)) {
      node.type = "ForInMap"
      node.keyVariable = node.indexVariable
      node.map = node.iterable

      this.symbolTable.pushScope()
      const keyType = (iterableType as MapType).keyType || new PointerType()
      const valueType = (iterableType as MapType).valueType || new IntegerType("i64")
      this.symbolTable.declare(node.keyVariable, "variable", keyType)
      this.symbolTable.declare(node.valueVariable, "variable", valueType)
      this.loopDepth++
      this.visitBlock(node.body)
      this.loopDepth--
      this.symbolTable.popScope()
      return
    }

    if (iterableType && !isArrayType(iterableType)) {
      this.emitError(`For-in-index requires array or map type, got ${iterableType.toString()}`, node.position)
    }

    this.symbolTable.pushScope()
    // Index variable is always i64
    this.symbolTable.declare(node.indexVariable, "variable", new IntegerType("i64"))
    // Value variable type is the array element type
    let elemType: Type = new IntegerType("i64")
    if (iterableType && isArrayType(iterableType)) {
      elemType = (iterableType as ArrayType).elementType
    }
    this.symbolTable.declare(node.valueVariable, "variable", elemType)
    this.symbolTable.setCurrentType(node.valueVariable, elemType)
    this.loopDepth++
    this.visitBlock(node.body)
    this.loopDepth--
    this.symbolTable.popScope()
  }

  private visitForInMap(node: any): void {
    const mapType = this.visitExpression(node.map)

    if (mapType && !isMapType(mapType)) {
      this.emitError(`For-in-map requires map type, got ${mapType.toString()}`, node.position)
    }

    this.symbolTable.pushScope()
    // Key and value types from the map type
    let keyType: Type = new ArrayType(new UnsignedType("u8"), [])  // default: string
    let valueType: Type = new IntegerType("i64")
    if (mapType && isMapType(mapType)) {
      keyType = (mapType as MapType).keyType
      valueType = (mapType as MapType).valueType
    }
    this.symbolTable.declare(node.keyVariable, "variable", keyType)
    this.symbolTable.declare(node.valueVariable, "variable", valueType)
    this.loopDepth++
    this.visitBlock(node.body)
    this.loopDepth--
    this.symbolTable.popScope()
  }

  private visitFunctionDeclaration(node: FunctionDeclarationNode): void {
    // Resolve CustomType return type to actual type
    let returnType = node.returnType
    if (returnType?.type === "CustomType") {
      const resolved = this.symbolTable.lookup((returnType as any).name)
      if (resolved?.declaredType) {
        returnType = resolved.declaredType
      }
    }

    // Build FunctionType for the function
    const paramTypes = node.params.map((p: any) => p.paramType)
    const funcType = new FunctionType(paramTypes, returnType)
    this.symbolTable.declare(node.name, "function", funcType)

    const prevFunction = this.currentFunction
    this.currentFunction = node.name

    this.symbolTable.pushScope()

    // Track if we've seen a default value (all subsequent params must also have defaults)
    let seenDefault = false
    for (const param of node.params) {
      let paramType = param.paramType
      // Resolve CustomType to actual type from symbol table (for struct/SoA types)
      if (paramType?.type === "CustomType") {
        const resolved = this.symbolTable.lookup((paramType as any).name)
        if (resolved?.declaredType) {
          paramType = resolved.declaredType
        }
      }

      // Validate default values
      if ((param as any).defaultValue) {
        seenDefault = true
        const defaultType = this.visitExpression((param as any).defaultValue)
        // Type check: default value should be compatible with param type
        if (defaultType && paramType && !compatibleTypes(defaultType, paramType) && !canCoerceWithWidening(defaultType, paramType)) {
          this.emitError(
            `Default value type ${defaultType.toString()} is not compatible with parameter type ${paramType.toString()}`,
            (param as any).defaultValue.position || node.position
          )
        }
      } else if (seenDefault) {
        this.emitError(
          `Parameter '${param.name}' must have a default value because it follows a parameter with a default value`,
          node.position
        )
      }

      this.symbolTable.declare(param.name, "parameter", paramType)
      this.symbolTable.setCurrentType(param.name, paramType)
    }

    this.visitBlock(node.body)

    this.symbolTable.popScope()

    this.currentFunction = prevFunction
  }

  private visitAsyncFunctionDeclaration(node: AsyncFunctionDeclarationNode): void {
    // Resolve CustomType return type to actual type
    let returnType = node.returnType
    if (returnType?.type === "CustomType") {
      const resolved = this.symbolTable.lookup((returnType as any).name)
      if (resolved?.declaredType) {
        returnType = resolved.declaredType
      }
    }

    // async fn wraps return type in Future<T>
    const futureReturnType = new FutureType(returnType)

    // Build FunctionType for the function (return type is Future<T>)
    const paramTypes = node.params.map((p: any) => p.paramType)
    const funcType = new FunctionType(paramTypes, futureReturnType)
    this.symbolTable.declare(node.name, "function", funcType)

    const prevFunction = this.currentFunction
    const prevAsync = this.inAsyncFunction
    this.currentFunction = node.name
    this.inAsyncFunction = true

    this.symbolTable.pushScope()

    // Track if we've seen a default value (all subsequent params must also have defaults)
    let seenDefault = false
    for (const param of node.params) {
      let paramType = param.paramType
      // Resolve CustomType to actual type from symbol table (for struct/SoA types)
      if (paramType?.type === "CustomType") {
        const resolved = this.symbolTable.lookup((paramType as any).name)
        if (resolved?.declaredType) {
          paramType = resolved.declaredType
        }
      }

      // Validate default values
      if ((param as any).defaultValue) {
        seenDefault = true
        const defaultType = this.visitExpression((param as any).defaultValue)
        if (defaultType && paramType && !compatibleTypes(defaultType, paramType) && !canCoerceWithWidening(defaultType, paramType)) {
          this.emitError(
            `Default value type ${defaultType.toString()} is not compatible with parameter type ${paramType.toString()}`,
            (param as any).defaultValue.position || node.position
          )
        }
      } else if (seenDefault) {
        this.emitError(
          `Parameter '${param.name}' must have a default value because it follows a parameter with a default value`,
          node.position
        )
      }

      this.symbolTable.declare(param.name, "parameter", paramType)
      this.symbolTable.setCurrentType(param.name, paramType)
    }

    this.visitBlock(node.body)

    this.symbolTable.popScope()

    this.currentFunction = prevFunction
    this.inAsyncFunction = prevAsync
  }

  private visitStructDefinition(node: StructDefinitionNode): void {
    // Validate that all field types are valid
    const fields = new Map<string, Type>()

    for (const field of node.fields) {
      // Check for void type fields
      if (field.fieldType instanceof VoidType) {
        this.emitError(
          `Struct field '${field.name}' cannot have void type`,
          node.position
        )
        continue
      }

      fields.set(field.name, field.fieldType)
    }

    // Create StructType and store in symbol table as a type definition
    const structType = new StructType(node.name, fields)
    this.symbolTable.declare(node.name, "type", structType)
    // @ts-ignore - resultType is set dynamically
    node.resultType = structType
  }

  private visitStructDeclaration(node: StructDeclarationNode): void {
    // Same implementation as visitStructDefinition
    const fields = new Map<string, Type>()

    for (const field of node.fields) {
      // Check for void type fields
      if (field.fieldType instanceof VoidType) {
        this.emitError(
          `Struct field '${field.name}' cannot have void type`,
          node.position
        )
        continue
      }

      fields.set(field.name, field.fieldType)
    }

    // Create StructType and store in symbol table as a type definition
    const structType = new StructType(node.name, fields)
    this.symbolTable.declare(node.name, "type", structType)
    // @ts-ignore - resultType is set dynamically
    node.resultType = structType
  }

  private visitSoALiteral(_node: any): Type | null {
    // SoA literals are no longer supported in the new design.
    // SoA containers are declared with: soa name: StructType[capacity]
    // and accessed with: name[i].field
    return null
  }



  private visitStructLiteral(node: StructLiteralNode): Type | null {
    // If structName is provided, look up the type definition
    if (node.structName) {
      const structTypeDef = this.symbolTable.lookup(node.structName)
      if (!structTypeDef) {
        this.emitError(`Undefined struct type '${node.structName}'`, node.position)
        return null
      }

      if (structTypeDef.symbolType !== "type") {
        this.emitError(`'${node.structName}' is not a struct type`, node.position)
        return null
      }

      if (!structTypeDef.declaredType || !(structTypeDef.declaredType instanceof StructType)) {
        this.emitError(`'${node.structName}' is not a struct type`, node.position)
        return null
      }

      const expectedStruct = structTypeDef.declaredType as StructType

      // Validate field types match expected struct definition
      for (const field of node.fields) {
        const fieldType = this.visitExpression(field.value)
        const expectedFieldType = expectedStruct.fields.get(field.name)

        if (expectedFieldType) {
          if (fieldType && !compatibleTypes(fieldType, expectedFieldType)) {
            this.emitError(
              `Struct field '${field.name}' type mismatch: cannot assign ${fieldType.toString()} to ${expectedFieldType.toString()}`,
              node.position
            )
          }
        } else {
          this.emitError(
            `Struct '${node.structName}' does not have field '${field.name}'`,
            node.position
          )
        }
      }

      // Check for missing fields
      for (const [fieldName, _] of expectedStruct.fields) {
        const hasField = node.fields.some((f) => f.name === fieldName)
        if (!hasField) {
          this.emitError(
            `Missing field '${fieldName}' in struct literal for '${node.structName}'`,
            node.position
          )
        }
      }

      return expectedStruct
    }

    // Anonymous struct literal - infer type from fields
    const fields = new Map<string, Type>()
    for (const field of node.fields) {
      const fieldType = this.visitExpression(field.value)
      if (fieldType) {
        fields.set(field.name, fieldType)
      }
    }

    // Create anonymous struct type
    return new StructType("<anonymous>", fields)
  }

  private visitExpression(node: ExpressionNode): Type | null {
    let result: Type | null = null
    switch (node.type) {
      case "Identifier":
        result = this.visitIdentifier(node as IdentifierNode)
        break
      case "NumberLiteral":
        result = this.visitNumberLiteral(node as NumberLiteralNode)
        break
      case "StringLiteral":
        result = this.visitStringLiteral(node as StringLiteralNode)
        break
      case "TemplateLiteral":
        result = this.visitTemplateLiteral(node as TemplateLiteralNode)
        break
      case "ArrayLiteral":
        result = this.visitArrayLiteral(node as ArrayLiteralNode)
        break
      case "ArrayFill":
        result = this.visitArrayFill(node as ArrayFillNode)
        break
      case "MapLiteral":
        result = this.visitMapLiteral(node as MapLiteralNode)
        break
      case "SoALiteral":
        result = this.visitSoALiteral(node as any)
        break
      case "StructLiteral":
        result = this.visitStructLiteral(node as StructLiteralNode)
        break
      case "BinaryExpression":
        result = this.visitBinaryExpression(node as BinaryExpressionNode)
        break
      case "UnaryExpression":
        result = this.visitUnaryExpression(node as UnaryExpressionNode)
        break
      case "CallExpression":
        result = this.visitCallExpression(node as CallExpressionNode)
        break
      case "MemberExpression":
        result = this.visitMemberExpression(node as MemberExpressionNode)
        break
      case "IndexExpression":
        result = this.visitIndexExpression(node as IndexExpressionNode)
        break
      case "SliceExpression":
        result = this.visitSliceExpression(node as SliceExpressionNode)
        break
      case "LLMExpression":
        result = this.visitLLMExpression(node as LLMExpressionNode)
        break
      case "Lambda":
        result = this.visitLambda(node as LambdaNode)
        break
      case "BlockExpression":
        result = this.visitBlockExpression(node as BlockExpressionNode)
        break
      case "CastExpression":
        result = this.visitCastExpression(node as CastExpressionNode)
        break
      case "AssignmentExpression":
        result = this.visitAssignmentExpression(node as AssignmentExpressionNode)
        break
      case "IfExpression":
        result = this.visitIfExpression(node as any)
        break
      case "MatchExpression":
        result = this.visitMatchExpression(node as any)
        break
      case "OkExpression":
        result = this.visitOkExpression(node as any)
        break
      case "ErrExpression":
        result = this.visitErrExpression(node as any)
        break
      case "SomeExpression":
        result = this.visitSomeExpression(node as any)
        break
      case "NoneExpression":
        result = new OptionalType(new IntegerType("i64"))
        break
      case "PropagateExpression":
        result = this.visitPropagateExpression(node as any)
        break
      case "IsSomeExpression":
        result = this.visitIsSomeExpression(node as any)
        break
      case "IsNoneExpression":
        result = this.visitIsNoneExpression(node as any)
        break
      case "IsOkExpression":
        result = this.visitIsOkExpression(node as any)
        break
      case "IsErrExpression":
        result = this.visitIsErrExpression(node as any)
        break
      case "AwaitExpression":
        result = this.visitAwaitExpression(node as AwaitExpressionNode)
        break
      case "SpawnExpression":
        result = this.visitSpawnExpression(node as SpawnExpressionNode)
        break
      case "TensorConstruction":
        result = this.visitTensorConstruction(node as any)
        break
      case "SoAConstructor":
        result = this.visitSoAConstructor(node as any)
        break
      case "BooleanLiteral":
        result = boolType
        break
      default:
        const unknown = node as { type: string; position: { start: Position; end: Position } }
        this.emitError(`Unknown expression type: ${unknown.type}`, unknown.position)
        return null
    }
    // Store the result type on the node for codegen
    ;(node as any).resultType = result
    return result
  }

  private visitIdentifier(node: IdentifierNode): Type | null {
    const symbol = this.symbolTable.lookup(node.name)

    if (!symbol) {
      this.emitError(`Undefined variable '${node.name}'`, node.position)
      return null
    }

    // Track captured variables for closure analysis
    if (this.currentCapturedVars && symbol &&
        (symbol.symbolType === "variable" || symbol.symbolType === "parameter") &&
        symbol.depth <= this.lambdaScopeDepth) {
      this.currentCapturedVars.add(node.name)
    }

    return symbol.declaredType ?? symbol.inferredType
  }

  private visitNumberLiteral(node: NumberLiteralNode): Type | null {
    if (node.literalType) {
      return node.literalType
    }

    const value = typeof node.value === "string" ? node.value.toLowerCase() : String(node.value)
    const isFloat = value.includes(".") || value.includes("e") || value.includes("E") || Number.isFinite(node.value) && !Number.isInteger(node.value)

    if (isFloat) {
      return new FloatType("f64")
    }

    return new IntegerType("i64")
  }

  private visitStringLiteral(node: StringLiteralNode): Type | null {
    return new StringType()
  }

  private visitTemplateLiteral(node: TemplateLiteralNode): Type | null {
    // Visit all expression parts to type-check them
    for (const part of node.parts) {
      if (typeof part !== "string") {
        this.visitExpression(part)
      }
    }
    // Template literals always return strings
    return new StringType()
  }

  private visitArrayLiteral(node: ArrayLiteralNode): Type | null {
    if (node.elements.length === 0) {
      this.emitError(`Cannot infer type for empty array literal`, node.position)
      return null
    }

    const elementTypes: Type[] = []

    for (const elem of node.elements) {
      const elemType = this.visitExpression(elem)
      if (elemType) {
        elementTypes.push(elemType)
      }
    }

    if (elementTypes.length === 0) {
      return null
    }

    let commonType = elementTypes[0]

    for (let i = 1; i < elementTypes.length; i++) {
      if (!sameType(commonType, elementTypes[i])) {
        if (sameType(elementTypes[i], commonType)) {
          commonType = commonType
        } else if (sameType(commonType, elementTypes[i])) {
          commonType = elementTypes[i]
        } else {
          this.emitError(
            `Array elements have incompatible types: ${commonType.toString()} and ${elementTypes[i].toString()}`,
            node.position,
          )
          return commonType
        }
      }
    }

    // Only include dimensions for the outermost array level
    // For nested arrays (arrays of arrays), don't include dimensions in the type
    // This allows "i64[][]" to match "[[i64[3]][3]]"
    let resultType: Type
    if (commonType instanceof ArrayType) {
      // For arrays of arrays, create without dimensions on outer level too
      // The inner array already has dimensions from its own literal
      resultType = new ArrayType(commonType, [node.elements.length])
    } else {
      resultType = new ArrayType(commonType, [node.elements.length])
    }
    return resultType
  }

  private visitArrayFill(node: ArrayFillNode): Type | null {
    // Check that count is an integer type
    const countType = this.visitExpression(node.count)
    if (countType && !isIntegerType(countType) && !isUnsignedType(countType)) {
      this.emitError(
        `Array fill count must be an integer type, got ${countType.toString()}`,
        node.count.position
      )
    }

    // Get the element type from the value
    const elementType = this.visitExpression(node.value)
    if (!elementType) {
      return null
    }

    // The count needs to be a compile-time constant for now
    // Return an array type with the element type but unknown dimensions
    return new ArrayType(elementType)
  }

  private visitMapLiteral(node: MapLiteralNode): Type | null {
    if (node.entries.length === 0) {
      return null
    }

    // Visit first entry to get the value type
    const firstValueType = this.visitExpression(node.entries[0].value)

    if (!firstValueType) {
      return null
    }

    // Check that all values have compatible types
    for (let i = 1; i < node.entries.length; i++) {
      const entryType = this.visitExpression(node.entries[i].value)
      if (entryType && !sameType(firstValueType, entryType)) {
        this.emitError(`Map entry '${node.entries[i].key}' has incompatible type`, node.position)
      }
    }

    return new MapType(new IntegerType("i32"), firstValueType)
  }

  private visitBinaryExpression(node: BinaryExpressionNode): Type | null {
    const leftType = this.visitExpression(node.left)
    const rightType = this.visitExpression(node.right)

    if (!leftType || !rightType) {
      return null
    }

    const operator = node.operator

    const arithmeticOperators = ["+", "-", "*", "/", "%", "&", "|", "^", "<<", ">>", "TIMES", "DIVIDE"]

    const comparisonOperators = ["==", "!=", "<", ">", "<=", ">="]

    const logicalOperators = ["&&", "||", "and", "or", "AND", "OR"]

    if (arithmeticOperators.includes(operator)) {
      // Handle tensor operations
      if (isTensorType(leftType) && isTensorType(rightType)) {
        // Tensor binary ops: +, -, * are elementwise, return TensorType
        return leftType
      }
      if (isTensorType(leftType) || isTensorType(rightType)) {
        // Mixed tensor/scalar - return tensor type
        return isTensorType(leftType) ? leftType : rightType
      }

      // Handle vector/array operations
      if (isArrayType(leftType) && isArrayType(rightType)) {
        // Both operands are arrays - check dimensions match
        if (leftType.rank !== rightType.rank) {
          this.emitError(`Array operations require same rank, got rank ${leftType.rank} and rank ${rightType.rank}`, node.position)
          return leftType
        }
        // Check dimensions match (if both have known dimensions)
        for (let i = 0; i < leftType.rank; i++) {
          if (leftType.dimensions[i] !== undefined && rightType.dimensions[i] !== undefined &&
              leftType.dimensions[i] !== rightType.dimensions[i]) {
            this.emitError(`Array operations require matching dimensions, got ${leftType.toString()} and ${rightType.toString()}`, node.position)
            return leftType
          }
        }
        // Element types must match exactly - no implicit coercion
        if (!sameType(leftType.elementType, rightType.elementType)) {
          this.emitError(`Array element types must match exactly: ${leftType.elementType.toString()} and ${rightType.elementType.toString()}. Use explicit cast to convert.`, node.position)
          return leftType
        }
        return leftType
      }

      // Check if one operand is array and other is scalar (broadcasting)
      // Scalar type must match array element type exactly - no implicit coercion
      if (isArrayType(leftType) && isNumericType(rightType)) {
        if (!sameType(leftType.elementType, rightType)) {
          this.emitError(`Scalar type ${rightType.toString()} does not match array element type ${leftType.elementType.toString()}. Use explicit cast to convert.`, node.position)
          return leftType
        }
        return leftType
      }
      if (isNumericType(leftType) && isArrayType(rightType)) {
        if (!sameType(leftType, rightType.elementType)) {
          this.emitError(`Scalar type ${leftType.toString()} does not match array element type ${rightType.elementType.toString()}. Use explicit cast to convert.`, node.position)
          return rightType
        }
        return rightType
      }

      // Handle tensor binary operations (+, -, *)
      if (isTensorType(leftType) && isTensorType(rightType)) {
        return leftType
      }

      // Allow pointer arithmetic: ptr + int or ptr - int
      if ((operator === "+" || operator === "-") && isPointerType(leftType) && isNumericType(rightType)) {
        return leftType
      }

      // Allow string concatenation: string + string
      if (operator === "+" && isStringType(leftType) && isStringType(rightType)) {
        return new StringType()
      }

      if (!isNumericType(leftType) || !isNumericType(rightType)) {
        this.emitError(`Operator '${operator}' requires numeric types`, node.position)
        return leftType
      }

      // Numeric types must match exactly - no implicit coercion
      if (!sameType(leftType, rightType)) {
        this.emitError(
          `Operator '${operator}' requires identical types, got ${leftType.toString()} and ${rightType.toString()}. Use explicit cast to convert.`,
          node.position,
        )
        return leftType
      }

      return leftType
    }

    if (comparisonOperators.includes(operator)) {
      const commonType = getCommonType(leftType, rightType)
      if (!commonType) {
        this.emitError(`Cannot compare ${leftType.toString()} with ${rightType.toString()}`, node.position)
      }

      return new IntegerType("i32")
    }

    if (logicalOperators.includes(operator)) {
      if (!isNumericType(leftType) || !isNumericType(rightType)) {
        this.emitError(`Logical operator '${operator}' requires numeric types`, node.position)
      }

      return new IntegerType("i32")
    }

    this.emitError(`Unknown binary operator '${operator}'`, node.position)
    return leftType
  }

  private visitUnaryExpression(node: UnaryExpressionNode): Type | null {
    const operand = node.operand || (node as any).argument
    const argumentType = this.visitExpression(operand)

    if (!argumentType) {
      return null
    }

    const operator = node.operator

    if (operator === "!") {
      if (!isNumericType(argumentType)) {
        this.emitError(`Operator '!' requires numeric type`, node.position)
      }
      return argumentType
    }

    if (operator === "-" || operator === "+") {
      if (!isNumericType(argumentType)) {
        this.emitError(`Operator '${operator}' requires numeric type`, node.position)
      }
      return argumentType
    }

    if (operator === "~") {
      if (!isNumericType(argumentType)) {
        this.emitError(`Operator '~' requires numeric type`, node.position)
      }
      return argumentType
    }

    this.emitError(`Unknown unary operator '${operator}'`, node.position)
      return argumentType
  }

  private visitCastExpression(node: CastExpressionNode): Type | null {
    const valueType = this.visitExpression(node.value)

    if (!valueType) {
      return null
    }

    const targetType = node.targetType

    if (!(isIntegerType(targetType) || isUnsignedType(targetType) || isFloatType(targetType))) {
      this.emitError(`Cast target must be a numeric type, got ${targetType.toString()}`, node.position)
      return null
    }

    const isSourceNumeric = isIntegerType(valueType) || isUnsignedType(valueType) || isFloatType(valueType)
    const isSourceArray = isArrayType(valueType)

    if (!isSourceNumeric && !isSourceArray) {
      this.emitError(`Cast source must be a numeric type or array, got ${valueType.toString()}`, node.position)
      return null
    }

    if (isSourceArray) {
      node.sourceType = valueType
      return targetType
    }

    const sourceUnsigned = isUnsignedType(valueType)
    const sourceSigned = isIntegerType(valueType)
    const targetUnsigned = isUnsignedType(targetType)
    const targetSigned = isIntegerType(targetType)
    const sourceFloat = isFloatType(valueType)
    const targetFloat = isFloatType(targetType)
    const sourceBits = valueType.bits
    const targetBits = targetType.bits

    if (sourceUnsigned && targetUnsigned) {
      if (sourceBits > targetBits) {
        this.emitError(
          `Lossy cast from ${valueType.toString()} to ${targetType.toString()}: would lose precision`,
          node.position
        )
      }
    } else if (sourceSigned && targetSigned) {
      if (sourceBits > targetBits) {
        this.emitError(
          `Lossy cast from ${valueType.toString()} to ${targetType.toString()}: would lose precision`,
          node.position
        )
      }
    } else if ((sourceSigned || sourceUnsigned) && targetFloat) {
      return targetType
    } else if (sourceFloat && (targetSigned || targetUnsigned)) {
      this.emitError(
        `Precision loss warning: casting from ${valueType.toString()} to ${targetType.toString()} may lose precision`,
        node.position
      )
    } else if (sourceUnsigned && targetSigned) {
      if (sourceBits > targetBits) {
        this.emitError(
          `Lossy cast from ${valueType.toString()} to ${targetType.toString()}: would lose precision`,
          node.position
        )
      }
    } else if (sourceSigned && targetUnsigned) {
      this.emitError(
        `Signed to unsigned cast from ${valueType.toString()} to ${targetType.toString()} may change sign semantics`,
        node.position
      )
    }

    return targetType
  }

  private posixBufferFunctions = new Set([
    "read", "write", "pread", "pwrite",
    "stat", "lstat", "fstat",
    "readlink", "getcwd",
    "access", "faccessat",
    "chmod", "fchmod", "chown", "fchown",
    "utimes", "futimes", "utimensat",
    "truncate", "ftruncate",
    "link", "symlink",
    "rename", "unlink",
    "mkdir", "rmdir",
    "chdir", "fchdir",
    "mkfifo", "mknod",
  ])

  private visitAwaitExpression(node: AwaitExpressionNode): Type | null {
    if (!this.inAsyncFunction) {
      this.emitError("await can only be used inside an async function", node.position)
    }

    const argType = this.visitExpression(node.argument)

    // await unwraps Future<T> to T
    if (argType && isFutureType(argType)) {
      return (argType as FutureType).innerType
    }

    // If it's not a Future, just return the type as-is (synchronous fallback)
    return argType
  }

  private visitSpawnExpression(node: SpawnExpressionNode): Type | null {
    if (!this.inAsyncFunction) {
      this.emitError("spawn can only be used inside an async function", node.position)
    }

    // spawn runs the async function without awaiting — returns the Future itself (as ptr)
    const argType = this.visitExpression(node.argument)
    // spawn returns a ptr (the MogFuture pointer) that can be ignored
    return new PointerType()
  }

  private visitCallExpression(node: CallExpressionNode): Type | null {
    if (node.callee.type === "Identifier") {
      const identifier = node.callee as IdentifierNode

      // Handle all([...]) and race([...]) builtins for structured concurrency
      if (identifier.name === "all" || identifier.name === "race") {
        const args = (node as any).args ?? node.arguments ?? []
        if (args.length !== 1) {
          this.emitError(`${identifier.name}() expects exactly 1 argument (an array of futures)`, node.position)
          return null
        }
        const arg = args[0]
        if (arg.type === "ArrayLiteral") {
          // Visit all elements and collect their types
          const elemTypes: (Type | null)[] = []
          for (const elem of (arg as any).elements) {
            elemTypes.push(this.visitExpression(elem))
          }
          if (identifier.name === "all") {
            // all([...]) returns Future<[T]> where T is the common element type
            // For simplicity, use the first element's unwrapped type
            const firstType = elemTypes[0]
            if (firstType && isFutureType(firstType)) {
              const innerType = (firstType as FutureType).innerType
              return new FutureType(new ArrayType(innerType, []))
            }
            if (firstType) {
              return new FutureType(new ArrayType(firstType, []))
            }
            return null
          } else {
            // race([...]) returns Future<T> where T is the common element type
            const firstType = elemTypes[0]
            if (firstType && isFutureType(firstType)) {
              return firstType // Already a Future<T>
            }
            if (firstType) {
              return new FutureType(firstType)
            }
            return null
          }
        }
        // If not an array literal, visit the argument
        const argType = this.visitExpression(arg)
        return argType
      }

      const symbol = this.symbolTable.lookup(identifier.name)

      if (!symbol) {
        this.emitError(`Undefined function '${identifier.name}'`, node.position)
        return null
      }

      // Check for cast<i64>() with POSIX buffer functions
      if (this.posixBufferFunctions.has(identifier.name)) {
        const args = (node as any).args ?? node.arguments ?? []
        for (let i = 0; i < args.length; i++) {
          const arg = args[i]
          if (arg.type === "CastExpression") {
            this.emitWarning(
              `Avoid cast<i64>() with POSIX buffer parameters. Pass arrays directly instead of casting to i64.`,
              arg.position
            )
          }
        }
      }
    }

    const calleeType = this.visitExpression(node.callee)

    if (!calleeType) {
      return null
    }

    if (node.callee.type === "Identifier") {
      const identifier = node.callee as IdentifierNode
      const symbol = this.symbolTable.lookup(identifier.name)

      if (symbol && symbol.symbolType === "variable") {
        if (symbol.inferredType && isArrayType(symbol.inferredType)) {
          const arrayType = symbol.inferredType as ArrayType
          const args = (node as any).args ?? node.arguments
          if (args.length === 1) {
            const indexType = this.visitExpression(args[0])
            if (indexType && !(isIntegerType(indexType) || isUnsignedType(indexType))) {
              this.emitError(`Array index must be integer type`, args[0].position)
            }
            return arrayType.elementType
          }
        }
      }

      // Handle regular function calls - return the function's return type
      if (symbol && symbol.symbolType === "function") {
        // Visit named args if present
        const namedArgs = (node as any).namedArgs
        if (namedArgs) {
          for (const namedArg of namedArgs) {
            this.visitExpression(namedArg.value)
          }
        }
        // If declaredType is a FunctionType, return its returnType
        if (symbol.declaredType && isFunctionType(symbol.declaredType)) {
          return (symbol.declaredType as FunctionType).returnType
        }
        return symbol.declaredType
      }

      // Handle calling a variable that holds a function (higher-order function)
      if (symbol && symbol.symbolType === "variable") {
        const varType = symbol.inferredType || symbol.declaredType
        if (varType && isFunctionType(varType)) {
          const funcType = varType as FunctionType
          // Visit all args
          const args = (node as any).args ?? node.arguments
          for (const arg of args) {
            this.visitExpression(arg)
          }
          // Visit named args if present
          const namedArgs = (node as any).namedArgs
          if (namedArgs) {
            for (const namedArg of namedArgs) {
              this.visitExpression(namedArg.value)
            }
          }
          return funcType.returnType
        }
      }
    }

    // Handle capability function calls (e.g., fs.read(...))
    if (node.callee.type === "MemberExpression") {
      const memberExpr = node.callee as MemberExpressionNode
      if (memberExpr.object.type === "Identifier") {
        const objName = (memberExpr.object as IdentifierNode).name
        const allCaps = [...this.requiredCapabilities, ...this.optionalCapabilities]
        if (allCaps.includes(objName)) {
          // This is a capability call - type check against declaration
          const capDecl = this.capabilityDecls.get(objName)
          if (capDecl) {
            const funcDecl = capDecl.functions.find((f: any) => f.name === memberExpr.property)
            if (!funcDecl) {
              this.emitError(`Capability '${objName}' has no function '${memberExpr.property}'`, node.position)
              return null
            }
            // Type check arguments
            const args = (node as any).args ?? node.arguments ?? []
            const requiredParams = funcDecl.params.filter((p: any) => !p.optional)
            if (args.length < requiredParams.length) {
              this.emitError(`${objName}.${memberExpr.property}() expects at least ${requiredParams.length} arguments, got ${args.length}`, node.position)
            }
            if (args.length > funcDecl.params.length) {
              this.emitError(`${objName}.${memberExpr.property}() expects at most ${funcDecl.params.length} arguments, got ${args.length}`, node.position)
            }
            // Visit args to check their types
            for (const arg of args) {
              this.visitExpression(arg)
            }
            // Return the declared return type (mapped to Mog types)
            if (funcDecl.returnType) {
              switch (funcDecl.returnType) {
                case "int": return new IntegerType("i64")
                case "float": return new FloatType("f64")
                case "bool": return boolType
                case "string": return new PointerType()
                default:
                  if (funcDecl.returnType.startsWith("Result<")) {
                    const inner = funcDecl.returnType.slice(7, -1)
                    switch (inner) {
                      case "int": return new IntegerType("i64")
                      case "float": return new FloatType("f64")
                      case "string": return new PointerType()
                      default: return new IntegerType("i64")
                    }
                  }
                  return new IntegerType("i64")
              }
            }
            return new VoidType()
          }
          // If no declaration loaded, still allow it (runtime will handle)
          for (const arg of (node.arguments || [])) {
            this.visitExpression(arg)
          }
          return new IntegerType("i64")
        }
      }
    }

    if (node.callee.type === "MemberExpression") {
      const memberExpr = node.callee as MemberExpressionNode
      const objectType = this.visitExpression(memberExpr.object)

      // Handle string method calls
      if (objectType && this.isStringLikeType(objectType)) {
        const method = memberExpr.property
        const args = (node as any).args ?? node.arguments
        // Visit arguments for type checking
        for (const arg of args) {
          this.visitExpression(arg)
        }
        const strType = new StringType()
        switch (method) {
          case "upper":
          case "lower":
          case "trim":
          case "replace":
            return strType
          case "split":
            return new ArrayType(strType, [0])
          case "contains":
          case "starts_with":
          case "ends_with":
            return new IntegerType("i64")
          case "len":
            return new UnsignedType("u64")
        }
      }

      // Handle array method calls (non-string arrays)
      if (objectType && isArrayType(objectType) && !this.isStringLikeType(objectType)) {
        const arrayType = objectType as ArrayType
        const method = memberExpr.property
        const args = (node as any).args ?? node.arguments

        switch (method) {
          case "push": {
            // Visit arguments for type checking
            for (const arg of args) {
              this.visitExpression(arg)
            }
            return new IntegerType("i64") // void-like
          }
          case "pop":
            return arrayType.elementType || new IntegerType("i64")
          case "contains": {
            for (const arg of args) {
              this.visitExpression(arg)
            }
            return new IntegerType("i64") // bool as i64 (0 or 1)
          }
          case "sort":
          case "reverse":
            return new IntegerType("i64") // void-like (in-place)
          case "join": {
            for (const arg of args) {
              this.visitExpression(arg)
            }
            return new StringType()
          }
          case "len":
            return new UnsignedType("u64")
          case "map":
          case "filter": {
            for (const arg of args) {
              this.visitExpression(arg)
            }
            return new ArrayType(arrayType.elementType, [])
          }
          case "reduce": {
            for (const arg of args) {
              this.visitExpression(arg)
            }
            return new IntegerType("i64")
          }
        }

        // Fallback: array indexing via call syntax arr(i)
        if (args.length === 1) {
          const indexType = this.visitExpression(args[0])
          if (indexType && !(isIntegerType(indexType) || isUnsignedType(indexType))) {
            this.emitError(`Array index must be integer type`, args[0].position)
          }
          return arrayType.elementType
        }
      }

      // Handle map method calls
      if (objectType && isMapType(objectType)) {
        const method = memberExpr.property
        const args = (node as any).args ?? node.arguments
        for (const arg of args) {
          this.visitExpression(arg)
        }
        switch (method) {
          case "has":
            return new IntegerType("i64") // bool as i64 (0 or 1)
        }
      }

      // Handle array indexing via call syntax for string-like arrays
      if (objectType && isArrayType(objectType) && this.isStringLikeType(objectType)) {
        // Already handled above in string method section; this is a fallback for call-as-index
      }

      // Handle tensor method calls
      if (objectType && isTensorType(objectType)) {
        const tensorType = objectType as TensorType
        const method = memberExpr.property
        const args = (node as any).args ?? node.arguments
        // Visit arguments for type checking
        for (const arg of args) {
          this.visitExpression(arg)
        }
        switch (method) {
          case "matmul":
          case "transpose":
          case "reshape":
          case "view":
          case "flatten":
          case "squeeze":
          case "unsqueeze":
            return tensorType
          case "norm":
          case "sum":
          case "mean":
          case "max":
          case "min":
          case "prod":
            return new FloatType("f64")
          case "argmax":
          case "argmin":
            return new IntegerType("i64")
          case "dot":
            return new FloatType("f64")
          case "backward":
          case "requires_grad":
            return tensorType
        }
      }
    }

    return null
  }

  private visitMemberExpression(node: MemberExpressionNode): Type | null {
    // Allow member access on capability names (handled by visitCallExpression)
    if (node.object.type === "Identifier") {
      const objName = (node.object as IdentifierNode).name
      const allCaps = [...this.requiredCapabilities, ...this.optionalCapabilities]
      if (allCaps.includes(objName)) {
        return new IntegerType("i64")
      }
    }

    const objectType = this.visitExpression(node.object)

    if (!objectType) {
      return null
    }

    // Handle string member access
    if (this.isStringLikeType(objectType)) {
      if (node.property === "len") {
        return new UnsignedType("u64")
      }
      return new PointerType()
    }

    if (node.object.type === "Identifier") {
      const identifier = node.object as IdentifierNode
      const symbol = this.symbolTable.lookup(identifier.name)

      if (symbol && symbol.declaredType && isMapType(symbol.declaredType)) {
        const mapType = symbol.declaredType as MapType
        return mapType.valueType
      }
    }

    if (isArrayType(objectType) && !this.isStringLikeType(objectType)) {
      // Handle array property access: .len
      if (node.property === "len") {
        return new UnsignedType("u64")
      }

      const arrayType = objectType as ArrayType
      if (arrayType.rank > 0) {
        const newDimensions = arrayType.dimensions.slice(0, -1)
        if (newDimensions.length === 0) {
          return arrayType.elementType
        }
        return new ArrayType(arrayType.elementType, newDimensions)
      }
      return arrayType.elementType
    }

    // Allow member access on StructType - returns the field type
    if (objectType.type === "StructType") {
      const structType = objectType as StructType
      const fieldName = typeof node.property === "string" ? node.property : (node.property as any)?.name
      const fieldType = structType.fields.get(fieldName)
      if (fieldType) {
        return fieldType
      }
      this.emitError(`Struct '${structType.name}' has no field '${fieldName}'`, node.position)
      return null
    }

    // Allow member access on SOAType - returns the field type from the backing struct
    // In the new design, this handles datums[i].field where datums[i] resolves to StructType
    // but also handles direct datums.field for error recovery
    if (objectType.type === "SOAType") {
      const soaType = objectType as SOAType
      const fieldName = typeof node.property === "string" ? node.property : (node.property as any)?.name
      const fieldType = soaType.structType.fields.get(fieldName)
      if (fieldType) {
        return fieldType
      }
      this.emitError(`SoA type '${soaType.structType.name}' has no field '${fieldName}'`, node.position)
      return null
    }

    // Handle tensor property access
    if (isTensorType(objectType)) {
      const tensorType = objectType as TensorType
      if (node.property === "shape") {
        return new ArrayType(new IntegerType("i64"), [])
      }
      if (node.property === "dtype") {
        return tensorType.dtype
      }
      if (node.property === "grad") {
        return tensorType
      }
      // For method references (without call), return the tensor type
      // The actual call will be resolved by visitCallExpression
      return tensorType
    }

    // Allow member access on pointers (tables)
    if (isPointerType(objectType)) {
      return new IntegerType("i64")
    }

    this.emitError(`Cannot access property '${node.property}' on type ${objectType.toString()}`, node.position)

    return null
  }

  private visitTensorConstruction(node: any): Type | null {
    // Visit constructor arguments for type checking
    if (node.args) {
      for (const arg of node.args) {
        this.visitExpression(arg)
      }
    }
    return new TensorType(node.dtype)
  }

  private visitSoAConstructor(node: any): Type | null {
    const structInfo = this.symbolTable.lookup(node.structName)
    if (!structInfo || structInfo.symbolType !== "type") {
      this.emitError(`Undefined struct type '${node.structName}'`, node.position)
      return null
    }
    const structType = structInfo.declaredType
    if (!(structType instanceof StructType)) {
      this.emitError(`'${node.structName}' is not a struct type`, node.position)
      return null
    }
    return new SOAType(structType, node.capacity)
  }

  private visitIndexExpression(node: IndexExpressionNode): Type | null {
    const objectType = this.visitExpression(node.object)

    if (!objectType) {
      return null
    }

    const indexType = this.visitExpression(node.index)

    if (!indexType) {
      return null
    }

    // Handle map indexing: map[key]
    if (isMapType(objectType)) {
      const mapType = objectType as MapType
      // For maps, index can be string or integer
      if (!(indexType instanceof ArrayType && indexType.isString) && !isIntegerType(indexType) && !isUnsignedType(indexType)) {
        this.emitError(`Map key must be string or integer type, got ${indexType.toString()}`, node.index.position)
      }
      return mapType.valueType
    }

    if (!(isIntegerType(indexType) || isUnsignedType(indexType))) {
      this.emitError(`Array index must be integer type, got ${indexType.toString()}`, node.index.position)
    }

    // Handle SoA indexing: datums[i] returns the backing StructType
    if (isSOAType(objectType)) {
      const soaType = objectType as SOAType
      return soaType.structType
    }

    if (isArrayType(objectType)) {
      const arrayType = objectType as ArrayType
      // Check if this is a string type [u8]
      if (isUnsignedType(arrayType.elementType) && arrayType.elementType.kind === "u8" && arrayType.dimensions.length === 0) {
        // String indexing returns a string (single char as string)
        return objectType
      }
      if (arrayType.rank > 0) {
        const newDimensions = arrayType.dimensions.slice(0, -1)
        if (newDimensions.length === 0) {
          return arrayType.elementType
        }
        return new ArrayType(arrayType.elementType, newDimensions)
      }
      return arrayType.elementType
    }

    // Allow indexing raw pointers (from gc_alloc) — returns i64
    if (isPointerType(objectType)) {
      return new IntegerType("i64")
    }

    this.emitError(`Cannot index into non-array type ${objectType.toString()}`, node.position)

    return null
  }

  private visitSliceExpression(node: SliceExpressionNode): Type | null {
    const objectType = this.visitExpression(node.object)
    if (!objectType) return null

    const startType = this.visitExpression(node.start)
    const endType = this.visitExpression(node.end)

    if (startType && !(isIntegerType(startType) || isUnsignedType(startType))) {
      this.emitError(`Slice start must be integer type, got ${startType.toString()}`, node.start.position)
    }
    if (endType && !(isIntegerType(endType) || isUnsignedType(endType))) {
      this.emitError(`Slice end must be integer type, got ${endType.toString()}`, node.end.position)
    }

    // Type check step if provided
    if (node.step) {
      const stepType = this.visitExpression(node.step)
      if (stepType && !(isIntegerType(stepType) || isUnsignedType(stepType))) {
        this.emitError(`Slice step must be integer type, got ${stepType.toString()}`, node.step.position)
      }
    }

    // Allow slicing strings - returns a string
    if (isStringType(objectType)) {
      return new StringType()
    }

    if (isArrayType(objectType)) {
      const arrayType = objectType as ArrayType
      // Check if this is a string type [u8]
      if (isUnsignedType(arrayType.elementType) && arrayType.elementType.kind === "u8" && arrayType.dimensions.length === 0) {
        // String slicing returns a string
        return objectType
      }
      return objectType
    }

    this.emitError(`Cannot slice non-array type ${objectType.toString()}`, node.position)
    return null
  }

  private visitLLMExpression(node: LLMExpressionNode): Type | null {
    const promptType = this.visitExpression(node.prompt)

    if (promptType) {
      const expectedStringType = new ArrayType(new UnsignedType("u8"), [])
      if (!sameType(promptType, expectedStringType) && !sameType(promptType, expectedStringType)) {
        this.emitError(`LLM prompt must be string type ([u8])`, node.prompt.position)
      }
    }

    const modelSizeType = this.visitExpression(node.modelSize)

    if (modelSizeType) {
      const expectedStringType = new ArrayType(new UnsignedType("u8"), [])
      if (!sameType(modelSizeType, expectedStringType) && !sameType(modelSizeType, expectedStringType)) {
        this.emitError(`LLM model_size must be string type`, node.modelSize.position)
      }
    }

    const reasoningEffortType = this.visitExpression(node.reasoningEffort)

    if (reasoningEffortType) {
      if (
        !isFloatType(reasoningEffortType) &&
        !(isIntegerType(reasoningEffortType) || isUnsignedType(reasoningEffortType))
      ) {
        this.emitError(`LLM reasoning_effort must be numeric type`, node.reasoningEffort.position)
      }
    }

    const contextType = this.visitExpression(node.context)

    if (contextType && !isArrayType(contextType)) {
      this.emitError(`LLM context must be array type`, node.context.position)
    }

    return node.returnType
  }

  private visitLambda(node: LambdaNode): Type | null {
    // Save and set currentFunction so return statements are allowed
    const prevFunction = this.currentFunction
    this.currentFunction = "__lambda__"

    // Record the scope depth BEFORE pushing the lambda's scope
    // Variables declared at depths <= this are "outer" variables
    const outerScopeDepth = this.symbolTable.getCurrentDepth()

    // Push a new scope for the lambda's parameters
    this.symbolTable.pushScope()

    const paramTypes: Type[] = []
    const paramNames = new Set<string>()
    for (const param of node.params) {
      let paramType = param.paramType
      if (paramType?.type === "CustomType") {
        const resolved = this.symbolTable.lookup((paramType as any).name)
        if (resolved?.declaredType) {
          paramType = resolved.declaredType
        }
      }
      paramTypes.push(paramType)
      paramNames.add(param.name)
      this.symbolTable.declare(param.name, "parameter", paramType)
      this.symbolTable.setCurrentType(param.name, paramType)
    }

    // Set up capture tracking for the lambda body
    const prevCaptured = this.currentCapturedVars
    const prevLambdaScopeDepth = this.lambdaScopeDepth
    this.currentCapturedVars = new Set<string>()
    this.lambdaScopeDepth = outerScopeDepth

    // Visit the body (which can reference variables from enclosing scope - closures)
    this.visitBlock(node.body)

    // Collect captured variables (exclude params - they are in the lambda's own scope)
    const capturedVars: string[] = []
    const capturedVarTypes: Map<string, Type> = new Map()
    for (const varName of this.currentCapturedVars) {
      if (!paramNames.has(varName)) {
        capturedVars.push(varName)
        // Look up the type of the captured variable
        const symbol = this.symbolTable.lookup(varName)
        if (symbol?.declaredType) {
          capturedVarTypes.set(varName, symbol.declaredType)
        } else if (symbol?.inferredType) {
          capturedVarTypes.set(varName, symbol.inferredType)
        }
      }
    }
    ;(node as any).capturedVars = capturedVars
    ;(node as any).capturedVarTypes = Object.fromEntries(capturedVarTypes)

    // Restore capture tracking state
    this.currentCapturedVars = prevCaptured
    this.lambdaScopeDepth = prevLambdaScopeDepth

    this.symbolTable.popScope()
    this.currentFunction = prevFunction

    // Return a FunctionType
    return new FunctionType(paramTypes, node.returnType)
  }

  private visitBlockExpression(node: BlockExpressionNode): Type | null {
    this.visitBlock(node.block)

    if (node.block.statements.length > 0) {
      const lastStmt = node.block.statements[node.block.statements.length - 1]

      if (lastStmt.type === "ExpressionStatement") {
        return this.visitExpression(lastStmt.expression)
      }
    }

    return new VoidType()
  }

  private visitIfExpression(node: any): Type | null {
    const conditionType = this.visitExpression(node.condition)

    if (conditionType) {
      if (!isIntegerType(conditionType) && !isUnsignedType(conditionType) && !isBoolType(conditionType)) {
        this.emitError(
          `If-expression condition must be integer, unsigned, or bool type, got ${conditionType.toString()}`,
          node.condition.position,
        )
      }
    }

    // Visit true branch - get the type of the last expression
    let trueType: Type | null = null
    if (node.trueBranch.type === "Block") {
      this.symbolTable.pushScope()
      for (const stmt of node.trueBranch.statements) {
        this.visitStatement(stmt)
      }
      this.symbolTable.popScope()
      // The type is from the last expression statement
      const stmts = node.trueBranch.statements
      if (stmts.length > 0) {
        const last = stmts[stmts.length - 1]
        if (last.type === "ExpressionStatement") {
          trueType = this.visitExpression(last.expression)
        }
      }
    }

    // Visit false branch
    let falseType: Type | null = null
    if (node.falseBranch) {
      if (node.falseBranch.type === "IfExpression") {
        falseType = this.visitIfExpression(node.falseBranch)
      } else if (node.falseBranch.type === "Block") {
        this.symbolTable.pushScope()
        for (const stmt of node.falseBranch.statements) {
          this.visitStatement(stmt)
        }
        this.symbolTable.popScope()
        const stmts = node.falseBranch.statements
        if (stmts.length > 0) {
          const last = stmts[stmts.length - 1]
          if (last.type === "ExpressionStatement") {
            falseType = this.visitExpression(last.expression)
          }
        }
      }
    }

    // Return the type from true branch (both should match)
    return trueType || falseType || new IntegerType("i64")
  }

  private visitMatchExpression(node: any): Type | null {
    const subjectType = this.visitExpression(node.subject)

    // Determine the binding type for variant patterns based on the subject type
    const getBindingType = (patternName: string): Type => {
      if (subjectType) {
        if (isResultType(subjectType) && (patternName === "ok" || patternName === "err")) {
          // ok(x) binds to the inner type; err(e) binds to i64 (error code)
          return patternName === "ok" ? subjectType.innerType : new IntegerType("i64")
        }
        if (isOptionalType(subjectType) && patternName === "some") {
          // some(x) binds to the inner type
          return subjectType.innerType
        }
      }
      return new IntegerType("i64")
    }

    let resultType: Type | null = null
    const patternNames = new Set<string>()

    for (const arm of node.arms) {
      // Collect pattern types for exhaustiveness checking
      if (arm.pattern) {
        if (arm.pattern.type === "WildcardPattern") {
          patternNames.add("_")
        } else if (arm.pattern.type === "VariantPattern") {
          patternNames.add(arm.pattern.name)
        }
      }

      // If this arm has a VariantPattern with a binding, declare it in a new scope
      if (arm.pattern && arm.pattern.type === "VariantPattern" && arm.pattern.binding) {
        this.symbolTable.pushScope()
        this.symbolTable.declare(arm.pattern.binding, "variable", getBindingType(arm.pattern.name))
        const armType = this.visitExpression(arm.body)
        if (!resultType && armType) {
          resultType = armType
        }
        this.symbolTable.popScope()
      } else {
        // Visit the arm body expression
        const armType = this.visitExpression(arm.body)
        if (!resultType && armType) {
          resultType = armType
        }
      }
    }

    // Exhaustiveness checking for Result and Optional types
    if (subjectType && !patternNames.has("_")) {
      if (isResultType(subjectType)) {
        const hasOk = patternNames.has("ok")
        const hasErr = patternNames.has("err")
        if (!hasOk && !hasErr) {
          this.emitWarning(
            `Non-exhaustive match on ${subjectType.toString()}: missing 'ok' and 'err' arms`,
            node.position
          )
        } else if (!hasOk) {
          this.emitWarning(
            `Non-exhaustive match on ${subjectType.toString()}: missing 'ok' arm`,
            node.position
          )
        } else if (!hasErr) {
          this.emitWarning(
            `Non-exhaustive match on ${subjectType.toString()}: missing 'err' arm`,
            node.position
          )
        }
      }
      if (isOptionalType(subjectType)) {
        const hasSome = patternNames.has("some")
        const hasNone = patternNames.has("none")
        if (!hasSome && !hasNone) {
          this.emitWarning(
            `Non-exhaustive match on ${subjectType.toString()}: missing 'some' and 'none' arms`,
            node.position
          )
        } else if (!hasSome) {
          this.emitWarning(
            `Non-exhaustive match on ${subjectType.toString()}: missing 'some' arm`,
            node.position
          )
        } else if (!hasNone) {
          this.emitWarning(
            `Non-exhaustive match on ${subjectType.toString()}: missing 'none' arm`,
            node.position
          )
        }
      }
    }

    return resultType || new IntegerType("i64")
  }

  private visitTryCatch(node: TryCatchNode): void {
    // Visit try body
    this.symbolTable.pushScope()
    for (const stmt of node.tryBody.statements) {
      this.visitStatement(stmt)
    }
    this.symbolTable.popScope()

    // Visit catch body with error variable declared
    this.symbolTable.pushScope()
    this.symbolTable.declare(node.errorVar, "variable", new ArrayType(new UnsignedType("u8"), []))
    for (const stmt of node.catchBody.statements) {
      this.visitStatement(stmt)
    }
    this.symbolTable.popScope()
  }

  private visitOkExpression(node: any): Type | null {
    const valueType = this.visitExpression(node.value)
    if (!valueType) return null
    return new ResultType(valueType)
  }

  private visitErrExpression(node: any): Type | null {
    const valueType = this.visitExpression(node.value)
    // err always creates Result<i64> by default (inner type determined by context)
    return new ResultType(new IntegerType("i64"))
  }

  private visitSomeExpression(node: any): Type | null {
    const valueType = this.visitExpression(node.value)
    if (!valueType) return null
    return new OptionalType(valueType)
  }

  private visitPropagateExpression(node: any): Type | null {
    const valueType = this.visitExpression(node.value)
    if (!valueType) return null
    // ? operator unwraps Result<T> to T
    if (valueType instanceof ResultType) {
      return valueType.innerType
    }
    // ? operator unwraps ?T to T
    if (valueType instanceof OptionalType) {
      return valueType.innerType
    }
    return valueType
  }

  private visitIsSomeExpression(node: any): Type | null {
    this.visitExpression(node.value)
    // `is some(name)` binds the name variable in the enclosing if-true scope
    // The binding is handled at codegen time
    return boolType
  }

  private visitIsNoneExpression(node: any): Type | null {
    this.visitExpression(node.value)
    return boolType
  }

  private visitIsOkExpression(node: any): Type | null {
    this.visitExpression(node.value)
    return boolType
  }

  private visitIsErrExpression(node: any): Type | null {
    this.visitExpression(node.value)
    return boolType
  }

  private isStringLikeType(type: Type): boolean {
    if (type instanceof StringType) {
      return true
    }
    if (type instanceof ArrayType) {
      return type.elementType instanceof UnsignedType &&
             (type.elementType as UnsignedType).kind === "u8" &&
             type.dimensions.length === 0
    }
    if (type instanceof PointerType) {
      return true  // ptr types could be strings
    }
    return false
  }
}

export { SemanticAnalyzer, SymbolTable, SemanticError }
export type {
  ProgramNode,
  StatementNode,
  ExpressionNode,
  ASTNode,
  Position,
  VariableDeclarationNode,
  AssignmentNode,
  FunctionDeclarationNode,
  FunctionParam,
  IdentifierNode,
  NumberLiteralNode,
  StringLiteralNode,
  ArrayLiteralNode,
  MapLiteralNode,
  BinaryExpressionNode,
  UnaryExpressionNode,
  CallExpressionNode,
  MemberExpressionNode,
  IndexExpressionNode,
  SliceExpressionNode,
  LLMExpressionNode,
  LambdaNode,
  BlockExpressionNode,
  BlockNode,
  ReturnNode,
  ConditionalNode,
  ExpressionStatementNode,
  CastExpressionNode,
  RequiresDeclarationNode,
  OptionalDeclarationNode,
  ForInRangeNode,
  ForInIndexNode,
  ForInMapNode,
  ForEachLoopNode,
  ForLoopNode,
  IfExpressionNode,
  MatchExpressionNode,
  MatchArm,
  MatchPattern,
  TryCatchNode,
  OkExpressionNode,
  ErrExpressionNode,
  SomeExpressionNode,
  NoneExpressionNode,
  PropagateExpressionNode,
  IsSomeExpressionNode,
  IsNoneExpressionNode,
  IsOkExpressionNode,
  IsErrExpressionNode,
  AsyncFunctionDeclarationNode,
  AwaitExpressionNode,
  PackageDeclarationNode,
  ImportDeclarationNode,
}
