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
