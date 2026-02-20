# The Mog Language Guide

> A complete guide to learning and using the Mog programming language.

---



# Chapter 1: Introduction

Mog is a small, statically-typed, embeddable programming language. It compiles to native code through LLVM (and a lightweight QBE backend), runs inside a host application, and is designed for one job: giving LLM agents and automation scripts a language that is safe to execute, fast enough for real work, and small enough for a model to hold in its context window.

If you have written code in Rust, Go, or TypeScript, Mog's syntax will feel familiar. If you have embedded Lua or Wren in an application, Mog's execution model will make sense immediately. The difference is that Mog is statically typed, compiles to native code, and enforces a capability-based security model that makes it safe to run untrusted code.

## What Mog Is For

Mog targets a specific set of use cases:

- **LLM agent tool-use scripts.** An agent generates a Mog script, the host compiles and runs it in a sandbox, and the script can only access capabilities the host explicitly grants. No container needed.
- **Plugin and extension scripting.** A host application exposes functionality through capabilities, and third-party scripts extend the application without being able to crash it or access the filesystem behind its back.
- **ML workflow orchestration.** Tensors are a built-in data type with hardware-relevant dtypes like `f16` and `bf16`. The host provides ML operations (matmul, activations, autograd) through capabilities, so the script describes *what* to compute and the host decides *where* to run it.
- **Short automation scripts.** Configuration, data transformation, and glue code where you want static types and native speed without the weight of a general-purpose language.

Here is what a small Mog program looks like — an agent tool script that searches an API and filters results:

```mog
requires http, log;

async fn search(query: string) -> Result<string[]> {
  response := await http.get("/api/search?q={query}")?;
  results := parse_results(response);
  log.info("found {results.len} results for '{query}'");
  return ok(results);
}

async fn main() -> int {
  results := await search("machine learning")?;
  top := results.filter(fn(r) { r.score > 0.8 });
  for r in top {
    print("{r.title}: {r.url}");
  }
  return 0;
}
```

The `requires http, log` declaration tells the host what capabilities this script needs. If the host doesn't provide them, the script won't run. The script itself has no way to access the network or filesystem directly — everything goes through the host.

## Design Philosophy

Mog is built around six principles. They explain why the language looks the way it does, and why many features you might expect are deliberately absent.

**1. Small surface area.** The entire language should fit in an LLM's context window. Every feature must justify its existence. When in doubt, leave it out. This is why Mog has no macros, no generics (beyond tensor dtype parameterization), no inheritance, and no operator overloading. A smaller language is easier to learn, easier to generate, and easier to reason about.

**2. Predictable semantics.** No implicit coercion surprises, no operator precedence puzzles, no hidden control flow. Code reads top-to-bottom, left-to-right. When you see an expression like `a + b`, the types of `a` and `b` determine exactly what happens — there is no overloading, no implicit conversion from string to int, no surprising promotion rules.

**3. Familiar syntax.** Curly braces, `fn`, `->`, `:=`. A blend of Rust, Go, and TypeScript that LLMs already generate fluently. Mog introduces no novel syntax without strong justification. If you can read any of those languages, you can read Mog.

**4. Safe by default.** Garbage collected, bounds-checked, no null, no raw pointers. A Mog script cannot crash the host process, corrupt memory, or access resources outside its sandbox. The runtime includes a mark-and-sweep garbage collector, array bounds checking, and cooperative interrupt polling at loop back-edges so the host can terminate runaway scripts.

**5. Host provides I/O.** The language has no built-in file, network, or system access. All side effects go through capabilities explicitly granted by the embedding host. This is the foundation of Mog's security model — a script can only do what the host lets it do.

**6. Tensors as data, ML as capability.** The language provides n-dimensional arrays (tensors) as a built-in data structure with element-level read/write and shape manipulation. All ML operations — matmul, activations, loss functions, autograd — are provided by the host through capabilities. The language gives you the data structure; the host gives you the compute. This means the host can route tensor operations to CPU, GPU, or a remote accelerator without the script needing to know.

## How Mog Compares to Other Embeddable Languages

If you have used other embeddable languages, here is how Mog differs:

**Lua** is dynamically typed, interpreted (or JIT-compiled via LuaJIT), and has a minimal core. Mog shares Lua's philosophy of being small and embeddable, but adds static types, compiles to native code ahead of time, and enforces capability-based security rather than relying on environment sandboxing. Lua's flexibility is a strength for interactive scripting; Mog trades that flexibility for compile-time safety and native performance.

**Wren** is a class-based, dynamically-typed embeddable language with a clean syntax. Like Mog, it is designed for embedding in host applications. The key differences are that Mog is statically typed, compiles ahead of time, has no classes or inheritance, and provides a formal capability model for security. Wren's object-oriented design makes it natural for game scripting; Mog's functional style with explicit capabilities makes it natural for agent scripting and ML workflows.

**Rhai** is a scripting language for Rust applications, dynamically typed, interpreted. Mog targets a similar embedding scenario but takes a different approach: static types catch errors before execution, LLVM compilation produces native-speed code, and the capability system provides security guarantees that a dynamic language cannot offer at compile time.

The common thread: Mog is the statically-typed, ahead-of-time-compiled option in the embeddable language space. It pays for this with a compilation step, but gains type safety, native performance, and a security model that is enforced before the script runs.

## The Capability-Based Security Model

Security in Mog is not an afterthought bolted onto the runtime — it is a structural property of the language. A Mog script declares the capabilities it needs at the top of the file:

```mog
requires fs, log;        // these must be provided or the script won't run
optional env;             // this is used if available, ignored if not
```

The host application decides which capabilities to grant:

```c
MogVM *vm = mog_vm_new();
mog_register_capability(vm, "fs", fs_functions, 6);
mog_register_capability(vm, "log", log_functions, 4);
```

If a script declares `requires http` but the host doesn't provide the `http` capability, the script is rejected before it executes. There is no way for the script to discover or access capabilities that weren't explicitly registered.

This means you can look at a Mog script's `requires` declarations and know exactly what it can do. A script that says `requires log` can write log messages and nothing else — it cannot read files, make network requests, or access the filesystem. This property is enforced by the compiler and runtime together, not by convention.

Standard capabilities that hosts commonly provide include:

| Capability | What it provides |
|---|---|
| `fs` | File read/write/exists/remove |
| `http` | HTTP requests |
| `log` | Structured logging (info, warn, error, debug) |
| `env` | Environment info, timestamps, random numbers |
| `process` | Sleep, environment variables, working directory |
| `ml` | ML operations (matmul, activations, autograd) |
| `model` | LLM inference |
| `db` | Database queries |

A host can also define custom capabilities — there is nothing special about the standard ones. They are just conventions.

## What Mog Is Not

Mog is deliberately not many things. Each omission keeps the language small, the security model tractable, and the compilation fast:

- **Not a systems language.** No raw pointers, no manual memory management, no POSIX syscalls, no direct OS access.
- **Not standalone.** Mog is always embedded in a host application. There is no standard library for file I/O or networking — the host provides everything.
- **Not general-purpose.** Mog is for scripts, plugins, and orchestration. It is not designed for building web servers, operating systems, or databases.
- **Not object-oriented.** No classes, no inheritance, no methods on types. Structs hold data; functions operate on data. Higher-order functions and closures provide the abstraction mechanisms.
- **No macros or metaprogramming.** The language you see is the language that runs. No code generation, no compile-time evaluation, no syntax extensions.
- **No generics.** Beyond tensor dtype parameterization (`tensor<f32>`, `tensor<f16>`), there are no generic types or functions. This keeps the type system simple and the compiler small.
- **No exceptions with stack unwinding.** Error handling uses `Result<T>` with explicit propagation via `?`. Errors are values, not control flow.
- **No threads or locks.** Concurrency is cooperative via `async`/`await`, with the host managing the event loop.

If you need any of these features, Mog is probably not the right language for your use case — and that's fine. Mog is designed to do a few things well rather than everything adequately.

## What's Ahead

The rest of this guide walks through the language feature by feature, with runnable examples at every step. Chapter 2 starts with the simplest possible program and builds up from there. By the end, you will be able to write Mog scripts that use the full language: variables, functions, closures, structs, arrays, maps, error handling, async operations, tensors, and host capabilities.

Let's write some code.
# Chapter 2: Your First Mog Program

This chapter walks through the basics of writing and running Mog code. By the end, you will understand how a Mog program is structured, how to print output, how to write comments, and how to define and call functions.

## Hello, World

Every Mog program needs a `main` function. Here is the simplest complete program:

```mog
fn main() -> int {
  println("Hello, world!");
  return 0;
}
```

`fn main() -> int` declares the entry point. The `-> int` annotation means the function returns an integer — by convention, `0` signals success. The `println` function prints a string followed by a newline. The `return 0;` statement exits the program.

Semicolons are required after every statement. Curly braces delimit blocks. If you are coming from Go or Rust, this will feel natural. If you are coming from Python, the semicolons may take a few minutes to get used to.

## How Mog Programs Are Compiled and Run

Mog is a compiled language. You write a `.mog` source file, compile it to a native executable, and run the executable. The compiler is written in TypeScript and runs under Bun:

```bash
# Compile a Mog program to a native executable
bun run src/index.ts hello.mog

# Run the resulting binary
./hello
```

The compilation pipeline works like this: the compiler reads your `.mog` file, lexes it into tokens, parses it into an abstract syntax tree, type-checks it, generates LLVM IR, and links it with the Mog runtime to produce a native binary. The whole process takes milliseconds for small programs.

There is also a convenience script that compiles, links, and runs in a single step:

```bash
./algb hello.mog
```

In production, Mog programs run embedded inside a host application. The host compiles the script, registers capabilities, and invokes the compiled code through a C API. But for learning the language, the standalone compilation path is all you need.

Mog also has a lightweight QBE backend as an alternative to LLVM. QBE compiles roughly twice as fast as LLVM with `-O1`, at the cost of less optimized output. Both backends produce correct native code from the same source.

## Program Structure

A Mog source file is a sequence of top-level declarations followed by a `main` function. Top-level declarations include functions, structs, and capability requirements. You cannot put executable statements at the top level — all code runs inside functions.

```mog
// Top-level: struct declaration
struct Point {
  x: float,
  y: float
}

// Top-level: function declaration
fn distance(a: Point, b: Point) -> float {
  dx := a.x - b.x;
  dy := a.y - b.y;
  return sqrt((dx * dx) + (dy * dy));
}

// Top-level: entry point
fn main() -> int {
  p1 := Point { x: 3.0, y: 0.0 };
  p2 := Point { x: 0.0, y: 4.0 };
  d := distance(p1, p2);
  println(f"Distance: {d}");
  return 0;
}
```

The order of declarations does not matter — you can call a function that is defined later in the file. The compiler resolves all names before generating code.

Here is a program with multiple functions that call each other:

```mog
fn square(x: int) -> int {
  return x * x;
}

fn sum_of_squares(a: int, b: int) -> int {
  return square(a) + square(b);
}

fn main() -> int {
  result := sum_of_squares(3, 4);
  println(f"3² + 4² = {result}");  // 3² + 4² = 25
  return 0;
}
```

And a program that uses a struct and a helper function:

```mog
struct Rectangle {
  width: float,
  height: float
}

fn area(r: Rectangle) -> float {
  return r.width * r.height;
}

fn main() -> int {
  r := Rectangle { width: 5.0, height: 3.0 };
  a := area(r);
  print_string("Area: ");
  print_f64(a);
  println("");
  return 0;
}
```

## Comments

Mog supports two kinds of comments. Single-line comments start with `//` and run to the end of the line. Multi-line comments are delimited by `/*` and `*/`.

```mog
// This is a single-line comment

/* This is a
   multi-line comment */

fn main() -> int {
  // Comments can appear on their own line
  x := 42;  // or at the end of a line

  /*
   * Multi-line comments are useful for temporarily
   * disabling blocks of code or writing longer
   * explanations.
   */

  println(f"x = {x}");
  return 0;
}
```

Multi-line comments do not nest. A `/*` inside a multi-line comment is treated as ordinary text, and the first `*/` ends the comment. In practice, single-line comments are far more common.

```mog
fn main() -> int {
  // Calculate the sum of integers from 1 to 10
  sum := 0;
  i := 1;
  while (i <= 10) {
    sum = sum + i;
    i = i + 1;
  }
  println(f"Sum: {sum}");  // Sum: 55

  /* Temporarily disabled:
  println("This line does not execute");
  */

  return 0;
}
```

## Print Functions

Mog provides several built-in functions for printing output. These are always available — no imports needed.

| Function | Description |
|---|---|
| `println(s)` | Print a string followed by a newline |
| `print_string(s)` | Print a string without a newline |
| `print(n)` | Print an integer |
| `println_i64(n)` | Print an integer followed by a newline |
| `print_f64(x)` | Print a float |

The most common is `println`, which prints a string with a trailing newline. For formatted output, combine it with f-string interpolation:

```mog
fn main() -> int {
  println("Hello, world!");

  name := "Mog";
  println(f"Welcome to {name}!");

  x := 42;
  println(f"The answer is {x}");

  return 0;
}
```

Output:

```
Hello, world!
Welcome to Mog!
The answer is 42
```

When you need to print numbers without f-string formatting, use the type-specific print functions:

```mog
fn main() -> int {
  // Print an integer
  print_string("Count: ");
  println_i64(42);

  // Print a float
  print_string("Pi: ");
  print_f64(3.14159);
  println("");

  // print() also works for integers
  print_string("Score: ");
  print(100);
  println("");

  return 0;
}
```

Output:

```
Count: 42
Pi: 3.141590
Score: 100
```

The `print_string` function is useful when you want to build a line of output from multiple parts without newlines between them:

```mog
fn main() -> int {
  print_string("Loading");
  print_string(".");
  print_string(".");
  print_string(".");
  println(" done!");
  return 0;
}
```

Output:

```
Loading... done!
```

F-string interpolation (the `f"..."` syntax) is the preferred way to format output in Mog. It can embed any expression inside `{}` delimiters:

```mog
fn main() -> int {
  width := 10;
  height := 5;
  println(f"Rectangle: {width} x {height} = {width * height}");

  name := "Alice";
  score := 95;
  println(f"{name} scored {score} points");

  a := 3.14;
  println(f"Value: {a}");

  return 0;
}
```

Output:

```
Rectangle: 10 x 5 = 50
Alice scored 95 points
Value: 3.14
```

## A More Complete Example

Let's put everything together. Here is a program that defines a few functions, uses variables and arithmetic, and prints formatted output:

```mog
fn factorial(n: i64) -> i64 {
  if (n <= 1) {
    return 1;
  }
  return n * factorial(n - 1);
}

fn fibonacci(n: i64) -> i64 {
  if (n <= 0) { return 0; }
  if (n == 1) { return 1; }
  a: i64 = 0;
  b: i64 = 1;
  i: i64 = 2;
  while (i <= n) {
    temp: i64 = a + b;
    a = b;
    b = temp;
    i = i + 1;
  }
  return b;
}

fn main() -> int {
  // Factorial
  println(f"5! = {factorial(5)}");
  println(f"10! = {factorial(10)}");

  // Fibonacci
  println(f"fib(10) = {fibonacci(10)}");
  println(f"fib(20) = {fibonacci(20)}");

  // Simple arithmetic
  x := 42;
  y := 13;
  println(f"{x} + {y} = {x + y}");
  println(f"{x} - {y} = {x - y}");
  println(f"{x} * {y} = {x * y}");
  println(f"{x} / {y} = {x / y}");
  println(f"{x} % {y} = {x % y}");

  return 0;
}
```

Output:

```
5! = 120
10! = 3628800
fib(10) = 55
fib(20) = 6765
42 + 13 = 55
42 - 13 = 29
42 * 13 = 546
42 / 13 = 3
42 % 13 = 3
```

A few things to notice:

- Functions are defined with `fn`, parameters have type annotations, and the return type follows `->`.
- The `factorial` function calls itself recursively. Mog supports recursion naturally.
- The `fibonacci` function uses a `while` loop with mutable variables. The `:=` operator is used inside the loop (`i = i + 1`) to reassign. Variables declared with an explicit type annotation like `a: i64 = 0` can be reassigned with `=`.
- F-strings can embed arbitrary expressions: `{x + y}` computes the sum inline.
- Integer division (`/`) truncates toward zero, and `%` gives the remainder.

Here is one more example — a program that computes the sum of the first N squares:

```mog
fn sum_of_squares(n: int) -> int {
  total := 0;
  for i in 1..n + 1 {
    total = total + (i * i);
  }
  return total;
}

fn main() -> int {
  for n in [5, 10, 20, 100] {
    println(f"Sum of squares(1..{n}) = {sum_of_squares(n)}");
  }
  return 0;
}
```

Output:

```
Sum of squares(1..5) = 55
Sum of squares(1..10) = 385
Sum of squares(1..20) = 2870
Sum of squares(1..100) = 338350
```

This example shows `for-in` loops over both ranges (`1..n + 1`) and arrays (`[5, 10, 20, 100]`). We will cover control flow in detail in a later chapter — for now, the syntax should be readable.

## What's Next

You now know how to write, compile, and run a Mog program. You have seen the basic program structure, comments, print functions, and how to define and call functions with typed parameters. Chapter 3 covers variables and bindings in depth — how `:=` and `=` work, type annotations, and scoping rules.
# Chapter 3: Variables and Bindings

Mog keeps variable declaration simple: there are no `var`, `let`, or `const` keywords. You create bindings with `:=` and reassign them with `=`. That's it.

## Creating Bindings with `:=`

The `:=` operator declares a new variable and assigns it a value. The type is inferred from the right-hand side:

```mog
name := "Alice";
age := 30;
score := 95.5;
active := true;
```

Every binding needs an initial value — there are no uninitialized variables in Mog.

```mog
fn main() {
  greeting := "hello, world";
  count := 0;
  pi := 3.14159;
  print(greeting);
  print(count);
  print(pi);
}
```

You can create bindings in sequence, and later bindings can reference earlier ones:

```mog
fn main() {
  width := 10;
  height := 20;
  area := width * height;
  print("area = {area}");  // area = 200
}
```

Bindings work inside any block scope — function bodies, `if` branches, loop bodies:

```mog
fn main() {
  x := 42;
  if x > 0 {
    label := "positive";
    print(label);
  }
  // label is not accessible here
}
```

## Reassignment with `=`

Once a variable exists, use `=` to change its value. All variables in Mog are mutable:

```mog
fn main() {
  count := 0;
  count = 1;
  count = count + 1;
  print(count);  // 2
}
```

The distinction matters: `:=` creates, `=` updates. Using `=` on a name that doesn't exist is a compile error. Using `:=` on a name that already exists in the same scope creates a new shadowed binding (see below).

```mog
fn main() {
  x := 10;
  x = 20;       // reassignment — fine
  x = x * 2;    // reassignment — x is now 40
  print(x);     // 40
}
```

A common pattern is accumulating a result in a loop:

```mog
fn main() {
  total := 0;
  for i in 1..11 {
    total = total + i;
  }
  print(total);  // 55
}
```

Building a string incrementally:

```mog
fn main() {
  result := "";
  names := ["Alice", "Bob", "Charlie"];
  for i, name in names {
    if i > 0 {
      result = result + ", ";
    }
    result = result + name;
  }
  print(result);  // Alice, Bob, Charlie
}
```

## Type Annotations

Mog infers types from the right-hand side, but you can annotate explicitly when you want to be clear about intent:

```mog
x := 42;             // inferred as int
x: int = 42;         // explicit — same result

ratio := 3.14;       // inferred as float
ratio: float = 3.14; // explicit — same result

name := "Mog";       // inferred as string
name: string = "Mog"; // explicit
```

Explicit annotations are useful when the default inference isn't what you want:

```mog
// Without annotation, 42 is int
n := 42;

// With annotation, you can specify a different integer type
n: i32 = 42;
n: u64 = 42;
```

They also help document intent in longer functions:

```mog
fn process_data(items: [string]) -> int {
  count: int = 0;
  total_length: int = 0;

  for item in items {
    count = count + 1;
    total_length = total_length + item.len;
  }

  average: float = total_length as float / count as float;
  print("average length: {average}");
  return count;
}
```

Note: function parameters and return types always require type annotations. There is no inference for function signatures:

```mog
fn add(a: int, b: int) -> int {
  return a + b;
}
```

## Shadowing

Using `:=` with a name that already exists in the current scope creates a *new* binding that shadows the old one. The old value is no longer accessible:

```mog
fn main() {
  x := 10;
  print(x);     // 10

  x := "hello";  // shadows the previous x — this is a new binding
  print(x);      // hello
}
```

Shadowing is useful when you want to transform a value and keep the same name:

```mog
fn main() {
  input := "  hello world  ";
  input := input.trim();
  input := input.upper();
  print(input);  // HELLO WORLD
}
```

Each `:=` creates a genuinely new variable. The shadowed and shadowing variables can have different types:

```mog
fn main() {
  value := 42;          // int
  value := str(value);  // string — shadows the int
  print(value);         // "42"
}
```

Inner scopes can also shadow outer variables without affecting them:

```mog
fn main() {
  x := "outer";
  if true {
    x := "inner";  // shadows outer x inside this block
    print(x);      // inner
  }
  print(x);        // outer — unchanged
}
```

Compare this to reassignment, which modifies the existing variable:

```mog
fn main() {
  x := "outer";
  if true {
    x = "modified";  // reassigns the outer x
  }
  print(x);          // modified
}
```

## Practical Examples

### Swapping Two Values

```mog
fn main() {
  a := 10;
  b := 20;

  temp := a;
  a = b;
  b = temp;

  print("a = {a}, b = {b}");  // a = 20, b = 10
}
```

### Fibonacci Sequence

```mog
fn fibonacci(n: int) -> int {
  if n <= 1 { return n; }

  a := 0;
  b := 1;
  for i in 2..n+1 {
    temp := a + b;
    a = b;
    b = temp;
  }
  return b;
}

fn main() {
  for i in 0..10 {
    print(fibonacci(i));
  }
}
```

### Processing a List

```mog
fn main() {
  scores := [85, 92, 78, 95, 88];

  sum := 0;
  max_score := scores[0];
  min_score := scores[0];

  for score in scores {
    sum = sum + score;
    if score > max_score {
      max_score = score;
    }
    if score < min_score {
      min_score = score;
    }
  }

  average := sum as float / scores.len as float;
  print("average: {average}");
  print("max: {max_score}");
  print("min: {min_score}");
}
```

### Counting Occurrences

```mog
fn count_char(s: string, target: string) -> int {
  count := 0;
  parts := s.split("");
  for ch in parts {
    if ch == target {
      count = count + 1;
    }
  }
  return count;
}

fn main() {
  text := "hello world";
  l_count := count_char(text, "l");
  print("l appears {l_count} times");  // l appears 3 times
}
```

## Summary

| Syntax | Meaning |
|---|---|
| `x := 42;` | Create a new binding (type inferred) |
| `x: int = 42;` | Create a new binding (type explicit) |
| `x = 100;` | Reassign an existing binding |
| `x := "hi";` | Shadow — create a new binding with the same name |

The rule is simple: `:=` introduces, `=` updates. When in doubt, use `:=` for new things and `=` for changing existing things.
# Chapter 4: Types and Operators

Mog is statically typed with no implicit coercion. Every value has a known type at compile time, and the compiler will reject any operation that mixes types without an explicit conversion.

## Scalar Types

Mog has four categories of scalar types: integers, floats, booleans, and strings.

### Integers

The default integer type is `int` — a 64-bit signed integer. This is what you should use for virtually all integer work:

```mog
count := 42;              // int (inferred)
count: int = 42;          // int (explicit)
negative := -17;          // int
big := 1_000_000;         // int — underscores for readability
```

Integer literals support multiple bases:

```mog
decimal := 255;           // decimal
hex := 0xFF;              // hexadecimal
binary := 0b11111111;     // binary
with_sep := 1_000_000;    // underscores ignored, just for readability
```

All four of these hold the same value: 255 (except `with_sep`, which is 1,000,000).

### Explicit-Width Integers

When precision matters — tensor element types, bitwise operations, or interop with hardware — Mog provides explicit-width integers:

```mog
small: i32 = 42;          // 32-bit signed
index: u32 = 0;           // 32-bit unsigned
offset: u64 = 1024;       // 64-bit unsigned
```

The commonly used integer types:

| Type | Width | Range |
|---|---|---|
| `int` | 64-bit signed | -2^63 to 2^63 - 1 |
| `i32` | 32-bit signed | -2^31 to 2^31 - 1 |
| `u32` | 32-bit unsigned | 0 to 2^32 - 1 |
| `u64` | 64-bit unsigned | 0 to 2^64 - 1 |

> Mog also supports `i8`, `i16`, `u8`, and `u16`, but you'll rarely need them outside of tensor element types. Use `int` unless you have a specific reason to reach for something else.

In practice, the explicit-width types exist primarily for tensors and low-level work:

```mog
// Scalar code: just use int
count := 0;
limit := 100;

// Tensor code: width matters
indices := tensor<i32>([1000]);
image := tensor<u8>([3, 224, 224]);
```

### Floats

The default float type is `float` — a 64-bit (double-precision) floating point number:

```mog
pi := 3.14159;            // float (inferred)
pi: float = 3.14159;      // float (explicit)
small := 1.0e-5;          // scientific notation
half := .5;               // leading dot is allowed
```

For single-precision, use `f32`:

```mog
ratio: f32 = 0.75;
```

The commonly used float types:

| Type | Width | Use case |
|---|---|---|
| `float` | 64-bit (double) | Default for all float math |
| `f32` | 32-bit (single) | Tensor element type, GPU work |

> Mog also supports `f16` and `bf16` (bfloat16) for ML tensor element types — see Chapter 16 for details on tensors. For scalar code, always use `float`.

```mog
// Scalar code: just use float
loss := 0.0;
learning_rate := 0.001;

// Tensor code: precision matters
weights := tensor<f16>([768, 768]);
gradients := tensor<f32>([768, 768]);
```

### Booleans

The `bool` type has two values: `true` and `false`.

```mog
active := true;
found := false;
is_valid: bool = true;
```

Booleans are returned by comparison and logical operators:

```mog
fn main() -> int {
  x := 42;
  is_positive := x > 0;       // true
  is_even := (x % 2) == 0;     // true
  both := is_positive && is_even;  // true
  println(both);
  return 0;
}
```

There are no truthy/falsy values in Mog. Conditions must be actual `bool` values — you can't use `0`, `""`, or `none` as a boolean:

```mog
count := 0;
// if count { ... }       // compile error — count is int, not bool
if count == 0 { ... }     // correct
```

> **No implicit boolean conversions.** This is deliberate — it catches a whole class of bugs at compile time. If you want a boolean, write a comparison.

### Strings

Strings are UTF-8, immutable, and double-quoted only:

```mog
name := "Alice";
greeting := "hello, world";
empty := "";
```

Strings support escape sequences:

| Escape | Meaning |
|---|---|
| `\n` | Newline |
| `\t` | Tab |
| `\\` | Backslash |
| `\"` | Double quote |

```mog
newline := "line one\nline two";
tab := "col1\tcol2";
quote := "she said \"hello\"";
backslash := "C:\\Users\\alice";
```

String interpolation uses f-strings — prefix the string with `f` and wrap expressions in `{braces}`:

```mog
name := "Alice";
age := 30;
greeting := f"hello {name}";              // "hello Alice"
info := f"{name} is {age} years old";     // "Alice is 30 years old"
math := f"2 + 2 = {2 + 2}";              // "2 + 2 = 4"
```

Concatenation uses `+`:

```mog
first := "hello";
second := " world";
combined := first + second;  // "hello world"
```

> For most cases, f-string interpolation is cleaner than concatenation — see the String Concatenation section at the end of this chapter.

## Type Conversions

Mog has no implicit coercion. If you want to convert between types, you must be explicit.

### The `as` Keyword

Use `as` to cast between numeric types:

```mog
// int to float
x := 42;
y := x as float;          // 42.0

// float to int (truncates toward zero)
pi := 3.14;
n := pi as int;            // 3

// int to narrower int
big := 1000;
small := big as i32;

// Between float widths
precise: float = 3.14159265358979;
approx := precise as f32;
```

`as` works between any numeric types:

```mog
count := 42;
count_f := count as float;   // 42.0
count_i32 := count as i32;   // 42 (as 32-bit)
count_u64 := count as u64;   // 42 (as unsigned 64-bit)
```

> **Warning:** Narrowing conversions can lose data. Casting a large `int` to `i8` will wrap on overflow — no runtime error, just silent truncation.

### String Conversions

Use `str()` to convert any scalar to a string:

```mog
s1 := str(42);         // "42"
s2 := str(3.14);       // "3.14"
s3 := str(true);       // "true"
s4 := str(false);      // "false"
```

To parse strings into numbers, use `int_from_string()` and `parse_float()`. These return `Result` because parsing can fail (see Chapter 11 for full coverage of error handling):

```mog
result := int_from_string("42");
match result {
  ok(n) => println(f"parsed: {n}"),
  err(msg) => println(f"failed: {msg}"),
}

pi := parse_float("3.14");
match pi {
  ok(f) => println(f"got: {f}"),
  err(msg) => println(f"bad float: {msg}"),
}
```

Using the `?` operator for concise error propagation:

```mog
fn parse_pair(a: string, b: string) -> Result<int> {
  x := int_from_string(a)?;
  y := int_from_string(b)?;
  return ok(x + y);
}
```

### No Implicit Conversions

Mog will not silently convert between types. Every one of these is a compile error:

```mog
x := 42;
y := 3.14;

// z := x + y;           // error: can't add int and float
z := x as float + y;     // correct: explicit cast first

// flag := x;            // error if flag is typed as bool
flag := x != 0;          // correct: explicit comparison
```

This strictness catches bugs early. If Mog rejects an expression, it's telling you to think about what conversion you actually want.

## Operators

### Arithmetic Operators

Standard math operators work on numeric types. Both operands must be the same type:

```mog
fn main() -> int {
  a := 10;
  b := 3;

  println(a + b);    // 13   addition
  println(a - b);    // 7    subtraction
  println(a * b);    // 30   multiplication
  println(a / b);    // 3    integer division (truncates)
  println(a % b);    // 1    modulo (remainder)
  return 0;
}
```

With floats:

```mog
fn main() -> int {
  a := 10.0;
  b := 3.0;

  println(a + b);    // 13.0
  println(a - b);    // 7.0
  println(a * b);    // 30.0
  println(a / b);    // 3.3333333333333335
  println(a % b);    // 1.0
  return 0;
}
```

Integer division truncates toward zero:

```mog
fn main() -> int {
  println(7 / 2);     // 3
  println(-7 / 2);    // -3
  println(7 / -2);    // -3
  return 0;
}
```

### Comparison Operators

Comparisons return `bool`. Both operands must be the same type:

```mog
fn main() -> int {
  x := 10;
  y := 20;

  println(x == y);    // false  — equal
  println(x != y);    // true   — not equal
  println(x < y);     // true   — less than
  println(x > y);     // false  — greater than
  println(x <= y);    // true   — less or equal
  println(x >= y);    // false  — greater or equal
  return 0;
}
```

