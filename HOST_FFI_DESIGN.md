# Mog Host FFI Design

How the host application registers functions that Mog scripts can call.

## The Design Problem

Mog is **statically typed** but **embedded** — the host registers functions at runtime. This creates a tension that Lua/Wren don't face (they're dynamically typed) and that Nelua doesn't face (it's compile-time only).

We need to solve:
1. **Type safety**: The Mog compiler must know the types of host functions at compile time, but the host provides them at runtime.
2. **Ergonomic registration**: The host (written in C, Rust, Python, etc.) should be able to register functions with minimal boilerplate.
3. **Security**: Scripts can only call functions the host explicitly grants. The capability system (`requires`/`optional`) controls this.
4. **Performance**: Host function calls should be fast — ideally a direct function pointer call, not stack/slot manipulation.

## How Lua Does It (and what we learn)

Lua's approach: every host function has the signature `int fn(lua_State *L)`. Arguments arrive on a stack. The function pops arguments, pushes results. Registration is just storing the function pointer in a table.

**What's great about it:**
- Dead simple to register: `lua_register(L, "sin", l_sin)` — one line
- Flexible: any function can accept any number of arguments of any type
- Libraries are just tables of function pointers (`luaL_Reg` arrays)
- Userdata + metatables give OOP-style method dispatch

**What we can't copy:**
- The stack-based marshaling model exists because Lua is dynamically typed. Mog already knows types at compile time — we should use that.
- `luaL_checknumber(L, 1)` is a runtime type check. We want compile-time type errors.
- Lua's approach means every host function has identical C signature, sacrificing type information. We can do better.

## The Design: Type-Declared FFI

### Core Idea

The host provides **two things**:
1. A **type declaration** (at compile time, via a declaration file or header)
2. A **function pointer** (at runtime, during initialization)

The compiler sees the type declarations and type-checks calls against them. The runtime links function pointers at initialization. If a declared function isn't provided, the runtime errors before execution begins.

### Part 1: Declaration Files (`.mogdecl`)

The host writes a declaration file describing what it exposes. This is like a C header or a TypeScript `.d.ts`:

```
// fs.mogdecl — File system capability

capability fs {
  fn read(path: string) -> async Result<string>;
  fn write(path: string, content: string) -> async Result<()>;
  fn list(path: string) -> async Result<[string]>;
  fn exists(path: string) -> async Result<bool>;
  fn mkdir(path: string) -> async Result<()>;
  fn remove(path: string) -> async Result<()>;
}
```

```
// http.mogdecl — HTTP capability

struct HttpResponse {
  status: int,
  body: string,
  headers: {string: string},
}

capability http {
  fn get(url: string, params: ?{string: string}, headers: ?{string: string}) -> async Result<HttpResponse>;
  fn post(url: string, body: ?string, headers: ?{string: string}) -> async Result<HttpResponse>;
}
```

```
// model.mogdecl — ML model capability

capability model {
  fn predict(prompt: string, context: ?[string]) -> async Result<string>;
  fn embed(texts: [string]) -> async Result<tensor<f32>>;
  fn next_batch(batch_size: int) -> async Result<{string: tensor<f32>}>;
}
```

```
// log.mogdecl — Logging capability (sync, no Result)

capability log {
  fn info(message: string);
  fn warn(message: string);
  fn error(message: string);
  fn debug(message: string);
}
```

**Key properties:**
- Functions can be `async` or sync
- Functions can return `Result<T>` or plain types
- Capabilities can define helper structs
- Named and optional parameters supported
- This file ships with the host application, not the script

### Part 2: Host Registration (C API)

The host initializes a Mog VM and registers capability implementations. Taking heavy inspiration from Lua's simplicity:

