# Mog Language Reference

Mog is a statically-typed, LLVM-compiled language designed for embedding. Think Lua with static types. Host programs provide capabilities; Mog scripts consume them safely.

## Syntax Overview

```mog
// Binding and reassignment
x := 42;           // initial binding (type inferred)
x = 100;           // reassignment
name: string = "hello";  // explicit type annotation

// Semicolons are required
// Comments: // only (no /* */)
```

## Types

| Type | Description |
|------|-------------|
| `int` (i64) | Default integer |
| `float` (f64) | Default float |
| `bool` | `true` / `false` |
| `string` | Heap-allocated, immutable content |
| `i8` `i16` `i32` `i64` | Signed integers |
| `u8` `u16` `u32` `u64` | Unsigned integers |
| `f32` `f64` | Floats |
| `[T]` | Array of T, e.g. `[int]`, `[string]`, `[Point]` |
| `{K: V}` | Map, e.g. `{string: int}` |
| `?T` | Optional, e.g. `?int`, `?string` |
| `Result<T>` | Result type with ok/err variants |
| `fn(T) -> U` | Function type |
| `tensor<dtype>` | N-dimensional array (f32/f64/i32/i64) |

Type aliases: `type Name = ExistingType;`

## Operators (FLAT — no precedence)

All binary operators have equal precedence. Mixing different operators requires parentheses.

```mog
// OK: same operator can chain if associative (+, *, and, or, &, |)
a + b + c;          // OK
a * b * c;          // OK

// REQUIRES PARENS: different operators
(a + b) * c;        // required
a + (b * c);        // required
(a + b) == (c + d); // required

// CANNOT CHAIN: non-associative operators (-, /, %, comparisons)
// a - b - c;       // ERROR — use (a - b) - c
```

Operators: `+` `-` `*` `/` `%` `==` `!=` `<` `<=` `>` `>=` `and`/`&&` `or`/`||` `&` `|` `^` `<<` `>>`

Unary: `-x` `!x`

String `+` concatenates: `"hello" + " " + "world"`

## Variables and Constants

```mog
x := 42;                    // inferred int
name: string = "Mog";       // explicit type
nums: [int] = [1, 2, 3];    // array
scores: {string: int} = {}; // empty map
```

## Functions

```mog
fn add(a: int, b: int) -> int {
  return a + b;
}

// Void function (no return type)
fn greet(name: string) {
  println(f"Hello, {name}!");
}

// Named arguments with defaults
fn connect(host: string, port: int = 8080, timeout: int = 30) -> int {
  return 0;
}
connect("localhost", port: 9090);

// Function types and closures
fn make_adder(x: int) -> fn(int) -> int {
  return fn(y: int) -> int { x + y };
}

// Lambdas (in filter, map, sort)
items.filter(fn(x: int) -> int { return x > 5; });
items.sort(fn(a: int, b: int) -> int { return a - b; });
```

## Control Flow

```mog
// If/else (also usable as expression)
if (x > 0) {
  println("positive");
} else if (x == 0) {
  println("zero");
} else {
  println("negative");
}

val := if (x > 0) { 1 } else { 0 };

// While
while (x > 0) {
  x = x - 1;
}

// For-in range (exclusive end)
for i in 0..10 {
  println_i64(i);   // 0 through 9
}

// For-in-to range (inclusive end)
for i := 1 to 10 {
  println_i64(i);   // 1 through 10
}

// For-in array
for item in items {
  println_i64(item);
}

// For-in with index
for i, item in items {
  println(f"{i}: {item}");
}

// For key/value in map
for key, value in my_map {
  println(f"{key}: {value}");
}

// Match expression
result := match x {
  0 => "zero",
  1 => "one",
  _ => "other"
};

// Break and continue work in loops
```

## Strings

