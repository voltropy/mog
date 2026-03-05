//! Register spill insertion.
//!
//! Faithfully ported from QBE 1.2 `spill.c` (538 lines).
//!
//! Cost-based heuristic weighted by loop depth. When register pressure
//! exceeds the target's register count, cheapest temps are evicted to
//! stack slots with 4B/8B slot packing. Uses BSet for tracking.

use crate::ir::{BSet, Cls, Fn, Ins, Op, Ref, Target, TmpId, TMP0};
use crate::live::liveon;
use crate::util::{phicls, InsBuffer};

// ---------------------------------------------------------------------------
// Loop aggregation (port of static aggreg())
// ---------------------------------------------------------------------------

/// Aggregate looping information at loop headers.
///
/// Port of C QBE's `aggreg(Blk *hd, Blk *b)`.
fn aggreg(f: &mut Fn, hd: u32, b: u32) {
    let b_blk = &f.blks[b as usize];
    let b_gen_clone = b_blk.gen.clone();
    let b_nlive = b_blk.nlive;

    let hd_blk = &mut f.blks[hd as usize];
    hd_blk.gen.union(&b_gen_clone);
    for k in 0..2 {
        if b_nlive[k] > hd_blk.nlive[k] {
            hd_blk.nlive[k] = b_nlive[k];
        }
    }
}

// ---------------------------------------------------------------------------
// Tmp use counting (port of static tmpuse())
// ---------------------------------------------------------------------------

