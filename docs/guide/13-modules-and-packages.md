# Chapter 13: Modules and Packages

As Mog programs grow beyond a single file, you need a way to split code into logical units, control what's visible to the outside, and compose libraries. Mog uses a Go-style module system: packages group related code, `pub` controls visibility, and `import` brings packages into scope.

There are no header files, no include guards, no complex build configurations. A package is a directory. A module is a project. The compiler resolves everything from the file system.

## Package Declaration

Every Mog file begins with a `package` declaration. It names the package the file belongs to:

```mog
package math;
```

All `.mog` files in the same directory must declare the same package name. The package name becomes the namespace used by importers:

```mog
// geometry/shapes.mog
package geometry;

pub fn circle_area(radius: float) -> float {
  return PI * (radius ** 2.0);
}
```

```mog
// geometry/volumes.mog
package geometry;

pub fn sphere_volume(radius: float) -> float {
  return (4.0 / 3.0) * PI * (radius ** 3.0);
}
```

Both files belong to `package geometry`. They share the same namespace — functions in one file can call private functions in the other, because they're in the same package.

The `package main` package is special. It must contain a `fn main()` entry point. Every executable Mog program has exactly one `package main`:

```mog
package main;

fn main() -> int {
  print("hello");
  return 0;
}
```

Files without a `package` declaration are implicitly `package main`. This is single-file mode — convenient for scripts and quick experiments.

## Public vs Private — The `pub` Keyword

Symbols are package-private by default. Only symbols marked with `pub` are visible to importers:

```mog
package mathutil;

// Public — importers can call this
pub fn gcd(a: int, b: int) -> int {
  while b != 0 {
    temp := b;
    b = a % b;
    a = temp;
  }
  return a;
}

// Public — importers can call this
pub fn lcm(a: int, b: int) -> int {
  return (a * b) / gcd(a, b);
}

// Private — only visible within package mathutil
fn validate_positive(n: int) -> bool {
  return n > 0;
}
```

`pub` works on functions, async functions, structs, and type aliases:

```mog
package models;

pub struct User {
  name: string,
  email: string,
  age: int,
}

pub type UserList = [User];

pub fn create_user(name: string, email: string, age: int) -> User {
  return User { name: name, email: email, age: age };
}

pub async fn fetch_user(id: int) -> Result<User> {
  data := await http.get("http://api.example.com/users/{id}")?;
  return ok(parse_user(data));
}

// Private helper — not exported
fn parse_user(data: string) -> User {
  return User { name: "parsed", email: "parsed", age: 0 };
}
```

Struct fields are always accessible when the struct itself is `pub`. There is no field-level visibility — if you export the struct, you export all its fields:

```mog
package config;

// Exported struct — all fields accessible to importers
pub struct Settings {
  host: string,
  port: int,
  debug: bool,
}

// Private struct — importers cannot see this at all
struct InternalState {
  cache: [string],
  dirty: bool,
}
```

> **Tip:** If you need to hide a struct's internals, keep the struct private and export functions that construct and inspect it. This is the idiomatic way to build opaque types in Mog.

## Importing Packages

Use `import` to bring a package into scope. Access its public symbols with qualified names — `package.symbol`:

```mog
package main;

import mathutil;

fn main() -> int {
  result := mathutil.gcd(48, 18);
  print("GCD: {result}");  // GCD: 6
  return 0;
}
```

The import name matches the package name. After importing, every public function, struct, and type from that package is available through the qualified prefix:

```mog
package main;

import models;

fn main() -> int {
  user := models.create_user("alice", "alice@example.com", 30);
  print(user.name);   // alice
  print(user.email);  // alice@example.com
  return 0;
}
```

Importing a package does not import its symbols into the local namespace. You always use the qualified form. This keeps names unambiguous when multiple packages are in play:

```mog
package main;

import math;
import physics;

fn main() -> int {
  // Clear which 'distance' function you mean
  d1 := math.distance(0.0, 0.0, 3.0, 4.0);
  d2 := physics.distance(10.0, 9.8, 2.0);
  print("math: {d1}, physics: {d2}");
  return 0;
}
```

## Multi-Import Syntax

When importing several packages, use the grouped form:

```mog
package main;

import (math, utils, models, config);

fn main() -> int {
  settings := config.load_defaults();
  data := utils.read_input("data.txt");
  result := math.dot_product(data, data);
  print(result);
  return 0;
}
```

This is equivalent to writing four separate `import` statements. The grouped form is preferred when a file has three or more imports:

```mog
package main;

// Equivalent — but the grouped form is cleaner
import math;
import utils;
import models;
import config;
```

## Qualified Access

All access to imported symbols uses the `package.name` form. This applies to functions, structs, and type aliases:

```mog
package main;

import geometry;

fn main() -> int {
  // Function call
  area := geometry.circle_area(5.0);

  // Struct construction
  p := geometry.Point { x: 10.0, y: 20.0 };

  // Struct field access
  print("area: {area}, x: {p.x}");
  return 0;
}
```

Qualified access also works in type annotations:

```mog
package main;

import models;

fn process_users(users: [models.User]) -> [string] {
  names: [string] = [];
  for user in users {
    names.push(user.name);
  }
  return names;
}

fn main() -> int {
  users := [
    models.create_user("alice", "a@x.com", 30),
    models.create_user("bob", "b@x.com", 25),
  ];
  names := process_users(users);
  for name in names {
    print(name);
  }
  return 0;
}
```

## The Module File

Every Mog project has a `mog.mod` file at its root. It declares the module path — the name that identifies the entire project:

```
module myapp
```

That's it. No version numbers, no dependency lists — just the module name. The module file tells the compiler where the project root is, and the directory structure below it defines the packages.

A typical project layout:

```
myapp/
  mog.mod            // module myapp
  main.mog           // package main — entry point
  math/
    math.mog         // package math
    trig.mog         // package math (same package, same dir)
  models/
    user.mog         // package models
    post.mog         // package models
  utils/
    strings.mog      // package utils
```

The compiler resolves `import math` by looking for a `math/` directory relative to the module root. Every `.mog` file in that directory is part of the `math` package.

> **Note:** The directory name and package name must match. A file in `math/` that declares `package geometry` is a compile error.

## Circular Import Detection

Mog does not allow circular imports. If package A imports package B and package B imports package A, the compiler rejects the program with an error:

```mog
// a/a.mog
package a;
import b;           // a depends on b

pub fn from_a() -> int {
  return b.from_b() + 1;
}
```

```mog
// b/b.mog
package b;
import a;           // b depends on a — COMPILE ERROR: circular import

pub fn from_b() -> int {
  return a.from_a() + 1;
}
```

The fix is to extract shared code into a third package that both A and B can import:

```mog
// shared/shared.mog
package shared;

pub fn base_value() -> int {
  return 42;
}
```

```mog
// a/a.mog
package a;
import shared;

pub fn from_a() -> int {
  return shared.base_value() + 1;
}
```

```mog
// b/b.mog
package b;
import shared;

pub fn from_b() -> int {
  return shared.base_value() + 2;
}
```

> **Warning:** Circular dependencies are always a compile error — there is no way to forward-declare or defer imports. If you hit this, it usually means two packages are too tightly coupled and should share a common dependency.

## Practical Example: Splitting a Program into Packages

Here is a small project that fetches user data, processes it, and writes output — split across three packages.

**Project structure:**

```
userapp/
  mog.mod
  main.mog
  api/
    api.mog
  transform/
    transform.mog
```

**mog.mod:**

```
module userapp
```

**api/api.mog** — handles network communication:

```mog
package api;

requires http;

pub struct UserData {
  name: string,
  score: int,
}

pub async fn fetch_user(id: int) -> Result<UserData> {
  body := await http.get("http://api.example.com/users/{id}")?;
  return ok(parse(body));
}

pub async fn fetch_users(ids: [int]) -> Result<[UserData]> {
  futures := ids.map(fn(id) { fetch_user(id) });
  results := await all(futures)?;
  return ok(results);
}

fn parse(body: string) -> UserData {
  return UserData { name: body, score: 0 };
}
```

**transform/transform.mog** — pure data transformations:

```mog
package transform;

import api;

pub fn top_scorers(users: [api.UserData], threshold: int) -> [api.UserData] {
  result: [api.UserData] = [];
  for user in users {
    if user.score >= threshold {
      result.push(user);
    }
  }
  return result;
}

pub fn format_report(users: [api.UserData]) -> string {
  report := "User Report\n";
  report = report + "===========\n";
  for i, user in users {
    report = report + "{i + 1}. {user.name} (score: {user.score})\n";
  }
  return report;
}
```

**main.mog** — entry point that wires everything together:

```mog
package main;

import (api, transform);

requires fs;

async fn main() -> int {
  ids := [1, 2, 3, 4, 5];

  match await api.fetch_users(ids) {
    ok(users) => {
      top := transform.top_scorers(users, 80);
      report := transform.format_report(top);
      print(report);
      await fs.write_file("report.txt", report)?;
    },
    err(msg) => print("failed to fetch users: {msg}"),
  }

  return 0;
}
```

Each package has a single responsibility. The `api` package handles network calls and data parsing. The `transform` package contains pure functions that process data. The `main` package wires them together. Private functions like `parse` in the `api` package stay hidden — importers see only what's marked `pub`.

## Summary

Mog's module system keeps things flat and explicit. There are no nested modules, no re-exports, no renaming on import. A package is a directory, `pub` controls visibility, and qualified access eliminates naming conflicts.

Key points:
- **Declare** packages with `package name;` at the top of every file
- **Export** symbols with `pub` — everything else is package-private
- **Import** with `import name;` or `import (a, b, c);` for groups
- **Access** imported symbols with `package.name` — always qualified, never bare
- **Structure** projects with one directory per package and `mog.mod` at the root
- **Avoid** circular imports — extract shared code into a common package

Capabilities (Chapter 14) use the same dot-syntax as package access — `fs.read_file()` looks like a module call but is backed by host-provided functions rather than Mog source code. The next chapter explains how that works.
