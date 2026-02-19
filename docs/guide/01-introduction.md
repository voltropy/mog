# Chapter 1: Introduction

Mog is a small, statically-typed, embeddable programming language. It compiles to native code through LLVM (and a lightweight QBE backend), runs inside a host application, and is designed for one job: giving LLM agents and automation scripts a language that is safe to execute, fast enough for real work, and small enough for a model to hold in its context window.

If you have written code in Rust, Go, or TypeScript, Mog's syntax will feel familiar. If you have embedded Lua or Wren in an application, Mog's execution model will make sense immediately. The difference is that Mog is statically typed, compiles to native code, and enforces a capability-based security model that makes it safe to run untrusted code.

## What Mog Is For

Mog targets a specific set of use cases:

- **LLM agent tool-use scripts.** An agent generates a Mog script, the host compiles and runs it in a sandbox, and the script can only access capabilities the host explicitly grants. No container needed.
- **Plugin and extension scripting.** A host application exposes functionality through capabilities, and third-party scripts extend the application without being able to crash it or access the filesystem behind its back.
- **ML workflow orchestration.** Tensors are a built-in data type with hardware-relevant dtypes like `f16` and `bf16`. The host provides ML operations (matmul, activations, autograd) through capabilities, so the script describes *what* to compute and the host decides *where* to run it.
- **Short automation scripts.** Configuration, data transformation, and glue code where you want static types and native speed without the weight of a general-purpose language.

Here is what a small Mog program looks like — an agent tool script that searches an API and filters results:

```mog
requires http, log;

async fn search(query: string) -> Result<string[]> {
  response := await http.get("/api/search?q={query}")?;
  results := parse_results(response);
  log.info("found {results.len} results for '{query}'");
  return ok(results);
}

async fn main() -> int {
  results := await search("machine learning")?;
  top := results.filter(fn(r) { r.score > 0.8 });
  for r in top {
    print("{r.title}: {r.url}");
  }
  return 0;
}
```

The `requires http, log` declaration tells the host what capabilities this script needs. If the host doesn't provide them, the script won't run. The script itself has no way to access the network or filesystem directly — everything goes through the host.

## Design Philosophy

Mog is built around six principles. They explain why the language looks the way it does, and why many features you might expect are deliberately absent.

**1. Small surface area.** The entire language should fit in an LLM's context window. Every feature must justify its existence. When in doubt, leave it out. This is why Mog has no macros, no generics (beyond tensor dtype parameterization), no inheritance, and no operator overloading. A smaller language is easier to learn, easier to generate, and easier to reason about.

**2. Predictable semantics.** No implicit coercion surprises, no operator precedence puzzles, no hidden control flow. Code reads top-to-bottom, left-to-right. When you see an expression like `a + b`, the types of `a` and `b` determine exactly what happens — there is no overloading, no implicit conversion from string to int, no surprising promotion rules.

**3. Familiar syntax.** Curly braces, `fn`, `->`, `:=`. A blend of Rust, Go, and TypeScript that LLMs already generate fluently. Mog introduces no novel syntax without strong justification. If you can read any of those languages, you can read Mog.

**4. Safe by default.** Garbage collected, bounds-checked, no null, no raw pointers. A Mog script cannot crash the host process, corrupt memory, or access resources outside its sandbox. The runtime includes a mark-and-sweep garbage collector, array bounds checking, and cooperative interrupt polling at loop back-edges so the host can terminate runaway scripts.

**5. Host provides I/O.** The language has no built-in file, network, or system access. All side effects go through capabilities explicitly granted by the embedding host. This is the foundation of Mog's security model — a script can only do what the host lets it do.

**6. Tensors as data, ML as capability.** The language provides n-dimensional arrays (tensors) as a built-in data structure with element-level read/write and shape manipulation. All ML operations — matmul, activations, loss functions, autograd — are provided by the host through capabilities. The language gives you the data structure; the host gives you the compute. This means the host can route tensor operations to CPU, GPU, or a remote accelerator without the script needing to know.

## How Mog Compares to Other Embeddable Languages

If you have used other embeddable languages, here is how Mog differs:

**Lua** is dynamically typed, interpreted (or JIT-compiled via LuaJIT), and has a minimal core. Mog shares Lua's philosophy of being small and embeddable, but adds static types, compiles to native code ahead of time, and enforces capability-based security rather than relying on environment sandboxing. Lua's flexibility is a strength for interactive scripting; Mog trades that flexibility for compile-time safety and native performance.

**Wren** is a class-based, dynamically-typed embeddable language with a clean syntax. Like Mog, it is designed for embedding in host applications. The key differences are that Mog is statically typed, compiles ahead of time, has no classes or inheritance, and provides a formal capability model for security. Wren's object-oriented design makes it natural for game scripting; Mog's functional style with explicit capabilities makes it natural for agent scripting and ML workflows.

**Rhai** is a scripting language for Rust applications, dynamically typed, interpreted. Mog targets a similar embedding scenario but takes a different approach: static types catch errors before execution, LLVM compilation produces native-speed code, and the capability system provides security guarantees that a dynamic language cannot offer at compile time.

The common thread: Mog is the statically-typed, ahead-of-time-compiled option in the embeddable language space. It pays for this with a compilation step, but gains type safety, native performance, and a security model that is enforced before the script runs.

## The Capability-Based Security Model

Security in Mog is not an afterthought bolted onto the runtime — it is a structural property of the language. A Mog script declares the capabilities it needs at the top of the file:

```mog
requires fs, log;        // these must be provided or the script won't run
optional env;             // this is used if available, ignored if not
```

The host application decides which capabilities to grant:

```c
MogVM *vm = mog_vm_new();
mog_register_capability(vm, "fs", fs_functions, 6);
mog_register_capability(vm, "log", log_functions, 4);
```

If a script declares `requires http` but the host doesn't provide the `http` capability, the script is rejected before it executes. There is no way for the script to discover or access capabilities that weren't explicitly registered.

This means you can look at a Mog script's `requires` declarations and know exactly what it can do. A script that says `requires log` can write log messages and nothing else — it cannot read files, make network requests, or access the filesystem. This property is enforced by the compiler and runtime together, not by convention.

Standard capabilities that hosts commonly provide include:

| Capability | What it provides |
|---|---|
| `fs` | File read/write/exists/remove |
| `http` | HTTP requests |
| `log` | Structured logging (info, warn, error, debug) |
| `env` | Environment info, timestamps, random numbers |
| `process` | Sleep, environment variables, working directory |
| `ml` | ML operations (matmul, activations, autograd) |
| `model` | LLM inference |
| `db` | Database queries |

A host can also define custom capabilities — there is nothing special about the standard ones. They are just conventions.

## What Mog Is Not

Mog is deliberately not many things. Each omission keeps the language small, the security model tractable, and the compilation fast:

- **Not a systems language.** No raw pointers, no manual memory management, no POSIX syscalls, no direct OS access.
- **Not standalone.** Mog is always embedded in a host application. There is no standard library for file I/O or networking — the host provides everything.
- **Not general-purpose.** Mog is for scripts, plugins, and orchestration. It is not designed for building web servers, operating systems, or databases.
- **Not object-oriented.** No classes, no inheritance, no methods on types. Structs hold data; functions operate on data. Higher-order functions and closures provide the abstraction mechanisms.
- **No macros or metaprogramming.** The language you see is the language that runs. No code generation, no compile-time evaluation, no syntax extensions.
- **No generics.** Beyond tensor dtype parameterization (`tensor<f32>`, `tensor<f16>`), there are no generic types or functions. This keeps the type system simple and the compiler small.
- **No exceptions with stack unwinding.** Error handling uses `Result<T>` with explicit propagation via `?`. Errors are values, not control flow.
- **No threads or locks.** Concurrency is cooperative via `async`/`await`, with the host managing the event loop.

If you need any of these features, Mog is probably not the right language for your use case — and that's fine. Mog is designed to do a few things well rather than everything adequately.

## What's Ahead

The rest of this guide walks through the language feature by feature, with runnable examples at every step. Chapter 2 starts with the simplest possible program and builds up from there. By the end, you will be able to write Mog scripts that use the full language: variables, functions, closures, structs, arrays, maps, error handling, async operations, tensors, and host capabilities.

Let's write some code.
