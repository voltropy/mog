type TokenType =
  | "BEGIN"
  | "END"
  | "FUNCTION"
  | "RETURN"
  | "IF"
  | "THEN"
  | "ELSE"
  | "FI"
  | "WHILE"
  | "DO"
  | "OD"
  | "FOR"
  | "TO"
  | "NOT"
  | "PLUS"
  | "MINUS"
  | "TIMES"
  | "DIVIDE"
  | "MODULO"
  | "BITWISE_AND"
  | "BITWISE_OR"
  | "LESS"
  | "GREATER"
  | "EQUAL"
  | "NOT_EQUAL"
  | "LESS_EQUAL"
  | "GREATER_EQUAL"
  | "ASSIGN"
  | "LBRACKET"
  | "RBRACKET"
  | "LBRACE"
  | "RBRACE"
  | "LPAREN"
  | "RPAREN"
  | "COMMA"
  | "COLON"
  | "SEMICOLON"
  | "DOT"
  | "ARROW"
  | "LLM"
  | "IDENTIFIER"
  | "NUMBER"
  | "STRING"
  | "TYPE"
  | "UNKNOWN"
  | "WHITESPACE"
  | "COMMENT"

interface Position {
  line: number
  column: number
  index: number
}

interface Token {
  type: TokenType
  value: string
  position: {
    start: Position
    end: Position
  }
}

class Lexer {
  private input: string
  private pos: number = 0
  private line: number = 1
  private column: number = 1

  constructor(input: string) {
    this.input = input
  }

  private currentPosition(): Position {
    return { line: this.line, column: this.column, index: this.pos }
  }

  private peek(offset: number = 0): string {
    return this.input[this.pos + offset] ?? ""
  }

  private advance(n: number = 1): void {
    for (let i = 0; i < n; i++) {
      if (this.peek() === "\n") {
        this.line++
        this.column = 1
      } else if (this.peek() !== "") {
        this.column++
      }
      this.pos++
    }
  }

  private match(pattern: RegExp): string | null {
    pattern.lastIndex = this.pos
    const match = pattern.exec(this.input)
    return match ? match[0] : null
  }

