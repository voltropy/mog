# Chapter 7: Closures and Higher-Order Functions

In the previous chapter, functions were always named and declared at the top level. Mog also supports **closures** — anonymous functions that can be created inline, stored in variables, passed as arguments, and returned from other functions. When combined with higher-order functions (functions that accept or return other functions), closures unlock powerful and concise patterns for working with data.

## Closure Syntax

A closure is an anonymous function written with the `fn` keyword but without a name. It is typically assigned to a variable or passed directly as an argument.

```mog
add := fn(a: int, b: int) -> int { return a + b; };

print(add(3, 4));  // 7
```

The syntax mirrors named functions: parameters with types, an optional return type, and a body in curly braces. The trailing semicolon is required because the closure assignment is a statement.

```mog
square := fn(n: int) -> int { return n * n; };
is_positive := fn(x: float) -> bool { return x > 0.0; };
greet := fn(name: string) -> string { return "hello, {name}!"; };

print(square(5));         // 25
print(is_positive(-3.0)); // false
print(greet("Alice"));    // hello, Alice!
```

Closures that take no parameters and return nothing work too:

```mog
say_hi := fn() { print("hi"); };
say_hi();  // hi
```

## Capturing Variables

Closures can reference variables from their enclosing scope. This is what distinguishes a closure from a plain function pointer — it "closes over" the environment where it was created.

```mog
multiplier := 3;
triple := fn(n: int) -> int { return n * multiplier; };

print(triple(10));  // 30
print(triple(7));   // 21
```

Closures can capture multiple variables:

```mog
base_url := "https://api.example.com";
api_key := "sk-12345";

make_url := fn(endpoint: string) -> string {
  return "{base_url}/{endpoint}?key={api_key}";
};

print(make_url("users"));  // https://api.example.com/users?key=sk-12345
```

### Value Capture Semantics

Mog captures variables **by value** — the closure gets a snapshot of each captured variable at the moment the closure is created. Later changes to the original do not affect the closure's copy.

```mog
count := 10;
get_count := fn() -> int { return count; };

count = 20;
print(get_count());  // 10 — captured the value 10
print(count);        // 20 — the original is 20
```

This matters in loops. Each iteration creates a new closure that captures the loop variable's current value:

```mog
makers: [fn() -> int] = [];
for i in 0..5 {
  makers.push(fn() -> int { return i; });
}

print(makers[0]());  // 0
print(makers[3]());  // 3
```

> Internally, closures are implemented as a fat pointer: `{fn_ptr, env_ptr}`. The runtime copies only the variables the closure actually references. This is an implementation detail you rarely need to think about.

## Type Aliases for Function Types

Function type signatures can get verbose. Use `type` to create aliases:

```mog
type Predicate = fn(int) -> bool;
type Transform = fn(int) -> int;
type Callback = fn(string);
```

These aliases simplify function signatures:

```mog
type Transform = fn(int) -> int;

fn apply_twice(f: Transform, value: int) -> int {
  return f(f(value));
}

double := fn(n: int) -> int { return n * 2; };
print(apply_twice(double, 3));  // 12
```

Without the alias, the signature would be `fn apply_twice(f: fn(int) -> int, value: int) -> int` — correct but harder to read at a glance.

```mog
type Predicate = fn(int) -> bool;
type Formatter = fn(int) -> string;

fn find_first(items: [int], pred: Predicate) -> ?int {
  for item in items {
    if pred(item) { return some(item); }
  }
  return none;
}

fn format_all(items: [int], fmt: Formatter) -> [string] {
  return items.map(fmt);
}
```

## Passing Closures to Functions

Closures are first-class values. You can pass them as arguments using the function type syntax `fn(ParamTypes) -> ReturnType`.

```mog
fn apply(f: fn(int) -> int, x: int) -> int {
  return f(x);
}

double := fn(n: int) -> int { return n * 2; };
negate := fn(n: int) -> int { return -n; };

print(apply(double, 5));   // 10
print(apply(negate, 5));   // -5
```

You can pass closures inline without naming them:

```mog
print(apply(fn(n: int) -> int { return n * n; }, 4));  // 16
```

A function that transforms every element of an array:

```mog
fn transform(arr: [int], f: fn(int) -> int) -> [int] {
  result: [int] = [];
  for item in arr {
    result.push(f(item));
  }
  return result;
}

numbers := [1, 2, 3, 4, 5];

doubled := transform(numbers, fn(n: int) -> int { return n * 2; });
print(doubled);  // [2, 4, 6, 8, 10]

offset := 100;
shifted := transform(numbers, fn(n: int) -> int { return n + offset; });
print(shifted);  // [101, 102, 103, 104, 105]
```

