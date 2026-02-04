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

#### Break and Continue

```algol
while (condition) {
  if (some_condition) {
    break;     // Exit the loop immediately
  }
  if (other_condition) {
    continue;  // Skip to next iteration
  }
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

### POSIX Filesystem Operations

AlgolScript provides direct access to POSIX filesystem functions for low-level file operations.

**Note:** On macOS ARM64, `open()` with `O_CREAT` has calling convention issues. Use `creat()` instead for creating files.

#### Basic File Operations

```algol
# Open, write, and close a file
fd: i64 = open("output.txt", O_CREAT | O_WRONLY, 0644);
if (fd == -1) {
  return 1;  # Error handling
}
write(fd, "Hello World\n", 12);
close(fd);

# Read from a file
fd = open("output.txt", O_RDONLY, 0);
buf: i64[] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];  # Buffer array
bytes_read: i64 = read(fd, buf, 12);
close(fd);

# Alternative: Use creat() on macOS instead of open() with O_CREAT
fd = creat("newfile.txt", 0644);
write(fd, "data", 4);
close(fd);
```

Common open flags: `O_RDONLY`, `O_WRONLY`, `O_RDWR`, `O_CREAT`, `O_TRUNC`, `O_APPEND`

#### Directory Operations

```algol
# Create and remove directories
result: i64 = mkdir("mydir", 0755);
if (result == -1) {
  return 1;
}
rmdir("mydir");

# Open and read directory entries
dir: ptr = opendir("mydir");
if (dir != 0) {
  # Process directory entries...
  rewinddir(dir);  # Reset to beginning
  closedir(dir);
}
```

#### File Metadata

```algol
# stat - get file information
statbuf: i64[] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                  0, 0, 0];
result: i64 = stat("file.txt", statbuf);

# lstat - get symlink info (not target)
lstatbuf: i64[] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                   0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                   0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                   0, 0, 0];
result = lstat("symlink.txt", lstatbuf);

# fstat - get info from file descriptor
fd: i64 = open("file.txt", O_RDONLY, 0);
fstatbuf: i64[] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                   0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                   0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                   0, 0, 0];
result = fstat(fd, fstatbuf);
close(fd);

# Change permissions
chmod("file.txt", 0644);       # By path
fchmod(fd, 0755);              # By file descriptor
```

#### Working with Buffers

Buffers for POSIX functions are created as i64 arrays. Pass the array directly (no cast needed):

```algol
# Small buffer for string operations
small_buf: i64[] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

# Large buffer for file reading
read_buf: i64[] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                   0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                   0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                   0, 0, 0, 0, 0, 0, 0];
bytes: i64 = read(fd, read_buf, 512);
```

#### Common Patterns and Best Practices

```algol
# Pattern 1: Always check for errors
fd: i64 = open("file.txt", O_RDONLY, 0);
if (fd == -1) {
  return 1;  # Handle error
}

# Pattern 2: Cleanup on error
dir_result: i64 = mkdir("test_dir", 0755);
if (dir_result == -1) {
  return 1;
}
fd = open("test_dir/file.txt", O_CREAT | O_WRONLY, 0644);
if (fd == -1) {
  rmdir("test_dir");  # Cleanup
  return 2;
}

# Pattern 3: Close files before unlinking
write(fd, "data", 4);
close(fd);  # Close first
unlink("file.txt");  # Then remove

# Pattern 4: Cleanup files after tests
unlink("temp_file.txt");
rmdir("temp_dir");
```

## Examples

The repository includes several example programs:

- `basic_features.algol` - Demonstrates variables, arithmetic, if/else, while, functions
- `combined_features.algol` - All features combined without nested functions
- `nested_operations.algol` - Nested loops, functions, conditionals
- `fibonacci_tco.algol` - Tail-call optimized fibonacci
- `fibonacci_exit.algol` - Fibonacci with exit code
- `simple_loop.algol` - Minimal loop example
- `while_loop.algol` - While loop demonstration
- `counter_loop.algol` - Simple counter loop
- `countdown.algol` - Countdown loop example
- `factorial_recursive.algol` - Recursive factorial
- `add.algol` / `add_simple.algol` - Simple addition examples
- `exit42.algol` / `exit127.algol` - Exit code examples
- `success.algol` - Minimal success program
- `main_example.algol` - main() function example
- `test_continue.algol` - Continue statement example
- `test_nested.algol` - Nested function example

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
- `src/linker.ts` - Links LLVM IR to executable
- `src/stdlib.ts` - Standard library functions
- `src/types.ts` - Type definitions
- `runtime/` - C runtime library (GC, I/O, POSIX bindings)

## POSIX

### Platform Notes

#### macOS ARM64 Variadic Function Limitation

On macOS ARM64, the `open()` system call with the `O_CREAT` flag may fail due to variadic calling convention differences. The `open()` function is variadic (takes variable arguments), and the extra arguments required by `O_CREAT` (file mode/permissions) may not be passed correctly.

**Workaround**: Use `creat()` instead when creating files:

```algol
// This may fail on macOS ARM64
// fd: i32 = open("file.txt", O_WRONLY | O_CREAT | O_TRUNC, 0644);

// Use creat() instead for file creation
fd: i32 = creat("file.txt", 0644);

// Later, re-open with open() if you need different flags
// fd = open("file.txt", O_RDONLY);
```

This limitation affects any variadic C library function when called from AlgolScript on macOS ARM64.

## License

MIT