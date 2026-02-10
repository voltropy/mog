# Mog

A compiled programming language with an LLVM backend, designed for clean syntax and predictable behavior. Mog compiles to native executables with strong static typing, structs, n-dimensional arrays, and direct POSIX access.

## Features

- **Compiled to native code** - LLVM backend produces optimized executables
- **Strong static typing** - Explicit types with no implicit coercion (`i8`-`i64`, `u8`-`u64`, `f32`/`f64`, `ptr`, `string`)
- **Structs with named fields** - Heap-allocated structs with field access and assignment
- **N-dimensional arrays** - Fixed-size and dynamic arrays with element-wise operations
- **Maps** - Lua-like key-value stores with dot and bracket access
- **POSIX bindings** - Direct access to filesystem operations and BSD sockets
- **Automatic memory management** - Mark-and-sweep garbage collector with large object support
- **467 tests** passing across 10 test files

## Installation

```bash
bun install
```

## Usage

Compile an Mog program:

```bash
bun run src/index.ts input.mog
```

Run the compiled executable:

```bash
./input  # (if your file was named input.mog)
```

## Language Syntax

### Program Structure

Mog programs can be written in two styles:

**Style 1: Script-style (no main function)**

```mog
// Script-style - runs top-level statements
x: i64 = 10;
y: i64 = 20;
result: i64 = x + y;
```

**Style 2: main() function (with exit code)**

```mog
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

```mog
name: type = value;
```

Supported types:
- **Signed integers**: `i8`, `i16`, `i32`, `i64`, `i128`, `i256`
- **Unsigned integers**: `u8`, `u16`, `u32`, `u64`, `u128`, `u256`
- **Floats**: `f8`, `f16`, `f32`, `f64`, `f128`, `f256`
- **Other**: `ptr` (pointer), `string` (byte array)

### Functions

```mog
fn name(param: type) -> return_type {
  return value;
}
```

Example with multiple parameters:

```mog
fn add(a: i64, b: i64) -> i64 {
  return a + b;
}
```

Nested functions are supported - define functions inside other functions:

```mog
fn outer() -> i64 {
  fn inner() -> i64 {
    return 42;
  }
  return inner();
}
```

### Control Flow

#### If Statement

```mog
if (condition) {
  statements;
} else {
  statements;
}
```

#### While Loop

```mog
while (condition) {
  statements;
}
```

#### For Loop

```mog
for variable := start to end {
  statements;
}
```

#### Break and Continue

```mog
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
- **Bitwise**: `&` (AND), `|` (OR), `^` (XOR), `~` (NOT), `<<` (left shift), `>>` (right shift)
- **Comparison**: `<`, `>`, `==`, `!=`, `<=`, `>=`
- **Logical**: `not`, `!` (logical NOT)
- **Assignment**: `:=`

All operators have clear, explicit precedence—no surprises.

### I/O Functions

#### Print Functions

```mog
print(value);          # Print any type without newline (auto-detects type)
println(value);        # Print any type with newline
println();             # Print just a newline

// Type-specific variants (for explicit control):
print_i64(value);      # Print i64 without newline
println_i64(value);    # Print i64 with newline
print_f64(value);      # Print f64 without newline
println_f64(value);    # Print f64 with newline
```

Type-specific variants:
- `print_i64`, `print_u64`, `print_f64` - Print without newline
- `println_i64`, `println_u64`, `println_f64` - Print with newline

#### Input Functions

```mog
value: i64 = input_i64();  # Read integer from stdin
```

Type-specific variants:
- `input_i64` - Read signed 64-bit integer
- `input_u64` - Read unsigned 64-bit integer  
- `input_f64` - Read 64-bit float
- `input_string` - Read string (returns pointer)

### Arrays and N-Dimensional Arrays

```mog
// 1D arrays
arr: i64[] = [1, 2, 3];
element: i64 = arr[0];
arr[0] := 100;  // Modify element

// 2D arrays (matrices)
mat: i64[][] = [[1, 2], [3, 4]];
val: i64 = mat[0][1];  // Returns 2

// Array fill syntax - create array with repeated value
zeros: [i64; 5] = [0; 5];      // [0, 0, 0, 0, 0]
fives: [f64; 3] = [5.0; 3];    // [5.0, 5.0, 5.0]

// N-dimensional arrays for ML workloads (future)
// tensor: f64[3][224][224] = ...;  // For image processing
```

### Maps

Key-value stores with string keys, created using `table_new` and accessed with bracket notation:

