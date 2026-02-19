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