Strings compare lexicographically:

```mog
fn main() -> int {
  println("apple" < "banana");    // true
  println("abc" == "abc");        // true
  println("Abc" == "abc");        // false — case sensitive
  return 0;
}
```

### Logical Operators

Logical operators work on `bool` values only:

```mog
fn main() -> int {
  a := true;
  b := false;

  println(a && b);    // false  — logical AND
  println(a || b);    // true   — logical OR
  println(!a);        // false  — logical NOT
  println(!b);        // true
  return 0;
}
```

`&&` and `||` short-circuit: the right side is only evaluated if needed:

```mog
fn check(items: [int], index: int) -> bool {
  // Safe: if index is out of bounds, the second condition is never evaluated
  return (index < items.len) && (items[index] > 0);
}
```

Combining conditions:

```mog
fn is_valid_age(age: int) -> bool {
  return (age >= 0) && (age <= 150);
}

fn needs_review(score: int, flagged: bool) -> bool {
  return (score < 50) || flagged;
}
```

### Bitwise Operators

Bitwise operators work on integer types:

```mog
fn main() -> int {
  a := 0b1100;    // 12
  b := 0b1010;    // 10

  println(a & b);    // 8    — bitwise AND
  println(a | b);    // 14   — bitwise OR
  println(a ^ b);    // 6    — bitwise XOR
  println(a << 2);   // 48   — left shift
  println(a >> 1);   // 6    — right shift
  return 0;
}
```

Common bitwise patterns:

```mog
fn main() -> int {
  // Check if a number is even
  n := 42;
  is_even := (n & 1) == 0;    // true

  // Set a flag bit
  flags := 0;
  flags = flags | 0b0100;     // set bit 2

  // Clear a flag bit
  flags = flags & 0b1011;     // clear bit 2

  // Toggle a flag bit
  flags = flags ^ 0b0010;     // toggle bit 1
  return 0;
}
```

### String Concatenation

The `+` operator concatenates strings:

```mog
fn main() -> int {
  greeting := "hello" + " " + "world";
  println(greeting);  // hello world

  name := "Alice";
  message := "hi, " + name + "!";
  println(message);  // hi, Alice!
  return 0;
}
```

For most cases, f-string interpolation is cleaner than concatenation:

```mog
// Concatenation
message := "User " + name + " scored " + str(score) + " points";

// Interpolation — prefer this
message := f"User {name} scored {score} points";
```

## Flat Operators (No Precedence)

Mog has **no operator precedence**. All binary operators are flat — the compiler does not
silently reorder operations based on a precedence table. Instead, Mog enforces explicit
grouping through two simple rules:

**1. Associative operators can chain with themselves.**
The operators `+`, `*`, `and`/`&&`, `or`/`||`, `&`, and `|` are associative, so
repeating the same one is unambiguous:

```mog
total := a + b + c;           // OK — same operator throughout
mask := READ | WRITE | EXEC;  // OK
all_ok := x && y && z;        // OK
```

**2. Everything else requires parentheses.**

- **Different operators cannot mix.** Use parentheses to show intent:

```mog
result := a + (b * c);              // OK — parens make grouping explicit
result := a + b * c;                // COMPILE ERROR — mixed + and *

check := (x > 0) && (y > 0);       // OK
check := x > 0 && y > 0;           // COMPILE ERROR — mixed > and &&

is_even := (n % 2) == 0;           // OK
is_even := n % 2 == 0;             // COMPILE ERROR — mixed % and ==
```

- **Non-associative operators cannot chain**, even with themselves:

```mog
diff := (a - b) - c;               // OK — parenthesized
diff := a - b - c;                  // COMPILE ERROR — - is non-associative

ratio := (a / b) / c;              // OK
ratio := a / b / c;                // COMPILE ERROR — / is non-associative
```

Non-associative operators: `-`, `/`, `%`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `<<`, `>>`, `^`.

### Why no precedence?

Precedence tables are a common source of bugs — especially when mixing arithmetic,
comparison, and logical operators. Mog makes every grouping decision visible in the
source code. The cost is a few extra parentheses; the benefit is that the code always
means exactly what it says.

```mog
// Clear and correct
fahrenheit := ((celsius * 9) / 5) + 32;
in_range := (x >= low) && (x <= high);
masked := (flags & MASK) != 0;
```

## Common Patterns

### Clamping a Value

```mog
fn clamp(value: int, low: int, high: int) -> int {
  if value < low { return low; }
  if value > high { return high; }
  return value;
}

fn main() -> int {
  score := 150;
  clamped := clamp(score, 0, 100);
  println(clamped);  // 100
  return 0;
}
```

### Safe Division

```mog
fn safe_divide(a: float, b: float) -> Result<float> {
  if b == 0.0 {
    return err("division by zero");
  }
  return ok(a / b);
}

fn main() -> int {
  match safe_divide(10.0, 3.0) {
    ok(result) => println(f"result: {result}"),
    err(msg) => println(f"error: {msg}"),
  }
  return 0;
}
```

### Type-Aware Accumulation

```mog
fn average(numbers: [int]) -> float {
  sum := 0;
  for n in numbers {
    sum = sum + n;
  }
  return (sum as float) / (numbers.len as float);
}

fn main() -> int {
  scores := [85, 92, 78, 95, 88];
  avg := average(scores);
  println(f"average: {avg}");  // average: 87.6
  return 0;
}
```

### Bitflag Permissions

```mog
fn main() -> int {
  READ := 1;       // 0b001
  WRITE := 2;      // 0b010
  EXEC := 4;       // 0b100

  // Grant read and write
  perms := READ | WRITE;

  // Check permissions
  can_read := (perms & READ) != 0;     // true
  can_exec := (perms & EXEC) != 0;     // false

  // Add execute permission
  perms = perms | EXEC;
  can_exec = (perms & EXEC) != 0;      // true

  println(f"read: {can_read}");
  println(f"exec: {can_exec}");
  return 0;
}
```

### Building Results with Type Conversion

```mog
fn format_percentage(value: int, total: int) -> string {
  pct := (value as float / total as float) * 100.0;
  return str(round(pct)) + "%";
}

fn main() -> int {
  passed := 87;
  total := 100;
  println(format_percentage(passed, total));  // 87%
  return 0;
}
```

## Summary

Mog's type system is small and strict:

- **Use `int` and `float`** for almost everything. Reach for `i32`, `u32`, `u64`, or `f32` only when working with tensors or hardware interop.
- **No implicit coercion.** Use `as` for numeric casts, `str()` for string conversion, `int_from_string()` and `parse_float()` for parsing.
- **Operators require matching types.** Both sides of `+`, `*`, `==`, etc. must be the same type.
- **Booleans are booleans.** No truthy/falsy — use explicit comparisons.
- **Operators are flat — no precedence.** Different operators cannot mix without parentheses, and non-associative operators cannot chain. This eliminates an entire class of bugs.
# Chapter 5: Control Flow

Mog's control flow is familiar if you've used any C-family language: `if`/`else`, `while`, `for`, `break`, `continue`, and `match`. No surprises — but a few details matter, like braces being required, `if` working as an expression, and `match` handling Result and Optional patterns.

## If/Else

The basic form: a condition, a block, and optional `else if` / `else` chains. Braces are always required. Parentheses around the condition are optional.

```mog
fn main() -> int {
  x := 42;

  if x > 0 {
    println("positive");
  } else if x == 0 {
    println("zero");
  } else {
    println("negative");
  }
  return 0;
}
```

Parentheses are allowed but not required — use them when they help readability:

```mog
fn main() -> int {
  a := 10;
  b := 20;

  // Both of these are valid
  if a > b {
    println("a wins");
  }

  if (a + b) > 25 {
    println("sum is large");
  }
  return 0;
}
```

### Nested Conditions

Chains of `else if` work exactly as you'd expect. For complex classification, they read top to bottom:

```mog
fn classify_temperature(temp: int) -> string {
  if temp >= 100 {
    return "boiling";
  } else if temp >= 80 {
    return "very hot";
  } else if temp >= 60 {
    return "warm";
  } else if temp >= 40 {
    return "cool";
  } else if temp >= 20 {
    return "cold";
  } else {
    return "freezing";
  }
}

fn main() -> int {
  println(classify_temperature(95));   // very hot
  println(classify_temperature(55));   // warm
  println(classify_temperature(-10));  // freezing
  return 0;
}
```

Nested `if` blocks inside other `if` blocks:

```mog
fn describe_number(n: int) -> string {
  if n > 0 {
    if (n % 2) == 0 {
      return "positive even";
    } else {
      return "positive odd";
    }
  } else if n < 0 {
    if (n % 2) == 0 {
      return "negative even";
    } else {
      return "negative odd";
    }
  } else {
    return "zero";
  }
}

fn main() -> int {
  println(describe_number(7));    // positive odd
  println(describe_number(-4));   // negative even
  println(describe_number(0));    // zero
  return 0;
}
```

### If as Expression

`if` can return a value. The last expression in each branch becomes the result. When used this way, `else` is required — the compiler needs a value for every case:

```mog
fn main() -> int {
  x := 42;

  sign := if x > 0 { 1 } else if x < 0 { -1 } else { 0 };
  println(f"sign of {x}: {sign}");  // sign of 42: 1
  return 0;
}
```

This works anywhere you need an expression:

```mog
fn abs(n: int) -> int {
  return if n >= 0 { n } else { 0 - n };
}

fn max(a: int, b: int) -> int {
  return if a > b { a } else { b };
}

fn min(a: int, b: int) -> int {
  return if a < b { a } else { b };
}

fn main() -> int {
  println(abs(-17));       // 17
  println(max(10, 20));    // 20
  println(min(10, 20));    // 10
  return 0;
}
```

If-expressions are useful for inline decisions without creating temporary variables:

```mog
fn format_count(n: int) -> string {
  label := if n == 1 { "item" } else { "items" };
  return f"{n} {label}";
}

fn main() -> int {
  println(format_count(1));   // 1 item
  println(format_count(5));   // 5 items
  println(format_count(0));   // 0 items
  return 0;
}
```

Combining conditions with logical operators:

```mog
fn can_vote(age: int, is_citizen: bool) -> bool {
  return (age >= 18) && is_citizen;
}

fn main() -> int {
  age := 25;
  citizen := true;

  if can_vote(age, citizen) {
    println("eligible to vote");
  } else {
    println("not eligible");
  }

  // Compound conditions
  score := 85;
  if score >= 90 {
    println("A");
  } else if (score >= 80) && (score < 90) {
    println("B");
  } else if (score >= 70) && (score < 80) {
    println("C");
  } else {
    println("below C");
  }
  return 0;
}
```

## While Loops

`while` repeats a block as long as a condition is true:

```mog
fn main() -> int {
  i := 0;
  while i < 5 {
    println(f"i = {i}");
    i = i + 1;
  }
  return 0;
}
```

### Accumulator Pattern

The most common use of `while` is accumulating a result when the loop condition depends on something more complex than a simple range:

```mog
fn sum_1_to_n(n: int) -> int {
  total := 0;
  i := 1;
  while i <= n {
    total = total + i;
    i = i + 1;
  }
  return total;
}

fn main() -> int {
  println(f"sum 1..100 = {sum_1_to_n(100)}");  // sum 1..100 = 5050
  return 0;
}
```

Factorial with a while loop:

```mog
fn factorial(n: int) -> int {
  result := 1;
  i := 2;
  while i <= n {
    result = result * i;
    i = i + 1;
  }
  return result;
}

fn main() -> int {
  println(f"5! = {factorial(5)}");    // 5! = 120
  println(f"10! = {factorial(10)}");  // 10! = 3628800
  return 0;
}
```

### Convergence Loops

`while` is the right choice when you're iterating until a condition is met, not over a known range:

```mog
fn collatz_steps(n: int) -> int {
  steps := 0;
  val := n;
  while val != 1 {
    if (val % 2) == 0 {
      val = val / 2;
    } else {
      val = (val * 3) + 1;
    }
    steps = steps + 1;
  }
  return steps;
}

fn main() -> int {
  println(f"collatz(27) = {collatz_steps(27)} steps");  // collatz(27) = 111 steps
  println(f"collatz(1) = {collatz_steps(1)} steps");    // collatz(1) = 0 steps
  return 0;
}
```

Integer square root by repeated approximation:

```mog
fn isqrt(n: int) -> int {
  if n <= 1 { return n; }
  guess := n / 2;
  while (guess * guess) > n {
    guess = (guess + (n / guess)) / 2;
  }
  return guess;
}

fn main() -> int {
  println(f"isqrt(100) = {isqrt(100)}");  // isqrt(100) = 10
  println(f"isqrt(50) = {isqrt(50)}");    // isqrt(50) = 7
  return 0;
}
```

### Infinite Loops

Use `while true` for loops that exit with `break`:

```mog
fn main() -> int {
  sum := 0;
  n := 1;
  while true {
    sum = sum + n;
    if sum > 100 {
      break;
    }
    n = n + 1;
  }
  println(f"stopped at n={n}, sum={sum}");  // stopped at n=14, sum=105
  return 0;
}
```

## For Loops

Mog has two styles of `for` loop: `for..to` with an inclusive upper bound, and `for..in` which iterates over ranges, arrays, and maps.

### For-To (Inclusive Range)

`for..to` counts from a start value to an end value, inclusive of both ends:

```mog
fn main() -> int {
  for i := 1 to 5 {
    println(f"i = {i}");
  }
  // prints: 1, 2, 3, 4, 5
  return 0;
}
```

The counter variable is scoped to the loop body — it doesn't exist outside:

```mog
fn main() -> int {
  for i := 1 to 10 {
    println(i);
  }
  // i is not accessible here
  return 0;
}
```

Summing with `for..to`:

```mog
fn main() -> int {
  total := 0;
  for i := 1 to 100 {
    total = total + i;
  }
  println(f"sum = {total}");  // sum = 5050
  return 0;
}
```

### For-In Range (Exclusive End)

The `..` range operator creates a half-open range — inclusive of the start, exclusive of the end:

```mog
fn main() -> int {
  for i in 0..5 {
    println(f"i = {i}");
  }
  // prints: 0, 1, 2, 3, 4
  return 0;
}
```

This is the natural choice for zero-based indexing:

```mog
fn main() -> int {
  names := ["Alice", "Bob", "Charlie"];
  for i in 0..names.len {
    println(f"index {i}: {names[i]}");
  }
  return 0;
}
```

Computing a power function:

```mog
fn power(base: int, exp: int) -> int {
  result := 1;
  for i in 0..exp {
    result = result * base;
  }
  return result;
}

fn main() -> int {
  println(f"2^10 = {power(2, 10)}");    // 2^10 = 1024
  println(f"3^5 = {power(3, 5)}");      // 3^5 = 243
  return 0;
}
```

### When to Use Which

| Style | Syntax | End Bound | Best For |
|---|---|---|---|
| `for..to` | `for i := 1 to 10` | Inclusive | Human-friendly ranges ("1 through 10") |
| `for..in` range | `for i in 0..10` | Exclusive | Array indexing, zero-based iteration |

```mog
fn main() -> int {
  // Print multiplication table for 7 — "1 through 10" is natural
  for i := 1 to 10 {
    println(f"7 x {i} = {7 * i}");
  }

  // Sum array elements — zero-based indexing is natural
  values := [10, 20, 30, 40, 50];
  total := 0;
  for i in 0..values.len {
    total = total + values[i];
  }
  println(f"total = {total}");  // total = 150
  return 0;
}
```

### For-In Array

Iterating directly over array elements — no index needed:

```mog
fn main() -> int {
  fruits := ["apple", "banana", "cherry", "date"];
  for fruit in fruits {
    println(f"I like {fruit}");
  }
  return 0;
}
```

Summing, filtering, searching:

```mog
fn sum(numbers: [int]) -> int {
  total := 0;
  for n in numbers {
    total = total + n;
  }
  return total;
}

fn contains(items: [string], target: string) -> bool {
  for item in items {
    if item == target {
      return true;
    }
  }
  return false;
}

fn main() -> int {
  scores := [85, 92, 78, 95, 88];
  println(f"sum = {sum(scores)}");  // sum = 438

  colors := ["red", "green", "blue"];
  println(contains(colors, "green"));  // true
  println(contains(colors, "pink"));   // false
  return 0;
}
```

Collecting results into a new array:

```mog
fn filter_even(numbers: [int]) -> [int] {
  result: [int] = [];
  for n in numbers {
    if n % 2 == 0 {
      result.push(n);
    }
  }
  return result;
}

fn main() -> int {
  nums := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
  evens := filter_even(nums);
  for n in evens {
    print_string(f"{n} ");
  }
  println("");  // 2 4 6 8 10
  return 0;
}
```

### For-In with Index

When you need both the index and the value, add a second variable before the comma:

```mog
fn main() -> int {
  names := ["Alice", "Bob", "Charlie", "Diana"];
  for i, name in names {
    println(f"{i}: {name}");
  }
  // 0: Alice
  // 1: Bob
  // 2: Charlie
  // 3: Diana
  return 0;
}
```

This is cleaner than manually tracking an index counter:

```mog
fn find_index(items: [string], target: string) -> int {
  for i, item in items {
    if item == target {
      return i;
    }
  }
  return -1;
}

fn main() -> int {
  colors := ["red", "green", "blue", "yellow"];
  idx := find_index(colors, "blue");
  println(f"blue is at index {idx}");  // blue is at index 2

  idx2 := find_index(colors, "purple");
  println(f"purple is at index {idx2}");  // purple is at index -1
  return 0;
}
```

Printing a numbered list:

```mog
fn main() -> int {
  tasks := ["write code", "run tests", "fix bugs", "deploy"];
  for i, task in tasks {
    println(f"  {i + 1}. {task}");
  }
  //   1. write code
  //   2. run tests
  //   3. fix bugs
  //   4. deploy
  return 0;
}
```

Finding the maximum element and its position:

```mog
fn max_with_index(numbers: [int]) -> [int] {
  best := numbers[0];
  best_idx := 0;
  for i, n in numbers {
    if n > best {
      best = n;
      best_idx = i;
    }
  }
  return [best_idx, best];
}

fn main() -> int {
  scores := [72, 95, 88, 91, 67];
  result := max_with_index(scores);
  println(f"max value {result[1]} at index {result[0]}");  // max value 95 at index 1
  return 0;
}
```

### For-In Map

Maps iterate as key-value pairs:

```mog
fn main() -> int {
  scores := {"alice": 95, "bob": 87, "charlie": 92};

  for name, score in scores {
    println(f"{name} scored {score}");
  }
  return 0;
}
```

Building a report from a map:

```mog
fn main() -> int {
  inventory := {"apples": 12, "bananas": 5, "oranges": 8, "grapes": 0};

  total := 0;
  out_of_stock := 0;

  for item, count in inventory {
    total = total + count;
    if count == 0 {
      println(f"  WARNING: {item} is out of stock");
      out_of_stock = out_of_stock + 1;
    }
  }

  println(f"total items: {total}");
  println(f"out of stock: {out_of_stock}");
  return 0;
}
```

Transforming map data:

```mog
fn main() -> int {
  temps_celsius := {"London": 15, "Tokyo": 28, "New York": 22, "Sydney": 19};

  for city, celsius in temps_celsius {
    fahrenheit := ((celsius * 9) / 5) + 32;
    println(f"{city}: {celsius}C = {fahrenheit}F");
  }
  return 0;
}
```

Counting occurrences with a map:

```mog
fn main() -> int {
  words := ["the", "cat", "sat", "on", "the", "mat", "the", "cat"];
  counts: {string: int} = {};

  for word in words {
    if counts[word] is some(n) {
      counts[word] = n + 1;
    } else {
      counts[word] = 1;
    }
  }

  for word, count in counts {
    println(f"{word}: {count}");
  }
  return 0;
}
```

## Break and Continue

`break` exits the innermost enclosing loop immediately. `continue` skips the rest of the current iteration and moves to the next one.

### Break

```mog
fn main() -> int {
  // Find the first multiple of 7 greater than 50
  for i in 1..100 {
    if (i * 7) > 50 {
      println(f"found: {i} (7 * {i} = {i * 7})");
      break;
    }
  }
  // found: 8 (7 * 8 = 56)
  return 0;
}
```

### Continue

```mog
fn main() -> int {
  // Print only odd numbers from 0 to 19
  for i in 0..20 {
    if (i % 2) == 0 {
      continue;
    }
    print_string(f"{i} ");
  }
  println("");  // 1 3 5 7 9 11 13 15 17 19
  return 0;
}
```

### Combined Break and Continue

```mog
fn main() -> int {
  // Sum numbers from 1 to 100, skipping multiples of 3, stopping if sum exceeds 500
  total := 0;
  stopped_at := 0;
  for i in 1..101 {
    if (i % 3) == 0 {
      continue;
    }
    total = total + i;
    if total > 500 {
      stopped_at = i;
      break;
    }
  }
  println(f"stopped at {stopped_at}, total = {total}");
  return 0;
}
```

### Break and Continue in Nested Loops

`break` and `continue` affect only the innermost loop:

```mog
fn main() -> int {
  // Find the first pair (i, j) where i * j == 42
  found_i := 0;
  found_j := 0;
  done := false;

  for i in 1..20 {
    if done { break; }
    for j in 1..20 {
      if (i * j) == 42 {
        found_i = i;
        found_j = j;
        done = true;
        break;  // breaks the inner loop
      }
    }
  }
  println(f"{found_i} * {found_j} = 42");
  return 0;
}
```

Skipping specific combinations in nested loops:

```mog
fn main() -> int {
  // Print coordinate pairs, skip the diagonal where i == j
  for i in 0..4 {
    for j in 0..4 {
      if i == j {
        continue;  // skips this iteration of the inner loop
      }
      print_string(f"({i},{j}) ");
    }
  }
  println("");
  return 0;
}
```

A practical example — searching a 2D grid:

```mog
fn main() -> int {
  grid := [
    [1, 2, 3, 4],
    [5, 6, 7, 8],
    [9, 10, 11, 12],
  ];

  target := 7;
  found_row := -1;
  found_col := -1;

  for r in 0..3 {
    for c in 0..4 {
      if grid[r][c] == target {
        found_row = r;
        found_col = c;
        break;
      }
    }
    if found_row >= 0 {
      break;
    }
  }

  if found_row >= 0 {
    println(f"found {target} at ({found_row}, {found_col})");  // found 7 at (1, 2)
  } else {
    println(f"{target} not found");
  }
  return 0;
}
```

## Match

`match` compares a value against a series of patterns and executes the first one that matches. Arms are separated by commas, and `_` is the wildcard that matches anything.

### Matching Integers

```mog
fn main() -> int {
  day := 3;
  match day {
    1 => println("Monday"),
    2 => println("Tuesday"),
    3 => println("Wednesday"),
    4 => println("Thursday"),
    5 => println("Friday"),
    6 => println("Saturday"),
    7 => println("Sunday"),
    _ => println("invalid day"),
  }
  return 0;
}
```

### Matching Strings

```mog
fn describe_color(color: string) -> string {
  return match color {
    "red" => "warm",
    "orange" => "warm",
    "yellow" => "warm",
    "blue" => "cool",
    "green" => "cool",
    "purple" => "cool",
    _ => "unknown",
  };
}

fn main() -> int {
  println(describe_color("red"));      // warm
  println(describe_color("blue"));     // cool
  println(describe_color("magenta"));  // unknown
  return 0;
}
```

### Multi-Statement Arms

When an arm needs more than one expression, wrap it in braces:

```mog
fn main() -> int {
  code := 404;
  match code {
    200 => println("OK"),
    301 => {
      println("Moved Permanently");
      println("Check the Location header");
    },
    404 => {
      println("Not Found");
      println("The resource does not exist");
    },
    500 => {
      println("Internal Server Error");
      println("Something went wrong on the server");
    },
    _ => println(f"HTTP {code}"),
  }
  return 0;
}
```

### Match as Expression

`match` returns a value, so you can assign its result:

```mog
fn main() -> int {
  score := 85;
  grade := match score / 10 {
    10 => "A+",
    9 => "A",
    8 => "B",
    7 => "C",
    6 => "D",
    _ => "F",
  };
  println(f"score {score} -> grade {grade}");  // score 85 -> grade B
  return 0;
}
```

Using match to drive computation:

```mog
fn fibonacci(n: int) -> int {
  return match n {
    0 => 0,
    1 => 1,
    _ => fibonacci(n - 1) + fibonacci(n - 2),
  };
}

fn main() -> int {
  for i in 0..10 {
    print_string(f"{fibonacci(i)} ");
  }
  println("");  // 0 1 1 2 3 5 8 13 21 34
  return 0;
}
```

Assigning different values based on a key:

```mog
fn http_status_message(code: int) -> string {
  return match code {
    200 => "OK",
    201 => "Created",
    204 => "No Content",
    301 => "Moved Permanently",
    400 => "Bad Request",
    401 => "Unauthorized",
    403 => "Forbidden",
    404 => "Not Found",
    500 => "Internal Server Error",
    502 => "Bad Gateway",
    503 => "Service Unavailable",
    _ => f"Unknown ({code})",
  };
}

fn main() -> int {
  codes := [200, 404, 500, 418];
  for code in codes {
    println(f"{code}: {http_status_message(code)}");
  }
  // 200: OK
  // 404: Not Found
  // 500: Internal Server Error
  // 418: Unknown (418)
  return 0;
}
```

### Matching on Result and Optional

Mog's `Result<T>` and Optional (`?T`) types have variants that `match` can destructure. This is a brief preview — Chapter 11 covers error handling in depth.

**Result patterns:** `ok(value)` and `err(message)`:

```mog
fn safe_divide(a: int, b: int) -> Result<int> {
  if b == 0 {
    return err("division by zero");
  }
  return ok(a / b);
}

fn main() -> int {
  result := safe_divide(42, 6);
  match result {
    ok(value) => println(f"result: {value}"),
    err(msg) => println(f"error: {msg}"),
  }

  result2 := safe_divide(10, 0);
  match result2 {
    ok(value) => println(f"result: {value}"),
    err(msg) => println(f"error: {msg}"),
  }
  // result: 7
  // error: division by zero
  return 0;
}
```

**Optional patterns:** `some(value)` and `none`:

```mog
fn find_positive(numbers: [int]) -> ?int {
  for n in numbers {
    if n > 0 {
      return some(n);
    }
  }
  return none;
}

fn main() -> int {
  result := find_positive([-3, -1, 4, -2, 5]);
  val := match result {
    some(n) => n,
    none => 0,
  };
  println(f"first positive: {val}");  // first positive: 4

  result2 := find_positive([-3, -1, -2]);
  val2 := match result2 {
    some(n) => n,
    none => 0,
  };
  println(f"first positive: {val2}");  // first positive: 0
  return 0;
}
```

Match on Result with multi-statement arms:

```mog
fn parse_and_double(input: string) -> Result<int> {
  n := int_from_string(input)?;
  return ok(n * 2);
}

fn main() -> int {
  inputs := ["21", "abc", "50"];
  for input in inputs {
    match parse_and_double(input) {
      ok(value) => {
        println(f"  '{input}' -> {value}");
      },
      err(msg) => {
        println(f"  '{input}' failed: {msg}");
      },
    }
  }
  //   '21' -> 42
  //   'abc' failed: invalid integer
  //   '50' -> 100
  return 0;
}
```

## Practical Examples

### Fibonacci (Iterative)

```mog
fn fibonacci(n: int) -> int {
  if n <= 1 { return n; }

  a := 0;
  b := 1;
  for i in 2..n+1 {
    temp := a + b;
    a = b;
    b = temp;
  }
  return b;
}

fn main() -> int {
  for i in 0..15 {
    println(f"fib({i}) = {fibonacci(i)}");
  }
  return 0;
}
```

### Sum of Squares

```mog
fn sum_of_squares(n: int) -> int {
  total := 0;
  for i := 1 to n {
    total = total + (i * i);
  }
  return total;
}

fn main() -> int {
  println(f"sum of squares 1..10 = {sum_of_squares(10)}");  // 385
  println(f"sum of squares 1..100 = {sum_of_squares(100)}");  // 338350
  return 0;
}
```

### Linear Search

```mog
fn linear_search(items: [int], target: int) -> int {
  for i, item in items {
    if item == target {
      return i;
    }
  }
  return -1;
}

fn main() -> int {
  data := [4, 8, 15, 16, 23, 42];

  idx := linear_search(data, 23);
  println(f"23 is at index {idx}");  // 23 is at index 4

  idx2 := linear_search(data, 99);
  println(f"99 is at index {idx2}");  // 99 is at index -1
  return 0;
}
```

### Bubble Sort

```mog
fn bubble_sort(arr: [int]) -> [int] {
  sorted := arr;
  n := sorted.len;
  for i in 0..n {
    for j in 0..((n - i) - 1) {
      if sorted[j] > sorted[j + 1] {
        temp := sorted[j];
        sorted[j] = sorted[j + 1];
        sorted[j + 1] = temp;
      }
    }
  }
  return sorted;
}

fn main() -> int {
  data := [64, 34, 25, 12, 22, 11, 90];
  sorted := bubble_sort(data);
  for n in sorted {
    print_string(f"{n} ");
  }
  println("");  // 11 12 22 25 34 64 90
  return 0;
}
```

### FizzBuzz

```mog
fn main() -> int {
  for i := 1 to 30 {
    if (i % 15) == 0 {
      println("FizzBuzz");
    } else if (i % 3) == 0 {
      println("Fizz");
    } else if (i % 5) == 0 {
      println("Buzz");
    } else {
      println(i);
    }
  }
  return 0;
}
```

### GCD (Euclidean Algorithm)

```mog
fn gcd(a: int, b: int) -> int {
  x := a;
  y := b;
  while y != 0 {
    temp := y;
    y = x % y;
    x = temp;
  }
  return x;
}

fn main() -> int {
  println(f"gcd(48, 18) = {gcd(48, 18)}");    // gcd(48, 18) = 6
  println(f"gcd(100, 75) = {gcd(100, 75)}");   // gcd(100, 75) = 25
  println(f"gcd(17, 13) = {gcd(17, 13)}");     // gcd(17, 13) = 1
  return 0;
}
```

### Prime Checker

```mog
fn is_prime(n: int) -> bool {
  if n < 2 { return false; }
  if n < 4 { return true; }
  if (n % 2) == 0 { return false; }

  i := 3;
  while (i * i) <= n {
    if (n % i) == 0 {
      return false;
    }
    i = i + 2;
  }
  return true;
}

fn main() -> int {
  println("Primes up to 50:");
  for n in 2..51 {
    if is_prime(n) {
      print_string(f"{n} ");
    }
  }
  println("");  // 2 3 5 7 11 13 17 19 23 29 31 37 41 43 47
  return 0;
}
```

### Command Dispatcher with Match

```mog
fn handle_command(cmd: string) -> string {
  return match cmd {
    "help" => "Available commands: help, version, quit",
    "version" => "Mog v1.0.0",
    "quit" => "Goodbye!",
    _ => f"Unknown command: {cmd}",
  };
}

fn main() -> int {
  commands := ["help", "version", "status", "quit"];
  for cmd in commands {
    println(f"> {cmd}");
    println(f"  {handle_command(cmd)}");
  }
  return 0;
}
```

