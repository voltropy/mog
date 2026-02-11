# Mog Language Specification

## Overview

Mog is a small, statically-typed, embeddable language for ML workflows and LLM agent scripting. It compiles to native code via LLVM. Think of it as a statically-typed Lua with native tensor support and async I/O.

**Target use cases:**
- LLM agent tool-use scripts
- ML training and inference workflows
- Plugin/extension scripting for host applications
- Short automation scripts

**Non-goals:**
- Systems programming (no raw pointers, no manual memory management)
- Standalone applications (always embedded in a host)
- Replacing general-purpose languages

## Design Philosophy

1. **Small surface area.** The entire language should fit in an LLM's context window. Every feature must justify its existence. When in doubt, leave it out.

2. **Predictable semantics.** No implicit coercion surprises, no operator precedence puzzles, no hidden control flow. Code reads top-to-bottom, left-to-right.

3. **Familiar syntax.** Curly braces, `fn`, `->`, `:=`. Blend of Rust/Go/TypeScript that LLMs already generate fluently. No novel syntax without strong justification.

4. **Safe by default.** Garbage collected, bounds-checked, no null, no raw pointers. The language cannot crash the host or escape its sandbox.

5. **Host provides I/O.** The language has no built-in file, network, or system access. All side effects go through capabilities explicitly granted by the embedding host.

6. **ML as a first-class concern.** Tensor types with hardware-relevant dtypes (f16, bf16, f32, f64), shape checking, and operations that map to accelerator hardware.

## Syntax

### General Structure

Programs are sequences of top-level declarations (functions, structs, constants) and a `main` entry point.

```mog
struct Point { x: float, y: float }

fn distance(a: Point, b: Point) -> float {
  dx := a.x - b.x;
  dy := a.y - b.y;
  return sqrt((dx * dx) + (dy * dy));
}

fn main() {
  p1 := Point { x: 1.0, y: 2.0 };
  p2 := Point { x: 4.0, y: 6.0 };
  d := distance(p1, p2);
  print(d);
}
```

### Statements

```mog
// Variable declaration with type inference
x := 42;
name := "hello";

// Variable declaration with explicit type
x: int = 42;
ratio: float = 3.14;

// Assignment (walrus operator for initial binding, = for reassignment)
x := 42;       // initial binding
x = x + 1;    // reassignment

// Return
return expression;

// Expression statement
some_function();
```

### Comments

```mog
// Single line comment

/* Multi-line comment */
```

## Type System

### Scalar Types

```
int       64-bit signed integer (default integer type)
float     64-bit floating point (default float type)
bool      true or false
string    UTF-8 string (immutable, GC-managed)
```

`int` and `float` are the default types for literals. You never need to spell out a width for ordinary code.

### Numeric Precision Types

Used when precision matters — primarily in tensor element types, ML configurations, and interop with hardware:

```
// Integers (for tensor dtypes and bitwise operations)
i8  i16  i32  i64

// Unsigned integers (for tensor dtypes, indexing)
u8  u16  u32  u64

// Floating point (for tensor dtypes and ML precision control)
f16  bf16  f32  f64
```

These exist primarily as **tensor element types**. In scalar code, you almost always use `int` and `float`:

```mog
// Scalar code: use int and float
count := 42;            // int
ratio := 3.14;          // float

// Tensor code: precision types matter
weights := tensor<f16>([768, 768]);      // half-precision weights
indices := tensor<i32>([1000]);          // integer indices
image := tensor<u8>([3, 224, 224]);      // byte image data
```

Widening conversions are implicit (i32 -> int, f32 -> float). Narrowing requires explicit `as` cast.

### Composite Types

#### Arrays

Dynamically-sized, homogeneous, GC-managed:

```mog
numbers := [1, 2, 3, 4, 5];             // [int]
names := ["alice", "bob", "charlie"];    // [string]

// Type annotation
scores: [float] = [95.5, 88.0, 92.3];

// Operations
numbers.push(6);
last := numbers.pop();
length := numbers.len;
slice := numbers[1:3];       // [2, 3]
```

#### Maps

