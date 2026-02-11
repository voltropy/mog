import { describe, test, expect } from "bun:test"
import { tokenize, Lexer } from "./lexer"

describe("Lexer", () => {
  describe("BASIC tokens", () => {
    test("tokenizes fn keyword", () => {
      const tokens = tokenize("fn")
      expect(tokens).toEqual([
        {
          type: "fn",
          value: "fn",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 3, index: 2 } },
        },
      ])
    })

    test("tokenizes LBRACE", () => {
      const tokens = tokenize("{")
      expect(tokens).toEqual([
        {
          type: "LBRACE",
          value: "{",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 2, index: 1 } },
        },
      ])
    })

    test("tokenizes RBRACE", () => {
      const tokens = tokenize("}")
      expect(tokens).toEqual([
        {
          type: "RBRACE",
          value: "}",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 2, index: 1 } },
        },
      ])
    })

    test("tokenizes LLM keyword", () => {
      const tokens = tokenize("LLM")
      expect(tokens).toEqual([
        {
          type: "LLM",
          value: "LLM",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 4, index: 3 } },
        },
      ])
    })

    test("tokenizes control flow keywords", () => {
      const tokens = tokenize("if else while for to return cast")
      expect(tokens).toEqual([
        {
          type: "if",
          value: "if",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 3, index: 2 } },
        },
        {
          type: "WHITESPACE",
          value: " ",
          position: { start: { line: 1, column: 3, index: 2 }, end: { line: 1, column: 4, index: 3 } },
        },
        {
          type: "else",
          value: "else",
          position: { start: { line: 1, column: 4, index: 3 }, end: { line: 1, column: 8, index: 7 } },
        },
        {
          type: "WHITESPACE",
          value: " ",
          position: { start: { line: 1, column: 8, index: 7 }, end: { line: 1, column: 9, index: 8 } },
        },
        {
          type: "while",
          value: "while",
          position: { start: { line: 1, column: 9, index: 8 }, end: { line: 1, column: 14, index: 13 } },
        },
        {
          type: "WHITESPACE",
          value: " ",
          position: { start: { line: 1, column: 14, index: 13 }, end: { line: 1, column: 15, index: 14 } },
        },
        {
          type: "for",
          value: "for",
          position: { start: { line: 1, column: 15, index: 14 }, end: { line: 1, column: 18, index: 17 } },
        },
        {
          type: "WHITESPACE",
          value: " ",
          position: { start: { line: 1, column: 18, index: 17 }, end: { line: 1, column: 19, index: 18 } },
        },
        {
          type: "to",
          value: "to",
          position: { start: { line: 1, column: 19, index: 18 }, end: { line: 1, column: 21, index: 20 } },
        },
        {
          type: "WHITESPACE",
          value: " ",
          position: { start: { line: 1, column: 21, index: 20 }, end: { line: 1, column: 22, index: 21 } },
        },
        {
          type: "return",
          value: "return",
          position: { start: { line: 1, column: 22, index: 21 }, end: { line: 1, column: 28, index: 27 } },
        },
        {
          type: "WHITESPACE",
          value: " ",
          position: { start: { line: 1, column: 28, index: 27 }, end: { line: 1, column: 29, index: 28 } },
        },
        {
          type: "cast",
          value: "cast",
          position: { start: { line: 1, column: 29, index: 28 }, end: { line: 1, column: 33, index: 32 } },
        },
      ])
    })
  })

  describe("Identifiers", () => {
    test("tokenizes simple identifiers", () => {
      const tokens = tokenize("foo bar baz")
      const ids = tokens.filter((t) => t.type === "IDENTIFIER")
      expect(ids).toHaveLength(3)
      expect(ids[0].value).toBe("foo")
      expect(ids[1].value).toBe("bar")
      expect(ids[2].value).toBe("baz")
    })

    test("tokenizes identifiers with underscores", () => {
      const tokens = tokenize("my_variable _underscore __dunder__")
      const ids = tokens.filter((t) => t.type === "IDENTIFIER")
      expect(ids).toHaveLength(3)
      expect(ids[0].value).toBe("my_variable")
      expect(ids[1].value).toBe("_underscore")
      expect(ids[2].value).toBe("__dunder__")
    })

    test("tokenizes identifiers with numbers", () => {
      const tokens = tokenize("var1 temp123 x99")
      const ids = tokens.filter((t) => t.type === "IDENTIFIER")
      expect(ids).toHaveLength(3)
      expect(ids[0].value).toBe("var1")
      expect(ids[1].value).toBe("temp123")
      expect(ids[2].value).toBe("x99")
    })

    test("handles numbers followed by identifiers (requires word boundary)", () => {
      const tokens = tokenize("123 var")
      expect(tokens.filter((t) => t.type === "NUMBER")[0].type).toBe("NUMBER")
      expect(tokens.filter((t) => t.type === "NUMBER")[0].value).toBe("123")
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0].type).toBe("IDENTIFIER")
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0].value).toBe("var")
    })

    test("tokenizes uppercase identifiers", () => {
      const tokens = tokenize("MyClass CONSTANT_VALUE")
      const ids = tokens.filter((t) => t.type === "IDENTIFIER")
      expect(ids).toHaveLength(2)
      expect(ids[0].value).toBe("MyClass")
      expect(ids[1].value).toBe("CONSTANT_VALUE")
    })

    test("tokenizes single character identifiers", () => {
      const tokens = tokenize("a b c x y z")
      const ids = tokens.filter((t) => t.type === "IDENTIFIER")
      expect(ids).toHaveLength(6)
    })
  })

  describe("Operators", () => {
    test("tokenizes arithmetic operators", () => {
      const tokens = tokenize("+ - * /")
      const ops = tokens.filter((t) => t.type !== "WHITESPACE")
      expect(ops[0].type).toBe("PLUS")
      expect(ops[0].value).toBe("+")
      expect(ops[1].type).toBe("MINUS")
      expect(ops[1].value).toBe("-")
      expect(ops[2].type).toBe("TIMES")
      expect(ops[2].value).toBe("*")
      expect(ops[3].type).toBe("DIVIDE")
      expect(ops[3].value).toBe("/")
    })

    test("tokenizes comparison operators", () => {
      const tokens = tokenize("< > = != <= >=")
      const ops = tokens.filter((t) => t.type !== "WHITESPACE")
      expect(ops[0].type).toBe("LESS")
      expect(ops[1].type).toBe("GREATER")
      expect(ops[2].type).toBe("EQUAL")
      expect(ops[3].type).toBe("NOT_EQUAL")
      expect(ops[4].type).toBe("LESS_EQUAL")
      expect(ops[5].type).toBe("GREATER_EQUAL")
    })

    test("tokenizes assignment operator", () => {
      const tokens = tokenize(":=")
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "ASSIGN")[0]).toEqual({
        type: "ASSIGN",
        value: ":=",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 3, index: 2 } },
      })
    })

    test("tokenizes multiple operators in sequence", () => {
      const tokens = tokenize("a := b + c * d")
      const nonWs = tokens.filter((t) => t.type !== "WHITESPACE")
      expect(nonWs[0].value).toBe("a")
      expect(nonWs[1].type).toBe("ASSIGN")
      expect(nonWs[2].value).toBe("b")
      expect(nonWs[3].type).toBe("PLUS")
      expect(nonWs[4].value).toBe("c")
      expect(nonWs[5].type).toBe("TIMES")
      expect(nonWs[6].value).toBe("d")
    })
  })

  describe("Delimiters", () => {
    test("tokenizes parentheses", () => {
      const tokens = tokenize("( )")
      expect(tokens.filter((t) => t.type === "LPAREN")[0].type).toBe("LPAREN")
      expect(tokens.filter((t) => t.type === "LPAREN")[0].value).toBe("(")
      expect(tokens.filter((t) => t.type === "RPAREN")[0].type).toBe("RPAREN")
      expect(tokens.filter((t) => t.type === "RPAREN")[0].value).toBe(")")
    })

    test("tokenizes brackets", () => {
      const tokens = tokenize("[ ]")
      expect(tokens.filter((t) => t.type === "LBRACKET")[0].type).toBe("LBRACKET")
      expect(tokens.filter((t) => t.type === "LBRACKET")[0].value).toBe("[")
      expect(tokens.filter((t) => t.type === "RBRACKET")[0].type).toBe("RBRACKET")
      expect(tokens.filter((t) => t.type === "RBRACKET")[0].value).toBe("]")
    })

    test("tokenizes braces", () => {
      const tokens = tokenize("{ }")
      expect(tokens.filter((t) => t.type === "LBRACE")[0].type).toBe("LBRACE")
      expect(tokens.filter((t) => t.type === "LBRACE")[0].value).toBe("{")
      expect(tokens.filter((t) => t.type === "RBRACE")[0].type).toBe("RBRACE")
      expect(tokens.filter((t) => t.type === "RBRACE")[0].value).toBe("}")
    })

    test("tokenizes comma", () => {
      const tokens = tokenize(",")
      expect(tokens).toEqual([
        {
          type: "COMMA",
          value: ",",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 2, index: 1 } },
        },
      ])
    })

    test("tokenizes semicolon", () => {
      const tokens = tokenize(";")
      expect(tokens).toEqual([
        {
          type: "SEMICOLON",
          value: ";",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 2, index: 1 } },
        },
      ])
    })

    test("tokenizes colon", () => {
      const tokens = tokenize(":")
      expect(tokens).toEqual([
        {
          type: "COLON",
          value: ":",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 2, index: 1 } },
        },
      ])
    })

    test("tokenizes dot", () => {
      const tokens = tokenize(".")
      expect(tokens).toEqual([
        {
          type: "DOT",
          value: ".",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 2, index: 1 } },
        },
      ])
    })
  })

  describe("Numbers", () => {
    test("tokenizes integers", () => {
      const tokens = tokenize("42")
      expect(tokens).toEqual([
        {
          type: "NUMBER",
          value: "42",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 3, index: 2 } },
        },
      ])
    })

    test("tokenizes zero", () => {
      const tokens = tokenize("0")
      expect(tokens.filter((t) => t.type === "NUMBER")[0].type).toBe("NUMBER")
      expect(tokens.filter((t) => t.type === "NUMBER")[0].value).toBe("0")
    })

    test("tokenizes floats", () => {
      const tokens = tokenize("3.14 2.5 0.5")
      const nums = tokens.filter((t) => t.type === "NUMBER")
      expect(nums).toHaveLength(3)
      expect(nums[0].value).toBe("3.14")
      expect(nums[1].value).toBe("2.5")
      expect(nums[2].value).toBe("0.5")
    })

    test("tokenizes numbers with scientific notation", () => {
      const tokens = tokenize("1e10 2.5e-3 1e+5")
      const nums = tokens.filter((t) => t.type === "NUMBER")
      expect(nums).toHaveLength(3)
      expect(nums[0].value).toBe("1e10")
      expect(nums[1].value).toBe("2.5e-3")
      expect(nums[2].value).toBe("1e+5")
    })

    test("tokenizes large numbers", () => {
      const tokens = tokenize("999999999999999999")
      expect(tokens.filter((t) => t.type === "NUMBER")[0].type).toBe("NUMBER")
      expect(tokens.filter((t) => t.type === "NUMBER")[0].value).toBe("999999999999999999")
    })

    test("tokenizes negative number expressions (as minus and number)", () => {
      const tokens = tokenize("-42")
      expect(tokens[0].type).toBe("MINUS")
      expect(tokens[1].type).toBe("NUMBER")
      expect(tokens[1].value).toBe("42")
    })

    test("tokenizes multiple integers", () => {
      const tokens = tokenize("42")
      expect(tokens).toEqual([
        {
          type: "NUMBER",
          value: "42",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 3, index: 2 } },
        },
      ])
    })

    test("tokenizes numbers with decimal but no whole part", () => {
      const tokens = tokenize(".5 .25")
      const nums = tokens.filter((t) => t.type === "NUMBER")
      expect(nums).toHaveLength(2)
      expect(nums[0].value).toBe("5")
      expect(nums[1].value).toBe("25")
    })
  })

  describe("Strings", () => {
    test("tokenizes double-quoted strings", () => {
      const tokens = tokenize('"hello"')
      expect(tokens).toEqual([
        {
          type: "STRING",
          value: '"hello"',
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 8, index: 7 } },
        },
      ])
    })

    test("tokenizes single-quoted strings", () => {
      const tokens = tokenize("'world'")
      expect(tokens).toEqual([
        {
          type: "STRING",
          value: "'world'",
          position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 8, index: 7 } },
        },
      ])
    })

    test("tokenizes empty strings", () => {
      const tokens = tokenize("\"\" ''")
      const strs = tokens.filter((t) => t.type === "STRING")
      expect(strs[0].value).toBe('""')
      expect(strs[1].value).toBe("''")
    })

    test("tokenizes strings with escaped characters", () => {
      const tokens = tokenize("\"hello\\nworld\" 'escaped\\tstring'")
      const strs = tokens.filter((t) => t.type === "STRING")
      expect(strs).toHaveLength(2)
      expect(strs[0].value).toBe('"hello\\nworld"')
      expect(strs[1].value).toBe("'escaped\\tstring'")
    })

    test("tokenizes strings with escaped quotes", () => {
      const tokens = tokenize("\"\\\"escaped\\\"\" '\\'single\\''")
      const strs = tokens.filter((t) => t.type === "STRING")
      expect(strs[0].value).toBe('"\\\"escaped\\""')
      expect(strs[1].value).toBe("'\\'single\\''")
    })

    test("tokenizes strings with backslashes", () => {
      const tokens = tokenize('"path\\\\to\\\\file"')
      expect(tokens.filter((t) => t.type === "STRING")[0].value).toBe('"path\\\\to\\\\file"')
    })

    test("tokenizes strings with spaces", () => {
      const tokens = tokenize('"hello world" "foo bar baz"')
      const strs = tokens.filter((t) => t.type === "STRING")
      expect(strs[0].value).toBe('"hello world"')
      expect(strs[1].value).toBe('"foo bar baz"')
    })
  })

  describe("F-strings (template literals)", () => {
    test("tokenizes simple f-string", () => {
      const tokens = tokenize('f"hello world"')
      expect(tokens).toHaveLength(3)
      expect(tokens[0].type).toBe("TEMPLATE_STRING_START")
      expect(tokens[1].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[1].value).toBe("hello world")
      expect(tokens[2].type).toBe("TEMPLATE_STRING_END")
    })

    test("tokenizes f-string with interpolation", () => {
      const tokens = tokenize('f"Hello, {name}!"')
      expect(tokens[0].type).toBe("TEMPLATE_STRING_START")
      expect(tokens[1].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[1].value).toBe("Hello, ")
      expect(tokens[2].type).toBe("TEMPLATE_INTERP_START")
      expect(tokens[3].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[3].value).toBe("name")
      expect(tokens[4].type).toBe("TEMPLATE_INTERP_END")
      expect(tokens[5].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[5].value).toBe("!")
      expect(tokens[6].type).toBe("TEMPLATE_STRING_END")
    })

    test("tokenizes f-string with expression", () => {
      const tokens = tokenize('f"Result: {x + y}"')
      expect(tokens[0].type).toBe("TEMPLATE_STRING_START")
      expect(tokens[1].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[1].value).toBe("Result: ")
      expect(tokens[2].type).toBe("TEMPLATE_INTERP_START")
      expect(tokens[3].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[3].value).toBe("x + y")
      expect(tokens[4].type).toBe("TEMPLATE_INTERP_END")
      expect(tokens[5].type).toBe("TEMPLATE_STRING_END")
    })

    test("tokenizes f-string with multiple interpolations", () => {
      const tokens = tokenize('f"{a} and {b}"')
      expect(tokens[0].type).toBe("TEMPLATE_STRING_START")
      expect(tokens[1].type).toBe("TEMPLATE_INTERP_START")
      expect(tokens[2].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[2].value).toBe("a")
      expect(tokens[3].type).toBe("TEMPLATE_INTERP_END")
      expect(tokens[4].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[4].value).toBe(" and ")
      expect(tokens[5].type).toBe("TEMPLATE_INTERP_START")
      expect(tokens[6].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[6].value).toBe("b")
      expect(tokens[7].type).toBe("TEMPLATE_INTERP_END")
      expect(tokens[8].type).toBe("TEMPLATE_STRING_END")
    })

    test("tokenizes f-string with escaped braces", () => {
      const tokens = tokenize('f"{{literal}} {value} }}"')
      expect(tokens[0].type).toBe("TEMPLATE_STRING_START")
      expect(tokens[1].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[1].value).toBe("{")
      expect(tokens[2].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[2].value).toBe("literal")
      expect(tokens[3].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[3].value).toBe("}")
      expect(tokens[4].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[4].value).toBe(" ")
      expect(tokens[5].type).toBe("TEMPLATE_INTERP_START")
      expect(tokens[6].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[6].value).toBe("value")
      expect(tokens[7].type).toBe("TEMPLATE_INTERP_END")
      expect(tokens[8].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[8].value).toBe(" ")
      expect(tokens[9].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[9].value).toBe("}")
      expect(tokens[10].type).toBe("TEMPLATE_STRING_END")
    })

    test("tokenizes f-string with escape sequences", () => {
      const tokens = tokenize('f"line1\\nline2\\t{var}"')
      expect(tokens[0].type).toBe("TEMPLATE_STRING_START")
      expect(tokens[1].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[1].value).toBe("line1\nline2\t")
      expect(tokens[2].type).toBe("TEMPLATE_INTERP_START")
      expect(tokens[3].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[3].value).toBe("var")
      expect(tokens[4].type).toBe("TEMPLATE_INTERP_END")
      expect(tokens[5].type).toBe("TEMPLATE_STRING_END")
    })

    test("tokenizes single-quoted f-string", () => {
      const tokens = tokenize("f'hello {name}'")
      expect(tokens[0].type).toBe("TEMPLATE_STRING_START")
      expect(tokens[1].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[1].value).toBe("hello ")
      expect(tokens[2].type).toBe("TEMPLATE_INTERP_START")
      expect(tokens[3].type).toBe("TEMPLATE_STRING_PART")
      expect(tokens[3].value).toBe("name")
      expect(tokens[4].type).toBe("TEMPLATE_INTERP_END")
      expect(tokens[5].type).toBe("TEMPLATE_STRING_END")
    })

    test("tokenizes empty f-string", () => {
      const tokens = tokenize('f""')
      expect(tokens[0].type).toBe("TEMPLATE_STRING_START")
      expect(tokens[1].type).toBe("TEMPLATE_STRING_END")
    })
  })

  describe("Type identifiers", () => {
    test("tokenizes integer types", () => {
      const tokens = tokenize("i8 i16 i32 i64 i128 i256")
      const types = tokens.filter((t) => t.type === "TYPE")
      expect(types).toHaveLength(6)
      expect(types[0].value).toBe("i8")
      expect(types[1].value).toBe("i16")
      expect(types[2].value).toBe("i32")
      expect(types[3].value).toBe("i64")
      expect(types[4].value).toBe("i128")
      expect(types[5].value).toBe("i256")
    })

    test("tokenizes unsigned integer types", () => {
      const tokens = tokenize("u8 u16 u32 u64 u128 u256")
      const types = tokens.filter((t) => t.type === "TYPE")
      expect(types).toHaveLength(6)
      expect(types[0].value).toBe("u8")
      expect(types[1].value).toBe("u16")
      expect(types[2].value).toBe("u32")
      expect(types[3].value).toBe("u64")
      expect(types[4].value).toBe("u128")
      expect(types[5].value).toBe("u256")
    })

    test("tokenizes float types", () => {
      const tokens = tokenize("f32 f64 f128 f256")
      const types = tokens.filter((t) => t.type === "TYPE")
      expect(types).toHaveLength(4)
      expect(types[0].value).toBe("f32")
      expect(types[1].value).toBe("f64")
      expect(types[2].value).toBe("f128")
      expect(types[3].value).toBe("f256")
    })

    test("does not tokenize invalid type identifiers as TYPE", () => {
      const tokens = tokenize("i128 u256 f32 i999 u1 f16 invalid_type")
      const types = tokens.filter((t) => t.type === "TYPE")
      expect(types).toHaveLength(4)
      expect(types[0].value).toBe("i128")
      expect(types[1].value).toBe("u256")
      expect(types[2].value).toBe("f32")
      expect(types[3].value).toBe("f16")
      const ids = tokens.filter((t) => t.type === "IDENTIFIER")
      expect(ids.some((t) => t.value === "i999")).toBe(true)
      expect(ids.some((t) => t.value === "invalid_type")).toBe(true)
    })
  })

  describe("Whitespace", () => {
    test("tokenizes spaces", () => {
      const tokens = tokenize("   ")
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].type).toBe("WHITESPACE")
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].value).toBe("   ")
    })

    test("tokenizes tabs", () => {
      const tokens = tokenize("\t\t")
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].type).toBe("WHITESPACE")
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].value).toBe("\t\t")
    })

    test("tokenizes newlines", () => {
      const tokens = tokenize("\n\n")
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].type).toBe("WHITESPACE")
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].value).toBe("\n\n")
    })