## Summary

| Construct | Syntax | Notes |
|---|---|---|
| If/else | `if cond { } else { }` | Braces required, parens optional |
| If expression | `x := if cond { a } else { b };` | Returns last value of each branch |
| While | `while cond { }` | Loop until condition is false |
| For-to | `for i := 1 to 10 { }` | Inclusive upper bound |
| For-in range | `for i in 0..10 { }` | Exclusive upper bound |
| For-in array | `for item in arr { }` | Iterates values |
| For-in indexed | `for i, item in arr { }` | Iterates index-value pairs |
| For-in map | `for key, val in map { }` | Iterates key-value pairs |
| Break | `break;` | Exits innermost loop |
| Continue | `continue;` | Skips to next iteration |
| Match | `match val { pat => expr, }` | Comma-separated arms, `_` wildcard |
| Match expression | `x := match val { ... };` | Returns value from matched arm |
# Chapter 6: Functions

Functions are the primary building blocks of any Mog program. They group reusable logic behind a name, accept typed parameters, and can return values. This chapter covers everything from basic declarations to recursion and the built-in functions that ship with every Mog program.

## Basic Function Declarations

A function starts with the `fn` keyword, followed by a name, a parameter list with type annotations, an optional return type after `->`, and a body in curly braces:

```mog
fn add(a: int, b: int) -> int {
  return a + b;
}

fn multiply(x: float, y: float) -> float {
  return x * y;
}

fn is_even(n: int) -> bool {
  return (n % 2) == 0;
}
```

Every function that produces a value must use an explicit `return` statement. Mog does not support implicit returns — the last expression in a function body is not automatically returned. This keeps control flow unambiguous.

```mog
// WRONG — this compiles but returns void, discarding the result
fn broken_add(a: int, b: int) -> int {
  a + b;
}

// CORRECT
fn working_add(a: int, b: int) -> int {
  return a + b;
}
```

> **Always use `return`.** Unlike some languages where the last expression is the return value, Mog requires you to be explicit. This avoids subtle bugs when refactoring.

Multiple return points are fine when the logic calls for it:

```mog
fn classify_temperature(celsius: float) -> string {
  if celsius < 0.0 {
    return "freezing";
  }
  if celsius < 20.0 {
    return "cold";
  }
  if celsius < 35.0 {
    return "warm";
  }
  return "hot";
}
```

## Void Functions

When a function performs an action but doesn't produce a value, omit the `-> Type` annotation. The function implicitly returns void:

```mog
fn log_message(level: string, message: string) {
  println(f"[{level}] {message}");
}

fn swap(arr: [int], i: int, j: int) {
  temp := arr[i];
  arr[i] = arr[j];
  arr[j] = temp;
}

fn print_separator(width: int) {
  line := "";
  for _ in 0..width {
    line = line + "-";
  }
  println(line);
}
```

A void function can still use `return;` to exit early:

```mog
fn print_if_positive(n: int) {
  if n <= 0 {
    return;
  }
  println(n);
}
```

## Parameters and Return Types

Parameters are declared with `name: Type` syntax. Every parameter must have a type annotation — Mog does not infer parameter types (see Chapter 3 for why type annotations are required on function signatures).

```mog
fn format_price(amount: float, currency: string) -> string {
  return f"{currency} {amount}";
}

fn clamp(value: int, low: int, high: int) -> int {
  if value < low {
    return low;
  }
  if value > high {
    return high;
  }
  return value;
}
```

Functions can accept and return composite types — arrays, maps, and structs:

```mog
fn sum(numbers: [int]) -> int {
  total := 0;
  for n in numbers {
    total = total + n;
  }
  return total;
}

fn zip_names_ages(names: [string], ages: [int]) -> [{name: string, age: int}] {
  result: [{name: string, age: int}] = [];
  for i in 0..names.len {
    result.push({name: names[i], age: ages[i]});
  }
  return result;
}
```

## Named Arguments and Default Values

Functions can declare default values for parameters. Callers may then omit those arguments or pass them by name in any order:

```mog
fn greet(name: string, greeting: string = "Hello") -> string {
  return f"{greeting}, {name}!";
}

// All of these work:
greet("Alice");                        // "Hello, Alice!"
greet("Bob", "Hey");                   // "Hey, Bob!"
greet(name: "Charlie");                // "Hello, Charlie!"
greet(name: "Dave", greeting: "Hi");   // "Hi, Dave!"
greet(greeting: "Howdy", name: "Eve"); // "Howdy, Eve!"
```

Named arguments are especially useful when a function has several optional parameters:

```mog
fn create_server(
  host: string = "localhost",
  port: int = 8080,
  max_connections: int = 100,
  timeout_ms: int = 30000,
) {
  println(f"Starting server on {host}:{port}");
  println(f"Max connections: {max_connections}");
  println(f"Timeout: {timeout_ms}ms");
}

// Only override what you need:
create_server();                                // all defaults
create_server(port: 3000);                      // just change port
create_server(host: "0.0.0.0", port: 443);     // host and port
create_server(timeout_ms: 60000, port: 9090);   // any order
```

A practical example — building a configurable search:

```mog
fn search(
  query: string,
  max_results: int = 10,
  case_sensitive: bool = false,
  sort_by: string = "relevance",
) -> [string] {
  println(f"Searching for '{query}' (max={max_results}, case={case_sensitive}, sort={sort_by})");
  results: [string] = [];
  return results;
}

fn main() -> int {
  search("mog language");
  search("mog language", max_results: 50, sort_by: "date");
  search(query: "Functions", case_sensitive: true);
  return 0;
}
```

## Calling Conventions

Mog supports both positional and named calling styles. You can mix them, but positional arguments must come before named arguments:

```mog
fn send_email(to: string, subject: string, body: string = "", urgent: bool = false) {
  println(f"To: {to}");
  println(f"Subject: {subject}");
  if urgent {
    println("[URGENT]");
  }
  if body != "" {
    println(body);
  }
}

// Positional
send_email("alice@example.com", "Meeting", "See you at 3pm", true);

// Named
send_email(to: "bob@example.com", subject: "Lunch?");

// Mixed: positional first, then named
send_email("carol@example.com", "Report", urgent: true);
```

> **Rule of thumb:** Use positional args for 1-2 required parameters. Use named args when a function has 3+ parameters or when the call site would otherwise be ambiguous.

## Recursion

Functions can call themselves. Mog does not guarantee tail-call optimization, so deep recursion will consume stack space proportional to the call depth.

**Factorial:**

```mog
fn factorial(n: int) -> int {
  if n <= 1 {
    return 1;
  }
  return n * factorial(n - 1);
}

fn main() -> int {
  println(factorial(5));   // 120
  println(factorial(10));  // 3628800
  return 0;
}
```

**Fibonacci:**

```mog
fn fibonacci(n: int) -> int {
  if n <= 0 {
    return 0;
  }
  if n == 1 {
    return 1;
  }
  return fibonacci(n - 1) + fibonacci(n - 2);
}

fn main() -> int {
  for i in 0..10 {
    println(fibonacci(i));
  }
  // 0, 1, 1, 2, 3, 5, 8, 13, 21, 34
  return 0;
}
```

**Binary search — recursion with arrays:**

```mog
fn binary_search(arr: [int], target: int, low: int, high: int) -> int {
  if low > high {
    return -1;
  }
  mid := (low + high) / 2;
  if arr[mid] == target {
    return mid;
  }
  if arr[mid] < target {
    return binary_search(arr, target, mid + 1, high);
  }
  return binary_search(arr, target, low, mid - 1);
}

fn main() -> int {
  data := [2, 5, 8, 12, 16, 23, 38, 56, 72, 91];
  idx := binary_search(data, 23, 0, data.len - 1);
  println(f"Found 23 at index {idx}");  // Found 23 at index 5
  return 0;
}
```

**Greatest common divisor:**

```mog
fn gcd(a: int, b: int) -> int {
  if b == 0 {
    return a;
  }
  return gcd(b, a % b);
}

fn main() -> int {
  println(gcd(48, 18));   // 6
  println(gcd(100, 75));  // 25
  return 0;
}
```

> **Tip:** For deep or performance-sensitive recursion, consider rewriting with a loop (see Chapter 5). The recursive Fibonacci above is O(2^n) — the iterative version is O(n):

```mog
// Iterative fibonacci — much faster for large n
fn fibonacci_fast(n: int) -> int {
  if n <= 0 {
    return 0;
  }
  a := 0;
  b := 1;
  for _ in 1..n {
    temp := b;
    b = a + b;
    a = temp;
  }
  return b;
}
```

## Math Builtins

Mog provides a set of math functions as builtins. No imports required. All math builtins operate on `float` (f64) values.

**Single-argument functions:**

| Function | Description |
|----------|-------------|
| `sqrt(x)` | Square root |
| `sin(x)` | Sine (radians) |
| `cos(x)` | Cosine (radians) |
| `tan(x)` | Tangent (radians) |
| `asin(x)` | Arcsine |
| `acos(x)` | Arccosine |
| `exp(x)` | e^x |
| `log(x)` | Natural logarithm |
| `log2(x)` | Base-2 logarithm |
| `floor(x)` | Round down |
| `ceil(x)` | Round up |
| `round(x)` | Round to nearest |
| `abs(x)` | Absolute value |

**Two-argument functions:**

| Function | Description |
|----------|-------------|
| `pow(x, y)` | x raised to the power y |
| `atan2(y, x)` | Two-argument arctangent |
| `min(a, b)` | Smaller of two values |
| `max(a, b)` | Larger of two values |

> All math builtins take and return `float`. If you have an `int`, cast it first with `as float` (see Chapter 4).

Examples:

```mog
// Distance between two points
fn distance(x1: float, y1: float, x2: float, y2: float) -> float {
  dx := x2 - x1;
  dy := y2 - y1;
  return sqrt((dx * dx) + (dy * dy));
}

fn main() -> int {
  println(distance(0.0, 0.0, 3.0, 4.0));  // 5.0
  return 0;
}
```

```mog
// Convert degrees to radians and compute trig values
fn deg_to_rad(degrees: float) -> float {
  return (degrees * 3.14159265) / 180.0;
}

fn main() -> int {
  angle := deg_to_rad(45.0);
  println(sin(angle));   // ~0.7071
  println(cos(angle));   // ~0.7071
  return 0;
}
```

```mog
// Compound interest
fn compound_interest(principal: float, rate: float, years: int) -> float {
  return principal * pow(1.0 + rate, years as float);
}

fn main() -> int {
  result := compound_interest(1000.0, 0.05, 10);
  println(round(result));  // 1629.0
  return 0;
}
```

```mog
// Clamp a float between bounds
fn clamp_float(value: float, lo: float, hi: float) -> float {
  return max(lo, min(hi, value));
}

fn main() -> int {
  println(clamp_float(150.0, 0.0, 100.0));  // 100.0
  println(clamp_float(-5.0, 0.0, 100.0));   // 0.0
  return 0;
}
```

```mog
// Estimate how many bits are needed to represent n
fn bits_needed(n: int) -> int {
  if n <= 0 {
    return 1;
  }
  return floor(log2(n as float)) as int + 1;
}

fn main() -> int {
  println(bits_needed(255));   // 8
  println(bits_needed(256));   // 9
  return 0;
}
```

## Other Builtins

Mog provides several non-math builtins that are available without imports.

### Output Functions

```mog
fn main() -> int {
  // println auto-dispatches by type
  println(42);           // prints "42\n"
  println(3.14);         // prints "3.14\n"
  println("hello");      // prints "hello\n"
  println(true);         // prints "true\n"

  // Type-specific print (no newline)
  print_string("name: ");
  print_i64(42);
  print_f64(3.14);
  return 0;
}
```

### Conversion Functions

**`str(value)`** — Convert an int, float, or bool to a string:

```mog
s1 := str(42);       // "42"
s2 := str(3.14);     // "3.14"
s3 := str(true);     // "true"

println("The answer is " + str(42));
```

**`len(array)`** — Get the length of an array as an int:

```mog
numbers := [10, 20, 30, 40];
println(len(numbers));  // 4

empty: [string] = [];
println(len(empty));    // 0
```

**`int_from_string(s)`** — Parse a string into an int. Returns `Result<int>` because the parse can fail (see Chapter 11 for full coverage of Result):

```mog
result := int_from_string("42");
match result {
  ok(n) => println(f"Parsed: {n}"),
  err(msg) => println(f"Failed: {msg}"),
}

// Using ? to propagate errors
fn read_port(input: string) -> Result<int> {
  port := int_from_string(input)?;
  if (port < 1) || (port > 65535) {
    return err("port out of range");
  }
  return ok(port);
}
```

**`parse_float(s)`** — Parse a string into a float. Returns `Result<float>`:

```mog
result := parse_float("3.14159");
match result {
  ok(f) => println(f"Got pi: {f}"),
  err(msg) => println(f"Not a float: {msg}"),
}

// Practical use: parsing user input
fn parse_temperature(input: string) -> Result<float> {
  temp := parse_float(input)?;
  if temp < -273.15 {
    return err("below absolute zero");
  }
  return ok(temp);
}
```

### A Complete Example

Combining functions, conversions, and error handling:

```mog
fn parse_csv_row(line: string) -> Result<{name: string, age: int, score: float}> {
  parts := line.split(",");
  if len(parts) != 3 {
    return err(f"expected 3 fields, got {len(parts)}");
  }
  age := int_from_string(parts[1])?;
  score := parse_float(parts[2])?;
  return ok({name: parts[0], age: age, score: score});
}

fn main() -> int {
  row := parse_csv_row("Alice,30,95.5");
  match row {
    ok(data) => println(f"{data.name} is {str(data.age)} years old with score {str(data.score)}"),
    err(msg) => println(f"Parse error: {msg}"),
  }
  return 0;
}
```

## Summary

| Feature | Syntax | Notes |
|---|---|---|
| Declaration | `fn name(p: T) -> R { }` | `fn` keyword, typed params, explicit return type |
| Void function | `fn name(p: T) { }` | Omit `-> R` for side-effect-only functions |
| Return | `return value;` | Required — no implicit returns |
| Default args | `fn f(x: int = 10)` | Caller can omit or pass by name |
| Named call | `f(x: 42, y: 10)` | Named args can be in any order |
| Mixed call | `f(42, y: 10)` | Positional before named |
| Recursion | `fn f(n: int) { f(n-1); }` | No guaranteed tail-call optimization |
# Chapter 7: Closures and Higher-Order Functions

In the previous chapter, functions were always named and declared at the top level. Mog also supports **closures** — anonymous functions that can be created inline, stored in variables, passed as arguments, and returned from other functions. When combined with higher-order functions (functions that accept or return other functions), closures unlock powerful and concise patterns for working with data.

## Closure Syntax

A closure is an anonymous function written with the `fn` keyword but without a name. It is typically assigned to a variable or passed directly as an argument.

```mog
add := fn(a: int, b: int) -> int { return a + b; };

print(add(3, 4));  // 7
```

The syntax mirrors named functions: parameters with types, an optional return type, and a body in curly braces. The trailing semicolon is required because the closure assignment is a statement.

```mog
square := fn(n: int) -> int { return n * n; };
is_positive := fn(x: float) -> bool { return x > 0.0; };
greet := fn(name: string) -> string { return "hello, {name}!"; };

print(square(5));         // 25
print(is_positive(-3.0)); // false
print(greet("Alice"));    // hello, Alice!
```

Closures that take no parameters and return nothing work too:

```mog
say_hi := fn() { print("hi"); };
say_hi();  // hi
```

## Capturing Variables

Closures can reference variables from their enclosing scope. This is what distinguishes a closure from a plain function pointer — it "closes over" the environment where it was created.

```mog
multiplier := 3;
triple := fn(n: int) -> int { return n * multiplier; };

print(triple(10));  // 30
print(triple(7));   // 21
```

Closures can capture multiple variables:

```mog
base_url := "https://api.example.com";
api_key := "sk-12345";

make_url := fn(endpoint: string) -> string {
  return "{base_url}/{endpoint}?key={api_key}";
};

print(make_url("users"));  // https://api.example.com/users?key=sk-12345
```

### Value Capture Semantics

Mog captures variables **by value** — the closure gets a snapshot of each captured variable at the moment the closure is created. Later changes to the original do not affect the closure's copy.

```mog
count := 10;
get_count := fn() -> int { return count; };

count = 20;
print(get_count());  // 10 — captured the value 10
print(count);        // 20 — the original is 20
```

This matters in loops. Each iteration creates a new closure that captures the loop variable's current value:

```mog
makers: [fn() -> int] = [];
for i in 0..5 {
  makers.push(fn() -> int { return i; });
}

print(makers[0]());  // 0
print(makers[3]());  // 3
```

> Internally, closures are implemented as a fat pointer: `{fn_ptr, env_ptr}`. The runtime copies only the variables the closure actually references. This is an implementation detail you rarely need to think about.

## Type Aliases for Function Types

Function type signatures can get verbose. Use `type` to create aliases:

```mog
type Predicate = fn(int) -> bool;
type Transform = fn(int) -> int;
type Callback = fn(string);
```

These aliases simplify function signatures:

```mog
type Transform = fn(int) -> int;

fn apply_twice(f: Transform, value: int) -> int {
  return f(f(value));
}

double := fn(n: int) -> int { return n * 2; };
print(apply_twice(double, 3));  // 12
```

Without the alias, the signature would be `fn apply_twice(f: fn(int) -> int, value: int) -> int` — correct but harder to read at a glance.

```mog
type Predicate = fn(int) -> bool;
type Formatter = fn(int) -> string;

fn find_first(items: [int], pred: Predicate) -> ?int {
  for item in items {
    if pred(item) { return some(item); }
  }
  return none;
}

fn format_all(items: [int], fmt: Formatter) -> [string] {
  return items.map(fmt);
}
```

## Passing Closures to Functions

Closures are first-class values. You can pass them as arguments using the function type syntax `fn(ParamTypes) -> ReturnType`.

```mog
fn apply(f: fn(int) -> int, x: int) -> int {
  return f(x);
}

double := fn(n: int) -> int { return n * 2; };
negate := fn(n: int) -> int { return -n; };

print(apply(double, 5));   // 10
print(apply(negate, 5));   // -5
```

You can pass closures inline without naming them:

```mog
print(apply(fn(n: int) -> int { return n * n; }, 4));  // 16
```

A function that transforms every element of an array:

```mog
fn transform(arr: [int], f: fn(int) -> int) -> [int] {
  result: [int] = [];
  for item in arr {
    result.push(f(item));
  }
  return result;
}

numbers := [1, 2, 3, 4, 5];

doubled := transform(numbers, fn(n: int) -> int { return n * 2; });
print(doubled);  // [2, 4, 6, 8, 10]

offset := 100;
shifted := transform(numbers, fn(n: int) -> int { return n + offset; });
print(shifted);  // [101, 102, 103, 104, 105]
```

## Returning Closures from Functions

Functions can create and return closures. The returned closure retains access to any variables it captured — even after the enclosing function has returned.

```mog
fn make_adder(n: int) -> fn(int) -> int {
  return fn(x: int) -> int { return x + n; };
}

add5 := make_adder(5);
add100 := make_adder(100);

print(add5(3));    // 8
print(add100(3));  // 103
```

```mog
fn make_multiplier(factor: float) -> fn(float) -> float {
  return fn(x: float) -> float { return x * factor; };
}

to_km := make_multiplier(1.60934);
print(to_km(10.0));  // 16.0934
```

This factory pattern is useful for creating families of related functions from a single template. You will see it again in Chapter 9 when we build constructor functions for structs.

## Closures with Array Methods

Mog arrays have built-in methods — `filter`, `map`, and `sort` — that accept closures. These methods return new arrays; they do not modify the original. See Chapter 10 for the full set of collection operations.

### filter

`filter` takes a predicate closure and returns a new array containing only the elements for which the predicate returns `true`.

```mog
numbers := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

evens := numbers.filter(fn(n: int) -> bool { return (n % 2) == 0; });
print(evens);  // [2, 4, 6, 8, 10]

big := numbers.filter(fn(n: int) -> bool { return n > 7; });
print(big);  // [8, 9, 10]
```

Filter with a captured threshold:

```mog
scores := [45, 72, 88, 91, 53, 67, 79, 95];
cutoff := 70;

passing := scores.filter(fn(s: int) -> bool { return s >= cutoff; });
print(passing);  // [72, 88, 91, 79, 95]
```

### map

`map` takes a transform closure and returns a new array with each element replaced by the closure's result.

```mog
numbers := [1, 2, 3, 4, 5];

doubled := numbers.map(fn(n: int) -> int { return n * 2; });
print(doubled);  // [2, 4, 6, 8, 10]

labels := numbers.map(fn(n: int) -> string { return "item-{str(n)}"; });
print(labels);  // ["item-1", "item-2", "item-3", "item-4", "item-5"]
```

```mog
names := ["alice", "bob", "carol"];
lengths := names.map(fn(name: string) -> int { return name.len; });
print(lengths);  // [5, 3, 5]
```

### sort

`sort` takes a comparator closure that returns `true` when the first argument should come before the second. It returns a new sorted array.

```mog
numbers := [5, 2, 8, 1, 9, 3];

ascending := numbers.sort(fn(a: int, b: int) -> bool { return a < b; });
print(ascending);  // [1, 2, 3, 5, 8, 9]

descending := numbers.sort(fn(a: int, b: int) -> bool { return a > b; });
print(descending);  // [9, 8, 5, 3, 2, 1]
```

Sorting structs by a specific field:

```mog
struct Player {
  name: string,
  score: int,
}

players := [
  Player{name: "Alice", score: 250},
  Player{name: "Bob", score: 180},
  Player{name: "Carol", score: 320},
];

by_score := players.sort(fn(a: Player, b: Player) -> bool {
  return a.score > b.score;
});

for p in by_score {
  print("{p.name}: {str(p.score)}");
}
// Carol: 320
// Alice: 250
// Bob: 180
```

### Chaining Methods

Filter, map, and sort can be chained for expressive data pipelines:

```mog
numbers := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

result := numbers
  .filter(fn(n: int) -> bool { return (n % 2) == 0; })
  .map(fn(n: int) -> int { return n * n; })
  .sort(fn(a: int, b: int) -> bool { return a > b; });

print(result);  // [100, 64, 36, 16, 4]
```

> Method chaining reads top-to-bottom: filter the evens, square them, sort descending. Each step returns a new array, so the original `numbers` is untouched.

## Summary

| Concept | Syntax |
|---|---|
| Create a closure | `fn(params) -> Type { body }` |
| Type alias | `type Name = fn(ParamTypes) -> ReturnType;` |
| Pass to function | `fn do_it(f: fn(int) -> int) { ... }` |
| Return from function | `fn make() -> fn(int) -> int { ... }` |
| Filter an array | `arr.filter(fn(x: T) -> bool { ... })` |
| Map an array | `arr.map(fn(x: T) -> U { ... })` |
| Sort an array | `arr.sort(fn(a: T, b: T) -> bool { ... })` |

Closures capture by value, are first-class values, and combine naturally with array methods for concise data processing. In the next chapter, we will look at Mog's string type in detail.
# Chapter 8: Strings

Strings are one of the most frequently used types in any language. Mog strings are immutable, UTF-8 encoded, and garbage-collected — you create them, pass them around, and the runtime handles the rest. This chapter covers everything from basic literals and escape sequences to interpolation, methods, and parsing.

## String Basics

Mog strings use double quotes only. There are no single-quoted strings, no raw strings, and no heredocs. A string literal is a sequence of bytes between `"` and `"`, encoded as UTF-8 and null-terminated internally.

```mog
fn main() -> int {
  greeting := "Hello, world!";
  println(greeting);

  empty := "";
  println(empty);  // prints an empty line

  return 0;
}
```

### Escape Sequences

Mog supports the standard escape sequences you'd expect:

| Escape | Meaning |
|--------|---------|
| `\n` | Newline |
| `\t` | Tab |
| `\\` | Literal backslash |
| `\"` | Literal double quote |

```mog
fn main() -> int {
  multiline := "Line 1\nLine 2\nLine 3";
  println(multiline);
  // Line 1
  // Line 2
  // Line 3

  tabbed := "Name:\tAlice\nAge:\t30";
  println(tabbed);
  // Name:	Alice
  // Age:	30

  path := "C:\\Users\\alice\\docs";
  println(path);  // C:\Users\alice\docs

  quoted := "She said \"hello\" and left.";
  println(quoted);  // She said "hello" and left.

  return 0;
}
```

### Immutability

Strings in Mog are immutable reference types. You can rebind a variable to a new string, but you cannot modify a string's contents in place. Every operation that "changes" a string — concatenation, `upper()`, `replace()` — returns a new string.

```mog
fn main() -> int {
  s := "hello";
  s = s.upper();  // rebinds s to a new string "HELLO"
  println(s);     // HELLO

  return 0;
}
```

### UTF-8 Encoding

Mog strings are UTF-8 encoded, so they handle international text and emoji naturally. Keep in mind that `.len` returns the **byte count**, not the number of characters — multibyte characters take more than one byte.

```mog
fn main() -> int {
  cafe := "café au lait";
  println(cafe);

  emoji := "Hello! 👍";
  println(emoji);

  chinese := "你好世界";
  println(chinese);

  japanese := "こんにちは";
  println(japanese);

  // Byte lengths — not character counts
  ascii := "hello";
  println(ascii.len);    // 5 — one byte per character

  return 0;
}
```

## String Interpolation

Mog supports f-strings — string literals prefixed with `f` that can embed expressions inside `{}` braces. This is the most readable way to build strings from mixed data.

```mog
fn main() -> int {
  name := "Alice";
  age := 30;
  println(f"Hello, {name}!");              // Hello, Alice!
  println(f"You are {age} years old.");    // You are 30 years old.

  return 0;
}
```

Expressions inside `{}` can be arithmetic, function calls, or any expression that produces a value. The result is automatically converted to a string.

```mog
fn main() -> int {
  age := 30;
  println(f"Next year you'll be {age + 1}.");  // Next year you'll be 31.

  x := 7;
  println(f"{x} squared is {x * x}.");  // 7 squared is 49.

  price := 9.99;
  qty := 3;
  println(f"Total: {price * 3.0}");  // Total: 29.970000

  return 0;
}
```

You can have multiple interpolations in a single string, and they can appear anywhere — at the start, middle, or end.

```mog
fn main() -> int {
  a := 10;
  b := 20;
  println(f"{a} + {b} = {a + b}");  // 10 + 20 = 30

  name := "World";
  println(f"[{name}]");  // [World]

  return 0;
}
```

If you need a literal `{` or `}` in an f-string, escape it by doubling: `{{` and `}}`.

```mog
fn main() -> int {
  x := 42;
  println(f"The value is: {x} (in braces: {{{x}}})");
  // The value is: 42 (in braces: {42})

  return 0;
}
```

## String Concatenation

The `+` operator joins two strings together, returning a new string. You can chain multiple `+` operations.

```mog
fn main() -> int {
  first := "Hello";
  second := " World";
  result := first + second;
  println(result);  // Hello World

  full := "one" + " " + "two" + " " + "three";
  println(full);  // one two three

  return 0;
}
```

To concatenate a non-string value, convert it first with `str()`.

```mog
fn main() -> int {
  count := 42;
  msg := "Count: " + str(count);
  println(msg);  // Count: 42

  pi := 3.14;
  info := "Pi is approximately " + str(pi);
  println(info);  // Pi is approximately 3.140000

  return 0;
}
```

For most cases, f-strings are cleaner than manual concatenation — they handle the conversion automatically and are easier to read.

```mog
fn main() -> int {
  name := "Alice";
  score := 95;

  // Concatenation — works, but verbose
  msg1 := name + " scored " + str(score) + " points.";

  // F-string — same result, easier to read
  msg2 := f"{name} scored {score} points.";

  println(msg1);  // Alice scored 95 points.
  println(msg2);  // Alice scored 95 points.

  return 0;
}
```

## String Methods

Mog strings have built-in methods for common operations. These are called with dot syntax on any string value.

### `.len`

Returns the byte length of the string. This is a property, not a function call.

```mog
fn main() -> int {
  s := "hello";
  println(s.len);  // 5

  empty := "";
  println(empty.len);  // 0

  // Multibyte characters take more than one byte
  accent := "café";
  println(accent.len);  // 5 — the é is 2 bytes

  return 0;
}
```

### `.contains(substr)`

Returns true if the string contains the given substring.

```mog
fn main() -> int {
  s := "hello world";

  if s.contains("world") {
    println("Found 'world'");
  }

  if s.contains("xyz") == false {
    println("No 'xyz' here");
  }

  // Case-sensitive
  if s.contains("Hello") == false {
    println("'Hello' not found — case matters");
  }

  return 0;
}
```

### `.starts_with(prefix)` and `.ends_with(suffix)`

Check whether a string begins or ends with a given substring.

```mog
fn main() -> int {
  path := "/usr/local/bin/mog";

  if path.starts_with("/usr") {
    println("System path");  // prints
  }

  if path.ends_with(".mog") == false {
    println("Not a .mog file");  // prints
  }

  filename := "report.csv";
  if filename.ends_with(".csv") {
    println("CSV file detected");  // prints
  }

  return 0;
}
```

### `.upper()` and `.lower()`

Return a new string with all ASCII characters converted to uppercase or lowercase.

```mog
fn main() -> int {
  s := "Hello World";
  println(s.upper());  // HELLO WORLD
  println(s.lower());  // hello world
  println(s);          // Hello World — original unchanged

  // Useful for case-insensitive comparison
  input := "Yes";
  if input.lower() == "yes" {
    println("Confirmed");
  }

  return 0;
}
```

### `.trim()`

Returns a new string with leading and trailing whitespace removed.

```mog
fn main() -> int {
  raw := "  hello  ";
  cleaned := raw.trim();
  println(cleaned);       // hello
  println(cleaned.len);   // 5

  padded := "\t spaced \n";
  println(padded.trim());  // spaced

  return 0;
}
```

### `.replace(old, new)`

Returns a new string with all occurrences of `old` replaced by `new`.

```mog
fn main() -> int {
  s := "hello world";
  println(s.replace("world", "Mog"));  // hello Mog

  csv := "a,b,c,d";
  println(csv.replace(",", " | "));  // a | b | c | d

  // Replaces all occurrences, not just the first
  repeated := "aaa";
  println(repeated.replace("a", "bb"));  // bbbbbb

  return 0;
}
```

### `.split(delimiter)`

Splits a string into an array of substrings at each occurrence of the delimiter.

```mog
fn main() -> int {
  csv := "alice,bob,carol";
  parts := csv.split(",");

  for i, name in parts {
    println(f"{i}: {name}");
  }
  // 0: alice
  // 1: bob
  // 2: carol

  return 0;
}
```

### `.index_of(substr)`

Returns the byte offset of the first occurrence of `substr`, or -1 if not found.

```mog
fn main() -> int {
  s := "hello world";
  println(s.index_of("world"));  // 6
  println(s.index_of("xyz"));    // -1

  return 0;
}
```

