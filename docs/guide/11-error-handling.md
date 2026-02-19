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
    if n % 2 == 0 {
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
