import type { Token } from "./lexer.js"
import type { ProgramNode, StatementNode, ExpressionNode, Position, BlockNode } from "./analyzer.js"
import { IntegerType, UnsignedType, FloatType, ArrayType, TableType, VoidType } from "./types.js"

type ParseResult<T> = T | null

class Parser {
  private tokens: Token[]
  private current = 0

  constructor(tokens: Token[]) {
    this.tokens = tokens
  }

  parse(): ProgramNode {
    const statements: StatementNode[] = []

    while (!this.isAtEnd()) {
      const stmt = this.statement()
      if (stmt) {
        statements.push(stmt)
      }
    }

    const unwrappedStatements = this.getUnwrappedStatements(statements)

    return {
      type: "Program",
      statements: unwrappedStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private getUnwrappedStatements(statements: StatementNode[]): StatementNode[] {
    if (statements.length === 1 && statements[0].type === "Block") {
      const block = statements[0] as any
      if (block.statements.length === 1) {
        return block.statements
      }
    }
    return statements
  }

  private statement(): StatementNode | null {
    while (this.matchType("SEMICOLON")) {}

    if (this.checkType("RBRACE") || this.isAtEnd()) {
      return null
    }

    if (this.matchType("LBRACE")) {
      return this.blockStatement()
    }
    if (this.matchType("fn")) {
      return this.functionDeclaration()
    }
    if (this.matchType("return")) {
      return this.returnStatement()
    }
    if (this.matchType("if")) {
      return this.ifStatement()
    }
    if (this.matchType("while")) {
      return this.whileStatement()
    }
    if (this.matchType("for")) {
      return this.forStatement()
    }
    if (
      this.checkType("VARIABLE") ||
      this.checkType("TYPE") ||
      (this.checkType("IDENTIFIER") && this.peekNext()?.type === "COLON")
    ) {
      return this.variableDeclaration()
    }

    const expr = this.expression()
    this.matchType("SEMICOLON")
    return {
      type: "ExpressionStatement",
      expression: expr,
      position: expr.position,
    }
  }

  private functionDeclaration(): StatementNode {
    const name = this.consume("IDENTIFIER", "Expected function name").value
    this.consume("LPAREN", "Expected ( after function name")

    const params: any[] = []
if (!this.checkType("RPAREN")) {
        do {
          const paramName = this.consume("IDENTIFIER", "Expected parameter name").value
          this.consume("COLON", "Expected : after parameter name")
          let paramToken = this.consume("TYPE", "Expected parameter type")
          let typeName = paramToken.value
          while (this.matchType("LBRACKET")) {
            typeName += "["
            this.consume("RBRACKET", "Expected ] after array bracket")
            typeName += "]"
          }
          const paramType = this.parseType(typeName)
          params.push({ name: paramName, paramType })
        } while (this.matchType("COMMA"))
      }
      this.consume("RPAREN", "Expected ) after parameters")

      this.consume("ARROW", "Expected -> after parameter list")
      let returnToken = this.consume("TYPE", "Expected return type")
      let returnTypeName = returnToken.value
      while (this.matchType("LBRACKET")) {
        returnTypeName += "["
        this.consume("RBRACKET", "Expected ] after array bracket")
        returnTypeName += "]"
      }
      const returnType = this.parseType(returnTypeName)

    this.consume("LBRACE", "Expected { after function signature")

    const bodyStatements: StatementNode[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement()
      if (stmt) {
        bodyStatements.push(stmt)
      }
    }
    this.consume("RBRACE", "Expected } after function body")

    const body: BlockNode = {
      type: "Block",
      statements: bodyStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }

    return {
      type: "FunctionDeclaration",
      name,
      params,
      returnType,
      body,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private returnStatement(): StatementNode {
    let value: ExpressionNode | null = null
    if (!this.checkType("SEMICOLON")) {
      value = this.expression()
    }
    this.consume("SEMICOLON", "Expected ; after return")

    return {
      type: "Return",
      value,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private ifStatement(): StatementNode {
    this.consume("LPAREN", "Expected ( after if")
    const condition = this.expression()
    this.consume("RPAREN", "Expected ) after condition")

    this.consume("LBRACE", "Expected { after if condition")

    const trueBranchStatements: StatementNode[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement()
      if (stmt) {
        trueBranchStatements.push(stmt)
      }
    }
    this.consume("RBRACE", "Expected } after if body")

    const trueBranch: BlockNode = {
      type: "Block",
      statements: trueBranchStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }

    let falseBranch: BlockNode | null = null
    if (this.matchType("else")) {
      this.consume("LBRACE", "Expected { after else")
      const falseBranchStatements: StatementNode[] = []
      while (!this.checkType("RBRACE") && !this.isAtEnd()) {
        const stmt = this.statement()
        if (stmt) {
          falseBranchStatements.push(stmt)
        }
      }
      this.consume("RBRACE", "Expected } after else body")
      falseBranch = {
        type: "Block",
        statements: falseBranchStatements,
        position: {
          start: { line: 1, column: 1, index: 0 },
          end: this.lastPosition(),
        },
      }
    }

    return {
      type: "Conditional",
      condition,
      trueBranch,
      falseBranch,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private whileStatement(): StatementNode {
    this.consume("LPAREN", "Expected ( after while")
    const test = this.expression()
    this.consume("RPAREN", "Expected ) after while condition")

    this.consume("LBRACE", "Expected { after while condition")

    const bodyStatements: StatementNode[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement()
      if (stmt) {
        bodyStatements.push(stmt)
      }
    }

    this.consume("RBRACE", "Expected } after while loop")

    const body: BlockNode = {
      type: "Block",
      statements: bodyStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }

    return {
      type: "WhileLoop",
      test,
      body,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private forStatement(): StatementNode {
    const variable = this.consume("IDENTIFIER", "Expected variable name after for").value
    this.consume("ASSIGN", "Expected := after variable name")
    const start = this.expression()
    this.consume("to", "Expected to after start value")
    const end = this.expression()

    this.consume("LBRACE", "Expected { after for header")

    const bodyStatements: StatementNode[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement()
      if (stmt) {
        bodyStatements.push(stmt)
      }
    }

    this.consume("RBRACE", "Expected } after for loop")

    const body: BlockNode = {
      type: "Block",
      statements: bodyStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }

    return {
      type: "ForLoop",
      variable,
      start,
      end,
      body,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private blockStatement(): BlockNode {
    const statements: StatementNode[] = []

    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement()
      if (stmt) {
        statements.push(stmt)
      }
    }

    this.consume("RBRACE", "Expected } after block")

    return {
      type: "Block",
      statements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private variableDeclaration(): StatementNode {
    const name = this.consume("IDENTIFIER", "Expected variable name").value
    this.consume("COLON", "Expected : after variable name")
    
    let typeToken = this.consume("TYPE", "Expected type annotation")
    let typeName = typeToken.value
    
    while (this.matchType("LBRACKET")) {
      typeName += "["
      this.consume("RBRACKET", "Expected ] after array bracket")
      typeName += "]"
    }
    
    const varType = this.parseType(typeName)
    this.consume("EQUAL", "Expected = after type")
    this.matchType("ASSIGN")

    const value: ExpressionNode = this.expression()

    this.consume("SEMICOLON", "Expected ; after variable declaration")

    return {
      type: "VariableDeclaration",
      name,
      varType,
      value,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private expression(): ExpressionNode {
    return this.assignment()
  }

  private assignment(): ExpressionNode {
    const expr = this.conditional()

    if (this.matchType("ASSIGN")) {
      const value = this.assignment()
      if (expr.type === "Identifier") {
        return {
          type: "AssignmentExpression",
          name: expr.name,
          value,
          position: this.combinePositions(expr, value),
        }
      }
      if (expr.type === "IndexExpression") {
        return {
          type: "AssignmentExpression",
          target: expr,
          value,
          position: this.combinePositions(expr, value),
        }
      }
      throw new Error("Invalid assignment target")
    }

    return expr
  }

  private conditional(): ExpressionNode {
    const expr = this.logicalOr()

    if (this.matchType("QUESTION")) {
      const thenExpr = this.expression()
      this.consume("COLON", "Expected : in conditional expression")
      const elseExpr = this.expression()
      return {
        type: "Conditional",
        condition: expr,
        trueBranch: thenExpr,
        falseBranch: elseExpr,
        position: this.combinePositions(expr, elseExpr),
      }
    }

    return expr
  }

  private logicalOr(): ExpressionNode {
    let expr = this.logicalAnd()

    while (this.matchType("BITWISE_OR")) {
      const operator = this.previous()
      const right = this.logicalAnd()
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right),
      }
    }

    return expr
  }

  private logicalAnd(): ExpressionNode {
    let expr = this.equality()

    while (this.matchType("BITWISE_AND")) {
      const operator = this.previous()
      const right = this.equality()
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right),
      }
    }

    return expr
  }

  private equality(): ExpressionNode {
    let expr = this.comparison()

    while (this.matchType("EQUAL") || this.matchType("NOT_EQUAL")) {
      const operator = this.previous()
      const right = this.comparison()
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right),
      }
    }

    return expr
  }

  private comparison(): ExpressionNode {
    let expr = this.additive()

    while (
      this.matchType("LESS") ||
      this.matchType("LESS_EQUAL") ||
      this.matchType("GREATER") ||
      this.matchType("GREATER_EQUAL")
    ) {
      const operator = this.previous()
      const right = this.additive()
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right),
      }
    }

    return expr
  }

  private additive(): ExpressionNode {
    let expr = this.multiplicative()

    while (this.matchType("PLUS") || this.matchType("MINUS")) {
      const operator = this.previous()
      const right = this.multiplicative()
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right),
      }
    }

    return expr
  }

  private multiplicative(): ExpressionNode {
    let expr = this.unary()

    while (
      this.matchType("TIMES") ||
      this.matchType("DIVIDE") ||
      this.matchType("MODULO")
    ) {
      const operator = this.previous()
      const right = this.unary()
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right),
      }
    }

    return expr
  }

  private unary(): ExpressionNode {
    if (this.matchType("MINUS") || this.matchType("BANG") || this.matchType("not")) {
      const operator = this.previous()
      const argument = this.unary()
      return {
        type: "UnaryExpression",
        operator: operator.value,
        argument,
        position: this.combinePositions({ position: operator.position }, argument),
      }
    }

    return this.primary()
  }

  private primary(): ExpressionNode {
    if (this.matchType("FALSE")) {
      const token = this.previous()
      return {
        type: "BooleanLiteral",
        value: false,
        position: token.position,
      }
    }

    if (this.matchType("TRUE")) {
      const token = this.previous()
      return {
        type: "BooleanLiteral",
        value: true,
        position: token.position,
      }
    }

    if (this.matchType("NUMBER")) {
      const token = this.previous()
      return {
        type: "NumberLiteral",
        value: parseFloat(token.value),
        position: token.position,
      }
    }

    if (this.matchType("STRING")) {
      const token = this.previous()
      return {
        type: "StringLiteral",
        value: token.value.slice(1, -1),
        position: token.position,
      }
    }

    if (this.matchType("POSIX_CONSTANT")) {
      const token = this.previous()
      return {
        type: "POSIXConstant",
        value: token.value,
        position: token.position,
      }
    }

    if (this.matchType("IDENTIFIER")) {
      const token = this.previous()
      let object: ExpressionNode = {
        type: "Identifier",
        name: token.value,
        position: token.position,
      }

      while (this.matchType("LBRACKET")) {
        const index = this.expression()
        this.consume("RBRACKET", "Expected ] after index")
        object = {
          type: "IndexExpression",
          object,
          index,
          position: this.combinePositions(object, index),
        }
      }

      if (this.matchType("LPAREN")) {
        const callArgs: ExpressionNode[] = []
        if (!this.checkType("RPAREN")) {
          do {
            callArgs.push(this.expression())
          } while (this.matchType("COMMA"))
        }
        this.consume("RPAREN", "Expected ) after arguments")

        return {
          type: "CallExpression",
          callee: object.type === "Identifier" ? object : { type: "Identifier", name: (object as any).name, position: token.position },
          args: callArgs,
          position: this.combinePositions(object, this.previous()),
        }
      }

      if (this.matchType("DOT")) {
        const property = this.consume("IDENTIFIER", "Expected property name").value
        return {
          type: "MemberExpression",
          object: object.type === "Identifier" ? object : { type: "Identifier", name: (object as any).name, position: token.position },
          property,
          position: this.combinePositions(object, this.previous()),
        }
      }

      return object
    }

    if (this.matchType("LBRACKET")) {
      return this.arrayLiteral()
    }

    if (this.matchType("LBRACE")) {
      return this.tableLiteral()
    }

    if (this.matchType("LPAREN")) {
      const expr = this.expression()
      this.consume("RPAREN", "Expected ) after expression")
      return expr
    }

    const token = this.peek()
    throw new Error(`Unexpected token: ${token.type} at line ${token.position.start.line}`)
  }

  private arrayLiteral(): ExpressionNode {
    const start = this.previous().position.start
    const elements: ExpressionNode[] = []

    if (!this.checkType("RBRACKET")) {
      do {
        elements.push(this.expression())
      } while (this.matchType("COMMA"))
    }

    this.consume("RBRACKET", "Expected ] after array elements")

    return {
      type: "ArrayLiteral",
      elements,
      position: {
        start,
        end: this.lastPosition(),
      },
    }
  }

  private parseArrayType(): any {
    let dimensions = 0;
    while (this.matchType("LBRACKET")) {
      dimensions++;
      this.consume("RBRACKET", "Expected ] after array dimension");
    }
    return dimensions;
  }

  private tableLiteral(): ExpressionNode {
    const start = this.previous().position.start
    const columns: any[] = []

    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const key = this.consume("IDENTIFIER", "Expected key in table literal")

      this.consume("COLON", "Expected : after key")

      const values: ExpressionNode[] = []
      values.push(this.expression())

      columns.push({ name: key.value, values, columnType: null })

      this.matchType("COMMA")
    }

    this.consume("RBRACE", "Expected } after table literal")

    return {
      type: "TableLiteral",
      columns,
      position: {
        start,
        end: this.lastPosition(),
      },
    }
  }

  private parseType(typeName: string): any {
    const bracketMatch = typeName.match(/^(.*?)(\[\]*)$/);
    if (bracketMatch) {
      const baseName = bracketMatch[1];
      const arraySuffix = bracketMatch[2];
      
      let elementType: Type;
      if (baseName.startsWith("i")) {
        elementType = new IntegerType(baseName as any);
      } else if (baseName.startsWith("u")) {
        elementType = new UnsignedType(baseName as any);
      } else if (baseName.startsWith("f")) {
        elementType = new FloatType(baseName as any);
      } else {
        return new VoidType();
      }
      
      const dimensions = [];
      let currentType = elementType;
      const bracketCount = (arraySuffix.match(/\[/g) || []).length;
      for (let i = 0; i < bracketCount; i++) {
        currentType = new ArrayType(currentType, []);
      }
      return currentType;
    }
    
    if (typeName.startsWith("i")) {
      return new IntegerType(typeName as any)
    }
    if (typeName.startsWith("u")) {
      return new UnsignedType(typeName as any)
    }
    if (typeName.startsWith("f")) {
      return new FloatType(typeName as any)
    }
    return new VoidType()
  }

  private consume(type: string, message: string): Token {
    if (this.checkType(type)) {
      return this.advance()
    }
    const token = this.peek()
    throw new Error(`${message} at line ${token.position.start.line}`)
  }

  private checkType(type: string): boolean {
    if (this.isAtEnd()) return false
    return this.peek().type === type
  }

  private skipWhitespace(): void {}

  private matchType(type: string): boolean {
    if (this.checkType(type)) {
      this.advance()
      return true
    }
    return false
  }

  private advance(steps: number = 1): Token {
    this.current += steps
    return this.tokens[this.current - steps]
  }

  private previous(): Token {
    return this.tokens[this.current - 1]
  }

  private peek(): Token {
    return this.tokens[this.current]
  }

  private peekNext(): Token | null {
    if (this.current + 1 >= this.tokens.length) return null
    return this.tokens[this.current + 1]
  }

  private isAtEnd(): boolean {
    return this.current >= this.tokens.length
  }

  private lastPosition(): Position {
    if (this.tokens.length > 0) {
      return this.tokens[this.tokens.length - 1].position.end
    }
    return { line: 1, column: 1, index: 0 }
  }

  private combinePositions(a: any, b: any): any {
    return {
      start: a.position.start,
      end: b.position.end,
    }
  }
}

export function parseTokens(tokens: Token[]): ProgramNode {
  const parser = new Parser(tokens)
  return parser.parse()
}

export { Parser }