### Method Chaining

String methods return new strings, so you can chain them.

```mog
fn main() -> int {
  raw := "  Hello, World!  ";
  result := raw.trim().lower().replace("world", "mog");
  println(result);  // hello, mog!

  return 0;
}
```

## String Slicing

You can extract a substring using bracket syntax with a range: `s[start:end]`. Both `start` and `end` are byte offsets. The slice includes `start` and excludes `end`.

```mog
fn main() -> int {
  s := "hello world";
  println(s[0:5]);   // hello
  println(s[6:11]);  // world

  // Single character access
  println(s[0]);  // h
  println(s[4]);  // o

  // Using variables
  start := 6;
  end := 11;
  println(s[start:end]);  // world

  return 0;
}
```

## String Comparison

Use `==` and `!=` to compare string contents. These compare the actual bytes, not pointer identity.

```mog
fn main() -> int {
  a := "hello";
  b := "hello";
  c := "world";

  if a == b {
    println("a and b are equal");  // prints
  }

  if a != c {
    println("a and c are different");  // prints
  }

  return 0;
}
```

Comparisons are case-sensitive. Use `.lower()` or `.upper()` if you need case-insensitive matching.

```mog
fn main() -> int {
  input := "YES";
  expected := "yes";

  if input == expected {
    println("exact match");
  }

  if input.lower() == expected {
    println("case-insensitive match");  // prints
  }

  return 0;
}
```

## Conversions

### To String: `str()`

The `str()` function converts integers and floats to their string representation.

```mog
fn main() -> int {
  s1 := str(42);
  println(s1);  // 42

  s2 := str(-7);
  println(s2);  // -7

  s3 := str(3.14);
  println(s3);  // 3.140000

  // Useful for concatenation
  label := "Score: " + str(100);
  println(label);  // Score: 100

  return 0;
}
```

### From String: Parsing

Mog provides two sets of parsing functions with different error-handling strategies.

**Safe parsing** — `int_from_string()` and `float_from_string()` return a `Result` type that you can match on for error handling (see Chapter 11 for details on Result types):

```mog
fn main() -> int {
  r := int_from_string("42");
  // r is a Result<int> — use match to handle success or failure

  return 0;
}
```

**Simple parsing** — `parse_int()` and `parse_float()` return the value directly, giving 0 or 0.0 on failure:

```mog
fn main() -> int {
  n := parse_int("123");
  println(n);  // 123

  f := parse_float("3.14");
  println(f);  // 3.140000

  // Invalid input returns 0
  bad := parse_int("abc");
  println(bad);  // 0

  return 0;
}
```

Use `parse_int` and `parse_float` when you trust the input or have already validated it. Use `int_from_string` and `float_from_string` when you need to handle errors explicitly.

## Print Functions

Mog provides generic `print` and `println` functions that automatically dispatch based on the argument type. There are also type-specific variants when you need precise control.

### Generic Printing

`println()` detects the argument type and calls the appropriate variant:

```mog
fn main() -> int {
  println("hello");   // dispatches to println_string
  println(42);        // dispatches to println_i64
  println(3.14);      // dispatches to println_f64

  return 0;
}
```

### Type-Specific Variants

These print without a trailing newline, which is useful for building output piece by piece:

| Function | Description |
|----------|-------------|
| `print_string(s)` | Print a string, no newline |
| `print_i64(n)` | Print an integer, no newline |
| `print_f64(f)` | Print a float, no newline |
| `println_string(s)` | Print a string with newline |
| `println_i64(n)` | Print an integer with newline |
| `println_f64(f)` | Print a float with newline |

```mog
fn main() -> int {
  print_string("Loading");
  print_string(".");
  print_string(".");
  print_string(".");
  println_string(" done!");
  // Loading... done!

  print_string("Value: ");
  print_i64(42);
  print_string("\n");
  // Value: 42

  return 0;
}
```

## Building Strings

### Concatenation in Loops

You can build a string incrementally by concatenating in a loop. Since strings are immutable, each `+` creates a new string.

```mog
fn main() -> int {
  arr := [1, 2, 3, 4, 5];
  result := "";
  for i, v in arr {
    if i > 0 {
      result = result + ", ";
    }
    result = result + str(v);
  }
  println(result);  // 1, 2, 3, 4, 5

  return 0;
}
```

### Formatting Tables

F-strings and concatenation let you format aligned output:

```mog
fn main() -> int {
  println("Name          Score");
  println("----          -----");
  println(f"Alice         {95}");
  println(f"Bob           {87}");
  println(f"Carol         {92}");

  return 0;
}
```

### Building Messages

F-strings shine when assembling messages with mixed data:

```mog
fn main() -> int {
  user := "alice";
  action := "login";
  code := 200;

  log_msg := f"[{code}] User '{user}' performed '{action}'";
  println(log_msg);
  // [200] User 'alice' performed 'login'

  items := 3;
  total := 29.97;
  receipt := f"You purchased {items} items for a total of {total}";
  println(receipt);

  return 0;
}
```

### Parsing with Validation

A common pattern is parsing user input and handling the case where it might not be valid:

```mog
fn main() -> int {
  inputs := ["42", "hello", "100", "3.14"];

  for i, s in inputs {
    n := parse_int(s);
    if n != 0 {
      println(f"Parsed '{s}' as {n}");
    } else {
      if s == "0" {
        println(f"Parsed '{s}' as 0");
      } else {
        println(f"Could not parse '{s}'");
      }
    }
  }

  return 0;
}
```

## Summary

| Operation | Syntax | Returns |
|-----------|--------|---------|
| Create string | `"hello"` | `string` |
| Interpolation | `f"value is {x}"` | `string` |
| Concatenation | `a + b` | `string` |
| Byte length | `s.len` | `int` |
| Contains | `s.contains("x")` | `bool` |
| Starts with | `s.starts_with("x")` | `bool` |
| Ends with | `s.ends_with("x")` | `bool` |
| Uppercase | `s.upper()` | `string` |
| Lowercase | `s.lower()` | `string` |
| Trim whitespace | `s.trim()` | `string` |
| Replace | `s.replace("a", "b")` | `string` |
| Split | `s.split(",")` | `[string]` |
| Index of | `s.index_of("x")` | `int` |
| Slice | `s[0:5]` | `string` |
| Char at | `s[0]` | `string` |
| To string | `str(42)` | `string` |
| Parse int (safe) | `int_from_string("42")` | `Result<int>` |
| Parse float (safe) | `float_from_string("3.14")` | `Result<float>` |
| Parse int (simple) | `parse_int("42")` | `int` |
| Parse float (simple) | `parse_float("3.14")` | `float` |
| Equality | `a == b`, `a != b` | `bool` |

Strings are straightforward in Mog — double-quoted, immutable, UTF-8, and garbage-collected. For error handling with `int_from_string` and `float_from_string`, see Chapter 11 on Result types.
# Chapter 9: Structs

Structs are Mog's way of grouping related data under a single name. They are simple named product types with typed fields — no methods, no inheritance, no interfaces. You define the shape, construct instances, and pass them around. Functions that operate on structs live outside the struct as standalone functions.

## Declaring Structs

A struct declaration lists named fields with their types, separated by commas:

```mog
struct Point {
  x: int,
  y: int,
}
```

Fields can be any type — scalars, strings, arrays, maps, or other structs:

```mog
struct Color {
  r: int,
  g: int,
  b: int,
  a: float,
}

struct Config {
  name: string,
  version: int,
  debug: bool,
  tags: [string],
}

struct User {
  id: int,
  username: string,
  email: string,
  active: bool,
}
```

Structs are always declared at the top level. They cannot be declared inside functions or other structs.

## Constructing Instances

Create a struct instance by naming the type and providing values for all fields inside braces:

```mog
fn main() {
  p := Point { x: 10, y: 20 };
  c := Color { r: 255, g: 128, b: 0, a: 1.0 };
  cfg := Config { name: "myapp", version: 3, debug: false, tags: ["prod", "v3"] };
}
```

Every field must be provided. There are no default values and no partial construction — if a struct has four fields, you supply four values:

```mog
fn main() {
  // This is a compile error — missing field `a`:
  // c := Color { r: 255, g: 128, b: 0 };

  // All fields required:
  c := Color { r: 255, g: 128, b: 0, a: 1.0 };
}
```

You can use expressions as field values, not just literals:

```mog
fn main() {
  base := 100;
  p := Point { x: base * 2, y: base + 50 };
  print(p.x);  // 200
  print(p.y);  // 150
}
```

## Field Access

Access individual fields with dot notation:

```mog
fn main() {
  p := Point { x: 10, y: 20 };
  print(p.x);  // 10
  print(p.y);  // 20

  c := Color { r: 255, g: 128, b: 0, a: 1.0 };
  print(c.r);  // 255
  print(c.a);  // 1.0
}
```

Fields work anywhere an expression of that type is expected:

```mog
fn main() {
  p := Point { x: 3, y: 4 };
  distance_squared := (p.x * p.x) + (p.y * p.y);
  print(distance_squared);  // 25
}
```

## Field Mutation

Struct fields are mutable. Assign to them with `=`:

```mog
fn main() {
  p := Point { x: 10, y: 20 };
  print(p.x);  // 10

  p.x = 30;
  print(p.x);  // 30

  p.y = p.y + 5;
  print(p.y);  // 25
}
```

You can mutate any field at any time:

```mog
fn main() {
  user := User { id: 1, username: "alice", email: "alice@example.com", active: true };
  print(user.active);  // true

  user.active = false;
  user.email = "alice@newdomain.com";
  print(user.active);  // false
  print(user.email);   // alice@newdomain.com
}
```

## Passing Structs to Functions

Structs are heap-allocated and passed by reference. When you pass a struct to a function, the function receives a pointer to the same data. Modifications inside the function affect the original:

```mog
fn move_right(p: Point, amount: int) {
  p.x = p.x + amount;
}

fn main() {
  p := Point { x: 0, y: 0 };
  move_right(p, 10);
  print(p.x);  // 10 — the original was modified
}
```

> This is different from closures, which capture variables by value (see Chapter 7). Structs are always passed by reference — there is no copy-on-pass.

Functions can read struct fields without modifying them:

```mog
struct Rect {
  width: int,
  height: int,
}

fn area(r: Rect) -> int {
  return r.width * r.height;
}

fn main() {
  r := Rect { width: 10, height: 5 };
  print(area(r));  // 50
}
```

Returning structs from functions works naturally — you return a reference to a heap-allocated struct:

```mog
fn make_point(x: int, y: int) -> Point {
  return Point { x: x, y: y };
}

fn main() {
  p := make_point(3, 7);
  print(p.x);  // 3
  print(p.y);  // 7
}
```

## No Methods — Use Standalone Functions

Mog structs have no methods. Instead, write standalone functions that take the struct as a parameter. This keeps data and behavior separate:

```mog
struct Vec2 {
  x: int,
  y: int,
}

fn vec2_add(a: Vec2, b: Vec2) -> Vec2 {
  return Vec2 { x: a.x + b.x, y: a.y + b.y };
}

fn vec2_dot(a: Vec2, b: Vec2) -> int {
  return (a.x * b.x) + (a.y * b.y);
}

fn vec2_to_string(v: Vec2) -> string {
  return "({v.x}, {v.y})";
}

fn main() {
  a := Vec2 { x: 1, y: 2 };
  b := Vec2 { x: 3, y: 4 };

  sum := vec2_add(a, b);
  print(vec2_to_string(sum));  // (4, 6)
  print(vec2_dot(a, b));       // 11
}
```

> A common convention is to prefix function names with the struct name: `point_distance`, `color_mix`, `user_validate`. This makes it clear which type the function operates on.

## Constructor Functions

Since there are no constructors or default values, a common pattern is to write factory functions that return pre-configured struct instances:

```mog
fn new_user(name: string, email: string) -> User {
  return User { id: 0, username: name, email: email, active: true };
}

fn main() {
  u := new_user("alice", "alice@example.com");
  print(u.username);  // alice
  print(u.active);    // true
}
```

```mog
struct DatabaseConfig {
  host: string,
  port: int,
  name: string,
}

fn default_db_config() -> DatabaseConfig {
  return DatabaseConfig { host: "localhost", port: 5432, name: "appdb" };
}

fn main() {
  cfg := default_db_config();
  cfg.name = "testdb";
  print(cfg.host);  // localhost
  print(cfg.name);  // testdb
}
```

This pattern gives you the flexibility of default values while keeping construction explicit. See Chapter 7 for how closures can create factory functions that return configured behavior.

## Nested Structs

Structs can contain other structs as fields:

```mog
struct Address {
  street: string,
  city: string,
  zip: string,
}

struct Person {
  name: string,
  age: int,
  address: Address,
}

fn main() {
  p := Person {
    name: "Alice",
    age: 30,
    address: Address { street: "123 Main St", city: "Portland", zip: "97201" },
  };

  print(p.name);            // Alice
  print(p.address.city);    // Portland
  print(p.address.zip);     // 97201
}
```

Mutation works through nested field access:

```mog
fn main() {
  p := Person {
    name: "Bob",
    age: 25,
    address: Address { street: "456 Oak Ave", city: "Seattle", zip: "98101" },
  };

  p.address.city = "Tacoma";
  p.age = 26;
  print(p.address.city);  // Tacoma
}
```

Since structs are passed by reference, modifying a nested struct through a function affects the original all the way up:

```mog
fn relocate(person: Person, new_city: string) {
  person.address.city = new_city;
}

fn main() {
  p := Person {
    name: "Dana",
    age: 35,
    address: Address { street: "100 Elm St", city: "Austin", zip: "73301" },
  };

  relocate(p, "Houston");
  print(p.address.city);  // Houston
}
```

## Structs with Arrays and Maps

Struct fields can hold arrays and maps, enabling rich data models:

```mog
struct StudentRecord {
  name: string,
  grades: [int],
}

fn average_grade(s: StudentRecord) -> float {
  sum := 0;
  for g in s.grades {
    sum = sum + g;
  }
  return (sum as float) / (s.grades.len as float);
}

fn main() {
  student := StudentRecord { name: "Eve", grades: [88, 92, 75, 96] };
  print(average_grade(student));  // 87.75

  student.grades.push(100);
  print(student.grades.len);     // 5
}
```

```mog
struct Inventory {
  items: map[string]int,
}

fn add_item(inv: Inventory, name: string, qty: int) {
  if inv.items.has(name) {
    inv.items[name] = inv.items[name] + qty;
  } else {
    inv.items[name] = qty;
  }
}

fn main() {
  inv := Inventory { items: {} };
  add_item(inv, "apples", 5);
  add_item(inv, "bananas", 3);
  add_item(inv, "apples", 2);
  print(inv.items["apples"]);  // 7
}
```

See Chapter 10 for the full set of array and map operations.

## Practical Examples

### RGB Color Manipulation

```mog
struct Color {
  r: int,
  g: int,
  b: int,
}

fn clamp(val: int, lo: int, hi: int) -> int {
  if val < lo { return lo; }
  if val > hi { return hi; }
  return val;
}

fn brighten(c: Color, amount: int) {
  c.r = clamp(c.r + amount, 0, 255);
  c.g = clamp(c.g + amount, 0, 255);
  c.b = clamp(c.b + amount, 0, 255);
}

fn mix(a: Color, b: Color) -> Color {
  return Color {
    r: (a.r + b.r) / 2,
    g: (a.g + b.g) / 2,
    b: (a.b + b.b) / 2,
  };
}

fn color_to_string(c: Color) -> string {
  return "rgb({c.r}, {c.g}, {c.b})";
}

fn main() {
  red := Color { r: 200, g: 50, b: 50 };
  blue := Color { r: 50, g: 50, b: 200 };

  brighten(red, 40);
  print(color_to_string(red));    // rgb(240, 90, 90)

  purple := mix(red, blue);
  print(color_to_string(purple)); // rgb(145, 70, 145)
}
```

### Tree Structure

Structs that contain arrays of the same type enable tree-like patterns:

```mog
struct TreeNode {
  value: int,
  children: [TreeNode],
}

fn sum_tree(node: TreeNode) -> int {
  total := node.value;
  for child in node.children {
    total = total + sum_tree(child);
  }
  return total;
}

fn main() {
  tree := TreeNode {
    value: 1,
    children: [
      TreeNode { value: 2, children: [] },
      TreeNode {
        value: 3,
        children: [
          TreeNode { value: 4, children: [] },
          TreeNode { value: 5, children: [] },
        ],
      },
    ],
  };

  print(sum_tree(tree));  // 15
}
```

## Summary

| Concept | Syntax |
|---|---|
| Declare a struct | `struct Name { field: type, ... }` |
| Construct an instance | `Name { field: value, ... }` |
| Read a field | `instance.field` |
| Mutate a field | `instance.field = value;` |
| Nested field access | `instance.field.subfield` |

Structs are heap-allocated and passed by reference. There are no methods — use standalone functions that take the struct as a parameter. Keep structs simple: they hold data, functions provide behavior.
# Chapter 10: Collections

Mog provides three collection types: arrays for ordered sequences, maps for key-value lookup, and SoA (Struct of Arrays) for cache-friendly columnar storage. Together they cover the vast majority of data organization needs.

## Arrays

Arrays are dynamically-sized, ordered, homogeneous sequences. They grow and shrink as needed and are the most common collection type.

### Array Literals

Create arrays with bracket syntax. The element type is inferred:

```mog
fn main() -> int {
  numbers := [1, 2, 3, 4, 5];
  names := ["Alice", "Bob", "Charlie"];
  flags := [true, false, true];

  println(numbers.len());  // 5
  println(names.len());    // 3
  return 0;
}
```

### Repeat Syntax

Create arrays filled with a repeated value using `[value; count]`:

```mog
fn main() -> int {
  zeros := [0; 100];           // 100 zeros
  blank := [""; 10];           // 10 empty strings
  grid := [false; 64];         // 64 false values

  println(zeros.len());   // 100
  println(zeros[50]);     // 0
  return 0;
}
```

Use repeat syntax to create empty arrays — `[0; 0]` gives you an empty `[int]` ready for `.push()`:

```mog
fn main() -> int {
  buffer := [0; 0];     // empty int array
  scores := [0.0; 50];  // 50 floats, all 0.0
  return 0;
}
```

### Type Annotations

Array types are written as `[ElementType]`. Function parameters and return types always require explicit types:

```mog
fn sum(numbers: [int]) -> int {
  total := 0;
  for n in numbers {
    total = total + n;
  }
  return total;
}

fn first_or_default(items: [string], fallback: string) -> string {
  if items.len() > 0 {
    return items[0];
  }
  return fallback;
}

fn main() -> int {
  vals := [10, 20, 30];
  println(sum(vals));  // 60

  empty: [string] = [];
  println(first_or_default(empty, "none"));  // none
  return 0;
}
```

### Indexing

Access elements by zero-based index with brackets. Out-of-bounds access is a runtime error:

```mog
fn main() -> int {
  arr := [10, 20, 30, 40, 50];

  println(arr[0]);   // 10
  println(arr[4]);   // 50

  // Mutation by index
  arr[2] = 99;
  println(arr[2]);   // 99

  // Index with a variable
  i := 3;
  println(arr[i]);   // 40
  return 0;
}
```

> **Warning:** Accessing an index beyond the array's length causes a runtime panic. Always check `.len()` if the index is computed dynamically.

### Iteration

Use `for` to iterate over elements. The two-variable form gives you the index (see Chapter 5):

```mog
fn main() -> int {
  colors := ["red", "green", "blue"];

  // Value only
  for color in colors {
    println(color);
  }

  // Index and value
  for i, color in colors {
    println(f"{i}: {color}");
  }
  // 0: red
  // 1: green
  // 2: blue
  return 0;
}
```

### `.push()` and `.pop()`

Append to the end with `.push()`. Remove and return the last element with `.pop()`:

```mog
fn main() -> int {
  stack := [0; 0];

  stack.push(10);
  stack.push(20);
  stack.push(30);
  println(stack.len());  // 3

  top := stack.pop();
  println(top);          // 30
  println(stack.len());  // 2
  return 0;
}
```

A stack using push/pop:

```mog
fn main() -> int {
  stack := [0; 0];
  items := [5, 3, 8, 1, 9];

  for item in items {
    stack.push(item);
  }

  for stack.len() > 0 {
    println(stack.pop());
  }
  // 9, 1, 8, 3, 5
  return 0;
}
```

### `.slice()`

Extract a sub-array with `.slice(start, end)`. The range is half-open — `start` is inclusive, `end` is exclusive:

```mog
fn main() -> int {
  arr := [10, 20, 30, 40, 50];

  first_three := arr.slice(0, 3);
  println(first_three);  // [10, 20, 30]

  middle := arr.slice(1, 4);
  println(middle);  // [20, 30, 40]

  last_two := arr.slice(3, 5);
  println(last_two);  // [40, 50]
  return 0;
}
```

### `.contains()`

Check if an element exists in the array:

```mog
fn main() -> int {
  primes := [2, 3, 5, 7, 11, 13];

  println(primes.contains(7));   // true
  println(primes.contains(4));   // false

  allowed := ["admin", "editor", "viewer"];
  role := "editor";
  if allowed.contains(role) {
    println("access granted");
  }
  return 0;
}
```

### `.reverse()`

Reverse an array in place:

```mog
fn main() -> int {
  arr := [1, 2, 3, 4, 5];
  arr.reverse();
  println(arr);  // [5, 4, 3, 2, 1]
  return 0;
}
```

### `.join()`

Combine array elements into a single string with a separator:

```mog
fn main() -> int {
  words := ["hello", "world"];
  println(words.join(" "));   // hello world
  println(words.join(", "));  // hello, world
  println(words.join(""));    // helloworld

  numbers := [1, 2, 3];
  println(numbers.join("-"));  // 1-2-3
  return 0;
}
```

### `.filter()`

Return a new array containing only elements that pass a test:

```mog
fn main() -> int {
  numbers := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

  evens := numbers.filter(fn(n: int) -> bool { (n % 2) == 0 });
  println(evens);  // [2, 4, 6, 8, 10]

  big := numbers.filter(fn(n: int) -> bool { n > 5 });
  println(big);  // [6, 7, 8, 9, 10]
  return 0;
}
```

Filter with a named function:

```mog
fn is_positive(n: int) -> bool {
  return n > 0;
}

fn main() -> int {
  values := [-3, -1, 0, 2, 5, -7, 4];
  positives := values.filter(is_positive);
  println(positives);  // [2, 5, 4]
  return 0;
}
```

### `.map()`

Return a new array with each element transformed:

```mog
fn main() -> int {
  numbers := [1, 2, 3, 4, 5];

  doubled := numbers.map(fn(n: int) -> int { n * 2 });
  println(doubled);  // [2, 4, 6, 8, 10]

  as_strings := numbers.map(fn(n: int) -> string { str(n) });
  println(as_strings.join(", "));  // 1, 2, 3, 4, 5
  return 0;
}
```

### `.sort()`

Sort an array in place using a comparator function. The comparator returns a negative integer if the first element should come before the second, positive if after, and zero if equal:

```mog
fn main() -> int {
  numbers := [5, 2, 8, 1, 9, 3];

  // Ascending
  numbers.sort(fn(a: int, b: int) -> int { a - b });
  println(numbers);  // [1, 2, 3, 5, 8, 9]

  // Descending
  numbers.sort(fn(a: int, b: int) -> int { b - a });
  println(numbers);  // [9, 8, 5, 3, 2, 1]
  return 0;
}
```

Sorting strings:

```mog
fn main() -> int {
  names := ["Charlie", "Alice", "Bob", "Dana"];

  names.sort(fn(a: string, b: string) -> int {
    if a < b { return -1; }
    if a > b { return 1; }
    return 0;
  });

  println(names.join(", "));  // Alice, Bob, Charlie, Dana
  return 0;
}
```

### Chaining Array Methods

Methods like `.filter()` and `.map()` return new arrays, so you can chain them into pipelines:

```mog
fn main() -> int {
  numbers := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

  // Get the squares of even numbers
  result := numbers
    .filter(fn(n: int) -> bool { (n % 2) == 0 })
    .map(fn(n: int) -> int { n * n });
  println(result);  // [4, 16, 36, 64, 100]
  return 0;
}
```

```mog
fn main() -> int {
  words := ["hello", "world", "", "mog", "", "lang"];

  // Remove empty strings, convert to uppercase, join
  output := words
    .filter(fn(s: string) -> bool { s.len() > 0 })
    .map(fn(s: string) -> string { s.upper() })
    .join(" ");
  println(output);  // HELLO WORLD MOG LANG
  return 0;
}
```

> **Tip:** Chaining `.filter()` then `.map()` is the most common pipeline. If you need to both filter and transform, this reads more clearly than a manual loop.

## Maps

Maps are unordered key-value collections with string keys. Use them when you need to look up values by name rather than by position.

### Creating Maps

Create maps with brace syntax:

```mog
fn main() -> int {
  ages := { "Alice": 30, "Bob": 25, "Charlie": 35 };
  config := { "host": "localhost", "port": "8080" };
  return 0;
}
```

### Access and Mutation

Read values with bracket syntax. Set values the same way — writing to a key that doesn't exist creates it:

```mog
fn main() -> int {
  scores := { "math": 95, "english": 88, "science": 92 };

  // Read
  println(scores["math"]);     // 95

  // Write — update existing key
  scores["math"] = 100;
  println(scores["math"]);     // 100

  // Write — add new key
  scores["history"] = 87;
  println(scores["history"]);  // 87
  return 0;
}
```

### `.has()` — Checking Key Existence

Check whether a key exists before accessing it:

```mog
fn main() -> int {
  m := { "name": "Alice", "city": "NYC" };

  if m.has("name") {
    println(m["name"]);  // Alice
  }

  if !m.has("age") {
    println("age not found");
  }
  return 0;
}
```

> **Note:** Accessing a key that doesn't exist is a runtime error. Always use `.has()` or iterate with `for` when keys are dynamic.

### `.len()`, `.keys()`, `.values()`

```mog
fn main() -> int {
  m := { "a": 1, "b": 2, "c": 3 };

  println(m.len());     // 3

  ks := m.keys();
  println(ks);          // ["a", "b", "c"] (order may vary)

  vs := m.values();
  println(vs);          // [1, 2, 3] (order may vary)
  return 0;
}
```

### `.delete()` — Removing Entries

```mog
fn main() -> int {
  m := { "x": 10, "y": 20, "z": 30 };

  m.delete("y");
  println(m.len());     // 2
  println(m.has("y"));  // false
  return 0;
}
```

### Iterating Maps

Use `for key, value in map` to iterate over all entries:

```mog
fn main() -> int {
  prices := { "apple": 120, "banana": 80, "cherry": 300 };

  for name, price in prices {
    println(f"{name}: {price} cents");
  }
  return 0;
}
```

Collecting keys that meet a condition:

```mog
fn main() -> int {
  grades := { "Alice": 92, "Bob": 67, "Charlie": 85, "Dana": 45, "Eve": 78 };

  passing := [0; 0];
  for name, grade in grades {
    if grade >= 70 {
      passing.push(name);
    }
  }
  println(passing.join(", "));
  return 0;
}
```

### Practical Example: Word Counting

```mog
fn word_count(text: string) -> {string: int} {
  counts := { "": 0 };
  counts.delete("");  // start with empty map

  words := text.split(" ");
  for word in words {
    if counts.has(word) {
      counts[word] = counts[word] + 1;
    } else {
      counts[word] = 1;
    }
  }
  return counts;
}

fn main() -> int {
  text := "the cat sat on the mat the cat";
  counts := word_count(text);

  for word, n in counts {
    println(f"{word}: {n}");
  }
  // the: 3
  // cat: 2
  // sat: 1
  // on: 1
  // mat: 1
  return 0;
}
```

### Practical Example: Grouping Data

```mog
struct Student {
  name: string,
  grade: string,
}

fn group_by_grade(students: [Student]) -> {string: [string]} {
  groups := { "": [""] };
  groups.delete("");

  for s in students {
    if !groups.has(s.grade) {
      groups[s.grade] = [0; 0];
    }
    groups[s.grade].push(s.name);
  }
  return groups;
}

fn main() -> int {
  students := [
    Student { name: "Alice", grade: "A" },
    Student { name: "Bob", grade: "B" },
    Student { name: "Charlie", grade: "A" },
    Student { name: "Dana", grade: "C" },
    Student { name: "Eve", grade: "B" },
  ];

  groups := group_by_grade(students);
  for grade, names in groups {
    println(f"{grade}: {names.join(", ")}");
  }
  // A: Alice, Charlie
  // B: Bob, Eve
  // C: Dana
  return 0;
}
```

## SoA (Struct of Arrays)

SoA flips the usual memory layout. Instead of storing an array of structs (each struct contiguous in memory), SoA stores one contiguous array per field. This improves cache performance when you iterate over a single field across many elements — common in simulations, game engines, and data processing.

### When to Use SoA

Use SoA when:
- You have many instances of the same struct (hundreds or thousands)
- Your hot loops touch one or two fields at a time, not all of them
- Performance matters and you want cache-friendly access patterns

Use regular arrays of structs when:
- You have few instances
- You access all fields of each element together
- Simplicity matters more than cache behavior

### Construction

Define a regular struct (see Chapter 9), then create an SoA container with `soa StructName[capacity]`:

```mog
struct Particle {
  x: float,
  y: float,
  mass: float,
}

fn main() -> int {
  particles := soa Particle[1000];
  return 0;
}
```

This allocates three separate arrays of 1000 elements — one for `x`, one for `y`, one for `mass` — instead of one array of 1000 three-field structs.

### Field Access

Read and write fields using array-index-then-dot syntax:

```mog
fn main() -> int {
  particles := soa Particle[100];

  // Set fields
  particles[0].x = 10.0;
  particles[0].y = 20.0;
  particles[0].mass = 5.0;

  // Read fields
  println(particles[0].x);     // 10.0
  println(particles[0].mass);  // 5.0

  // Initialize several elements
  for i in 0..10 {
    particles[i].x = i as float * 10.0;
    particles[i].y = i as float * 5.0;
    particles[i].mass = 1.0;
  }

  println(particles[5].x);  // 50.0
  println(particles[9].y);  // 45.0
  return 0;
}
```

> **Tip:** The syntax `particles[i].x` looks like regular struct access, but under the hood the compiler lowers it to an index into the `x` column array. You get columnar storage with familiar syntax.

### Iteration Patterns

Iterating over a single field is where SoA shines. The compiler reads from a single contiguous array, keeping the CPU cache hot:

```mog
struct Entity {
  x: int,
  y: int,
  health: int,
  speed: int,
}

fn main() -> int {
  entities := soa Entity[500];

  // Initialize
  for i in 0..500 {
    entities[i].x = i;
    entities[i].y = i * 2;
    entities[i].health = 100;
    entities[i].speed = 5;
  }

  // Update all x positions — touches only the x array
  for i in 0..500 {
    entities[i].x = entities[i].x + entities[i].speed;
  }

  // Sum all health values — touches only the health array
  total_health := 0;
  for i in 0..500 {
    total_health = total_health + entities[i].health;
  }
  println(total_health);  // 50000
  return 0;
}
```

