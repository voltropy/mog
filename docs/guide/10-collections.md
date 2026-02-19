# Chapter 10: Collections

Mog provides three collection types: arrays for ordered sequences, maps for key-value lookup, and SoA (Struct of Arrays) for cache-friendly columnar storage. Together they cover the vast majority of data organization needs.

## Arrays

Arrays are dynamically-sized, ordered, homogeneous sequences. They grow and shrink as needed and are the most common collection type.

### Array Literals

Create arrays with bracket syntax. The element type is inferred:

```mog
fn main() -> int {
  numbers := [1, 2, 3, 4, 5];
  names := ["Alice", "Bob", "Charlie"];
  flags := [true, false, true];

  println(numbers.len());  // 5
  println(names.len());    // 3
  return 0;
}
```

### Repeat Syntax

Create arrays filled with a repeated value using `[value; count]`:

```mog
fn main() -> int {
  zeros := [0; 100];           // 100 zeros
  blank := [""; 10];           // 10 empty strings
  grid := [false; 64];         // 64 false values

  println(zeros.len());   // 100
  println(zeros[50]);     // 0
  return 0;
}
```

Use repeat syntax to create empty arrays — `[0; 0]` gives you an empty `[int]` ready for `.push()`:

```mog
fn main() -> int {
  buffer := [0; 0];     // empty int array
  scores := [0.0; 50];  // 50 floats, all 0.0
  return 0;
}
```

### Type Annotations

Array types are written as `[ElementType]`. Function parameters and return types always require explicit types:

```mog
fn sum(numbers: [int]) -> int {
  total := 0;
  for n in numbers {
    total = total + n;
  }
  return total;
}

fn first_or_default(items: [string], fallback: string) -> string {
  if items.len() > 0 {
    return items[0];
  }
  return fallback;
}

fn main() -> int {
  vals := [10, 20, 30];
  println(sum(vals));  // 60

  empty: [string] = [];
  println(first_or_default(empty, "none"));  // none
  return 0;
}
```

### Indexing

Access elements by zero-based index with brackets. Out-of-bounds access is a runtime error:

```mog
fn main() -> int {
  arr := [10, 20, 30, 40, 50];

  println(arr[0]);   // 10
  println(arr[4]);   // 50

  // Mutation by index
  arr[2] = 99;
  println(arr[2]);   // 99

  // Index with a variable
  i := 3;
  println(arr[i]);   // 40
  return 0;
}
```

> **Warning:** Accessing an index beyond the array's length causes a runtime panic. Always check `.len()` if the index is computed dynamically.

### Iteration

Use `for` to iterate over elements. The two-variable form gives you the index (see Chapter 5):

```mog
fn main() -> int {
  colors := ["red", "green", "blue"];

  // Value only
  for color in colors {
    println(color);
  }

  // Index and value
  for i, color in colors {
    println(f"{i}: {color}");
  }
  // 0: red
  // 1: green
  // 2: blue
  return 0;
}
```

### `.push()` and `.pop()`

Append to the end with `.push()`. Remove and return the last element with `.pop()`:

```mog
fn main() -> int {
  stack := [0; 0];

  stack.push(10);
  stack.push(20);
  stack.push(30);
  println(stack.len());  // 3

  top := stack.pop();
  println(top);          // 30
  println(stack.len());  // 2
  return 0;
}
```

A stack using push/pop:

```mog
fn main() -> int {
  stack := [0; 0];
  items := [5, 3, 8, 1, 9];

  for item in items {
    stack.push(item);
  }

  for stack.len() > 0 {
    println(stack.pop());
  }
  // 9, 1, 8, 3, 5
  return 0;
}
```

### `.slice()`

Extract a sub-array with `.slice(start, end)`. The range is half-open — `start` is inclusive, `end` is exclusive:

```mog
fn main() -> int {
  arr := [10, 20, 30, 40, 50];

  first_three := arr.slice(0, 3);
  println(first_three);  // [10, 20, 30]

  middle := arr.slice(1, 4);
  println(middle);  // [20, 30, 40]

  last_two := arr.slice(3, 5);
  println(last_two);  // [40, 50]
  return 0;
}
```