test("tokenizes mixed whitespace", () => {
      const tokens = tokenize("  \t\n  ")
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].type).toBe("WHITESPACE")
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].value).toBe("  \t\n  ")
    })
  })

  describe("Comments", () => {
    test("tokenizes single-line comments", () => {
      const tokens = tokenize("# This is a comment")
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "COMMENT")[0]).toEqual({
        type: "COMMENT",
        value: "# This is a comment",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 20, index: 19 } },
      })
    })

    test("tokenizes empty comment", () => {
      const tokens = tokenize("#")
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "COMMENT")[0].type).toBe("COMMENT")
      expect(tokens.filter((t) => t.type === "COMMENT")[0].value).toBe("#")
    })

    test("tokenizes code followed by comment", () => {
      const tokens = tokenize("{ # start block")
      expect(tokens.filter((t) => t.type === "LBRACE")[0].type).toBe("LBRACE")
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].type).toBe("WHITESPACE")
      expect(tokens.filter((t) => t.type === "COMMENT")[0].type).toBe("COMMENT")
    })

    test("tokenizes comment followed by code", () => {
      const tokens = tokenize("# comment\n}")
      expect(tokens.filter((t) => t.type === "COMMENT")[0].type).toBe("COMMENT")
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].type).toBe("WHITESPACE")
      expect(tokens.filter((t) => t.type === "RBRACE")[0].type).toBe("RBRACE")
    })
  })

  describe("Line and column tracking", () => {
    test("tracks positions correctly across tokens", () => {
      const tokens = tokenize("foo\nbar")
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0].position.start.line).toBe(1)
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0].position.start.column).toBe(1)
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[1].position.start.line).toBe(2)
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[1].position.start.column).toBe(1)
    })

    test("tracks column correctly", () => {
      const tokens = tokenize("  foo  bar")
      const ids = tokens.filter((t) => t.type === "IDENTIFIER")
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].position.start.column).toBe(1)
      expect(ids[0].position.start.column).toBe(3)
      expect(ids[1].position.start.column).toBe(8)
    })

    test("tracks line correctly after multiple newlines", () => {
      const tokens = tokenize("line1\n\nline3")
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0].position.start.line).toBe(1)
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[1].position.start.line).toBe(3)
    })

    test("tracks index correctly", () => {
      const tokens = tokenize("abc def")
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0].position.start.index).toBe(0)
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0].position.end.index).toBe(3)
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].position.start.index).toBe(3)
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].position.end.index).toBe(4)
    })
  })

  describe("Complex expressions", () => {
    test("tokenizes a simple assignment", () => {
      const tokens = tokenize("x := 42")
      const nonWs = tokens.filter((t) => t.type !== "WHITESPACE")
      expect(nonWs).toHaveLength(3)
      expect(nonWs[0].value).toBe("x")
      expect(nonWs[1].type).toBe("ASSIGN")
      expect(nonWs[2].value).toBe("42")
    })

    test("tokenizes a function call", () => {
      const tokens = tokenize("foo(bar, baz)")
      const nonWs = tokens.filter((t) => t.type !== "WHITESPACE")
      expect(nonWs).toHaveLength(6)
      expect(nonWs[0].value).toBe("foo")
      expect(nonWs[1].type).toBe("LPAREN")
      expect(nonWs[2].value).toBe("bar")
      expect(nonWs[3].type).toBe("COMMA")
      expect(nonWs[4].value).toBe("baz")
      expect(nonWs[5].type).toBe("RPAREN")
    })

    test("tokenizes array access", () => {
      const tokens = tokenize("arr[0]")
      const nonWs = tokens.filter((t) => t.type !== "WHITESPACE")
      expect(nonWs).toHaveLength(4)
      expect(nonWs[0].value).toBe("arr")
      expect(nonWs[1].type).toBe("LBRACKET")
      expect(nonWs[2].type).toBe("NUMBER")
      expect(nonWs[3].type).toBe("RBRACKET")
    })

    test("tokenizes array syntax", () => {
      const tokens = tokenize("arr[1][2]")
      const nonWs = tokens.filter((t) => t.type !== "WHITESPACE")
      expect(nonWs.map((t) => t.value)).toEqual(["arr", "[", "1", "]", "[", "2", "]"])
    })

    test("tokenizes table syntax", () => {
      const tokens = tokenize("{key:value}")
      expect(tokens.map((t) => t.value)).toEqual(["{", "key", ":", "value", "}"])
    })

    test("tokenizes block delimiters", () => {
      const tokens = tokenize("{ x := 5 }")
      const nonWhitespace = tokens.filter((t) => t.type !== "WHITESPACE")
      expect(nonWhitespace.map((t) => t.type)).toEqual(["LBRACE", "IDENTIFIER", "ASSIGN", "NUMBER", "RBRACE"])
    })

    test("tokenizes a complex expression", () => {
      const tokens = tokenize("result := (a + b) * (c - d) / 2")
      const nonWs = tokens.filter((t) => t.type !== "WHITESPACE")
      expect(nonWs[0].value).toBe("result")
      expect(nonWs[1].type).toBe("ASSIGN")
      expect(nonWs[2].type).toBe("LPAREN")
      expect(nonWs[3].value).toBe("a")
      expect(nonWs[4].type).toBe("PLUS")
      expect(nonWs[5].value).toBe("b")
      expect(nonWs[6].type).toBe("RPAREN")
      expect(nonWs[7].type).toBe("TIMES")
      expect(nonWs[8].type).toBe("LPAREN")
      expect(nonWs[9].value).toBe("c")
      expect(nonWs[10].type).toBe("MINUS")
      expect(nonWs[11].value).toBe("d")
      expect(nonWs[12].type).toBe("RPAREN")
      expect(nonWs[13].type).toBe("DIVIDE")
      expect(nonWs[14].type).toBe("NUMBER")
      expect(nonWs[14].value).toBe("2")
    })

    test("tokenizes complex expression", () => {
      const tokens = tokenize("result[i] := x + y * (a - b)")
      const nonWhitespace = tokens.filter((t) => t.type !== "WHITESPACE")
      expect(nonWhitespace.map((t) => t.value)).toEqual([
        "result",
        "[",
        "i",
        "]",
        ":=",
        "x",
        "+",
        "y",
        "*",
        "(",
        "a",
        "-",
        "b",
        ")",
      ])
    })
  })

  describe("Edge cases", () => {
    test("tokenizes empty string", () => {
      const tokens = tokenize("")
      expect(tokens).toHaveLength(0)
    })

    test("tokenizes single character", () => {
      const tokens = tokenize("x")
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0]).toEqual({
        type: "IDENTIFIER",
        value: "x",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 2, index: 1 } },
      })
    })

