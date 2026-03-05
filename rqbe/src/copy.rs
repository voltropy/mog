use crate::cfg;
use crate::ir::*;

/// Check if `r` is a constant with type `Bits` and integer value equal to `bits`.
fn iscon(r: Ref, bits: i64, f: &Fn) -> bool {
    if let Ref::Con(cid) = r {
        let c = &f.cons[cid.0 as usize];
        c.typ == ConType::Bits && c.bits.i() == bits
    } else {
        false
    }
}

/// Determine if instruction `ins` is effectively a copy of its first argument.
/// `r` is the `copyof` of `ins.arg[0]` (used for extension redundancy checks).
fn iscopy(ins: &Ins, r: Ref, f: &Fn) -> bool {
    // Table mapping source Width to bitmask of extension ops that are no-ops.
    // Index by Width as u8. BIT(w) = 1u64 << (w as u64).
    // Width variants: WFull=0, Wsb=1, Wub=2, Wsh=3, Wuh=4, Wsw=5, Wuw=6
    //
    // extcpy[WFull] = 0
    // extcpy[Wsb]   = BIT(Wsb) | BIT(Wsh) | BIT(Wsw)
    // extcpy[Wub]   = BIT(Wub) | BIT(Wuh) | BIT(Wuw)
    // extcpy[Wsh]   = BIT(Wsh) | BIT(Wsw)
    // extcpy[Wuh]   = BIT(Wuh) | BIT(Wuw)
    // extcpy[Wsw]   = BIT(Wsw)
    // extcpy[Wuw]   = BIT(Wuw)
    const fn bit(w: u8) -> u64 {
        1u64 << (w as u64)
    }

    const EXTCPY: [u64; 7] = [
        /* WFull=0 */ 0,
        /* Wsb=1  */ bit(1) | bit(3) | bit(5), // BIT(Wsb)|BIT(Wsh)|BIT(Wsw)
        /* Wub=2  */ bit(2) | bit(4) | bit(6), // BIT(Wub)|BIT(Wuh)|BIT(Wuw)
        /* Wsh=3  */ bit(3) | bit(5), // BIT(Wsh)|BIT(Wsw)
        /* Wuh=4  */ bit(4) | bit(6), // BIT(Wuh)|BIT(Wuw)
        /* Wsw=5  */ bit(5), // BIT(Wsw)
        /* Wuw=6  */ bit(6), // BIT(Wuw)
    ];

    match ins.op {
        Op::Copy => return true,
        Op::Mul | Op::Div | Op::Udiv => return iscon(ins.arg[1], 1, f),
        Op::Add | Op::Sub | Op::Or | Op::Xor | Op::Sar | Op::Shl | Op::Shr => {
            return iscon(ins.arg[1], 0, f);
        }
        _ => {}
    }

    if !ins.op.is_ext() {
        return false;
    }
    let tid = match r {
        Ref::Tmp(t) => t,
        _ => return false,
    };

    // Extsw or Extuw with class Kw is always a copy
    if (ins.op == Op::Extsw || ins.op == Op::Extuw) && ins.cls == Cls::Kw {
        return true;
    }

    let t = &f.tmps[tid.0 as usize];
    debug_assert!(t.cls.base() == 0); // integer base
    if ins.cls == Cls::Kl && t.cls == Cls::Kw {
        return false;
    }

    let b = EXTCPY[t.width as u8 as usize];
    // The extension op index: Wsb + (ins.op - Extsb)
    // Extsb..Extuw map to Width Wsb(1)..Wuw(6)
    let ext_width = Width::Wsb as u8 + (ins.op as u16 - Op::Extsb as u16) as u8;
    (bit(ext_width) & b) != 0
}

/// If `r` is a Tmp with a known copy-of value, return it; otherwise return `r`.
fn copyof(r: Ref, cpy: &[Ref]) -> Ref {
    if let Ref::Tmp(t) = r {
        let c = cpy[t.0 as usize];
        if c != Ref::R {
            return c;
        }
    }
    r
}

/// Detects a cluster of phis/copies redundant with `r`.
///
/// The algorithm is inspired by Section 3.2 of "Simple
/// and Efficient SSA Construction" by Braun M. et al.
fn phisimpl(phi_idx: usize, bid: BlkId, r: Ref, cpy: &mut [Ref], f: &Fn) {
    let mut ts = BSet::new(f.tmps.len() as u32);
    let mut als = BSet::new(f.tmps.len() as u32);

    // Stack entries: (UseType, UseDetail) — enough to identify what we're looking at.
    // We start with the phi itself.
    let mut stk: Vec<Use> = Vec::with_capacity(16);

    // Seed with the initial phi
    stk.push(Use {
        typ: UseType::Phi,
        bid: bid.0,
        detail: UseDetail::PhiIdx(phi_idx as u32),
    });

    while let Some(u) = stk.pop() {
        // Determine which phi's args to scan (narg) and which tmp `t` is being defined.
        let (phi_narg, phi_args, t);
        if u.typ == UseType::Ins {
            // Check if this instruction is a copy
            let ins_idx = match u.detail {
                UseDetail::InsIdx(i) => i as usize,
                _ => continue,
            };
            let blk = &f.blks[u.bid as usize];
            let ins = &blk.ins[ins_idx];
            let ins_r = copyof(ins.arg[0], cpy);
            if !iscopy(ins, ins_r, f) {
                continue;
            }
            // Treat as a trivial phi with 0 args
            phi_narg = 0;
            phi_args = &[][..];
            t = match ins.to {
                Ref::Tmp(tid) => tid.0,
                _ => continue,
            };
        } else if u.typ == UseType::Phi {
            let pidx = match u.detail {
                UseDetail::PhiIdx(i) => i as usize,
                _ => continue,
            };
            let blk = &f.blks[u.bid as usize];
            let phi = &blk.phi[pidx];
            phi_narg = phi.narg();
            phi_args = &phi.args[..];
            t = match phi.to {
                Ref::Tmp(tid) => tid.0,
                _ => continue,
            };
        } else {
            continue;
        }

        if ts.has(t) {
            continue;
        }
        ts.set(t);

        // Check phi args
        for a in 0..phi_narg {
            let r1 = copyof(phi_args[a], cpy);
            if r1 == r {
                continue;
            }
            match r1 {
                Ref::Tmp(tid) => {
                    als.set(tid.0);
                }
                _ => {
                    // Non-tmp, non-r arg: cluster is not redundant
                    return;
                }
            }
        }

        // Push all uses of tmp `t` onto the stack
        let uses = &f.tmps[t as usize].uses;
        stk.reserve(uses.len());
        for u_entry in uses {
            stk.push(*u_entry);
        }
    }

    als.diff(&ts);
    if als.count() == 0 {
        for t in ts.iter() {
            cpy[t as usize] = r;
        }
    }
}

