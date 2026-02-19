# Chapter 7: Closures and Higher-Order Functions

In the previous chapter, functions were always named and declared at the top level.
Mog also supports **closures** — anonymous functions that can be created inline,
stored in variables, passed as arguments, and returned from other functions. When
combined with higher-order functions (functions that accept or return other
functions), closures unlock powerful and concise patterns for working with data.

## Closure Syntax

A closure is an anonymous function written with the `fn` keyword but without a
name. It's typically assigned to a variable or passed directly as an argument.

```mog
// Assign a closure to a variable
add := fn(a: int, b: int) -> int { return a + b; };

print(add(3, 4));  // 7
```

The syntax mirrors named functions: parameters with types, an optional return type,
and a body in curly braces. The trailing semicolon is required because the closure
assignment is a statement.

```mog
// A few more closures
square := fn(n: int) -> int { return n * n; };
is_positive := fn(x: float) -> bool { return x > 0.0; };
shout := fn(s: string) -> string { return s + "!!!"; };

print(square(5));         // 25
print(is_positive(-3.0)); // false
print(shout("hello"));   // hello!!!
```

Closures that take no parameters and return nothing work too:

```mog
say_hi := fn() { print("hi"); };
say_hi();  // hi
```

## Capturing Variables from Outer Scope

Closures can reference variables from their enclosing scope. This is what
distinguishes a closure from a plain function pointer — it "closes over" the
environment where it was created.

```mog
multiplier := 3;
triple := fn(n: int) -> int { return n * multiplier; };

print(triple(10));  // 30
print(triple(7));   // 21
```

```mog
prefix := "ERROR";
format_error := fn(msg: string) -> string {
  return "[{prefix}] {msg}";
};

print(format_error("disk full"));       // [ERROR] disk full
print(format_error("connection lost")); // [ERROR] connection lost
```

Closures can capture multiple variables:

```mog
base_url := "https://api.example.com";
api_key := "sk-12345";

make_request := fn(endpoint: string) -> string {
  return "{base_url}/{endpoint}?key={api_key}";
};

print(make_request("users"));   // https://api.example.com/users?key=sk-12345
print(make_request("posts"));   // https://api.example.com/posts?key=sk-12345
```

## Value Capture Semantics

Mog captures variables **by value** — the closure gets a snapshot of each
captured variable at the moment the closure is created. Later changes to the
original variable do not affect the closure's copy.

```mog
count := 10;
get_count := fn() -> int { return count; };

count = 20;  // modify the original
print(get_count());  // 10 — the closure captured the value 10
print(count);        // 20 — the original variable is 20
```

This matters in loops. Each iteration creates a new closure, and each one
captures the value of the loop variable at that point in time:

```mog
makers: [fn() -> int] = [];
for i in 0..5 {
  makers.push(fn() -> int { return i; });
}

// Each closure captured a different value of i
print(makers[0]());  // 0
print(makers[1]());  // 1
print(makers[2]());  // 2
print(makers[3]());  // 3
print(makers[4]());  // 4
```

Another example to reinforce this — mutations after closure creation have no
effect:

```mog
name := "Alice";
greeter := fn() -> string { return "Hello, {name}"; };

name = "Bob";
print(greeter());  // Hello, Alice — captured at creation time
```

Internally, closures are implemented as a **fat pointer**: `{fn_ptr, env_ptr}`.
The `fn_ptr` points to the compiled function code, and the `env_ptr` points to
a heap-allocated copy of the captured variables. This is an implementation detail
you rarely need to think about, but it explains why capture-by-value is efficient —
the runtime copies only the variables the closure actually references.

## Passing Closures to Functions

Closures are first-class values in Mog. You can pass them as arguments to other
functions using the function type syntax `fn(ParamTypes) -> ReturnType`.

```mog
fn apply(f: fn(int) -> int, x: int) -> int {
  return f(x);
}

double := fn(n: int) -> int { return n * 2; };
negate := fn(n: int) -> int { return -n; };

print(apply(double, 5));   // 10
print(apply(negate, 5));   // -5
```

You can also pass closures inline without naming them:

```mog
fn apply(f: fn(int) -> int, x: int) -> int {
  return f(x);
}

print(apply(fn(n: int) -> int { return n * n; }, 4));  // 16
```

A function that applies an operation to every element of an array:

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

