# AlgolScript

A TypeScript-based compiler for AlgolScript, a modern programming language with Algol-68 roots that compiles to LLVM IR.

## Features

- **Complete parser** with proper operator precedence
- **Semantic analysis** with type checking
- **LLVM IR code generation**
- **Runtime library** with GC, arrays, and tables
- **Full test suite** (319 tests, all passing)
- **I/O functions** - `print_i64`, `println_i64`, `input_i64` for console I/O
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

Supported types: `i8`, `i16`, `i32`, `i64`, `i128`, `i256`, `u8`, `u16`, `u32`, `u64`, `u128`, `u256`, `f32`, `f64`, `ptr`

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

### I/O Functions

#### Print Functions

```algol
print_i64(value);      # Print integer without newline
println_i64(value);    # Print integer with newline
println();             # Print just a newline
```

Type-specific variants:
- `print_i64`, `print_u64`, `print_f64` - Print without newline
- `println_i64`, `println_u64`, `println_f64` - Print with newline

#### Input Functions

```algol
value: i64 = input_i64();  # Read integer from stdin
```

Type-specific variants:
- `input_i64` - Read signed 64-bit integer
- `input_u64` - Read unsigned 64-bit integer  
- `input_f64` - Read 64-bit float
- `input_string` - Read string (returns pointer)

### Arrays

```algol
// 1D arrays
arr: i64[] = [1, 2, 3];
element: i64 = arr[0];
arr[0] := 100;  // Modify element

// 2D arrays (matrices)
mat: i64[][] = [[1, 2], [3, 4]];
val: i64 = mat[0][1];  // Returns 2
```

### Tables

Table literals with member access:
```algol
person = {name: "Alice", age: 30};
println_i64(person.age);  // Access member
```

Using runtime functions:
```algol
// Create table with initial capacity
tbl: ptr = table_new(4);

// Set values
table_set(tbl, "key", 3, 42);

// Get values
val: i64 = table_get(tbl, "key", 3);
```

## Examples

The repository includes several example programs:

- `basic_features.algol` - Demonstrates variables, arithmetic, if/else, while, functions
- `demo_print.algol` - Demonstrates print and I/O functions
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