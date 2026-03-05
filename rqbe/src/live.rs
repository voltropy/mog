//! Backward dataflow liveness analysis.
//!
//! Faithfully ported from QBE 1.2 `live.c` (144 lines).
//!
//! Computes in/out/gen bitsets per block via iterative fixed-point over
//! blocks in reverse RPO. Tracks peak live counts per register class
//! (int/fp). Accounts for caller-save clobbers around calls.

use crate::ir::{BSet, Fn, Op, Ref, Target};

/// Compute the live-in set for block `s` as seen from predecessor block `b`,
/// accounting for phi nodes.
///
/// Port of C QBE's `liveon(BSet *v, Blk *b, Blk *s)`.
pub fn liveon(f: &mut Fn, v: &mut BSet, b_id: u32, s_id: u32) {
    let s = &f.blks[s_id as usize];
    v.copy_from(&s.r#in);

    let nphi = f.blks[s_id as usize].phi.len();
    for pi in 0..nphi {
        let to = f.blks[s_id as usize].phi[pi].to;
        if let Ref::Tmp(tid) = to {
            v.clr(tid.0);
        }
    }

    for pi in 0..nphi {
        let narg = f.blks[s_id as usize].phi[pi].narg();
        for a in 0..narg {
            let blk_id = f.blks[s_id as usize].phi[pi].blks[a];
            if blk_id.0 == b_id {
                let arg = f.blks[s_id as usize].phi[pi].args[a];
                if let Ref::Tmp(tid) = arg {
                    v.set(tid.0);
                    f.blks[b_id as usize].gen.set(tid.0);
                }
            }
        }
    }
}

/// Helper: mark a reference as live in a block's gen and in sets.
#[inline]
fn bset_ref(r: Ref, b_id: u32, nlv: &mut [i32; 2], f: &mut Fn) {
    if let Ref::Tmp(tid) = r {
        let t = tid.0;
        f.blks[b_id as usize].gen.set(t);
        if !f.blks[b_id as usize].r#in.has(t) {
            let base = f.tmps[t as usize].cls.base();
            if base >= 0 {
                nlv[base as usize] += 1;
            }
            f.blks[b_id as usize].r#in.set(t);
        }
    }
}

/// Compute liveness information (in/out/gen sets for each block).
///
/// Iterative fixed-point over blocks in reverse RPO. Also computes
/// peak live counts (`nlive[2]`) per block for int and float register
/// classes, accounting for caller-save clobbers around call instructions.
///
/// Port of C QBE's `filllive(Fn *f)`.
///
/// Requires: RPO computation (`fillrpo`).
pub fn filllive(f: &mut Fn, t: &Target) {
    filllive_inner(f, Some(t));
}

/// Liveness analysis without target-specific register handling.
///
/// Used by SSA construction where target info isn't available yet.
pub fn filllive_no_target(f: &mut Fn) {
    filllive_inner(f, None);
}

fn filllive_inner(f: &mut Fn, t: Option<&Target>) {
    let ntmp = f.tmps.len() as u32;

    for &bid in f.rpo.iter() {
        let blk = &mut f.blks[bid.0 as usize];
        blk.r#in.init(ntmp);
        blk.out.init(ntmp);
        blk.gen.init(ntmp);
    }

    let mut u = BSet::new(ntmp);
    let mut v = BSet::new(ntmp);
    let mut chg = true;

    while chg {
        chg = false;

        let nblk = f.rpo.len();
        for n in (0..nblk).rev() {
            let bid = f.rpo[n];
            let b_idx = bid.0 as usize;

            u.copy_from(&f.blks[b_idx].out);

            if let Some(s1) = f.blks[b_idx].s1 {
                liveon(f, &mut v, bid.0, s1.0);
                f.blks[b_idx].out.union(&v);
            }
            if let Some(s2) = f.blks[b_idx].s2 {
                liveon(f, &mut v, bid.0, s2.0);
                f.blks[b_idx].out.union(&v);
            }

            chg |= !f.blks[b_idx].out.equal(&u);

            let mut nlv = [0i32; 2];

            // b->out->t[0] |= T.rglob
            if let Some(t) = t {
                if !f.blks[b_idx].out.bits_raw().is_empty() {
                    f.blks[b_idx].out.bits_raw_mut()[0] |= t.rglob;
                }
            }

            // in = copy of out (via scratch to avoid aliasing).
            u.copy_from(&f.blks[b_idx].out);
            f.blks[b_idx].r#in.copy_from(&u);

            for ti in f.blks[b_idx].r#in.iter() {
                let base = f.tmps[ti as usize].cls.base();
                if base >= 0 {
                    nlv[base as usize] += 1;
                }
            }

            let jmp_arg = f.blks[b_idx].jmp.arg;
            if let (Ref::Call(_), Some(t)) = (jmp_arg, t) {
                let retregs = t.retregs(jmp_arg, Some(&mut nlv));
                if !f.blks[b_idx].r#in.bits_raw().is_empty() {
                    f.blks[b_idx].r#in.bits_raw_mut()[0] |= retregs;
                }
            } else {
                bset_ref(jmp_arg, bid.0, &mut nlv, f);
            }

            f.blks[b_idx].nlive = [nlv[0], nlv[1]];

            let nins = f.blks[b_idx].ins.len();
            for i in (0..nins).rev() {
                let ins = f.blks[b_idx].ins[i];

                if let Some(t) = t {
                    if ins.op == Op::Call {
                        if let Ref::Call(_) = ins.arg[1] {
                            let call_ref = ins.arg[1];
                            let mut m = [0i32; 2];

                            let retregs = t.retregs(call_ref, Some(&mut m));
                            if !f.blks[b_idx].r#in.bits_raw().is_empty() {
                                f.blks[b_idx].r#in.bits_raw_mut()[0] &= !retregs;
                            }
                            for k in 0..2 {
                                nlv[k] -= m[k];
                                nlv[k] += t.nrsave[k];
                                if nlv[k] > f.blks[b_idx].nlive[k] {
                                    f.blks[b_idx].nlive[k] = nlv[k];
                                }
                            }

                            let argregs = t.argregs(call_ref, Some(&mut m));
                            if !f.blks[b_idx].r#in.bits_raw().is_empty() {
                                f.blks[b_idx].r#in.bits_raw_mut()[0] |= argregs;
                            }
                            for k in 0..2 {
                                nlv[k] -= t.nrsave[k];
                                nlv[k] += m[k];
                            }
                        }
                    }
                }

                let to = ins.to;
                if to != Ref::R {
                    if let Ref::Tmp(tid) = to {
                        let tv = tid.0;
                        if f.blks[b_idx].r#in.has(tv) {
                            let base = f.tmps[tv as usize].cls.base();
                            if base >= 0 {
                                nlv[base as usize] -= 1;
                            }
                        }
                        f.blks[b_idx].gen.set(tv);
                        f.blks[b_idx].r#in.clr(tv);
                    }
                }

                for k in 0..2 {
                    let arg = ins.arg[k];
                    match arg {
                        Ref::Mem(mid) => {
                            let mem_base = f.mems[mid.0 as usize].base;
                            let mem_index = f.mems[mid.0 as usize].index;
                            bset_ref(mem_base, bid.0, &mut nlv, f);
                            bset_ref(mem_index, bid.0, &mut nlv, f);
                        }
                        _ => {
                            bset_ref(arg, bid.0, &mut nlv, f);
                        }
                    }
                }

                for k in 0..2 {
                    if nlv[k] > f.blks[b_idx].nlive[k] {
                        f.blks[b_idx].nlive[k] = nlv[k];
                    }
                }
            }
        }
    }
}