### `.contains()`

Check if an element exists in the array:

```mog
fn main() -> int {
  primes := [2, 3, 5, 7, 11, 13];

  println(primes.contains(7));   // true
  println(primes.contains(4));   // false

  allowed := ["admin", "editor", "viewer"];
  role := "editor";
  if allowed.contains(role) {
    println("access granted");
  }
  return 0;
}
```

### `.reverse()`

Reverse an array in place:

```mog
fn main() -> int {
  arr := [1, 2, 3, 4, 5];
  arr.reverse();
  println(arr);  // [5, 4, 3, 2, 1]
  return 0;
}
```

### `.join()`

Combine array elements into a single string with a separator:

```mog
fn main() -> int {
  words := ["hello", "world"];
  println(words.join(" "));   // hello world
  println(words.join(", "));  // hello, world
  println(words.join(""));    // helloworld

  numbers := [1, 2, 3];
  println(numbers.join("-"));  // 1-2-3
  return 0;
}
```

### `.filter()`

Return a new array containing only elements that pass a test:

```mog
fn main() -> int {
  numbers := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

  evens := numbers.filter(fn(n: int) -> bool { n % 2 == 0 });
  println(evens);  // [2, 4, 6, 8, 10]

  big := numbers.filter(fn(n: int) -> bool { n > 5 });
  println(big);  // [6, 7, 8, 9, 10]
  return 0;
}
```

Filter with a named function:

```mog
fn is_positive(n: int) -> bool {
  return n > 0;
}

fn main() -> int {
  values := [-3, -1, 0, 2, 5, -7, 4];
  positives := values.filter(is_positive);
  println(positives);  // [2, 5, 4]
  return 0;
}
```

### `.map()`

Return a new array with each element transformed:

```mog
fn main() -> int {
  numbers := [1, 2, 3, 4, 5];

  doubled := numbers.map(fn(n: int) -> int { n * 2 });
  println(doubled);  // [2, 4, 6, 8, 10]

  as_strings := numbers.map(fn(n: int) -> string { str(n) });
  println(as_strings.join(", "));  // 1, 2, 3, 4, 5
  return 0;
}
```

### `.sort()`

Sort an array in place using a comparator function. The comparator returns a negative integer if the first element should come before the second, positive if after, and zero if equal:

```mog
fn main() -> int {
  numbers := [5, 2, 8, 1, 9, 3];

  // Ascending
  numbers.sort(fn(a: int, b: int) -> int { a - b });
  println(numbers);  // [1, 2, 3, 5, 8, 9]

  // Descending
  numbers.sort(fn(a: int, b: int) -> int { b - a });
  println(numbers);  // [9, 8, 5, 3, 2, 1]
  return 0;
}
```

Sorting strings:

```mog
fn main() -> int {
  names := ["Charlie", "Alice", "Bob", "Dana"];

  names.sort(fn(a: string, b: string) -> int {
    if a < b { return -1; }
    if a > b { return 1; }
    return 0;
  });

  println(names.join(", "));  // Alice, Bob, Charlie, Dana
  return 0;
}
```

### Chaining Array Methods

Methods like `.filter()` and `.map()` return new arrays, so you can chain them into pipelines:

```mog
fn main() -> int {
  numbers := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

  // Get the squares of even numbers
  result := numbers
    .filter(fn(n: int) -> bool { n % 2 == 0 })
    .map(fn(n: int) -> int { n * n });
  println(result);  // [4, 16, 36, 64, 100]
  return 0;
}
```

```mog
fn main() -> int {
  words := ["hello", "world", "", "mog", "", "lang"];

  // Remove empty strings, convert to uppercase, join
  output := words
    .filter(fn(s: string) -> bool { s.len() > 0 })
    .map(fn(s: string) -> string { s.upper() })
    .join(" ");
  println(output);  // HELLO WORLD MOG LANG
  return 0;
}
```

