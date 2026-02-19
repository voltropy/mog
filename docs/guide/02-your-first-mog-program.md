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
