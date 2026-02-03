import { isPOSIXConstant } from "./posix_constants.js"

type TokenType =
  | "fn"
  | "return"
  | "if"
  | "else"
  | "while"
  | "for"
  | "to"
  | "in"
  | "break"
  | "continue"
  | "cast"
  | "not"
  | "MODULO"
  | "BITWISE_AND"
  | "BITWISE_OR"
  | "LOGICAL_AND"
  | "LOGICAL_OR"
  | "PLUS"
  | "MINUS"
  | "TIMES"
  | "DIVIDE"
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
  | "POSIX_CONSTANT"
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
    const commentRegex = /#.*|\/\/.*/y
    const fnRegex = /fn\b/y
    const returnRegex = /return\b/y
    const ifRegex = /if\b/y
    const elseRegex = /else\b/y
    const whileRegex = /while\b/y
    const forRegex = /for\b/y
    const toRegex = /to\b/y
    const inRegex = /in\b/y
    const breakRegex = /break\b/y
    const continueRegex = /continue\b/y
    const castRegex = /cast\b/y
    const notRegex = /not\b/y
    const llmRegex = /LLM\b/y
    const notEqualRegex = /!=/y
    const equalEqualRegex = /==/y
    const assignRegex = /:=/y
    const lessEqualRegex = /<=/y
    const greaterEqualRegex = />=/y
    const arrowRegex = /->/y
    const plusRegex = /\+/y
    const minusRegex = /-/y
    const timesRegex = /\*/y
    const divideRegex = /\//y
    const moduloRegex = /%/y
    const logicalAndRegex = /&&/y
    const logicalOrRegex = /\|\|/y
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
    const typeRegex = /\b(?:(?:i|u|f)(?:8|16|32|64|128|256)((?:\[\])*)|ptr)\b/y
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

      value = this.match(fnRegex)
      if (value) {
        type = "fn"
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
        type = "return"
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
        type = "if"
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
        type = "else"
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
        type = "while"
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
        type = "for"
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
        type = "to"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(inRegex)
      if (value) {
        type = "in"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(breakRegex)
      if (value) {
        type = "break"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(continueRegex)
      if (value) {
        type = "continue"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(castRegex)
      if (value) {
        type = "cast"
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
        type = "not"
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

      value = this.match(equalEqualRegex)
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

      value = this.match(logicalAndRegex)
      if (value) {
        type = "LOGICAL_AND"
        this.advance(value.length)
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() },
        })
        continue
      }

      value = this.match(logicalOrRegex)
      if (value) {
        type = "LOGICAL_OR"
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
        if (isPOSIXConstant(value)) {
          type = "POSIX_CONSTANT"
        } else {
          type = "IDENTIFIER"
        }
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
