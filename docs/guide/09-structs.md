# Chapter 9: Structs

Structs are Mog's way of grouping related data under a single name. They are simple named product types with typed fields — no methods, no inheritance, no interfaces. You define the shape, construct instances, and pass them around. Functions that operate on structs live outside the struct as standalone functions.

## Declaring Structs

A struct declaration lists named fields with their types, separated by commas:

```mog
struct Point {
  x: int,
  y: int,
}
```

Fields can be any type — scalars, strings, arrays, maps, or other structs:

```mog
struct Color {
  r: int,
  g: int,
  b: int,
  a: float,
}

struct Config {
  name: string,
  version: int,
  debug: bool,
  tags: [string],
}

struct User {
  id: int,
  username: string,
  email: string,
  active: bool,
}
```

Structs are always declared at the top level. They cannot be declared inside functions or other structs.

## Constructing Instances

Create a struct instance by naming the type and providing values for all fields inside braces:

```mog
fn main() {
  p := Point { x: 10, y: 20 };
  c := Color { r: 255, g: 128, b: 0, a: 1.0 };
  cfg := Config { name: "myapp", version: 3, debug: false, tags: ["prod", "v3"] };
}
```

Every field must be provided. There are no default values and no partial construction — if a struct has four fields, you supply four values:

```mog
fn main() {
  // This is a compile error — missing field `a`:
  // c := Color { r: 255, g: 128, b: 0 };

  // All fields required:
  c := Color { r: 255, g: 128, b: 0, a: 1.0 };
}
```

You can use expressions as field values, not just literals:

```mog
fn main() {
  base := 100;
  p := Point { x: base * 2, y: base + 50 };
  print(p.x);  // 200
  print(p.y);  // 150
}
```

## Field Access

Access individual fields with dot notation:

```mog
fn main() {
  p := Point { x: 10, y: 20 };
  print(p.x);  // 10
  print(p.y);  // 20

  c := Color { r: 255, g: 128, b: 0, a: 1.0 };
  print(c.r);  // 255
  print(c.a);  // 1.0
}
```

Fields work anywhere an expression of that type is expected:

```mog
fn main() {
  p := Point { x: 3, y: 4 };
  distance_squared := p.x * p.x + p.y * p.y;
  print(distance_squared);  // 25
}
```

## Field Mutation

Struct fields are mutable. Assign to them with `=`:

```mog
fn main() {
  p := Point { x: 10, y: 20 };
  print(p.x);  // 10

  p.x = 30;
  print(p.x);  // 30

  p.y = p.y + 5;
  print(p.y);  // 25
}
```

You can mutate any field at any time:

```mog
fn main() {
  user := User { id: 1, username: "alice", email: "alice@example.com", active: true };
  print(user.active);  // true

  user.active = false;
  user.email = "alice@newdomain.com";
  print(user.active);  // false
  print(user.email);   // alice@newdomain.com
}
```

## Passing Structs to Functions

Structs are heap-allocated and passed by reference. When you pass a struct to a function, the function receives a pointer to the same data. Modifications inside the function affect the original:

```mog
fn move_right(p: Point, amount: int) {
  p.x = p.x + amount;
}

fn main() {
  p := Point { x: 0, y: 0 };
  move_right(p, 10);
  print(p.x);  // 10 — the original was modified
}
```

> This is different from closures, which capture variables by value (see Chapter 7). Structs are always passed by reference — there is no copy-on-pass.

Functions can read struct fields without modifying them:

```mog
struct Rect {
  width: int,
  height: int,
}

fn area(r: Rect) -> int {
  return r.width * r.height;
}

fn main() {
  r := Rect { width: 10, height: 5 };
  print(area(r));  // 50
}
```

Returning structs from functions works naturally — you return a reference to a heap-allocated struct:

```mog
fn make_point(x: int, y: int) -> Point {
  return Point { x: x, y: y };
}

fn main() {
  p := make_point(3, 7);
  print(p.x);  // 3
  print(p.y);  // 7
}
```

## No Methods — Use Standalone Functions

Mog structs have no methods. Instead, write standalone functions that take the struct as a parameter. This keeps data and behavior separate:

```mog
struct Vec2 {
  x: int,
  y: int,
}

fn vec2_add(a: Vec2, b: Vec2) -> Vec2 {
  return Vec2 { x: a.x + b.x, y: a.y + b.y };
}

fn vec2_dot(a: Vec2, b: Vec2) -> int {
  return a.x * b.x + a.y * b.y;
}

fn vec2_to_string(v: Vec2) -> string {
  return "({v.x}, {v.y})";
}

fn main() {
  a := Vec2 { x: 1, y: 2 };
  b := Vec2 { x: 3, y: 4 };

  sum := vec2_add(a, b);
  print(vec2_to_string(sum));  // (4, 6)
  print(vec2_dot(a, b));       // 11
}
```

> A common convention is to prefix function names with the struct name: `point_distance`, `color_mix`, `user_validate`. This makes it clear which type the function operates on.

## Constructor Functions

Since there are no constructors or default values, a common pattern is to write factory functions that return pre-configured struct instances:

```mog
fn new_user(name: string, email: string) -> User {
  return User { id: 0, username: name, email: email, active: true };
}

fn main() {
  u := new_user("alice", "alice@example.com");
  print(u.username);  // alice
  print(u.active);    // true
}
```

