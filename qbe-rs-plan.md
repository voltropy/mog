# QBE Rewrite in Safe Rust — Plan

## Goal

Rewrite QBE 1.2 as a safe Rust library (`rqbe`) that the Mog compiler can use directly as a crate dependency, eliminating the C FFI and external process spawning. ARM64-apple target only (what Mog needs). No `unsafe` blocks.

## Source

QBE 1.2 at `vendor/qbe-1.2/`: ~14,400 lines of C across 24 source files. 65 test files with a shell-based test runner.

## Output

A Rust crate at `rqbe/` with:
- `rqbe::ir` — IR types (Ref, Ins, Blk, Fn, Phi, Tmp, Con, Typ, Dat)
- `rqbe::parse` — QBE IL text parser
- `rqbe::compile` — top-level pipeline (parse → optimize → emit)
- `rqbe::ssa` — SSA construction (dominance frontiers, phi insertion, renaming)
- `rqbe::cfg` — CFG analysis (predecessors, RPO, dominators, loop detection)
- `rqbe::opt` — Optimization passes (fold, copy, simpl, load, alias, mem)
- `rqbe::spill` — Spill insertion
- `rqbe::regalloc` — Register allocation
- `rqbe::arm64` — ARM64 instruction selection, ABI lowering, assembly emission
- `rqbe::emit` — Shared emission utilities (data, linkage, debug, FP literal stash)

Public API:
```rust
pub fn compile(input: &str, target: Target) -> String;  // QBE IL → assembly text
pub fn compile_to_bytes(input: &str, target: Target) -> Vec<u8>;  // future: binary
```

## Key Design Decisions

### Index-based IR (no pointer graphs)

All IR nodes referenced by typed indices into arena vectors:

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct BlkId(u32);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct TmpId(u32);

