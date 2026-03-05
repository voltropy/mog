//! Flow-insensitive alias analysis.
//!
//! Port of QBE's `alias.c`. Classifies temporaries into alias categories
//! (ALoc, AEsc, ACon, ASym, AUnk) and provides queries for aliasing
//! relationships between memory references.

use crate::ir::{Alias, AliasLoc, AliasResult, AliasType, ConType, Fn, Ins, Jmp, Op, Ref, N_BIT};

// ---------------------------------------------------------------------------
// getalias — resolve alias info for a reference
// ---------------------------------------------------------------------------

/// Resolve the alias information for a reference.
///
/// For temporaries, copies the alias from `f.tmps` and updates the type
/// from the underlying slot if it's a stack reference.
/// For constants, constructs an alias directly.
///
/// Port of C QBE's `getalias()`.
pub fn getalias(f: &Fn, r: Ref) -> Alias {
    match r {
        Ref::Tmp(id) => {
            let mut a = f.tmps[id.0 as usize].alias.clone();
            if a.typ == AliasType::Bot {
                // Physical registers or temps not defined in this function
                // (e.g., from ABI lowering) may have Bot alias type.
                // Treat as unknown — conservative but safe.
                a.typ = AliasType::Unk;
                a.base = id.0 as i32;
                a.offset = 0;
                a.slot = None;
                return a;
            }
            if a.typ.is_stack() {
                // Update type from the slot's current type.
                // In C QBE: a->type = a->slot->type
                // In our Rust port, base identifies the original alloc tmp.
                let slot_typ = f.tmps[a.base as usize].alias.typ;
                a.typ = slot_typ;
            }
            a
        }
        Ref::Con(id) => {
            let c = &f.cons[id.0 as usize];
            let mut a = Alias::default();
            if c.typ == ConType::Addr {
                a.typ = AliasType::Sym;
                a.sym = c.sym;
            } else {
                a.typ = AliasType::Con;
            }
            a.offset = c.bits.i();
            a.slot = None;
            a
        }
        _ => panic!("getalias: unexpected ref type"),
    }
}

// ---------------------------------------------------------------------------
// alias — determine aliasing relationship
// ---------------------------------------------------------------------------

/// Determine the alias relationship between two memory references.
///
/// `p` is the first pointer with additional offset `op` and access size `sp`.
/// `q` is the second pointer with access size `sq`.
/// Returns `(AliasResult, delta)` where `delta = ap.offset - aq.offset`
/// (meaningful when the result is MustAlias).
///
/// Port of C QBE's `alias()`.
pub fn alias(f: &Fn, p: Ref, op: i32, sp: i32, q: Ref, sq: i32) -> (AliasResult, i32) {
    let mut ap = getalias(f, p);
    let aq = getalias(f, q);
    ap.offset = ap.offset.wrapping_add(op as i64);

    // When delta is meaningful (ovlap == true), we do not overflow i32
    // because sp and sq are bounded by 2^28.
    let delta = ap.offset.wrapping_sub(aq.offset) as i32;
    let ovlap = ap.offset < aq.offset.wrapping_add(sq as i64)
        && aq.offset < ap.offset.wrapping_add(sp as i64);

    if ap.typ.is_stack() && aq.typ.is_stack() {
        // Both are offsets of the same stack slot — they alias iff they overlap.
        if ap.base == aq.base && ovlap {
            return (AliasResult::MustAlias, delta);
        }
        return (AliasResult::NoAlias, delta);
    }

    if ap.typ == AliasType::Sym && aq.typ == AliasType::Sym {
        // Conservatively alias if symbols differ; must-alias if they overlap.
        if !ap.sym.eq(&aq.sym) {
            return (AliasResult::MayAlias, delta);
        }
        if ovlap {
            return (AliasResult::MustAlias, delta);
        }
        return (AliasResult::NoAlias, delta);
    }

    if (ap.typ == AliasType::Con && aq.typ == AliasType::Con)
        || (ap.typ == aq.typ && ap.base == aq.base)
    {
        debug_assert!(ap.typ == AliasType::Con || ap.typ == AliasType::Unk);
        // Same base — rely on offsets only.
        if ovlap {
            return (AliasResult::MustAlias, delta);
        }
        return (AliasResult::NoAlias, delta);
    }

    // If one is unknown, there may be aliasing unless the other is
    // provably local (ALoc).
    if ap.typ == AliasType::Unk && aq.typ != AliasType::Loc {
        return (AliasResult::MayAlias, delta);
    }
    if aq.typ == AliasType::Unk && ap.typ != AliasType::Loc {
        return (AliasResult::MayAlias, delta);
    }

    (AliasResult::NoAlias, delta)
}

