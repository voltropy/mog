# Chapter 18: Cookbook — Practical Programs

This chapter is a collection of complete, runnable Mog programs. Each one demonstrates a combination of language features in a realistic context — loops, structs, closures, error handling, async, capabilities. Read them in order or jump to whichever looks interesting.

## 1. FizzBuzz

The classic interview problem: print numbers 1 to 100, but replace multiples of 3 with "Fizz", multiples of 5 with "Buzz", and multiples of both with "FizzBuzz".

```mog
fn fizzbuzz(n: int) -> string {
  divisible_by_3 := n % 3 == 0;
  divisible_by_5 := n % 5 == 0;

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
  for i in 2..n + 1 {
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

The iterative version runs in linear time. The range `2..n + 1` is half-open, so it runs from 2 through `n` inclusive — exactly `n - 1` iterations, which is all you need.

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
    .filter(fn(e: Employee) -> bool { e.department == "engineering" && e.years >= 3 })
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
        sum = sum + a[i][k] * b[k][j];
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
