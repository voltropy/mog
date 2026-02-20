# Mog

A small, statically-typed, embeddable language for ML workflows and LLM agent scripting. Compiles to native code via LLVM. Think of it as a statically-typed Lua with tensors, async I/O, and a capability model where the host provides all I/O and ML operations.

## Why Mog

Most ML work today lives in Python scripts that are difficult to sandbox, expensive to deploy, and opaque to the LLM agents that increasingly need to write and execute code. Mog exists to give those agents — and the humans supervising them — a language that is small enough to fit entirely in a model's context window, safe enough to run untrusted code without fear, and expressive enough to do real ML work without dropping into a general-purpose language.

Mog is not a standalone systems language. It is always embedded inside a host application. The host decides what the script can do: read a file, call an API, run inference on a model. The script declares what it needs (`requires http, model`) and the host either grants those capabilities or refuses to run it. There is no way for a Mog script to escape its sandbox, access the filesystem behind the host's back, or crash the process. This makes it suitable for running agent-generated code in production, where the alternative is either no code execution at all or an elaborate container-based sandbox around a general-purpose language.

The language compiles to native code through LLVM, so tight numerical loops run at C speed rather than interpreter speed. Tensors are a built-in data structure — n-dimensional arrays with hardware-relevant dtypes like `f16` and `bf16`, element read/write, and shape manipulation. All ML operations (matmul, activations, loss functions, autograd) are provided by the host through the same capability system as I/O. The language gives you the data structure; the host gives you the compute. This means the host can route tensor operations to whatever backend makes sense — CPU, GPU, or a remote accelerator — without the language needing to know.

## Use Cases

- LLM agent tool-use scripts
- ML training and inference workflows
- Plugin/extension scripting for host applications
- Short automation scripts

Mog is explicitly **not** for systems programming, standalone applications, or replacing general-purpose languages. It has no raw pointers, no manual memory management, no threads, no POSIX syscalls, no inheritance, no macros, and no generics beyond tensor dtype parameterization. Each of these omissions is deliberate — they keep the surface area small and the security model tractable.

## Design Philosophy

1. **Small surface area** — the entire language should fit in an LLM's context window. Every feature must justify its existence. When in doubt, leave it out.
2. **Predictable semantics** — no implicit coercion surprises, no operator precedence puzzles, no hidden control flow. Code reads top-to-bottom, left-to-right.
3. **Familiar syntax** — curly braces, `fn`, `->`, `:=`. A blend of Rust, Go, and TypeScript that LLMs already generate fluently. No novel syntax without strong justification.
4. **Safe by default** — garbage collected, bounds-checked, no null, no raw pointers. The language cannot crash the host or escape its sandbox.
5. **Host provides I/O** — the language has no built-in file, network, or system access. All side effects go through capabilities explicitly granted by the embedding host.
6. **Tensors as data, ML as capability** — the language provides n-dimensional arrays with hardware-relevant dtypes (f16, bf16, f32, f64) and element read/write. All ML operations (matmul, activations, autograd) are provided by the host via capabilities, not built into the language.

## Installation

```bash
bun install
```

## Usage

```bash
# Compile a Mog program
bun run src/index.ts program.mog

# Run the executable
./program
```

## Language Overview

### Hello World

```mog
fn main() {
  print("hello, world");
}
```

### Variables

`:=` for initial binding, `=` for reassignment. Type inference or explicit annotation:

```mog
x := 42;             // inferred as int
name: string = "hi"; // explicit type
x = x + 1;           // reassignment
```

### Types

| Type | Description |
|------|-------------|
| `int` | 64-bit signed integer (default) |
| `float` | 64-bit floating point (default) |
| `bool` | `true` or `false` |
| `string` | UTF-8, immutable, GC-managed |

**Numeric precision types** (primarily for tensor element types):

- Integers: `i8`, `i16`, `i32`, `i64`
- Unsigned: `u8`, `u16`, `u32`, `u64`
- Floating point: `f16`, `bf16`, `f32`, `f64`

Widening conversions are implicit (`i32` → `int`, `f32` → `float`). Narrowing requires explicit `as` cast.

