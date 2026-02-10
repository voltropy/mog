# Mog Implementation Plan

Gap analysis between current implementation and the new `lang_spec.md`, organized into phases that can be implemented incrementally with tests passing at each stage.

---

## Phase 0: Remove Dead Features

**Goal:** Strip out everything the new spec explicitly removes. Reduces surface area before building new features. Tests will break and need updating.

### 0.1 Remove POSIX syscall wrappers
- **Lexer**: Remove `POSIX_CONSTANT` token type
- **Analyzer**: Remove all 42 POSIX filesystem function declarations, 8 socket function declarations
- **Codegen**: Remove `generatePOSIXCall`, `generateSocketCall`, all POSIX/socket LLVM declarations
- **Runtime**: Remove all socket code (`sys_socket`, `sys_connect`, `sys_send`, `sys_recv`, `sys_close`, `sys_fcntl`, `sys_inet_addr`, `sys_errno`, `sys_select`, `fd_zero`, `fd_set_add`, `fd_is_set`, socket constants). Remove POSIX includes and wrappers.
- **Tests**: Remove/update any tests that reference POSIX or socket functions
- **Files**: Delete `.mog` HTTP client example files

### 0.2 Remove raw pointer type
- **Types**: Remove `PointerType` class (or repurpose as internal-only, not user-facing)
- **Lexer**: Remove `ptr` from TYPE token recognition
- **Parser**: Remove `ptr` type annotation support
- **Analyzer**: Remove `gc_alloc` from builtin declarations, remove pointer-specific type checking
- **Codegen**: Remove raw pointer arithmetic codegen (`getelementptr i8`), remove `gc_alloc` as user-callable
- **Tests**: Update any tests using `ptr` type

### 0.3 Remove SoA as user-facing type
- **Lexer**: Remove `soa` keyword
- **Parser**: Remove `SoADeclaration` and `SoALiteral` parsing
- **Analyzer**: Remove `visitSoADeclaration`, `visitSoALiteral`
- **Types**: Remove `SOAType` class (or keep as internal optimizer hint)
- **Codegen**: Remove SoA-specific codegen
- **Tests**: Remove SoA tests

### 0.4 Remove oversized numeric types
- **Types**: Remove `i128`, `i256`, `u128`, `u256`, `f8`, `f128`, `f256` from type constructors
- **Lexer**: Stop recognizing these as TYPE tokens
- **Tests**: Update type tests

### 0.5 Remove ternary syntax
- **Parser**: Remove `ConditionalExpression` (ternary `? :`) parsing
- **Codegen**: Remove ternary codegen (keep `if` expression)
- **Tests**: Update any ternary tests

### 0.6 Remove LLM expression syntax
- **Lexer**: Remove `LLM` keyword
- **Parser**: Remove `LLMExpression` parsing
- **Analyzer**: Remove LLM expression validation
- **Codegen**: Remove `llm_call` generation
- **Runtime**: Remove `llm_call` stub
- The `model.predict()` capability replaces this

### 0.7 Clean up legacy naming
- Remove `table_new`, `table_get`, `table_set` references (already renamed to `map_*` but check for leftovers)
- Remove `print_buffer` (not in new spec)
- Remove `gc_benchmark_stats`, `gc_reset_stats` from user-facing builtins (keep as internal)

**Checkpoint:** All remaining tests pass. The language is smaller.

---

## Phase 1: Core Language Fixes

**Goal:** Align existing features with the new spec's syntax and semantics.

### 1.1 Add `int` and `float` as default type names
- **Lexer**: Recognize `int` and `float` as TYPE tokens (alongside `i64`/`f64`)
- **Types**: Add `int` as alias for `IntegerType("i64")`, `float` as alias for `FloatType("f64")`
- **Parser**: `int` and `float` in type annotations resolve to i64/f64
- **Tests**: Ensure `x: int = 42` and `y: float = 3.14` work

