import type { Type } from "./types.js"
import {
  isIntegerType,
  isUnsignedType,
  isFloatType,
  isArrayType,
  isTableType,
  isPointerType,
  isVoidType,
  isNumericType,
  sameType,
  compatibleTypes,
  getCommonType,
  IntegerType,
  UnsignedType,
  FloatType,
  ArrayType,
  TableType,
  PointerType,
  VoidType,
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
  | WhileLoopNode
  | ForLoopNode
  | ForEachLoopNode
  | BreakNode
  | ContinueNode

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
}

interface FunctionParam {
  name: string
  paramType: Type
}

type ExpressionNode =
  | IdentifierNode
  | NumberLiteralNode
  | StringLiteralNode
  | TemplateLiteralNode
  | ArrayLiteralNode
  | TableLiteralNode
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

interface TableLiteralNode extends ASTNode {
  type: "TableLiteral"
  columns: { name: string; columnType: Type | null; values: ExpressionNode[] }[]
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

interface SemanticError {
  message: string
  position: {
    start: Position
    end: Position
  }
}

interface SymbolInfo {
  name: string
  symbolType: "variable" | "function" | "parameter"
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

  declare(name: string, symbolType: "variable" | "function" | "parameter", declaredType: Type | null = null): void {
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

  constructor() {
    this.symbolTable = new SymbolTable()
  }

  analyze(program: ProgramNode): SemanticError[] {
    this.symbolTable = new SymbolTable()
    this.errors = []
    this.warnings = []
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
      print_string: { params: [{ name: "value", type: i64Type }], returnType: voidType },
      println: { params: [], returnType: voidType },
      println_i64: { params: [{ name: "value", type: i64Type }], returnType: voidType },
      println_u64: { params: [{ name: "value", type: u64Type }], returnType: voidType },
      println_f64: { params: [{ name: "value", type: f64Type }], returnType: voidType },
      println_string: { params: [{ name: "value", type: i64Type }], returnType: voidType },
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
    }

    for (const [name, func] of Object.entries(tableFunctions)) {
      this.symbolTable.declare(name, "function", func.returnType)
    }

    // String functions
    const stringFunctions: Record<string, { params: { name: string; type: Type }[]; returnType: Type }> = {
      string_length: { params: [{ name: "str", type: ptrType }], returnType: u64Type },
      string_concat: { params: [{ name: "a", type: ptrType }, { name: "b", type: ptrType }], returnType: ptrType },
    }

    for (const [name, func] of Object.entries(stringFunctions)) {
      this.symbolTable.declare(name, "function", func.returnType)
    }
  }