### Practical Example: Particle Simulation

A physics step that applies gravity and updates positions. Separating the gravity pass (which only touches `vy`) from the position pass (which touches `x`, `y`, `vx`, `vy`) maximizes cache efficiency:

```mog
struct Particle {
  x: float,
  y: float,
  vx: float,
  vy: float,
  mass: float,
}

fn step(particles: soa Particle, count: int, dt: float) {
  // Apply gravity — only touches vy array
  for i in 0..count {
    particles[i].vy = particles[i].vy - (9.8 * dt);
  }

  // Update positions — touches x, y, vx, vy arrays
  for i in 0..count {
    particles[i].x = particles[i].x + (particles[i].vx * dt);
    particles[i].y = particles[i].y + (particles[i].vy * dt);
  }
}

fn main() -> int {
  particles := soa Particle[1000];

  for i in 0..1000 {
    particles[i].x = 0.0;
    particles[i].y = 100.0;
    particles[i].vx = i as float * 0.1;
    particles[i].vy = 0.0;
    particles[i].mass = 1.0;
  }

  // Run 60 simulation steps
  for frame in 0..60 {
    step(particles, 1000, 0.016);
  }

  println(particles[0].y);
  println(particles[500].x);
  return 0;
}
```

### Practical Example: Column Operations

SoA is natural for column-oriented data processing — summing a column, finding a max, or filtering by a category all read from a single contiguous array:

```mog
struct Record {
  id: int,
  value: int,
  category: int,
}

fn column_sum(records: soa Record, count: int) -> int {
  total := 0;
  for i in 0..count {
    total = total + records[i].value;
  }
  return total;
}

fn column_max(records: soa Record, count: int) -> int {
  max_val := records[0].value;
  for i in 1..count {
    if records[i].value > max_val {
      max_val = records[i].value;
    }
  }
  return max_val;
}

fn count_category(records: soa Record, count: int, cat: int) -> int {
  n := 0;
  for i in 0..count {
    if records[i].category == cat {
      n = n + 1;
    }
  }
  return n;
}

fn main() -> int {
  data := soa Record[100];

  for i in 0..100 {
    data[i].id = i;
    data[i].value = (i * 7) % 50;
    data[i].category = i % 3;
  }

  println(column_sum(data, 100));
  println(column_max(data, 100));
  println(count_category(data, 100, 0));  // count of category 0
  return 0;
}
```

## Putting It All Together

### Data Processing Pipeline

Combining arrays, maps, and structs for a complete processing workflow:

```mog
struct Sale {
  product: string,
  amount: int,
  region: string,
}

fn total_by_region(sales: [Sale]) -> {string: int} {
  totals := { "": 0 };
  totals.delete("");

  for sale in sales {
    if totals.has(sale.region) {
      totals[sale.region] = totals[sale.region] + sale.amount;
    } else {
      totals[sale.region] = sale.amount;
    }
  }
  return totals;
}

fn main() -> int {
  sales := [
    Sale { product: "widget", amount: 100, region: "east" },
    Sale { product: "gadget", amount: 250, region: "west" },
    Sale { product: "widget", amount: 150, region: "west" },
    Sale { product: "gizmo", amount: 75, region: "east" },
    Sale { product: "gadget", amount: 200, region: "east" },
  ];

  region_totals := total_by_region(sales);
  for region, total in region_totals {
    println(f"{region}: {total}");
  }
  // east: 375
  // west: 400
  return 0;
}
```

### Filter-Map-Sort Pipeline

```mog
struct Task {
  title: string,
  priority: int,
  done: bool,
}

fn main() -> int {
  tasks := [
    Task { title: "Write docs", priority: 2, done: false },
    Task { title: "Fix bug", priority: 1, done: false },
    Task { title: "Add tests", priority: 3, done: true },
    Task { title: "Review PR", priority: 1, done: false },
    Task { title: "Deploy", priority: 2, done: false },
  ];

  // Get incomplete tasks, sorted by priority
  pending := tasks.filter(fn(t: Task) -> bool { !t.done });
  pending.sort(fn(a: Task, b: Task) -> int { a.priority - b.priority });

  for t in pending {
    println(f"[P{t.priority}] {t.title}");
  }
  // [P1] Fix bug
  // [P1] Review PR
  // [P2] Write docs
  // [P2] Deploy
  return 0;
}
```

## Summary

| Collection | Create | Access | Iterate |
|---|---|---|---|
| Array | `[1, 2, 3]` or `[0; n]` | `arr[i]` | `for item in arr` |
| Map | `{ "key": value }` | `m["key"]` | `for k, v in m` |
| SoA | `soa Struct[n]` | `soa[i].field` | `for i in 0..n` |

**Array methods:** `.len()`, `.push()`, `.pop()`, `.slice()`, `.contains()`, `.reverse()`, `.filter()`, `.map()`, `.sort()`, `.join()`

**Map methods:** `.len()`, `.keys()`, `.values()`, `.has()`, `.delete()`

Arrays are the default choice. Use maps when you need key-based lookup. Use SoA when you have many elements and need to iterate over individual fields efficiently — the syntax stays familiar while the memory layout optimizes for your access pattern.
# Chapter 11: Error Handling

Mog has no exceptions. There is no `throw`, no invisible stack unwinding, no `try`/`finally` cleanup semantics. When a function can fail, it says so in its return type, and the caller decides what to do. Errors are values — you create them, return them, match on them, and propagate them with the same tools you use for everything else.

This design means you can always see where failures are handled by reading the code. Nothing fails silently, and nothing interrupts your control flow from a distance.

## Result\<T\>

`Result<T>` is a built-in type with two variants: `ok(value)` wrapping a success value of type `T`, and `err(message)` wrapping an error message of type `string`. Any function that might fail returns a `Result`:

```mog
fn divide(a: int, b: int) -> Result<int> {
  if b == 0 {
    return err("division by zero");
  }
  return ok(a / b);
}
```

You construct results directly with `ok()` and `err()` — there are no special constructors or factory functions:

```mog
fn parse_port(s: string) -> Result<int> {
  n := parse_int(s)?;
  if n < 0 {
    return err("port cannot be negative");
  }
  if n > 65535 {
    return err(f"port out of range: {n}");
  }
  return ok(n);
}
```

Results can wrap any type, including compound types:

```mog
fn load_settings(path: string) -> Result<{string: string}> {
  content := fs.read(path)?;
  lines := content.split("\n");
  settings: {string: string} = {};

  for line in lines {
    parts := line.split("=");
    if parts.len() != 2 {
      return err(f"malformed setting: {line}");
    }
    settings[parts[0].trim()] = parts[1].trim();
  }
  return ok(settings);
}
```

Use `match` to handle both cases (see Chapter 5 for match syntax):

```mog
fn main() -> int {
  match divide(100, 7) {
    ok(v) => println(f"100 / 7 = {v}"),
    err(e) => println(f"failed: {e}"),
  }
  return 0;
}
```

You can bind the result first and match later:

```mog
fn main() -> int {
  result := divide(10, 0);

  match result {
    ok(v) => {
      println(f"result: {v}");
      println(f"doubled: {v * 2}");
    },
    err(e) => {
      println(f"division failed: {e}");
    },
  }
  return 0;
}
```

Match arms can contain any expression or block. A common pattern is returning from the enclosing function inside a match arm:

```mog
fn half_or_bail(n: int) -> Result<int> {
  match divide(n, 2) {
    ok(v) => return ok(v),
    err(e) => return err(f"halving failed: {e}"),
  }
}
```

## Optional ?T

`?T` is a built-in type with two variants: `some(value)` wrapping a value of type `T`, and `none` representing absence. Use it when a value might not exist — not because something went wrong, but because there may simply be nothing there:

```mog
fn find_index(arr: [int], target: int) -> ?int {
  for i, v in arr {
    if v == target {
      return some(i);
    }
  }
  return none;
}
```

The `?` prefix goes on the type: `?int`, `?string`, `?User`. You construct values with `some()` and signal absence with `none`:

```mog
fn first_positive(numbers: [int]) -> ?int {
  for n in numbers {
    if n > 0 {
      return some(n);
    }
  }
  return none;
}
```

```mog
struct User {
  name: string,
  email: string,
}

fn lookup_user(users: [User], name: string) -> ?User {
  for u in users {
    if u.name == name {
      return some(u);
    }
  }
  return none;
}
```

Match on optionals the same way you match on results:

```mog
fn main() -> int {
  match find_index([10, 20, 30], 20) {
    some(i) => println(f"found at index {i}"),
    none => println("not found"),
  }
  return 0;
}
```

> **Tip:** Optionals carry no error message. If you need to explain *why* a value is absent, use `Result<T>` instead.

```mog
// Use ?T when absence is normal
fn get_middle_name(user: User) -> ?string {
  return user.middle_name;
}

// Use Result<T> when absence is a failure
fn get_required_field(data: {string: string}, key: string) -> Result<string> {
  if data.has(key) {
    return ok(data[key]);
  }
  return err(f"missing required field: {key}");
}
```

## The ? Propagation Operator

Appending `?` to a `Result` or `?T` expression unwraps the success case and propagates the failure case. If the value is `ok(v)` or `some(v)`, it evaluates to `v`. If the value is `err(e)` or `none`, the current function returns immediately with that error or `none`.

The simplest case — unwrap or bail:

```mog
fn process() -> Result<int> {
  val := divide(10, 2)?;  // val is 5, or function returns the err
  return ok(val * 2);
}
```

Chaining multiple fallible operations is where `?` shines. Each `?` is an early-return point:

```mog
fn load_and_parse_config(path: string) -> Result<Config> {
  content := fs.read(path)?;           // bail if read fails
  json := parse_json(content)?;        // bail if parse fails
  config := validate_config(json)?;    // bail if validation fails
  return ok(config);
}
```

Without `?`, the same function requires nested matching:

```mog
fn load_and_parse_config(path: string) -> Result<Config> {
  match fs.read(path) {
    err(e) => return err(e),
    ok(content) => {
      match parse_json(content) {
        err(e) => return err(e),
        ok(json) => {
          match validate_config(json) {
            err(e) => return err(e),
            ok(config) => return ok(config),
          }
        },
      }
    },
  }
}
```

The `?` operator works inline in larger expressions:

```mog
fn compute_ratio(a: int, b: int, c: int) -> Result<float> {
  sum := divide(a, b)? + divide(a, c)?;
  return ok(sum as float);
}
```

It also works on optionals. A function returning `?T` can propagate `none` from another optional:

```mog
fn get_user_email(users: [User], name: string) -> ?string {
  user := lookup_user(users, name)?;  // returns none if not found
  return some(user.email);
}
```

> **Note:** The return type of your function must be compatible: `?` on a `Result` requires your function to return `Result`, and `?` on an optional requires your function to return an optional.

```mog
fn first_even(arr: [int]) -> ?int {
  for n in arr {
    if (n % 2) == 0 {
      return some(n);
    }
  }
  return none;
}

fn double_first_even(arr: [int]) -> ?int {
  val := first_even(arr)?;  // propagates none
  return some(val * 2);
}
```

## try-catch Blocks

Sometimes you want to handle errors from a group of operations in one place rather than propagating them up. `try`-`catch` catches errors from `?` propagation inside the `try` block:

```mog
try {
  config := fs.read("config.json")?;
  data := fs.read("data.csv")?;
  process(config, data)?;
  println("done");
} catch(e) {
  println(f"setup failed: {e}");
}
```

The variable `e` in `catch(e)` is a `string` — the error message from whichever `?` failed. The parentheses around `e` are required.

`try`-`catch` is useful at the top level of a program or at the boundary of a subsystem, where you want to handle all errors uniformly:

```mog
fn run_pipeline() {
  try {
    input := read_input()?;
    validated := validate(input)?;
    result := transform(validated)?;
    write_output(result)?;
    println("pipeline complete");
  } catch(e) {
    println(f"pipeline failed: {e}");
  }
}
```

You can use `try`-`catch` inside loops:

```mog
fn process_files(paths: [string]) {
  for path in paths {
    try {
      content := fs.read(path)?;
      result := parse(content)?;
      println(f"{path}: {result}");
    } catch(e) {
      println(f"skipping {path}: {e}");
    }
  }
}
```

`try`-`catch` does not change your function's return type. It's a local error-handling boundary — it consumes the error instead of propagating it. If you need the function to still return `Result`, use `?` outside the `try` block or return explicitly from within `catch`:

```mog
fn load_with_fallback(primary: string, backup: string) -> Result<string> {
  try {
    content := fs.read(primary)?;
    return ok(content);
  } catch(e) {
    println(f"primary failed ({e}), trying backup");
  }

  // If we reach here, primary failed — try backup without catching
  content := fs.read(backup)?;
  return ok(content);
}
```

> **Tip:** A `try`-`catch` block lets you use `?` in functions that don't return `Result`. Since `?` would normally require a `Result` return type, `try`-`catch` gives you a way to use `?` in void functions.

```mog
fn initialize() {
  try {
    config := load_config("app.json")?;
    connect_db(config.db_url)?;
    println("initialized");
  } catch(e) {
    println(f"init failed: {e}");
    process.exit(1);
  }
}
```

## Match Patterns for Result and Optional

The `ok`, `err`, `some`, and `none` patterns work like any other match arms. You can combine them with guards and nested patterns.

Matching with variable binding:

```mog
fn describe_result(r: Result<int>) -> string {
  match r {
    ok(v) => return f"success: {v}",
    err(e) => return f"failure: {e}",
  }
}
```

Matching an optional inside a struct:

```mog
struct Response {
  status: int,
  body: ?string,
}

fn print_response(resp: Response) {
  match resp.body {
    some(text) => println(f"[{resp.status}] {text}"),
    none => println(f"[{resp.status}] (no body)"),
  }
}
```

> **Warning:** Match arms for `Result` and `?T` must be exhaustive. The compiler warns if you handle `ok` but not `err`, or `some` but not `none`.

```mog
// Compiler warning: missing err arm
match divide(10, 3) {
  ok(v) => println(v),
}

// Correct: handle both
match divide(10, 3) {
  ok(v) => println(v),
  err(e) => println(f"error: {e}"),
}
```

## Practical Patterns

### Validation Chains

Build up validation by chaining multiple checks that each return `Result`:

```mog
fn validate_username(name: string) -> Result<string> {
  if name.len() == 0 {
    return err("username cannot be empty");
  }
  if name.len() < 3 {
    return err("username must be at least 3 characters");
  }
  if name.len() > 32 {
    return err("username must be at most 32 characters");
  }
  return ok(name);
}

fn validate_age(age: int) -> Result<int> {
  if age < 0 {
    return err("age cannot be negative");
  }
  if age > 150 {
    return err(f"age seems unrealistic: {age}");
  }
  return ok(age);
}

fn create_user(name: string, age: int) -> Result<User> {
  validated_name := validate_username(name)?;
  validated_age := validate_age(age)?;
  return ok(User { name: validated_name, age: validated_age });
}
```

### Safe Parsing

Parse user input and propagate failures naturally:

```mog
struct Point {
  x: float,
  y: float,
}

fn parse_point(s: string) -> Result<Point> {
  parts := s.split(",");
  if parts.len() != 2 {
    return err(f"expected 'x,y' but got: {s}");
  }
  x := parse_float(parts[0].trim())?;
  y := parse_float(parts[1].trim())?;
  return ok(Point { x: x, y: y });
}

fn parse_polygon(lines: [string]) -> Result<[Point]> {
  points: [Point] = [];
  for i, line in lines {
    match parse_point(line) {
      ok(p) => points.push(p),
      err(e) => return err(f"line {i + 1}: {e}"),
    }
  }
  if points.len() < 3 {
    return err(f"polygon needs at least 3 points, got {points.len()}");
  }
  return ok(points);
}
```

### Converting Between Result and Optional

Sometimes you have a `Result` but only care about the success case, or you have an optional but need an error message:

```mog
// Result<T> -> ?T: discard the error message
fn result_to_optional(r: Result<int>) -> ?int {
  match r {
    ok(v) => return some(v),
    err(_) => return none,
  }
}

// ?T -> Result<T>: supply an error message
fn optional_to_result(opt: ?int, msg: string) -> Result<int> {
  match opt {
    some(v) => return ok(v),
    none => return err(msg),
  }
}

fn find_or_fail(arr: [int], target: int) -> Result<int> {
  match find_index(arr, target) {
    some(i) => return ok(i),
    none => return err(f"value {target} not found in array"),
  }
}
```

### Providing Defaults for Optionals

When you have an optional and want a fallback:

```mog
fn get_port(config: {string: string}) -> int {
  match config["port"] {
    some(p) => {
      match parse_int(p) {
        ok(n) => return n,
        err(_) => return 8080,
      }
    },
    none => return 8080,
  }
}
```

Or using `?` inside `try`-`catch` for a more compact version:

```mog
fn get_port(config: {string: string}) -> int {
  try {
    p := config["port"]?;
    n := parse_int(p)?;
    return n;
  } catch(_) {
    return 8080;
  }
}
```

### Error Message Formatting

Build descriptive error messages with string interpolation (see Chapter 4):

```mog
fn load_user_record(id: int) -> Result<User> {
  path := f"data/users/{id}.json";
  content := match fs.read(path) {
    ok(c) => c,
    err(e) => return err(f"failed to read user {id}: {e}"),
  };
  user := match parse_user(content) {
    ok(u) => u,
    err(e) => return err(f"failed to parse user {id}: {e}"),
  };
  return ok(user);
}
```

The same function using `?` is shorter but loses the contextual error messages:

```mog
fn load_user_record(id: int) -> Result<User> {
  path := f"data/users/{id}.json";
  content := fs.read(path)?;
  user := parse_user(content)?;
  return ok(user);
}
```

Choose based on whether callers need context. Deep library code often adds context; top-level code often just propagates.

### Collecting Results

Process a list and stop at the first error, or collect all successes:

```mog
// Stop at first error
fn parse_all_ints(strings: [string]) -> Result<[int]> {
  results: [int] = [];
  for s in strings {
    n := parse_int(s)?;
    results.push(n);
  }
  return ok(results);
}

// Collect errors separately
fn parse_all_ints_lenient(strings: [string]) -> [int] {
  results: [int] = [];
  for s in strings {
    match parse_int(s) {
      ok(n) => results.push(n),
      err(_) => {},  // skip bad values
    }
  }
  return results;
}
```

### Nested Results

A function can return `Result<Result<T>>` when the outer and inner operations can both independently fail, though this is rare and usually a sign you should restructure:

```mog
fn fetch_and_parse(url: string) -> Result<Result<Config>> {
  response := match http.get(url) {
    ok(r) => r,
    err(e) => return err(f"network error: {e}"),
  };
  // Return ok wrapping the parse result — caller sees network vs parse errors separately
  return ok(parse_config(response.body));
}
```

In most cases, flatten the errors into a single `Result` instead:

```mog
fn fetch_and_parse(url: string) -> Result<Config> {
  response := http.get(url)?;
  config := parse_config(response.body)?;
  return ok(config);
}
```

## Summary

| Syntax | Meaning |
|---|---|
| `Result<T>` | A value that is either `ok(T)` or `err(string)` |
| `?T` | A value that is either `some(T)` or `none` |
| `ok(value)` | Construct a success result |
| `err(message)` | Construct an error result |
| `some(value)` | Construct a present optional |
| `none` | Construct an absent optional |
| `expr?` | Unwrap success or propagate failure |
| `try { ... } catch(e) { ... }` | Handle propagated errors locally |

Mog's error handling is explicit and local. You always know which functions can fail by looking at their return type, and you always know where errors are handled by following the `?` operators and `match` arms. There are no hidden control flow paths — what you read is what runs.
# Chapter 12: Async Programming

Mog uses `async`/`await` for asynchronous operations. Agent scripts need to wait on external operations — API calls, model inference, file I/O — and async functions let you express that waiting without blocking the entire program. The host runtime manages the event loop; Mog code never creates threads or manages concurrency primitives directly.

## Async Functions

Mark a function as `async` to indicate it returns a future. Use `await` inside to wait for other async operations:

```mog
async fn fetch(url: string) -> Result<string> {
  response := await http.get(url)?;
  return ok(response.body);
}
```

An `async fn` can be called like any other function, but its return value is a future that must be `await`ed to get the actual result:

```mog
async fn greet(name: string) -> string {
  return f"hello, {name}";
}

async fn main() -> int {
  msg := await greet("world");
  println(msg);  // hello, world
  return 0;
}
```

> **Note:** When `main` is declared `async`, the runtime creates the event loop automatically. You don't need to set up or start the loop yourself.

Async functions can call other async functions. Each `await` suspends the current function until the awaited future completes:

```mog
async fn fetch_json(url: string) -> Result<string> {
  raw := await http.get(url)?;
  parsed := parse_json(raw.body)?;
  return ok(parsed);
}

async fn get_user_name(id: int) -> Result<string> {
  data := await fetch_json(f"https://api.example.com/users/{id}")?;
  return ok(data["name"]);
}
```

## Await

The `await` keyword suspends execution until a future resolves. It works on any expression that produces a future:

```mog
async fn pipeline() -> Result<string> {
  raw := await fetch_data("https://api.example.com/data")?;
  processed := await transform(raw)?;
  return ok(processed);
}
```

Each `await` is a suspension point. The runtime can run other tasks while this function waits. Without `await`, the future is created but never resolved:

```mog
async fn example() -> Result<string> {
  // This creates a future but doesn't wait for it — probably a bug
  // fetch_data("https://api.example.com");

  // This creates the future AND waits for the result
  result := await fetch_data("https://api.example.com")?;
  return ok(result);
}
```

> **Warning:** Forgetting `await` is a common mistake. If you call an async function without `await`, you get a future object, not the actual result.

You can combine `await` with the `?` operator from Chapter 11. The `await` resolves the future, then `?` unwraps the `Result`:

```mog
async fn load_config(path: string) -> Result<Config> {
  content := await fs.read_async(path)?;    // await the I/O, then ? the Result
  config := parse_config(content)?;          // synchronous parse, just ? the Result
  return ok(config);
}
```

## Spawn

Use `spawn` to launch a task that runs in the background. The spawned task executes concurrently with the rest of your code — you don't wait for it to finish:

```mog
async fn log_event(event: string) -> Result<int> {
  await http.post("https://logs.example.com", event)?;
  return ok(0);
}

async fn handle_request(req: Request) -> Result<Response> {
  // Fire and forget — don't wait for logging to complete
  spawn log_event(f"received request: {req.path}");

  result := await process(req)?;
  return ok(result);
}
```

Spawn is useful for side effects you don't need to wait on — logging, analytics, cache warming:

```mog
async fn warm_cache(keys: [string]) {
  for key in keys {
    data := await fetch_data(key)?;
    cache.set(key, data);
  }
}

async fn main() -> int {
  // Start cache warming in the background
  spawn warm_cache(["users", "products", "settings"]);

  // Continue immediately without waiting
  println("server starting...");
  await start_server(8080);
  return 0;
}
```

> **Tip:** Spawned tasks that fail do so silently — their errors aren't propagated to the caller. If you need to know whether a background task succeeded, use `await` instead of `spawn`, or use `all()` to collect results.

```mog
async fn main() -> int {
  // Bad: error is silently lost
  spawn might_fail();

  // Good: error is handled
  try {
    await might_fail();
  } catch(e) {
    println(f"task failed: {e}");
  }
  return 0;
}
```

## all() — Wait for All

`all()` takes a list of futures and waits for all of them to complete. It returns when every future has resolved, giving you all the results:

```mog
async fn parallel_fetch() -> Result<[string]> {
  results := await all([
    fetch_data("https://api.example.com/a"),
    fetch_data("https://api.example.com/b"),
    fetch_data("https://api.example.com/c"),
  ])?;
  return ok(results);
}
```

This is significantly faster than sequential awaits when the operations are independent:

```mog
// Sequential — each waits for the previous to finish
async fn fetch_sequential() -> Result<[string]> {
  a := await fetch_data("https://api.example.com/a")?;
  b := await fetch_data("https://api.example.com/b")?;
  c := await fetch_data("https://api.example.com/c")?;
  return ok([a, b, c]);
}

// Parallel — all three run at the same time
async fn fetch_parallel() -> Result<[string]> {
  results := await all([
    fetch_data("https://api.example.com/a"),
    fetch_data("https://api.example.com/b"),
    fetch_data("https://api.example.com/c"),
  ])?;
  return ok(results);
}
```

> **Note:** If any future in `all()` fails, the entire `all()` returns that error. All futures are started concurrently, but a single failure short-circuits the result.

A practical example — loading multiple resources in parallel to build a page:

```mog
struct Page {
  user: string,
  posts: string,
  notifications: string,
}

async fn load_page(user_id: int) -> Result<Page> {
  results := await all([
    fetch_data(f"https://api.example.com/users/{user_id}"),
    fetch_data(f"https://api.example.com/users/{user_id}/posts"),
    fetch_data(f"https://api.example.com/users/{user_id}/notifications"),
  ])?;

  return ok(Page {
    user: results[0],
    posts: results[1],
    notifications: results[2],
  });
}
```

## race() — Wait for First

`race()` takes a list of futures and returns the result of whichever finishes first. The remaining futures are cancelled:

```mog
async fn fastest() -> Result<string> {
  result := await race([
    fetch_from_primary(),
    fetch_from_backup(),
  ])?;
  return ok(result);
}
```

The most common use for `race()` is implementing timeouts:

```mog
async fn timeout(ms: int) -> Result<string> {
  await sleep(ms);
  return err("operation timed out");
}

async fn fetch_with_timeout(url: string) -> Result<string> {
  result := await race([
    fetch_data(url),
    timeout(5000),
  ])?;
  return ok(result);
}
```

Another use — trying multiple strategies and going with the first to succeed:

```mog
async fn resolve_address(host: string) -> Result<string> {
  result := await race([
    dns_lookup_v4(host),
    dns_lookup_v6(host),
  ])?;
  return ok(result);
}
```

> **Warning:** `race()` returns the first future to complete, whether it succeeds or fails. If the fastest future returns an error, that error propagates — even if a slower future would have succeeded. Design your futures accordingly.

## Error Handling with Async

Async functions combine naturally with `Result<T>` and the `?` operator from Chapter 11. The `await` resolves the future, and `?` unwraps the result:

```mog
async fn create_user(name: string, email: string) -> Result<User> {
  // Validate inputs (synchronous)
  validated_name := validate_name(name)?;
  validated_email := validate_email(email)?;

  // Check for duplicates (async)
  existing := await db.find_user_by_email(validated_email)?;
  match existing {
    some(_) => return err("email already registered"),
    none => {},
  }

  // Create the user (async)
  user := await db.insert_user(validated_name, validated_email)?;
  return ok(user);
}
```

Use `try`-`catch` inside async functions the same way you would in synchronous code:

```mog
async fn sync_all_data() {
  sources := ["users", "products", "orders"];

  for source in sources {
    try {
      data := await fetch_data(f"https://api.example.com/{source}")?;
      await db.upsert(source, data)?;
      println(f"synced {source}");
    } catch(e) {
      println(f"failed to sync {source}: {e}");
    }
  }
}
```

A complete async program with error handling:

```mog
async fn fetch_weather(city: string) -> Result<string> {
  url := f"https://weather.example.com/api?city={city}";
  response := await http.get(url)?;
  data := parse_json(response.body)?;
  return ok(data["temperature"]);
}

async fn main() -> int {
  cities := ["London", "Tokyo", "New York"];

  results := await all([
    fetch_weather("London"),
    fetch_weather("Tokyo"),
    fetch_weather("New York"),
  ]);

  match results {
    ok(temps) => {
      for i, city in cities {
        println(f"{city}: {temps[i]}");
      }
    },
    err(e) => {
      println(f"weather fetch failed: {e}");
    },
  }
  return 0;
}
```

### Retry Pattern

Combine async with a loop to retry failed operations:

```mog
async fn fetch_with_retry(url: string, max_retries: int) -> Result<string> {
  attempts := 0;
  for attempts < max_retries {
    match await http.get(url) {
      ok(response) => return ok(response.body),
      err(e) => {
        attempts = attempts + 1;
        if attempts >= max_retries {
          return err(f"failed after {max_retries} attempts: {e}");
        }
        println(f"attempt {attempts} failed, retrying...");
        await sleep(1000 * attempts);  // exponential-ish backoff
      },
    }
  }
  return err("unreachable");
}
```

### Fan-out / Fan-in

Process multiple items concurrently, then aggregate the results:

```mog
async fn process_batch(urls: [string]) -> Result<[string]> {
  // Build a list of futures
  futures := urls.map(fn(url: string) -> Future<Result<string>> {
    return fetch_data(url);
  });

  // Wait for all concurrently
  results := await all(futures)?;
  return ok(results);
}

async fn main() -> int {
  urls := [
    "https://api.example.com/1",
    "https://api.example.com/2",
    "https://api.example.com/3",
    "https://api.example.com/4",
    "https://api.example.com/5",
  ];

  match await process_batch(urls) {
    ok(data) => {
      for i, d in data {
        println(f"result {i}: {d}");
      }
    },
    err(e) => println(f"batch failed: {e}"),
  }
  return 0;
}
```

## Summary

| Syntax | Meaning |
|---|---|
| `async fn f() -> T` | Declare an async function returning a future |
| `await expr` | Suspend until a future resolves |
| `spawn task()` | Launch a fire-and-forget background task |
| `all([f1, f2, f3])` | Wait for all futures to complete |
| `race([f1, f2])` | Wait for the first future to complete |

Async functions compose with `Result<T>` and `?` from Chapter 11 — `await` resolves the future, then `?` unwraps the result. Use `all()` when you have independent operations that can run in parallel. Use `race()` for timeouts and fallback strategies. Use `spawn` only for side effects you don't need to track. The runtime manages the event loop; your job is to describe what depends on what.
# Chapter 13: Modules and Packages

As Mog programs grow beyond a single file, you need a way to split code into logical units, control what's visible to the outside, and compose libraries. Mog uses a Go-style module system: packages group related code, `pub` controls visibility, and `import` brings packages into scope.

There are no header files, no include guards, no complex build configurations. A package is a directory. A module is a project. The compiler resolves everything from the file system.

## Package Declaration

Every Mog file begins with a `package` declaration. It names the package the file belongs to:

```mog
package math;
```

All `.mog` files in the same directory must declare the same package name. The package name becomes the namespace used by importers:

```mog
// geometry/shapes.mog
package geometry;

pub fn circle_area(radius: float) -> float {
  return PI * (radius ** 2.0);
}
```

```mog
// geometry/volumes.mog
package geometry;

pub fn sphere_volume(radius: float) -> float {
  return (4.0 / 3.0) * PI * (radius ** 3.0);
}
```

Both files belong to `package geometry`. They share the same namespace — functions in one file can call private functions in the other, because they're in the same package.

The `package main` package is special. It must contain a `fn main()` entry point. Every executable Mog program has exactly one `package main`:

```mog
package main;

fn main() -> int {
  print("hello");
  return 0;
}
```

Files without a `package` declaration are implicitly `package main`. This is single-file mode — convenient for scripts and quick experiments.