> **Tip:** Chaining `.filter()` then `.map()` is the most common pipeline. If you need to both filter and transform, this reads more clearly than a manual loop.

## Maps

Maps are unordered key-value collections with string keys. Use them when you need to look up values by name rather than by position.

### Creating Maps

Create maps with brace syntax:

```mog
fn main() -> int {
  ages := { "Alice": 30, "Bob": 25, "Charlie": 35 };
  config := { "host": "localhost", "port": "8080" };
  return 0;
}
```

### Access and Mutation

Read values with bracket syntax. Set values the same way — writing to a key that doesn't exist creates it:

```mog
fn main() -> int {
  scores := { "math": 95, "english": 88, "science": 92 };

  // Read
  println(scores["math"]);     // 95

  // Write — update existing key
  scores["math"] = 100;
  println(scores["math"]);     // 100

  // Write — add new key
  scores["history"] = 87;
  println(scores["history"]);  // 87
  return 0;
}
```

### `.has()` — Checking Key Existence

Check whether a key exists before accessing it:

```mog
fn main() -> int {
  m := { "name": "Alice", "city": "NYC" };

  if m.has("name") {
    println(m["name"]);  // Alice
  }

  if !m.has("age") {
    println("age not found");
  }
  return 0;
}
```

> **Note:** Accessing a key that doesn't exist is a runtime error. Always use `.has()` or iterate with `for` when keys are dynamic.

### `.len()`, `.keys()`, `.values()`

```mog
fn main() -> int {
  m := { "a": 1, "b": 2, "c": 3 };

  println(m.len());     // 3

  ks := m.keys();
  println(ks);          // ["a", "b", "c"] (order may vary)

  vs := m.values();
  println(vs);          // [1, 2, 3] (order may vary)
  return 0;
}
```

### `.delete()` — Removing Entries

```mog
fn main() -> int {
  m := { "x": 10, "y": 20, "z": 30 };

  m.delete("y");
  println(m.len());     // 2
  println(m.has("y"));  // false
  return 0;
}
```

### Iterating Maps

Use `for key, value in map` to iterate over all entries:

```mog
fn main() -> int {
  prices := { "apple": 120, "banana": 80, "cherry": 300 };

  for name, price in prices {
    println(f"{name}: {price} cents");
  }
  return 0;
}
```

Collecting keys that meet a condition:

```mog
fn main() -> int {
  grades := { "Alice": 92, "Bob": 67, "Charlie": 85, "Dana": 45, "Eve": 78 };

  passing := [0; 0];
  for name, grade in grades {
    if grade >= 70 {
      passing.push(name);
    }
  }
  println(passing.join(", "));
  return 0;
}
```

### Practical Example: Word Counting

```mog
fn word_count(text: string) -> {string: int} {
  counts := { "": 0 };
  counts.delete("");  // start with empty map

  words := text.split(" ");
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
  text := "the cat sat on the mat the cat";
  counts := word_count(text);

  for word, n in counts {
    println(f"{word}: {n}");
  }
  // the: 3
  // cat: 2
  // sat: 1
  // on: 1
  // mat: 1
  return 0;
}
```

### Practical Example: Grouping Data

```mog
struct Student {
  name: string,
  grade: string,
}

fn group_by_grade(students: [Student]) -> {string: [string]} {
  groups := { "": [""] };
  groups.delete("");

  for s in students {
    if !groups.has(s.grade) {
      groups[s.grade] = [0; 0];
    }
    groups[s.grade].push(s.name);
  }
  return groups;
}

fn main() -> int {
  students := [
    Student { name: "Alice", grade: "A" },
    Student { name: "Bob", grade: "B" },
    Student { name: "Charlie", grade: "A" },
    Student { name: "Dana", grade: "C" },
    Student { name: "Eve", grade: "B" },
  ];

  groups := group_by_grade(students);
  for grade, names in groups {
    println(f"{grade}: {names.join(", ")}");
  }
  // A: Alice, Charlie
  // B: Bob, Eve
  // C: Dana
  return 0;
}
```