```mog
// Create a map
tbl: ptr = table_new(4);

// Set values using bracket notation
tbl["host"] := 42;           // String key
tbl["port"] := 8080;         // String key

// Set values using function call (key, key_length, value)
table_set(tbl, "key", 3, 42);

// Get values
val: i64 = table_get(tbl, "key", 3);
```

Map literals with member access:
```mog
person = {name: "Alice", age: 30};
print(person.age);
println();
```

### Structs

Structs group related data with named, typed fields. Fields are separated by commas. Structs are heap-allocated and accessed via pointer.

```mog
struct Point { x: f64, y: f64 }

// Create with explicit type prefix
p: Point = Point { x: 1.0, y: 2.0 };

// Field access
print(p.x);
println();

// Field assignment
p.x := 5.0;
```

Structs work as function parameters and return types:
```mog
struct Point { x: f64, y: f64 }

fn create_point(x: f64, y: f64) -> Point {
  return Point { x: x, y: y };
}

fn move_point(p: Point, dx: f64, dy: f64) -> Point {
  p.x := p.x + dx;
  p.y := p.y + dy;
  return p;
}

fn main() -> i64 {
  p: Point = create_point(1.0, 2.0);
  p := move_point(p, 5.0, 10.0);
  print(p.x);   // 6.0
  println();
  return 0;
}
```

### SoA (Struct of Arrays)

SoA declarations store each field as a separate array. Access pattern is `soa.field[i]`:

```mog
soa Particles {
  x: [f64],
  y: [f64]
}

particles: Particles = Particles {
  x: [1.0, 2.0, 3.0],
  y: [4.0, 5.0, 6.0]
};

// Access: field then index
first_x: f64 = particles.x[0];
particles.x[0] := 10.0;
```

### Strings

Strings are arrays of unsigned bytes (`u8`) with special syntax support:

```mog
// String literals use double quotes
msg: string = "Hello, World!";
greeting: string = "Hi";

// String indexing - get character at position
first: u8 = msg[0];         // 'H' (ASCII 72)

// String slicing - extract substring
sub: string = msg[0:5];     // "Hello" (positions 0-4)
world: string = msg[7:12];  // "World" (positions 7-11)

// String length
len: i64 = string_length(msg);

// String concatenation
combined: string = string_concat(greeting, " there!");
```

### POSIX Filesystem Operations

Mog provides direct access to POSIX filesystem functions for low-level file operations.

**Note:** On macOS ARM64, `open()` with `O_CREAT` has calling convention issues. Use `creat()` instead for creating files.

#### Basic File Operations

```mog
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

```mog
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

```mog
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

```mog
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

```mog
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

### POSIX Sockets

Mog provides BSD socket wrappers for network programming:

```mog
fn main() -> i64 {
  // Create a TCP socket
  sockfd: i64 = sys_socket(2, 1, 0);  // AF_INET, SOCK_STREAM

  // Resolve IP address
  ip_addr: i64 = sys_inet_addr("93.184.216.34");

  // Connect to remote host on port 80
  result: i64 = sys_connect(sockfd, ip_addr, 80);
  if (result < 0) {
    sys_close(sockfd);
    return 1;
  }

  // Send HTTP request
  request: ptr = "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n";
  sys_send(sockfd, request, 40);

  // Receive response
  buf: ptr = gc_alloc(4096);
  bytes: i64 = sys_recv(sockfd, buf, 4096);

  // Print and clean up
  print_string(buf);
  sys_close(sockfd);
  return 0;
}
```

Available socket functions: `sys_socket`, `sys_connect`, `sys_send`, `sys_recv`, `sys_close`, `sys_fcntl`, `sys_inet_addr`, `sys_errno`.

## Examples

The repository includes HTTP client examples demonstrating socket programming:

- `http_client.mog` - Full HTTP client with connection, request, and response handling
- `minimal_http.mog` - Minimal HTTP GET request
- `clean_http.mog` - Clean HTTP client with error handling

Try compiling one:

```bash
bun run src/index.ts http_client.mog
./http_client
```

## Testing

```bash
bun test
```

## Formatting

Use the included formatter to ensure consistent indentation:

```bash
python3 format_mog.py
```

This formats all `.mog` files with 2-space indentation based on brace nesting.

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

```mog
// This may fail on macOS ARM64
// fd: i32 = open("file.txt", O_WRONLY | O_CREAT | O_TRUNC, 0644);

// Use creat() instead for file creation
fd: i32 = creat("file.txt", 0644);

// Later, re-open with open() if you need different flags
// fd = open("file.txt", O_RDONLY);
```

This limitation affects any variadic C library function when called from Mog on macOS ARM64.

## License

MIT