```mog
s := "hello world";

// F-strings (interpolation)
println(f"name={name}, age={age}");

// Methods
s.len;                      // length (property, not method)
s.upper();                  // "HELLO WORLD"
s.lower();                  // "hello world"
s.trim();                   // strip whitespace
s.split(",");               // returns [string]
s.contains("world");        // bool
s.starts_with("hello");     // bool
s.ends_with("world");       // bool
s.replace("old", "new");    // new string

// Slicing
s[0:5];                     // "hello"

// Parsing
parse_int("42");            // 42
parse_float("3.14");        // 3.14

// Conversion
str(42);                    // "42"
```

## Arrays

```mog
nums: [int] = [1, 2, 3];
names: [string] = ["alice", "bob"];
empty: [int] = [];

// Repeat fill
zeros := [0; 100];          // 100 zeros

// Access and mutation
x := nums[0];
nums[0] = 10;

// Methods
nums.push(4);               // append
last := nums.pop();         // remove last
n := nums.len;              // length (property)
joined := names.join(", "); // "alice, bob"

// Higher-order
evens := nums.filter(fn(x: int) -> int { return x % 2 == 0; });
doubled := nums.map(fn(x: int) -> int { return x * 2; });
nums.sort(fn(a: int, b: int) -> int { return a - b; });
```

## Maps

```mog
scores: {string: int} = {"alice": 100, "bob": 85};

// Access
val := scores["alice"];

// Mutation
scores["carol"] = 90;

// Check existence
if (scores.has("alice")) {
  println("found");
}

// Iteration
for key, value in scores {
  println(f"{key}: {value}");
}

// Methods
n := scores.len;
keys := scores.keys();
values := scores.values();
```

## Structs

```mog
struct Point {
  x: int,
  y: int,
}

// Construction
p := Point { x: 3, y: 4 };

// Field access
println_i64(p.x);

// Structs are heap-allocated, passed by reference
```

## SoA (Struct of Arrays)

```mog
struct Particle {
  x: int,
  y: int,
  mass: int,
}

particles := soa Particle[1000];

// Access like array of structs
particles[0].x = 10;
val := particles[42].y;
```

## Error Handling

```mog
// Result<T>
fn divide(a: int, b: int) -> Result<int> {
  if (b == 0) {
    return err("division by zero");
  }
  return ok(a / b);
}

// Match on Result
match divide(10, 3) {
  ok(value) => println_i64(value),
  err(msg) => println(f"error: {msg}"),
}

// ? propagation (calling function must also return Result<T>)
fn calc() -> Result<int> {
  x := divide(10, 2)?;     // unwraps or propagates error
  return ok(x + 1);
}

// Try/catch
try {
  val := divide(10, 0)?;
} catch(e) {
  println(f"caught: {e}");
}

// Optional
fn find(items: [int], target: int) -> ?int {
  for i, v in items {
    if (v == target) {
      return some(i);
    }
  }
  return none;
}

match find(items, 42) {
  some(idx) => println_i64(idx),
  none => println("not found"),
}
```

## Async/Await

```mog
async fn fetch_data(url: string) -> int {
  response := await http.get(url);
  return parse_int(response);
}

async fn main() -> int {
  // Sequential
  a := await fetch_data("/api/1");
  b := await fetch_data("/api/2");

  // Concurrent (all waits for all, race waits for first)
  results := await all(fetch_data("/a"), fetch_data("/b"));

  // Fire-and-forget
  spawn background_task();

  return 0;
}
```

## Capabilities (Host FFI)

Programs declare what host services they need:

```mog
requires process, fs;     // must be provided by host
optional log;             // runs without it

async fn main() -> int {
  await process.sleep(100);
  cwd := process.cwd();
  
  fs.write_file("out.txt", "hello");
  contents := fs.read_file("out.txt");
  
  log.info("done");       // no-op if host doesn't provide log
  return 0;
}
```

### Available Capabilities (POSIX host)

**process**: `sleep(ms) -> int` (async), `getenv(name) -> string`, `cwd() -> string`, `exit(code) -> int`, `timestamp() -> int`

