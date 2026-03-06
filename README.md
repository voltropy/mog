# Mog

A small, statically-typed, embeddable language for ML workflows and LLM agent scripting. Has a pure-Rust toolchain: Rust compiler, Rust runtime (~6K lines), and an in-process QBE backend (`rqbe`, a safe Rust rewrite of QBE). Think of it as a statically-typed Lua with tensors, async I/O, and a capability model where the host provides all I/O and ML operations.

- **[Language Guide](docs/guide.md)** — Full tutorial from first program through async, modules, embedding, and plugins.
- **[Showcase](showcase.mog)** — One file demonstrating every language feature (755 lines).
- **[LLM Context](docs/context.md)** — Compact reference designed to fit in an LLM's context window.

## Why Mog

Most ML work today lives in Python scripts that are difficult to sandbox, expensive to deploy, and opaque to the LLM agents that increasingly need to write and execute code. Mog exists to give those agents — and the humans supervising them — a language that is small enough to fit entirely in a model's context window, safe enough to run untrusted code without fear, and expressive enough to do real ML work without dropping into a general-purpose language.

Mog is not a standalone systems language. It is always embedded inside a host application. The host decides what the script can do: read a file, call an API, run inference on a model. The script declares what it needs (`requires http, model`) and the host either grants those capabilities or refuses to run it. There is no way for a Mog script to escape its sandbox, access the filesystem behind the host's back, or crash the process. This makes it suitable for running agent-generated code in production, where the alternative is either no code execution at all or an elaborate container-based sandbox around a general-purpose language.

The language compiles to native code through `rqbe` (an in-process QBE backend written in safe Rust), so tight numerical loops run at C speed rather than interpreter speed. Tensors are a built-in data structure — n-dimensional arrays with hardware-relevant dtypes like `f16` and `bf16`, element read/write, and shape manipulation. All ML operations (matmul, activations, loss functions, autograd) are provided by the host through the same capability system as I/O. The language gives you the data structure; the host gives you the compute. This means the host can route tensor operations to whatever backend makes sense — CPU, GPU, or a remote accelerator — without the language needing to know.

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

## Toolchain

The compiler and runtime are pure Rust.

The Rust compiler (`mogc`) is a standalone native binary. It uses `rqbe`, an in-process QBE backend written in safe Rust (~15K lines), to produce fast native code with minimal compile times. The Rust runtime (`runtime-rs/`, ~6K lines) provides GC, async, strings, arrays, maps, tensors, and plugin support.

**Build the compiler:**

```bash
cargo build --release --manifest-path compiler/Cargo.toml
```

This produces `compiler/target/release/mogc`.

**Build the runtime:**

```bash
cargo build --release --manifest-path runtime-rs/Cargo.toml
```

**Usage:**

```bash
# Compile to native binary
mogc program.mog -o program

# Compile with optimization (-O0, -O1, -O2)
mogc program.mog -o program -O1

# Emit QBE IR (for inspection)
mogc program.mog --emit-ir

# Compile as a plugin (.dylib/.so shared library)
mogc program.mog --plugin mylib --plugin-version 1.0.0 -o mylib.dylib

# Link additional Rust or C files (host capabilities, etc.)
mogc program.mog --link host.rs -o program
```

**Requirements:** A C compiler (`cc`) must be on `$PATH` for linking. The QBE backend (`rqbe`) is built in-process — no external `qbe` binary is needed.

**Embedding from Rust:**

The compiler is also a Rust library crate. Add it as a dependency and call the API directly:

```rust
use mog::compiler::{compile, compile_to_binary, CompileOptions};

let source = r#"fn main() { println("hello"); }"#;
let result = compile(source, None);
println!("{}", result.ir);  // QBE IL output
```

**Embedding from C:**

The crate builds as a `cdylib` and `staticlib`, exposing a C API via `compiler/include/mog_compiler.h`:

```c
#include "mog_compiler.h"

MogCompiler *c = mog_compiler_new();
MogCompileResult *r = mog_compile(c, source, source_len, NULL);
const char *ir = mog_result_ir(r);
```

**Testing:**