### 1.2 Add `bool` type
- **Types**: Add `BoolType` class (or use `IntegerType("i1")`)
- **Lexer**: Recognize `true`, `false` as keywords; `bool` as TYPE token
- **Parser**: Parse `BooleanLiteral` nodes (already partially exists in parser but rejected by type system)
- **Analyzer**: Handle bool in type checking, conditionals accept bool natively
- **Codegen**: `bool` maps to `i1` in LLVM IR

### 1.3 Add `string` as first-class type
- **Types**: `string` is already `ArrayType(u8, [])` internally. Add `StringType` as a distinct class or keep the alias but ensure `string` works everywhere as a type annotation.
- **Lexer**: `string` already recognized as TYPE token
- **Analyzer**: String methods (`.len`, `.upper()`, etc.) need type checking — defer method implementations to Phase 3
- **Tests**: `s: string = "hello"` works, string assignment/passing works

### 1.4 Add `bf16` float type
- **Types**: Add `bf16` to `FloatKind` union
- **Lexer**: Recognize `bf16` as TYPE token
- **Codegen**: `bf16` maps to `bfloat` in LLVM IR
- Primarily used as tensor dtype, not scalar operations

### 1.5 Fix `:=` vs `=` semantics
- Currently `:=` is the walrus operator used for both declaration and reassignment
- New spec: `:=` for initial binding, `=` for reassignment
- **Parser**: `:=` creates `VariableDeclaration`, `=` creates `Assignment`
- **Analyzer**: `=` on undeclared variable is an error. `:=` on already-declared variable is an error (or shadowing).
- **Tests**: Verify both paths

### 1.6 Add `for..in` with range syntax
- **Lexer**: Recognize `..` as a RANGE token
- **Parser**: Parse `for i in 0..10 { ... }` as `ForRangeLoop` (distinct from `ForEachLoop`)
- **Codegen**: Generate standard counted loop from range
- Also support `for item in array { ... }` and `for i, item in array { ... }`
- **Tests**: Range loops, array iteration with index

### 1.7 Add `if` as expression
- **Parser**: `if` can appear in expression position, returns last expression of taken branch
- **Codegen**: Generate phi nodes or alloca+store pattern for if-expressions
- **Tests**: `x := if cond { 1 } else { 2 }`

### 1.8 Add `**` power operator
- **Lexer**: Recognize `**` as operator token
- **Parser**: Add to precedence chain (higher than multiplicative)
- **Codegen**: Generate `call double @llvm.pow.f64(double, double)` or `call float @powf(float, float)`
- **Tests**: `x := 2.0 ** 3.0` equals 8.0

**Checkpoint:** Core language syntax matches spec. All tests pass.

---

## Phase 2: New Type System Features

**Goal:** Add optional types, Result type, and type aliases.

### 2.1 Optional type `?T`
- **Types**: Add `OptionalType` class wrapping inner type
- **Lexer**: Recognize `?` before type names, `none` as keyword
- **Parser**: Parse `?int`, `?string` etc. in type positions; parse `none` literal
- **Analyzer**: Type check optional assignment, unwrapping
- **Codegen**: Represent as tagged union (i1 flag + value), or as pointer (null = none)
- Add `is some(binding)` pattern check to `if` statements

### 2.2 Result type `Result<T>`
- **Types**: Add `ResultType` class parameterized by success type
- **Parser**: Parse `Result<int>`, `Result<string>`, `Result<()>` in type positions
- **Parser**: Parse `ok(value)` and `err(message)` as constructor expressions
- **Codegen**: Represent as tagged union (i1 flag + value ptr + error string ptr)

### 2.3 `?` propagation operator
- **Parser**: Parse `expression?` as postfix operator
- **Analyzer**: Only valid inside functions returning `Result<T>`
- **Codegen**: Generate: evaluate expression, check if error, if so return the error, else unwrap value

