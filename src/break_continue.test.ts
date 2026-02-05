import { describe, it, expect } from "bun:test"
import { SemanticAnalyzer } from "./analyzer"
import { i64 } from "./types"

function pos(index: number = 0): { start: any; end: any } {
  return { start: { line: 1, column: 1, index }, end: { line: 1, column: 1, index } }
}

function program(...statements: any[]): any {
  return { type: "Program", statements, scopeId: "global", position: pos() }
}

function block(...statements: any[]): any {
  return { type: "Block", statements, scopeId: "block", position: pos() }
}

function varDecl(name: string, type: any, value: any): any {
  return { type: "VariableDeclaration", name, varType: type, value, position: pos() }
}

function ident(name: string): any {
  return { type: "Identifier", name, position: pos() }
}

function num(value: any, type: any = null): any {
  return { type: "NumberLiteral", value: String(value), literalType: type, position: pos() }
}

function binary(left: any, op: string, right: any): any {
  return { type: "BinaryExpression", left, operator: op, right, position: pos() }
}

function whileLoop(test: any, body: any): any {
  return { type: "WhileLoop", test, body, position: pos() }
}

function forLoop(variable: string, start: any, end: any, body: any): any {
  return { type: "ForLoop", variable, start, end, body, position: pos() }
}

function breakStmt(): any {
  return { type: "Break", position: pos() }
}

function continueStmt(): any {
  return { type: "Continue", position: pos() }
}

function funcDecl(name: string, params: any[], returnType: any, body: any): any {
  return { type: "FunctionDeclaration", name, params, returnType, body, position: pos() }
}

describe("break/continue semantic checking", () => {
  it("should allow break inside while loop", () => {
    const ast = program(
      funcDecl("main", [], i64, block(
        varDecl("i", i64, num(0, i64)),
        whileLoop(
          binary(ident("i"), "<", num(10, i64)),
          block(
            breakStmt(),
          )
        ),
      ))
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toHaveLength(0)
  })

  it("should allow continue inside while loop", () => {
    const ast = program(
      funcDecl("main", [], i64, block(
        varDecl("i", i64, num(0, i64)),
        whileLoop(
          binary(ident("i"), "<", num(10, i64)),
          block(
            continueStmt(),
          )
        ),
      ))
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toHaveLength(0)
  })

  it("should allow break inside for loop", () => {
    const ast = program(
      funcDecl("main", [], i64, block(
        forLoop("i", num(1, i64), num(10, i64), block(
          breakStmt(),
        )),
      ))
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toHaveLength(0)
  })

  it("should allow continue inside for loop", () => {
    const ast = program(
      funcDecl("main", [], i64, block(
        forLoop("i", num(1, i64), num(10, i64), block(
          continueStmt(),
        )),
      ))
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toHaveLength(0)
  })

  it("should error on break outside loop", () => {
    const ast = program(
      funcDecl("main", [], i64, block(
        breakStmt(),
      ))
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.length).toBeGreaterThan(0)
    expect(errors[0].message).toContain("break statement can only be used inside a loop")
  })

  it("should error on continue outside loop", () => {
    const ast = program(
      funcDecl("main", [], i64, block(
        continueStmt(),
      ))
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors.length).toBeGreaterThan(0)
    expect(errors[0].message).toContain("continue statement can only be used inside a loop")
  })

  it("should allow nested loop with break/continue", () => {
    const ast = program(
      funcDecl("main", [], i64, block(
        forLoop("i", num(1, i64), num(10, i64), block(
          forLoop("j", num(1, i64), num(5, i64), block(
            breakStmt(),
          )),
          continueStmt(),
        )),
      ))
    )
    const analyzer = new SemanticAnalyzer()
    const errors = analyzer.analyze(ast)
    expect(errors).toHaveLength(0)
  })
})
