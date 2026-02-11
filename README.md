# Mog

A small, statically-typed, embeddable language for ML workflows and LLM agent scripting. Compiles to native code via LLVM. Think of it as a statically-typed Lua with native tensor support and async I/O.

## Use Cases

- LLM agent tool-use scripts
- ML training and inference workflows
- Plugin/extension scripting for host applications
- Short automation scripts

## Design Philosophy

1. **Small surface area** — entire language fits in an LLM's context window
2. **Predictable semantics** — no implicit coercion, no operator precedence puzzles, no hidden control flow
3. **Familiar syntax** — curly braces, `fn`, `->`, `:=`; blend of Rust/Go/TypeScript
4. **Safe by default** — GC, bounds-checked, no null, no raw pointers; cannot crash host
5. **Host provides I/O** — no built-in file/network/system access; capabilities explicitly granted by host
6. **ML as first-class concern** — tensor types with hardware-relevant dtypes, shape checking

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

N-dimensional arrays with fixed element dtype. The core ML primitive:

```mog
// Creation
t := tensor([1.0, 2.0, 3.0]);
m := tensor<f16>([[1, 2], [3, 4]]);
z := tensor.zeros([3, 224, 224]);

// Elementwise ops
c := a + b;
d := a * b;
e := a ** 2;

// Reductions
total := t.sum();
avg := t.mean(dim: 0);

// Linear algebra
result := matmul(weights, input);
d := dot(a, b);

// ML operations
out := relu(linear_out);
loss := cross_entropy(logits, labels);
attn := scaled_dot_product_attention(q, k, v);

// Autograd
w := tensor([1.0, 2.0]).requires_grad();
loss.backward();
gradient := w.grad;

with no_grad() {
  w = w - lr * w.grad;
}

// Dtype conversion
half := t.to(f16);
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

Standard capabilities: `fs`, `http`, `model`, `log`, `env`, `db`. The compiler rejects undeclared capability usage. The host rejects scripts needing capabilities it doesn't provide.

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
requires model, log;

fn main() {
  x := tensor.randn([64, 784]);
  w := tensor.randn([784, 10]).requires_grad();
  target := tensor.zeros([64, 10]);
  lr := 0.01;

  for epoch in 0..100 {
    logits := matmul(x, w);
    loss := cross_entropy(logits, target);
    loss.backward();

    with no_grad() {
      w = w - lr * w.grad;
    }

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
- No operator overloading (beyond tensor ops)

## Testing

```bash
bun test
```

504 tests passing across 11 test files.

## Architecture

```
src/
  lexer.ts          Tokenization
  parser.ts         AST generation
  analyzer.ts       Type checking, capability checking, scope resolution
  compiler.ts       Compiler orchestration
  llvm_codegen.ts   LLVM IR generation
  linker.ts         Links IR to native executable
  capability.ts     .mogdecl parser for host FFI declarations
  types.ts          Type system definitions
  stdlib.ts         Standard library functions

runtime/
  runtime.c         C runtime (GC, I/O primitives)
  mog.h             Public C API for host embedding
  mog_vm.c          VM for host FFI capability dispatch

capabilities/
  *.mogdecl         Capability declaration files
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
```

## License

MIT