### 2.4 `match` expression
- **Parser**: Parse `match expr { ok(binding) => expr, err(binding) => expr }`
- **Analyzer**: Exhaustiveness check (ok + err must both be present)
- **Codegen**: Generate branch on tag, bind value, evaluate arm

### 2.5 `try`/`catch`
- **Lexer**: Add `try`, `catch` keywords
- **Parser**: Parse `try { ... } catch(e) { ... }`
- **Codegen**: Wrap block, propagate errors, catch binds error message
- This is sugar over Result — not exception-based

### 2.6 Type aliases
- **Parser**: Parse `type Name = Type;` as top-level declaration
- **Analyzer**: Register alias in symbol table, resolve during type checking
- Supports function types: `type Callback = fn(int) -> bool`

**Checkpoint:** Error handling works end-to-end. Tests pass.

---

## Phase 3: Functions and Closures

**Goal:** First-class functions, closures, named arguments, defaults.

### 3.1 Closures (first-class functions)
- **Parser**: Parse `fn(params) -> ReturnType { body }` as expression (lambda)
- **Types**: Add `FunctionType` class with param types and return type
- **Analyzer**: Capture analysis — determine which variables from enclosing scope are referenced
- **Codegen**: Closures as fat pointers (function ptr + environment ptr). Environment is a GC-allocated struct of captured variables.
- **Runtime**: Add closure calling convention support

### 3.2 Named arguments
- **Parser**: Parse `func(positional, name: value, name: value)` — named args after positional
- **Analyzer**: Match named args to parameter names, check types
- **Codegen**: Reorder arguments to match parameter order

### 3.3 Default parameter values
- **Parser**: Parse `param: Type = default_value` in function declarations
- **Analyzer**: Fill in defaults for missing arguments at call site
- **Codegen**: Evaluate default expressions when argument not provided

### 3.4 Array higher-order methods
- `.map(fn)`, `.filter(fn)`, `.sort(fn)`, `.join(separator)`
- **Analyzer**: Type check callback signatures against array element type
- **Codegen**: Generate loops that call the closure for each element
- **Runtime**: Array map/filter/sort/join C implementations (or generate inline)

**Checkpoint:** Closures and HOFs work. Tests pass.

---

## Phase 4: Async/Await

**Goal:** Async functions, await, structured concurrency.

### 4.1 `async fn` declaration
- **Lexer**: Add `async` keyword
- **Parser**: Parse `async fn` — identical to `fn` but marked async
- **Types**: Async functions return `Future<T>` internally (wrapping their declared return type)
- **Analyzer**: `await` only valid inside `async fn`. Async functions must return `Result<T>`.

### 4.2 `await` expression
- **Parser**: Parse `await expression` as prefix operator
- **Codegen**: Generate coroutine suspension point. The function is split into a state machine at each await point.
- **Runtime**: Add coroutine/continuation support. Host provides event loop integration hooks.

### 4.3 `all()` and `race()` builtins
- **Analyzer**: Accept array of async expressions, return array of results (all) or single result (race)
- **Codegen**: Generate parallel task spawning through host runtime

### 4.4 Host event loop integration
- **Runtime**: Define C API for host to drive async execution:
  - `mog_create_context()` — create execution context
  - `mog_poll(ctx)` — advance coroutines until all blocked
  - `mog_complete(ctx, task_id, result)` — deliver async result from host
  - `mog_is_done(ctx)` — check if all tasks complete

**Checkpoint:** Async works for simple sequential await chains. Tests pass.

---

## Phase 5: Capability System

**Goal:** `requires`/`optional` declarations, capability checking.

### 5.1 Capability declarations
- **Lexer**: Add `requires`, `optional` keywords
- **Parser**: Parse `requires cap1, cap2;` and `optional cap1, cap2;` as top-level declarations
- **Analyzer**: Track declared capabilities. Error if code uses `cap.method()` without declaring `cap`.