Key-value dictionaries. Keys must be `int`, `float`, `string`, or `bool`:

```mog
ages := {"alice": 30, "bob": 25};       // {string: int}
config := {1: "one", 2: "two"};         // {int: string}

// Access
age := ages["alice"];

// Mutation
ages["charlie"] = 28;

// Check existence
if ages.has("alice") {
  print(ages["alice"]);
}

// Iteration
for key, value in ages {
  print(key, value);
}
```

#### Structs

Named product types with named fields. No methods, no inheritance:

```mog
struct Point { x: float, y: float }

struct Model {
  name: string,
  layers: int,
  weights: tensor<f32>,
}

// Construction
p := Point { x: 1.0, y: 2.0 };

// Field access
print(p.x);

// Field mutation
p.x = 3.0;
```

#### SoA (Struct of Arrays)

Columnar containers backed by an existing struct. You interact using AoS syntax but the compiler transposes access to SoA storage — one contiguous array per field. This gives cache-friendly memory layout for field-iteration patterns common in ML and game engines:

```mog
struct Datum { id: i64, val: i64 }

datums := soa Datum[100];  // type-inferred, regular var decl with semicolon

datums[0].id = 1;          // lowers to columnar array access
datums[1].val = 200;       // lowers to columnar array access
x := datums[0].id;         // lowers to columnar array read
```

`soa Datum[100]` is both a type annotation and a constructor expression. Under the hood, `datums` is stored as separate contiguous arrays (`id: [i64; 100]`, `val: [i64; 100]`) rather than an array of structs. Element access like `datums[i].field` compiles to a direct index into the corresponding field array.

#### Tensors

N-dimensional arrays with a fixed element dtype. The core ML primitive:

```mog
// Create from literal
t := tensor([1.0, 2.0, 3.0]);                    // tensor<float>, shape [3]

// Create with explicit dtype
t := tensor<f16>([1.0, 2.0, 3.0]);               // tensor<f16>, shape [3]

// Create with shape
zeros := tensor<f32>.zeros([3, 224, 224]);        // shape [3, 224, 224]
ones := tensor<f32>.ones([768]);                   // shape [768]
rand := tensor<f32>.randn([128, 64]);             // random normal

// Shape and dtype
print(t.shape);     // [3]
print(t.dtype);     // f16

// Indexing
val := t[0];                                       // scalar
row := matrix[0, :];                               // slice row
block := volume[:, 0:10, :];                       // slice block

// Reshape (view, no copy)
reshaped := t.reshape([1, 3]);

// Elementwise operations
c := a + b;          // add
c := a * b;          // multiply
c := a / b;          // divide
c := a ** 2.0;       // power

// Reduction
sum := t.sum();
mean := t.mean();
max_val := t.max();

// Linear algebra
product := matmul(a, b);                           // matrix multiply
d := dot(v1, v2);                                  // dot product
n := norm(v);                                      // L2 norm

// ML operations
y := relu(x);
y := softmax(x, dim: -1);
y := layer_norm(x, normalized_shape: [768]);
y := conv2d(input, weight, bias, stride: 1, padding: 0);
loss := cross_entropy(logits, targets);

// Autograd
loss.backward();
grad := weights.grad;
```

