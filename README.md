# AlgolScript

A TypeScript-based compiler for AlgolScript, a modern programming language with Algol-68 roots that compiles to LLVM IR.

## Features

- **Complete parser** with proper operator precedence
- **Semantic analysis** with type checking
- **LLVM IR code generation**
- **Runtime library** with GC, arrays, and tables
- **Full test suite** (307 tests, all passing)
- **Modern syntax** with lowercase keywords and curly braces
- **Optional outer braces** - write code at file level without wrapping in `{ }`
- **main() function support** - define a `fn main() -> i64` entry point with exit codes

## Installation

```bash
bun install
```

## Usage

Compile an AlgolScript program:

```bash
bun run src/index.ts input.algol
```

Run the compiled executable:

```bash
./input  # (if your file was named input.algol)
```

## Language Syntax

### Program Structure

AlgolScript programs can be written in two styles:

**Style 1: Script-style (no main function)**

```algol
// Script-style - runs top-level statements
x: i64 = 10;
y: i64 = 20;
result: i64 = x + y;
```

**Style 2: main() function (with exit code)**

```algol
// main() function style - returns exit code
fn main() -> i64 {
  x: i64 = 10;
  y: i64 = 20;
  result: i64 = x + y;
  return result;
}
```

When using `main()`, the return value becomes the program's exit code (truncated from i64 to i32).

### Variable Declaration

```algol
name: type = value;
```

Supported types: `i8`, `i16`, `i32`, `i64`, `i128`, `i256`, `u8`, `u16`, `u32`, `u64`, `u128`, `u256`, `f32`, `f64`

### Functions

```algol
fn name(param: type) -> return_type {
  return value;
}
```

Example with multiple parameters:

```algol
fn add(a: i64, b: i64) -> i64 {
  return a + b;
}
```

### Control Flow

#### If Statement

```algol
if (condition) {
  statements;
} else {
  statements;
}
```

#### While Loop

```algol
while (condition) {
  statements;
}
```

#### For Loop

```algol
for variable := start to end {
  statements;
}
```

### Operators

- **Arithmetic**: `+`, `-`, `*`, `/`, `%`
- **Bitwise**: `&`, `|`
- **Comparison**: `<`, `>`, `==`, `!=`, `<=`, `>=`
- **Logical**: `not`, `!` (logical NOT)
- **Assignment**: `:=`

### Arrays

```algol
arr: [1, 2, 3];
element: i64 = arr[0];
```

### Tables

```algol
table: { a: 1, b: 2 };
value: i64 = table["a"];
```

## Examples

The repository includes several example programs:

- `basic_features.algol` - Demonstrates variables, arithmetic, if/else, while, functions
- `combined_features.algol` - All features combined without nested functions
- `nested_operations.algol` - Nested loops, functions, conditionals
- `fibonacci_tco.algol` - Tail-call optimized fibonacci
- `simple_loop.algol` - Minimal loop example
- `while_loop.algol` - While loop demonstration
- `factorial_recursive.algol` - Recursive factorial
- `counter_loop.algol` - Simple counter loop
- `test_arithmetic.algol` - Arithmetic operations

Try compiling one:

```bash
bun run src/index.ts basic_features.algol
./basic_features
```

## Testing

```bash
bun test
```

## Formatting

Use the included formatter to ensure consistent indentation:

```bash
python3 format_algol.py
```

This formats all `.algol` files with 2-space indentation based on brace nesting.

## Architecture

- `src/lexer.ts` - Lexical analysis (tokenization)
- `src/parser.ts` - Parsing (AST generation)
- `src/analyzer.ts` - Semantic analysis (type checking)
- `src/compiler.ts` - Compiler orchestration
- `src/llvm_codegen.ts` - LLVM IR code generation
- `runtime/` - C runtime library

## License

MIT