test("tokenizes only whitespace", () => {
      const tokens = tokenize("   \t\n   ")
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].type).toBe("WHITESPACE")
    })

    test("tokenizes only a comment", () => {
      const tokens = tokenize("# just a comment")
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "COMMENT")[0].type).toBe("COMMENT")
    })

    test("tokenizes underscores", () => {
      const tokens = tokenize("_a _b1 __foo__")
      const ids = tokens.filter((t) => t.type === "IDENTIFIER")
      expect(ids).toHaveLength(3)
      expect(ids[0].value).toBe("_a")
      expect(ids[1].value).toBe("_b1")
      expect(ids[2].value).toBe("__foo__")
    })

    test("tokenizes very long identifier", () => {
      const longId = "a".repeat(1000)
      const tokens = tokenize(longId)
      expect(tokens).toHaveLength(1)
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0].type).toBe("IDENTIFIER")
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0].value).toBe(longId)
    })

    test("tokenizes large numbers", () => {
      const tokens = tokenize("1e308")
      expect(tokens.filter((t) => t.type === "NUMBER")[0].type).toBe("NUMBER")
      expect(tokens.filter((t) => t.type === "NUMBER")[0].value).toBe("1e308")
    })
  })

  describe("Unknown characters", () => {
    test("tokenizes unknown characters as UNKNOWN", () => {
      const tokens = tokenize("@ $")
      const unknown = tokens.filter((t) => t.type === "UNKNOWN")
      expect(unknown).toHaveLength(2)
      expect(unknown[0].value).toBe("@")
      expect(unknown[1].value).toBe("$")
    })

    test("tokenizes ? as QUESTION_MARK", () => {
      const tokens = tokenize("?")
      const qm = tokens.filter((t) => t.type === "QUESTION_MARK")
      expect(qm).toHaveLength(1)
      expect(qm[0].value).toBe("?")
    })

    test("tokenizes unicode characters as UNKNOWN", () => {
      const tokens = tokenize("€ 漢字")
      const unknown = tokens.filter((t) => t.type === "UNKNOWN")
      expect(unknown.length).toBeGreaterThan(0)
    })

    test("handles mix of known and unknown characters", () => {
      const tokens = tokenize("foo @ bar")
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[0].type).toBe("IDENTIFIER")
      expect(tokens.filter((t) => t.type === "WHITESPACE")[0].type).toBe("WHITESPACE")
      expect(tokens.filter((t) => t.type === "UNKNOWN")[0].type).toBe("UNKNOWN")
      expect(tokens.filter((t) => t.type === "UNKNOWN")[0].value).toBe("@")
      expect(tokens.filter((t) => t.type === "WHITESPACE")[1].type).toBe("WHITESPACE")
      expect(tokens.filter((t) => t.type === "IDENTIFIER")[1].type).toBe("IDENTIFIER")
    })
  })