## Public vs Private — The `pub` Keyword

Symbols are package-private by default. Only symbols marked with `pub` are visible to importers:

```mog
package mathutil;

// Public — importers can call this
pub fn gcd(a: int, b: int) -> int {
  while b != 0 {
    temp := b;
    b = a % b;
    a = temp;
  }
  return a;
}

// Public — importers can call this
pub fn lcm(a: int, b: int) -> int {
  return (a * b) / gcd(a, b);
}

// Private — only visible within package mathutil
fn validate_positive(n: int) -> bool {
  return n > 0;
}
```

`pub` works on functions, async functions, structs, and type aliases:

```mog
package models;

pub struct User {
  name: string,
  email: string,
  age: int,
}

pub type UserList = [User];

pub fn create_user(name: string, email: string, age: int) -> User {
  return User { name: name, email: email, age: age };
}

pub async fn fetch_user(id: int) -> Result<User> {
  data := await http.get("http://api.example.com/users/{id}")?;
  return ok(parse_user(data));
}

// Private helper — not exported
fn parse_user(data: string) -> User {
  return User { name: "parsed", email: "parsed", age: 0 };
}
```

Struct fields are always accessible when the struct itself is `pub`. There is no field-level visibility — if you export the struct, you export all its fields:

```mog
package config;

// Exported struct — all fields accessible to importers
pub struct Settings {
  host: string,
  port: int,
  debug: bool,
}

// Private struct — importers cannot see this at all
struct InternalState {
  cache: [string],
  dirty: bool,
}
```

> **Tip:** If you need to hide a struct's internals, keep the struct private and export functions that construct and inspect it. This is the idiomatic way to build opaque types in Mog.

## Importing Packages

Use `import` to bring a package into scope. Access its public symbols with qualified names — `package.symbol`:

```mog
package main;

import mathutil;

fn main() -> int {
  result := mathutil.gcd(48, 18);
  print("GCD: {result}");  // GCD: 6
  return 0;
}
```

The import name matches the package name. After importing, every public function, struct, and type from that package is available through the qualified prefix:

```mog
package main;

import models;

fn main() -> int {
  user := models.create_user("alice", "alice@example.com", 30);
  print(user.name);   // alice
  print(user.email);  // alice@example.com
  return 0;
}
```

Importing a package does not import its symbols into the local namespace. You always use the qualified form. This keeps names unambiguous when multiple packages are in play:

```mog
package main;

import math;
import physics;

fn main() -> int {
  // Clear which 'distance' function you mean
  d1 := math.distance(0.0, 0.0, 3.0, 4.0);
  d2 := physics.distance(10.0, 9.8, 2.0);
  print("math: {d1}, physics: {d2}");
  return 0;
}
```

## Multi-Import Syntax

When importing several packages, use the grouped form:

```mog
package main;

import (math, utils, models, config);

fn main() -> int {
  settings := config.load_defaults();
  data := utils.read_input("data.txt");
  result := math.dot_product(data, data);
  print(result);
  return 0;
}
```

This is equivalent to writing four separate `import` statements. The grouped form is preferred when a file has three or more imports:

```mog
package main;

// Equivalent — but the grouped form is cleaner
import math;
import utils;
import models;
import config;
```

## Qualified Access

All access to imported symbols uses the `package.name` form. This applies to functions, structs, and type aliases:

```mog
package main;

import geometry;

fn main() -> int {
  // Function call
  area := geometry.circle_area(5.0);

  // Struct construction
  p := geometry.Point { x: 10.0, y: 20.0 };

  // Struct field access
  print("area: {area}, x: {p.x}");
  return 0;
}
```

Qualified access also works in type annotations:

```mog
package main;

import models;

fn process_users(users: [models.User]) -> [string] {
  names: [string] = [];
  for user in users {
    names.push(user.name);
  }
  return names;
}

fn main() -> int {
  users := [
    models.create_user("alice", "a@x.com", 30),
    models.create_user("bob", "b@x.com", 25),
  ];
  names := process_users(users);
  for name in names {
    print(name);
  }
  return 0;
}
```

## The Module File

Every Mog project has a `mog.mod` file at its root. It declares the module path — the name that identifies the entire project:

```
module myapp
```

That's it. No version numbers, no dependency lists — just the module name. The module file tells the compiler where the project root is, and the directory structure below it defines the packages.

A typical project layout:

```
myapp/
  mog.mod            // module myapp
  main.mog           // package main — entry point
  math/
    math.mog         // package math
    trig.mog         // package math (same package, same dir)
  models/
    user.mog         // package models
    post.mog         // package models
  utils/
    strings.mog      // package utils
```

The compiler resolves `import math` by looking for a `math/` directory relative to the module root. Every `.mog` file in that directory is part of the `math` package.

> **Note:** The directory name and package name must match. A file in `math/` that declares `package geometry` is a compile error.

## Circular Import Detection

Mog does not allow circular imports. If package A imports package B and package B imports package A, the compiler rejects the program with an error:

```mog
// a/a.mog
package a;
import b;           // a depends on b

pub fn from_a() -> int {
  return b.from_b() + 1;
}
```

```mog
// b/b.mog
package b;
import a;           // b depends on a — COMPILE ERROR: circular import

pub fn from_b() -> int {
  return a.from_a() + 1;
}
```

The fix is to extract shared code into a third package that both A and B can import:

```mog
// shared/shared.mog
package shared;

pub fn base_value() -> int {
  return 42;
}
```

```mog
// a/a.mog
package a;
import shared;

pub fn from_a() -> int {
  return shared.base_value() + 1;
}
```

```mog
// b/b.mog
package b;
import shared;

pub fn from_b() -> int {
  return shared.base_value() + 2;
}
```

> **Warning:** Circular dependencies are always a compile error — there is no way to forward-declare or defer imports. If you hit this, it usually means two packages are too tightly coupled and should share a common dependency.

## Practical Example: Splitting a Program into Packages

Here is a small project that fetches user data, processes it, and writes output — split across three packages.

**Project structure:**

```
userapp/
  mog.mod
  main.mog
  api/
    api.mog
  transform/
    transform.mog
```

**mog.mod:**

```
module userapp
```

**api/api.mog** — handles network communication:

```mog
package api;

requires http;

pub struct UserData {
  name: string,
  score: int,
}

pub async fn fetch_user(id: int) -> Result<UserData> {
  body := await http.get("http://api.example.com/users/{id}")?;
  return ok(parse(body));
}

pub async fn fetch_users(ids: [int]) -> Result<[UserData]> {
  futures := ids.map(fn(id) { fetch_user(id) });
  results := await all(futures)?;
  return ok(results);
}

fn parse(body: string) -> UserData {
  return UserData { name: body, score: 0 };
}
```

**transform/transform.mog** — pure data transformations:

```mog
package transform;

import api;

pub fn top_scorers(users: [api.UserData], threshold: int) -> [api.UserData] {
  result: [api.UserData] = [];
  for user in users {
    if user.score >= threshold {
      result.push(user);
    }
  }
  return result;
}

pub fn format_report(users: [api.UserData]) -> string {
  report := "User Report\n";
  report = report + "===========\n";
  for i, user in users {
    report = report + "{i + 1}. {user.name} (score: {user.score})\n";
  }
  return report;
}
```

**main.mog** — entry point that wires everything together:

```mog
package main;

import (api, transform);

requires fs;

async fn main() -> int {
  ids := [1, 2, 3, 4, 5];

  match await api.fetch_users(ids) {
    ok(users) => {
      top := transform.top_scorers(users, 80);
      report := transform.format_report(top);
      print(report);
      await fs.write_file("report.txt", report)?;
    },
    err(msg) => print("failed to fetch users: {msg}"),
  }

  return 0;
}
```

Each package has a single responsibility. The `api` package handles network calls and data parsing. The `transform` package contains pure functions that process data. The `main` package wires them together. Private functions like `parse` in the `api` package stay hidden — importers see only what's marked `pub`.

## Summary

Mog's module system keeps things flat and explicit. There are no nested modules, no re-exports, no renaming on import. A package is a directory, `pub` controls visibility, and qualified access eliminates naming conflicts.

Key points:
- **Declare** packages with `package name;` at the top of every file
- **Export** symbols with `pub` — everything else is package-private
- **Import** with `import name;` or `import (a, b, c);` for groups
- **Access** imported symbols with `package.name` — always qualified, never bare
- **Structure** projects with one directory per package and `mog.mod` at the root
- **Avoid** circular imports — extract shared code into a common package

Capabilities (Chapter 14) use the same dot-syntax as package access — `fs.read_file()` looks like a module call but is backed by host-provided functions rather than Mog source code. The next chapter explains how that works.
# Chapter 14: Capabilities — Safe I/O

Mog has no built-in I/O. No file reads, no network calls, no environment variables — nothing that touches the outside world lives in the language itself. All side effects flow through **capabilities**: named interfaces that the host application provides to the script at runtime.

This is the foundation of Mog's security model. If the host doesn't grant a capability, the script can't use it. There's no escape hatch, no FFI backdoor, no unsafe block. A Mog script can only do what the host explicitly allows.

## The Capability Model

A capability is a named collection of functions provided by the host. From the script's perspective, it looks like a module with dot-syntax method calls:

```mog
requires fs;

async fn main() -> int {
  content := await fs.read_file("config.json")?;
  print(content);
  return 0;
}
```

The key difference from a regular module: there is no Mog source code behind `fs`. The host application implements those functions in C (or whatever language hosts the VM) and registers them before the script runs.

This design has three consequences:

1. **Sandboxing is the default.** A script with no `requires` declaration has zero access to the outside world. It can compute, but it can't affect anything.
2. **The compiler enforces declarations.** If you call `fs.read_file` without declaring `requires fs`, the compiler rejects your program.
3. **The host enforces availability.** If a script declares `requires http` but the host doesn't provide `http`, the host rejects the script before it runs.

## Declaring Capabilities: `requires` and `optional`

Capability declarations go at the top of the file, before any function definitions:

```mog
requires fs, process;
optional log;
```

`requires` means the program cannot run without these capabilities. If any are missing, the host refuses to execute the script:

```mog
requires fs, process;

async fn main() -> int {
  content := await fs.read_file("data.txt")?;
  dir := process.cwd();
  print("read from {dir}");
  return 0;
}
```

`optional` means the program can function without these capabilities but will use them if available. You check at runtime whether an optional capability is present:

```mog
requires fs;
optional log;

async fn main() -> int {
  data := await fs.read_file("input.txt")?;
  // log may or may not be available
  log.info("loaded input file");
  return 0;
}
```

A program with no declarations at all is a pure computation — it takes no input and produces no output beyond its return value:

```mog
fn fibonacci(n: int) -> int {
  if n <= 1 { return n; }
  a := 0;
  b := 1;
  for i in 2..(n + 1) {
    temp := a + b;
    a = b;
    b = temp;
  }
  return b;
}

fn main() -> int {
  return fibonacci(10);  // 55
}
```

> **Note:** Capabilities use the same dot-syntax as package imports (Chapter 13), but they are backed by host-provided C functions, not Mog source code. The compiler knows the difference because capabilities are declared with `requires`/`optional`, not `import`.

## Built-in Capabilities

The Mog runtime ships with a POSIX host that provides two standard capabilities: `fs` and `process`. These are conventional names — the host registers them, not the language.

### `fs` — File System

The `fs` capability provides file operations. Some may be async under the hood depending on the host, but the Mog interface uses `await`:

```mog
requires fs;

async fn main() -> int {
  // Read an entire file as a string
  content := await fs.read_file("data.txt")?;
  print(content);

  // Write a string to a file (creates or overwrites)
  await fs.write_file("output.txt", "hello, world")?;

  // Append to a file
  await fs.append_file("log.txt", "new entry\n")?;

  // Check if a file exists
  if await fs.exists("config.json")? {
    print("config found");
  }

  // Get file size in bytes
  size := await fs.file_size("data.txt")?;
  print("file is {size} bytes");

  // Remove a file
  await fs.remove("temp.txt")?;

  return 0;
}
```

The full `fs` interface:

| Function | Signature | Description |
|---|---|---|
| `read_file` | `(path: string) -> string` | Read entire file contents |
| `write_file` | `(path: string, contents: string) -> int` | Write string to file |
| `append_file` | `(path: string, contents: string) -> int` | Append string to file |
| `exists` | `(path: string) -> bool` | Check if path exists |
| `remove` | `(path: string) -> int` | Delete a file |
| `file_size` | `(path: string) -> int` | Get file size in bytes |

### `process` — Process and Environment

The `process` capability provides access to the runtime environment:

```mog
requires process;

async fn main() -> int {
  // Sleep for 500 milliseconds
  await process.sleep(500);

  // Get current timestamp (milliseconds since Unix epoch)
  now := process.timestamp();
  print("current time: {now}");

  // Get the current working directory
  dir := process.cwd();
  print("working in: {dir}");

  // Read an environment variable
  home := process.getenv("HOME");
  print("home directory: {home}");

  // Exit with a specific code
  process.exit(0);

  return 0;
}
```

The full `process` interface:

| Function | Signature | Description |
|---|---|---|
| `sleep` | `async (ms: int) -> int` | Pause execution for `ms` milliseconds |
| `timestamp` | `() -> int` | Milliseconds since Unix epoch |
| `cwd` | `() -> string` | Current working directory |
| `getenv` | `(name: string) -> string` | Read environment variable |
| `exit` | `(code: int) -> int` | Terminate the program |

## Practical Examples

### Copying a File

```mog
requires fs;

async fn copy_file(src: string, dst: string) -> int {
  content := await fs.read_file(src)?;
  await fs.write_file(dst, content)?;
  return 0;
}

async fn main() -> int {
  await copy_file("original.txt", "backup.txt")?;
  print("file copied");
  return 0;
}
```

### Reading Configuration

```mog
requires fs, process;

async fn main() -> int {
  // Try multiple config locations
  home := process.getenv("HOME");
  paths := [
    ".config.json",
    "{home}/.mogrc",
    "/etc/mog/config.json",
  ];

  for path in paths {
    if await fs.exists(path)? {
      config := await fs.read_file(path)?;
      print("loaded config from {path}");
      return 0;
    }
  }

  print("no config file found");
  return 1;
}
```

### Timed Operations

```mog
requires process;

async fn main() -> int {
  start := process.timestamp();

  // Do some work
  sum := 0;
  for i in 0..1000000 {
    sum = sum + i;
  }

  elapsed := process.timestamp() - start;
  print("computed sum={sum} in {elapsed}ms");
  return 0;
}
```

### Simple Logger Using `fs`

```mog
requires fs, process;

async fn log(message: string) -> int {
  ts := process.timestamp();
  line := "[{ts}] {message}\n";
  await fs.append_file("app.log", line)?;
  return 0;
}

async fn main() -> int {
  await log("application started")?;
  await log("processing data")?;

  result := do_work();
  await log("finished with result: {result}")?;

  return 0;
}

fn do_work() -> int {
  // pure computation — no capabilities needed
  total := 0;
  for i in 1..101 {
    total = total + (i * i);
  }
  return total;
}
```

> **Tip:** Keep capability-using functions separate from pure computation. This makes your code easier to test — pure functions need no host at all. Notice how `do_work` above has no `requires` and no `await`.

## Custom Host Capabilities

The built-in `fs` and `process` capabilities are just the standard ones. Any host application can define its own capabilities with whatever functions make sense for the domain.

### `.mogdecl` Files

Custom capabilities are declared in `.mogdecl` files. These files tell the compiler what functions a capability provides, so it can type-check calls at compile time:

```
capability env {
  fn get_name() -> string
  fn get_version() -> int
  fn timestamp() -> int
  fn random(min: int, max: int) -> int
  fn log(message: string)
  async fn delay_square(value: int, delay_ms: int) -> int
}
```

A `.mogdecl` file contains no implementation — it's a type declaration. The host provides the actual function bodies in C (or whatever the host language is).

Here's the declaration for the built-in `fs` capability:

```
capability fs {
  fn read_file(path: string) -> string
  fn write_file(path: string, contents: string) -> int
  fn append_file(path: string, contents: string) -> int
  fn exists(path: string) -> bool
  fn remove(path: string) -> int
  fn file_size(path: string) -> int
}
```

And `process`:

```
capability process {
  async fn sleep(ms: int) -> int
  fn getenv(name: string) -> string
  fn cwd() -> string
  fn exit(code: int) -> int
  fn timestamp() -> int
}
```

### Using a Custom Capability

Once the host registers a capability and provides a matching `.mogdecl` file, a Mog script uses it exactly like a built-in:

```mog
requires env;

async fn main() -> int {
  name := env.get_name();
  version := env.get_version();
  print("running {name} v{version}");

  // Call an async host function
  result := await env.delay_square(7, 100)?;
  print("7 squared (after 100ms delay): {result}");

  // Get a random number from the host
  roll := env.random(1, 6);
  print("dice roll: {roll}");

  return 0;
}
```

### How the Pieces Fit Together

The flow from script to host and back:

1. The host application starts and creates a `MogVM`.
2. The host registers capability implementations — C functions grouped by capability name.
3. The compiler reads `.mogdecl` files and validates that every capability call in the script matches a declared function signature.
4. At runtime, when the script calls `env.random(1, 6)`, the VM routes the call through `mog_cap_call_out()` to the host's C function.
5. The host function receives the arguments as `MogValue`s, does its work, and returns a `MogValue` result.

The script never knows or cares how the host implements the functions. The `.mogdecl` file is the contract between the two sides.

> **Note:** For details on implementing host functions in C and registering capabilities with the VM, see Chapter 15: Embedding Mog.

### Capability Validation Example

```mog
requires fs, process, http;

async fn main() -> int {
  data := await http.get("https://api.example.com/data")?;
  await fs.write_file("cached.json", data)?;
  return 0;
}
```

If the host only provides `fs` and `process` but not `http`, the script is rejected before execution. The host calls `mog_validate_capabilities()` and gets back an error indicating that `http` is missing. This is a deliberate, early failure — no partial execution, no runtime surprise.

## Summary

| Concept | Meaning |
|---|---|
| `requires fs, process;` | Script needs these capabilities to run |
| `optional log;` | Script can use these if available |
| `.mogdecl` file | Type declaration for a custom capability |
| `fs.read_file(path)` | Call a capability function |
| No declaration | Pure computation, no I/O |

Capabilities are the only way for Mog code to interact with the outside world. This constraint is what makes Mog safe to embed — the host is always in control. The next chapter shows how to set up that host: creating a VM, registering capabilities from C, and enforcing resource limits.
# Chapter 15: Embedding Mog in a Host Application

Mog is designed to be embedded. It's a scripting language that runs inside your application, not a standalone runtime. The host creates a VM, decides what the script can do, enforces resource limits, and tears everything down when it's finished.

This chapter covers the C API that makes embedding work.

## The Embedding Lifecycle

Every embedded Mog program follows the same five-step lifecycle:

```
[Host Application]
  1. Create a MogVM
  2. Register capabilities (what the script can do)
  3. Set resource limits (how long, how much memory)
  4. Compile and run the Mog script
  5. Free the VM
```

Here's the minimal version in C:

```c
#include "mog.h"

int main(void) {
  // 1. Create the VM
  MogVM *vm = mog_vm_new();
  mog_vm_set_global(vm);

  // 2. Register built-in capabilities (fs + process)
  mog_register_posix_host(vm);

  // 3. Set resource limits
  MogLimits limits = { .max_cpu_ms = 5000 };
  mog_vm_set_limits(vm, &limits);

  // 4. Run the compiled program
  //    (program_user is generated by the Mog compiler)
  int result = program_user();

  // 5. Cleanup
  mog_vm_free(vm);
  return result;
}
```

`mog_vm_set_global()` stores the VM pointer in a global so that generated code can find it. `program_user()` is the entry point the Mog compiler produces from the script's `main` function.

## The C API

### VM Lifecycle

```c
// Create a new VM instance
MogVM *vm = mog_vm_new();

// Store as the global VM (required for generated code)
mog_vm_set_global(vm);

// Retrieve the global VM from anywhere
MogVM *vm = mog_vm_get_global();

// Destroy the VM and free all resources
mog_vm_free(vm);
```

A program typically creates one VM and sets it as the global. The global pointer is what the compiler-generated code uses to route capability calls back to the host.

### Registering Capabilities

Capabilities are registered as arrays of name-function pairs, terminated by a `{NULL, NULL}` sentinel:

```c
// Define host functions
static MogValue my_get_name(MogVM *vm, MogArgs *args) {
  (void)vm; (void)args;
  return mog_string("MyApp");
}

static MogValue my_get_version(MogVM *vm, MogArgs *args) {
  (void)vm; (void)args;
  return mog_int(42);
}

// Build the registration table
static const MogCapEntry app_functions[] = {
  { "get_name",    my_get_name    },
  { "get_version", my_get_version },
  { NULL, NULL }  // sentinel
};

// Register under the capability name "app"
mog_register_capability(vm, "app", app_functions);
```

After this, any Mog script that declares `requires app;` can call `app.get_name()` and `app.get_version()`.

For the standard POSIX capabilities (`fs` and `process`), there's a convenience function:

```c
// Registers both "fs" and "process" capabilities
mog_register_posix_host(vm);
```

### Validating Script Requirements

Before running a script, you can check that the VM provides everything the script needs:

```c
const char *required[] = { "fs", "process", "http", NULL };
int result = mog_validate_capabilities(vm, required);
if (result != 0) {
  // Some required capability is missing — refuse to run
  fprintf(stderr, "script requires capabilities this host doesn't provide\n");
  return 1;
}
```

You can also check individual capabilities:

```c
if (mog_has_capability(vm, "http")) {
  // http is available
}
```

> **Note:** Capability validation is covered from the Mog side in Chapter 14. The `requires` declaration is the script's half of the contract; `mog_validate_capabilities()` is the host's half.

## Implementing a Custom Capability

Here's a complete example: implementing an `env` capability that provides application metadata, random numbers, logging, and an async delay function.

### Step 1: Write the `.mogdecl` File

```
capability env {
  fn get_name() -> string
  fn get_version() -> int
  fn timestamp() -> int
  fn random(min: int, max: int) -> int
  fn log(message: string)
  async fn delay_square(value: int, delay_ms: int) -> int
}
```

This tells the compiler what functions exist and what their types are. See Chapter 14 for more on `.mogdecl` files.

### Step 2: Implement the Host Functions in C

Every host function has the same signature: it takes a `MogVM*` and `MogArgs*` and returns a `MogValue`. Extract arguments with `mog_arg_int`, `mog_arg_string`, etc. Return values with `mog_int`, `mog_string`, etc.

```c
#include "mog.h"
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

static MogValue host_get_name(MogVM *vm, MogArgs *args) {
  (void)vm; (void)args;
  return mog_string("MogShowcase");
}

static MogValue host_get_version(MogVM *vm, MogArgs *args) {
  (void)vm; (void)args;
  return mog_int(1);
}

static MogValue host_timestamp(MogVM *vm, MogArgs *args) {
  (void)vm; (void)args;
  return mog_int((int64_t)time(NULL));
}

static MogValue host_random(MogVM *vm, MogArgs *args) {
  (void)vm;
  int64_t min_val = mog_arg_int(args, 0);
  int64_t max_val = mog_arg_int(args, 1);
  if (max_val <= min_val) return mog_int(min_val);
  int64_t range = max_val - min_val + 1;
  return mog_int(min_val + (rand() % range));
}

static MogValue host_log(MogVM *vm, MogArgs *args) {
  (void)vm;
  const char *message = mog_arg_string(args, 0);
  printf("[mog] %s\n", message);
  return mog_none();
}
```

### Step 3: Build the Registration Table and Register

```c
static const MogCapEntry env_functions[] = {
  { "get_name",     host_get_name     },
  { "get_version",  host_get_version  },
  { "timestamp",    host_timestamp    },
  { "random",       host_random       },
  { "log",          host_log          },
  { NULL, NULL }
};

// In your setup code:
MogVM *vm = mog_vm_new();
mog_register_capability(vm, "env", env_functions);
mog_vm_set_global(vm);
```

### Step 4: Use It from Mog

```mog
requires env;

fn main() -> int {
  name := env.get_name();
  version := env.get_version();
  env.log("starting {name} v{version}");

  roll := env.random(1, 100);
  env.log("random roll: {roll}");

  ts := env.timestamp();
  env.log("timestamp: {ts}");

  return 0;
}
```

The compiler checks the calls against `env.mogdecl`. At runtime, the VM routes each call to the corresponding C function.

## MogValue: Data Exchange Between Host and Script

All data crossing the host-script boundary is wrapped in `MogValue`, a 24-byte tagged union:

```c
typedef struct {
  enum {
    MOG_INT,     // int64_t
    MOG_FLOAT,   // double
    MOG_BOOL,    // bool
    MOG_STRING,  // const char*
    MOG_NONE,    // no value
    MOG_HANDLE,  // opaque pointer + type name
    MOG_ERROR    // error message string
  } tag;
  union {
    int64_t     i;
    double      f;
    bool        b;
    const char *s;
    struct { void *ptr; const char *type_name; } handle;
    const char *error;
  } data;
} MogValue;
```

### Constructing Values

Return values to the script using constructor functions:

```c
MogValue v1 = mog_int(42);
MogValue v2 = mog_float(3.14);
MogValue v3 = mog_bool(true);
MogValue v4 = mog_string("hello");
MogValue v5 = mog_none();
MogValue v6 = mog_error("file not found");
MogValue v7 = mog_handle(my_pointer, "DatabaseConn");
```

There are also result helpers that read more naturally for success returns:

```c
return mog_ok_int(42);      // same as mog_int(42)
return mog_ok_float(3.14);  // same as mog_float(3.14)
return mog_ok_string("ok"); // same as mog_string("ok")
```

### Extracting Arguments

Read arguments from `MogArgs` with bounds-checked extractors. These abort on type mismatch — a host programming error, not a script error:

```c
static MogValue host_add(MogVM *vm, MogArgs *args) {
  (void)vm;
  int64_t a = mog_arg_int(args, 0);     // first argument as int
  int64_t b = mog_arg_int(args, 1);     // second argument as int
  return mog_int(a + b);
}

static MogValue host_greet(MogVM *vm, MogArgs *args) {
  (void)vm;
  const char *name = mog_arg_string(args, 0);
  printf("hello, %s\n", name);
  return mog_none();
}

static MogValue host_scale(MogVM *vm, MogArgs *args) {
  (void)vm;
  double value = mog_arg_float(args, 0);
  double factor = mog_arg_float(args, 1);
  return mog_float(value * factor);
}
```

You can also extract from standalone `MogValue`s (not wrapped in `MogArgs`):

```c
MogValue result = mog_cap_call(vm, "math", "add", args, 2);
int64_t sum = mog_as_int(result);
```

### Returning Errors

Host functions signal errors by returning a `MogValue` with the `MOG_ERROR` tag. The script sees this as an error value that propagates through `?` (see Chapter 11):

```c
static MogValue host_read_sensor(MogVM *vm, MogArgs *args) {
  (void)vm;
  int64_t sensor_id = mog_arg_int(args, 0);

  if (sensor_id < 0 || sensor_id > 7) {
    return mog_error("invalid sensor id");
  }

  double reading = read_hardware_sensor(sensor_id);
  return mog_float(reading);
}
```

From the Mog side:

```mog
requires hw;

async fn main() -> int {
  // The ? propagates the error if sensor_id is invalid
  temp := hw.read_sensor(3)?;
  print("sensor 3: {temp}");

  // This will produce an error
  bad := hw.read_sensor(99)?;  // error: "invalid sensor id"
  return 0;
}
```

### Opaque Handles

For host-side resources that shouldn't be inspected by the script (database connections, file handles, GPU contexts), use `MOG_HANDLE`:

```c
static MogValue host_db_connect(MogVM *vm, MogArgs *args) {
  (void)vm;
  const char *connstr = mog_arg_string(args, 0);
  DBConn *conn = db_open(connstr);
  if (!conn) return mog_error("connection failed");
  return mog_handle(conn, "DBConn");
}

static MogValue host_db_query(MogVM *vm, MogArgs *args) {
  (void)vm;
  // Extract handle with type checking — aborts if type doesn't match
  DBConn *conn = mog_arg_handle(args, 0, "DBConn");
  const char *sql = mog_arg_string(args, 1);
  // ... execute query ...
  return mog_string(result_json);
}
```

> **Warning:** Opaque handles are not garbage-collected. The host is responsible for managing the lifetime of the underlying resource. If the script drops a handle without calling a cleanup function, the host must detect this (e.g., via a finalizer or VM teardown hook) to avoid leaks.

## Resource Limits and Timeouts

An embedded script should never be able to freeze or crash the host. Mog provides three mechanisms for resource control.

### CPU Time Limits

Set a maximum execution time via `MogLimits`:

```c
MogLimits limits = {
  .max_memory     = 0,     // 0 = unlimited
  .max_cpu_ms     = 5000,  // 5 seconds
  .max_stack_depth = 0     // 0 = default (1024)
};
mog_vm_set_limits(vm, &limits);
```

When `max_cpu_ms` is set, the VM automatically arms a timeout. If the script exceeds the time limit, it is interrupted at the next loop back-edge and returns `MOG_INTERRUPT_CODE` (-99) to the host.

### Manual Timeout Arming

For finer control, arm and cancel timeouts directly:

```c
// Arm: interrupt after 3 seconds
mog_arm_timeout(3000);

int result = program_user();

if (result == MOG_INTERRUPT_CODE) {
  printf("script timed out\n");
}

// Cancel if script finished early
mog_cancel_timeout();
```

### Host-Initiated Interrupts

The interrupt mechanism is thread-safe. A watchdog thread, signal handler, or UI callback can request termination at any time:

```c
// From any thread:
mog_request_interrupt();
```

The script checks the interrupt flag at every loop back-edge. When it sees the flag, it stops execution and returns `MOG_INTERRUPT_CODE` to the host. This is cooperative — not a signal kill — so the VM stays in a clean state.

```c
// Clear the flag before running another script
mog_clear_interrupt();

// Check if an interrupt was requested
if (mog_interrupt_requested()) {
  printf("interrupt pending\n");
}
```

> **Tip:** The cooperative interrupt model means a script that blocks inside a host function (e.g., a long-running C call) cannot be interrupted until it returns to Mog code. Keep host functions short, or implement your own cancellation within long-running C functions.

### Complete Timeout Example

```c
#include "mog.h"
#include <stdio.h>

int main(void) {
  MogVM *vm = mog_vm_new();
  mog_vm_set_global(vm);
  mog_register_posix_host(vm);

  // 2-second timeout
  MogLimits limits = { .max_cpu_ms = 2000 };
  mog_vm_set_limits(vm, &limits);

  // Run the script
  int result = program_user();

  if (result == MOG_INTERRUPT_CODE) {
    fprintf(stderr, "script exceeded 2-second time limit\n");
  } else {
    printf("script exited with code %d\n", result);
  }

  mog_vm_free(vm);
  return (result == MOG_INTERRUPT_CODE) ? 1 : result;
}
```

## Safety Guarantees

Mog's embedding model provides several layers of safety:

