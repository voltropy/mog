import type { Token } from "./lexer.js"
import { tokenize } from "./lexer.js"
import type { ProgramNode, StatementNode, ExpressionNode, Position, BlockNode } from "./analyzer.js"
import { IntegerType, UnsignedType, FloatType, BoolType, TypeAliasType, ArrayType, MapType, PointerType, VoidType, CustomType, FunctionType, TensorType, ResultType, OptionalType } from "./types.js"
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
  private allowStructLiteral = true

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

    if (this.matchType("requires")) {
      return this.requiresDeclaration()
    }
    if (this.matchType("optional_kw")) {
      return this.optionalDeclaration()
    }

    if (this.matchType("LBRACE")) {
      return this.blockStatement()
    }
    if (this.checkType("fn") && this.peekNext()?.type === "IDENTIFIER") {
      this.advance() // consume 'fn'
      return this.functionDeclaration()
    }
    if (this.matchType("struct")) {
      return this.structDeclaration()
    }

    if (this.matchType("type_kw")) {
      return this.typeAliasDeclaration()
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
    if (this.matchType("try")) {
      return this.tryCatchStatement()
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

  private parseFunctionParams(): any[] {
    const params: any[] = []
    if (!this.checkType("RPAREN")) {
      do {
        const paramName = this.consume("IDENTIFIER", "Expected parameter name").value
        this.consume("COLON", "Expected : after parameter name")
        // Accept both TYPE and IDENTIFIER tokens as parameter types (for custom struct types)
        // Also handle function types: fn(int) -> int
        let paramType: any
        if (this.checkType("fn")) {
          paramType = this.parseFunctionTypeAnnotation()
        } else if (this.checkType("LBRACKET")) {
          const arrTypeName = this.parseArrayTypeAnnotation()
          paramType = this.parseType(arrTypeName)
        } else {
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
          paramType = this.parseType(typeName)
        }

        // Check for default value: param: type = defaultValue
        let defaultValue: ExpressionNode | null = null
        if (this.checkType("EQUAL") && this.peek().value === "=") {
          this.advance() // consume =
          defaultValue = this.expression()
        }

        params.push({ name: paramName, paramType, defaultValue })
      } while (this.matchType("COMMA"))
    }
    return params
  }

  private parseReturnType(): any {
    // Check for function type return: fn(int) -> int
    if (this.checkType("fn")) {
      return this.parseFunctionTypeAnnotation()
    }
    // Check for ?T optional type
    if (this.matchType("QUESTION_MARK")) {
      const innerType = this.parseReturnType()
      return new OptionalType(innerType)
    }
    // Check for tensor<T> type
    if (this.checkType("tensor")) {
      return this.parseTensorTypeAnnotation()
    }
    // Accept both TYPE and IDENTIFIER tokens as return types (for custom struct types)
    let returnToken: any
    if (this.checkType("TYPE")) {
      returnToken = this.consume("TYPE", "Expected return type")
    } else {
      returnToken = this.consume("IDENTIFIER", "Expected return type")
    }
    let returnTypeName = returnToken.value
    // Check for Result<T> type
    if (returnTypeName === "Result" && this.matchType("LESS")) {
      const innerType = this.parseReturnType()
      this.consume("GREATER", "Expected > after Result<T>")
      return new ResultType(innerType)
    }
    while (this.matchType("LBRACKET")) {
      returnTypeName += "["
      this.consume("RBRACKET", "Expected ] after array bracket")
      returnTypeName += "]"
    }
    return this.parseType(returnTypeName)
  }

  private parseFunctionTypeAnnotation(): any {
    // Parse fn(paramTypes) -> returnType as a FunctionType
    this.consume("fn", "Expected fn keyword")
    this.consume("LPAREN", "Expected ( after fn")
    const paramTypes: any[] = []
    if (!this.checkType("RPAREN")) {
      do {
        // In function type annotations, params are just types (no names)
        // But we also allow name: type syntax for clarity
        let paramType: any
        if (this.checkType("fn")) {
          paramType = this.parseFunctionTypeAnnotation()
        } else if (this.checkType("LBRACKET")) {
          const arrTypeName = this.parseArrayTypeAnnotation()
          paramType = this.parseType(arrTypeName)
        } else {
          let typeToken: any
          if (this.checkType("TYPE")) {
            typeToken = this.consume("TYPE", "Expected type")
          } else {
            typeToken = this.consume("IDENTIFIER", "Expected type")
          }
          let typeName = typeToken.value
          while (this.matchType("LBRACKET")) {
            typeName += "["
            this.consume("RBRACKET", "Expected ] after array bracket")
            typeName += "]"
          }
          paramType = this.parseType(typeName)
        }
        paramTypes.push(paramType)
      } while (this.matchType("COMMA"))
    }
    this.consume("RPAREN", "Expected ) after fn param types")
    this.consume("ARROW", "Expected -> after fn param types")
    const returnType = this.parseReturnType()
    return new FunctionType(paramTypes, returnType)
  }

  private parseFunctionBody(): BlockNode {
    this.consume("LBRACE", "Expected { after function signature")

    const bodyStatements: StatementNode[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement()
      if (stmt) {
        bodyStatements.push(stmt)
      }
    }
    this.consume("RBRACE", "Expected } after function body")

    // Single-expression body: if there's only one ExpressionStatement,
    // wrap it as an implicit return
    if (bodyStatements.length === 1 && bodyStatements[0].type === "ExpressionStatement") {
      const exprStmt = bodyStatements[0] as any
      const implicitReturn: StatementNode = {
        type: "Return",
        value: exprStmt.expression,
        position: exprStmt.position,
      } as any
      return {
        type: "Block",
        statements: [implicitReturn],
        position: {
          start: { line: 1, column: 1, index: 0 },
          end: this.lastPosition(),
        },
      } as any
    }

    return {
      type: "Block",
      statements: bodyStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    } as any
  }

  private functionDeclaration(): StatementNode {
    const name = this.consume("IDENTIFIER", "Expected function name").value
    this.consume("LPAREN", "Expected ( after function name")

    const params = this.parseFunctionParams()
    this.consume("RPAREN", "Expected ) after parameters")

    // Return type is optional - if no ->, default to void
    let returnType: any = new VoidType()
    if (this.matchType("ARROW")) {
      returnType = this.parseReturnType()
    }

    const body = this.parseFunctionBody()

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
      } else if (this.checkType("TYPE")) {
        const typeName = this.consume("TYPE", "Expected type after :").value
        fieldType = this.parseType(typeName)
      } else {
        const typeName = this.consume("IDENTIFIER", "Expected type after :").value
        fieldType = this.parseType(typeName)
      }
      fields.push({ name: fieldName, fieldType })
      this.matchType("COMMA") || this.matchType("SEMICOLON")
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
    // Support both (condition) and condition { styles
    let condition: ExpressionNode
    if (this.matchType("LPAREN")) {
      condition = this.expression()
      this.consume("RPAREN", "Expected ) after condition")
    } else {
      // Parse condition up to the opening brace - disable struct literal so { starts the block
      const prevAllow = this.allowStructLiteral
      this.allowStructLiteral = false
      condition = this.expression()
      this.allowStructLiteral = prevAllow
    }

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
      // Support else if chaining
      if (this.checkType("if")) {
        this.advance() // consume 'if'
        const nestedIf = this.ifStatement()
        // Wrap the nested if in a block
        falseBranch = {
          type: "Block",
          statements: [nestedIf],
          position: nestedIf.position,
        }
      } else {
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

  private parseForBody(): BlockNode {
    this.consume("LBRACE", "Expected { after for header")
    const bodyStatements: StatementNode[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement()
      if (stmt) {
        bodyStatements.push(stmt)
      }
    }
    this.consume("RBRACE", "Expected } after for loop")
    return {
      type: "Block",
      statements: bodyStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private forStatement(): StatementNode {
    const variable = this.consume("IDENTIFIER", "Expected variable name after for").value

    // Check if this is a for-each loop with type annotation: for item: type in arr { ... }
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
      const prevAllow = this.allowStructLiteral
      this.allowStructLiteral = false
      const array = this.expression()
      this.allowStructLiteral = prevAllow
      const body = this.parseForBody()

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

    // Check for comma: for i, item in ... (index+value or map key+value)
    if (this.matchType("COMMA")) {
      const secondVar = this.consume("IDENTIFIER", "Expected second variable name after comma").value
      this.consume("in", "Expected 'in' after variable names")
      const prevAllow = this.allowStructLiteral
      this.allowStructLiteral = false
      const iterable = this.expression()
      this.allowStructLiteral = prevAllow
      const body = this.parseForBody()

      // We'll determine ForInIndex vs ForInMap at analysis/codegen time based on the iterable type
      // For now, parse as ForInIndex (works for both arrays and maps)
      return {
        type: "ForInIndex",
        indexVariable: variable,
        valueVariable: secondVar,
        iterable,
        body,
        position: {
          start: { line: 1, column: 1, index: 0 },
          end: this.lastPosition(),
        },
      }
    }

    // Check for `in` keyword: for item in ... (foreach without type annotation, or range)
    if (this.matchType("in")) {
      // Parse the expression after `in` - disable struct literal so { starts the body block
      const prevAllow = this.allowStructLiteral
      this.allowStructLiteral = false
      const iterableExpr = this.expression()
      this.allowStructLiteral = prevAllow

      // Check if the next token is `..` (RANGE) indicating a range loop
      if (this.matchType("RANGE")) {
        // for i in start..end { ... } - disable struct literal for end expression too
        const prevAllow2 = this.allowStructLiteral
        this.allowStructLiteral = false
        const endExpr = this.expression()
        this.allowStructLiteral = prevAllow2
        const body = this.parseForBody()
        return {
          type: "ForInRange",
          variable,
          start: iterableExpr,
          end: endExpr,
          body,
          position: {
            start: { line: 1, column: 1, index: 0 },
            end: this.lastPosition(),
          },
        }
      }

      // for item in array { ... } (no type annotation)
      const body = this.parseForBody()
      return {
        type: "ForEachLoop",
        variable,
        varType: null,
        array: iterableExpr,
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
    const body = this.parseForBody()

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

  private tryCatchStatement(): StatementNode {
    this.consume("LBRACE", "Expected { after try")
    const tryStatements: StatementNode[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement()
      if (stmt) {
        tryStatements.push(stmt)
      }
    }
    this.consume("RBRACE", "Expected } after try body")

    const tryBody: BlockNode = {
      type: "Block",
      statements: tryStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    } as any

    this.consume("catch", "Expected catch after try block")
    this.consume("LPAREN", "Expected ( after catch")
    const errorVar = this.consume("IDENTIFIER", "Expected error variable name").value
    this.consume("RPAREN", "Expected ) after catch variable")

    this.consume("LBRACE", "Expected { after catch(...)")
    const catchStatements: StatementNode[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement()
      if (stmt) {
        catchStatements.push(stmt)
      }
    }
    this.consume("RBRACE", "Expected } after catch body")

    const catchBody: BlockNode = {
      type: "Block",
      statements: catchStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    } as any

    return {
      type: "TryCatch" as any,
      tryBody,
      errorVar,
      catchBody,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    } as any
  }

  private requiresDeclaration(): StatementNode {
    const startPos = this.previous().position?.start || { line: 1, column: 1, index: 0 }
    const capabilities: string[] = []
    capabilities.push(this.consume("IDENTIFIER", "Expected capability name").value)
    while (this.matchType("COMMA")) {
      capabilities.push(this.consume("IDENTIFIER", "Expected capability name").value)
    }
    this.matchType("SEMICOLON")
    return {
      type: "RequiresDeclaration" as any,
      capabilities,
      position: {
        start: startPos,
        end: this.lastPosition(),
      },
    } as any
  }

  private optionalDeclaration(): StatementNode {
    const startPos = this.previous().position?.start || { line: 1, column: 1, index: 0 }
    const capabilities: string[] = []
    capabilities.push(this.consume("IDENTIFIER", "Expected capability name").value)
    while (this.matchType("COMMA")) {
      capabilities.push(this.consume("IDENTIFIER", "Expected capability name").value)
    }
    this.matchType("SEMICOLON")
    return {
      type: "OptionalDeclaration" as any,
      capabilities,
      position: {
        start: startPos,
        end: this.lastPosition(),
      },
    } as any
  }

  private typeAliasDeclaration(): StatementNode {
    const startPos = this.lastPosition()
    const name = this.consume("IDENTIFIER", "Expected type alias name").value
    this.consume("EQUAL", "Expected = after type alias name")

    // Parse the aliased type
    let typeName: string
    if (this.checkType("LBRACE")) {
      // Map type: type Config = {string: string};
      this.advance() // consume {
      let keyTypeName: string
      if (this.checkType("TYPE")) {
        keyTypeName = this.consume("TYPE", "Expected key type").value
      } else {
        keyTypeName = this.consume("IDENTIFIER", "Expected key type").value
      }
      this.consume("COLON", "Expected : after key type")
      let valueTypeName: string
      if (this.checkType("TYPE")) {
        valueTypeName = this.consume("TYPE", "Expected value type").value
      } else {
        valueTypeName = this.consume("IDENTIFIER", "Expected value type").value
      }
      this.consume("RBRACE", "Expected } after map type")
      this.matchType("SEMICOLON")

      const keyType = this.parseType(keyTypeName)
      const valueType = this.parseType(valueTypeName)
      const mapType = new MapType(keyType, valueType)

      return {
        type: "TypeAliasDeclaration",
        name,
        aliasedType: new TypeAliasType(name, mapType),
        position: {
          start: startPos,
          end: this.lastPosition(),
        },
      } as any
    }

    // Handle function type: type Callback = fn(int) -> bool;
    if (this.checkType("fn")) {
      const funcType = this.parseFunctionTypeAnnotation()
      this.matchType("SEMICOLON")
      return {
        type: "TypeAliasDeclaration",
        name,
        aliasedType: new TypeAliasType(name, funcType),
        position: {
          start: startPos,
          end: this.lastPosition(),
        },
      } as any
    }

    if (this.checkType("LBRACKET")) {
      typeName = this.parseArrayTypeAnnotation()
    } else if (this.checkType("TYPE")) {
      typeName = this.consume("TYPE", "Expected type").value
    } else {
      typeName = this.consume("IDENTIFIER", "Expected type").value
    }

    while (this.matchType("LBRACKET")) {
      typeName += "["
      this.consume("RBRACKET", "Expected ] after array bracket")
      typeName += "]"
    }

    this.matchType("SEMICOLON")

    const aliasedType = this.parseType(typeName)

    return {
      type: "TypeAliasDeclaration",
      name,
      aliasedType: new TypeAliasType(name, aliasedType),
      position: {
        start: startPos,
        end: this.lastPosition(),
      },
    } as any
  }

  private variableDeclaration(): StatementNode {
    const name = this.consume("IDENTIFIER", "Expected variable name").value
    this.consume("COLON", "Expected : after variable name")
    
    // Handle array types that start with [ (e.g., [u8] or [f32; 3])
    let typeName: string
    let declaredType: { name: string; type: string } | null = null
    let varType: any

    if (this.checkType("fn")) {
      // Function type annotation: callback: fn(int) -> int = ...
      varType = this.parseFunctionTypeAnnotation()
      typeName = varType.toString()
    } else if (this.checkType("tensor")) {
      // Tensor type annotation: t: tensor<f32> = ...
      varType = this.parseTensorTypeAnnotation()
      typeName = varType.toString()
    } else if (this.checkType("LBRACKET")) {
      // Parse array type like [u8], [i32], [f32; 3], or nested [[f64; 3]; 2], etc.
      typeName = this.parseArrayTypeAnnotation()
      varType = this.parseType(typeName)
    } else if (this.checkType("TYPE")) {
      let typeToken = this.consume("TYPE", "Expected type annotation")
      typeName = typeToken.value
      // Handle trailing [] for array of type (e.g., f64[][])
      while (this.matchType("LBRACKET")) {
        typeName += "[]"
        this.consume("RBRACKET", "Expected ] after array bracket")
      }
      varType = this.parseType(typeName)
    } else if (this.checkType("soa")) {
      // SoA type annotation: name: soa Struct[N]
      this.advance() // consume 'soa'
      const structName = this.consume("IDENTIFIER", "Expected struct name after soa").value
      this.consume("LBRACKET", "Expected [ after struct name")
      let capacity: number | null = null
      if (this.checkType("NUMBER")) {
        capacity = parseInt(this.consume("NUMBER", "Expected capacity").value, 10)
      }
      this.consume("RBRACKET", "Expected ] after capacity")
      varType = { type: "SOAType", structName, capacity }
      typeName = `soa ${structName}[${capacity ?? ''}]`
    } else if (this.checkType("IDENTIFIER")) {
      // Handle custom type names (e.g., Point, Particle)
      let typeToken = this.consume("IDENTIFIER", "Expected type annotation")
      typeName = typeToken.value
      declaredType = { name: typeName, type: "CustomType" }
      // Handle trailing [] for array of type (e.g., f64[][])
      while (this.matchType("LBRACKET")) {
        typeName += "[]"
        this.consume("RBRACKET", "Expected ] after array bracket")
      }
      varType = this.parseType(typeName)
    } else {
      throw new Error("Expected type annotation")
    }
    
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

    // Support both := and = for reassignment
    const isAssign = this.matchType("ASSIGN")
    const isEqualAssign = !isAssign && this.checkType("EQUAL") && this.peek().value === "="
    if (isEqualAssign) this.advance()

    if (isAssign || isEqualAssign) {
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

    while ((this.checkType("EQUAL") && this.peek().value === "==") || this.checkType("NOT_EQUAL")) {
      this.advance()
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

    // Handle `expr is some(name)` or `expr is none` pattern matching
    if (this.matchType("is")) {
      if (this.matchType("some")) {
        this.consume("LPAREN", "Expected ( after some in is-pattern")
        const binding = this.consume("IDENTIFIER", "Expected binding name in is some(...)").value
        this.consume("RPAREN", "Expected ) after binding name in is some(...)")
        return {
          type: "IsSomeExpression" as any,
          value: expr,
          binding,
          position: this.combinePositions(expr, { position: this.previous().position }),
        } as any
      } else if (this.matchType("none")) {
        return {
          type: "IsNoneExpression" as any,
          value: expr,
          position: this.combinePositions(expr, { position: this.previous().position }),
        } as any
      } else if (this.matchType("ok")) {
        this.consume("LPAREN", "Expected ( after ok in is-pattern")
        const binding = this.consume("IDENTIFIER", "Expected binding name in is ok(...)").value
        this.consume("RPAREN", "Expected ) after binding name in is ok(...)")
        return {
          type: "IsOkExpression" as any,
          value: expr,
          binding,
          position: this.combinePositions(expr, { position: this.previous().position }),
        } as any
      } else if (this.matchType("err")) {
        this.consume("LPAREN", "Expected ( after err in is-pattern")
        const binding = this.consume("IDENTIFIER", "Expected binding name in is err(...)").value
        this.consume("RPAREN", "Expected ) after binding name in is err(...)")
        return {
          type: "IsErrExpression" as any,
          value: expr,
          binding,
          position: this.combinePositions(expr, { position: this.previous().position }),
        } as any
      }
    }

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

    let expr = this.primary()

    // Handle `?` postfix operator for error propagation
    if (this.matchType("QUESTION_MARK")) {
      expr = {
        type: "PropagateExpression" as any,
        value: expr,
        position: this.combinePositions(expr, { position: this.previous().position }),
      } as any
    }

    // Handle `expr as Type` postfix cast syntax
    while (this.matchType("as")) {
      let typeName: string
      if (this.checkType("TYPE")) {
        typeName = this.consume("TYPE", "Expected type name after as").value
      } else {
        typeName = this.consume("IDENTIFIER", "Expected type name after as").value
      }
      while (this.matchType("LBRACKET")) {
        typeName += "["
        this.consume("RBRACKET", "Expected ] after array bracket")
        typeName += "]"
      }
      const targetType = this.parseType(typeName)
      expr = {
        type: "CastExpression",
        targetType,
        value: expr,
        position: this.combinePositions(expr, expr),
      }
    }

    return expr
  }

  private primary(): ExpressionNode {
    // SoA constructor expression: soa Struct[N]
    if (this.matchType("soa")) {
      const structName = this.consume("IDENTIFIER", "Expected struct name after soa").value
      this.consume("LBRACKET", "Expected [ after struct name")
      let capacity: number | null = null
      if (this.checkType("NUMBER")) {
        capacity = parseInt(this.consume("NUMBER", "Expected capacity").value, 10)
      }
      this.consume("RBRACKET", "Expected ] after capacity")
      return {
        type: "SoAConstructor",
        structName,
        capacity,
        position: {
          start: { line: 1, column: 1, index: 0 },
          end: this.lastPosition(),
        },
      } as any
    }

    // If expression: if condition { expr } else { expr }
    if (this.matchType("if")) {
      return this.ifExpression()
    }

    // Match expression: match expr { pattern => expr, ... }
    if (this.matchType("match")) {
      return this.matchExpression()
    }

    // Anonymous function expression: fn(params) -> type { body }
    if (this.matchType("fn")) {
      return this.anonymousFunction()
    }

    // ok(expr) - Result ok constructor
    if (this.matchType("ok")) {
      const token = this.previous()
      this.consume("LPAREN", "Expected ( after ok")
      const value = this.expression()
      this.consume("RPAREN", "Expected ) after ok value")
      return {
        type: "OkExpression" as any,
        value,
        position: this.combinePositions({ position: token.position }, value),
      } as any
    }

    // err(expr) - Result err constructor
    if (this.matchType("err")) {
      const token = this.previous()
      this.consume("LPAREN", "Expected ( after err")
      const value = this.expression()
      this.consume("RPAREN", "Expected ) after err value")
      return {
        type: "ErrExpression" as any,
        value,
        position: this.combinePositions({ position: token.position }, value),
      } as any
    }

    // some(expr) - Optional some constructor
    if (this.matchType("some")) {
      const token = this.previous()
      this.consume("LPAREN", "Expected ( after some")
      const value = this.expression()
      this.consume("RPAREN", "Expected ) after some value")
      return {
        type: "SomeExpression" as any,
        value,
        position: this.combinePositions({ position: token.position }, value),
      } as any
    }

    // none - Optional none value
    if (this.matchType("none")) {
      const token = this.previous()
      return {
        type: "NoneExpression" as any,
        position: token.position,
      } as any
    }

    if (this.matchType("false")) {
      const token = this.previous()
      return {
        type: "BooleanLiteral",
        value: false,
        position: token.position,
      }
    }

    if (this.matchType("true")) {
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

    // tensor<dtype>(shape) construction expression
    if (this.matchType("tensor")) {
      const token = this.previous()
      this.consume("LESS", "Expected < after tensor")
      let dtypeName: string
      if (this.checkType("TYPE")) {
        dtypeName = this.consume("TYPE", "Expected dtype").value
      } else {
        dtypeName = this.consume("IDENTIFIER", "Expected dtype").value
      }
      this.consume("GREATER", "Expected > after tensor dtype")
      const dtype = this.parsePrimitiveType(dtypeName)
      if (!dtype) {
        throw new Error(`Invalid tensor dtype: ${dtypeName}`)
      }
      // Parse optional constructor arguments: tensor<f32>([2, 3])
      let args: ExpressionNode[] = []
      if (this.matchType("LPAREN")) {
        if (!this.checkType("RPAREN")) {
          do {
            args.push(this.expression())
          } while (this.matchType("COMMA"))
        }
        this.consume("RPAREN", "Expected ) after tensor arguments")
      }

      let object: ExpressionNode = {
        type: "TensorConstruction" as any,
        dtype,
        args,
        position: {
          start: token.position.start,
          end: this.lastPosition(),
        },
      } as any

      // Parse chained postfix operations: .method()
      let done = false
      while (!done) {
        if (this.matchType("DOT")) {
          const property = this.consume("IDENTIFIER", "Expected property name").value
          object = {
            type: "MemberExpression",
            object,
            property,
            position: this.combinePositions(object, this.previous()),
          }
          // Handle method call: .matmul(other)
          if (this.matchType("LPAREN")) {
            const callArgs: ExpressionNode[] = []
            if (!this.checkType("RPAREN")) {
              do {
                callArgs.push(this.expression())
              } while (this.matchType("COMMA"))
            }
            this.consume("RPAREN", "Expected ) after arguments")
            object = {
              type: "CallExpression",
              callee: object,
              args: callArgs,
              position: this.combinePositions(object, this.previous()),
            } as any
          }
        } else {
          done = true
        }
      }

      return object
    }

    if (this.matchType("IDENTIFIER")) {
      const token = this.previous()

      // Check for struct literal: TypeName { field: value, ... }
      if (this.allowStructLiteral && this.matchType("LBRACE")) {
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
          const namedArgs: { name: string; value: ExpressionNode }[] = []
          if (!this.checkType("RPAREN")) {
            do {
              // Check for named argument: identifier followed by COLON
              // But only if the next token after COLON is not a type keyword
              // (to distinguish from struct literal or type annotations)
              if (this.checkType("IDENTIFIER") && this.peekNext()?.type === "COLON") {
                // Could be named arg or just an expression with ternary
                // Save position and try named arg parse
                const savedPos = this.current
                const argName = this.consume("IDENTIFIER", "").value
                this.consume("COLON", "")
                // If the next thing is a valid expression start, treat as named arg
                // But only if we already have some positional args or previous named args
                // Named args can also be the first args (mixed with positional)
                const argValue = this.expression()
                namedArgs.push({ name: argName, value: argValue })
              } else {
                callArgs.push(this.expression())
              }
            } while (this.matchType("COMMA"))
          }
          this.consume("RPAREN", "Expected ) after arguments")

          object = {
            type: "CallExpression",
            callee: (object.type === "Identifier" || object.type === "MemberExpression") ? object : { type: "Identifier", name: (object as any).name, position: token.position },
            args: callArgs,
            namedArgs: namedArgs.length > 0 ? namedArgs : undefined,
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

  private anonymousFunction(): ExpressionNode {
    // fn(params) -> type { body }
    // The 'fn' token has already been consumed
    this.consume("LPAREN", "Expected ( after fn")
    const params = this.parseFunctionParams()
    this.consume("RPAREN", "Expected ) after parameters")

    // Return type after ->
    let returnType: any = new VoidType()
    if (this.matchType("ARROW")) {
      returnType = this.parseReturnType()
    }

    const body = this.parseFunctionBody()

    return {
      type: "Lambda",
      params,
      returnType,
      body,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    } as any
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

  private isPrimitiveTypeName(name: string): boolean {
    return name === "int" || name === "float" || name === "bool" || name === "bf16" ||
           name === "ptr" || name === "string" ||
           /^(?:i|u|f)(?:8|16|32|64|128|256)$/.test(name)
  }

  private parsePrimitiveType(name: string): any {
    if (name === "int") return new IntegerType("i64")
    if (name === "float") return new FloatType("f64")
    if (name === "bool") return new BoolType()
    if (name === "bf16") return new FloatType("bf16")
    if (name.startsWith("i")) return new IntegerType(name as any)
    if (name.startsWith("u")) return new UnsignedType(name as any)
    if (name.startsWith("f")) return new FloatType(name as any)
    if (name === "ptr") return new PointerType()
    return null
  }

  private parseTensorTypeAnnotation(): TensorType {
    this.consume("tensor", "Expected tensor keyword")
    this.consume("LESS", "Expected < after tensor")
    let dtypeName: string
    if (this.checkType("TYPE")) {
      dtypeName = this.consume("TYPE", "Expected dtype").value
    } else {
      dtypeName = this.consume("IDENTIFIER", "Expected dtype").value
    }
    this.consume("GREATER", "Expected > after tensor dtype")
    const dtype = this.parsePrimitiveType(dtypeName)
    if (!dtype) {
      throw new Error(`Invalid tensor dtype: ${dtypeName}`)
    }
    return new TensorType(dtype)
  }

  private parseType(typeName: string): any {
    // Handle fixed-size array types like [f32; 3], [i64; 10], or [Point; 100]
    const fixedSizeMatch = typeName.match(/^\[(\w+);\s*(\d+)\]$/);
    if (fixedSizeMatch) {
      const innerType = fixedSizeMatch[1];
      const size = parseInt(fixedSizeMatch[2], 10);
      const elementType = this.parsePrimitiveType(innerType);
      if (elementType) {
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
        const elementType = this.parsePrimitiveType(innerType);
        if (elementType) {
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
      
      const elementType = this.parsePrimitiveType(baseName);
      if (!elementType) {
        return new VoidType();
      }
      
      let currentType = elementType;
      const bracketCount = (arraySuffix.match(/\[/g) || []).length;
      for (let i = 0; i < bracketCount; i++) {
        currentType = new ArrayType(currentType, []);
      }
      return currentType;
    }
    
    const primitive = this.parsePrimitiveType(typeName);
    if (primitive) return primitive;

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

  private ifExpression(): ExpressionNode {
    // Parse condition - support both (condition) and condition { styles
    let condition: ExpressionNode
    if (this.matchType("LPAREN")) {
      condition = this.expression()
      this.consume("RPAREN", "Expected ) after condition")
    } else {
      // Parse condition up to the opening brace - disable struct literal so { starts the block
      const prevAllow = this.allowStructLiteral
      this.allowStructLiteral = false
      condition = this.expression()
      this.allowStructLiteral = prevAllow
    }

    // Parse true branch block
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

    // Parse else branch (required for if-expression)
    let falseBranch: any = null
    if (this.matchType("else")) {
      if (this.matchType("if")) {
        // else if - recursive
        falseBranch = this.ifExpression()
      } else {
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
    }

    return {
      type: "IfExpression",
      condition,
      trueBranch,
      falseBranch,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
    }
  }

  private matchExpression(): ExpressionNode {
    // Parse the subject expression - disable struct literal so { starts the match body
    const prevAllow = this.allowStructLiteral
    this.allowStructLiteral = false
    const subject = this.expression()
    this.allowStructLiteral = prevAllow

    this.consume("LBRACE", "Expected { after match subject")

    const arms: any[] = []
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      // Parse pattern
      let pattern: any

      if (this.matchType("UNDERSCORE")) {
        // Wildcard pattern: _
        pattern = { type: "WildcardPattern" }
      } else if (
        (this.checkType("IDENTIFIER") || this.checkType("ok") || this.checkType("err") || this.checkType("some") || this.checkType("none"))
        && this.peekNext()?.type === "LPAREN"
      ) {
        // Variant pattern: ok(val), err(msg), some(x), none()
        const nameToken = this.advance()
        const name = nameToken.value
        this.consume("LPAREN", "Expected ( after variant name")
        let binding: string | null = null
        if (!this.checkType("RPAREN")) {
          binding = this.consume("IDENTIFIER", "Expected binding variable").value
        }
        this.consume("RPAREN", "Expected ) after variant binding")
        pattern = { type: "VariantPattern", name, binding }
      } else if (this.checkType("none") && this.peekNext()?.type !== "LPAREN") {
        // Bare `none` pattern (without parentheses)
        this.advance()
        pattern = { type: "VariantPattern", name: "none", binding: null }
      } else {
        // Literal pattern (number, string, identifier, boolean)
        const value = this.expression()
        pattern = { type: "LiteralPattern", value }
      }

      this.consume("FAT_ARROW", "Expected => after pattern")

      // Parse arm body - can be a single expression or a block
      let body: ExpressionNode
      if (this.checkType("LBRACE")) {
        this.advance() // consume {
        const stmts: StatementNode[] = []
        while (!this.checkType("RBRACE") && !this.isAtEnd()) {
          const stmt = this.statement()
          if (stmt) {
            stmts.push(stmt)
          }
        }
        this.consume("RBRACE", "Expected } after match arm body")
        // Convert block to an expression - use last expression
        if (stmts.length > 0) {
          const last = stmts[stmts.length - 1]
          if (last.type === "ExpressionStatement") {
            body = (last as any).expression
          } else {
            body = { type: "NumberLiteral", value: "0", position: last.position, literalType: null } as any
          }
        } else {
          body = { type: "NumberLiteral", value: "0", position: subject.position, literalType: null } as any
        }
      } else {
        body = this.expression()
      }

      arms.push({ pattern, body })

      // Arms separated by commas (optional trailing comma)
      this.matchType("COMMA")
    }

    this.consume("RBRACE", "Expected } after match expression")

    return {
      type: "MatchExpression",
      subject,
      arms,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition(),
      },
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
