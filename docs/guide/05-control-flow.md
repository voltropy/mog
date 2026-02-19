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