**No raw pointers.** Mog scripts cannot construct or dereference pointers. The only way to hold a host resource is through an opaque `MOG_HANDLE`, and the host controls what operations are valid on it.

**No system calls without capabilities.** There is no `syscall()`, no `exec()`, no way to touch the OS except through capability functions the host explicitly registered. Chapter 14 explains the capability model in detail.

**GC-managed memory.** The script cannot leak memory or cause use-after-free. The VM's garbage collector handles all allocations.

**Cooperative interrupts.** The compiler inserts interrupt checks at every loop back-edge. An infinite loop can always be stopped by the host — no need for SIGKILL or process termination.

**Capability validation.** Before execution, `mog_validate_capabilities()` confirms that every capability the script requires is registered. Missing capabilities are caught before a single instruction runs.

## Practical Example: Embedding in a Game Server

Consider a game server that lets players write Mog scripts to customize NPC behavior. The host exposes a `game` capability:

### The `.mogdecl` File

```
capability game {
  fn get_npc_health(npc_id: int) -> int
  fn get_npc_position(npc_id: int) -> string
  fn move_npc(npc_id: int, x: int, y: int)
  fn npc_say(npc_id: int, message: string)
  fn get_nearest_player(npc_id: int) -> int
  fn distance_to(npc_id: int, target_id: int) -> int
}
```

### The Host Implementation (C)

```c
static MogValue game_get_npc_health(MogVM *vm, MogArgs *args) {
  (void)vm;
  int64_t npc_id = mog_arg_int(args, 0);
  NPC *npc = find_npc(npc_id);
  if (!npc) return mog_error("unknown npc");
  return mog_int(npc->health);
}

static MogValue game_move_npc(MogVM *vm, MogArgs *args) {
  (void)vm;
  int64_t npc_id = mog_arg_int(args, 0);
  int64_t x = mog_arg_int(args, 1);
  int64_t y = mog_arg_int(args, 2);
  NPC *npc = find_npc(npc_id);
  if (!npc) return mog_error("unknown npc");
  npc->x = (int)x;
  npc->y = (int)y;
  return mog_none();
}

static MogValue game_npc_say(MogVM *vm, MogArgs *args) {
  (void)vm;
  int64_t npc_id = mog_arg_int(args, 0);
  const char *msg = mog_arg_string(args, 1);
  broadcast_chat(npc_id, msg);
  return mog_none();
}

// ... remaining functions ...

static const MogCapEntry game_functions[] = {
  { "get_npc_health",    game_get_npc_health    },
  { "get_npc_position",  game_get_npc_position  },
  { "move_npc",          game_move_npc          },
  { "npc_say",           game_npc_say           },
  { "get_nearest_player",game_get_nearest_player},
  { "distance_to",       game_distance_to       },
  { NULL, NULL }
};
```

### The Player Script (Mog)

```mog
requires game;

fn tick(npc_id: int) -> int {
  health := game.get_npc_health(npc_id);

  // If low health, flee
  if health < 20 {
    game.npc_say(npc_id, "I must retreat!");
    game.move_npc(npc_id, 0, 0);  // move to safe zone
    return 0;
  }

  // Otherwise, approach the nearest player
  player := game.get_nearest_player(npc_id);
  dist := game.distance_to(npc_id, player);

  if dist < 5 {
    game.npc_say(npc_id, "Welcome, traveler!");
  }

  return 0;
}
```

### The Host Runner

```c
void run_npc_script(const char *script_path, int npc_id) {
  MogVM *vm = mog_vm_new();
  mog_register_capability(vm, "game", game_functions);
  mog_vm_set_global(vm);

  // Player scripts get 50ms max — one game tick
  MogLimits limits = { .max_cpu_ms = 50 };
  mog_vm_set_limits(vm, &limits);

  mog_arm_timeout(50);
  int result = program_user();

  if (result == MOG_INTERRUPT_CODE) {
    log_warning("npc %d script timed out", npc_id);
  }

  mog_vm_free(vm);
}
```

The script has access to exactly the game functions the host provides. It cannot read files, access the network, or inspect other NPCs' internal state beyond what `game` exposes. The 50ms timeout prevents a buggy script from stalling the server tick.

## Summary

| API Function | Purpose |
|---|---|
| `mog_vm_new()` | Create a VM instance |
| `mog_vm_set_global(vm)` | Set the global VM pointer |
| `mog_vm_free(vm)` | Destroy the VM |
| `mog_register_capability(vm, name, entries)` | Register a capability |
| `mog_register_posix_host(vm)` | Register built-in `fs` + `process` |
| `mog_validate_capabilities(vm, caps)` | Check all required capabilities exist |
| `mog_vm_set_limits(vm, &limits)` | Set resource limits |
| `mog_arm_timeout(ms)` | Arm a timeout timer |
| `mog_request_interrupt()` | Request script termination |
| `mog_cap_call(vm, cap, fn, args, n)` | Call a capability from C |
| `mog_int(v)`, `mog_string(s)`, ... | Construct a MogValue |
| `mog_arg_int(args, i)`, ... | Extract an argument |

The embedding model is intentionally simple: create, configure, run, destroy. The host is always in control — it decides what capabilities exist, how long scripts can run, and when to stop them. The script operates in a sandbox defined entirely by the host.

## Plugins — Dynamic Loading of Mog Code

The embedding model above compiles and runs a Mog script directly. Plugins take a different approach — you compile `.mog` files into shared libraries (`.dylib` on macOS, `.so` on Linux) ahead of time, then load them at runtime with `dlopen`. The host never sees the source code. It loads a binary, queries what functions are available, and calls them.

This is the right model when you need hot-swappable logic, third-party extensions, or a modular architecture where components are developed and compiled independently.

### Plugin Overview

A plugin is a pre-compiled Mog shared library. It contains:

- One or more exported functions callable from C
- A built-in copy of the Mog runtime (GC, value representation, etc.)
- Metadata: plugin name, version, and export table

The key difference from direct embedding:

| | Direct Embedding | Plugins |
|---|---|---|
| Source visible to host? | Yes | No |
| Compilation happens | At load time | Ahead of time |
| Swappable at runtime? | Requires recompile | Just replace the `.dylib` |
| Distribution | Ship `.mog` files | Ship binaries |

Use embedding when you control the scripts and want simplicity. Use plugins when you want pre-compiled, distributable, hot-swappable modules.

### Writing a Plugin

A plugin is a regular `.mog` file. The only difference is that you mark exported functions with `pub`:

```mog
// math_plugin.mog — Math utilities plugin

pub fn fibonacci(n: int) -> int {
  if (n <= 1) { return n; }
  return fibonacci(n - 1) + fibonacci(n - 2);
}

pub fn factorial(n: int) -> int {
  if (n <= 1) { return 1; }
  return n * factorial(n - 1);
}

pub fn gcd(a: int, b: int) -> int {
  x := a;
  y := b;
  while (y != 0) {
    t := x % y;
    x = y;
    y = t;
  }
  return x;
}

// Internal helper — NOT exported (no `pub` keyword)
fn square(x: int) -> int {
  return x * x;
}

pub fn sum_of_squares(a: int, b: int) -> int {
  return square(a) + square(b);
}
```

`pub` functions become symbols in the shared library that the host can call by name. Functions without `pub` are compiled with hidden linkage — they exist in the binary but are not visible to the loader.

Any top-level statements in the file run during plugin initialization, before the host calls any exported function. Use this for setup work like populating lookup tables.

### Compiling a Plugin

Use the `compilePluginToSharedLib()` API from TypeScript/Bun:

```typescript
import { compilePluginToSharedLib } from './src/compiler.ts';
import { readFileSync } from 'fs';

const source = readFileSync('math_plugin.mog', 'utf-8');
const result = await compilePluginToSharedLib(
  source,
  'math_plugin',         // plugin name
  'math_plugin.dylib',   // output path
  '1.0.0'                // version
);

if (result.errors.length > 0) {
  for (const e of result.errors) {
    console.error(`[${e.line}:${e.column}] ${e.message}`);
  }
}
```

The compiler runs the full pipeline: Mog source → LLVM IR → position-independent object code → shared library. The resulting `.dylib` is self-contained — it includes the Mog runtime, so the host doesn't need to link against anything beyond `mog_plugin.h`.

### Loading and Calling Plugins from C

Include `mog_plugin.h` alongside the standard `mog.h` header.

```c
#include <stdio.h>
#include "mog.h"
#include "mog_plugin.h"

int main(int argc, char **argv) {
    // Create a VM (required even for capability-free plugins)
    MogVM *vm = mog_vm_new();
    mog_vm_set_global(vm);

    // Load the plugin
    MogPlugin *plugin = mog_load_plugin("./math_plugin.dylib", vm);
    if (!plugin) {
        fprintf(stderr, "Failed: %s\n", mog_plugin_error());
        return 1;
    }

    // Inspect plugin metadata
    const MogPluginInfo *info = mog_plugin_get_info(plugin);
    printf("Plugin: %s v%s (%lld exports)\n",
           info->name, info->version, (long long)info->num_exports);

    // Call exported functions
    MogValue args[] = { mog_int(10) };
    MogValue result = mog_plugin_call(plugin, "fibonacci", args, 1);
    printf("fibonacci(10) = %lld\n", result.data.i);  // 55

    // Multiple arguments
    MogValue gcd_args[] = { mog_int(48), mog_int(18) };
    MogValue gcd_result = mog_plugin_call(plugin, "gcd", gcd_args, 2);
    printf("gcd(48, 18) = %lld\n", gcd_result.data.i);  // 6

    // Error handling for unknown functions
    MogValue bad = mog_plugin_call(plugin, "nonexistent", NULL, 0);
    if (bad.tag == MOG_ERROR) {
        printf("Error: %s\n", bad.data.error);
    }

    // Cleanup
    mog_unload_plugin(plugin);
    mog_vm_free(vm);
    return 0;
}
```

The pattern mirrors the embedding lifecycle: create VM, load, use, free. The difference is that `mog_load_plugin` replaces the compile-and-run step — the code is already compiled.

### Plugin C API Reference

| Function | Purpose |
|---|---|
| `mog_load_plugin(path, vm)` | Load a `.dylib`/`.so` plugin |
| `mog_load_plugin_sandboxed(path, vm, caps)` | Load with capability allowlist |
| `mog_plugin_call(plugin, name, args, nargs)` | Call an exported function by name |
| `mog_plugin_get_info(plugin)` | Get plugin metadata (name, version, exports) |
| `mog_plugin_error()` | Get last error message |
| `mog_unload_plugin(plugin)` | Unload and free plugin |

Arguments and return values use the same `MogValue` tagged union described earlier. Construct arguments with `mog_int()`, `mog_string()`, etc., and inspect results through the `.tag` and `.data` fields.

### Capability Sandboxing

Plugins can use `requires` declarations just like regular Mog programs. When a plugin declares `requires fs`, it means it will attempt to call filesystem functions during execution.

`mog_load_plugin` allows all capabilities. If you're loading untrusted code, use `mog_load_plugin_sandboxed` instead — it takes a `NULL`-terminated allowlist of capability names. If the plugin requires a capability not on the list, loading fails immediately:

```c
// Only allow 'log' capability — reject plugins needing 'fs', 'http', etc.
const char *allowed[] = { "log", NULL };
MogPlugin *plugin = mog_load_plugin_sandboxed("./untrusted.dylib", vm, allowed);
if (!plugin) {
    // Plugin requires a capability we don't allow
    fprintf(stderr, "%s\n", mog_plugin_error());
}
```

This is the same capability model from Chapter 14, applied at load time. The plugin's `requires` declarations are checked against the allowlist before any code runs. A plugin that passes the check cannot later escalate to capabilities it didn't declare.

### Plugin Protocol (Advanced)

Under the hood, the compiler generates four symbols in every plugin shared library:

- **`mog_plugin_info()`** — returns a pointer to a static `MogPluginInfo` struct containing the plugin name, version, and export count.
- **`mog_plugin_init(MogVM*)`** — initializes the GC, wires up the VM, and runs any top-level statements in the source file.
- **`mog_plugin_exports(int*)`** — returns an array of `{name, func_ptr}` pairs and writes the count to the output parameter.
- **Exported wrappers** — each `pub fn foo(...)` gets a `mogp_foo` wrapper with default (visible) linkage. Internal functions are emitted with `internal` linkage so they exist in the binary but are invisible to `dlopen`/`dlsym`.

`mog_load_plugin` calls these in order: resolve `mog_plugin_info` to validate compatibility, call `mog_plugin_init` to set up the runtime, then call `mog_plugin_exports` to build the dispatch table. After that, `mog_plugin_call` is just a name lookup and function pointer call.

You don't need to know any of this to use plugins. It's documented here so you can debug issues, write tooling, or implement plugin loaders in languages other than C.

# Chapter 16: Tensors — N-Dimensional Arrays

Mog provides tensors as a built-in data type — n-dimensional arrays with a fixed element dtype. They are the interchange format between Mog scripts and host-provided ML capabilities. You create tensors, read and write their elements, reshape them, and pass them to host functions. That's it.

Tensors in Mog are deliberately not a full tensor library. There is no built-in matmul, no convolution, no autograd. The language gives you the data container; the host gives you the compute. A script describes *what* data to prepare and *where* results go. The host decides *how* to execute the math — CPU, GPU, or remote accelerator.

## What Are Tensors?

A tensor is an n-dimensional array where every element has the same dtype. A 1D tensor is a vector. A 2D tensor is a matrix. A 3D tensor might be an image batch or a sequence of embeddings. The dimensionality is limited only by available memory.

Three properties define a tensor:

1. **Shape.** An array of integers describing the size of each dimension. A shape of `[3, 224, 224]` means 3 planes of 224×224 elements — 150,528 elements total.
2. **Dtype.** The element type. Mog supports `f16`, `bf16`, `f32`, and `f64`. The dtype is part of the tensor's type — `tensor<f32>` and `tensor<f16>` are different types.
3. **Data.** A flat, contiguous buffer of elements in row-major order.

Tensors are heap-allocated and garbage-collected like arrays and maps. They are passed by reference — assigning a tensor to a new variable does not copy the data.

```mog
a := tensor<f32>([3], [1.0, 2.0, 3.0]);
b := a;       // b and a point to the same data
b[0] = 99.0;  // a[0] is now 99.0 too
```

> **Note:** If you need an independent copy, use `.reshape()` with the same shape — it returns a new tensor with its own data buffer.

## Creating Tensors

### From a Literal

The most direct way to create a tensor is with an explicit shape and data array:

```mog
tensor<dtype>(shape, data)
```

The dtype must be specified — there is no inference. The shape is an array of integers. The data is a flat array of values in row-major order, and its length must equal the product of the shape dimensions.

```mog
// 1D tensor (vector) — 3 elements
v := tensor<f32>([3], [1.0, 2.0, 3.0]);

// 2D tensor (matrix) — 3 rows, 4 columns
matrix := tensor<f32>([3, 4], [
  1.0, 2.0,  3.0,  4.0,
  5.0, 6.0,  7.0,  8.0,
  9.0, 10.0, 11.0, 12.0,
]);

// 3D tensor — 2 matrices of 2x3
batch := tensor<f64>([2, 2, 3], [
  1.0, 2.0, 3.0,
  4.0, 5.0, 6.0,
  7.0, 8.0, 9.0,
  10.0, 11.0, 12.0,
]);

// Scalar (0D tensor) — shape is empty
scalar := tensor<f32>([], [42.0]);
```

If the data length doesn't match the shape, you get a runtime error:

```mog
// Runtime error: shape [2, 3] requires 6 elements, got 4
bad := tensor<f32>([2, 3], [1.0, 2.0, 3.0, 4.0]);
```

### Static Constructors

For common initialization patterns, tensor types provide static constructors:

```mog
// All zeros
zeros := tensor<f32>.zeros([3, 224, 224]);

// All ones
ones := tensor<f32>.ones([768]);

// Random normal distribution (mean 0, stddev 1)
random := tensor<f32>.randn([10, 10]);
```

These are the only built-in constructors. If you need other initialization patterns — linspace, arange, identity matrices — build them with a loop or request them from a host capability.

```mog
// Building an identity matrix manually
fn eye(n: int) -> tensor<f32> {
  t := tensor<f32>.zeros([n, n]);
  for i in 0..n {
    t[(i * n) + i] = 1.0;
  }
  return t;
}
```

### Supported Dtypes

| Dtype | Size | Description |
|---|---|---|
| `f16` | 2 bytes | IEEE 754 half-precision float |
| `bf16` | 2 bytes | Brain floating point (truncated f32 mantissa) |
| `f32` | 4 bytes | IEEE 754 single-precision float |
| `f64` | 8 bytes | IEEE 754 double-precision float |

The dtype is part of the type system. You cannot assign a `tensor<f32>` to a variable of type `tensor<f16>` — they are different types. There is no implicit conversion between tensor dtypes.

```mog
a := tensor<f32>.zeros([10]);
b := tensor<f16>.zeros([10]);
// a = b;  // Compile error: type mismatch
```

> **Tip:** Use `f32` as the default. Use `f16` or `bf16` when the host ML capability expects reduced-precision inputs — this is common for inference on GPUs. Use `f64` only when you need the extra precision, such as accumulating loss values over many steps.

## Tensor Properties

Every tensor exposes three read-only properties:

```mog
t := tensor<f32>([3, 224, 224], /* ... */);

t.shape   // [3, 224, 224] — array of ints
t.dtype   // "f32" — string
t.ndim    // 3 — int (same as t.shape.len)
```

These properties are useful for validating inputs, debugging shapes, and writing generic data-processing functions:

```mog
fn describe(t: tensor<f32>) {
  print("shape: {t.shape}, dtype: {t.dtype}, ndim: {t.ndim}");
  total := 1;
  for dim in t.shape {
    total = total * dim;
  }
  print("total elements: {total}");
}
```

| Property | Type | Description |
|---|---|---|
| `.shape` | `int[]` | Array of dimension sizes |
| `.dtype` | `string` | Element type name (`"f16"`, `"bf16"`, `"f32"`, `"f64"`) |
| `.ndim` | `int` | Number of dimensions (equal to `shape.len`) |

## Element Access

Tensor elements are accessed using flat indexing in row-major order. This is a single integer index into the underlying data buffer, not multi-dimensional indexing.

### Reading Elements

```mog
t := tensor<f32>([2, 3], [10.0, 20.0, 30.0, 40.0, 50.0, 60.0]);

// Flat index — row-major order
// Row 0: indices 0, 1, 2 → 10.0, 20.0, 30.0
// Row 1: indices 3, 4, 5 → 40.0, 50.0, 60.0
val := t[0];   // 10.0
val2 := t[4];  // 50.0
```

The returned value type depends on the tensor's dtype. For `tensor<f32>` and `tensor<f64>`, element access returns a `float`. For `tensor<f16>` and `tensor<bf16>`, the value is promoted to `float` on read.

### Writing Elements

```mog
t := tensor<f32>([3], [0.0, 0.0, 0.0]);
t[0] = 1.0;
t[1] = 2.0;
t[2] = 3.0;
// t is now [1.0, 2.0, 3.0]
```

Out-of-bounds access is a runtime error:

```mog
t := tensor<f32>([3], [1.0, 2.0, 3.0]);
// val := t[5];  // Runtime error: index 5 out of bounds for tensor with 3 elements
```

### Computing Flat Indices

For multi-dimensional tensors, you compute the flat index from coordinates manually. For a tensor with shape `[d0, d1, d2]`, the flat index of element `[i, j, k]` is `(i * d1 * d2) + (j * d2) + k`:

```mog
// 3x4 matrix
m := tensor<f32>([3, 4], [
  1.0, 2.0, 3.0, 4.0,
  5.0, 6.0, 7.0, 8.0,
  9.0, 10.0, 11.0, 12.0,
]);

// Element at row 1, column 2 → flat index = 1*4 + 2 = 6
val := m[(1 * 4) + 2];  // 7.0

// Helper function for 2D indexing
fn idx2d(shape: int[], row: int, col: int) -> int {
  return (row * shape[1]) + col;
}

val2 := m[idx2d(m.shape, 2, 3)];  // 12.0
```

## Shape Operations

Mog provides two built-in shape operations on tensors. Everything else — slicing, concatenation, padding — comes from host capabilities.

### Reshape

`.reshape(new_shape)` returns a new tensor with the same data but a different shape. The total number of elements must be the same:

```mog
t := tensor<f32>([2, 6], [
  1.0, 2.0, 3.0, 4.0, 5.0, 6.0,
  7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
]);

// Reshape 2x6 → 3x4
reshaped := t.reshape([3, 4]);
print(reshaped.shape);  // [3, 4]

// Reshape to 1D
flat := t.reshape([12]);
print(flat.shape);  // [12]

// Reshape to higher dimensions
cube := t.reshape([2, 2, 3]);
print(cube.shape);  // [2, 2, 3]
```

Mismatched element counts cause a runtime error:

```mog
t := tensor<f32>([2, 3], [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
// bad := t.reshape([2, 4]);  // Runtime error: cannot reshape 6 elements into shape [2, 4] (8 elements)
```

### Transpose

`.transpose()` reverses the order of dimensions. For a 2D tensor (matrix), this swaps rows and columns:

```mog
m := tensor<f32>([2, 3], [
  1.0, 2.0, 3.0,
  4.0, 5.0, 6.0,
]);

mt := m.transpose();
print(mt.shape);  // [3, 2]
// mt data in row-major order: [1.0, 4.0, 2.0, 5.0, 3.0, 6.0]
```

For higher-dimensional tensors, `.transpose()` reverses all axes. A tensor with shape `[2, 3, 4]` becomes `[4, 3, 2]`:

```mog
t := tensor<f32>.zeros([2, 3, 4]);
tt := t.transpose();
print(tt.shape);  // [4, 3, 2]
```

For 1D tensors, transpose is a no-op — it returns a tensor with the same shape.

## Tensors and Host Capabilities

Tensors exist to be passed between Mog scripts and host ML capabilities. The language provides the data structure; the host provides the operations. This separation means the host can implement operations however it wants — BLAS on CPU, CUDA on GPU, or a remote inference service — without the script needing to change.

### Passing Tensors to Host Functions

A host capability that performs ML operations receives tensors as arguments and returns tensors as results:

```mog
requires ml;

async fn main() -> int {
  // Prepare input — e.g., a flattened 28x28 grayscale image
  input := tensor<f32>.randn([1, 784]);

  // Pass to host for inference
  output := await ml.forward(input)?;

  // Inspect the result
  print("output shape: {output.shape}");
  print("output dtype: {output.dtype}");

  return 0;
}
```

The `.mogdecl` for such a capability might look like:

```
capability ml {
  async fn forward(input: tensor<f32>) -> tensor<f32>
  async fn matmul(a: tensor<f32>, b: tensor<f32>) -> tensor<f32>
  async fn relu(input: tensor<f32>) -> tensor<f32>
  async fn softmax(input: tensor<f32>) -> tensor<f32>
  async fn loss_mse(predicted: tensor<f32>, target: tensor<f32>) -> tensor<f32>
}
```

### Composing Operations

Since host capability calls are regular function calls that take and return tensors, you compose them naturally:

```mog
requires ml;

async fn two_layer_forward(x: tensor<f32>, w1: tensor<f32>, w2: tensor<f32>) -> Result<tensor<f32>> {
  // Layer 1: matmul + relu
  h := await ml.matmul(x, w1)?;
  h = await ml.relu(h)?;

  // Layer 2: matmul + softmax
  out := await ml.matmul(h, w2)?;
  out = await ml.softmax(out)?;

  return ok(out);
}
```

The script reads like a neural network definition, but every heavy operation is executed by the host. The Mog script is orchestrating — not computing.

### Why This Design?

Three reasons:

1. **Performance.** Tensor math belongs on hardware accelerators. A Mog script running on CPU should not be doing matrix multiplication — the host routes these operations to the fastest available backend.
2. **Safety.** ML operations that allocate GPU memory, launch kernels, or talk to remote services are side effects. Keeping them in capabilities means the host controls resource usage.
3. **Portability.** The same Mog script runs unchanged whether the host uses CPU, CUDA, Metal, or a cloud inference API. The script never names a device.

## Practical Examples

### Creating a Data Batch

Preparing a batch of input tensors for inference — for example, 32 flattened 28×28 images:

```mog
requires fs;

async fn load_batch(paths: string[], batch_size: int) -> tensor<f32> {
  image_size := 784;  // 28 * 28
  batch := tensor<f32>.zeros([batch_size, image_size]);

  for i in 0..batch_size {
    data := await fs.read_file(paths[i])?;
    // Assume data is a raw float file — parse into tensor elements
    for j in 0..image_size {
      batch[(i * image_size) + j] = parse_float(data, j);
    }
  }

  return batch;
}
```

### Reading Model Output Elements

After inference, extracting results from the output tensor:

```mog
fn argmax(t: tensor<f32>) -> int {
  // Find the index of the largest element (1D tensor)
  best_idx := 0;
  best_val := t[0];

  total := t.shape[0];
  for i in 1..total {
    if t[i] > best_val {
      best_val = t[i];
      best_idx = i;
    }
  }

  return best_idx;
}

fn top_k(t: tensor<f32>, k: int) -> int[] {
  // Return indices of the k largest elements
  indices := []int{};
  used := map<int, bool>{};

  for round in 0..k {
    best_idx := -1;
    best_val := -1.0e30;
    total := t.shape[0];

    for i in 0..total {
      if !used[i] && (t[i] > best_val) {
        best_val = t[i];
        best_idx = i;
      }
    }

    indices = append(indices, best_idx);
    used[best_idx] = true;
  }

  return indices;
}
```

### Preparing Input Tensors

Normalizing raw data before passing to a model:

```mog
fn normalize(t: tensor<f32>) -> tensor<f32> {
  total := t.shape[0];

  // Compute mean
  sum := 0.0;
  for i in 0..total {
    sum = sum + t[i];
  }
  mean := sum / float(total);

  // Compute standard deviation
  sq_sum := 0.0;
  for i in 0..total {
    diff := t[i] - mean;
    sq_sum = sq_sum + (diff * diff);
  }
  std := sqrt(sq_sum / float(total));

  // Normalize
  result := tensor<f32>.zeros(t.shape);
  for i in 0..total {
    result[i] = (t[i] - mean) / std;
  }

  return result;
}
```

### End-to-End Inference Script

Putting it all together — load data, normalize, run inference, report results:

```mog
requires ml, fs;

async fn main() -> int {
  // Load raw input
  raw := tensor<f32>([1, 784], /* loaded from file */);

  // Normalize
  input := normalize(raw);
  print("input shape: {input.shape}, dtype: {input.dtype}");

  // Run inference
  logits := await ml.forward(input)?;
  probs := await ml.softmax(logits)?;

  // Find prediction
  prediction := argmax(probs);
  confidence := probs[prediction];

  print("predicted class: {prediction}");
  print("confidence: {confidence}");

  return 0;
}
```

## Summary

| Concept | Syntax / Example | Description |
|---|---|---|
| Create from literal | `tensor<f32>([2, 3], [1.0, ...])` | Shape + flat data in row-major order |
| Zeros | `tensor<f32>.zeros([3, 224, 224])` | All elements initialized to 0.0 |
| Ones | `tensor<f32>.ones([768])` | All elements initialized to 1.0 |
| Random normal | `tensor<f32>.randn([10, 10])` | Elements from N(0, 1) distribution |
| Shape | `t.shape` | `int[]` — dimension sizes |
| Dtype | `t.dtype` | `string` — `"f32"`, `"f16"`, etc. |
| Dimensions | `t.ndim` | `int` — number of dimensions |
| Read element | `t[i]` | Flat index, row-major order |
| Write element | `t[i] = 42.0` | Flat index assignment |
| Reshape | `t.reshape([3, 4])` | New tensor, same data, new shape |
| Transpose | `t.transpose()` | Reverses dimension order |
| Supported dtypes | `f16`, `bf16`, `f32`, `f64` | Dtype is part of the type — no implicit conversion |
| Host ML ops | `await ml.forward(t)?` | All compute goes through capabilities |

Tensors are data containers. They hold the numbers. The host provides the math. This separation keeps Mog scripts portable, safe, and small — a script that prepares tensors and calls host capabilities works the same whether the host runs on a laptop CPU or a cluster of GPUs.
# Chapter 17: Advanced Topics

The previous chapters covered the core language — variables, functions, control flow, data structures, error handling, tensors, and embedding. This chapter collects the advanced features and design decisions that round out Mog: type aliases, scoped context blocks, memory layout optimizations, compilation backends, the interrupt system, garbage collection, and the things Mog deliberately leaves out.

## Type Aliases

Complex types get unwieldy. A function that accepts `fn(int, int) -> Result<int>` is harder to read than one that accepts `BinaryOp`. Type aliases give names to existing types without creating new ones:

```mog
type Callback = fn(int) -> int;
```

The alias is interchangeable with the original — no wrapping, no conversion:

```mog
type Callback = fn(int) -> int;

fn apply(f: Callback, x: int) -> int {
  return f(x);
}

fn double(n: int) -> int {
  return n * 2;
}

fn main() -> int {
  result := apply(double, 5);
  print(result);  // 10
  return 0;
}
```

Aliases work with any type — scalars, collections, maps, function signatures:

```mog
type Count = int;
type Score = float;
type Matrix = [[int]];
type StringMap = {string: string};
type Predicate = fn(int) -> bool;
```

Map type aliases are useful when a function accepts or returns configuration-style data:

```mog
type Config = {string: string};

fn default_config() -> Config {
  config: Config = {};
  config["mode"] = "release";
  config["backend"] = "llvm";
  return config;
}
```

Function type aliases shine when callbacks appear in multiple signatures:

```mog
type Comparator = fn(int, int) -> bool;

fn sort_by(arr: [int], cmp: Comparator) -> [int] {
  // sorting logic using cmp
  return arr;
}

fn ascending(a: int, b: int) -> bool {
  return a < b;
}

fn descending(a: int, b: int) -> bool {
  return a > b;
}
```

Aliases resolve transparently — the compiler sees through them during type checking. You cannot create a "distinct" type this way. A `Count` is an `int`, and the two are fully interchangeable.

## With Blocks

Some operations need a scoped context — a mode that activates before a block and deactivates after it, regardless of how the block exits. Mog uses `with` blocks for this:

```mog
with no_grad() {
  // gradient tracking is disabled here
  output := model_forward(input);
  print(output);
}
// gradient tracking resumes here
```

The `with` keyword takes a context expression and a block. The context is entered before the block runs and exited after it completes.

Currently, `no_grad()` is the primary context — it disables gradient tracking during ML inference when using host tensor capabilities. This avoids unnecessary memory and computation for operations that do not need backpropagation:

```mog
// Training: gradients tracked (default)
loss := compute_loss(predictions, targets);

// Inference: no gradients needed
with no_grad() {
  result := model_forward(test_input);
  print(result);
}
```

The block inside `with` is a normal scope. Variables declared inside are local to that block:

```mog
with no_grad() {
  temp := model_forward(input);
  print(temp);
}
// temp is not accessible here
```

`with` blocks can appear anywhere a statement is valid — inside functions, inside loops, nested inside other `with` blocks:

