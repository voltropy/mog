# Chapter 17: Advanced Topics

The previous chapters covered the core language — variables, functions, control flow, data structures, error handling, tensors, and embedding. This chapter collects the advanced features and design decisions that round out Mog: type aliases, scoped context blocks, memory layout optimizations, compilation backends, the interrupt system, garbage collection, and the things Mog deliberately leaves out.

## Type Aliases

Complex types get unwieldy. A function that accepts `fn(int, int) -> Result<int>` is harder to read than one that accepts `BinaryOp`. Type aliases give names to existing types without creating new ones:

```mog
type Callback = fn(int) -> int;
```

The alias is interchangeable with the original — no wrapping, no conversion:

```mog
type Callback = fn(int) -> int;

fn apply(f: Callback, x: int) -> int {
  return f(x);
}

fn double(n: int) -> int {
  return n * 2;
}

fn main() -> int {
  result := apply(double, 5);
  print(result);  // 10
  return 0;
}
```

Aliases work with any type — scalars, collections, maps, function signatures:

```mog
type Count = int;
type Score = float;
type Matrix = [[int]];
type StringMap = {string: string};
type Predicate = fn(int) -> bool;
```

Map type aliases are useful when a function accepts or returns configuration-style data:

```mog
type Config = {string: string};

fn default_config() -> Config {
  config: Config = {};
  config["mode"] = "release";
  config["backend"] = "llvm";
  return config;
}
```

Function type aliases shine when callbacks appear in multiple signatures:

```mog
type Comparator = fn(int, int) -> bool;

fn sort_by(arr: [int], cmp: Comparator) -> [int] {
  // sorting logic using cmp
  return arr;
}

fn ascending(a: int, b: int) -> bool {
  return a < b;
}

fn descending(a: int, b: int) -> bool {
  return a > b;
}
```

Aliases resolve transparently — the compiler sees through them during type checking. You cannot create a "distinct" type this way. A `Count` is an `int`, and the two are fully interchangeable.

## With Blocks

Some operations need a scoped context — a mode that activates before a block and deactivates after it, regardless of how the block exits. Mog uses `with` blocks for this:

```mog
with no_grad() {
  // gradient tracking is disabled here
  output := model_forward(input);
  print(output);
}
// gradient tracking resumes here
```

The `with` keyword takes a context expression and a block. The context is entered before the block runs and exited after it completes.

Currently, `no_grad()` is the primary context — it disables gradient tracking during ML inference when using host tensor capabilities. This avoids unnecessary memory and computation for operations that do not need backpropagation:

```mog
// Training: gradients tracked (default)
loss := compute_loss(predictions, targets);

// Inference: no gradients needed
with no_grad() {
  result := model_forward(test_input);
  print(result);
}
```

The block inside `with` is a normal scope. Variables declared inside are local to that block:

```mog
with no_grad() {
  temp := model_forward(input);
  print(temp);
}
// temp is not accessible here
```

`with` blocks can appear anywhere a statement is valid — inside functions, inside loops, nested inside other `with` blocks:

```mog
fn evaluate_batch(inputs: [tensor<f32>]) -> [tensor<f32>] {
  results: [tensor<f32>] = [];
  with no_grad() {
    for i in 0..inputs.len() {
      output := model_forward(inputs[i]);
      results.push(output);
    }
  }
  return results;
}
```

## Struct-of-Arrays (SoA) Performance

When you have thousands of structs and iterate over a single field, the default layout — an array of structs (AoS) — scatters that field's values across memory. Each struct's fields sit next to each other, so reading one field means loading every field into cache lines.

Struct-of-Arrays flips the layout. Instead of interleaving all fields per element, SoA stores each field in its own contiguous array. Iterating one field touches only that field's memory — ideal for cache performance.

Consider a particle system:

```mog
struct Particle {
  x: int,
  y: int,
  mass: int,
}
```

The default approach stores particles as an array of structs:

```mog
// Array of Structs — each element has all three fields together
particles: [Particle] = [];
particles.push(Particle { x: 0, y: 0, mass: 1 });
particles.push(Particle { x: 5, y: 3, mass: 2 });
```

The SoA approach uses the `soa` keyword to create a transposed layout:

```mog
// Struct of Arrays — each field stored in its own contiguous array
particles := soa Particle[10000];
```