// ---------------------------------------------------------------------------
// escapes — check if a reference escapes
// ---------------------------------------------------------------------------

/// Check whether a reference potentially escapes its local scope.
///
/// Returns `true` if the reference is not a stack-local temp, or if its
/// underlying slot has been marked as escaping.
///
/// Port of C QBE's `escapes()`.
pub fn escapes(r: Ref, f: &Fn) -> bool {
    match r {
        Ref::Tmp(id) => {
            let a = &f.tmps[id.0 as usize].alias;
            if !a.typ.is_stack() {
                return true;
            }
            // Check the slot's type.
            f.tmps[a.base as usize].alias.typ == AliasType::Esc
        }
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Mark a reference's underlying stack slot as escaped.
///
/// Port of C QBE's static `esc()`.
fn esc(r: Ref, f: &mut Fn) {
    if let Ref::Tmp(id) = r {
        let a_typ = f.tmps[id.0 as usize].alias.typ;
        if a_typ.is_stack() {
            let base = f.tmps[id.0 as usize].alias.base;
            f.tmps[base as usize].alias.typ = AliasType::Esc;
        }
    }
}

/// Track a store to a potential stack slot by updating its access bitmask.
///
/// Port of C QBE's static `store()` in alias.c.
fn store(r: Ref, sz: i32, f: &mut Fn) {
    if let Ref::Tmp(id) = r {
        let a_typ = f.tmps[id.0 as usize].alias.typ;
        let a_base = f.tmps[id.0 as usize].alias.base;
        if a_typ.is_stack() {
            let off = f.tmps[id.0 as usize].alias.offset;
            let m: u64 = if sz as u32 >= N_BIT || off < 0 || off >= N_BIT as i64 {
                !0u64
            } else {
                ((1u64 << sz) - 1) << (off as u32)
            };
            f.tmps[a_base as usize].alias.loc.m |= m;
        }
    }
}

/// Helper to get store size from a store instruction.
fn storesz(i: &Ins) -> i32 {
    match i.op {
        Op::Storeb => 1,
        Op::Storeh => 2,
        Op::Storew | Op::Stores => 4,
        Op::Storel | Op::Stored => 8,
        _ => panic!("storesz: not a store op"),
    }
}

// ---------------------------------------------------------------------------
// fillalias — main alias analysis pass
// ---------------------------------------------------------------------------

/// Compute alias information for all temporaries in the function.
///
/// This is a flow-insensitive analysis that classifies each temporary as:
/// - `ALoc`: stack local (non-escaping allocation)
/// - `AEsc`: stack allocation that has escaped
/// - `ACon`: known constant address
/// - `ASym`: known global symbol
/// - `AUnk`: unknown (conservative)
///
/// It also tracks pointer arithmetic to maintain offset information and
/// marks slots as escaped when their address is passed to unknown uses.
///
/// Port of C QBE's `fillalias()`.
pub fn fillalias(f: &mut Fn) {
    // Initialize all aliases to Bot.
    for t in 0..f.tmps.len() {
        f.tmps[t].alias.typ = AliasType::Bot;
    }

    let nblk = f.rpo.len();
    for n in 0..nblk {
        let bid = f.rpo[n];
        let b = &f.blks[bid.0 as usize];

        // Process phi nodes.
        let phi_data: Vec<(Vec<Ref>, Ref)> = b.phi.iter().map(|p| (p.args.clone(), p.to)).collect();

        for (args, to) in &phi_data {
            for &arg in args {
                esc(arg, f);
            }
            if let Ref::Tmp(id) = *to {
                let a = &mut f.tmps[id.0 as usize].alias;
                assert!(a.typ == AliasType::Bot);
                a.typ = AliasType::Unk;
                a.base = id.0 as i32;
                a.offset = 0;
                a.slot = None;
            }
        }

        // Collect instruction data we need (to avoid borrow conflicts).
        let ins: Vec<Ins> = f.blks[bid.0 as usize].ins.clone();

        let mut idx = 0;
        while idx < ins.len() {
            let i = ins[idx];

            // Set up alias for the destination temp, if any.
            if i.to != Ref::R {
                if let Ref::Tmp(to_id) = i.to {
                    let a = &mut f.tmps[to_id.0 as usize].alias;
                    assert!(a.typ == AliasType::Bot);

                    if i.op.is_alloc() {
                        a.typ = AliasType::Loc;
                        // slot = self (base points to self)
                        a.base = to_id.0 as i32;
                        a.offset = 0;
                        a.loc = AliasLoc { sz: -1, m: 0 };

                        // Check if the alloc size is a known constant.
                        if let Ref::Con(cid) = i.arg[0] {
                            let c = &f.cons[cid.0 as usize];
                            let x = c.bits.i();
                            if c.typ == ConType::Bits && x >= 0 && x <= N_BIT as i64 {
                                a.loc.sz = x as i32;
                            }
                        }
                    } else {
                        a.typ = AliasType::Unk;
                        a.base = to_id.0 as i32;
                        a.offset = 0;
                        a.slot = None;
                    }
                }
            }

            // Handle copy: inherit alias from source.
            if i.op == Op::Copy && i.to != Ref::R {
                if let Ref::Tmp(to_id) = i.to {
                    let src_alias = getalias(f, i.arg[0]);
                    f.tmps[to_id.0 as usize].alias = src_alias;
                }
            }

            // Handle add: track pointer arithmetic.
            if i.op == Op::Add && i.to != Ref::R {
                if let Ref::Tmp(to_id) = i.to {
                    let a0 = getalias(f, i.arg[0]);
                    let a1 = getalias(f, i.arg[1]);

                    if a0.typ == AliasType::Con {
                        let mut result = a1.clone();
                        result.offset = result.offset.wrapping_add(a0.offset);
                        f.tmps[to_id.0 as usize].alias = result;
                    } else if a1.typ == AliasType::Con {
                        let mut result = a0.clone();
                        result.offset = result.offset.wrapping_add(a1.offset);
                        f.tmps[to_id.0 as usize].alias = result;
                    }
                }
            }

            // Escape arguments for instructions that might leak pointers.
            let a_is_unk = if let Ref::Tmp(to_id) = i.to {
                f.tmps[to_id.0 as usize].alias.typ == AliasType::Unk
            } else {
                false
            };

            if (i.to == Ref::R || a_is_unk) && i.op != Op::Blit0 {
                if !i.op.is_load() {
                    esc(i.arg[0], f);
                }
                if !i.op.is_store() && i.op != Op::Argc {
                    esc(i.arg[1], f);
                }
            }

            // Handle blit0/blit1 pair.
            if i.op == Op::Blit0 {
                idx += 1;
                let i2 = ins[idx];
                assert!(i2.op == Op::Blit1);
                if let Ref::Int(v) = i2.arg[0] {
                    let sz = (v as i32).abs();
                    store(i.arg[1], sz, f);
                }
            }

            // Handle stores.
            if i.op.is_store() {
                let sz = storesz(&i);
                store(i.arg[1], sz, f);
            }

            idx += 1;
        }

        // Escape the jump argument (unless it's a retc).
        let jmp = f.blks[bid.0 as usize].jmp;
        if jmp.typ != Jmp::Retc {
            esc(jmp.arg, f);
        }
    }
}
