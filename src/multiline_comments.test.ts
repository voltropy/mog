import { describe, test, expect } from "bun:test"
import { tokenize } from "./lexer"

describe("Multi-line comments", () => {
  test("basic multi-line comment", () => {
    const tokens = tokenize("/* comment */")
    expect(tokens).toEqual([
      {
        type: "COMMENT",
        value: "/* comment */",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 14, index: 13 } },
      },
    ])
  })

  test("multi-line comment spanning several lines", () => {
    const tokens = tokenize("/* line1\nline2\nline3 */")
    expect(tokens).toEqual([
      {
        type: "COMMENT",
        value: "/* line1\nline2\nline3 */",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 3, column: 9, index: 23 } },
      },
    ])
  })

  test("nested multi-line comments are supported", () => {
    const tokens = tokenize("/* outer /* inner */ outer */")
    expect(tokens).toEqual([
      {
        type: "COMMENT",
        value: "/* outer /* inner */ outer */",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 30, index: 29 } },
      },
    ])
  })

  test("comment at end of line after code", () => {
    const tokens = tokenize("x /* comment */")
    const nonWs = tokens.filter(t => t.type !== "WHITESPACE")
    expect(nonWs).toEqual([
      {
        type: "IDENTIFIER",
        value: "x",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 2, index: 1 } },
      },
      {
        type: "COMMENT",
        value: "/* comment */",
        position: { start: { line: 1, column: 3, index: 2 }, end: { line: 1, column: 16, index: 15 } },
      },
    ])
  })

  test("comment between tokens", () => {
    const tokens = tokenize("x /* comment */ y")
    const nonWs = tokens.filter(t => t.type !== "WHITESPACE")
    expect(nonWs).toEqual([
      {
        type: "IDENTIFIER",
        value: "x",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 2, index: 1 } },
      },
      {
        type: "COMMENT",
        value: "/* comment */",
        position: { start: { line: 1, column: 3, index: 2 }, end: { line: 1, column: 16, index: 15 } },
      },
      {
        type: "IDENTIFIER",
        value: "y",
        position: { start: { line: 1, column: 17, index: 16 }, end: { line: 1, column: 18, index: 17 } },
      },
    ])
  })

  test("empty comment /**/", () => {
    const tokens = tokenize("/**/")
    expect(tokens).toEqual([
      {
        type: "COMMENT",
        value: "/**/",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 5, index: 4 } },
      },
    ])
  })

  test("comment with special characters", () => {
    const tokens = tokenize("/* @#$%^&*()!~`|\\{}[]<>?;:'\",. */")
    expect(tokens).toEqual([
      {
        type: "COMMENT",
        value: "/* @#$%^&*()!~`|\\{}[]<>?;:'\",. */",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 34, index: 33 } },
      },
    ])
  })

  test("unterminated comment produces UNKNOWN token", () => {
    const tokens = tokenize("/* unterminated")
    expect(tokens).toEqual([
      {
        type: "UNKNOWN",
        value: "/* unterminated",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 16, index: 15 } },
      },
    ])
  })

  test("unterminated nested comment produces UNKNOWN token", () => {
    const tokens = tokenize("/* outer /* inner */")
    expect(tokens).toEqual([
      {
        type: "UNKNOWN",
        value: "/* outer /* inner */",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 21, index: 20 } },
      },
    ])
  })

  test("multi-line comment with code before and after on different lines", () => {
    const tokens = tokenize("x\n/* comment */\ny")
    const nonWs = tokens.filter(t => t.type !== "WHITESPACE")
    expect(nonWs).toEqual([
      {
        type: "IDENTIFIER",
        value: "x",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 2, index: 1 } },
      },
      {
        type: "COMMENT",
        value: "/* comment */",
        position: { start: { line: 2, column: 1, index: 2 }, end: { line: 2, column: 14, index: 15 } },
      },
      {
        type: "IDENTIFIER",
        value: "y",
        position: { start: { line: 3, column: 1, index: 16 }, end: { line: 3, column: 2, index: 17 } },
      },
    ])
  })

  test("single-line comments still work alongside multi-line", () => {
    const tokens = tokenize("// single\n/* multi */")
    expect(tokens).toEqual([
      {
        type: "COMMENT",
        value: "// single",
        position: { start: { line: 1, column: 1, index: 0 }, end: { line: 1, column: 10, index: 9 } },
      },
      {
        type: "WHITESPACE",
        value: "\n",
        position: { start: { line: 1, column: 10, index: 9 }, end: { line: 2, column: 1, index: 10 } },
      },
      {
        type: "COMMENT",
        value: "/* multi */",
        position: { start: { line: 2, column: 1, index: 10 }, end: { line: 2, column: 12, index: 21 } },
      },
    ])
  })

  test("division operator is not confused with multi-line comment", () => {
    const tokens = tokenize("a / b")
    const nonWs = tokens.filter(t => t.type !== "WHITESPACE")
    expect(nonWs.map(t => t.type)).toEqual(["IDENTIFIER", "DIVIDE", "IDENTIFIER"])
  })
})