**fs**: `read_file(path) -> string`, `write_file(path, contents) -> int`, `append_file(path, contents) -> int`, `exists(path) -> bool`, `remove(path) -> int`, `file_size(path) -> int`

**log**: `info(msg)`, `warn(msg)`, `error(msg)`, `debug(msg)`

**http**: `get(url) -> string` (async), `post(url, body) -> string` (async)

Hosts can provide custom capabilities. Declare them in `.mogdecl` files:

```
capability my_service {
  fn compute(x: int) -> int;
  async fn fetch(key: string) -> string;
}
```

## Modules

```mog
// math/lib.mog
package math;

pub fn add(a: int, b: int) -> int {
  return a + b;
}

fn helper() -> int {  // private to module
  return 42;
}

// main.mog
package main;
import math;

fn main() -> int {
  result := math.add(1, 2);
  return 0;
}
```

Module root declared in `mog.mod`. `pub` makes declarations visible to importers.

## Plugins

Compile `.mog` files to shared libraries (`.dylib`/`.so`) loadable via `dlopen`:

```mog
// my_plugin.mog
pub fn fibonacci(n: int) -> int {
  if (n <= 1) { return n; }
  return fibonacci(n - 1) + fibonacci(n - 2);
}

fn internal_helper() -> int {   // not exported (no pub)
  return 42;
}
```

## Math Builtins

`abs` `sqrt` `pow` `sin` `cos` `tan` `asin` `acos` `atan2` `exp` `log` `log2` `floor` `ceil` `round` `min` `max`

Constants: `PI`, `E`

```mog
y := sqrt(2.0);
angle := atan2(1.0, 1.0);
m := max(a, b);
```

## Print Functions

```mog
println("hello");          // string with newline
println_i64(42);           // int with newline
print_string("no newline");
print_i64(42);
print_f64(3.14);
println_f64(3.14);
```

## Tensors (nd-arrays)

```mog
t := tensor<f32>([2, 3], [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
zeros := tensor_zeros([3, 3]);
ones := tensor_ones([4]);
random := tensor_randn([2, 2]);

// Element access
val := t[0];
t[1] = 3.14;

// Shape
println_i64(t.shape[0]);
println_i64(t.rank);
println_i64(t.size);

// Builtins
t2 := tensor_transpose(t);
t3 := tensor_relu(t);
t4 := tensor_softmax(t);

// ML operations are provided by host via capabilities, not builtins
```

## Type Casting

```mog
x := 42;
f := x as float;   // int -> float
n := f as int;      // float -> int (truncates)
```

## Entry Point

Every runnable program needs `fn main() -> int` (or `async fn main() -> int` for async programs). Return value is exit code.

```mog
fn main() -> int {
  println("Hello, Mog!");
  return 0;
}
```

## Complete Example

```mog
requires http, log;

struct SearchResult {
  title: string,
  url: string,
  score: int,
}

fn parse_results(response: string) -> [SearchResult] {
  results: [SearchResult] = [];
  r1 := SearchResult { title: "Intro to ML", url: "https://example.com/ml", score: 90 };
  results.push(r1);
  r2 := SearchResult { title: "Deep Learning", url: "https://example.com/dl", score: 85 };
  results.push(r2);
  return results;
}

async fn search(query: string) -> Result<[SearchResult]> {
  response := await http.get(f"/api/search?q={query}");
  results := parse_results(response);
  log.info(f"found {results.len()} results for '{query}'");
  return ok(results);
}

async fn main() -> int {
  result := await search("machine learning");
  match result {
    ok(results) => {
      top := results.filter(fn(r: SearchResult) -> int { return r.score > 50; });
      for i, r in top {
        println(f"{r.title}: {r.url} (score: {r.score})");
      }
    },
    err(e) => {
      println(f"Search failed: {e}");
    },
  }
  return 0;
}
```