describe("Multi-line programs", () => {
    test("tokenizes multi-line program", () => {
      const code = `{
  x := 42
  result := x * 2
}`
      const tokens = tokenize(code)
      expect(tokens.some((t) => t.type === "LBRACE" && t.position.start.line === 1)).toBe(true)
      expect(tokens.some((t) => t.type === "IDENTIFIER" && t.value === "x" && t.position.start.line === 2)).toBe(true)
      expect(tokens.some((t) => t.type === "RBRACE" && t.position.start.line === 4)).toBe(true)
    })

    test("tracks line numbers correctly in multi-line program", () => {
      const code = `line1
line2
line3`
      const identifiers = tokenize(code).filter((t) => t.type === "IDENTIFIER")
      expect(identifiers[0].position.start.line).toBe(1)
      expect(identifiers[1].position.start.line).toBe(2)
      expect(identifiers[2].position.start.line).toBe(3)
    })

    test("tokenizes comments in multi-line code", () => {
      const tokens = tokenize("x := 5 # this is a comment\ny := 10")
      const commentToken = tokens.find((t) => t.type === "COMMENT")
      expect(commentToken).toBeDefined()
      expect(commentToken?.value).toBe("# this is a comment")
    })

    test("tracks positions correctly", () => {
      const tokens = tokenize("{\n  x := 5\n}")
      const lbraceToken = tokens.find((t) => t.type === "LBRACE")
      const rbraceToken = tokens.find((t) => t.type === "RBRACE")

      expect(lbraceToken?.position.start.line).toBe(1)
      expect(lbraceToken?.position.start.column).toBe(1)
      expect(rbraceToken?.position.start.line).toBe(3)
      expect(rbraceToken?.position.start.column).toBe(1)
    })
  })

  describe("Lexer class API", () => {
    test("is instantiable with Lexer class", () => {
      const lexer = new Lexer("fn")
      const tokens = lexer.tokenize()
      expect(tokens).toHaveLength(1)
      expect(tokens[0].type).toBe("fn")
    })

    test("tokenizes multiple times with new instance", () => {
      const lexer1 = new Lexer("foo")
      const lexer2 = new Lexer("bar")
      expect(lexer1.tokenize()[0].value).toBe("foo")
      expect(lexer2.tokenize()[0].value).toBe("bar")
    })

    test("tokenizes with tokenize() function", () => {
      const tokens = tokenize("fn")
      expect(tokens[0].type).toBe("fn")
    })
  })
})