## SoA (Struct of Arrays)

SoA flips the usual memory layout. Instead of storing an array of structs (each struct contiguous in memory), SoA stores one contiguous array per field. This improves cache performance when you iterate over a single field across many elements — common in simulations, game engines, and data processing.

### When to Use SoA

Use SoA when:
- You have many instances of the same struct (hundreds or thousands)
- Your hot loops touch one or two fields at a time, not all of them
- Performance matters and you want cache-friendly access patterns

Use regular arrays of structs when:
- You have few instances
- You access all fields of each element together
- Simplicity matters more than cache behavior

### Construction

Define a regular struct (see Chapter 9), then create an SoA container with `soa StructName[capacity]`:

```mog
struct Particle {
  x: float,
  y: float,
  mass: float,
}

fn main() -> int {
  particles := soa Particle[1000];
  return 0;
}
```

This allocates three separate arrays of 1000 elements — one for `x`, one for `y`, one for `mass` — instead of one array of 1000 three-field structs.

### Field Access

Read and write fields using array-index-then-dot syntax:

```mog
fn main() -> int {
  particles := soa Particle[100];

  // Set fields
  particles[0].x = 10.0;
  particles[0].y = 20.0;
  particles[0].mass = 5.0;

  // Read fields
  println(particles[0].x);     // 10.0
  println(particles[0].mass);  // 5.0

  // Initialize several elements
  for i in 0..10 {
    particles[i].x = i as float * 10.0;
    particles[i].y = i as float * 5.0;
    particles[i].mass = 1.0;
  }

  println(particles[5].x);  // 50.0
  println(particles[9].y);  // 45.0
  return 0;
}
```

> **Tip:** The syntax `particles[i].x` looks like regular struct access, but under the hood the compiler lowers it to an index into the `x` column array. You get columnar storage with familiar syntax.

### Iteration Patterns

Iterating over a single field is where SoA shines. The compiler reads from a single contiguous array, keeping the CPU cache hot:

```mog
struct Entity {
  x: int,
  y: int,
  health: int,
  speed: int,
}

fn main() -> int {
  entities := soa Entity[500];

  // Initialize
  for i in 0..500 {
    entities[i].x = i;
    entities[i].y = i * 2;
    entities[i].health = 100;
    entities[i].speed = 5;
  }

  // Update all x positions — touches only the x array
  for i in 0..500 {
    entities[i].x = entities[i].x + entities[i].speed;
  }

  // Sum all health values — touches only the health array
  total_health := 0;
  for i in 0..500 {
    total_health = total_health + entities[i].health;
  }
  println(total_health);  // 50000
  return 0;
}
```

### Practical Example: Particle Simulation

A physics step that applies gravity and updates positions. Separating the gravity pass (which only touches `vy`) from the position pass (which touches `x`, `y`, `vx`, `vy`) maximizes cache efficiency:

```mog
struct Particle {
  x: float,
  y: float,
  vx: float,
  vy: float,
  mass: float,
}

fn step(particles: soa Particle, count: int, dt: float) {
  // Apply gravity — only touches vy array
  for i in 0..count {
    particles[i].vy = particles[i].vy - 9.8 * dt;
  }

  // Update positions — touches x, y, vx, vy arrays
  for i in 0..count {
    particles[i].x = particles[i].x + particles[i].vx * dt;
    particles[i].y = particles[i].y + particles[i].vy * dt;
  }
}

fn main() -> int {
  particles := soa Particle[1000];

  for i in 0..1000 {
    particles[i].x = 0.0;
    particles[i].y = 100.0;
    particles[i].vx = i as float * 0.1;
    particles[i].vy = 0.0;
    particles[i].mass = 1.0;
  }

  // Run 60 simulation steps
  for frame in 0..60 {
    step(particles, 1000, 0.016);
  }

  println(particles[0].y);
  println(particles[500].x);
  return 0;
}
```