### 5.2 Capability objects
- Capability names (`fs`, `http`, `model`, `log`, `env`, `db`) are not keywords — they're identifiers injected by the host
- **Analyzer**: When a MemberExpression's object matches a declared capability, validate the method exists in the capability's interface
- **Codegen**: Capability method calls generate FFI calls to host-provided function pointers

### 5.3 Host-side capability API
- **Runtime**: Define C API for host to register capabilities:
  - `mog_register_capability(ctx, name, vtable)` — provide capability implementation
  - Each capability is a struct of function pointers
  - `mog_validate_capabilities(module)` — check required caps are available

**Checkpoint:** Scripts declare and use capabilities. Host can grant/deny. Tests pass.

---

## Phase 6: Tensor Type

**Goal:** `tensor<dtype>` as first-class type with operations.

This is the largest phase and should be broken into sub-phases.

### 6.1 Tensor type and creation
- **Types**: Add `TensorType` class with dtype parameter
- **Parser**: Parse `tensor<f32>`, `tensor([1.0, 2.0])`, `tensor<f16>.zeros([3, 3])`
- **Analyzer**: Type check tensor operations, shape inference
- **Codegen**: Tensors are opaque pointers to host-managed tensor objects
- **Runtime**: Tensor operations dispatch to host ML backend via capability-like FFI
  - `mog_tensor_zeros(dtype, shape, ndim)` -> tensor handle
  - `mog_tensor_from_data(dtype, data, shape, ndim)` -> tensor handle

### 6.2 Tensor indexing and slicing
- **Parser**: Multi-dimensional indexing `t[0, :]`, `t[:, 0:10, :]`
- **Codegen**: Generate calls to `mog_tensor_index` / `mog_tensor_slice`

### 6.3 Tensor elementwise operations
- **Analyzer**: Overload `+`, `-`, `*`, `/`, `**` for tensor operands
- **Codegen**: Generate calls to `mog_tensor_add`, `mog_tensor_mul`, etc.
- Broadcasting rules in analyzer

### 6.4 Tensor shape operations
- `.reshape()`, `.transpose()`, `.squeeze()`, `.unsqueeze()`, `.flatten()`, `.view()`, `.expand()`, `.contiguous()`
- **Analyzer**: Method resolution on tensor type
- **Codegen**: Generate calls to `mog_tensor_reshape`, etc.

### 6.5 Tensor reduction operations
- `.sum()`, `.mean()`, `.max()`, `.min()`, `.argmax()`, `.argmin()`, `.prod()`, `.any()`, `.all()`
- Optional `dim` parameter

### 6.6 Linear algebra builtins
- `matmul(a, b)`, `dot(a, b)`, `norm(a)`, `norm(a, p)`, `cross(a, b)`
- **Analyzer**: Shape checking for matmul compatibility
- **Codegen**: Dispatch to `mog_tensor_matmul`, etc.

### 6.7 ML operations
- Activations: `relu`, `gelu`, `silu`, `sigmoid`, `tanh`, `softmax`
- Normalization: `layer_norm`, `batch_norm`, `group_norm`
- Convolution: `conv1d`, `conv2d`
- Pooling: `max_pool2d`, `avg_pool2d`
- Loss: `cross_entropy`, `mse_loss`, `binary_cross_entropy`
- Attention: `scaled_dot_product_attention`
- Dropout: `dropout`
- Embedding: `embedding`
- All generate FFI calls to host tensor backend

### 6.8 Autograd
- `.requires_grad()` — mark tensor for gradient tracking
- `.backward()` — backpropagation
- `.grad` — access gradient tensor
- `.item()` — extract scalar value
- `with no_grad() { ... }` — scoped gradient disable
- **Codegen**: These are method calls on tensor handles dispatched to host backend

### 6.9 Dtype conversion
- `.to(dtype)` — `t.to(f16)`, `t.to(f32)`
- **Codegen**: `mog_tensor_to_dtype(handle, dtype)`

