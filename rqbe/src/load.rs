//! Store-to-load forwarding optimization.
//!
//! Port of QBE's `load.c`. Walks backward through instructions and CFG
//! predecessors to find stores that satisfy loads, inserting phi nodes
//! at join points for forwarded values. Handles partial overlaps via
//! bit manipulation.

use crate::alias::{alias, escapes};
use crate::cfg::dom;
use crate::ir::{AliasType, BlkId, Cls, Con, ConType, Fn, Ins, Op, Phi, Ref, TmpId};
use crate::util::{getcon, newcon, newtmp};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Bitmask for a width in bytes: `(1 << (8*w - 1)) * 2 - 1`.
/// Must work when w == 8.
#[inline]
fn mask_for(w: i32) -> u64 {
    if w == 8 {
        !0u64
    } else {
        (1u64 << (8 * w)) - 1
    }
}

/// Return the size in bytes of a load instruction.
pub fn loadsz(i: &Ins) -> i32 {
    match i.op {
        Op::Loadsb | Op::Loadub => 1,
        Op::Loadsh | Op::Loaduh => 2,
        Op::Loadsw | Op::Loaduw => 4,
        Op::Load => {
            if i.cls.is_wide() {
                8
            } else {
                4
            }
        }
        _ => panic!("loadsz: not a load op"),
    }
}

/// Return the size in bytes of a store instruction.
pub fn storesz(i: &Ins) -> i32 {
    match i.op {
        Op::Storeb => 1,
        Op::Storeh => 2,
        Op::Storew | Op::Stores => 4,
        Op::Storel | Op::Stored => 8,
        _ => panic!("storesz: not a store op"),
    }
}

// ---------------------------------------------------------------------------
// Insertion log
// ---------------------------------------------------------------------------

/// Where an insertion happens, and what constraints it has.
#[derive(Clone)]
enum LocType {
    /// Right above the original load.
    Root,
    /// Inserting a load is allowed.
    Load,
    /// Only scalar operations allowed.
    NoLoad,
}

/// Insertion location.
#[derive(Clone)]
struct Loc {
    typ: LocType,
    off: usize,
    blk: BlkId,
}

/// A memory slice being tracked.
#[derive(Clone)]
struct Slice {
    r: Ref,
    off: i32,
    sz: i16,
    cls: Cls,
}

/// An insertion record — either a phi or an instruction.
#[derive(Clone)]
enum InsertNew {
    Ins(Ins),
    Phi { sl: Slice, phi: Phi },
}

#[derive(Clone)]
struct Insert {
    is_phi: bool,
    num: u32,
    bid: u32,
    off: u32,
    new: InsertNew,
}

// ---------------------------------------------------------------------------
// Load optimization state
// ---------------------------------------------------------------------------

struct LoadOpt<'a> {
    f: &'a mut Fn,
    inum: u32,
    ilog: Vec<Insert>,
}

impl<'a> LoadOpt<'a> {
    fn new(f: &'a mut Fn) -> Self {
        Self {
            f,
            inum: 0,
            ilog: Vec::new(),
        }
    }

    /// Insert an instruction at the given location, returning a ref to a new temp.
    fn iins(&mut self, cls: Cls, op: Op, a0: Ref, a1: Ref, l: &Loc) -> Ref {
        let num = self.inum;
        self.inum += 1;

        let to = newtmp("ld", cls, self.f);
        let ins = Ins {
            op,
            cls,
            to,
            arg: [a0, a1],
        };

        self.ilog.push(Insert {
            is_phi: false,
            num,
            bid: self.f.blks[l.blk.0 as usize].id,
            off: l.off as u32,
            new: InsertNew::Ins(ins),
        });

        to
    }

    /// Cast a ref to a different class, inserting conversion instructions.
    fn cast(&mut self, r: &mut Ref, cls: Cls, l: &Loc) {
        if let Ref::Con(_) = *r {
            return;
        }
        debug_assert!(matches!(*r, Ref::Tmp(_)));
        let cls0 = match *r {
            Ref::Tmp(id) => self.f.tmps[id.0 as usize].cls,
            _ => return,
        };

        if cls0 == cls || (cls == Cls::Kw && cls0 == Cls::Kl) {
            return;
        }

        if (cls0.is_wide() as u8) < (cls.is_wide() as u8) {
            if cls0 == Cls::Ks {
                *r = self.iins(Cls::Kw, Op::Cast, *r, Ref::R, l);
            }
            *r = self.iins(Cls::Kl, Op::Extuw, *r, Ref::R, l);
            if cls == Cls::Kd {
                *r = self.iins(Cls::Kd, Op::Cast, *r, Ref::R, l);
            }
        } else {
            if cls0 == Cls::Kd && cls != Cls::Kl {
                *r = self.iins(Cls::Kl, Op::Cast, *r, Ref::R, l);
            }
            if cls0 != Cls::Kd || cls != Cls::Kw {
                *r = self.iins(cls, Op::Cast, *r, Ref::R, l);
            }
        }
    }

