# Chapter 15b: Plugins — Dynamic Loading of Mog Code

Chapter 15 covered embedding: the host compiles and runs a Mog script directly. Plugins take a different approach — you compile `.mog` files into shared libraries (`.dylib` on macOS, `.so` on Linux) ahead of time, then load them at runtime with `dlopen`. The host never sees the source code. It loads a binary, queries what functions are available, and calls them.

This is the right model when you need hot-swappable logic, third-party extensions, or a modular architecture where components are developed and compiled independently.

## Plugin Overview

A plugin is a pre-compiled Mog shared library. It contains:

- One or more exported functions callable from C
- A built-in copy of the Mog runtime (GC, value representation, etc.)
- Metadata: plugin name, version, and export table

The key difference from direct embedding:

| | Embedding (Ch. 15) | Plugins (this chapter) |
|---|---|---|
| Source visible to host? | Yes | No |
| Compilation happens | At load time | Ahead of time |
| Swappable at runtime? | Requires recompile | Just replace the `.dylib` |
| Distribution | Ship `.mog` files | Ship binaries |

Use embedding when you control the scripts and want simplicity. Use plugins when you want pre-compiled, distributable, hot-swappable modules.

## Writing a Plugin

A plugin is a regular `.mog` file. The only difference is that you mark exported functions with `pub`:

```mog
// math_plugin.mog — Math utilities plugin

pub fn fibonacci(n: int) -> int {
  if (n <= 1) { return n; }
  return fibonacci(n - 1) + fibonacci(n - 2);
}

pub fn factorial(n: int) -> int {
  if (n <= 1) { return 1; }
  return n * factorial(n - 1);
}

pub fn gcd(a: int, b: int) -> int {
  x := a;
  y := b;
  while (y != 0) {
    t := x % y;
    x = y;
    y = t;
  }
  return x;
}

// Internal helper — NOT exported (no `pub` keyword)
fn square(x: int) -> int {
  return x * x;
}

pub fn sum_of_squares(a: int, b: int) -> int {
  return square(a) + square(b);
}
```

`pub` functions become symbols in the shared library that the host can call by name. Functions without `pub` are compiled with hidden linkage — they exist in the binary but are not visible to the loader.

Any top-level statements in the file run during plugin initialization, before the host calls any exported function. Use this for setup work like populating lookup tables.

## Compiling a Plugin

Use the `compilePluginToSharedLib()` API from TypeScript/Bun:

```typescript
import { compilePluginToSharedLib } from './src/compiler.ts';
import { readFileSync } from 'fs';

const source = readFileSync('math_plugin.mog', 'utf-8');
const result = await compilePluginToSharedLib(
  source,
  'math_plugin',         // plugin name
  'math_plugin.dylib',   // output path
  '1.0.0'                // version
);

if (result.errors.length > 0) {
  for (const e of result.errors) {
    console.error(`[${e.line}:${e.column}] ${e.message}`);
  }
}
```

The compiler runs the full pipeline: Mog source → LLVM IR → position-independent object code → shared library. The resulting `.dylib` is self-contained — it includes the Mog runtime, so the host doesn't need to link against anything beyond `mog_plugin.h`.

## Loading and Calling Plugins from C

Include `mog_plugin.h` alongside the standard `mog.h` header.