```mog
fn evaluate_batch(inputs: [tensor<f32>]) -> [tensor<f32>] {
  results: [tensor<f32>] = [];
  with no_grad() {
    for i in 0..inputs.len() {
      output := model_forward(inputs[i]);
      results.push(output);
    }
  }
  return results;
}
```

## Struct-of-Arrays (SoA) Performance

When you have thousands of structs and iterate over a single field, the default layout — an array of structs (AoS) — scatters that field's values across memory. Each struct's fields sit next to each other, so reading one field means loading every field into cache lines.

Struct-of-Arrays flips the layout. Instead of interleaving all fields per element, SoA stores each field in its own contiguous array. Iterating one field touches only that field's memory — ideal for cache performance.

Consider a particle system:

```mog
struct Particle {
  x: int,
  y: int,
  mass: int,
}
```

The default approach stores particles as an array of structs:

```mog
// Array of Structs — each element has all three fields together
particles: [Particle] = [];
particles.push(Particle { x: 0, y: 0, mass: 1 });
particles.push(Particle { x: 5, y: 3, mass: 2 });
```

The SoA approach uses the `soa` keyword to create a transposed layout:

```mog
// Struct of Arrays — each field stored in its own contiguous array
particles := soa Particle[10000];
```

Access syntax is identical — `particles[i].x` works the same way. The difference is in memory layout, not in your code:

```mog
// Initialize positions
for i in 0..10000 {
  particles[i].x = i;
  particles[i].y = i * 2;
  particles[i].mass = 1;
}

// Update all x values — contiguous memory access, cache-friendly
for i in 0..10000 {
  particles[i].x = particles[i].x + 1;
}
```

When should you use SoA? When you iterate over many elements and touch only one or two fields at a time. Physics simulations, particle systems, columnar data processing — these benefit from SoA. When you access all fields of each element together, regular arrays of structs are fine.

```mog
struct Star {
  brightness: float,
  temperature: float,
  distance: float,
  name: string,
}

// SoA: good when filtering by one field
stars := soa Star[50000];

// This loop only touches 'brightness' — contiguous reads
for i in 0..50000 {
  if stars[i].brightness > 5.0 {
    print(i);
  }
}
```

The capacity is fixed at creation. `soa Particle[10000]` allocates space for exactly 10,000 elements. This is a deliberate tradeoff — fixed size enables the compiler to lay out memory optimally and elide bounds checks in release builds.

## Compilation Backends: LLVM vs QBE

Mog compiles to native ARM64 and x86 binaries through two backends: LLVM and QBE. Both produce standalone executables — the difference is in compile speed versus runtime performance.

**LLVM** is the mature, industrial-strength backend. It applies aggressive optimizations — inlining, loop vectorization, dead code elimination, register allocation — and produces fast binaries. The cost is compile time. LLVM's optimization pipeline is large and thorough.

**QBE** is a lightweight backend — roughly 14,000 lines of C. It compiles significantly faster than LLVM but produces less optimized output. QBE focuses on correctness and simplicity over peak performance.

The practical tradeoffs:

| | LLVM | QBE |
|---|---|---|
| Compile speed | Slower | ~2x faster |
| Runtime performance | Better (at -O1) | Good, not optimized |
| Binary size | Comparable | Comparable |
| Target architectures | ARM64, x86 | ARM64, x86 |

During development, QBE's faster compile times shorten the edit-run cycle. For production or performance-sensitive scripts, LLVM at `-O1` produces better runtime results.

Both backends compile Mog through the same frontend — parsing, analysis, and type checking are identical. The divergence happens at code generation: one path emits LLVM IR, the other emits QBE IL. Both then link against the same C runtime library (which provides the garbage collector, tensor operations, async runtime, and host bindings).

## The Interrupt System

Mog scripts run inside a host application. The host needs a way to stop long-running or runaway scripts — a tight loop that never yields, an accidental infinite recursion, or simply a user pressing cancel.

Mog uses cooperative interrupt polling. The compiler inserts a check at every loop back-edge — the point where a `while`, `for`, or `for-each` loop jumps back to its condition. At each check, the script reads a volatile global flag. If the flag is set, the script exits immediately.

From the host side, two C functions control interrupts:

```c
// Request that the running script stop
void mog_request_interrupt(void);

// Arm an automatic timeout — interrupt fires after ms milliseconds
int mog_arm_timeout(int ms);
```

`mog_request_interrupt()` sets the flag directly. The script will stop at the next loop back-edge — usually within microseconds for any loop-heavy code.

`mog_arm_timeout(ms)` spawns a background thread that sleeps for the given duration, then sets the interrupt flag. This is useful for enforcing time limits on untrusted scripts:

```c
// Give the script 5 seconds
mog_arm_timeout(5000);
mog_run_script(vm, script);
mog_cancel_timeout();  // cancel if script finished early
```

The overhead of interrupt checking is small — roughly 1–3% in loop-heavy benchmarks. Each check is a single volatile load and a branch, which modern CPUs predict correctly almost every time (the flag is almost always zero).

The interrupt flag can also be cleared:

```c
void mog_clear_interrupt(void);
int mog_interrupt_requested(void);
```

This lets a host reset the flag between running multiple scripts, or check whether a script was interrupted versus completed normally.

Every loop type is covered — `while`, `for` with ranges, `for-each` over collections. Nested loops get independent checks. There is no way for a Mog script to disable or bypass interrupt polling.

## Memory Management

Mog is garbage collected. All heap allocations — structs, arrays, maps, strings, closures, tensors, SoA containers — go through the garbage collector. There is no manual `free`, no RAII, no reference counting.

The GC uses a mark-and-sweep algorithm with a shadow stack for root tracking:

1. **Allocation.** `gc_alloc` requests memory from the GC. If the allocation count exceeds a threshold, a collection cycle runs first.
2. **Root tracking.** Each function call pushes a GC frame (`gc_push_frame`). Local variables that hold heap pointers are registered as roots in the current frame (`gc_add_root`). When the function returns, the frame is popped (`gc_pop_frame`).
3. **Collection.** The collector walks the shadow stack, marks all reachable objects, then sweeps unmarked objects and frees their memory. The allocation threshold grows after each cycle to avoid excessive collection.

This is an implementation detail you rarely need to think about. You allocate by creating values — the language handles the rest:

```mog
fn make_particles(n: int) -> [Particle] {
  particles: [Particle] = [];
  for i in 0..n {
    // Each Particle is GC-allocated; no manual cleanup needed
    particles.push(Particle { x: i, y: i * 2, mass: 1 });
  }
  return particles;
}
// When particles is no longer reachable, the GC reclaims the memory
```

Closures capture variables by value (see Chapter 7), and the captured environment is itself a GC-allocated block:

```mog
fn make_counter(start: int) -> fn() -> int {
  count := start;
  return fn() -> int {
    count = count + 1;
    return count;
  };
}
// The closure's captured 'count' lives on the GC heap
```

There are no weak references, no finalizers, and no way to manually trigger collection from Mog code. The GC is non-generational and non-concurrent — it stops the script during collection. For the short-lived scripts Mog targets, this is a reasonable tradeoff: simplicity and correctness over pause-time optimization.

## What Mog Does NOT Have

Mog's design is subtractive. Every feature must justify its complexity, and many common language features did not make the cut. This is intentional — a smaller language is easier to learn, easier to embed, and easier to reason about.

**No generics** (beyond `tensor<dtype>`, `Result<T>`, and `?T`). Mog has a small, fixed set of parameterized types built into the language. You cannot define your own generic structs or functions. This eliminates an entire class of complexity — no type parameter inference, no trait bounds, no monomorphization. If you need a collection of a specific type, you use the built-in array, map, or struct types directly.

**No classes, inheritance, interfaces, or traits.** Mog has structs with fields and standalone functions. There is no method dispatch, no vtables, no subtyping hierarchy. If you need polymorphism, use closures — a `fn(int) -> int` doesn't care which function it points to.

**No operator overloading.** `+` means numeric addition or string concatenation. It cannot be redefined for custom types. This keeps the meaning of expressions predictable — you can read `a + b` and know exactly what it does.

**No macros or metaprogramming.** No compile-time code generation, no syntax extensions, no preprocessor. The language you read is the language that runs. This makes Mog code uniformly readable — there are no project-specific DSLs hiding behind macro expansions.

**No exceptions.** Error handling uses `Result<T>` and the `?` propagation operator (see Chapter 11). Errors are values, not control flow. Every function that can fail says so in its return type, and the compiler enforces that you handle the error or propagate it.

**No null.** Mog uses `?T` (Optional) for values that might be absent — `?int`, `?string` (see Chapter 11). The type system distinguishes between "definitely has a value" and "might not have a value." You cannot accidentally dereference something that does not exist.

**No raw pointers or manual memory management.** All memory is GC-managed. You cannot take the address of a variable, cast between pointer types, or free memory. This eliminates use-after-free, double-free, buffer overflows, and dangling pointers by construction.

**No implicit type coercion.** An `int` does not silently become a `float`. A `float` does not silently become a `string`. All conversions are explicit function calls — `float_from_string`, `int(x)`, and so on. This prevents an entire category of subtle bugs where silent coercion produces unexpected results.

**No standalone execution.** Mog scripts run inside a host application. There is no `mog run file.mog` command that produces a self-contained process with filesystem access, network sockets, or an event loop. The host provides capabilities, and the script declares which ones it needs (see Chapter 14). This is the core security model — a Mog script can only do what its host explicitly permits.

These omissions are not gaps to be filled in future versions. They are design decisions that keep Mog small, predictable, and safe for embedding. A language that tries to be everything ends up being harder to trust. Mog trades breadth for clarity.
# Chapter 18: Cookbook — Practical Programs

This chapter is a collection of complete, runnable Mog programs. Each one demonstrates a combination of language features in a realistic context — loops, structs, closures, error handling, async, capabilities. Read them in order or jump to whichever looks interesting.

## 1. FizzBuzz

The classic interview problem: print numbers 1 to 100, but replace multiples of 3 with "Fizz", multiples of 5 with "Buzz", and multiples of both with "FizzBuzz".

```mog
fn fizzbuzz(n: int) -> string {
  divisible_by_3 := (n % 3) == 0;
  divisible_by_5 := (n % 5) == 0;

  if divisible_by_3 && divisible_by_5 {
    return "FizzBuzz";
  } else if divisible_by_3 {
    return "Fizz";
  } else if divisible_by_5 {
    return "Buzz";
  }
  return str(n);
}

fn main() -> int {
  for i := 1 to 100 {
    println(fizzbuzz(i));
  }
  return 0;
}
```

The `for i := 1 to 100` loop is inclusive on both ends, so it covers exactly 1 through 100. The `str()` builtin converts an integer to its string representation.

## 2. Fibonacci Sequence

Two approaches to computing Fibonacci numbers: a clean recursive version and a fast iterative version. Both print the first 20 values.

### Recursive

```mog
fn fib(n: int) -> int {
  if n <= 1 {
    return n;
  }
  return fib(n - 1) + fib(n - 2);
}

fn main() -> int {
  for i in 0..20 {
    println(f"fib({i}) = {fib(i)}");
  }
  return 0;
}
```

This is the textbook definition. It's simple but exponentially slow — `fib(40)` would take a noticeable pause. Fine for small inputs and clarity.

### Iterative

```mog
fn fib_iter(n: int) -> int {
  if n <= 1 {
    return n;
  }
  a := 0;
  b := 1;
  for i in 2..(n + 1) {
    temp := a + b;
    a = b;
    b = temp;
  }
  return b;
}

fn main() -> int {
  results: [string] = [];
  for i in 0..20 {
    results.push(str(fib_iter(i)));
  }
  println(results.join(", "));
  return 0;
}
```

Output:

```
0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610, 987, 1597, 2584, 4181
```

The iterative version runs in linear time. The range `2..(n + 1)` is half-open, so it runs from 2 through `n` inclusive — exactly `n - 1` iterations, which is all you need.

## 3. Word Frequency Counter

Count how often each word appears in a list. This uses a map as a frequency table and demonstrates map creation, key checking, and iteration.

```mog
fn count_words(words: [string]) -> {string: int} {
  counts: {string: int} = {};
  for word in words {
    if counts.has(word) {
      counts[word] = counts[word] + 1;
    } else {
      counts[word] = 1;
    }
  }
  return counts;
}

fn main() -> int {
  words := [
    "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy",
    "dog", "the", "fox", "jumps", "high", "over", "the", "fence",
    "and", "the", "dog", "sleeps", "under", "the", "fence",
  ];

  freq := count_words(words);

  // Collect into an array of formatted strings for sorting
  entries: [string] = [];
  for word, count in freq {
    entries.push(f"{count}\t{word}");
  }
  entries.sort(fn(a: string, b: string) -> int {
    if a > b { return -1; }
    if a < b { return 1; }
    return 0;
  });

  println("Word frequencies (descending):");
  for entry in entries {
    println(f"  {entry}");
  }
  return 0;
}
```

The empty map `{}` needs a type annotation because Mog can't infer the value type from an empty literal. Sorting the formatted strings lexicographically (descending) puts higher counts first since the count is the leading character.

## 4. Simple Calculator with Error Handling

A calculator function that takes an operator string and two numbers, returning a `Result<int>`. This demonstrates how `Result` communicates failure through return types instead of exceptions.

```mog
fn calculate(op: string, a: int, b: int) -> Result<int> {
  match op {
    "+" => return ok(a + b),
    "-" => return ok(a - b),
    "*" => return ok(a * b),
    "/" => {
      if b == 0 {
        return err("division by zero");
      }
      return ok(a / b);
    },
    "%" => {
      if b == 0 {
        return err("modulo by zero");
      }
      return ok(a % b);
    },
    _ => return err(f"unknown operator: {op}"),
  }
}

fn eval_and_print(op: string, a: int, b: int) {
  match calculate(op, a, b) {
    ok(result) => println(f"  {a} {op} {b} = {result}"),
    err(e) => println(f"  {a} {op} {b} => ERROR: {e}"),
  }
}

fn main() -> int {
  println("Calculator:");
  eval_and_print("+", 10, 3);
  eval_and_print("-", 10, 3);
  eval_and_print("*", 10, 3);
  eval_and_print("/", 10, 3);
  eval_and_print("/", 10, 0);
  eval_and_print("%", 10, 3);
  eval_and_print("^", 2, 8);
  return 0;
}
```

Output:

```
Calculator:
  10 + 3 = 13
  10 - 3 = 7
  10 * 3 = 30
  10 / 3 = 3
  10 / 0 => ERROR: division by zero
  10 % 3 = 1
  2 ^ 8 => ERROR: unknown operator: ^
```

Every call site decides how to handle the error. The `match` on `calculate`'s return value forces the caller to acknowledge both cases. Nothing fails silently.

## 5. Data Validation Chain

Validate user input using a chain of `Result`-returning functions. The `?` operator propagates the first failure, short-circuiting the rest of the chain.

```mog
struct UserInput {
  username: string,
  email: string,
  age: int,
}

struct ValidUser {
  username: string,
  email: string,
  age: int,
}

fn validate_username(name: string) -> Result<string> {
  if name.len() == 0 {
    return err("username cannot be empty");
  }
  if name.len() < 3 {
    return err(f"username too short: {name.len()} chars (minimum 3)");
  }
  if name.len() > 20 {
    return err(f"username too long: {name.len()} chars (maximum 20)");
  }
  return ok(name);
}

fn validate_email(email: string) -> Result<string> {
  if email.len() == 0 {
    return err("email cannot be empty");
  }
  if !email.contains("@") {
    return err(f"invalid email (missing @): {email}");
  }
  if !email.contains(".") {
    return err(f"invalid email (missing domain): {email}");
  }
  return ok(email);
}

fn validate_age(age: int) -> Result<int> {
  if age < 0 {
    return err("age cannot be negative");
  }
  if age < 13 {
    return err(f"must be at least 13 years old (got {age})");
  }
  if age > 150 {
    return err(f"age seems unrealistic: {age}");
  }
  return ok(age);
}

fn validate_user(input: UserInput) -> Result<ValidUser> {
  name := validate_username(input.username)?;
  email := validate_email(input.email)?;
  age := validate_age(input.age)?;
  return ok(ValidUser { username: name, email: email, age: age });
}

fn main() -> int {
  inputs := [
    UserInput { username: "alice", email: "alice@example.com", age: 30 },
    UserInput { username: "", email: "bob@test.com", age: 25 },
    UserInput { username: "charlie", email: "not-an-email", age: 17 },
    UserInput { username: "dana", email: "dana@corp.io", age: 8 },
  ];

  for i, input in inputs {
    match validate_user(input) {
      ok(user) => println(f"[{i}] Valid: {user.username} ({user.email}), age {user.age}"),
      err(e) => println(f"[{i}] Invalid: {e}"),
    }
  }
  return 0;
}
```

Output:

```
[0] Valid: alice (alice@example.com), age 30
[1] Invalid: username cannot be empty
[2] Invalid: invalid email (missing @): not-an-email
[3] Invalid: must be at least 13 years old (got 8)
```

Each validator is a small, testable function. The `?` in `validate_user` means the function returns the first error it hits — you never get a confusing cascade of failures. If you wanted to collect *all* errors instead, you'd call each validator independently and accumulate the results.

## 6. Sorting and Filtering Pipeline

Create an array of structs, filter by a condition, sort the results, and display them. This is the bread-and-butter pattern for data processing in Mog.

```mog
struct Employee {
  name: string,
  department: string,
  salary: int,
  years: int,
}

fn main() -> int {
  staff := [
    Employee { name: "Alice", department: "engineering", salary: 120000, years: 5 },
    Employee { name: "Bob", department: "marketing", salary: 85000, years: 3 },
    Employee { name: "Charlie", department: "engineering", salary: 135000, years: 8 },
    Employee { name: "Dana", department: "design", salary: 95000, years: 4 },
    Employee { name: "Eve", department: "engineering", salary: 110000, years: 2 },
    Employee { name: "Frank", department: "marketing", salary: 78000, years: 1 },
    Employee { name: "Grace", department: "design", salary: 105000, years: 6 },
    Employee { name: "Hank", department: "engineering", salary: 145000, years: 10 },
  ];

  // Find engineers with 3+ years, sorted by salary descending
  senior_engineers := staff
    .filter(fn(e: Employee) -> bool { (e.department == "engineering") && (e.years >= 3) })
    .sort(fn(a: Employee, b: Employee) -> int { b.salary - a.salary });

  println("Senior Engineers (3+ years, by salary):");
  for e in senior_engineers {
    println(f"  {e.name} — ${e.salary}/yr, {e.years} years");
  }

  // Compute total payroll by department
  departments := ["engineering", "marketing", "design"];
  for dept in departments {
    total := 0;
    count := 0;
    for e in staff {
      if e.department == dept {
        total = total + e.salary;
        count = count + 1;
      }
    }
    avg := total / count;
    println(f"{dept}: {count} people, ${total} total, ${avg} avg");
  }

  return 0;
}
```

The `.filter().sort()` chain reads top-to-bottom: first narrow the dataset, then order it. The comparator `b.salary - a.salary` sorts descending because a positive result means `b` should come first.

## 7. Matrix Operations

Multiply two matrices represented as nested arrays. This demonstrates nested loops and 2D array construction.

```mog
fn matrix_multiply(a: [[int]], b: [[int]]) -> Result<[[int]]> {
  rows_a := a.len();
  if rows_a == 0 {
    return err("matrix A is empty");
  }
  cols_a := a[0].len();
  rows_b := b.len();
  if cols_a != rows_b {
    return err(f"dimension mismatch: A is {rows_a}x{cols_a}, B is {rows_b}x{b[0].len()}");
  }
  cols_b := b[0].len();

  // Build result matrix filled with zeros
  result: [[int]] = [];
  for i in 0..rows_a {
    row := [0; cols_b];
    result.push(row);
  }

  // Multiply
  for i in 0..rows_a {
    for j in 0..cols_b {
      sum := 0;
      for k in 0..cols_a {
        sum = sum + (a[i][k] * b[k][j]);
      }
      result[i][j] = sum;
    }
  }

  return ok(result);
}

fn print_matrix(label: string, m: [[int]]) {
  println(f"{label}:");
  for row in m {
    parts: [string] = [];
    for val in row {
      parts.push(str(val));
    }
    println(f"  [{parts.join(", ")}]");
  }
}

fn main() -> int {
  a := [
    [1, 2, 3],
    [4, 5, 6],
  ];

  b := [
    [7, 8],
    [9, 10],
    [11, 12],
  ];

  print_matrix("A (2x3)", a);
  print_matrix("B (3x2)", b);

  match matrix_multiply(a, b) {
    ok(c) => print_matrix("A × B", c),
    err(e) => println(f"Error: {e}"),
  }

  return 0;
}
```

Output:

```
A (2x3):
  [1, 2, 3]
  [4, 5, 6]
B (3x2):
  [7, 8]
  [9, 10]
  [11, 12]
A × B:
  [58, 64]
  [139, 154]
```

The `[0; cols_b]` syntax creates a pre-filled array of zeros. The function returns `Result` because the caller might pass matrices with incompatible dimensions — better to report the error than to crash with an out-of-bounds access.

## 8. Agent Tool-Use Script

This is the primary Mog use case: a script generated by an LLM agent that uses host-provided capabilities to accomplish a real task. It reads files, runs a shell command, checks environment variables, and writes a report — all through capabilities the host explicitly grants.

```mog
requires fs, process;
optional log;

struct FileInfo {
  path: string,
  size: int,
  extension: string,
}

fn get_extension(path: string) -> string {
  parts := path.split(".");
  if parts.len() < 2 {
    return "";
  }
  return parts[parts.len() - 1];
}

async fn scan_directory(dir: string) -> Result<[FileInfo]> {
  log.info(f"scanning directory: {dir}");
  entries := await fs.list_dir(dir)?;
  files: [FileInfo] = [];

  for entry in entries {
    path := f"{dir}/{entry}";
    if await fs.is_file(path)? {
      size := await fs.file_size(path)?;
      files.push(FileInfo {
        path: path,
        size: size,
        extension: get_extension(entry),
      });
    }
  }

  return ok(files);
}

async fn generate_report(files: [FileInfo]) -> Result<string> {
  // Group sizes by extension
  ext_sizes: {string: int} = {};
  ext_counts: {string: int} = {};
  total_size := 0;

  for f in files {
    ext := if f.extension.len() > 0 { f.extension } else { "(none)" };
    if ext_sizes.has(ext) {
      ext_sizes[ext] = ext_sizes[ext] + f.size;
      ext_counts[ext] = ext_counts[ext] + 1;
    } else {
      ext_sizes[ext] = f.size;
      ext_counts[ext] = 1;
    }
    total_size = total_size + f.size;
  }

  lines: [string] = [];
  lines.push(f"Directory Report");
  lines.push(f"================");
  lines.push(f"Total files: {files.len()}");
  lines.push(f"Total size:  {total_size} bytes");
  lines.push(f"");
  lines.push(f"By extension:");

  for ext, size in ext_sizes {
    count := ext_counts[ext];
    lines.push(f"  .{ext}: {count} files, {size} bytes");
  }

  // Find largest files
  sorted := files.sort(fn(a: FileInfo, b: FileInfo) -> int { b.size - a.size });
  lines.push(f"");
  lines.push(f"Largest files:");
  limit := if sorted.len() < 5 { sorted.len() } else { 5 };
  for i in 0..limit {
    lines.push(f"  {sorted[i].path} ({sorted[i].size} bytes)");
  }

  return ok(lines.join("\n"));
}

async fn main() -> int {
  dir := process.getenv("SCAN_DIR");
  if dir.len() == 0 {
    dir = process.cwd();
  }

  log.info(f"starting scan of {dir}");

  match await scan_directory(dir) {
    ok(files) => {
      println(f"Found {files.len()} files");
      match await generate_report(files) {
        ok(report) => {
          println(report);
          await fs.write_file("report.txt", report)?;
          log.info("report written to report.txt");
        },
        err(e) => println(f"Report generation failed: {e}"),
      }
    },
    err(e) => println(f"Scan failed: {e}"),
  }

  return 0;
}
```

A few things to notice:

- `requires fs, process;` declares the capabilities this script needs. The host must provide both, or the script won't run.
- `optional log;` means the script can function without logging — calls to `log.info()` are silently ignored if the host doesn't provide the `log` capability.
- Every filesystem operation is `async` and returns `Result`, so the script uses `await` and `?` together: `await fs.read_file(path)?`.
- The `main` function is `async fn main()` because it calls async capability functions.

## 9. Recursive Tree Traversal

Build a tree out of structs and traverse it with recursion. This demonstrates self-referential structs, recursive functions, and accumulating results.

```mog
struct TreeNode {
  label: string,
  value: int,
  children: [TreeNode],
}

fn tree_sum(node: TreeNode) -> int {
  total := node.value;
  for child in node.children {
    total = total + tree_sum(child);
  }
  return total;
}

fn tree_depth(node: TreeNode) -> int {
  if node.children.len() == 0 {
    return 1;
  }
  max_child := 0;
  for child in node.children {
    d := tree_depth(child);
    if d > max_child {
      max_child = d;
    }
  }
  return max_child + 1;
}

fn tree_find(node: TreeNode, target: string) -> ?TreeNode {
  if node.label == target {
    return some(node);
  }
  for child in node.children {
    match tree_find(child, target) {
      some(found) => return some(found),
      none => {},
    }
  }
  return none;
}

fn print_tree(node: TreeNode, indent: int) {
  prefix := "";
  for i in 0..indent {
    prefix = prefix + "  ";
  }
  println(f"{prefix}{node.label} ({node.value})");
  for child in node.children {
    print_tree(child, indent + 1);
  }
}

fn main() -> int {
  tree := TreeNode {
    label: "root", value: 1,
    children: [
      TreeNode {
        label: "math", value: 10,
        children: [
          TreeNode { label: "algebra", value: 3, children: [] },
          TreeNode { label: "calculus", value: 7, children: [] },
        ],
      },
      TreeNode {
        label: "science", value: 20,
        children: [
          TreeNode {
            label: "physics", value: 5,
            children: [
              TreeNode { label: "optics", value: 2, children: [] },
              TreeNode { label: "mechanics", value: 4, children: [] },
            ],
          },
          TreeNode { label: "chemistry", value: 8, children: [] },
        ],
      },
      TreeNode { label: "art", value: 15, children: [] },
    ],
  };

  println("Tree structure:");
  print_tree(tree, 0);

  println(f"\nTotal value: {tree_sum(tree)}");
  println(f"Max depth: {tree_depth(tree)}");

  match tree_find(tree, "physics") {
    some(node) => println(f"Found '{node.label}' with value {node.value}"),
    none => println("Not found"),
  }

  match tree_find(tree, "history") {
    some(node) => println(f"Found '{node.label}'"),
    none => println("'history' not found in tree"),
  }

  return 0;
}
```

Output:

```
Tree structure:
root (1)
  math (10)
    algebra (3)
    calculus (7)
  science (20)
    physics (5)
      optics (2)
      mechanics (4)
    chemistry (8)
  art (15)

Total value: 75
Max depth: 4
Found 'physics' with value 5
'history' not found in tree
```

Three recursive functions work the tree: `tree_sum` adds all values, `tree_depth` finds the longest path, and `tree_find` searches by label and returns `?TreeNode`. The `tree_find` function shows how `?` return types compose with recursion — each level either returns `some(found)` or continues searching.

## 10. Async Pipeline

Chain multiple async operations together, each depending on the previous result. This demonstrates `await`, `?` propagation through async calls, and parallel execution with `all()`.

```mog
requires http, fs;
optional log;

struct ApiResult {
  source: string,
  data: string,
  status: int,
}

async fn fetch_json(url: string) -> Result<ApiResult> {
  log.info(f"fetching {url}");
  response := await http.get(url)?;
  if response.status != 200 {
    return err(f"HTTP {response.status} from {url}");
  }
  return ok(ApiResult {
    source: url,
    data: response.body,
    status: response.status,
  });
}

async fn fetch_all_sources(urls: [string]) -> Result<[ApiResult]> {
  // Build futures for parallel execution
  futures: [Result<ApiResult>] = [];
  for url in urls {
    futures.push(fetch_json(url));
  }

  results := await all(futures)?;
  log.info(f"fetched {results.len()} sources");
  return ok(results);
}

fn merge_results(results: [ApiResult]) -> string {
  lines: [string] = [];
  for r in results {
    lines.push(f"--- Source: {r.source} (HTTP {r.status}) ---");
    lines.push(r.data);
    lines.push("");
  }
  return lines.join("\n");
}

async fn process_and_save(urls: [string], output_path: string) -> Result<int> {
  // Step 1: Fetch all sources in parallel
  results := await fetch_all_sources(urls)?;

  // Step 2: Filter out empty responses
  valid := results.filter(fn(r: ApiResult) -> bool { r.data.len() > 0 });
  log.info(f"{valid.len()} of {results.len()} sources returned data");

  if valid.len() == 0 {
    return err("all sources returned empty data");
  }

  // Step 3: Merge into a single document
  merged := merge_results(valid);

  // Step 4: Write to disk
  await fs.write_file(output_path, merged)?;
  log.info(f"wrote {merged.len()} bytes to {output_path}");

  return ok(valid.len());
}

async fn main() -> int {
  urls := [
    "https://api.example.com/users",
    "https://api.example.com/orders",
    "https://api.example.com/inventory",
  ];

  match await process_and_save(urls, "combined_data.txt") {
    ok(count) => println(f"Pipeline complete: {count} sources merged"),
    err(e) => {
      println(f"Pipeline failed: {e}");
      return 1;
    },
  }

  return 0;
}
```

The pipeline flows top-to-bottom: fetch in parallel, filter, merge, write. Each step either succeeds and feeds into the next, or fails and the error propagates up through `?`. The `await all(futures)?` line does two things — it waits for all fetches to complete in parallel, then unwraps the combined result, failing if any single fetch failed.

> **Note:** Returning a nonzero exit code from `main` signals failure to the host. The `return 1;` in the error branch lets the host know the script didn't complete successfully.

## Summary

| Program | Key Features |
|---|---|
| FizzBuzz | `for` loop, `if`/`else`, `str()` |
| Fibonacci | Recursion, iterative accumulation, `0..n` range |
| Word Frequency | Maps, `.has()`, `for key, value in map` |
| Calculator | `Result<T>`, `match` on result, `ok()` / `err()` |
| Validation Chain | `?` propagation, composing `Result` functions |
| Sort & Filter | Structs, `.filter()`, `.sort()`, closures |
| Matrix Multiply | Nested arrays, `[0; n]` fill, triple nested loop |
| Agent Script | `requires`/`optional`, `async`/`await`, capabilities |
| Tree Traversal | Recursive structs, `?T` optional, depth-first search |
| Async Pipeline | `await all()`, chained async steps, error propagation |

These programs cover the core of practical Mog: data processing with arrays and maps, error handling with `Result` and `?`, struct-based domain modeling, recursive algorithms, and async capability-driven scripts. Each pattern scales — the validation chain works for 3 fields or 30, the async pipeline works for 3 URLs or 300.
