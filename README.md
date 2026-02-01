# AlgolScript

A TypeScript-based compiler for AlgolScript, an Algol-68 style programming language that compiles to LLVM IR.

## Features

- **Complete parser** with proper operator precedence
- **Semantic analysis** with type checking
- **LLVM IR code generation**
- **Runtime library** with GC, arrays, and tables
- **Full test suite** (306 tests, all passing)

## Installation

```bash
bun install
```

## Usage

Compile an AlgolScript program:

```bash
bun run src/compiler.ts input.algol
```

Run the compiler script directly:

```bash
bun run build_program.sh input.algol
```

## Language Syntax

### Basic Structure

```
BEGIN
  <statements>;
END
```

### Variable Declaration

```
name: type = value
```

Supported types: `i8`, `i16`, `i32`, `i64`, `i128`, `i256`, `u8`, `u16`, `u32`, `u64`, `u128`, `u256`, `f32`, `f64`

### Functions

```
FUNCTION name(param: type): type
BEGIN
  <body>;
  RETURN value;
END
```

### Control Flow

```
IF (condition) THEN
  <statements>;
FI

WHILE (condition) DO
  <statements>;
OD
```

### Operators

- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Bitwise: `&`, `|`
- Comparison: `<`, `>`, `=`, `!=`, `<=`, `>=`
- Logical: `AND`, `OR`, `not`, `!`
- Assignment: `:=`

### Arrays

```
arr: [1, 2, 3]
```

### Tables

```
table: { a: 1, b: 2 }
```

## Examples

See `example.algol` for a comprehensive example program.

## Testing

```bash
bun test
```

## Architecture

- `src/lexer.ts` - Lexical analysis (tokenization)
- `src/parser.ts` - Parsing (AST generation)
- `src/analyzer.ts` - Semantic analysis (type checking)
- `src/compiler.ts` - Compiler orchestration
- `src/llvm_codegen.ts` - LLVM IR code generation
- `runtime/` - C runtime library

## License

MIT