```c
#include "mog.h"

// Every host function has a uniform signature, like Lua.
// But instead of a stack, we use a typed context.
typedef MogValue (*MogHostFn)(MogVM *vm, MogArgs *args);

// MogArgs provides type-safe argument access.
// The compiler already verified types — these are just extraction.
// They abort on type mismatch (programmer error in the host, not the script).
int64_t       mog_arg_int(MogArgs *args, int index);
double        mog_arg_float(MogArgs *args, int index);
bool          mog_arg_bool(MogArgs *args, int index);
const char   *mog_arg_string(MogArgs *args, int index);
MogValue      mog_arg_value(MogArgs *args, int index);  // generic
bool          mog_arg_present(MogArgs *args, int index); // for optional params

// Return value construction
MogValue mog_ok(MogVM *vm, MogValue value);
MogValue mog_err(MogVM *vm, const char *message);
MogValue mog_int(int64_t value);
MogValue mog_float(double value);
MogValue mog_bool(bool value);
MogValue mog_string(MogVM *vm, const char *s);
MogValue mog_none(void);

// For async functions, the host returns a pending value
// and completes it later:
MogFuture *mog_future_new(MogVM *vm);
void       mog_future_resolve(MogFuture *f, MogValue value);
void       mog_future_reject(MogFuture *f, const char *error);
MogValue   mog_pending(MogFuture *f);
```

**Registering a capability:**

```c
// The Lua-inspired pattern: array of name+function pairs
typedef struct {
    const char *name;
    MogHostFn   func;
} MogCapEntry;

// Register a capability (like luaL_newlib)
void mog_register_capability(MogVM *vm, const char *name, const MogCapEntry *entries);

// Example: registering the 'log' capability
static MogValue log_info(MogVM *vm, MogArgs *args) {
    const char *msg = mog_arg_string(args, 0);
    printf("[INFO] %s\n", msg);
    return mog_none();
}

static MogValue log_error(MogVM *vm, MogArgs *args) {
    const char *msg = mog_arg_string(args, 0);
    fprintf(stderr, "[ERROR] %s\n", msg);
    return mog_none();
}

static const MogCapEntry log_cap[] = {
    {"info",  log_info},
    {"warn",  log_warn},
    {"error", log_error},
    {"debug", log_debug},
    {NULL,    NULL}
};

// In host initialization:
MogVM *vm = mog_vm_new();
mog_register_capability(vm, "log", log_cap);
```

**Registering an async capability:**

```c
static MogValue fs_read(MogVM *vm, MogArgs *args) {
    const char *path = mog_arg_string(args, 0);

    // Create a future that the script will await
    MogFuture *future = mog_future_new(vm);

    // Kick off async work on host's event loop
    host_async_read(path, ^(char *content, char *error) {
        if (error) {
            mog_future_reject(future, error);
        } else {
            mog_future_resolve(future, mog_string(vm, content));
        }
    });

    return mog_pending(future);
}

static const MogCapEntry fs_cap[] = {
    {"read",  fs_read},
    {"write", fs_write},
    {"list",  fs_list},
    {NULL,    NULL}
};

mog_register_capability(vm, "fs", fs_cap);
```

### Part 3: How It All Connects

**Compile time:**
1. Script says `requires fs, log;`
2. Compiler loads `fs.mogdecl` and `log.mogdecl` from a known search path
3. Compiler type-checks all calls to `fs.read(...)`, `log.info(...)`, etc.
4. Calls to capability functions compile to indirect calls through a capability dispatch table

**Runtime initialization:**
1. Host creates VM: `mog_vm_new()`
2. Host registers capabilities: `mog_register_capability(vm, "fs", fs_entries)`
3. Host loads compiled module: `mog_load(vm, bytecode, len)`
4. Runtime validates: does the module `require` any capabilities that weren't registered? If so, error before execution.
5. Runtime populates the dispatch table with registered function pointers

**Runtime call path:**
1. Script calls `fs.read("data.csv")`
2. Compiled code loads the dispatch table entry for `fs.read`
3. Calls through function pointer: `cap_table[FS_READ](vm, args)`
4. Host function runs, returns `MogValue`
5. For async: returns `mog_pending(future)`, script suspends, host resolves future later

### Part 4: Userdata (Host Objects)

For host-managed objects that scripts need to hold references to (database connections, model handles, tensor objects), we use **opaque handles** — inspired by Lua's userdata but with static types:

```
// In tensor.mogdecl:
opaque TensorHandle;

capability tensor_ops {
  fn zeros(dtype: string, shape: [int]) -> TensorHandle;
  fn add(a: TensorHandle, b: TensorHandle) -> TensorHandle;
  fn matmul(a: TensorHandle, b: TensorHandle) -> TensorHandle;
  fn backward(t: TensorHandle);
  fn grad(t: TensorHandle) -> TensorHandle;
  fn to_list(t: TensorHandle) -> [float];
}
```

On the C side:
```c
// Create a handle wrapping a host pointer
MogValue mog_handle_new(MogVM *vm, const char *type_name, void *ptr,
                         void (*destructor)(void *));

// Extract the host pointer (aborts if wrong type)
void *mog_handle_get(MogArgs *args, int index, const char *type_name);
```

The `type_name` acts like Lua's metatable name — it prevents one opaque type from being confused with another. The destructor is called by the GC (like Lua's `__gc` metamethod, but host-only — scripts can never call it).

### Part 5: Security Properties

**Capability isolation (like Lua's environment tables):**
- A script can only call functions from declared capabilities
- The compiler enforces this statically — no capability access = no code generation for it
- At runtime, missing capabilities cause a clean error before any code runs

**No ambient authority (unlike Lua's default):**
- Lua starts with all of `_G` available. You have to remove things.
- Mog starts with nothing. You have to add things.
- This is whitelist-by-default, like Lua's "empty environment" sandbox pattern but enforced by the compiler.

**No escape routes:**
- No `eval`/`loadstring` — scripts can't load code at runtime
- No debug/reflection APIs — scripts can't inspect the runtime
- No raw memory access — no pointers, no `gc_alloc`
- Opaque handles can't be cast or inspected by scripts
- Handles have type names checked at extraction — cross-type confusion is impossible

**Resource limits (like Lua's hooks + custom allocator):**
```c
MogLimits limits = {
    .max_memory = 512 * 1024 * 1024,  // 512 MB
    .max_cpu_ms = 30000,               // 30 seconds
    .max_stack_depth = 1024,
};
mog_vm_set_limits(vm, &limits);
```

## Comparison to Lua

| Aspect | Lua | Mog |
|--------|-----|-----|
| Function signature | `int fn(lua_State *L)` (uniform) | `MogValue fn(MogVM *vm, MogArgs *args)` (uniform) |
| Argument access | Stack-based, numbered from bottom | Indexed args, position matches declaration |
| Type checking at boundary | Runtime only (`luaL_checknumber`) | Compile-time (declaration files) + runtime abort for host bugs |
| Registration | `lua_register` / `luaL_newlib` | `mog_register_capability` with `MogCapEntry[]` |
| Namespacing | Tables (ad-hoc) | Capabilities (enforced) |
| Object wrapping | Userdata + metatables | Opaque handles + type names |
| Sandbox model | Remove things from `_G` | Start with nothing, add capabilities |
| Async | Coroutines (cooperative) | `async`/`await` with host event loop |
| Lines to register a function | ~3 (push + setglobal) | ~3 (entry in array + implement) |

## What This Design Deliberately Omits

- **Automatic marshaling / code generation.** No `bindgen`-style tool that reads C headers. Host authors write declaration files and implement the `MogHostFn` signature. This is simple enough that auto-generation isn't worth the complexity.

- **Method syntax on opaque handles.** You write `tensor_ops.add(a, b)`, not `a.add(b)`. This keeps the capability boundary visible and explicit. (The `tensor<dtype>` built-in type in the spec has method syntax, but that's the compiler's sugar over these FFI calls — the host sees capability function calls.)

- **Callbacks from host into script.** The initial design is one-directional: script calls host. If the host needs to call back into the script (e.g., for event handlers), that's a Phase 2 feature using exported functions.

- **Variadic arguments.** Every function has a fixed signature declared in the `.mogdecl`. This keeps the type checking simple and the codegen fast.