See [Tensor Operations](#tensor-operations) for the complete list.

### Optional Type

No null. Use `?T` for values that might not exist:

```mog
fn find(items: [string], target: string) -> ?int {
  for i, item in items {
    if item == target {
      return i;
    }
  }
  return none;
}

result := find(names, "alice");
if result is some(idx) {
  print("found at", idx);
} else {
  print("not found");
}
```

### Type Aliases

```mog
type Batch = tensor<f32>;
type Config = {string: string};
type Callback = fn(int) -> bool;
```

## Functions

### Definition

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

### Closures

Functions are first-class values. Closures capture their environment:

```mog
fn make_adder(n: int) -> fn(int) -> int {
  return fn(x: int) -> int { x + n };
}

add5 := make_adder(5);
print(add5(10));    // 15

// Inline closures
numbers := [3, 1, 4, 1, 5];
sorted := numbers.sort(fn(a: int, b: int) -> bool { a < b });
doubled := numbers.map(fn(x: int) -> int { x * 2 });
```

### Named Arguments

For functions with many parameters (common in ML APIs):

```mog
fn train(
  model: Model,
  data: tensor<f32>,
  epochs: int = 10,
  lr: float = 0.001,
  batch_size: int = 32,
) -> Model {
  // ...
}

// Call with named args
trained := train(model, data, epochs: 50, lr: 0.0001);
```

## Control Flow

### If/Else

```mog
if x > 0 {
  print("positive");
} else if x < 0 {
  print("negative");
} else {
  print("zero");
}

// If as expression
sign := if x > 0 { 1 } else if x < 0 { -1 } else { 0 };
```

### While Loop

```mog
i := 0;
while i < 10 {
  print(i);
  i = i + 1;
}
```

### For Loop

Iterates over arrays, maps, ranges, and tensors:

```mog
// Range
for i in 0..10 {
  print(i);
}

// Array
for item in items {
  print(item);
}

// Array with index
for i, item in items {
  print(i, item);
}

// Map
for key, value in config {
  print(key, value);
}
```

### Break and Continue

```mog
for i in 0..100 {
  if i % 2 == 0 { continue; }
  if i > 50 { break; }
  print(i);
}
```

## Error Handling

Errors are values, not exceptions. Functions that can fail return `Result<T>`:

```mog
// Result is a built-in sum type:
//   ok(value: T)
//   err(message: string)

fn parse_int(s: string) -> Result<int> {
  // ... parsing logic ...
  if valid {
    return ok(value);
  }
  return err("invalid integer: " + s);
}

// Handle with match
result := parse_int("42");
match result {
  ok(n) => print("parsed:", n),
  err(msg) => print("error:", msg),
}

// Propagate with ?
fn load_config(path: string) -> Result<Config> {
  content := fs.read(path)?;          // returns early on error
  config := parse_json(content)?;
  return ok(config);
}

// try/catch for convenience
try {
  data := fs.read("data.csv")?;
  model := load_model("weights.bin")?;
  result := model.predict(data)?;
  print(result);
} catch(e) {
  print("failed:", e);
}
```

## Async/Await

Agent scripts need to wait on external operations (API calls, model inference, file I/O). Mog uses async/await with structured concurrency:

```mog
// Async function
async fn fetch_data(url: string) -> Result<string> {
  response := await http.get(url)?;
  return ok(response.body);
}

// Sequential async
async fn pipeline() -> Result<string> {
  raw := await fetch_data("https://api.example.com/data")?;
  processed := await transform(raw)?;
  return ok(processed);
}

// Concurrent async (structured)
async fn parallel_fetch() -> Result<[string]> {
  // all() runs tasks concurrently and waits for all to complete
  results := await all([
    fetch_data("https://api.example.com/a"),
    fetch_data("https://api.example.com/b"),
    fetch_data("https://api.example.com/c"),
  ])?;
  return ok(results);
}

// Race (first to complete wins)
async fn fastest() -> Result<string> {
  result := await race([
    fetch_from_primary(),
    fetch_from_backup(),
  ])?;
  return ok(result);
}
```

The host runtime manages the event loop. Mog code never creates threads or manages concurrency primitives directly.

## Host Capabilities

Mog has **no built-in I/O**. All side effects go through capability objects provided by the host at initialization. This makes sandboxing trivial — if you don't grant a capability, the script can't use it.

### Standard Capabilities

These are conventional names the host may provide. None are guaranteed — a host can provide all, some, or none:

```mog
// File system (if granted by host)
content := await fs.read("data.csv")?;
await fs.write("output.txt", result)?;
files := await fs.list("./models/")?;

// HTTP (if granted by host)
response := await http.get("https://api.example.com/data")?;
response := await http.post(url, body: json_data, headers: headers)?;

// Model inference (if granted by host)
result := await model.predict(input_tensor)?;
embeddings := await model.embed(texts)?;

// Logging (if granted by host)
log.info("training epoch", epoch);
log.error("failed to load", path);

// Environment (if granted by host)
api_key := env.get("API_KEY")?;

// Database (if granted by host)
rows := await db.query("SELECT * FROM users WHERE age > ?", [18])?;
```

### Capability Declaration

Scripts declare what capabilities they require. The host checks this before execution:

```mog
// At top of script — declares required capabilities
requires fs, http, model;

// Optional capabilities (script works without them)
optional log, env;
```

If a script tries to use an undeclared capability, the compiler rejects it. If it declares a capability the host doesn't provide, the host rejects the script before running it.

## Tensor Operations

### Creation

```mog
tensor<dtype>.zeros(shape)          // all zeros
tensor<dtype>.ones(shape)           // all ones
tensor<dtype>.full(shape, value)    // filled with value
tensor<dtype>.randn(shape)          // random normal
tensor<dtype>.rand(shape)           // random uniform [0, 1)
tensor<dtype>.arange(start, end, step)  // range
tensor<dtype>.eye(n)                // identity matrix
tensor([1.0, 2.0, 3.0])            // from literal (infers dtype)
tensor<f16>([1.0, 2.0, 3.0])       // from literal with explicit dtype
```

### Shape Operations

```mog
t.shape                   // [int] - dimensions
t.dtype                   // dtype enum
t.reshape(new_shape)      // view with new shape
t.transpose()             // transpose last two dims
t.transpose(dim0, dim1)   // transpose specific dims
t.squeeze(dim)            // remove size-1 dimension
t.unsqueeze(dim)          // add size-1 dimension
t.expand(shape)           // broadcast to shape
t.contiguous()            // ensure contiguous memory
t.flatten()               // flatten to 1D
t.view(shape)             // alias for reshape
```

### Elementwise Operations

```mog
a + b       a - b       a * b       a / b       // arithmetic
a ** b                                           // power
a == b      a != b      a < b       a > b       // comparison (returns bool tensor)
a & b       a | b       ~a                       // bitwise (integer tensors)
abs(a)      neg(a)      sign(a)                  // unary
exp(a)      log(a)      log2(a)     log10(a)     // exponential
sqrt(a)     rsqrt(a)                              // roots
sin(a)      cos(a)      tan(a)                   // trigonometric
floor(a)    ceil(a)     round(a)                 // rounding
clamp(a, min, max)                               // clamping
```

### Reduction Operations

```mog
t.sum()               t.sum(dim)             // sum
t.mean()              t.mean(dim)            // mean
t.max()               t.max(dim)             // max
t.min()               t.min(dim)             // min
t.argmax(dim)         t.argmin(dim)          // arg max/min
t.prod()              t.prod(dim)            // product
t.any()               t.all()                // logical
```

### Linear Algebra

```mog
matmul(a, b)                    // matrix multiplication
dot(a, b)                       // dot product
norm(a)                         // L2 norm
norm(a, p)                      // Lp norm
cross(a, b)                     // cross product
```

### ML Operations

```mog
// Activations
relu(x)
gelu(x)
silu(x)                         // SiLU / swish
sigmoid(x)
tanh(x)
softmax(x, dim: -1)

// Normalization
layer_norm(x, normalized_shape, weight, bias)
batch_norm(x, running_mean, running_var, weight, bias)
group_norm(x, num_groups, weight, bias)

// Convolution
conv1d(input, weight, bias, stride, padding)
conv2d(input, weight, bias, stride, padding)

// Pooling
max_pool2d(input, kernel_size, stride, padding)
avg_pool2d(input, kernel_size, stride, padding)

// Loss
cross_entropy(logits, targets)
mse_loss(prediction, target)
binary_cross_entropy(prediction, target)

// Attention
scaled_dot_product_attention(query, key, value, mask)

// Dropout (training only)
dropout(x, p: 0.1, training: true)

// Embedding
embedding(indices, weight)
```

### Autograd

```mog
// Enable gradient tracking
x := tensor<f32>.randn([10, 10]).requires_grad();
y := matmul(x, x) + x;
loss := y.sum();

// Backpropagation
loss.backward();

// Access gradients
grad := x.grad;

// Gradient context
with no_grad() {
  // Operations here don't track gradients
  inference_result := model_forward(input);
}
```

### Dtype Conversion

```mog
t_f16 := t.to(f16);              // convert to half precision
t_f32 := t.to(f32);              // convert to single precision
t_int := t.to(i32);              // convert to integer
```

## String Operations

```mog
// String interpolation
name := "world";
greeting := "hello {name}";         // "hello world"
result := "sum = {a + b}";          // expression interpolation

// Methods
s.len                               // length
s.upper()                           // uppercase
s.lower()                           // lowercase
s.trim()                            // strip whitespace
s.split(delimiter)                  // -> [string]
s.contains(substring)               // -> bool
s.starts_with(prefix)               // -> bool
s.ends_with(suffix)                 // -> bool
s.replace(old, new)                 // -> string
s[start:end]                        // slice

// Conversion
str(42)                              // "42"
str(3.14)                            // "3.14"
int("42")                            // parse to int (returns Result)
float("3.14")                        // parse to float (returns Result)
```

## Math Operations

Available as builtins (no import needed):

```mog
abs(x)       // absolute value
sqrt(x)      // square root
pow(x, n)    // power
sin(x)       cos(x)       tan(x)       // trig
asin(x)      acos(x)      atan2(y, x)  // inverse trig
exp(x)       log(x)       log2(x)      // exponential/logarithmic
floor(x)     ceil(x)      round(x)     // rounding
min(a, b)    max(a, b)                  // comparison
PI           E                          // constants
```

## Example Programs

### Simple Script

```mog
fn fibonacci(n: int) -> int {
  if n <= 1 { return n; }
  a := 0;
  b := 1;
  for i in 2..n+1 {
    temp := a + b;
    a = b;
    b = temp;
  }
  return b;
}

fn main() {
  for i in 0..10 {
    print(fibonacci(i));
  }
}
```

### Agent Tool Script

```mog
requires http, model, log;

struct SearchResult {
  title: string,
  url: string,
  relevance: float,
}

async fn search_and_summarize(query: string) -> Result<string> {
  log.info("searching for: {query}");

  response := await http.get(
    "https://api.search.com/v1/search",
    params: {"q": query, "limit": "5"},
  )?;

  results: [SearchResult] = parse_json(response.body)?;

  // Filter by relevance
  relevant := results.filter(fn(r: SearchResult) -> bool {
    r.relevance > 0.7
  });

  // Summarize with model
  context := relevant.map(fn(r: SearchResult) -> string {
    "{r.title}: {r.url}"
  });

  summary := await model.predict(
    prompt: "Summarize these search results for: {query}",
    context: context,
  )?;

  return ok(summary);
}

async fn main() -> Result<()> {
  summary := await search_and_summarize("Mog language")?;
  print(summary);
  return ok(());
}
```

### ML Training Loop

```mog
requires model, log;

struct TrainConfig {
  epochs: int,
  lr: float,
  batch_size: int,
}

fn create_linear(in_features: int, out_features: int) -> tensor<f32> {
  return tensor<f32>.randn([in_features, out_features]) * 0.01;
}

async fn train(config: TrainConfig) -> Result<tensor<f32>> {
  weights := create_linear(784, 10).requires_grad();
  bias := tensor<f32>.zeros([10]).requires_grad();

  for epoch in 0..config.epochs {
    // Get batch from host
    batch := await model.next_batch(config.batch_size)?;
    images := batch.images;     // tensor<f32> [batch, 784]
    labels := batch.labels;     // tensor<i32> [batch]

    // Forward pass
    logits := matmul(images, weights) + bias;
    loss := cross_entropy(logits, labels);

    // Backward pass
    loss.backward();

    // SGD update
    with no_grad() {
      weights = weights - config.lr * weights.grad;
      bias = bias - config.lr * bias.grad;
    }

    if epoch % 10 == 0 {
      log.info("epoch {epoch}: loss = {loss.item()}");
    }
  }

  return ok(weights);
}

async fn main() -> Result<()> {
  config := TrainConfig {
    epochs: 100,
    lr: 0.01,
    batch_size: 32,
  };
  weights := await train(config)?;
  log.info("training complete");
  return ok(());
}
```

### Plugin Example

```mog
// A plugin that the host loads and calls into
// Only needs logging — no file or network access

optional log;

struct PluginInput {
  text: string,
  max_length: int,
}

struct PluginOutput {
  summary: string,
  word_count: int,
  truncated: bool,
}

fn process(input: PluginInput) -> PluginOutput {
  words := input.text.split(" ");

  truncated := false;
  summary_words := words;
  if words.len > input.max_length {
    summary_words = words[0:input.max_length];
    truncated = true;
  }

  summary := summary_words.join(" ");
  if truncated {
    summary = summary + "...";
  }

  return PluginOutput {
    summary: summary,
    word_count: words.len,
    truncated: truncated,
  };
}
```

## Compilation and Execution

### Compilation Phases

1. **Lexing** — tokenization
2. **Parsing** — AST construction
3. **Analysis** — type checking, capability checking, scope resolution
4. **LLVM IR Generation** — typed intermediate representation
5. **Optimization** — LLVM optimization passes
6. **Code Generation** — native binary or shared library

### Embedding Model

The host application:
1. Compiles Mog source to a module
2. Validates capability requirements against granted capabilities
3. Provides capability objects (fs, http, model, etc.) at initialization
4. Calls exported functions (like `main` or `process`)
5. Manages the event loop for async operations
6. Controls resource limits (memory, CPU time, tensor sizes)

```
Host Application
├── Mog Runtime
│   ├── GC (mark-and-sweep)
│   ├── Tensor Engine (dispatches to host ML backend)
│   └── Async Executor (host-managed event loop)
├── Capability Providers
│   ├── fs: FileSystemCapability
│   ├── http: HttpCapability
│   ├── model: ModelCapability
│   └── log: LogCapability
└── Resource Limits
    ├── max_memory: 512MB
    ├── max_cpu_time: 30s
    └── max_tensor_size: 1GB
```

## What This Language Is NOT

To keep the surface area small, the following are explicitly **out of scope**:

- **Raw pointers and manual memory management.** The GC handles it.
- **POSIX syscalls and direct OS access.** The host provides capabilities.
- **Threads and locks.** Use async/await. The host manages concurrency.
- **Inheritance and OOP.** Use structs and functions.
- **Macros.** Keep the language simple and readable.
- **Generics** (beyond tensors). Tensor dtype parameterization is the one generic-like feature. Everything else uses concrete types.
- **Exceptions with stack unwinding.** Use Result types.
- **Operator overloading** (beyond tensor ops). Tensors get arithmetic operators. User types don't.

## Changes from Previous Spec

### Removed
- Raw pointer type (`ptr`) and `gc_alloc`
- All POSIX syscall wrappers (`sys_socket`, `sys_connect`, `sys_send`, etc.)
- Old `soa Name { field: [type] }` syntax (replaced by `datums := soa Struct[N]` backed by existing structs)
- `table` type (replaced by `map` and `struct`)
- Ternary conditional syntax (`condition ? (a) : (b)`) — use `if` expression
- Custom loop patterns with `continue`/`break` in conditional blocks — use `while` and `for`
- i128/i256/u128/u256/f8/f128/f256 types (no hardware support)

### Added
- `string` as first-class type (not `[u8]`)
- `tensor<dtype>` with shape checking and autograd
- `?T` optional type with `none`
- `Result<T>` error handling with `?` propagation
- `async`/`await` with structured concurrency
- `requires`/`optional` capability declarations
- Closures as first-class values
- `for..in` loops with ranges and iterables
- Named function arguments with defaults
- `match` expressions for Result types
- `with` blocks (for `no_grad()` and similar scoped contexts)
- `datums := soa Struct[N]` columnar containers with AoS syntax and SoA storage

### Changed
- Default integer is `int` (64-bit), not `i64`. `i64` still works but is for tensor dtypes.
- Default float is `float` (64-bit), not `f64`. `f64` still works but is for tensor dtypes.
- `:=` for initial binding, `=` for reassignment (was `:=` for both)
- Semicolons are required (were optional in some contexts)
- `print()` is a builtin, not a syscall wrapper