  tokenize(): Token[] {
    const tokens: Token[] = []
    const whitespaceRegex = /\s+/y
    const commentRegex = /#.*/y
    const beginRegex = /BEGIN\b/y
    const endRegex = /END\b/y
    const functionRegex = /FUNCTION\b/y
    const returnRegex = /RETURN\b/y
    const ifRegex = /IF\b/y
    const thenRegex = /THEN\b/y
    const elseRegex = /ELSE\b/y
    const fiRegex = /FI\b/y
    const whileRegex = /WHILE\b/y
    const doRegex = /DO\b/y
    const odRegex = /OD\b/y
    const forRegex = /FOR\b/y
    const toRegex = /TO\b/y
    const notRegex = /not\b/y
    const llmRegex = /LLM\b/y
    const notEqualRegex = /!=/y
    const assignRegex = /:=/y
    const lessEqualRegex = /<=/y
    const greaterEqualRegex = />=/y
    const arrowRegex = /->/y
    const plusRegex = /\+/y
    const minusRegex = /-/y
    const timesRegex = /\*/y
    const divideRegex = /\//y
    const moduloRegex = /%/y
    const bitwiseAndRegex = /&/y
    const bitwiseOrRegex = /\|/y
    const lessRegex = /</y
    const greaterRegex = />/y
    const equalRegex = /=/y
    const lbracketRegex = /\[/y
    const rbracketRegex = /\]/y
    const lbraceRegex = /\{/y
    const rbraceRegex = /\}/y
    const lparenRegex = /\(/y
    const rparenRegex = /\)/y
    const commaRegex = /,/y
    const colonRegex = /:/y
    const semicolonRegex = /;/y
    const dotRegex = /\./y
    const doubleStringRegex = /"(?:[^"\\]|\\.)*"/y
    const singleStringRegex = /'(?:[^'\\]|\\.)*'/y
    const numberRegex = /\b\d+(?:\.\d*)?(?:[eE][+-]?\d+)?\b/y
    const typeRegex = /\b(?:i|u|f)(?:8|16|32|64|128|256)\b/y
    const identifierRegex = /\b[a-zA-Z_][a-zA-Z0-9_]*\b/y

    while (this.pos < this.input.length) {
      const startPos = this.currentPosition()
      let value: string | null = null
      let type: TokenType

      value = this.match(whitespaceRegex)
      if (value) {
        type = "WHITESPACE"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(commentRegex)
      if (value) {
        type = "COMMENT"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(beginRegex)
      if (value) {
        type = "BEGIN"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(endRegex)
      if (value) {
        type = "END"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(functionRegex)
      if (value) {
        type = "FUNCTION"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(returnRegex)
      if (value) {
        type = "RETURN"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(ifRegex)
      if (value) {
        type = "IF"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(thenRegex)
      if (value) {
        type = "THEN"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(elseRegex)
      if (value) {
        type = "ELSE"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(fiRegex)
      if (value) {
        type = "FI"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(whileRegex)
      if (value) {
        type = "WHILE"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(doRegex)
      if (value) {
        type = "DO"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(odRegex)
      if (value) {
        type = "OD"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(forRegex)
      if (value) {
        type = "FOR"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(toRegex)
      if (value) {
        type = "TO"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(notRegex)
      if (value) {
        type = "NOT"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(llmRegex)
      if (value) {
        type = "LLM"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(notEqualRegex)
      if (value) {
        type = "NOT_EQUAL"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(assignRegex)
      if (value) {
        type = "ASSIGN"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(arrowRegex)
      if (value) {
        type = "ARROW"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(lessEqualRegex)
      if (value) {
        type = "LESS_EQUAL"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(greaterEqualRegex)
      if (value) {
        type = "GREATER_EQUAL"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(plusRegex)
      if (value) {
        type = "PLUS"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(minusRegex)
      if (value) {
        type = "MINUS"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(timesRegex)
      if (value) {
        type = "TIMES"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(divideRegex)
      if (value) {
        type = "DIVIDE"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(moduloRegex)
      if (value) {
        type = "MODULO"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(bitwiseAndRegex)
      if (value) {
        type = "BITWISE_AND"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(bitwiseOrRegex)
      if (value) {
        type = "BITWISE_OR"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(lessRegex)
      if (value) {
        type = "LESS"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(greaterRegex)
      if (value) {
        type = "GREATER"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(equalRegex)
      if (value) {
        type = "EQUAL"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(lbracketRegex)
      if (value) {
        type = "LBRACKET"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(rbracketRegex)
      if (value) {
        type = "RBRACKET"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(lbraceRegex)
      if (value) {
        type = "LBRACE"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(rbraceRegex)
      if (value) {
        type = "RBRACE"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(lparenRegex)
      if (value) {
        type = "LPAREN"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(rparenRegex)
      if (value) {
        type = "RPAREN"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(commaRegex)
      if (value) {
        type = "COMMA"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(colonRegex)
      if (value) {
        type = "COLON"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(semicolonRegex)
      if (value) {
        type = "SEMICOLON"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(dotRegex)
      if (value) {
        type = "DOT"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(doubleStringRegex)
      if (value) {
        type = "STRING"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(singleStringRegex)
      if (value) {
        type = "STRING"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(typeRegex)
      if (value) {
        type = "TYPE"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(numberRegex)
      if (value) {
        type = "NUMBER"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(identifierRegex)
      if (value) {
        type = "IDENTIFIER"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      tokens.push({
        type: "UNKNOWN",
        value: this.peek(),
        position: { start: startPos, end: this.currentPosition() },
      })
      this.advance(1)
    }

    return tokens
  }
}

export function tokenize(input: string): Token[] {
  const lexer = new Lexer(input)
  return lexer.tokenize()
}

export { Lexer }
export type { Token, TokenType, Position }
