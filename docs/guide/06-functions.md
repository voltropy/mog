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
  return n % 2 == 0;
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
  return sqrt(dx * dx + dy * dy);
}

fn main() -> int {
  println(distance(0.0, 0.0, 3.0, 4.0));  // 5.0
  return 0;
}
```

```mog
// Convert degrees to radians and compute trig values
fn deg_to_rad(degrees: float) -> float {
  return degrees * 3.14159265 / 180.0;
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
  if port < 1 || port > 65535 {
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