## Returning Closures from Functions

Functions can create and return closures. The returned closure retains access to any variables it captured — even after the enclosing function has returned.

```mog
fn make_adder(n: int) -> fn(int) -> int {
  return fn(x: int) -> int { return x + n; };
}

add5 := make_adder(5);
add100 := make_adder(100);

print(add5(3));    // 8
print(add100(3));  // 103
```

```mog
fn make_multiplier(factor: float) -> fn(float) -> float {
  return fn(x: float) -> float { return x * factor; };
}

to_km := make_multiplier(1.60934);
print(to_km(10.0));  // 16.0934
```

This factory pattern is useful for creating families of related functions from a single template. You will see it again in Chapter 9 when we build constructor functions for structs.

## Closures with Array Methods

Mog arrays have built-in methods — `filter`, `map`, and `sort` — that accept closures. These methods return new arrays; they do not modify the original. See Chapter 10 for the full set of collection operations.

### filter

`filter` takes a predicate closure and returns a new array containing only the elements for which the predicate returns `true`.

```mog
numbers := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

evens := numbers.filter(fn(n: int) -> bool { return (n % 2) == 0; });
print(evens);  // [2, 4, 6, 8, 10]

big := numbers.filter(fn(n: int) -> bool { return n > 7; });
print(big);  // [8, 9, 10]
```

Filter with a captured threshold:

```mog
scores := [45, 72, 88, 91, 53, 67, 79, 95];
cutoff := 70;

passing := scores.filter(fn(s: int) -> bool { return s >= cutoff; });
print(passing);  // [72, 88, 91, 79, 95]
```

### map

`map` takes a transform closure and returns a new array with each element replaced by the closure's result.

```mog
numbers := [1, 2, 3, 4, 5];

doubled := numbers.map(fn(n: int) -> int { return n * 2; });
print(doubled);  // [2, 4, 6, 8, 10]

labels := numbers.map(fn(n: int) -> string { return "item-{str(n)}"; });
print(labels);  // ["item-1", "item-2", "item-3", "item-4", "item-5"]
```

```mog
names := ["alice", "bob", "carol"];
lengths := names.map(fn(name: string) -> int { return name.len; });
print(lengths);  // [5, 3, 5]
```

### sort

`sort` takes a comparator closure that returns `true` when the first argument should come before the second. It returns a new sorted array.

```mog
numbers := [5, 2, 8, 1, 9, 3];

ascending := numbers.sort(fn(a: int, b: int) -> bool { return a < b; });
print(ascending);  // [1, 2, 3, 5, 8, 9]

descending := numbers.sort(fn(a: int, b: int) -> bool { return a > b; });
print(descending);  // [9, 8, 5, 3, 2, 1]
```

Sorting structs by a specific field:

```mog
struct Player {
  name: string,
  score: int,
}

players := [
  Player{name: "Alice", score: 250},
  Player{name: "Bob", score: 180},
  Player{name: "Carol", score: 320},
];

by_score := players.sort(fn(a: Player, b: Player) -> bool {
  return a.score > b.score;
});

for p in by_score {
  print("{p.name}: {str(p.score)}");
}
// Carol: 320
// Alice: 250
// Bob: 180
```

### Chaining Methods

Filter, map, and sort can be chained for expressive data pipelines:

```mog
numbers := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

result := numbers
  .filter(fn(n: int) -> bool { return (n % 2) == 0; })
  .map(fn(n: int) -> int { return n * n; })
  .sort(fn(a: int, b: int) -> bool { return a > b; });

print(result);  // [100, 64, 36, 16, 4]
```

> Method chaining reads top-to-bottom: filter the evens, square them, sort descending. Each step returns a new array, so the original `numbers` is untouched.

## Summary

| Concept | Syntax |
|---|---|
| Create a closure | `fn(params) -> Type { body }` |
| Type alias | `type Name = fn(ParamTypes) -> ReturnType;` |
| Pass to function | `fn do_it(f: fn(int) -> int) { ... }` |
| Return from function | `fn make() -> fn(int) -> int { ... }` |
| Filter an array | `arr.filter(fn(x: T) -> bool { ... })` |
| Map an array | `arr.map(fn(x: T) -> U { ... })` |
| Sort an array | `arr.sort(fn(a: T, b: T) -> bool { ... })` |

Closures capture by value, are first-class values, and combine naturally with array methods for concise data processing. In the next chapter, we will look at Mog's string type in detail.