### Functions

```mog
fn add(a: int, b: int) -> int {
  return a + b;
}

// Single-expression body
fn double(x: int) -> int { x * 2 }

// No return value
fn greet(name: string) {
  print("hello " + name);
}
```

**Closures** — functions are first-class values:

```mog
fn make_adder(n: int) -> fn(int) -> int {
  return fn(x: int) -> int { x + n };
}
```

**Named arguments with defaults:**

```mog
fn train(model: Model, data: tensor<f32>, epochs: int = 10, lr: float = 0.001) -> Model {
  // ...
}
trained := train(model, data, epochs: 50, lr: 0.0001);
```

### Control Flow

```mog
// If/else (also works as expression)
sign := if x > 0 { 1 } else if x < 0 { -1 } else { 0 };

// While
while condition {
  // ...
}

// For loop — ranges, arrays, maps
for i in 0..10 { ... }
for item in items { ... }
for i, item in items { ... }
for key, value in config { ... }

// Break and continue
while true {
  if done { break; }
  if skip { continue; }
}
```

### Composite Types

**Arrays** — dynamically-sized, homogeneous, GC-managed:

```mog
scores := [95.5, 88.0, 72.3];
scores.push(91.0);
first := scores[0];
slice := scores[1:3];
length := scores.len;
```

**Maps** — key-value dictionaries:

```mog
ages := {"alice": 30, "bob": 25};
ages["charlie"] = 28;
if ages.has("alice") { ... }
for key, value in ages { ... }
```

**Structs** — named product types, no methods, no inheritance:

```mog
struct Point { x: float, y: float }

p := Point { x: 1.0, y: 2.0 };
p.x = 3.0;
```

**SoA (Struct of Arrays)** — AoS interface over columnar storage:

```mog
struct Datum { id: i64, val: i64 }
datums := soa Datum[100];

datums[0].id = 1;       // writes to contiguous id array
datums[0].val = 100;    // writes to contiguous val array
print(datums[0].id);    // 1
```

You write natural per-element code (`datums[i].field`) but the compiler stores each
field in its own contiguous array — cache-friendly for column iteration in ML and
game-engine workloads.

**Optional type** — no null, use `?T`:

```mog
fn find(items: [string], target: string) -> ?int {
  for i, item in items {
    if item == target { return some(i); }
  }
  return none;
}

result := find(names, "alice");
if result is some(idx) {
  print("found at {idx}");
}
```

### Tensors

N-dimensional arrays with fixed element dtype. The language provides tensors as a data structure with creation, shape manipulation, and element read/write. All ML operations (matmul, activations, loss functions, autograd) are provided by the host through the `ml` capability.

```mog
// Creation
t := tensor([1.0, 2.0, 3.0]);
m := tensor<f16>([[1, 2], [3, 4]]);
z := tensor.zeros([3, 224, 224]);
o := tensor.ones([10]);
r := tensor.randn([64, 784]);

// Shape and metadata
s := t.shape;       // [3]
n := m.ndim;        // 2
flat := m.reshape([4]);
mt := m.transpose();

// Element read/write
val := t[0];        // read element
t[1] = 5.0;         // write element

// ML operations come from the host
requires ml;
result := ml.matmul(weights, input);
out := ml.relu(linear_out);
loss := ml.cross_entropy(logits, labels);
```

### Error Handling

Errors are values, not exceptions. `Result<T>` with `?` propagation:

```mog
fn load_config(path: string) -> Result<Config> {
  content := fs.read(path)?;  // returns early on error
  return ok(parse(content));
}

match load_config("settings.json") {
  ok(config) => use(config),
  err(msg) => print("failed: {msg}"),
}
```

### Async/Await

For external operations (API calls, model inference, file I/O):

```mog
async fn fetch_all(urls: [string]) -> [Result<string>] {
  tasks := urls.map(fn(url) { http.get(url) });
  return await all(tasks);
}

result := await http.get("https://example.com");
```

### Host Capabilities

Mog has **no built-in I/O**. All side effects come through capability objects provided by the host:

```mog
requires fs, http, model;
optional log, env;

fn main() {
  data := fs.read("input.txt")?;
  result := await model.predict(data);
  fs.write("output.txt", result)?;
}
```

Standard capabilities: `fs`, `http`, `model`, `ml`, `log`, `env`, `db`. The compiler rejects undeclared capability usage. The host rejects scripts needing capabilities it doesn't provide. The `ml` capability provides all tensor/ML operations (matmul, activations, loss functions, autograd) — the language itself only provides tensors as a data structure.

### Module System

Mog uses a Go-style module system:

```mog
// mog.mod
module myapp
```

```mog
// math/math.mog
package math

pub fn add(a: int, b: int) -> int {
    return a + b;
}
```

```mog
// main.mog
package main

import "math"

fn main() -> int {
    result := math.add(10, 20);
    print(result);
    return 0;
}
```

- **`package`** declaration at top of every file
- **`import`** by path relative to module root (supports grouped imports)
- **`pub`** keyword for exported symbols (functions, structs, types)
- **Package = directory** — all `.mog` files in a dir share a package
- **`mog.mod`** at project root declares the module path
- **Backward compatible** — files without `package` work in single-file mode
- Circular imports detected at compile time

### String Operations

```mog
name := "world";
greeting := "hello {name}";        // interpolation
upper := greeting.upper();
parts := "a,b,c".split(",");
sub := greeting[0:5];              // slicing
n := str(42);                      // conversion
```

### Math Builtins

Available without import: `abs`, `sqrt`, `pow`, `sin`, `cos`, `tan`, `exp`, `log`, `log2`, `floor`, `ceil`, `round`, `min`, `max`, `PI`, `E`.

### Operators

- **Arithmetic**: `+`, `-`, `*`, `/`, `%`, `**`
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical**: `and`, `or`, `not`
- **Bitwise**: `&`, `|`, `~`
- **Assignment**: `:=` (bind), `=` (reassign)
- **Other**: `?` (error propagation), `..` (range), `as` (cast)

## Examples

### Agent Tool Script

```mog
requires http, model, log;

struct SearchResult { title: string, url: string, score: float }

async fn search(query: string) -> Result<[SearchResult]> {
  response := await http.get("https://api.search.com?q={query}")?;
  results := parse_results(response);
  log.info("found {results.len} results for '{query}'");
  return ok(results);
}

async fn main() {
  results := await search("machine learning")?;
  top := results.filter(fn(r) { r.score > 0.8 });
  for r in top {
    print("{r.title}: {r.url}");
  }
}
```

### ML Training Loop

```mog
requires ml, log;

fn main() {
  x := tensor.randn([64, 784]);
  w := tensor.randn([784, 10]);
  target := tensor.zeros([64, 10]);
  lr := 0.01;

  w = ml.requires_grad(w);

  for epoch in 0..100 {
    logits := ml.matmul(x, w);
    loss := ml.cross_entropy(logits, target);
    ml.backward(loss);

    w = ml.no_grad(fn() {
      return w - lr * ml.grad(w);
    });

    if epoch % 10 == 0 {
      log.info("epoch {epoch}: loss = {loss}");
    }
  }
}
```

### Plugin Script

```mog
optional log;

struct Input { text: string, max_length: int }
struct Output { result: string, truncated: bool }

fn process(input: Input) -> Output {
  text := input.text;
  truncated := false;
  if text.len > input.max_length {
    text = text[0:input.max_length];
    truncated = true;
  }
  return Output { result: text, truncated: truncated };
}
```

## What Mog Is Not

- No raw pointers or manual memory management
- No POSIX syscalls or direct OS access
- No threads or locks
- No inheritance or OOP
- No macros
- No generics (beyond tensor dtype parameterization)
- No exceptions with stack unwinding
- No operator overloading

## Implementation Status

The core language is fully implemented and tested. ML operations are not language features — they are provided by the host through capabilities.

### Fully Implemented