    /// Apply a bitmask to a ref.
    fn mask(&mut self, cls: Cls, r: &mut Ref, msk: u64, l: &Loc) {
        self.cast(r, cls, l);
        let con = getcon(msk as i64, self.f);
        *r = self.iins(cls, Op::And, *r, con, l);
    }

    /// Load a slice from memory, applying the given mask.
    fn load(&mut self, sl: &Slice, msk: u64, l: &Loc) -> Ref {
        let ld = match sl.sz {
            1 => Op::Loadub,
            2 => Op::Loaduh,
            4 => Op::Loaduw,
            8 => Op::Load,
            _ => panic!("load: unexpected size {}", sl.sz),
        };

        let all = msk == mask_for(sl.sz as i32);
        let cls = if all {
            sl.cls
        } else if sl.sz > 4 {
            Cls::Kl
        } else {
            Cls::Kw
        };

        // sl.ref might not be live here, but its alias base ref will be.
        let mut r = sl.r;
        if let Ref::Tmp(id) = r {
            let a = &self.f.tmps[id.0 as usize].alias;
            match a.typ {
                AliasType::Loc | AliasType::Esc | AliasType::Unk => {
                    let base = a.base;
                    let offset = a.offset;
                    r = Ref::Tmp(TmpId(base as u32));
                    if offset != 0 {
                        let r1 = getcon(offset, self.f);
                        r = self.iins(Cls::Kl, Op::Add, r, r1, l);
                    }
                }
                AliasType::Con | AliasType::Sym => {
                    let mut c = Con::default();
                    c.typ = ConType::Addr;
                    c.sym = a.sym;
                    c.bits = crate::ir::ConBits::from_i64(a.offset);
                    r = newcon(&c, self.f);
                }
                _ => panic!("load: unexpected alias type"),
            }
        }

        r = self.iins(cls, ld, r, Ref::R, l);
        if !all {
            self.mask(cls, &mut r, msk, l);
        }
        r
    }

    /// Check if a definition kills the slice's live range.
    fn killsl(&self, r: Ref, sl: &Slice) -> bool {
        if let Ref::Tmp(sl_id) = sl.r {
            let a = &self.f.tmps[sl_id.0 as usize].alias;
            match a.typ {
                AliasType::Loc | AliasType::Esc | AliasType::Unk => {
                    r == Ref::Tmp(TmpId(a.base as u32))
                }
                AliasType::Con | AliasType::Sym => false,
                _ => panic!("killsl: unexpected alias type"),
            }
        } else {
            false
        }
    }