fn tmpuse(r: Ref, is_use: bool, loop_cost: i32, f: &mut Fn) {
    match r {
        Ref::Mem(mid) => {
            let m = f.mems[mid.0 as usize].clone();
            tmpuse(m.base, true, loop_cost, f);
            tmpuse(m.index, true, loop_cost, f);
        }
        Ref::Tmp(tid) if tid.0 >= TMP0 => {
            let t = &mut f.tmps[tid.0 as usize];
            if is_use {
                t.nuse += 1;
            } else {
                t.ndef += 1;
            }
            t.cost = t.cost.wrapping_add(loop_cost as u32);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// fillcost — evaluate spill costs
// ---------------------------------------------------------------------------

/// Evaluate spill costs of temporaries.
///
/// This also fills usage information (nuse, ndef, cost).
/// Requires rpo, preds.
///
/// Port of C QBE's `fillcost(Fn *fn)`.
pub fn fillcost(f: &mut Fn) {
    // Run loop aggregation (port of loopiter(fn, aggreg)).
    // Collect pairs first to avoid borrow conflict.
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    crate::cfg::loopiter(f, &mut |hd, b| {
        pairs.push((hd.0, b.0));
    });
    for (hd, b) in pairs {
        aggreg(f, hd, b);
    }

    // Initialize cost/use/def for all temps.
    let ntmp = f.tmps.len();
    for ti in 0..ntmp {
        if ti < TMP0 as usize {
            f.tmps[ti].cost = u32::MAX;
        } else {
            f.tmps[ti].cost = 0;
        }
        f.tmps[ti].nuse = 0;
        f.tmps[ti].ndef = 0;
    }

    // Walk all blocks, phis, and instructions to compute costs.
    let rpo = f.rpo.clone();
    for &bid in &rpo {
        let b_idx = bid.0 as usize;

        // Process phi nodes.
        let nphi = f.blks[b_idx].phi.len();
        for pi in 0..nphi {
            let phi_to = f.blks[b_idx].phi[pi].to;
            tmpuse(phi_to, false, 0, f);
            let narg = f.blks[b_idx].phi[pi].narg();
            for a in 0..narg {
                let blk = f.blks[b_idx].phi[pi].blks[a];
                let n = f.blks[blk.0 as usize].loop_depth;
                // Add loop cost to the phi destination.
                if let Ref::Tmp(tid) = phi_to {
                    f.tmps[tid.0 as usize].cost =
                        f.tmps[tid.0 as usize].cost.wrapping_add(n as u32);
                }
                let arg = f.blks[b_idx].phi[pi].args[a];
                tmpuse(arg, true, n, f);
            }
        }

        let n = f.blks[b_idx].loop_depth;

        // Process instructions.
        let nins = f.blks[b_idx].ins.len();
        for ii in 0..nins {
            let ins = f.blks[b_idx].ins[ii];
            tmpuse(ins.to, false, n, f);
            tmpuse(ins.arg[0], true, n, f);
            tmpuse(ins.arg[1], true, n, f);
        }

        // Process jump argument.
        let jmp_arg = f.blks[b_idx].jmp.arg;
        tmpuse(jmp_arg, true, n, f);
    }
}

// ---------------------------------------------------------------------------
// Slot allocation (port of static slot())
// ---------------------------------------------------------------------------

/// Slot packing state.
struct SlotState {
    locs: i32,
    slot4: i32,
    slot8: i32,
}

impl SlotState {
    fn new(locs: i32) -> Self {
        Self {
            locs,
            slot4: 0,
            slot8: 0,
        }
    }

    /// Assign a stack slot to temporary `t`.
    ///
    /// Port of C QBE's `slot(int t)`. Uses packing logic for 4B/8B slots.
    fn slot(&mut self, t: u32, tmps: &mut [crate::ir::Tmp]) -> Ref {
        assert!(t >= TMP0, "cannot spill register");
        let s = tmps[t as usize].slot;
        if s != -1 {
            return Ref::Slot(s as u32);
        }

        let s;
        if tmps[t as usize].cls.is_wide() {
            s = self.slot8;
            if self.slot4 == self.slot8 {
                self.slot4 += 2;
            }
            self.slot8 += 2;
        } else {
            s = self.slot4;
            if self.slot4 == self.slot8 {
                self.slot8 += 2;
                self.slot4 += 1;
            } else {
                self.slot4 = self.slot8;
            }
        }
        let s = s + self.locs;
        tmps[t as usize].slot = s;
        Ref::Slot(s as u32)
    }
}

// ---------------------------------------------------------------------------
// Sort helpers
// ---------------------------------------------------------------------------

/// Sort by cost descending (highest cost first = most expensive to spill).
fn sort_by_cost(arr: &mut [u32], tmps: &[crate::ir::Tmp]) {
    arr.sort_by(|&a, &b| tmps[b as usize].cost.cmp(&tmps[a as usize].cost));
}

/// Sort by: first prefer items in `fst` set, then by cost descending.
fn sort_by_fst_then_cost(arr: &mut [u32], fst: &BSet, tmps: &[crate::ir::Tmp]) {
    arr.sort_by(|&a, &b| {
        let fa = fst.has(b) as i32 - fst.has(a) as i32;
        if fa != 0 {
            return fa.cmp(&0);
        }
        tmps[b as usize].cost.cmp(&tmps[a as usize].cost)
    });
}

// ---------------------------------------------------------------------------
// limit / limit2 (port of static limit/limit2)
// ---------------------------------------------------------------------------

/// Restricts bitset `b` to hold at most `k` temporaries, preferring those
/// present in `fst_set` (if given), then those with the largest spill cost.
/// Excess temps are spilled.
fn limit(
    b: &mut BSet,
    k: i32,
    fst_set: Option<&BSet>,
    slots: &mut SlotState,
    tmps: &mut [crate::ir::Tmp],
) {
    let nt = b.count() as i32;
    if nt <= k {
        return;
    }

    // Collect all set bits.
    let mut arr: Vec<u32> = b.iter().collect();
    b.zero();

    // Sort: prefer fst members, then by cost descending.
    if let Some(fst) = fst_set {
        sort_by_fst_then_cost(&mut arr, fst, tmps);
    } else {
        sort_by_cost(&mut arr, tmps);
    }

    // Keep the first k, spill the rest.
    let k = k.max(0) as usize;
    for (i, &t) in arr.iter().enumerate() {
        if i < k {
            b.set(t);
        } else {
            slots.slot(t, tmps);
        }
    }
}

/// Spills temporaries to fit the target limits. Splits by register class,
/// limits each separately, then unions.
fn limit2(
    b: &mut BSet,
    k1: i32,
    k2: i32,
    fst_set: Option<&BSet>,
    mask: &[BSet; 2],
    t: &Target,
    slots: &mut SlotState,
    tmps: &mut [crate::ir::Tmp],
) {
    let ntmp = tmps.len() as u32;
    let mut b2 = BSet::new(ntmp);
    b2.copy_from(b);
    b.inter(&mask[0]);
    b2.inter(&mask[1]);
    limit(b, t.ngpr - k1, fst_set, slots, tmps);
    limit(&mut b2, t.nfpr - k2, fst_set, slots, tmps);
    b.union(&b2);
}

// ---------------------------------------------------------------------------
// Helper: set hints on a bitset of temps
// ---------------------------------------------------------------------------

fn sethint(u: &BSet, r: u64, tmps: &mut [crate::ir::Tmp]) {
    for t in u.iter() {
        if t >= TMP0 {
            let pc = phicls(t as usize, tmps);
            tmps[pc].hint.m |= r;
        }
    }
}

// ---------------------------------------------------------------------------
// reloads / store helpers
// ---------------------------------------------------------------------------

/// Emit reloads for temps in `u` that are not in `v`.
fn reloads(
    u: &BSet,
    v: &BSet,
    buf: &mut InsBuffer,
    slots: &mut SlotState,
    tmps: &mut [crate::ir::Tmp],
) {
    for t in u.iter() {
        if t >= TMP0 && !v.has(t) {
            let cls = tmps[t as usize].cls;
            let s = slots.slot(t, tmps);
            buf.emit(Op::Load, cls, Ref::Tmp(TmpId(t)), s, Ref::R);
        }
    }
}

/// Emit a store for a ref if it has a slot.
fn store(r: Ref, buf: &mut InsBuffer, tmps: &[crate::ir::Tmp]) {
    if let Ref::Tmp(tid) = r {
        let s = tmps[tid.0 as usize].slot;
        if s != -1 {
            let store_op = match tmps[tid.0 as usize].cls {
                Cls::Kw => Op::Storew,
                Cls::Kl => Op::Storel,
                Cls::Ks => Op::Stores,
                Cls::Kd => Op::Stored,
                Cls::Kx => Op::Storew,
            };
            buf.emit(store_op, Cls::Kw, Ref::R, r, Ref::Slot(s as u32));
        }
    }
}

fn regcpy(ins: &Ins) -> bool {
    ins.op == Op::Copy && crate::ir::isreg(ins.arg[0])
}

// ---------------------------------------------------------------------------
// dopm — handle parallel moves (consecutive register copies)
// ---------------------------------------------------------------------------

/// Process a block of consecutive register copy instructions as a single unit.
///
/// Port of C QBE's `dopm()` in spill.c.
///
/// Returns the new instruction index (pointing before the PM block).
fn dopm(
    f: &mut Fn,
    b_idx: usize,
    mut i: usize,
    v: &mut BSet,
    buf: &mut InsBuffer,
    slots: &mut SlotState,
    mask: &[BSet; 2],
    t: &Target,
) -> usize {
    let ntmp = f.tmps.len() as u32;
    let mut u = BSet::new(ntmp);

    // Find the start of the PM block (consecutive regcpy's).
    let start = {
        let mut s = i;
        while s > 0 && regcpy(&f.blks[b_idx].ins[s - 1]) {
            s -= 1;
        }
        s
    };

    // Process from i down to start.
    let j = i + 1;
    loop {
        let ins = f.blks[b_idx].ins[i];
        if ins.to != Ref::R {
            let tv = ins.to.val();
            if v.has(tv) {
                v.clr(tv);
                store(ins.to, buf, &f.tmps);
            }
        }
        if let Ref::Tmp(tid) = ins.arg[0] {
            v.set(tid.0);
        }
        if i == start {
            break;
        }
        i -= 1;
    }

    u.copy_from(v);

    // Check if preceded by a call.
    if start > 0 && f.blks[b_idx].ins[start - 1].op == Op::Call {
        let call_ref = f.blks[b_idx].ins[start - 1].arg[1];
        let retregs = t.retregs(call_ref, None);
        if !v.bits_raw().is_empty() {
            v.bits_raw_mut()[0] &= !retregs;
        }
        limit2(
            v,
            t.nrsave[0],
            t.nrsave[1],
            None,
            mask,
            t,
            slots,
            &mut f.tmps,
        );
        let mut _r: u64 = 0;
        for &rs in t.rsave {
            if rs < 0 {
                break;
            }
            _r |= 1u64 << rs;
        }
        let argregs = t.argregs(call_ref, None);
        if !v.bits_raw().is_empty() {
            v.bits_raw_mut()[0] |= argregs;
        }
    } else {
        limit2(v, 0, 0, None, mask, t, slots, &mut f.tmps);
    }

    let r_val = v.bits_raw().first().copied().unwrap_or(0);
    sethint(v, r_val, &mut f.tmps);
    reloads(&u, v, buf, slots, &mut f.tmps);

    // Emit the PM instructions in order.
    for idx in (start..j).rev() {
        buf.emiti(f.blks[b_idx].ins[idx]);
    }

    start
}

// ---------------------------------------------------------------------------
// merge — merge live sets considering loop depth
// ---------------------------------------------------------------------------

fn merge(u: &mut BSet, bu_loop: i32, v: &BSet, bv_loop: i32, tmps: &[crate::ir::Tmp]) {
    if bu_loop <= bv_loop {
        u.union(v);
    } else {
        for t in v.iter() {
            if tmps[t as usize].slot == -1 {
                u.set(t);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// spill — main spill pass
// ---------------------------------------------------------------------------

/// Insert spill and reload instructions.
///
/// Port of C QBE's `spill(Fn *fn)`.
///
/// Requires spill costs (fillcost), rpo, liveness (filllive).
///
/// Note: this replaces liveness information (in, out) with temporaries
/// that must be in registers at block borders.
pub fn spill(f: &mut Fn, t: &Target) {
    let ntmp = f.tmps.len() as u32;

    let mut u = BSet::new(ntmp);
    let mut v = BSet::new(ntmp);
    let mut w = BSet::new(ntmp);
    let mut mask = [BSet::new(ntmp), BSet::new(ntmp)];

    let mut slots = SlotState::new(f.slot);

    // Build register class masks.
    for ti in 0..ntmp as i32 {
        let mut k = 0;
        if ti >= t.fpr0 && ti < t.fpr0 + t.nfpr {
            k = 1;
        }
        if ti >= TMP0 as i32 {
            let cls = f.tmps[ti as usize].cls;
            k = cls.base().max(0) as usize;
        }
        mask[k].set(ti as u32);
    }

    // Process blocks in reverse RPO.
    let rpo = f.rpo.clone();
    for bp_idx in (0..rpo.len()).rev() {
        let bid = rpo[bp_idx];
        let b_idx = bid.0 as usize;
        let mut buf = InsBuffer::new();

        // 1. Find temporaries in registers at end of block (put in v).
        let s1 = f.blks[b_idx].s1;
        let s2 = f.blks[b_idx].s2;

        // Detect back-edge (loop header).
        let mut hd: Option<u32> = None;
        if let Some(s) = s1 {
            if f.blks[s.0 as usize].id <= f.blks[b_idx].id {
                hd = Some(s.0);
            }
        }
        if let Some(s) = s2 {
            if f.blks[s.0 as usize].id <= f.blks[b_idx].id {
                if hd.is_none() || f.blks[s.0 as usize].id >= f.blks[hd.unwrap() as usize].id {
                    hd = Some(s.0);
                }
            }
        }

        if let Some(hd_id) = hd {
            // Back-edge handling.
            v.zero();
            // Don't spill global registers.
            if !f.blks[hd_id as usize].gen.bits_raw().is_empty() {
                f.blks[hd_id as usize].gen.bits_raw_mut()[0] |= t.rglob;
            }
            for k in 0..2usize {
                let n = if k == 0 { t.ngpr } else { t.nfpr };
                u.copy_from(&f.blks[b_idx].out);
                u.inter(&mask[k]);
                w.copy_from(&u);
                u.inter(&f.blks[hd_id as usize].gen);
                w.diff(&f.blks[hd_id as usize].gen);
                if (u.count() as i32) < n {
                    let j = w.count() as i32;
                    let l = f.blks[hd_id as usize].nlive[k];
                    limit(&mut w, n - (l - j), None, &mut slots, &mut f.tmps);
                    u.union(&w);
                } else {
                    limit(&mut u, n, None, &mut slots, &mut f.tmps);
                }
                v.union(&u);
            }
        } else if s1.is_some() {
            // Normal successors — avoid reloading in middle of loops.
            v.zero();
            liveon(f, &mut w, bid.0, s1.unwrap().0);
            let b_loop = f.blks[b_idx].loop_depth;
            let s1_loop = f.blks[s1.unwrap().0 as usize].loop_depth;
            merge(&mut v, b_loop, &w, s1_loop, &f.tmps);

            if let Some(s2_id) = s2 {
                liveon(f, &mut u, bid.0, s2_id.0);
                let s2_loop = f.blks[s2_id.0 as usize].loop_depth;
                merge(&mut v, b_loop, &u, s2_loop, &f.tmps);
                w.inter(&u);
            }
            limit2(&mut v, 0, 0, Some(&w), &mask, t, &mut slots, &mut f.tmps);
        } else {
            // No successors (return block).
            v.copy_from(&f.blks[b_idx].out);
            if let Ref::Call(_) = f.blks[b_idx].jmp.arg {
                let retregs = t.retregs(f.blks[b_idx].jmp.arg, None);
                if !v.bits_raw().is_empty() {
                    v.bits_raw_mut()[0] |= retregs;
                }
            }
        }

        // Spill any out-of-register temps.
        for ti in f.blks[b_idx].out.iter() {
            if ti >= TMP0 && !v.has(ti) {
                slots.slot(ti, &mut f.tmps);
            }
        }
        f.blks[b_idx].out.copy_from(&v);

        // 2. Process block instructions.
        // Handle jump argument.
        if let Ref::Tmp(tid) = f.blks[b_idx].jmp.arg {
            let tv = tid.0;
            assert!(f.tmps[tv as usize].cls.base() == 0);
            let lvarg = v.has(tv);
            v.set(tv);
            u.copy_from(&v);
            limit2(&mut v, 0, 0, None, &mask, t, &mut slots, &mut f.tmps);
            if !v.has(tv) {
                if !lvarg {
                    u.clr(tv);
                }
                f.blks[b_idx].jmp.arg = slots.slot(tv, &mut f.tmps);
            }
            reloads(&u, &v, &mut buf, &mut slots, &mut f.tmps);
        }

        // Walk instructions bottom-to-top.
        let nins = f.blks[b_idx].ins.len();
        let mut i = nins;
        while i > 0 {
            i -= 1;

            if regcpy(&f.blks[b_idx].ins[i]) {
                i = dopm(f, b_idx, i, &mut v, &mut buf, &mut slots, &mask, t);
                continue;
            }

            w.zero();
            let ins = f.blks[b_idx].ins[i];

            // Handle definition.
            if ins.to != Ref::R {
                let tv = ins.to.val();
                if v.has(tv) {
                    v.clr(tv);
                } else {
                    // Make sure we have a reg for the result.
                    assert!(tv >= TMP0, "dead reg");
                    v.set(tv);
                    w.set(tv);
                }
            }

            // Count memory args.
            let mut j = t.memargs(ins.op);
            for n in 0..2 {
                if let Ref::Mem(_) = ins.arg[n] {
                    j -= 1;
                }
            }

            // Handle uses.
            let mut lvarg = [false; 2];
            for n in 0..2usize {
                match ins.arg[n] {
                    Ref::Mem(mid) => {
                        let m = f.mems[mid.0 as usize].clone();
                        if let Ref::Tmp(tid) = m.base {
                            v.set(tid.0);
                            w.set(tid.0);
                        }
                        if let Ref::Tmp(tid) = m.index {
                            v.set(tid.0);
                            w.set(tid.0);
                        }
                    }
                    Ref::Tmp(tid) => {
                        let tv = tid.0;
                        lvarg[n] = v.has(tv);
                        v.set(tv);
                        if j <= 0 {
                            w.set(tv);
                        }
                        j -= 1;
                    }
                    _ => {}
                }
            }

            u.copy_from(&v);
            limit2(&mut v, 0, 0, Some(&w), &mask, t, &mut slots, &mut f.tmps);

            // Replace spilled arguments with slots.
            for n in 0..2usize {
                if let Ref::Tmp(tid) = f.blks[b_idx].ins[i].arg[n] {
                    let tv = tid.0;
                    if !v.has(tv) {
                        if !lvarg[n] {
                            u.clr(tv);
                        }
                        f.blks[b_idx].ins[i].arg[n] = slots.slot(tv, &mut f.tmps);
                    }
                }
            }

            reloads(&u, &v, &mut buf, &mut slots, &mut f.tmps);

            // Store the result if it has a slot.
            if ins.to != Ref::R {
                let tv = ins.to.val();
                store(ins.to, &mut buf, &f.tmps);
                if tv >= TMP0 {
                    v.clr(tv);
                }
            }

            buf.emiti(f.blks[b_idx].ins[i]);

            let r = v.bits_raw().first().copied().unwrap_or(0);
            if r != 0 {
                sethint(&v, r, &mut f.tmps);
            }
        }

        // Process phis.
        let nphi = f.blks[b_idx].phi.len();
        for pi in 0..nphi {
            let phi_to = f.blks[b_idx].phi[pi].to;
            if let Ref::Tmp(tid) = phi_to {
                let tv = tid.0;
                if v.has(tv) {
                    v.clr(tv);
                    store(phi_to, &mut buf, &f.tmps);
                } else if f.blks[b_idx].r#in.has(tv) {
                    // Only if the phi is live.
                    f.blks[b_idx].phi[pi].to = slots.slot(tv, &mut f.tmps);
                }
            }
        }

        f.blks[b_idx].r#in.copy_from(&v);

        // Replace the block's instruction list with the buffered version.
        let new_ins = buf.finish();
        f.blks[b_idx].ins = new_ins;
    }

    // Align locals to 16-byte boundary (specific to NAlign == 3).
    slots.slot8 += slots.slot8 & 3;
    f.slot += slots.slot8;
}