```bash
# Compiler tests (1,146+ tests across 13 files)
cargo test --manifest-path compiler/Cargo.toml

# rqbe backend tests (186 tests)
cargo test --manifest-path rqbe/Cargo.toml
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

Conversions between numeric types require explicit `as` cast.

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
| **Async** | `async fn`, `await`, `spawn`, `all()`, `race()`, coroutine-based with host event loop |
| **Control flow** | `if`/`else`/`elif`, if-as-expression, `while`, `for i in 0..N`, `for item in array`, `for i, item in array`, `for key, value in map`, `break`, `continue`, `match` with literal/variant/wildcard patterns |
| **Error handling** | `Result<T>` with `ok`/`err`, `?` propagation, `try`/`catch`, `match` on Result/Optional |
| **Composite types** | Arrays (`.push`, `.pop`, `.len`, `.contains`, `.sort`, `.reverse`, `.slice`, `.join()`, `.filter()`, `.map()`), Maps (create, get, set, iterate, `.has()`), Structs (declare, construct, field access/mutation), SoA (`soa Struct[N]` with AoS syntax) |
| **Strings** | UTF-8 literals, f-string interpolation, `.len`, `.upper()`, `.lower()`, `.trim()`, `.split()`, `.contains()`, `.starts_with()`, `.ends_with()`, `.replace()`, `str()` conversion, `s[start:end]` slice syntax, `+` operator concatenation, `parse_float()` |
| **Math builtins** | `sqrt`, `sin`, `cos`, `tan`, `exp`, `log`, `floor`, `ceil`, `abs`, `pow`, `asin`, `acos`, `atan2`, `log2`, `round`, `min`, `max`, `PI`, `E` |
| **Host capabilities** | `requires`/`optional` declarations, `.mogdecl` files, Rust and C APIs for registration, `fs` (read/write/append/exists/remove/size), `process` (sleep/getenv/cwd/exit/timestamp), `env` (custom host functions, including async) |
| **Module system** | `package`, `import`, `pub`, `mog.mod`, name mangling, circular import detection |
| **Tensors** | N-dimensional arrays with dtype, creation (literal, `.zeros()`, `.ones()`, `.randn()`), `.shape`, `.ndim`, `.reshape()`, `.transpose()`, element read/write. ML operations (matmul, activations, autograd) provided by host capabilities. |
| **Match exhaustiveness** | Warnings for non-exhaustive `match` on `Result<T>` and `?T` types |
| **Backends** | Rust compiler with in-process `rqbe` backend (standalone `mogc` binary, safe Rust ~15K lines) |
| **Runtime** | Rust runtime (`runtime-rs/`, ~6K lines): mark-and-sweep GC, async event loop, strings, arrays, maps, tensors, plugin support |
| **Safety** | Cooperative interrupt polling at loop back-edges, `mog_request_interrupt()` host API, `mog_arm_timeout(ms)` for CPU time limits, automatic timeout via `MogLimits.max_cpu_ms` |
| **Plugins** | Compile .mog to .dylib/.so shared libraries, `mog_load_plugin()`, `mog_plugin_call()`, capability sandboxing via `mog_load_plugin_sandboxed()`, plugin metadata (`MogPluginInfo`), `pub fn` export visibility |
| **Operators** | Arithmetic (`+`, `-`, `*`, `/`, `%`), comparison, logical (`and`, `or`, `not`), bitwise (`&`, `\|`, `^`, `~`, `<<`, `>>`), `?` propagation, `..` range, `as` cast |

### Not Yet Implemented

| Feature | Description |
|---|---|
| **Generics** | Out of scope per spec — Mog uses concrete types and type aliases instead |
| **`http`, `model`, `ml`, `log`, `db` reference implementations** | Reference host capability implementations. The capability system itself works — hosts can register any capability via the Rust or C API. These are convenience implementations for common use cases. |

## Future Work

### In-Process Assembler

The QBE backend currently bottlenecks on the system assembler (`as`). For large programs, `rqbe` itself takes ~13ms but `as` takes ~59ms — 71% of backend time. Three options to eliminate this:

1. **Minimal ARM64 assembler in Rust** (~3-4 weeks). Write a purpose-built assembler handling only the ~54 ARM64 instruction mnemonics `rqbe` actually emits, plus Mach-O object file output. ARM64's fixed 4-byte encoding makes this tractable. Expected: ~2-5ms for 7K lines of assembly, saving ~55ms.

2. **Modify `rqbe` to emit machine code directly** (~2-3 weeks). Replace `rqbe`'s text emitters with binary encoders, emitting Mach-O `.o` files directly. Eliminates the text->parse->encode round-trip entirely. Cleanest long-term solution but requires deeper `rqbe` modifications.

3. **Skip assembly, JIT to memory** (~2 weeks). For development/REPL use cases, encode ARM64 directly into an executable memory page and jump to it. No assembler or linker needed. Limited to same-machine execution.

## Architecture

### Compiler

```
compiler/
  src/
    main.rs           CLI binary (mogc)
    lib.rs            Library crate root
    lexer.rs          Tokenization (85 token types)
    parser.rs         Recursive descent parser
    ast.rs            AST node types (25 statement + 41 expression kinds)
    analyzer.rs       Type checking, capability checking, scope resolution
    types.rs          Type system (19 type variants)
    qbe_codegen.rs    QBE IL code generation (~4,700 lines)
    compiler.rs       Compilation pipeline orchestration
    capability.rs     .mogdecl parser for host FFI declarations
    module.rs         Module resolver with circular import detection
    ffi.rs            C FFI bindings (cdylib/staticlib)
  include/
    mog_compiler.h    C header for embedding
  tests/
    test_*.rs         1,146+ tests across 13 files