    /// Try to find a definition for the given slice with the given mask,
    /// walking backward through block `b` starting before instruction index `i_idx`.
    /// Returns `Ref::R` on failure.
    fn def(&mut self, sl: &Slice, msk: u64, bid: BlkId, i_start: Option<usize>, il: &Loc) -> Ref {
        // Invariants from C QBE:
        // -1- b dominates il->blk
        // -2- if il->type != LNoLoad, then il->blk postdominates the original load
        // -3- if il->type != LNoLoad, then b postdominates il->blk
        debug_assert!(dom(self.f, bid, il.blk));

        let old_nlog = self.ilog.len();
        let old_ntmp = self.f.tmps.len();

        let b = &self.f.blks[bid.0 as usize];
        let ins = b.ins.clone();
        let nins = ins.len();
        let cls = if sl.sz > 4 { Cls::Kl } else { Cls::Kw };
        let msks = mask_for(sl.sz as i32);

        let mut i_idx = i_start.unwrap_or(nins);

        while i_idx > 0 {
            i_idx -= 1;
            let i = &ins[i_idx];

            if self.killsl(i.to, sl) || (i.op == Op::Call && escapes(sl.r, self.f)) {
                // Reload: fall through to load path.
                self.f.tmps.truncate(old_ntmp);
                self.ilog.truncate(old_nlog);
                return if !matches!(il.typ, LocType::Load) {
                    Ref::R
                } else {
                    self.load(sl, msk, il)
                };
            }

            let ld = i.op.is_load();
            let (sz, r1, mut r);
            if ld {
                sz = loadsz(i);
                r1 = i.arg[0];
                r = i.to;
            } else if i.op.is_store() {
                sz = storesz(i);
                r1 = i.arg[1];
                r = i.arg[0];
            } else if i.op == Op::Blit1 {
                // blit1 always preceded by blit0
                if let Ref::Int(v) = i.arg[0] {
                    sz = (v as i32).unsigned_abs() as i32;
                } else {
                    panic!("blit1: expected Int arg");
                }
                assert!(i_idx > 0);
                i_idx -= 1;
                let i_prev = &ins[i_idx];
                assert!(i_prev.op == Op::Blit0);
                r1 = i_prev.arg[1];
                r = Ref::R; // placeholder, handled below
            } else {
                continue;
            }

            let (alias_result, off) = alias(self.f, sl.r, sl.off, sl.sz as i32, r1, sz);

            match alias_result {
                crate::ir::AliasResult::MustAlias => {
                    // Handle blit0 case.
                    let is_blit = ins[i_idx].op == Op::Blit0;
                    let mut sl1;
                    if is_blit {
                        sl1 = sl.clone();
                        sl1.r = ins[i_idx].arg[0];
                        if off >= 0 {
                            debug_assert!(off < sz);
                            sl1.off = off;
                            let mut new_sz = sz - off;
                            if new_sz > sl1.sz as i32 {
                                new_sz = sl1.sz as i32;
                            }
                            assert!(new_sz <= 8);
                            sl1.sz = new_sz as i16;
                        } else {
                            sl1.off = 0;
                            sl1.sz = (sl1.sz as i32 + off) as i16;
                            if sz > sl1.sz as i32 {
                                // sz stays
                            }
                            if sl1.sz as i32 > sz {
                                sl1.sz = sz as i16;
                            }
                            assert!(sl1.sz <= 8);
                        }
                    } else {
                        sl1 = sl.clone(); // unused in non-blit path
                    }

                    let (msk1, shift_op);
                    if off < 0 {
                        let uoff = (-off) as u32;
                        msk1 = (mask_for(sz) << (8 * uoff)) & msks;
                        shift_op = Op::Shl;
                    } else {
                        msk1 = (mask_for(sz) >> (8 * off as u32)) & msks;
                        shift_op = Op::Shr;
                    }

                    if msk1 & msk == 0 {
                        continue;
                    }

                    if is_blit {
                        let blit_r = self.def(&sl1, mask_for(sz), bid, Some(i_idx), il);
                        if blit_r == Ref::R {
                            self.f.tmps.truncate(old_ntmp);
                            self.ilog.truncate(old_nlog);
                            return if !matches!(il.typ, LocType::Load) {
                                Ref::R
                            } else {
                                self.load(sl, msk, il)
                            };
                        }
                        r = blit_r;
                    }

                    if off != 0 {
                        let mut cls1 = cls;
                        if shift_op == Op::Shr && off as i32 + sl.sz as i32 > 4 {
                            cls1 = Cls::Kl;
                        }
                        self.cast(&mut r, cls1, il);
                        let r1 = getcon(8 * off.unsigned_abs() as i64, self.f);
                        r = self.iins(cls1, shift_op, r, r1, il);
                    }

                    if (msk1 & msk) != msk1 || (off.unsigned_abs() as i32 + sz) < sl.sz as i32 {
                        self.mask(cls, &mut r, msk1 & msk, il);
                    }

                    if (msk & !msk1) != 0 {
                        let r1 = self.def(sl, msk & !msk1, bid, Some(i_idx), il);
                        if r1 == Ref::R {
                            self.f.tmps.truncate(old_ntmp);
                            self.ilog.truncate(old_nlog);
                            return if !matches!(il.typ, LocType::Load) {
                                Ref::R
                            } else {
                                self.load(sl, msk, il)
                            };
                        }
                        r = self.iins(cls, Op::Or, r, r1, il);
                    }

                    if msk == msks {
                        self.cast(&mut r, sl.cls, il);
                    }
                    return r;
                }
                crate::ir::AliasResult::MayAlias => {
                    if ld {
                        continue;
                    }
                    // Store may-aliases => must reload.
                    self.f.tmps.truncate(old_ntmp);
                    self.ilog.truncate(old_nlog);
                    return if !matches!(il.typ, LocType::Load) {
                        Ref::R
                    } else {
                        self.load(sl, msk, il)
                    };
                }
                crate::ir::AliasResult::NoAlias => {
                    continue;
                }
            }
        }

        // Check if there's already a phi in the log for this slice.
        let b_rpoid = self.f.blks[bid.0 as usize].id;
        for ist in &self.ilog {
            if ist.is_phi && ist.bid == b_rpoid {
                if let InsertNew::Phi {
                    sl: ref m,
                    phi: ref p,
                } = ist.new
                {
                    if m.r == sl.r && m.off == sl.off && m.sz == sl.sz {
                        let mut r = p.to;
                        if msk != msks {
                            self.mask(cls, &mut r, msk, il);
                        } else {
                            self.cast(&mut r, sl.cls, il);
                        }
                        return r;
                    }
                }
            }
        }

        // Check if any phi in the block kills the slice.
        {
            let b = &self.f.blks[bid.0 as usize];
            for p in &b.phi {
                if self.killsl(p.to, sl) {
                    self.f.tmps.truncate(old_ntmp);
                    self.ilog.truncate(old_nlog);
                    return if !matches!(il.typ, LocType::Load) {
                        Ref::R
                    } else {
                        self.load(sl, msk, il)
                    };
                }
            }
        }

        let npred = self.f.blks[bid.0 as usize].pred.len();
        if npred == 0 {
            self.f.tmps.truncate(old_ntmp);
            self.ilog.truncate(old_nlog);
            return if !matches!(il.typ, LocType::Load) {
                Ref::R
            } else {
                self.load(sl, msk, il)
            };
        }

        if npred == 1 {
            let bp = self.f.blks[bid.0 as usize].pred[0];
            let bp_s2 = self.f.blks[bp.0 as usize].s2;
            let bp_loop = self.f.blks[bp.0 as usize].loop_depth;
            let il_loop = self.f.blks[il.blk.0 as usize].loop_depth;
            debug_assert!(bp_loop >= il_loop);
            let mut l = il.clone();
            if bp_s2.is_some() {
                l.typ = LocType::NoLoad;
            }
            let r1 = self.def(sl, msk, bp, None, &l);
            if r1 == Ref::R {
                self.f.tmps.truncate(old_ntmp);
                self.ilog.truncate(old_nlog);
                return if !matches!(il.typ, LocType::Load) {
                    Ref::R
                } else {
                    self.load(sl, msk, il)
                };
            }
            return r1;
        }

        // Multiple predecessors — insert a phi node.
        let r = newtmp("ld", sl.cls, self.f);
        let preds: Vec<BlkId> = self.f.blks[bid.0 as usize].pred.clone();

        // Create the phi node entry in the log first (so recursive calls find it).
        let phi = Phi {
            to: r,
            cls: sl.cls,
            args: Vec::new(),
            blks: Vec::new(),
        };
        let phi_log_idx = self.ilog.len();
        self.ilog.push(Insert {
            is_phi: true,
            bid: self.f.blks[bid.0 as usize].id,
            num: 0,
            off: 0,
            new: InsertNew::Phi {
                sl: sl.clone(),
                phi,
            },
        });

        let mut phi_args = Vec::with_capacity(preds.len());
        let mut phi_blks = Vec::with_capacity(preds.len());

        for &bp in &preds {
            let bp_s2 = self.f.blks[bp.0 as usize].s2;
            let bp_loop = self.f.blks[bp.0 as usize].loop_depth;
            let il_loop = self.f.blks[il.blk.0 as usize].loop_depth;

            let l_typ =
                if bp_s2.is_none() && !matches!(il.typ, LocType::NoLoad) && bp_loop < il_loop {
                    LocType::Load
                } else {
                    LocType::NoLoad
                };

            let bp_nins = self.f.blks[bp.0 as usize].ins.len();
            let l = Loc {
                typ: l_typ,
                off: bp_nins,
                blk: bp,
            };

            let r1 = self.def(sl, msks, bp, None, &l);
            if r1 == Ref::R {
                self.f.tmps.truncate(old_ntmp);
                self.ilog.truncate(old_nlog);
                return if !matches!(il.typ, LocType::Load) {
                    Ref::R
                } else {
                    self.load(sl, msk, il)
                };
            }
            phi_args.push(r1);
            phi_blks.push(bp);
        }

        // Update the phi in the log.
        if let InsertNew::Phi { ref mut phi, .. } = self.ilog[phi_log_idx].new {
            phi.args = phi_args;
            phi.blks = phi_blks;
        }

        let mut result = r;
        if msk != msks {
            self.mask(cls, &mut result, msk, il);
        }
        result
    }
}

