import type { Type } from "./types.js"
import {
  isIntegerType,
  isUnsignedType,
  isFloatType,
  isArrayType,
  isTableType,
  isVoidType,
  isNumericType,
  sameType,
  getCommonType,
  IntegerType,
  UnsignedType,
  FloatType,
  ArrayType,
  TableType,
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
  | ArrayLiteralNode
  | TableLiteralNode
  | BinaryExpressionNode
  | UnaryExpressionNode
  | AssignmentExpressionNode
  | ConditionalExpressionNode
  | CallExpressionNode
  | MemberExpressionNode
  | IndexExpressionNode
  | LLMExpressionNode
  | LambdaNode
  | BlockExpressionNode

interface IdentifierNode extends ASTNode {
  type: "Identifier"
  name: string
}

interface NumberLiteralNode extends ASTNode {
  type: "NumberLiteral"
  value: string
  literalType: Type | null
}

interface StringLiteralNode extends ASTNode {
  type: "StringLiteral"
  value: string
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
  name: string
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
  private currentFunction: string | null = null

  constructor() {
    this.symbolTable = new SymbolTable()
  }

  analyze(program: ProgramNode): SemanticError[] {
    this.symbolTable = new SymbolTable()
    this.errors = []
    this.visitProgram(program)
    return this.errors
  }

  private emitError(message: string, position: { start: Position; end: Position }): void {
    this.errors.push({ message, position })
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
        if (!sameType(valueType, declaredType)) {
          this.emitError(
            `Type mismatch: cannot assign ${valueType.toString()} to ${declaredType.toString()} (requires explicit cast)`,
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
      if (!sameType(valueType, symbol.declaredType)) {
        this.emitError(
          `Type mismatch: cannot assign ${valueType.toString()} to ${symbol.declaredType.toString()} (requires explicit cast)`,
          node.position,
        )
      }
    } else if (valueType && symbol.inferredType) {
      if (!sameType(valueType, symbol.inferredType)) {
        this.emitError(
          `Type mismatch: cannot assign ${valueType.toString()} to ${symbol.inferredType.toString()} (requires explicit cast)`,
          node.position,
        )
      }
    }
  }

  private visitExpressionStatement(node: ExpressionStatementNode): void {
    this.visitExpression(node.expression)
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
      case "LLMExpression":
        return this.visitLLMExpression(node as LLMExpressionNode)
      case "Lambda":
        return this.visitLambda(node as LambdaNode)
      case "BlockExpression":
        return this.visitBlockExpression(node as BlockExpressionNode)
      case "AssignmentExpression":
        this.visitAssignment(node as any)
        return new IntegerType("i64")
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

    const value = node.value.toLowerCase()

    if (value.includes(".") || value.toLowerCase().includes("e")) {
      return new FloatType("f32")
    }

    return new IntegerType("i32")
  }

  private visitStringLiteral(node: StringLiteralNode): Type | null {
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

    return new ArrayType(commonType, [node.elements.length])
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

    const comparisonOperators = ["=", "!=", "<", ">", "<=", ">="]

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
      if (!isNumericType(operandType)) {
        this.emitError(`Operator '${operator}' requires numeric type`, node.position)
      }
      return operandType
    }

    this.emitError(`Unknown unary operator '${operator}'`, node.position)
    return operandType
  }

  private visitCallExpression(node: CallExpressionNode): Type | null {
    if (node.callee.type === "Identifier") {
      const identifier = node.callee as IdentifierNode
      const symbol = this.symbolTable.lookup(identifier.name)

      if (!symbol) {
        this.emitError(`Undefined function '${identifier.name}'`, node.position)
        return null
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
      return objectType
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
      return arrayType.elementType
    }

    this.emitError(`Cannot index into non-array type ${objectType.toString()}`, node.position)

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
  LLMExpressionNode,
  LambdaNode,
  BlockExpressionNode,
  BlockNode,
  ReturnNode,
  ConditionalNode,
  ExpressionStatementNode,
}