rqbe/                 In-process QBE backend (safe Rust, ~15K lines, 186 tests)
```

The compiler uses the `rqbe` in-process QBE backend — a safe Rust rewrite of QBE (~15K lines) that runs in-process, eliminating the need for an external `qbe` binary. It produces `mogc`, a standalone native binary with no runtime dependencies beyond a C compiler for linking. The crate also builds as a C library (`libmog.dylib` / `libmog.a`) for embedding in C/C++ hosts.

### Runtime

The Rust runtime (`runtime-rs/`, ~6K lines) provides GC, strings, arrays, maps, tensors, async event loop, and plugin support — all in pure Rust.

```
runtime-rs/         Rust runtime (~6K lines)

examples/
  host.rs               Rust host application with custom capabilities
  guide_search_host.rs  Guide search embedding example
  timer.mog             Timer example
  plugins/
    plugin_host.rs      Rust plugin host example
    async_plugin_demo/  Complete example: runtime compilation + dlopen + async

mogfmt/             Code formatter (Rust crate)
benchmarks/         Benchmark suite (Rust crate, compares Mog vs Go vs Rust)

capabilities/
  *.mogdecl         Capability type declarations for host FFI
```

Build the runtime:

```bash
cargo build --release --manifest-path runtime-rs/Cargo.toml
```

## Embedding

Mog is designed to be embedded in a host application. The host provides capabilities and resource limits.

**Rust host examples:** See `examples/host.rs` and `examples/guide_search_host.rs` for complete Rust embedding examples. The compiler supports `--link file.rs` to link a Rust host file directly into the compiled binary.

**C embedding:** The compiler builds as a `cdylib` and `staticlib` (`libmog.dylib` / `libmog.a`), exposing a C API via `compiler/include/mog_compiler.h`. See the [Embedding from C](#embedding-from-c) section above for the compilation API.

### Plugins

Compile Mog code to shared libraries (`.dylib`/`.so`) and load them at runtime. On macOS, plugins are linked with `-undefined dynamic_lookup` so they share the host's runtime globals (VM, event loop, GC) — no duplicate state. A stub object provides `mog_interrupt_flag` for ARM64 ADRP addressing in plugins.

Plugin binaries are integrity-checked using BLAKE3 hashes. `compile_plugin()` returns the hash alongside the compiled path, and `mog_load_plugin_verified()` re-computes the hash before loading — rejecting tampered files before any code executes.

Plugins use `pub fn` to export functions. Non-pub functions have internal linkage. See `examples/plugins/` for plugin examples, and `examples/plugins/async_plugin_demo/` for a complete example of runtime compilation + dynamic loading (`dlopen`) + BLAKE3 verification + async plugin function calls from a Rust host.

## License

MIT