| Category | Features |
|---|---|
| **Type system** | `int`, `float`, `bool`, `string`, `?T` (Optional), `Result<T>`, type aliases, `as` casts, numeric precision types (`i8`–`i64`, `u8`–`u64`, `f16`, `bf16`, `f32`, `f64`) |
| **Variables** | `:=` binding, `=` reassignment, type inference, explicit annotation |
| **Functions** | `fn` with return types, single-expression bodies, closures/lambdas as first-class values, named arguments with default values |
| **Async** | `async fn`, `await`, `spawn`, `all()`, `race()`, coroutine-based with host event loop (LLVM + QBE backends) |
| **Control flow** | `if`/`else`/`elif`, if-as-expression, `while`, `for i in 0..N`, `for item in array`, `for i, item in array`, `for key, value in map`, `break`, `continue`, `match` with literal/variant/wildcard patterns |
| **Error handling** | `Result<T>` with `ok`/`err`, `?` propagation, `try`/`catch`, `match` on Result/Optional |
| **Composite types** | Arrays (`.push`, `.pop`, `.len`, `.contains`, `.sort`, `.reverse`, `.slice`, `.join()`, `.filter()`, `.map()`), Maps (create, get, set, iterate, `.has()`), Structs (declare, construct, field access/mutation), SoA (`soa Struct[N]` with AoS syntax) |
| **Strings** | UTF-8 literals, f-string interpolation, `.len`, `.upper()`, `.lower()`, `.trim()`, `.split()`, `.contains()`, `.starts_with()`, `.ends_with()`, `.replace()`, `str()` conversion, `s[start:end]` slice syntax, `+` operator concatenation, `parse_float()` |
| **Math builtins** | `sqrt`, `sin`, `cos`, `tan`, `exp`, `log`, `floor`, `ceil`, `abs`, `pow`, `asin`, `acos`, `atan2`, `log2`, `round`, `min`, `max`, `PI`, `E` |
| **Host capabilities** | `requires`/`optional` declarations, `.mogdecl` files, C API for registration, `fs` (read/write/append/exists/remove/size), `process` (sleep/getenv/cwd/exit/timestamp), `env` (custom host functions, including async) |
| **Module system** | `package`, `import`, `pub`, `mog.mod`, name mangling, circular import detection |
| **Tensors** | N-dimensional arrays with dtype, creation (literal, `.zeros()`, `.ones()`, `.randn()`), `.shape`, `.ndim`, `.reshape()`, `.transpose()`, element read/write. ML operations (matmul, activations, autograd) provided by host capabilities. |
| **Match exhaustiveness** | Warnings for non-exhaustive `match` on `Result<T>` and `?T` types |
| **Backends** | LLVM IR backend (full optimization), QBE lightweight backend (~2x faster compile than LLVM -O1), f64 codegen with proper `str()`/`println` dispatch |
| **Runtime** | Mark-and-sweep GC, `select()`-based async event loop with fd watchers and timers |
| **Safety** | Cooperative interrupt polling at loop back-edges, `mog_request_interrupt()` host API, `mog_arm_timeout(ms)` for CPU time limits, automatic timeout via `MogLimits.max_cpu_ms` |
| **Plugins** | Compile .mog to .dylib/.so shared libraries, `mog_load_plugin()`, `mog_plugin_call()`, capability sandboxing via `mog_load_plugin_sandboxed()`, plugin metadata (`MogPluginInfo`), `pub fn` export visibility |
| **Operators** | Arithmetic (`+`, `-`, `*`, `/`, `%`), comparison, logical (`and`, `or`, `not`), bitwise (`&`, `\|`, `^`, `~`, `<<`, `>>`), `?` propagation, `..` range, `as` cast |

### Not Yet Implemented

| Feature | Description |
|---|---|
| **Generics** | Out of scope per spec — Mog uses concrete types and type aliases instead |
| **`http`, `model`, `ml`, `log`, `db` reference implementations** | Reference host capability implementations. The capability system itself works — hosts can register any capability via the C API. These are convenience implementations for common use cases. |

## Future Work

### In-Process Assembler

The QBE backend currently bottlenecks on the system assembler (`as`). For large programs, QBE itself takes ~13ms but `as` takes ~59ms — 71% of backend time. Three options to eliminate this:

1. **Minimal ARM64 assembler in C** (~3-4 weeks). Write a purpose-built assembler handling only the ~54 ARM64 instruction mnemonics QBE actually emits, plus Mach-O object file output. ARM64's fixed 4-byte encoding makes this tractable. Expected: ~2-5ms for 7K lines of assembly, saving ~55ms.

2. **Modify QBE to emit machine code directly** (~2-3 weeks). Replace QBE's text emitters with binary encoders in `arm64/emit.c`, emitting Mach-O `.o` files directly. Eliminates the text→parse→encode round-trip entirely. Cleanest long-term solution but requires deeper QBE modifications.

3. **Skip assembly, JIT to memory** (~2 weeks). For development/REPL use cases, encode ARM64 directly into an executable memory page and jump to it. No assembler or linker needed. Limited to same-machine execution.

## Testing

```bash
bun test
```

1338 tests passing across 30 test files.

## Architecture

```
src/
  lexer.ts          Tokenization (84 token types, 31 keywords)
  parser.ts         AST generation (62 node types)
  analyzer.ts       Type checking, capability checking, scope resolution
  types.ts          Type system (21 type classes)
  compiler.ts       Compiler orchestration
  llvm_codegen.ts   LLVM IR generation (68 codegen methods)
  qbe_codegen.ts    QBE IL generation (lightweight backend)
  mog_backend_ffi.ts  Bun FFI bindings to in-process QBE
  linker.ts         Links IR to native executable
  capability.ts     .mogdecl parser for host FFI declarations
  module.ts         Module resolver with circular import detection
  stdlib.ts         Standard library functions

runtime/
  runtime.c         C runtime (GC, strings, arrays, maps, tensors, I/O)
  mog.h             Public C API for host embedding
  mog_vm.c          VM for host FFI capability dispatch, interrupt flag, timeout
  mog_async.c       Event loop, futures, coroutine resume, all/race
  mog_async.h       Async runtime headers
  posix_host.c      Built-in fs and process capability providers
  mog_plugin.h      Plugin loading C API (dlopen, sandboxing)
  mog_plugin.c      Plugin load/call/unload implementation
  mog_backend.c     In-process QBE + assembler bridge (FFI target)

capabilities/
  *.mogdecl         Capability type declarations for host FFI

examples/
  host.c            Example host application with custom capabilities
  build_showcase.sh Build script for the showcase demo
  plugins/           Plugin example (math_plugin.mog + C host)
```

## Embedding

Mog is designed to be embedded in a host application. The host provides capabilities and resource limits:

```c
#include "mog.h"

MogVM *vm = mog_vm_new();
mog_register_capability(vm, "math", math_functions, 3);
mog_register_capability(vm, "log", log_functions, 2);

// Validate script requirements
mog_validate_capabilities(vm, script_requires, script_optionals);

// Call into Mog
MogValue result = mog_cap_call(vm, "math", "add", args);

// Safety: set CPU time limit (auto-interrupts infinite loops)
MogLimits limits = { .max_cpu_ms = 5000 };  // 5 second timeout
mog_vm_set_limits(vm, limits);

// Or manually interrupt from another thread:
mog_request_interrupt();
```

### Plugins

Compile Mog code to shared libraries and load them at runtime:

```c
#include "mog_plugin.h"

// Load a pre-compiled Mog plugin
MogPlugin *plugin = mog_load_plugin("./math_plugin.dylib", vm);

// Call exported functions by name
MogValue args[] = { mog_int(10) };
MogValue result = mog_plugin_call(plugin, "fibonacci", args, 1);
printf("fibonacci(10) = %lld\n", result.data.i);  // 55

// Sandbox: restrict which capabilities a plugin can access
const char *allowed[] = { "log", NULL };
MogPlugin *safe = mog_load_plugin_sandboxed("./untrusted.dylib", vm, allowed);

mog_unload_plugin(plugin);
```

Plugins use `pub fn` to export functions. Non-pub functions have internal linkage. See `examples/plugins/` for a complete example.

## License

MIT
