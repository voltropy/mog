# Chapter 6: Functions

Functions are the primary building blocks of any Mog program. They group reusable
logic behind a name, accept typed parameters, and can return values. This chapter
covers everything from basic declarations to recursion and the built-in functions
that ship with every Mog program.

## Basic Function Declarations

A function starts with the `fn` keyword, followed by a name, a parameter list
with type annotations, an optional return type after `->`, and a body in curly
braces.

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

Every function that produces a value must use an explicit `return` statement.
Mog does not support implicit returns — the last expression in a function body
is not automatically returned. This keeps control flow unambiguous.

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

When a function performs an action but doesn't produce a value, omit the `-> Type`
annotation. The function implicitly returns void.

```mog
fn log_message(level: string, message: string) {
  print("[{level}] {message}");
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
  print(line);
}
```

A void function can still use `return;` to exit early:

```mog
fn print_if_positive(n: int) {
  if n <= 0 {
    return;
  }
  print(n);
}
```

## Parameters and Return Types

Parameters are declared with `name: Type` syntax. Every parameter must have a
type annotation — Mog does not infer parameter types.

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

fn contains(haystack: string, needle: string) -> bool {
  // String search logic
  return haystack.find(needle) >= 0;
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

Functions can declare default values for parameters. Callers may then omit those
arguments or pass them by name in any order.

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

Named arguments are especially useful when a function has several optional
parameters:

```mog
fn create_server(
  host: string = "localhost",
  port: int = 8080,
  max_connections: int = 100,
  timeout_ms: int = 30000,
) {
  print("Starting server on {host}:{port}");
  print("Max connections: {max_connections}");
  print("Timeout: {timeout_ms}ms");
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
  print("Searching for '{query}' (max={max_results}, case={case_sensitive}, sort={sort_by})");
  // ... search implementation ...
  results: [string] = [];
  return results;
}

search("mog language");
search("mog language", max_results: 50, sort_by: "date");
search(query: "Functions", case_sensitive: true);
```

## Calling Conventions

Mog supports both positional and named calling styles. You can mix them, but
positional arguments must come before named arguments:

```mog
fn send_email(to: string, subject: string, body: string = "", urgent: bool = false) {
  print("To: {to}");
  print("Subject: {subject}");
  if urgent {
    print("[URGENT]");
  }
  if body != "" {
    print(body);
  }
}

// Positional
send_email("alice@example.com", "Meeting", "See you at 3pm", true);

// Named
send_email(to: "bob@example.com", subject: "Lunch?");

// Mixed: positional first, then named
send_email("carol@example.com", "Report", urgent: true);
```

## Recursion

Functions can call themselves. Mog does not guarantee tail-call optimization, so
deep recursion will consume stack space proportional to the call depth.

**Factorial:**

```mog
fn factorial(n: int) -> int {
  if n <= 1 {
    return 1;
  }
  return n * factorial(n - 1);
}

print(factorial(5));   // 120
print(factorial(10));  // 3628800
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

for i in 0..10 {
  print(fibonacci(i));
}
// 0, 1, 1, 2, 3, 5, 8, 13, 21, 34
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

data := [2, 5, 8, 12, 16, 23, 38, 56, 72, 91];
idx := binary_search(data, 23, 0, data.len - 1);
print("Found 23 at index {idx}");  // Found 23 at index 5
```

**Greatest common divisor:**

```mog
fn gcd(a: int, b: int) -> int {
  if b == 0 {
    return a;
  }
  return gcd(b, a % b);
}

print(gcd(48, 18));   // 6
print(gcd(100, 75));  // 25
```

For deep or performance-sensitive recursion, consider rewriting with a loop:

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

Mog provides a set of math functions as builtins. No imports required. All math
builtins operate on `float` (f64) values.

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

Examples:

```mog
// Distance between two points
fn distance(x1: float, y1: float, x2: float, y2: float) -> float {
  dx := x2 - x1;
  dy := y2 - y1;
  return sqrt(dx * dx + dy * dy);
}

print(distance(0.0, 0.0, 3.0, 4.0));  // 5.0
```

```mog
// Convert degrees to radians and compute trig values
fn deg_to_rad(degrees: float) -> float {
  return degrees * 3.14159265 / 180.0;
}

angle := deg_to_rad(45.0);
print(sin(angle));   // ~0.7071
print(cos(angle));   // ~0.7071
```

```mog
// Compound interest
fn compound_interest(principal: float, rate: float, years: int) -> float {
  return principal * pow(1.0 + rate, years);
}

result := compound_interest(1000.0, 0.05, 10);
print(round(result));  // 1629.0
```

```mog
// Clamp a float between bounds
fn clamp_float(value: float, lo: float, hi: float) -> float {
  return max(lo, min(hi, value));
}

print(clamp_float(150.0, 0.0, 100.0));  // 100.0
print(clamp_float(-5.0, 0.0, 100.0));   // 0.0
```

```mog
// Estimate how many bits are needed to represent n
fn bits_needed(n: int) -> int {
  if n <= 0 {
    return 1;
  }
  return floor(log2(n + 0.0)) + 1;
}

print(bits_needed(255));   // 8
print(bits_needed(256));   // 9
```

## Conversion Functions

Mog provides built-in functions for converting between types.

**`str(value)`** — Convert an int, float, or bool to a string:

```mog
s1 := str(42);       // "42"
s2 := str(3.14);     // "3.14"
s3 := str(true);     // "true"

print("The answer is " + str(42));
```

**`len(array)`** — Get the length of an array as an int:

```mog
numbers := [10, 20, 30, 40];
print(len(numbers));  // 4

empty: [string] = [];
print(len(empty));    // 0
```

**`int_from_string(s)`** — Parse a string into an int. Returns `Result<int>`
because the parse can fail:

```mog
result := int_from_string("42");
match result {
  ok(n) => print("Parsed: {n}"),
  err(msg) => print("Failed: {msg}"),
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
  ok(f) => print("Got pi: {f}"),
  err(msg) => print("Not a float: {msg}"),
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

A complete example combining conversion functions:

```mog
fn parse_csv_row(line: string) -> Result<{name: string, age: int, score: float}> {
  parts := line.split(",");
  if len(parts) != 3 {
    return err("expected 3 fields, got {len(parts)}");
  }
  age := int_from_string(parts[1])?;
  score := parse_float(parts[2])?;
  return ok({name: parts[0], age: age, score: score});
}

row := parse_csv_row("Alice,30,95.5");
match row {
  ok(data) => print("{data.name} is {str(data.age)} years old with score {str(data.score)}"),
  err(msg) => print("Parse error: {msg}"),
}
```