**Checkpoint:** Tensor creation, operations, and autograd work through host FFI. Tests pass.

---

## Phase 7: String and Math Builtins

**Goal:** Complete string methods and math builtins.

### 7.1 String methods
Currently have: `string_length`, `string_concat`, `string_char_at`, `string_slice`

Need to add:
- `.len` (property, not method — may already work via array length)
- `.upper()`, `.lower()` — new runtime functions
- `.trim()` — new runtime function
- `.split(delimiter)` — new runtime function, returns `[string]`
- `.contains(substring)` — new runtime function, returns `bool`
- `.starts_with(prefix)`, `.ends_with(suffix)` — new runtime functions
- `.replace(old, new)` — new runtime function
- `str(value)` — conversion builtin (partially exists as `i64_to_string` etc.)
- `int(string)`, `float(string)` — parse builtins returning `Result`

### 7.2 Math builtins
Currently have: none as language builtins (only in tensor path)

Need to add all as scalar builtins calling `libm`:
- `abs`, `sqrt`, `pow`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan2`
- `exp`, `log`, `log2`, `floor`, `ceil`, `round`
- `min`, `max`
- `PI`, `E` constants

**Checkpoint:** All string/math builtins work. Tests pass.

---

## Phase 8: Polish

### 8.1 Multi-line comments
- **Lexer**: Add `/* ... */` support (currently only `//` and `#`)

### 8.2 `with` blocks
- **Parser**: Parse `with expr() { ... }` as scoped context
- Primarily for `no_grad()` but generalizable

### 8.3 `print()` cleanup
- `print()` should accept multiple arguments of any type
- Currently dispatches to `print_i64`, `print_f64`, etc. based on type
- Make `print(a, b, c)` work with space-separated output

### 8.4 Remove `#` comments
- Old spec used `#`, new spec only has `//` and `/* */`

### 8.5 `parse_json()` builtin
- Returns `Result<T>` (used in examples)
- May be host-provided or builtin

---

## Dependency Order

```
Phase 0 (Remove) ──> Phase 1 (Core fixes) ──> Phase 2 (Result/Optional)
                                                       │
                                            Phase 3 (Closures) ──> Phase 4 (Async)
                                                       │
                                            Phase 5 (Capabilities)
                                                       │
                                            Phase 6 (Tensors)
                                                       │
                                            Phase 7 (Builtins) ──> Phase 8 (Polish)
```

Phases 0-2 are strictly sequential (each builds on prior).
Phase 3 requires Phase 2 (closures need to work with Result types).
Phase 4 requires Phase 3 (async functions are closures under the hood).
Phase 5 can start after Phase 2 (capabilities return Results).
Phase 6 can start after Phase 5 (tensors dispatch through host capabilities).
Phase 7 can happen any time after Phase 2.
Phase 8 is independent cleanup.

---

## Estimated Effort

| Phase | Description | Size | Risk |
|-------|-------------|------|------|
| 0 | Remove dead features | Medium | Low — mostly deletion |
| 1 | Core language fixes | Medium | Low — extending existing patterns |
| 2 | Optional/Result/match | Large | Medium — new type system concepts |
| 3 | Closures/HOFs | Large | High — closure codegen is complex |
| 4 | Async/await | Very Large | High — coroutine transformation |
| 5 | Capability system | Medium | Medium — new concept but simple FFI |
| 6 | Tensor type | Very Large | High — massive API surface |
| 7 | String/math builtins | Small | Low — straightforward runtime additions |
| 8 | Polish | Small | Low — minor cleanups |

**Total: ~6-8 major implementation rounds.**

Phase 0+1 gets you a cleaner, working language.
Phase 2+3 makes it genuinely useful for scripting.
Phase 4+5 makes it embeddable with async host integration.
Phase 6 makes it ML-capable.
