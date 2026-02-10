/**
 * Capability declaration parser for .mogdecl files.
 *
 * Parses declarations like:
 *   capability fs {
 *     fn read(path: string) -> string;
 *     fn write(path: string, content: string);
 *   }
 */

export interface CapabilityFuncDecl {
  name: string
  params: { name: string; type: string; optional: boolean }[]
  returnType: string | null // null = void
  isAsync: boolean
}

export interface CapabilityDecl {
  name: string
  functions: CapabilityFuncDecl[]
}

// Simple tokenizer for .mogdecl files
interface DeclToken {
  type: "keyword" | "ident" | "punct" | "eof"
  value: string
}

function tokenizeMogDecl(source: string): DeclToken[] {
  const tokens: DeclToken[] = []
  let i = 0

  while (i < source.length) {
    // Skip whitespace
    if (/\s/.test(source[i])) {
      i++
      continue
    }

    // Skip comments (// and #)
    if ((source[i] === "/" && source[i + 1] === "/") || source[i] === "#") {
      while (i < source.length && source[i] !== "\n") i++
      continue
    }

    // Punctuation
    if ("{};,()->?<>:".includes(source[i])) {
      // Handle -> as a single token
      if (source[i] === "-" && source[i + 1] === ">") {
        tokens.push({ type: "punct", value: "->" })
        i += 2
        continue
      }
      tokens.push({ type: "punct", value: source[i] })
      i++
      continue
    }

    // Identifiers / keywords
    if (/[a-zA-Z_]/.test(source[i])) {
      let start = i
      while (i < source.length && /[a-zA-Z0-9_]/.test(source[i])) i++
      const word = source.slice(start, i)
      const keywords = ["capability", "fn", "async", "Result"]
      tokens.push({
        type: keywords.includes(word) ? "keyword" : "ident",
        value: word,
      })
      continue
    }

    // Skip unknown characters
    i++
  }

  tokens.push({ type: "eof", value: "" })
  return tokens
}

class DeclParser {
  private tokens: DeclToken[]
  private pos = 0

  constructor(tokens: DeclToken[]) {
    this.tokens = tokens
  }

  private peek(): DeclToken {
    return this.tokens[this.pos] || { type: "eof", value: "" }
  }

  private advance(): DeclToken {
    const tok = this.tokens[this.pos]
    this.pos++
    return tok
  }

  private expect(type: string, value?: string): DeclToken {
    const tok = this.advance()
    if (tok.type !== type || (value !== undefined && tok.value !== value)) {
      throw new Error(
        `Expected ${type}${value ? ` '${value}'` : ""}, got ${tok.type} '${tok.value}'`,
      )
    }
    return tok
  }

  parse(): CapabilityDecl[] {
    const decls: CapabilityDecl[] = []
    while (this.peek().type !== "eof") {
      if (this.peek().value === "capability") {
        decls.push(this.parseCapability())
      } else {
        // Skip unknown tokens
        this.advance()
      }
    }
    return decls
  }

  private parseCapability(): CapabilityDecl {
    this.expect("keyword", "capability")
    const name = this.expect("ident").value
    this.expect("punct", "{")

    const functions: CapabilityFuncDecl[] = []
    while (this.peek().value !== "}") {
      if (this.peek().type === "eof") break
      functions.push(this.parseFunction())
    }

    this.expect("punct", "}")
    return { name, functions }
  }

  private parseFunction(): CapabilityFuncDecl {
    let isAsync = false
    if (this.peek().value === "async") {
      this.advance()
      isAsync = true
    }

    this.expect("keyword", "fn")
    const name = this.expect("ident").value
    this.expect("punct", "(")

    const params: { name: string; type: string; optional: boolean }[] = []
    while (this.peek().value !== ")") {
      if (params.length > 0) {
        this.expect("punct", ",")
      }
      const paramName = this.expect("ident").value
      this.expect("punct", ":")

      let optional = false
      let paramType = ""

      // Check for ?Type (optional)
      if (this.peek().value === "?") {
        this.advance()
        optional = true
      }

      paramType = this.parseTypeName()
      params.push({ name: paramName, type: paramType, optional })
    }

    this.expect("punct", ")")

    // Parse return type
    let returnType: string | null = null
    if (this.peek().value === "->") {
      this.advance()
      returnType = this.parseTypeName()
    }

    // Consume optional semicolon
    if (this.peek().value === ";") {
      this.advance()
    }

    return { name, params, returnType, isAsync }
  }

  private parseTypeName(): string {
    // Handle Result<T>
    if (this.peek().value === "Result") {
      this.advance()
      if (this.peek().value === "<") {
        this.advance()
        const inner = this.parseTypeName()
        this.expect("punct", ">")
        return `Result<${inner}>`
      }
      return "Result"
    }

    const name = this.expect("ident").value
    return name
  }
}

/**
 * Parse a .mogdecl file and return capability declarations.
 */
export function parseCapabilityDecl(source: string): CapabilityDecl[] {
  const tokens = tokenizeMogDecl(source)
  const parser = new DeclParser(tokens)
  return parser.parse()
}

/**
 * Parse a single capability from a .mogdecl file (convenience function).
 * Returns the first capability found, or null if none.
 */
export function parseCapability(source: string): CapabilityDecl | null {
  const decls = parseCapabilityDecl(source)
  return decls.length > 0 ? decls[0] : null
}

/**
 * Map a mogdecl type name to the Mog type system string.
 * Used during analysis to convert declaration types to the internal type representation.
 */
export function mogDeclTypeToMogType(declType: string): string {
  switch (declType) {
    case "int":
      return "i64"
    case "float":
      return "f64"
    case "bool":
      return "bool"
    case "string":
      return "string"
    case "none":
      return "void"
    default:
      // Handle Result<T> -> extract T
      if (declType.startsWith("Result<") && declType.endsWith(">")) {
        return mogDeclTypeToMogType(declType.slice(7, -1))
      }
      // Handle handle types - they become ptr
      return declType
  }
}
