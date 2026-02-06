import type { Token } from "./lexer.js"
import { tokenize } from "./lexer.js"
import type { ProgramNode, StatementNode, ExpressionNode, Position, BlockNode } from "./analyzer.js"
import { IntegerType, UnsignedType, FloatType, ArrayType, MapType, PointerType, VoidType, CustomType } from "./types.js"
import { getPOSIXConstant } from "./posix_constants.js"

// Decode escape sequences in string literals
function decodeEscapeSequences(str: string): string {
  return str.replace(/\\(.)|\\x([0-9a-fA-F]{2})/g, (match, char, hex) => {
    if (hex !== undefined) {
      return String.fromCharCode(parseInt(hex, 16))
    }
    switch (char) {
      case "n": return "\n"
      case "r": return "\r"
      case "t": return "\t"
      case "\\": return "\\"
      case '"': return '"'
      case "'": return "'"
      default: return match  // Unknown escape, keep as-is
    }
  })
}

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
    if (this.matchType("struct")) {
      return this.structDeclaration()
    }
    if (this.matchType("soa")) {
      return this.soaDeclaration()
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
    if (this.matchType("break")) {
      return this.breakStatement()
    }
    if (this.matchType("continue")) {
      return this.continueStatement()
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
          // Accept both TYPE and IDENTIFIER tokens as parameter types (for custom struct types)
          let paramToken: any
          if (this.checkType("TYPE")) {
            paramToken = this.consume("TYPE", "Expected parameter type")
          } else {
            paramToken = this.consume("IDENTIFIER", "Expected parameter type")
          }
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
      // Accept both TYPE and IDENTIFIER tokens as return types (for custom struct types)
      let returnToken: any
      if (this.checkType("TYPE")) {
        returnToken = this.consume("TYPE", "Expected return type")
      } else {
        returnToken = this.consume("IDENTIFIER", "Expected return type")
      }
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

  private structDeclaration(): StatementNode {
    const name = this.consume("IDENTIFIER", "Expected struct name").value
    this.consume("LBRACE", "Expected { after struct name")

    const fields: { name: string; fieldType: any }[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const fieldName = this.consume("IDENTIFIER", "Expected field name").value
      this.consume("COLON", "Expected : after field name")
      // Handle array types in struct fields like [f64]
      let fieldType: any
      if (this.checkType("LBRACKET")) {
        fieldType = this.parseArrayTypeAnnotation()
      } else {
        const typeName = this.consume("TYPE", "Expected type after :").value
        fieldType = this.parseType(typeName)
      }
      fields.push({ name: fieldName, fieldType })
      this.matchType("COMMA")
    }

    this.consume("RBRACE", "Expected } after struct fields")

    return {
      type: "StructDeclaration",
      name,
      fields,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private soaDeclaration(): StatementNode {
    const name = this.consume("IDENTIFIER", "Expected SoA name").value
    this.consume("LBRACE", "Expected { after SoA name")

    const fields: { name: string; fieldType: any }[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const fieldName = this.consume("IDENTIFIER", "Expected field name").value
      this.consume("COLON", "Expected : after field name")
      // Handle array types like [f64]
      let fieldType: any
      if (this.checkType("LBRACKET")) {
        const typeName = this.parseArrayTypeAnnotation()
        fieldType = this.parseType(typeName)
      } else {
        const typeName = this.consume("TYPE", "Expected type after :").value
        fieldType = this.parseType(typeName)
      }
      fields.push({ name: fieldName, fieldType })
      this.matchType("COMMA")
    }

    this.consume("RBRACE", "Expected } after SoA fields")

    return {
      type: "SoADeclaration",
      name,
      fields,
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

    // Check if this is a for-each loop: for item: type in arr { ... }
    if (this.checkType("COLON")) {
      this.consume("COLON", "Expected : after variable name")
      let typeToken = this.consume("TYPE", "Expected type after :")
      let typeName = typeToken.value
      // Handle array types like i64[]
      while (this.matchType("LBRACKET")) {
        typeName += "["
        this.consume("RBRACKET", "Expected ] after array bracket")
        typeName += "]"
      }
      const varType = this.parseType(typeName)
      this.consume("in", "Expected 'in' after type annotation")
      const array = this.expression()

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
        type: "ForEachLoop",
        variable,
        varType,
        array,
        body,
        position: {
          start: { line: 1, column: 1, index: 0 },
          end: this.lastPosition(),
        },
      }
    }

    // Traditional for loop: for i := start to end { ... }
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

  private breakStatement(): StatementNode {
    this.matchType("SEMICOLON")
    return {
      type: "Break",
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private continueStatement(): StatementNode {
    this.matchType("SEMICOLON")
    return {
      type: "Continue",
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private variableDeclaration(): StatementNode {
    const name = this.consume("IDENTIFIER", "Expected variable name").value
    this.consume("COLON", "Expected : after variable name")
    
    // Handle array types that start with [ (e.g., [u8] or [f32; 3])
    let typeName: string
    let declaredType: { name: string; type: string } | null = null
    if (this.checkType("LBRACKET")) {
      // Parse array type like [u8], [i32], [f32; 3], or nested [[f64; 3]; 2], etc.
      typeName = this.parseArrayTypeAnnotation()
    } else if (this.checkType("TYPE")) {
      let typeToken = this.consume("TYPE", "Expected type annotation")
      typeName = typeToken.value
    } else if (this.checkType("IDENTIFIER")) {
      // Handle custom type names (e.g., Point, Particle)
      let typeToken = this.consume("IDENTIFIER", "Expected type annotation")
      typeName = typeToken.value
      declaredType = { name: typeName, type: "CustomType" }
    } else {
      throw new Error("Expected type annotation")
    }
    
    // Handle trailing [] for array of type (e.g., f64[][])
    while (this.matchType("LBRACKET")) {
      typeName += "[]"
      this.consume("RBRACKET", "Expected ] after array bracket")
    }
    
    const varType = this.parseType(typeName)
    
    // Handle AoS type - if varType is an AoS type object, use it for declaredType
    if (varType && typeof varType === 'object' && varType.type === "AOSType") {
      declaredType = varType
    }
    
    // Handle both `=` and `:=` assignment operators
    if (this.matchType("ASSIGN")) {
      // Walrus operator :=
    } else {
      this.consume("EQUAL", "Expected = or := after type")
      this.matchType("ASSIGN")
    }

    const initializer: ExpressionNode = this.expression()

    this.consume("SEMICOLON", "Expected ; after variable declaration")

    return {
      type: "VariableDeclaration",
      name,
      varType,
      value: initializer,
      declaredType: declaredType || { name: typeName, type: "PrimitiveType" },
      initializer,
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
      if (expr.type === "MemberExpression") {
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

    while (this.matchType("LOGICAL_OR")) {
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
    let expr = this.bitwiseOr()

    while (this.matchType("LOGICAL_AND")) {
      const operator = this.previous()
      const right = this.bitwiseOr()
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

  private bitwiseOr(): ExpressionNode {
    let expr = this.bitwiseXor()

    while (this.matchType("BITWISE_OR")) {
      const operator = this.previous()
      const right = this.bitwiseXor()
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

  private bitwiseXor(): ExpressionNode {
    let expr = this.bitwiseAnd()

    while (this.matchType("BITWISE_XOR")) {
      const operator = this.previous()
      const right = this.bitwiseAnd()
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

  private bitwiseAnd(): ExpressionNode {
    let expr = this.shift()

    while (this.matchType("BITWISE_AND")) {
      const operator = this.previous()
      const right = this.shift()
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

  private shift(): ExpressionNode {
    let expr = this.equality()

    while (this.matchType("LSHIFT") || this.matchType("RSHIFT")) {
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

  private bitwise(): ExpressionNode {
    let expr = this.multiplicative()

    while (this.matchType("BITWISE_AND") || this.matchType("BITWISE_OR")) {
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

  private unary(): ExpressionNode {
    if (this.matchType("MINUS") || this.matchType("BANG") || this.matchType("not") || this.matchType("BITWISE_NOT")) {
      const operator = this.previous()
      const argument = this.unary()
      return {
        type: "UnaryExpression",
        operator: operator.value,
        argument,
        position: this.combinePositions({ position: operator.position }, argument),
      }
    }

    if (this.matchType("cast")) {
      const castToken = this.previous()
      this.consume("LESS", "Expected < after cast")

      let typeName = this.consume("TYPE", "Expected type name after cast<").value
      while (this.matchType("LBRACKET")) {
        typeName += "["
        this.consume("RBRACKET", "Expected ] after array bracket")
        typeName += "]"
      }

      this.consume("GREATER", "Expected > after type name")
      this.consume("LPAREN", "Expected ( after cast<type>")

      const value = this.expression()
      this.consume("RPAREN", "Expected ) after cast value")

      const targetType = this.parseType(typeName)

      return {
        type: "CastExpression",
        targetType,
        value,
        position: this.combinePositions({ position: castToken.position }, value),
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
      // Store original string to preserve float vs int distinction
      return {
        type: "NumberLiteral",
        value: token.value,
        position: token.position,
        literalType: null,
      }
    }

    if (this.matchType("STRING")) {
      const token = this.previous()
      return {
        type: "StringLiteral",
        value: decodeEscapeSequences(token.value.slice(1, -1)),
        position: token.position,
      }
    }

    // Template string: f"Hello, {name}!"
    if (this.checkType("TEMPLATE_STRING_START")) {
      return this.parseTemplateString()
    }

    if (this.matchType("POSIX_CONSTANT")) {
      const token = this.previous()
      return {
        type: "POSIXConstant",
        value: getPOSIXConstant(token.value),
        position: token.position,
      }
    }

    if (this.matchType("IDENTIFIER")) {
      const token = this.previous()

      // Check for struct literal: TypeName { field: value, ... }
      if (this.matchType("LBRACE")) {
        const fields: { name: string; value: ExpressionNode }[] = []
        if (!this.checkType("RBRACE")) {
          do {
            const fieldName = this.consume("IDENTIFIER", "Expected field name").value
            this.consume("COLON", "Expected ':' after field name")
            const fieldValue = this.expression()
            fields.push({ name: fieldName, value: fieldValue })
          } while (this.matchType("COMMA"))
        }
        this.consume("RBRACE", "Expected '}' after struct literal fields")
        return {
          type: "StructLiteral",
          structName: token.value,
          fields,
          position: this.combinePositions(
            { position: token.position } as ExpressionNode,
            this.previous()
          ),
        }
      }

      let object: ExpressionNode = {
        type: "Identifier",
        name: token.value,
        position: token.position,
      }

      // Parse chained postfix operations: [], (), .
      let done = false
      while (!done) {
        if (this.matchType("LBRACKET")) {
          const firstExpr = this.expression()
          
          if (this.matchType("COLON")) {
            const endExpr = this.expression()
            this.consume("RBRACKET", "Expected ] after slice expression")
            object = {
              type: "SliceExpression",
              object,
              start: firstExpr,
              end: endExpr,
              position: this.combinePositions(object, endExpr),
            }
          } else {
            this.consume("RBRACKET", "Expected ] after index")
            object = {
              type: "IndexExpression",
              object,
              index: firstExpr,
              position: this.combinePositions(object, firstExpr),
            }
          }
        } else if (this.matchType("LPAREN")) {
          const callArgs: ExpressionNode[] = []
          if (!this.checkType("RPAREN")) {
            do {
              callArgs.push(this.expression())
            } while (this.matchType("COMMA"))
          }
          this.consume("RPAREN", "Expected ) after arguments")

          object = {
            type: "CallExpression",
            callee: object.type === "Identifier" ? object : { type: "Identifier", name: (object as any).name, position: token.position },
            args: callArgs,
            position: this.combinePositions(object, this.previous()),
          }
        } else if (this.matchType("DOT")) {
          const property = this.consume("IDENTIFIER", "Expected property name").value
          object = {
            type: "MemberExpression",
            object,
            property,
            position: this.combinePositions(object, this.previous()),
          }
        } else {
          done = true
        }
      }

      return object
    }

    if (this.matchType("LBRACKET")) {
      return this.arrayLiteral()
    }

    if (this.matchType("LBRACE")) {
      return this.mapLiteral()
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

    // Check for empty array []
    if (this.checkType("RBRACKET")) {
      this.consume("RBRACKET", "Expected ] after array elements")
      return {
        type: "ArrayLiteral",
        elements: [],
        position: {
          start,
          end: this.lastPosition(),
        },
      }
    }

    // Parse first element
    const firstExpr = this.expression()

    // Check if this is an array fill syntax: [value; count]
    if (this.matchType("SEMICOLON")) {
      const countExpr = this.expression()
      this.consume("RBRACKET", "Expected ] after array fill count")

      return {
        type: "ArrayFill",
        value: firstExpr,
        count: countExpr,
        position: {
          start,
          end: this.lastPosition(),
        },
      }
    }

    // Regular array literal: [elem1, elem2, ...]
    const elements: ExpressionNode[] = [firstExpr]
    while (this.matchType("COMMA")) {
      elements.push(this.expression())
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

  private parseArrayTypeAnnotation(): string {
    // Parse array type like [u8], [i32; 5], or nested [[f64; 3]; 2]
    this.consume("LBRACKET", "Expected [ to start array type")

    // Check if this is a nested array type (starts with [)
    if (this.checkType("LBRACKET")) {
      // Nested array: [[innerType; innerSize]; outerSize]
      const innerType = this.parseArrayTypeAnnotation()
      this.consume("SEMICOLON", "Expected ; after inner array type")
      const outerSize = this.consume("NUMBER", "Expected size after ; in array type").value
      this.consume("RBRACKET", "Expected ] after array type")
      return `[${innerType}; ${outerSize}]`
    }

    // Simple array type: [type] or [type; size]
    // Accept TYPE (primitive) or IDENTIFIER (struct type name)
    let innerType: string
    if (this.checkType("TYPE")) {
      innerType = this.consume("TYPE", "Expected type inside array brackets").value
    } else if (this.checkType("IDENTIFIER")) {
      innerType = this.consume("IDENTIFIER", "Expected type name inside array brackets").value
    } else {
      throw new Error("Expected type name inside array brackets")
    }

    if (this.matchType("SEMICOLON")) {
      const sizeToken = this.consume("NUMBER", "Expected size after ; in array type")
      this.consume("RBRACKET", "Expected ] after array type")
      return `[${innerType}; ${sizeToken.value}]`
    } else {
      this.consume("RBRACKET", "Expected ] after array type")
      return `[${innerType}]`
    }
  }

  private mapLiteral(): ExpressionNode {
    const start = this.previous().position.start
    const entries: { key: string; value: ExpressionNode }[] = []

    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const fieldName = this.consume("IDENTIFIER", "Expected field name in map literal")

      this.consume("COLON", "Expected : after field name")

      const value = this.expression()

      entries.push({ key: fieldName.value, value })

      this.matchType("COMMA")
    }

    this.consume("RBRACE", "Expected } after map literal")

    return {
      type: "MapLiteral",
      entries,
      position: {
        start,
        end: this.lastPosition(),
      },
    }
  }

  private parseType(typeName: string): any {
    // Handle fixed-size array types like [f32; 3], [i64; 10], or [Point; 100]
    const fixedSizeMatch = typeName.match(/^\[(\w+);\s*(\d+)\]$/);
    if (fixedSizeMatch) {
      const innerType = fixedSizeMatch[1];
      const size = parseInt(fixedSizeMatch[2], 10);
      // Check if it's a primitive type
      if (innerType.startsWith("i") || innerType.startsWith("u") || innerType.startsWith("f")) {
        let elementType: Type;
        if (innerType.startsWith("i")) {
          elementType = new IntegerType(innerType as any);
        } else if (innerType.startsWith("u")) {
          elementType = new UnsignedType(innerType as any);
        } else {
          elementType = new FloatType(innerType as any);
        }
        return new ArrayType(elementType, [size]);
      } else {
        // AoS type with struct: [Point; 100]
        return { type: "AOSType", elementType: innerType, capacity: size }
      }
    }
    
    // Handle array types that start with [ like [u8], [i32], [Point], etc.
    if (typeName.startsWith("[")) {
      const match = typeName.match(/^\[(\w+)\]$/);
      if (match) {
        const innerType = match[1];
        // Check if it's a primitive type
        if (innerType.startsWith("i") || innerType.startsWith("u") || innerType.startsWith("f")) {
          let elementType: Type;
          if (innerType.startsWith("i")) {
            elementType = new IntegerType(innerType as any);
          } else if (innerType.startsWith("u")) {
            elementType = new UnsignedType(innerType as any);
          } else {
            elementType = new FloatType(innerType as any);
          }
          return new ArrayType(elementType, []);
        } else {
          // AoS type with struct: [Point]
          return { type: "AOSType", elementType: innerType, capacity: null }
        }
      }
    }
    
    const bracketMatch = typeName.match(/^(.*?)(\[\])+$/);
    if (bracketMatch) {
      const baseName = bracketMatch[1];
      const arraySuffix = typeName.slice(baseName.length);
      
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
    if (typeName === "ptr") {
      return new PointerType()
    }
    // Handle custom struct types
    return new CustomType(typeName)
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

  private parseTemplateString(): ExpressionNode {
    const start = this.peek().position.start
    const parts: (string | ExpressionNode)[] = []

    // Consume TEMPLATE_STRING_START (contains the f" or f' prefix)
    this.advance()

    // Process string parts and interpolations
    while (!this.checkType("TEMPLATE_STRING_END") && !this.isAtEnd()) {
      if (this.checkType("TEMPLATE_STRING_PART")) {
        const strPart = this.advance()
        if (strPart.value) {
          parts.push(strPart.value)
        }
      } else if (this.checkType("TEMPLATE_INTERP_START")) {
        this.advance() // consume {

        // The expression is in the next TEMPLATE_STRING_PART token
        if (this.checkType("TEMPLATE_STRING_PART")) {
          const exprToken = this.advance()
          const exprTokens = tokenize(exprToken.value).filter(t => t.type !== "WHITESPACE")
          const exprParser = new Parser(exprTokens)
          const expr = exprParser.expression()
          parts.push(expr)
        }

        this.consume("TEMPLATE_INTERP_END", "Expected } after expression")
      } else {
        // Unexpected token, break to avoid infinite loop
        break
      }
    }

    this.consume("TEMPLATE_STRING_END", "Expected \" at end of template string")

    return {
      type: "TemplateLiteral",
      parts,
      position: {
        start,
        end: this.previous().position.end,
      },
    }
  }
}

export function parseTokens(tokens: Token[]): ProgramNode {
  const parser = new Parser(tokens)
  return parser.parse()
}

export { Parser }
