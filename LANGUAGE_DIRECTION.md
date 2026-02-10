# Mog Language Direction: Embedded ML Language for LLM Agents

## The Pitch

An embedded language that's good for ML, easy for LLMs to learn, read, and write. Something an LLM might use in a coding agent to set up workflows, replace PyTorch, write short scripts, and build plugins. Deliberately small and regular so LLMs can generate it correctly.

## What "easy for LLMs" actually means in practice

LLMs learn from training data, so "similar to other languages" is the strongest lever. The current syntax (curly braces, `:=` assignment, `fn`, `->` return types) is a reasonable blend of Rust/Go/Lua. Good.

But "few features" means you need to be ruthless about cutting. Every feature an LLM has to remember is a chance for it to hallucinate wrong syntax. Right now you have: structs, SoA, AoS, maps, arrays, raw pointers, POSIX syscalls, socket wrappers, GC allocation, multiple integer widths (i8-i64, u8-u64), six float types (f8-f256). That's a lot of surface area.

## What "embedded + ML + agent scripting" implies

This is three use cases that actually align well:

1. **Agent tool-use scripts** - short, imperative, call external APIs
2. **ML workflows** - tensor ops, model loading, training loops
3. **Plugin system** - sandboxed, safe, predictable resource usage

The common thread: **a small, safe language with good numerical primitives and easy FFI to a host runtime.**

## Concrete recommendations

### Cut aggressively

- Drop raw pointers entirely. If it's embedded, the host manages memory. The GC handles the rest.
- Drop POSIX syscalls and sockets. The host provides I/O capabilities. Plugins shouldn't open sockets directly - that's a sandbox violation.
- Drop most integer widths. Keep `int` (64-bit) and `float` (64-bit). Add `tensor` as the ML primitive. LLMs don't need to choose between i8/i16/i32/i64 - that's a foot-gun for code generation.
- Drop SoA as user-facing syntax. Make it a compiler optimization over AoS. The LLM shouldn't have to choose the memory layout.

### Keep and strengthen

- Structs (but simple, no inheritance)
- Arrays/tensors with shape typing
- Maps
- Functions (first-class, with closures - you need these for callbacks in agent workflows)
- Simple control flow (if/while/for - no pattern matching needed)

### Add what's missing for the use case

- `async/await` or at minimum coroutines - agents need to wait on API calls
- A capability system instead of raw syscalls - `fs.read()`, `http.get()`, `model.run()` provided by the host, revocable per-plugin
- Tensor operations as builtins: `matmul`, `conv2d`, `softmax`, `backward` - not as library calls but as language primitives the compiler can optimize
- String interpolation (you have template literals, good)
- Error handling (Result type or simple try/catch - agents need to handle failures)

### The ideal surface area

For an LLM-friendly embedded ML language, the entire type system and feature set should be roughly:

```
Types:      int, float, bool, string, tensor, [T], {K: V}, struct
Control:    if/else, while, for..in, return, break, continue
Functions:  fn, closures, async fn
Builtins:   tensor ops, math, string ops
Host API:   capabilities granted by embedder
```

That's small enough for an LLM to hold the entire language in context. Compare to Python where an LLM has to navigate dozens of stdlib modules, dunder methods, metaclasses, decorators, generators, async generators, context managers, etc.

## The real question

Do you want to build a better Lua (embeddable, tiny, predictable) or a better Mojo (ML-native, fast, typed)? The first is achievable and has clear demand from the agent ecosystem. The second is a massive undertaking competing with well-funded teams.

I'd pick the first and make tensor ops a well-designed FFI to the host's ML runtime rather than trying to replace PyTorch from scratch.