pub struct Function {
    pub name: String,
    pub blks: Vec<Block>,      // indexed by BlkId
    pub tmps: Vec<Tmp>,        // indexed by TmpId
    pub cons: Vec<Con>,        // indexed by ConId
    pub mems: Vec<Mem>,        // indexed by MemId
    pub start: BlkId,
    pub rpo: Vec<BlkId>,       // reverse post-order
    // ...
}
```

This replaces C's `Blk*`, `Tmp*` pointer graphs with safe indexing.

### Ref as a Rust enum

C's bitfield-packed `Ref { type:3, val:29 }` becomes:

```rust
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Ref {
    Tmp(TmpId),
    Con(ConId),
    Int(i32),       // 29-bit signed
    Typ(TypId),
    Slot(u32),
    Call(u32),
    Mem(MemId),
    None,           // R in C (the zero Ref)
}
```

### Per-function arena

C's `PFn` pool (bulk-freed after each function) becomes owned `Vec`s inside `Function`. When `Function` drops, everything drops. The `alloc()` calls become `Vec::push()`.

### String interning

C's `intern()`/`str()` with a global hash table becomes a `StringInterner` struct (or the `string_interner` crate). Passed as `&mut` through the pipeline.

### No global mutable state

All C statics (`curf`, `tmp`, `mem`, `namel`, `outf`, etc.) become fields on context structs passed through function parameters.

### Target as a struct with function pointers → trait or enum

```rust
pub enum Target {
    Arm64Apple,
}
```

Only one target for now. The target-specific functions (abi0, abi1, isel, emitfn, emitfin) become methods on the target or free functions dispatched by match.

## Phase 1: Skeleton (sequential, ~3 tasks)

Establish the crate structure, all IR types, the compilation pipeline, and the public API — all with stub implementations. This is the foundation everything else builds on.

1. **Crate + IR types** — Create `rqbe/` crate. Define all IR types: `Ref`, `Op` (90 opcodes), `Ins`, `Phi`, `Blk`, `Tmp`, `Con`, `Mem`, `Fn`, `Typ`, `Field`, `Dat`, `Lnk`, `BSet`, `Alias`, `Use`, `Target`. Define all typed index newtypes. Define `Cls` (Kw/Kl/Ks/Kd). Define jump types. Implement Display for IR pretty-printing.

2. **Pipeline + public API** — Implement the top-level `compile()` function that calls each pass in order (matching `func()` in main.c). All passes are stub functions that take `&mut Fn` and return. Wire up parse → abi0 → fillrpo → fillpreds → filluse → promote → ssa → ... → rega → emitfn. Implement the parser as a stub that can at least create an empty Fn. Create the public API.

3. **Bitset + utilities** — Implement `BSet` (bitset type), string interning, the `emalloc`/`alloc`/`vnew`/`vgrow` equivalents (just Vec operations), `hash()`, `strf()`, helper functions from util.c.

## Phase 2: Parallel implementation + test porting (~25 tasks)

Once Phase 1 is done, all of these can proceed in parallel because they work on isolated modules with well-defined interfaces (the IR types from Phase 1).

### Implementation tasks (can all run in parallel):

4. **Parser: tokenizer** — Port the lexer (lex(), lexinit()) with perfect hash keyword lookup.
5. **Parser: types + data** — Port parsetyp(), parsefields(), parsedat(), parselnk().
6. **Parser: functions** — Port parsefn(), parseline(), parserefl(), parsecls(), typecheck().
7. **CFG: predecessors + RPO** — Port fillpreds(), fillrpo(), newblk().
8. **CFG: dominators + frontiers** — Port filldom(), sdom(), dom(), fillfron().
9. **CFG: loops + jump simplification** — Port fillloop(), loopiter(), simpljmp() with union-find.
10. **SSA construction** — Port ssa(), phiins(), renblk(), getstk(), rendef(), filluse(), ssacheck().
11. **Alias analysis** — Port fillalias(), getalias(), alias(), escapes().
12. **Load optimization** — Port loadopt() with store-to-load forwarding.
13. **Memory: slot promotion** — Port promote() (mem2reg from mem.c).
14. **Memory: slot coalescing** — Port coalesce() from mem.c.
15. **Constant folding (SCCP)** — Port fold() from fold.c.
16. **Copy propagation** — Port copy() and phi simplification from copy.c.
17. **Blit lowering** — Port simpl() from simpl.c.
18. **Liveness analysis** — Port filllive(), liveon() from live.c.
19. **Spill cost + insertion** — Port fillcost(), spill() from spill.c.
20. **Register allocation** — Port rega() with RMap, parallel move resolution from rega.c.
21. **ARM64 ABI pass 0** — Port abi0 (parameter/return lowering, struct classify, HFA detection, Apple varargs).
22. **ARM64 ABI pass 1** — Port abi1 (call lowering, arg emission, callee-save setup).
23. **ARM64 instruction selection** — Port isel() from arm64/isel.c.
24. **ARM64 assembly emission** — Port emitfn() from arm64/emit.c with omap table.
25. **Shared emission** — Port emitdat(), emitfnlnk(), emitfin(), stashbits(), debug info from emit.c.
26. **IR printer** — Port printfn(), printref() from parse.c for debugging.

### Test porting tasks (can run in parallel with implementation):

27. **Port test infrastructure** — Create test runner in Rust that: reads .ssa files, extracts embedded C drivers and expected output, runs compile → assemble → link → execute → check.
28. **Port arithmetic/basic tests** — Port test .ssa files for basic ops, arithmetic, control flow.
29. **Port ABI tests** — Port abi1-8.ssa tests (struct passing, varargs, return conventions).
30. **Port memory/load tests** — Port load1-3.ssa, mem1-3.ssa tests.
31. **Port isel/spill/rega tests** — Port isel1-3.ssa, spill1.ssa, rega1.ssa.
32. **Port algorithm tests** — Port collatz, eucl, prime, mandel, queen, sum, etc.
33. **Port float/conversion tests** — Port fpcnv.ssa, double.ssa, float conversion tests.
34. **Port advanced tests** — Port TLS, dynalloc, dark type, blit, copy, phi tests.

## Phase 3: Get all tests passing (~1 task per failing area)

After Phase 2, spawn parallel tasks to fix each failing test category. Each task reads the test failure output, traces through the relevant pass, and fixes bugs.

## Phase 4: Integration + E2E

35. **Wire rqbe into Mog compiler** — Replace `qbe` process spawning and C FFI in `compiler/src/compiler.rs` with direct `rqbe::compile()` calls.
36. **Run Mog test suite** — All 1,145 Rust compiler tests must pass.
37. **Run showcase** — Build and run showcase.mog end-to-end through the new pipeline.
38. **Benchmark** — Compare compile times: old (spawn qbe + as) vs new (rqbe + as).

## File-to-module mapping

| C file(s) | Rust module | Lines (C) | Est. lines (Rust) |
|---|---|---|---|
| all.h | `ir.rs` (types) | 576 | ~800 |
| ops.h | `op.rs` | 196 | ~250 |
| parse.c | `parse.rs` | 1,402 | ~1,200 |
| util.c | `util.rs` | 596 | ~300 |
| ssa.c | `ssa.rs` | 432 | ~400 |
| cfg.c | `cfg.rs` | 331 | ~300 |
| fold.c | `fold.rs` | 535 | ~500 |
| copy.c | `copy.rs` | 229 | ~200 |
| simpl.c | `simpl.rs` | 82 | ~80 |
| load.c | `load.rs` | 493 | ~450 |
| alias.c | `alias.rs` | 220 | ~200 |
| mem.c | `mem.rs` | 483 | ~450 |
| live.c | `live.rs` | 144 | ~130 |
| spill.c | `spill.rs` | 538 | ~500 |
| rega.c | `rega.rs` | 698 | ~650 |
| arm64/isel.c | `arm64/isel.rs` | 317 | ~300 |
| arm64/emit.c | `arm64/emit.rs` | 644 | ~600 |
| arm64/abi.c | `arm64/abi.rs` | 852 | ~800 |
| arm64/targ.c | `arm64/targ.rs` | 68 | ~60 |
| emit.c | `emit.rs` | 244 | ~250 |
| main.c | (integrated into compile.rs) | 198 | ~100 |
| **Total** | | **~8,400** | **~7,500** |

(Excluding amd64/rv64 backends — not needed for Mog.)