Access syntax is identical — `particles[i].x` works the same way. The difference is in memory layout, not in your code:

```mog
// Initialize positions
for i in 0..10000 {
  particles[i].x = i;
  particles[i].y = i * 2;
  particles[i].mass = 1;
}

// Update all x values — contiguous memory access, cache-friendly
for i in 0..10000 {
  particles[i].x = particles[i].x + 1;
}
```

When should you use SoA? When you iterate over many elements and touch only one or two fields at a time. Physics simulations, particle systems, columnar data processing — these benefit from SoA. When you access all fields of each element together, regular arrays of structs are fine.

```mog
struct Star {
  brightness: float,
  temperature: float,
  distance: float,
  name: string,
}

// SoA: good when filtering by one field
stars := soa Star[50000];

// This loop only touches 'brightness' — contiguous reads
for i in 0..50000 {
  if stars[i].brightness > 5.0 {
    print(i);
  }
}
```

The capacity is fixed at creation. `soa Particle[10000]` allocates space for exactly 10,000 elements. This is a deliberate tradeoff — fixed size enables the compiler to lay out memory optimally and elide bounds checks in release builds.

## Compilation Backends: LLVM vs QBE

Mog compiles to native ARM64 and x86 binaries through two backends: LLVM and QBE. Both produce standalone executables — the difference is in compile speed versus runtime performance.

**LLVM** is the mature, industrial-strength backend. It applies aggressive optimizations — inlining, loop vectorization, dead code elimination, register allocation — and produces fast binaries. The cost is compile time. LLVM's optimization pipeline is large and thorough.

**QBE** is a lightweight backend — roughly 14,000 lines of C. It compiles significantly faster than LLVM but produces less optimized output. QBE focuses on correctness and simplicity over peak performance.

The practical tradeoffs:

| | LLVM | QBE |
|---|---|---|
| Compile speed | Slower | ~2x faster |
| Runtime performance | Better (at -O1) | Good, not optimized |
| Binary size | Comparable | Comparable |
| Target architectures | ARM64, x86 | ARM64, x86 |

During development, QBE's faster compile times shorten the edit-run cycle. For production or performance-sensitive scripts, LLVM at `-O1` produces better runtime results.

Both backends compile Mog through the same frontend — parsing, analysis, and type checking are identical. The divergence happens at code generation: one path emits LLVM IR, the other emits QBE IL. Both then link against the same C runtime library (which provides the garbage collector, tensor operations, async runtime, and host bindings).

## The Interrupt System

Mog scripts run inside a host application. The host needs a way to stop long-running or runaway scripts — a tight loop that never yields, an accidental infinite recursion, or simply a user pressing cancel.

Mog uses cooperative interrupt polling. The compiler inserts a check at every loop back-edge — the point where a `while`, `for`, or `for-each` loop jumps back to its condition. At each check, the script reads a volatile global flag. If the flag is set, the script exits immediately.

From the host side, two C functions control interrupts:

```c
// Request that the running script stop
void mog_request_interrupt(void);

// Arm an automatic timeout — interrupt fires after ms milliseconds
int mog_arm_timeout(int ms);
```

`mog_request_interrupt()` sets the flag directly. The script will stop at the next loop back-edge — usually within microseconds for any loop-heavy code.

`mog_arm_timeout(ms)` spawns a background thread that sleeps for the given duration, then sets the interrupt flag. This is useful for enforcing time limits on untrusted scripts:

```c
// Give the script 5 seconds
mog_arm_timeout(5000);
mog_run_script(vm, script);
mog_cancel_timeout();  // cancel if script finished early
```

The overhead of interrupt checking is small — roughly 1–3% in loop-heavy benchmarks. Each check is a single volatile load and a branch, which modern CPUs predict correctly almost every time (the flag is almost always zero).

The interrupt flag can also be cleared:

```c
void mog_clear_interrupt(void);
int mog_interrupt_requested(void);
```

This lets a host reset the flag between running multiple scripts, or check whether a script was interrupted versus completed normally.

Every loop type is covered — `while`, `for` with ranges, `for-each` over collections. Nested loops get independent checks. There is no way for a Mog script to disable or bypass interrupt polling.

## Memory Management