```mog
struct DatabaseConfig {
  host: string,
  port: int,
  name: string,
}

fn default_db_config() -> DatabaseConfig {
  return DatabaseConfig { host: "localhost", port: 5432, name: "appdb" };
}

fn main() {
  cfg := default_db_config();
  cfg.name = "testdb";
  print(cfg.host);  // localhost
  print(cfg.name);  // testdb
}
```

This pattern gives you the flexibility of default values while keeping construction explicit. See Chapter 7 for how closures can create factory functions that return configured behavior.

## Nested Structs

Structs can contain other structs as fields:

```mog
struct Address {
  street: string,
  city: string,
  zip: string,
}

struct Person {
  name: string,
  age: int,
  address: Address,
}

fn main() {
  p := Person {
    name: "Alice",
    age: 30,
    address: Address { street: "123 Main St", city: "Portland", zip: "97201" },
  };

  print(p.name);            // Alice
  print(p.address.city);    // Portland
  print(p.address.zip);     // 97201
}
```

Mutation works through nested field access:

```mog
fn main() {
  p := Person {
    name: "Bob",
    age: 25,
    address: Address { street: "456 Oak Ave", city: "Seattle", zip: "98101" },
  };

  p.address.city = "Tacoma";
  p.age = 26;
  print(p.address.city);  // Tacoma
}
```

Since structs are passed by reference, modifying a nested struct through a function affects the original all the way up:

```mog
fn relocate(person: Person, new_city: string) {
  person.address.city = new_city;
}

fn main() {
  p := Person {
    name: "Dana",
    age: 35,
    address: Address { street: "100 Elm St", city: "Austin", zip: "73301" },
  };

  relocate(p, "Houston");
  print(p.address.city);  // Houston
}
```

## Structs with Arrays and Maps

Struct fields can hold arrays and maps, enabling rich data models:

```mog
struct StudentRecord {
  name: string,
  grades: [int],
}

fn average_grade(s: StudentRecord) -> float {
  sum := 0;
  for g in s.grades {
    sum = sum + g;
  }
  return sum as float / s.grades.len as float;
}

fn main() {
  student := StudentRecord { name: "Eve", grades: [88, 92, 75, 96] };
  print(average_grade(student));  // 87.75

  student.grades.push(100);
  print(student.grades.len);     // 5
}
```

```mog
struct Inventory {
  items: map[string]int,
}

fn add_item(inv: Inventory, name: string, qty: int) {
  if inv.items.has(name) {
    inv.items[name] = inv.items[name] + qty;
  } else {
    inv.items[name] = qty;
  }
}

fn main() {
  inv := Inventory { items: {} };
  add_item(inv, "apples", 5);
  add_item(inv, "bananas", 3);
  add_item(inv, "apples", 2);
  print(inv.items["apples"]);  // 7
}
```

See Chapter 10 for the full set of array and map operations.

## Practical Examples

### RGB Color Manipulation

```mog
struct Color {
  r: int,
  g: int,
  b: int,
}

fn clamp(val: int, lo: int, hi: int) -> int {
  if val < lo { return lo; }
  if val > hi { return hi; }
  return val;
}

fn brighten(c: Color, amount: int) {
  c.r = clamp(c.r + amount, 0, 255);
  c.g = clamp(c.g + amount, 0, 255);
  c.b = clamp(c.b + amount, 0, 255);
}

fn mix(a: Color, b: Color) -> Color {
  return Color {
    r: (a.r + b.r) / 2,
    g: (a.g + b.g) / 2,
    b: (a.b + b.b) / 2,
  };
}

fn color_to_string(c: Color) -> string {
  return "rgb({c.r}, {c.g}, {c.b})";
}

fn main() {
  red := Color { r: 200, g: 50, b: 50 };
  blue := Color { r: 50, g: 50, b: 200 };

  brighten(red, 40);
  print(color_to_string(red));    // rgb(240, 90, 90)

  purple := mix(red, blue);
  print(color_to_string(purple)); // rgb(145, 70, 145)
}
```

### Tree Structure

Structs that contain arrays of the same type enable tree-like patterns:

```mog
struct TreeNode {
  value: int,
  children: [TreeNode],
}

fn sum_tree(node: TreeNode) -> int {
  total := node.value;
  for child in node.children {
    total = total + sum_tree(child);
  }
  return total;
}

fn main() {
  tree := TreeNode {
    value: 1,
    children: [
      TreeNode { value: 2, children: [] },
      TreeNode {
        value: 3,
        children: [
          TreeNode { value: 4, children: [] },
          TreeNode { value: 5, children: [] },
        ],
      },
    ],
  };

  print(sum_tree(tree));  // 15
}
```

## Summary

| Concept | Syntax |
|---|---|
| Declare a struct | `struct Name { field: type, ... }` |
| Construct an instance | `Name { field: value, ... }` |
| Read a field | `instance.field` |
| Mutate a field | `instance.field = value;` |
| Nested field access | `instance.field.subfield` |

Structs are heap-allocated and passed by reference. There are no methods — use standalone functions that take the struct as a parameter. Keep structs simple: they hold data, functions provide behavior.
