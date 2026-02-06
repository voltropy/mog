import { describe, test, expect } from "bun:test"
import { compile } from "./compiler"
import { compileRuntime, linkToExecutable } from "./linker"
import { execSync } from "child_process"
import { existsSync, unlinkSync } from "fs"
import { tmpdir } from "os"
import { join } from "path"

// Helper to compile and run AlgolScript code
async function compileAndRun(source: string, expectedExitCode: number = 0): Promise<string> {
  const result = await compile(source)
  if (result.errors.length > 0) {
    throw new Error(`Compilation errors: ${result.errors.map(e => e.message).join(", ")}`)
  }

  const runtimeLib = await compileRuntime()
  const tempDir = tmpdir()
  const baseName = `aos_soa_test_${Date.now()}_${Math.random().toString(36).slice(2)}`
  const exePath = join(tempDir, baseName)

  try {
    await linkToExecutable(result.llvmIR, exePath, runtimeLib)
    
    const output = execSync(exePath, { encoding: "utf-8", timeout: 5000 })
    return output
  } finally {
    if (existsSync(exePath)) unlinkSync(exePath)
  }
}

describe("AoS (Array of Structs) integration", () => {
  test("AoS creation and field access", async () => {
    const source = `
struct Point { x: f64, y: f64 }

fn main() -> i64 {
  p1: Point = Point { x: 1.0, y: 2.0 };
  p2: Point = Point { x: 3.0, y: 4.0 };
  
  print(p1.x);
  println();
  print(p1.y);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("1")
    expect(output).toContain("2")
  })

  test("AoS element assignment", async () => {
    const source = `
struct Point { x: f64, y: f64 }

fn main() -> i64 {
  p: Point = Point { x: 0.0, y: 0.0 };
  
  p.x := 5.0;
  p.y := 10.0;
  
  print(p.x);
  println();
  print(p.y);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("5")
    expect(output).toContain("10")
  })

  test("AoS with multiple field types", async () => {
    const source = `
struct Person {
  name: ptr,
  age: i64,
  height: f64
}

fn main() -> i64 {
  person: Person = Person { name: "Alice", age: 30, height: 1.75 };
  
  print(person.age);
  println();
  print(person.height);
  println();
  
  person.age := 31;
  print(person.age);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("30")
    expect(output).toContain("31")
  })

  test.skip("AoS as function parameter and return", async () => {
    const source = `
struct Point { x: f64, y: f64 }

fn create_point(x: f64, y: f64) -> Point {
  return Point { x: x, y: y };
}

fn move_point(p: Point, dx: f64, dy: f64) -> Point {
  p.x := p.x + dx;
  p.y := p.y + dy;
  return p;
}

fn main() -> i64 {
  p: Point = create_point(1.0, 2.0);
  p := move_point(p, 5.0, 10.0);
  
  print(p.x);
  println();
  print(p.y);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("6")
    expect(output).toContain("12")
  })

  test.skip("AoS array operations", async () => {
    const source = `
struct Point { x: f64, y: f64 }

fn main() -> i64 {
  p1: Point = Point { x: 1.0, y: 2.0 };
  p2: Point = Point { x: 3.0, y: 4.0 };
  
  print(p1.x + p2.x);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("4")
  })
})

describe("SoA (Struct of Arrays) integration", () => {
  test("SoA basic operations", async () => {
    const source = `
soa Particles {
  x: [f64],
  y: [f64]
}

fn main() -> i64 {
  particles: Particles = Particles { x: [1.0, 2.0, 3.0], y: [4.0, 5.0, 6.0] };
  
  print(particles.x[0]);
  println();
  print(particles.y[1]);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("1")
    expect(output).toContain("5")
  })

  test.skip("SoA field assignment", async () => {
    const source = `
soa Particles {
  x: [f64],
  y: [f64]
}

fn main() -> i64 {
  particles: Particles = Particles { x: [1.0, 2.0], y: [3.0, 4.0] };
  
  particles.x[0] := 10.0;
  particles.y[1] := 20.0;
  
  print(particles.x[0]);
  println();
  print(particles.y[1]);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("10")
    expect(output).toContain("20")
  })

  test("SoA with integer fields", async () => {
    const source = `
soa Indices {
  id: [i64],
  value: [i64]
}

fn main() -> i64 {
  indices: Indices = Indices { id: [1, 2, 3], value: [100, 200, 300] };
  
  print(indices.id[1]);
  println();
  print(indices.value[2]);
  println();
  
  indices.value[0] := 999;
  print(indices.value[0]);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("2")
    expect(output).toContain("300")
    expect(output).toContain("999")
  })

  test("SoA mixed field types", async () => {
    const source = `
soa Data {
  flags: [i64],
  values: [f64]
}

fn main() -> i64 {
  data: Data = Data { flags: [1, 0, 1], values: [1.5, 2.5, 3.5] };
  
  print(data.flags[0]);
  println();
  print(data.values[1]);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("1")
    expect(output).toContain("2")
  })
})

describe("AoS/SoA edge cases", () => {
  test.skip("nested struct access", async () => {
    const source = `
struct Inner { value: i64 }
struct Outer { inner: ptr }

fn main() -> i64 {
  inner: Inner = Inner { value: 42 };
  outer: Outer = Outer { inner: &inner };
  
  print(inner.value);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("42")
  })

  test("struct with string field", async () => {
    const source = `
struct Message {
  content: ptr,
  priority: i64
}

fn main() -> i64 {
  msg: Message = Message { content: "Hello", priority: 1 };
  
  print(msg.priority);
  println();
  print_string(msg.content);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("1")
    expect(output).toContain("Hello")
  })

  test("empty struct initialization", async () => {
    const source = `
struct Empty {}

fn main() -> i64 {
  e: Empty = Empty {};
  print(0);
  println();
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("0")
  })
})