```c
#include <stdio.h>
#include "mog.h"
#include "mog_plugin.h"

int main(int argc, char **argv) {
    // Create a VM (required even for capability-free plugins)
    MogVM *vm = mog_vm_new();
    mog_vm_set_global(vm);

    // Load the plugin
    MogPlugin *plugin = mog_load_plugin("./math_plugin.dylib", vm);
    if (!plugin) {
        fprintf(stderr, "Failed: %s\n", mog_plugin_error());
        return 1;
    }

    // Inspect plugin metadata
    const MogPluginInfo *info = mog_plugin_get_info(plugin);
    printf("Plugin: %s v%s (%lld exports)\n",
           info->name, info->version, (long long)info->num_exports);

    // Call exported functions
    MogValue args[] = { mog_int(10) };
    MogValue result = mog_plugin_call(plugin, "fibonacci", args, 1);
    printf("fibonacci(10) = %lld\n", result.data.i);  // 55

    // Multiple arguments
    MogValue gcd_args[] = { mog_int(48), mog_int(18) };
    MogValue gcd_result = mog_plugin_call(plugin, "gcd", gcd_args, 2);
    printf("gcd(48, 18) = %lld\n", gcd_result.data.i);  // 6

    // Error handling for unknown functions
    MogValue bad = mog_plugin_call(plugin, "nonexistent", NULL, 0);
    if (bad.tag == MOG_ERROR) {
        printf("Error: %s\n", bad.data.error);
    }

    // Cleanup
    mog_unload_plugin(plugin);
    mog_vm_free(vm);
    return 0;
}
```

The pattern mirrors the embedding lifecycle from Chapter 15: create VM, load, use, free. The difference is that `mog_load_plugin` replaces the compile-and-run step — the code is already compiled.

## Plugin C API Reference

| Function | Purpose |
|---|---|
| `mog_load_plugin(path, vm)` | Load a `.dylib`/`.so` plugin |
| `mog_load_plugin_sandboxed(path, vm, caps)` | Load with capability allowlist |
| `mog_plugin_call(plugin, name, args, nargs)` | Call an exported function by name |
| `mog_plugin_get_info(plugin)` | Get plugin metadata (name, version, exports) |
| `mog_plugin_error()` | Get last error message |
| `mog_unload_plugin(plugin)` | Unload and free plugin |

Arguments and return values use the same `MogValue` tagged union described in Chapter 15. Construct arguments with `mog_int()`, `mog_string()`, etc., and inspect results through the `.tag` and `.data` fields.

## Capability Sandboxing

Plugins can use `requires` declarations just like regular Mog programs. When a plugin declares `requires fs`, it means it will attempt to call filesystem functions during execution.

`mog_load_plugin` allows all capabilities. If you're loading untrusted code, use `mog_load_plugin_sandboxed` instead — it takes a `NULL`-terminated allowlist of capability names. If the plugin requires a capability not on the list, loading fails immediately:

```c
// Only allow 'log' capability — reject plugins needing 'fs', 'http', etc.
const char *allowed[] = { "log", NULL };
MogPlugin *plugin = mog_load_plugin_sandboxed("./untrusted.dylib", vm, allowed);
if (!plugin) {
    // Plugin requires a capability we don't allow
    fprintf(stderr, "%s\n", mog_plugin_error());
}
```

This is the same capability model from Chapter 14, applied at load time. The plugin's `requires` declarations are checked against the allowlist before any code runs. A plugin that passes the check cannot later escalate to capabilities it didn't declare.

## Plugin Protocol (Advanced)

Under the hood, the compiler generates four symbols in every plugin shared library:

- **`mog_plugin_info()`** — returns a pointer to a static `MogPluginInfo` struct containing the plugin name, version, and export count.
- **`mog_plugin_init(MogVM*)`** — initializes the GC, wires up the VM, and runs any top-level statements in the source file.
- **`mog_plugin_exports(int*)`** — returns an array of `{name, func_ptr}` pairs and writes the count to the output parameter.
- **Exported wrappers** — each `pub fn foo(...)` gets a `mogp_foo` wrapper with default (visible) linkage. Internal functions are emitted with `internal` linkage so they exist in the binary but are invisible to `dlopen`/`dlsym`.

`mog_load_plugin` calls these in order: resolve `mog_plugin_info` to validate compatibility, call `mog_plugin_init` to set up the runtime, then call `mog_plugin_exports` to build the dispatch table. After that, `mog_plugin_call` is just a name lookup and function pointer call.

You don't need to know any of this to use plugins. It's documented here so you can debug issues, write tooling, or implement plugin loaders in languages other than C.
