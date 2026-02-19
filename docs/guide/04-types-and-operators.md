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
  is_even := x % 2 == 0;      // true
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
  return index < items.len && items[index] > 0;
}
```

Combining conditions:

```mog
fn is_valid_age(age: int) -> bool {
  return age >= 0 && age <= 150;
}

fn needs_review(score: int, flagged: bool) -> bool {
  return score < 50 || flagged;
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

## Operator Precedence

Operators are evaluated in this order, from highest to lowest precedence:

| Precedence | Operators | Description |
|---|---|---|
| 1 (highest) | `!` | Unary NOT |
| 2 | `*`, `/`, `%` | Multiplication, division, modulo |
| 3 | `+`, `-` | Addition, subtraction |
| 4 | `<<`, `>>` | Bit shifts |
| 5 | `&` | Bitwise AND |
| 6 | `^` | Bitwise XOR |
| 7 | `\|` | Bitwise OR |
| 8 | `==`, `!=`, `<`, `>`, `<=`, `>=` | Comparison |
| 9 | `&&` | Logical AND |
| 10 (lowest) | `\|\|` | Logical OR |

When in doubt, use parentheses:

```mog
// These are equivalent
result := a + b * c;
result := a + (b * c);

// But be explicit when mixing categories
result := (a & mask) == 0;         // compare after masking
result := (x > 0) && (y > 0);     // logical after comparison
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
  return sum as float / numbers.len as float;
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
  pct := value as float / total as float * 100.0;
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
- **When in doubt, add parentheses.** Mog follows conventional precedence, but clarity beats cleverness.