squared := transform(numbers, fn(n: int) -> int { return n * n; });
print(squared);  // [1, 4, 9, 16, 25]

offset := 100;
shifted := transform(numbers, fn(n: int) -> int { return n + offset; });
print(shifted);  // [101, 102, 103, 104, 105]
```

Multiple function parameters work naturally:

```mog
fn combine(
  a: int,
  b: int,
  merge: fn(int, int) -> int,
  format: fn(int) -> string,
) -> string {
  return format(merge(a, b));
}

result := combine(
  3, 7,
  fn(x: int, y: int) -> int { return x + y; },
  fn(n: int) -> string { return "result = {str(n)}"; },
);
print(result);  // result = 10
```

## Returning Closures from Functions

Functions can create and return closures. The returned closure retains access to
any variables it captured from the enclosing function's scope — even after that
function has returned.

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
to_celsius := make_multiplier(0.55556);  // rough (F-32)*5/9

print(to_km(10.0));       // 16.0934
print(to_celsius(50.0));  // 27.778
```

**Building a counter factory:**

```mog
fn make_counter(start: int) -> {next: fn() -> int, peek: fn() -> int} {
  current := start;
  return {
    next: fn() -> int {
      val := current;
      current = current + 1;
      return val;
    },
    peek: fn() -> int {
      return current;
    },
  };
}

c := make_counter(0);
print(c.next());  // 0
print(c.next());  // 1
print(c.next());  // 2
print(c.peek());  // 3
```

**Building a prefix logger:**

```mog
fn make_logger(prefix: string) -> fn(string) {
  return fn(msg: string) {
    print("[{prefix}] {msg}");
  };
}

info := make_logger("INFO");
warn := make_logger("WARN");
err := make_logger("ERROR");

info("Server started");   // [INFO] Server started
warn("Disk 90% full");    // [WARN] Disk 90% full
err("Connection failed");  // [ERROR] Connection failed
```

## Closures with Array Methods

Mog arrays have built-in methods — `filter`, `map`, and `sort` — that accept
closures. These methods return new arrays; they do not modify the original.

### filter

`filter` takes a predicate closure and returns a new array containing only the
elements for which the predicate returns `true`.

```mog
numbers := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

evens := numbers.filter(fn(n: int) -> bool { return n % 2 == 0; });
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

`map` takes a transform closure and returns a new array with each element
replaced by the result of the closure.

```mog
numbers := [1, 2, 3, 4, 5];

doubled := numbers.map(fn(n: int) -> int { return n * 2; });
print(doubled);  // [2, 4, 6, 8, 10]

labels := numbers.map(fn(n: int) -> string { return "item-{str(n)}"; });
print(labels);  // ["item-1", "item-2", "item-3", "item-4", "item-5"]
```

```mog
names := ["alice", "bob", "carol"];
lengths := names.map(fn(name: string) -> int { return len(name); });
print(lengths);  // [5, 3, 5]
```

### sort

`sort` takes a comparator closure that returns `true` when the first argument
should come before the second. It returns a new sorted array.

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
  Player{name: "Dave", score: 290},
];

by_score := players.sort(fn(a: Player, b: Player) -> bool {
  return a.score > b.score;
});

for p in by_score {
  print("{p.name}: {str(p.score)}");
}
// Carol: 320
// Dave: 290
// Alice: 250
// Bob: 180
```

### Chaining array methods

Filter, map, and sort can be chained together for expressive data pipelines:

```mog
struct Transaction {
  description: string,
  amount: float,
}

transactions := [
  Transaction{description: "Groceries", amount: -52.30},
  Transaction{description: "Salary", amount: 3200.00},
  Transaction{description: "Gas", amount: -45.00},
  Transaction{description: "Freelance", amount: 800.00},
  Transaction{description: "Rent", amount: -1200.00},
];

// Get descriptions of expenses over $50, sorted by amount
big_expenses := transactions
  .filter(fn(t: Transaction) -> bool { return t.amount < -50.0; })
  .sort(fn(a: Transaction, b: Transaction) -> bool { return a.amount < b.amount; })
  .map(fn(t: Transaction) -> string { return "{t.description}: {str(t.amount)}"; });

for line in big_expenses {
  print(line);
}
// Rent: -1200.0
// Groceries: -52.3
```

## Practical Patterns

### Callbacks

Pass closures to functions that should call them when something happens:

```mog
fn retry(
  attempts: int,
  action: fn() -> Result<string>,
  on_failure: fn(string, int),
) -> Result<string> {
  for i in 0..attempts {
    result := action();
    match result {
      ok(value) => { return ok(value); },
      err(msg) => { on_failure(msg, i + 1); },
    }
  }
  return err("all {str(attempts)} attempts failed");
}

result := retry(
  3,
  fn() -> Result<string> {
    // simulate a flaky operation
    return err("timeout");
  },
  fn(msg: string, attempt: int) {
    print("Attempt {str(attempt)} failed: {msg}");
  },
);
// Attempt 1 failed: timeout
// Attempt 2 failed: timeout
// Attempt 3 failed: timeout
```

### Strategy Pattern

Select behavior at runtime by choosing different closures:

```mog
fn format_list(items: [string], formatter: fn([string]) -> string) -> string {
  return formatter(items);
}

bullet_list := fn(items: [string]) -> string {
  result := "";
  for item in items {
    result = result + "  - " + item + "\n";
  }
  return result;
};

numbered_list := fn(items: [string]) -> string {
  result := "";
  for i in 0..items.len {
    result = result + "  " + str(i + 1) + ". " + items[i] + "\n";
  }
  return result;
};

comma_list := fn(items: [string]) -> string {
  return items.join(", ");
};

fruits := ["apple", "banana", "cherry"];

print(format_list(fruits, bullet_list));
//   - apple
//   - banana
//   - cherry

print(format_list(fruits, numbered_list));
//   1. apple
//   2. banana
//   3. cherry

print(format_list(fruits, comma_list));
// apple, banana, cherry
```

### Accumulators

Use closures to build up a result across repeated calls:

```mog
fn make_accumulator(initial: float) -> {add: fn(float), total: fn() -> float} {
  sum := initial;
  return {
    add: fn(amount: float) {
      sum = sum + amount;
    },
    total: fn() -> float {
      return sum;
    },
  };
}

acc := make_accumulator(0.0);
acc.add(10.5);
acc.add(20.0);
acc.add(3.75);
print(acc.total());  // 34.25
```

```mog
fn make_running_average() -> {update: fn(float), average: fn() -> float} {
  sum := 0.0;
  count := 0;
  return {
    update: fn(value: float) {
      sum = sum + value;
      count = count + 1;
    },
    average: fn() -> float {
      if count == 0 {
        return 0.0;
      }
      return sum / count;
    },
  };
}

avg := make_running_average();
avg.update(100.0);
avg.update(80.0);
avg.update(90.0);
print(avg.average());  // 90.0
avg.update(70.0);
print(avg.average());  // 85.0
```

### Composing Functions

Build new functions by combining existing ones:

```mog
fn compose(f: fn(int) -> int, g: fn(int) -> int) -> fn(int) -> int {
  return fn(x: int) -> int { return f(g(x)); };
}

double := fn(n: int) -> int { return n * 2; };
add_one := fn(n: int) -> int { return n + 1; };

double_then_add := compose(add_one, double);
add_then_double := compose(double, add_one);

print(double_then_add(5));  // 11 — (5*2) + 1
print(add_then_double(5));  // 12 — (5+1) * 2
```

```mog
fn pipeline(value: int, steps: [fn(int) -> int]) -> int {
  result := value;
  for step in steps {
    result = step(result);
  }
  return result;
}

output := pipeline(3, [
  fn(n: int) -> int { return n * 2; },     // 6
  fn(n: int) -> int { return n + 10; },    // 16
  fn(n: int) -> int { return n * n; },     // 256
]);
print(output);  // 256
```

### Memoization

Wrap a function to cache its results:

```mog
fn memoize(f: fn(int) -> int) -> fn(int) -> int {
  cache: map[int]int = {};
  return fn(x: int) -> int {
    if cache.has(x) {
      return cache[x];
    }
    result := f(x);
    cache[x] = result;
    return result;
  };
}

// Expensive computation
slow_square := fn(n: int) -> int {
  print("Computing {str(n)} squared...");
  return n * n;
};

fast_square := memoize(slow_square);
print(fast_square(5));  // Computing 5 squared... 25
print(fast_square(5));  // 25 — cached, no "Computing" message
print(fast_square(3));  // Computing 3 squared... 9
print(fast_square(3));  // 9 — cached
```