// ---------------------------------------------------------------------------
// loadopt — main load optimization pass
// ---------------------------------------------------------------------------

/// Load optimization pass: eliminate redundant loads using alias analysis.
///
/// Requires RPO, SSA, and alias analysis to have been run.
/// Walks each load instruction and attempts to find a prior store that
/// satisfies it, inserting phi nodes at join points as needed.
///
/// Port of C QBE's `loadopt()`.
pub fn loadopt(f: &mut Fn) {
    let mut opt = LoadOpt::new(f);

    // For each block, for each load, try to forward from stores.
    let nblk = opt.f.rpo.len();
    for n in 0..nblk {
        let bid = opt.f.rpo[n];
        let b = &opt.f.blks[bid.0 as usize];
        let nins = b.ins.len();

        for ii in 0..nins {
            let i = opt.f.blks[bid.0 as usize].ins[ii];
            if !i.op.is_load() {
                continue;
            }
            let sz = loadsz(&i);
            let sl = Slice {
                r: i.arg[0],
                off: 0,
                sz: sz as i16,
                cls: i.cls,
            };
            let l = Loc {
                typ: LocType::Root,
                off: ii,
                blk: bid,
            };
            let result = opt.def(&sl, mask_for(sz), bid, Some(ii), &l);
            // Store the result in arg[1] — we'll use it below to rewrite.
            opt.f.blks[bid.0 as usize].ins[ii].arg[1] = result;
        }
    }

    // Sort the insertion log by (bid, isphi first, off, num).
    opt.ilog.sort_by(|a, b| {
        let c = a.bid.cmp(&b.bid);
        if c != std::cmp::Ordering::Equal {
            return c;
        }
        // phis come before instructions
        let pa = if a.is_phi { 0u32 } else { 1 };
        let pb = if b.is_phi { 0u32 } else { 1 };
        let c = pa.cmp(&pb);
        if c != std::cmp::Ordering::Equal {
            return c;
        }
        if a.is_phi && b.is_phi {
            return std::cmp::Ordering::Equal;
        }
        let c = a.off.cmp(&b.off);
        if c != std::cmp::Ordering::Equal {
            return c;
        }
        a.num.cmp(&b.num)
    });

    // Apply insertions: rebuild each block's instruction list.
    let mut ist_idx = 0;
    let nlog = opt.ilog.len();

    for n in 0..nblk {
        let bid = opt.f.rpo[n];

        // Insert phis.
        while ist_idx < nlog && opt.ilog[ist_idx].bid == n as u32 && opt.ilog[ist_idx].is_phi {
            if let InsertNew::Phi { phi, .. } = opt.ilog[ist_idx].new.clone() {
                opt.f.blks[bid.0 as usize].phi.push(phi);
            }
            ist_idx += 1;
        }

        // Rebuild instruction list.
        let old_ins = opt.f.blks[bid.0 as usize].ins.clone();
        let old_nins = old_ins.len();
        let mut new_ins: Vec<Ins> = Vec::new();
        let mut ni = 0;

        loop {
            if ist_idx < nlog
                && !opt.ilog[ist_idx].is_phi
                && opt.ilog[ist_idx].bid == n as u32
                && opt.ilog[ist_idx].off == ni as u32
            {
                if let InsertNew::Ins(ins) = opt.ilog[ist_idx].new.clone() {
                    new_ins.push(ins);
                }
                ist_idx += 1;
            } else {
                if ni >= old_nins {
                    break;
                }
                let mut i = old_ins[ni];
                ni += 1;

                // Rewrite loads that were successfully forwarded.
                if i.op.is_load() && i.arg[1] != Ref::R {
                    let ext = match i.op {
                        Op::Loadsb => Op::Extsb,
                        Op::Loadub => Op::Extub,
                        Op::Loadsh => Op::Extsh,
                        Op::Loaduh => Op::Extuh,
                        Op::Loadsw => {
                            if i.cls == Cls::Kl {
                                Op::Extsw
                            } else {
                                Op::Copy
                            }
                        }
                        Op::Loaduw => {
                            if i.cls == Cls::Kl {
                                Op::Extuw
                            } else {
                                Op::Copy
                            }
                        }
                        Op::Load => Op::Copy,
                        _ => panic!("unexpected load op"),
                    };
                    i.op = ext;
                    i.arg[0] = i.arg[1];
                    i.arg[1] = Ref::R;
                }

                new_ins.push(i);
            }
        }

        opt.f.blks[bid.0 as usize].ins = new_ins;
    }
}