  private errors: SemanticError[] = []
  private warnings: SemanticError[] = []

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
      case "Break":
        // Break is valid inside loops - semantic check could verify context
        break
      case "Continue":
        // Continue is valid inside loops - semantic check could verify context
        break
      case "FunctionDeclaration":
        this.visitFunctionDeclaration(node as FunctionDeclarationNode)
        break
    }
  }

  private visitVariableDeclaration(node: VariableDeclarationNode): void {
    const declaredType = node.varType
    const valueType = node.value ? this.visitExpression(node.value) : null

    if (valueType) {
      if (declaredType) {
        if (!compatibleTypes(valueType, declaredType)) {
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
        this.emitError(`Undefined variable '${node.name}'`, node.position)
        return null
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
      const objectValue = this.visitExpression(node.target.object)
      
      // Handle IndexExpression (arrays)
      if (node.target.type === "IndexExpression") {
        const indexValue = this.visitExpression(node.target.index)

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
      if (!isIntegerType(conditionType) && !isUnsignedType(conditionType)) {
        this.emitError(
          `Condition must be integer or unsigned type, got ${conditionType.toString()}`,
          node.condition.position,
        )
      }
    }

    this.visitBlock(node.trueBranch)

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

    this.visitBlock(node.body)
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
    this.visitBlock(node.body)
    this.symbolTable.popScope()
  }

  private visitForEachLoop(node: ForEachLoopNode): void {
    const arrayType = this.visitExpression(node.array)

    if (arrayType && !isArrayType(arrayType)) {
      this.emitError(`For-each loop requires array type, got ${arrayType.toString()}`, node.position)
    }

    this.symbolTable.pushScope()
    this.symbolTable.declare(node.variable, "variable", node.varType)
    this.symbolTable.setCurrentType(node.variable, node.varType)
    this.visitBlock(node.body)
    this.symbolTable.popScope()
  }

  private visitFunctionDeclaration(node: FunctionDeclarationNode): void {
    this.symbolTable.declare(node.name, "function", node.returnType)

    const prevFunction = this.currentFunction
    this.currentFunction = node.name

    this.symbolTable.pushScope()

    for (const param of node.params) {
      this.symbolTable.declare(param.name, "parameter", param.paramType)
      this.symbolTable.setCurrentType(param.name, param.paramType)
    }

    this.visitBlock(node.body)

    this.symbolTable.popScope()

    this.currentFunction = prevFunction
  }

  private visitExpression(node: ExpressionNode): Type | null {
    switch (node.type) {
      case "Identifier":
        return this.visitIdentifier(node as IdentifierNode)
      case "NumberLiteral":
        return this.visitNumberLiteral(node as NumberLiteralNode)
      case "StringLiteral":
        return this.visitStringLiteral(node as StringLiteralNode)
      case "TemplateLiteral":
        return this.visitTemplateLiteral(node as TemplateLiteralNode)
      case "ArrayLiteral":
        return this.visitArrayLiteral(node as ArrayLiteralNode)
      case "TableLiteral":
        return this.visitTableLiteral(node as TableLiteralNode)
      case "BinaryExpression":
        return this.visitBinaryExpression(node as BinaryExpressionNode)
      case "UnaryExpression":
        return this.visitUnaryExpression(node as UnaryExpressionNode)
      case "CallExpression":
        return this.visitCallExpression(node as CallExpressionNode)
      case "MemberExpression":
        return this.visitMemberExpression(node as MemberExpressionNode)
      case "IndexExpression":
        return this.visitIndexExpression(node as IndexExpressionNode)
      case "SliceExpression":
        return this.visitSliceExpression(node as SliceExpressionNode)
      case "LLMExpression":
        return this.visitLLMExpression(node as LLMExpressionNode)
      case "Lambda":
        return this.visitLambda(node as LambdaNode)
      case "BlockExpression":
        return this.visitBlockExpression(node as BlockExpressionNode)
      case "CastExpression":
        return this.visitCastExpression(node as CastExpressionNode)
      case "AssignmentExpression":
        return this.visitAssignmentExpression(node as AssignmentExpressionNode)
      default:
        const unknown = node as { type: string; position: { start: Position; end: Position } }
        this.emitError(`Unknown expression type: ${unknown.type}`, unknown.position)
        return null
    }
  }

  private visitIdentifier(node: IdentifierNode): Type | null {
    const symbol = this.symbolTable.lookup(node.name)

    if (!symbol) {
      this.emitError(`Undefined variable '${node.name}'`, node.position)
      return null
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
      return new FloatType("f32")
    }

    return new IntegerType("i64")
  }

  private visitStringLiteral(node: StringLiteralNode): Type | null {
    return new ArrayType(new UnsignedType("u8"), [])
  }

  private visitTemplateLiteral(node: TemplateLiteralNode): Type | null {
    // Visit all expression parts to type-check them
    for (const part of node.parts) {
      if (typeof part !== "string") {
        this.visitExpression(part)
      }
    }
    // Template literals always return strings ([u8])
    return new ArrayType(new UnsignedType("u8"), [])
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

  private visitTableLiteral(node: TableLiteralNode): Type | null {
    const keyTypes: (Type | null)[] = []
    const valueTypes: Type[] = []

    for (const col of node.columns) {
      if (col.columnType) {
        keyTypes.push(col.columnType)
      }

      if (col.values.length === 0) {
        continue
      }

      const firstValueType = this.visitExpression(col.values[0])

      if (!firstValueType) {
        continue
      }

      const colTypes: Type[] = [firstValueType]

      for (let i = 1; i < col.values.length; i++) {
        const valType = this.visitExpression(col.values[i])
        if (valType) {
          colTypes.push(valType)
        }
      }

      if (colTypes.length > 0) {
        let commonType = colTypes[0]

        for (let i = 1; i < colTypes.length; i++) {
          if (!sameType(commonType, colTypes[i])) {
            this.emitError(`Table column '${col.name}' has incompatible types`, node.position)
          }
        }

        valueTypes.push(commonType)
      }
    }

    if (valueTypes.length === 0) {
      return null
    }

    return new TableType(new IntegerType("i32"), valueTypes[0])
  }

  private visitBinaryExpression(node: BinaryExpressionNode): Type | null {
    const leftType = this.visitExpression(node.left)
    const rightType = this.visitExpression(node.right)

    if (!leftType || !rightType) {
      return null
    }

    const operator = node.operator

    const arithmeticOperators = ["+", "-", "*", "/", "%", "&", "|", "TIMES", "DIVIDE"]

    const comparisonOperators = ["==", "!=", "<", ">", "<=", ">="]

    const logicalOperators = ["&&", "||", "and", "or", "AND", "OR"]

    if (arithmeticOperators.includes(operator)) {
      if (!isNumericType(leftType) || !isNumericType(rightType)) {
        this.emitError(`Operator '${operator}' requires numeric types`, node.position)
        return leftType
      }

      if (!sameType(leftType, rightType)) {
        this.emitError(
          `Operator '${operator}' requires same types, got ${leftType.toString()} and ${rightType.toString()}`,
          node.position,
        )
        return leftType
      }

      if (isArrayType(leftType) || isArrayType(rightType)) {
        if (!sameType(leftType, rightType)) {
          this.emitError(`Array operations require same array types`, node.position)
        }
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
    const argumentType = this.visitExpression(node.argument)

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

  private visitCallExpression(node: CallExpressionNode): Type | null {
    if (node.callee.type === "Identifier") {
      const identifier = node.callee as IdentifierNode
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
    }

    if (node.callee.type === "MemberExpression") {
      const memberExpr = node.callee as MemberExpressionNode
      const objectType = this.visitExpression(memberExpr.object)

      if (objectType && isArrayType(objectType)) {
        const arrayType = objectType as ArrayType
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

    return null
  }

  private visitMemberExpression(node: MemberExpressionNode): Type | null {
    const objectType = this.visitExpression(node.object)

    if (!objectType) {
      return null
    }

    if (node.object.type === "Identifier") {
      const identifier = node.object as IdentifierNode
      const symbol = this.symbolTable.lookup(identifier.name)

      if (symbol && symbol.declaredType && isTableType(symbol.declaredType)) {
        const tableType = symbol.declaredType as TableType
        return tableType.valueType
      }
    }

    if (isArrayType(objectType)) {
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

    // Allow member access on pointers (tables)
    if (isPointerType(objectType)) {
      return new IntegerType("i64")
    }

    this.emitError(`Cannot access property '${node.property}' on type ${objectType.toString()}`, node.position)

    return null
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

    if (!(isIntegerType(indexType) || isUnsignedType(indexType))) {
      this.emitError(`Array index must be integer type, got ${indexType.toString()}`, node.index.position)
    }

    if (isArrayType(objectType)) {
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

    if (isArrayType(objectType)) {
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
    return node.returnType
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
  TableLiteralNode,
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
}
