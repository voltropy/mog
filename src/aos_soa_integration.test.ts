import { describe, test, expect } from "bun:test"
import { compile } from "./compiler"
import { compileRuntime, linkToExecutable } from "./linker"
import { execSync } from "child_process"
import { existsSync, unlinkSync } from "fs"
import { tmpdir } from "os"
import { join } from "path"

// Helper to compile and run Mog code
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

  test("AoS as function parameter and return", async () => {
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

describe("SoA (Struct of Arrays) integration - new design", () => {
  test("SoA basic i64 read/write", async () => {
    const source = `
struct Datum { id: i64, val: i64 }

fn main() -> i64 {
  soa datums: Datum[10]
  datums[0].id = 1;
  datums[0].val = 100;
  datums[1].id = 2;
  datums[1].val = 200;
  
  print(datums[0].id);
  println();
  print(datums[1].val);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("1")
    expect(output).toContain("200")
  })

  test("SoA field assignment and read-back", async () => {
    const source = `
struct Point { x: i64, y: i64 }

fn main() -> i64 {
  soa points: Point[5]
  points[0].x = 10;
  points[0].y = 20;
  points[1].x = 30;
  points[1].y = 40;
  
  print(points[0].x);
  println();
  print(points[0].y);
  println();
  print(points[1].x);
  println();
  print(points[1].y);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("10")
    expect(output).toContain("20")
    expect(output).toContain("30")
    expect(output).toContain("40")
  })

  test("SoA with f64 fields", async () => {
    const source = `
struct Particle { x: f64, y: f64 }

fn main() -> i64 {
  soa particles: Particle[10]
  particles[0].x = 1.5;
  particles[0].y = 2.5;
  particles[1].x = 3.5;
  particles[1].y = 4.5;
  
  print(particles[0].x);
  println();
  print(particles[1].y);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("1.5")
    expect(output).toContain("4.5")
  })

  test("SoA mixed field types (i64 and f64)", async () => {
    const source = `
struct Data { flag: i64, value: f64 }

fn main() -> i64 {
  soa data: Data[5]
  data[0].flag = 1;
  data[0].value = 3.14;
  data[1].flag = 0;
  data[1].value = 2.72;
  
  print(data[0].flag);
  println();
  print(data[0].value);
  println();
  
  return 0;
}
`
    const output = await compileAndRun(source)
    expect(output).toContain("1")
    expect(output).toContain("3.14")
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