Mog is garbage collected. All heap allocations — structs, arrays, maps, strings, closures, tensors, SoA containers — go through the garbage collector. There is no manual `free`, no RAII, no reference counting.

The GC uses a mark-and-sweep algorithm with a shadow stack for root tracking:

1. **Allocation.** `gc_alloc` requests memory from the GC. If the allocation count exceeds a threshold, a collection cycle runs first.
2. **Root tracking.** Each function call pushes a GC frame (`gc_push_frame`). Local variables that hold heap pointers are registered as roots in the current frame (`gc_add_root`). When the function returns, the frame is popped (`gc_pop_frame`).
3. **Collection.** The collector walks the shadow stack, marks all reachable objects, then sweeps unmarked objects and frees their memory. The allocation threshold grows after each cycle to avoid excessive collection.

This is an implementation detail you rarely need to think about. You allocate by creating values — the language handles the rest:

```mog
fn make_particles(n: int) -> [Particle] {
  particles: [Particle] = [];
  for i in 0..n {
    // Each Particle is GC-allocated; no manual cleanup needed
    particles.push(Particle { x: i, y: i * 2, mass: 1 });
  }
  return particles;
}
// When particles is no longer reachable, the GC reclaims the memory
```

Closures capture variables by value (see Chapter 7), and the captured environment is itself a GC-allocated block:

```mog
fn make_counter(start: int) -> fn() -> int {
  count := start;
  return fn() -> int {
    count = count + 1;
    return count;
  };
}
// The closure's captured 'count' lives on the GC heap
```

There are no weak references, no finalizers, and no way to manually trigger collection from Mog code. The GC is non-generational and non-concurrent — it stops the script during collection. For the short-lived scripts Mog targets, this is a reasonable tradeoff: simplicity and correctness over pause-time optimization.

## What Mog Does NOT Have

Mog's design is subtractive. Every feature must justify its complexity, and many common language features did not make the cut. This is intentional — a smaller language is easier to learn, easier to embed, and easier to reason about.

**No generics** (beyond `tensor<dtype>`, `Result<T>`, and `?T`). Mog has a small, fixed set of parameterized types built into the language. You cannot define your own generic structs or functions. This eliminates an entire class of complexity — no type parameter inference, no trait bounds, no monomorphization. If you need a collection of a specific type, you use the built-in array, map, or struct types directly.

**No classes, inheritance, interfaces, or traits.** Mog has structs with fields and standalone functions. There is no method dispatch, no vtables, no subtyping hierarchy. If you need polymorphism, use closures — a `fn(int) -> int` doesn't care which function it points to.

**No operator overloading.** `+` means numeric addition or string concatenation. It cannot be redefined for custom types. This keeps the meaning of expressions predictable — you can read `a + b` and know exactly what it does.

**No macros or metaprogramming.** No compile-time code generation, no syntax extensions, no preprocessor. The language you read is the language that runs. This makes Mog code uniformly readable — there are no project-specific DSLs hiding behind macro expansions.

**No exceptions.** Error handling uses `Result<T>` and the `?` propagation operator (see Chapter 11). Errors are values, not control flow. Every function that can fail says so in its return type, and the compiler enforces that you handle the error or propagate it.

**No null.** Mog uses `?T` (Optional) for values that might be absent — `?int`, `?string` (see Chapter 11). The type system distinguishes between "definitely has a value" and "might not have a value." You cannot accidentally dereference something that does not exist.

**No raw pointers or manual memory management.** All memory is GC-managed. You cannot take the address of a variable, cast between pointer types, or free memory. This eliminates use-after-free, double-free, buffer overflows, and dangling pointers by construction.

**No implicit type coercion.** An `int` does not silently become a `float`. A `float` does not silently become a `string`. All conversions are explicit function calls — `float_from_string`, `int(x)`, and so on. This prevents an entire category of subtle bugs where silent coercion produces unexpected results.

**No standalone execution.** Mog scripts run inside a host application. There is no `mog run file.mog` command that produces a self-contained process with filesystem access, network sockets, or an event loop. The host provides capabilities, and the script declares which ones it needs (see Chapter 14). This is the core security model — a Mog script can only do what its host explicitly permits.

These omissions are not gaps to be filled in future versions. They are design decisions that keep Mog small, predictable, and safe for embedding. A language that tries to be everything ends up being harder to trust. Mog trades breadth for clarity.