### Practical Example: Column Operations

SoA is natural for column-oriented data processing — summing a column, finding a max, or filtering by a category all read from a single contiguous array:

```mog
struct Record {
  id: int,
  value: int,
  category: int,
}

fn column_sum(records: soa Record, count: int) -> int {
  total := 0;
  for i in 0..count {
    total = total + records[i].value;
  }
  return total;
}

fn column_max(records: soa Record, count: int) -> int {
  max_val := records[0].value;
  for i in 1..count {
    if records[i].value > max_val {
      max_val = records[i].value;
    }
  }
  return max_val;
}

fn count_category(records: soa Record, count: int, cat: int) -> int {
  n := 0;
  for i in 0..count {
    if records[i].category == cat {
      n = n + 1;
    }
  }
  return n;
}

fn main() -> int {
  data := soa Record[100];

  for i in 0..100 {
    data[i].id = i;
    data[i].value = i * 7 % 50;
    data[i].category = i % 3;
  }

  println(column_sum(data, 100));
  println(column_max(data, 100));
  println(count_category(data, 100, 0));  // count of category 0
  return 0;
}
```

## Putting It All Together

### Data Processing Pipeline

Combining arrays, maps, and structs for a complete processing workflow:

```mog
struct Sale {
  product: string,
  amount: int,
  region: string,
}

fn total_by_region(sales: [Sale]) -> {string: int} {
  totals := { "": 0 };
  totals.delete("");

  for sale in sales {
    if totals.has(sale.region) {
      totals[sale.region] = totals[sale.region] + sale.amount;
    } else {
      totals[sale.region] = sale.amount;
    }
  }
  return totals;
}

fn main() -> int {
  sales := [
    Sale { product: "widget", amount: 100, region: "east" },
    Sale { product: "gadget", amount: 250, region: "west" },
    Sale { product: "widget", amount: 150, region: "west" },
    Sale { product: "gizmo", amount: 75, region: "east" },
    Sale { product: "gadget", amount: 200, region: "east" },
  ];

  region_totals := total_by_region(sales);
  for region, total in region_totals {
    println(f"{region}: {total}");
  }
  // east: 375
  // west: 400
  return 0;
}
```

### Filter-Map-Sort Pipeline

```mog
struct Task {
  title: string,
  priority: int,
  done: bool,
}

fn main() -> int {
  tasks := [
    Task { title: "Write docs", priority: 2, done: false },
    Task { title: "Fix bug", priority: 1, done: false },
    Task { title: "Add tests", priority: 3, done: true },
    Task { title: "Review PR", priority: 1, done: false },
    Task { title: "Deploy", priority: 2, done: false },
  ];

  // Get incomplete tasks, sorted by priority
  pending := tasks.filter(fn(t: Task) -> bool { !t.done });
  pending.sort(fn(a: Task, b: Task) -> int { a.priority - b.priority });

  for t in pending {
    println(f"[P{t.priority}] {t.title}");
  }
  // [P1] Fix bug
  // [P1] Review PR
  // [P2] Write docs
  // [P2] Deploy
  return 0;
}
```

## Summary

| Collection | Create | Access | Iterate |
|---|---|---|---|
| Array | `[1, 2, 3]` or `[0; n]` | `arr[i]` | `for item in arr` |
| Map | `{ "key": value }` | `m["key"]` | `for k, v in m` |
| SoA | `soa Struct[n]` | `soa[i].field` | `for i in 0..n` |

**Array methods:** `.len()`, `.push()`, `.pop()`, `.slice()`, `.contains()`, `.reverse()`, `.filter()`, `.map()`, `.sort()`, `.join()`

**Map methods:** `.len()`, `.keys()`, `.values()`, `.has()`, `.delete()`

Arrays are the default choice. Use maps when you need key-based lookup. Use SoA when you have many elements and need to iterate over individual fields efficiently — the syntax stays familiar while the memory layout optimizes for your access pattern.