/// Assert that a Tmp ref has been processed (its cpy entry is not R), then substitute.
fn subst(pr: &mut Ref, cpy: &[Ref]) {
    if let Ref::Tmp(t) = *pr {
        debug_assert!(cpy[t.0 as usize] != Ref::R, "subst: tmp not yet processed");
    }
    *pr = copyof(*pr, cpy);
}

/// Copy propagation pass.
///
/// Requires use information and dominance to be computed.
/// Breaks use information (caller should recompute).
pub fn copy(f: &mut Fn) {
    let ntmp = f.tmps.len();
    let mut cpy: Vec<Ref> = vec![Ref::R; ntmp];
    let nblk = f.rpo.len();

    // Phase 1: Build the copy-of map (in RPO order)
    for n in 0..nblk {
        let bid = f.rpo[n];
        let bidx = bid.0 as usize;

        // Process phis
        let nphi = f.blks[bidx].phi.len();
        for pi in 0..nphi {
            let to_val = match f.blks[bidx].phi[pi].to {
                Ref::Tmp(t) => t.0 as usize,
                _ => panic!("phi.to must be RTmp"),
            };
            debug_assert!(f.blks[bidx].phi[pi].to.is_tmp());

            if cpy[to_val] != Ref::R {
                continue;
            }

            let mut eq: u32 = 0;
            let mut r = Ref::R;
            let phi_narg = f.blks[bidx].phi[pi].narg();

            for a in 0..phi_narg {
                let pred_bid = f.blks[bidx].phi[pi].blks[a];
                let pred_id = f.blks[pred_bid.0 as usize].id;
                if pred_id < n as u32 {
                    let r1 = copyof(f.blks[bidx].phi[pi].args[a], &cpy);
                    if r == Ref::R || r == Ref::UNDEF {
                        r = r1;
                    }
                    if r1 == r || r1 == Ref::UNDEF {
                        eq += 1;
                    }
                }
            }

            debug_assert!(r != Ref::R);

            let phi_to = f.blks[bidx].phi[pi].to;

            if let Ref::Tmp(rtid) = r {
                let r_bid = f.tmps[rtid.0 as usize].bid;
                if !cfg::dom(f, BlkId(r_bid), bid) {
                    cpy[to_val] = phi_to;
                    continue;
                }
            }

            if eq == phi_narg as u32 {
                cpy[to_val] = r;
            } else {
                cpy[to_val] = phi_to;
                phisimpl(pi, bid, r, &mut cpy, f);
            }
        }

        // Process instructions
        let nins = f.blks[bidx].ins.len();
        for ii in 0..nins {
            let ins_to = f.blks[bidx].ins[ii].to;
            if ins_to == Ref::R {
                // Instructions without a destination (e.g. stores) — skip
                continue;
            }
            let to_val = match ins_to {
                Ref::Tmp(t) => t.0 as usize,
                _ => continue,
            };

            if cpy[to_val] != Ref::R {
                continue;
            }

            let r = copyof(f.blks[bidx].ins[ii].arg[0], &cpy);
            if iscopy(&f.blks[bidx].ins[ii], r, f) {
                cpy[to_val] = r;
            } else {
                cpy[to_val] = ins_to;
            }
        }
    }

    // Phase 2: Remove redundant phis/copies and rewrite uses
    for bidx in 0..f.blks.len() {
        // Remove redundant phis, substitute args in remaining phis
        f.blks[bidx].phi.retain_mut(|phi| {
            let to_val = match phi.to {
                Ref::Tmp(t) => t.0 as usize,
                _ => return true,
            };
            let r = cpy[to_val];
            if r != phi.to {
                // This phi is redundant — remove it
                return false;
            }
            for a in 0..phi.narg() {
                subst(&mut phi.args[a], &cpy);
            }
            true
        });

        // Process instructions: nop out redundant copies, substitute args
        let nins = f.blks[bidx].ins.len();
        for ii in 0..nins {
            if let Ref::Tmp(t) = f.blks[bidx].ins[ii].to {
                let r = cpy[t.0 as usize];
                if r != f.blks[bidx].ins[ii].to {
                    f.blks[bidx].ins[ii] = Ins::default();
                    continue;
                }
            }
            subst(&mut f.blks[bidx].ins[ii].arg[0], &cpy);
            subst(&mut f.blks[bidx].ins[ii].arg[1], &cpy);
        }

        // Substitute in jump argument
        subst(&mut f.blks[bidx].jmp.arg, &cpy);
    }
}
