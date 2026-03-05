use std::fmt;

use crate::ir::{Fn, Target};
use crate::parse::{self, ParseResult};
use crate::{alias, arm64, cfg, copy, emit, fold, live, load, mem, regalloc, simpl, spill, ssa};

/// Compilation error.
#[derive(Debug)]
pub enum Error {
    /// Error during parsing.
    Parse(String),
    /// Error during compilation.
    Compile(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse(msg) => write!(f, "parse error: {msg}"),
            Error::Compile(msg) => write!(f, "compile error: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

/// Compile QBE IR text to assembly for the given target.
///
/// Processes each function through QBE's full pass pipeline in the exact order
/// from the reference C implementation, then emits the final assembly trailer.
pub fn compile(input: &str, target: &Target) -> Result<String, Error> {
    let mut out = String::new();

    // Parse input into types, data blocks, and functions.
    let ParseResult {
        types: _types,
        data,
        functions,
    } = parse::parse(input);

    // Emit data segments.
    let fp_stash = emit::FPStash::new();
    for data_group in &data {
        let mut dat_state = emit::DatState::new();
        for dat in data_group {
            emit::emitdat(dat, &mut dat_state, Some(target), &mut out);
        }
    }

    // Process each function through the compilation pipeline.
    for mut f in functions {
        compile_fn(&mut f, target, &mut out);
    }

    // Emit final assembly trailer.
    emit::emitfin(&fp_stash, target, &mut out);

    Ok(out)
}

/// Run the full pass pipeline on a single function, matching QBE's `func()` in main.c.
///
/// Pass ordering (from QBE 1.2):
///   abi0 → fillrpo → fillpreds → filluse → promote → filluse → ssa →
///   filluse → ssacheck → fillalias → loadopt → filluse → fillalias →
///   coalesce → filluse → ssacheck → copy → filluse → fold → abi1 →
///   simpl → fillpreds → filluse → isel → fillrpo → filllive → fillloop →
///   fillcost → spill → rega → fillrpo → simpljmp → fillpreds → fillrpo →
///   link blocks → emitfn
fn compile_fn(f: &mut Fn, target: &Target, out: &mut String) {
    // ABI lowering pass 0: classify parameters and returns.
    arm64::abi0(f, target);

    // Build CFG: RPO numbering, predecessor lists, use/def info.
    cfg::fillrpo(f);
    cfg::fillpreds(f);
    ssa::filluse(f);

    // Memory promotion: promote stack slots to temporaries.
    mem::promote(f);
    ssa::filluse(f);

    // SSA construction.
    ssa::ssa(f);
    ssa::filluse(f);
    ssa::ssacheck(f);

    // Load optimization: compute alias info, then optimize loads.
    alias::fillalias(f);
    load::loadopt(f);
    ssa::filluse(f);

    // Memory coalescing: recompute aliases, merge adjacent slots.
    alias::fillalias(f);
    mem::coalesce(f);
    ssa::filluse(f);
    ssa::ssacheck(f);

    // Copy elimination.
    copy::copy(f);
    ssa::filluse(f);

    // Constant folding.
    fold::fold(f);

    // ABI lowering pass 1: lower ABI-specific operations.
    arm64::abi1(f, target);

    // Simplification.
    simpl::simpl(f);

    // Rebuild CFG for instruction selection.
    cfg::fillpreds(f);
    ssa::filluse(f);

    // Instruction selection: lower IR ops to machine instructions.
    arm64::isel(f, target);

    // Prepare for register allocation.
    cfg::fillrpo(f);
    live::filllive(f, target);
    cfg::fillloop(f);
    spill::fillcost(f);

    // Spilling and register allocation.
    spill::spill(f, target);
    regalloc::rega(f, target);

    // Final CFG cleanup.
    cfg::fillrpo(f);
    cfg::simpljmp(f);
    cfg::fillpreds(f);
    cfg::fillrpo(f);

    // Link blocks in RPO order: set each block's link to the next RPO block.
    assert!(!f.rpo.is_empty(), "function must have at least one block");
    debug_assert!(
        f.rpo[0] == f.start,
        "first RPO block must be the entry block"
    );
    let nblk = f.rpo.len();
    for i in 0..nblk {
        let bid = f.rpo[i];
        let blk = &mut f.blks[bid.0 as usize];
        // Clear any existing link — it's not a field on Blk, so we use
        // the successor fields or a separate linking structure. For now
        // this is a no-op until Blk gains a `link` field or we use a
        // separate data structure for the linked ordering.
        let _ = blk;
    }

    // Emit assembly for this function.
    arm64::emitfn(f, target, out);
}
