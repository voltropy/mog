//! Slot promotion and coalescing.
//!
//! Port of QBE's `mem.c`. Contains two passes:
//! - `promote()`: mem2reg — promotes uniformly-accessed stack allocs to SSA temps
//! - `coalesce()`: backward liveness on slots, dead store elimination,
//!   greedy size-sorted fusion of non-overlapping slot live ranges

use crate::alias::getalias;
use crate::ir::OP_TABLE;
use crate::ir::{AliasType, BlkId, Cls, Fn, Ins, Jmp, Op, Ref, TmpId, N_BIT};
use crate::load::{loadsz, storesz};

// ---------------------------------------------------------------------------
// promote — mem2reg for uniform stack slots
// ---------------------------------------------------------------------------

/// Promote uniformly-accessed stack allocations to SSA temporaries.
///
/// Scans alloc instructions in the start block. If all uses of an alloc
/// are loads/stores of a uniform size, the alloc is removed and loads/stores
/// are replaced with copies/extensions.
///
/// Requires use-def chains. Maintains use counts.
///
/// Port of C QBE's `promote()`.
pub fn promote(f: &mut Fn) {
    let start = f.start;
    let start_ins: Vec<Ins> = f.blks[start.0 as usize].ins.clone();

    for (ins_idx, i) in start_ins.iter().enumerate() {
        if !i.op.is_alloc() {
            continue;
        }

        let to_id = match i.to {
            Ref::Tmp(id) => id,
            _ => continue,
        };

        let t = &f.tmps[to_id.0 as usize];
        if t.ndef != 1 {
            continue;
        }

        // Check all uses for uniformity.
        let mut k: i8 = -1; // store class (Cls as i8), -1 = unset
        let mut s: i32 = -1; // uniform size, -1 = unset
        let mut ok = true;

        for u in &t.uses {
            match u.typ {
                crate::ir::UseType::Ins => {
                    // Find the instruction from the use.
                    let use_blk = &f.blks[u.bid as usize];
                    let use_ins = match u.detail {
                        crate::ir::UseDetail::InsIdx(idx) => {
                            if (idx as usize) >= use_blk.ins.len() {
                                ok = false;
                                break;
                            }
                            &use_blk.ins[idx as usize]
                        }
                        _ => {
                            ok = false;
                            break;
                        }
                    };

                    if use_ins.op.is_load() {
                        let lsz = loadsz(use_ins);
                        if s == -1 || s == lsz {
                            s = lsz;
                            continue;
                        }
                        ok = false;
                        break;
                    }

                    if use_ins.op.is_store() {
                        // For stores, the alloc must be arg[1] (the address),
                        // not arg[0] (the value).
                        if use_ins.arg[1] == Ref::Tmp(to_id) && use_ins.arg[0] != Ref::Tmp(to_id) {
                            let ssz = storesz(use_ins);
                            let op_idx = use_ins.op as usize;
                            let store_cls = OP_TABLE[op_idx].argcls[0][0] as i8;
                            if (s == -1 || s == ssz) && (k == -1 || k == store_cls) {
                                s = ssz;
                                k = store_cls;
                                continue;
                            }
                        }
                        ok = false;
                        break;
                    }

                    // Any other use type → skip this alloc.
                    ok = false;
                    break;
                }
                _ => {
                    ok = false;
                    break;
                }
            }
        }

        if !ok {
            continue;
        }

        let k_cls = if k >= 0 { Cls::from_i8(k) } else { Cls::Kx };

        // Remove the alloc.
        f.blks[start.0 as usize].ins[ins_idx] = Ins {
            op: Op::Nop,
            cls: Cls::Kx,
            to: Ref::R,
            arg: [Ref::R, Ref::R],
        };
        f.tmps[to_id.0 as usize].ndef -= 1;

        // Rewrite all uses.
        let uses = f.tmps[to_id.0 as usize].uses.clone();
        for u in &uses {
            if u.typ != crate::ir::UseType::Ins {
                continue;
            }
            let use_idx = match u.detail {
                crate::ir::UseDetail::InsIdx(idx) => idx as usize,
                _ => continue,
            };
            let bid = u.bid as usize;

            let l = f.blks[bid].ins[use_idx];

            if l.op.is_store() {
                // Store → copy: the stored value becomes a def of the alloc temp.
                f.blks[bid].ins[use_idx] = Ins {
                    op: Op::Copy,
                    cls: k_cls,
                    to: l.arg[1], // was the address = alloc temp
                    arg: [l.arg[0], Ref::R],
                };
                f.tmps[to_id.0 as usize].nuse -= 1;
                f.tmps[to_id.0 as usize].ndef += 1;
            } else {
                // Load → copy/extension.
                if k < 0 {
                    panic!(
                        "slot %{} is read but never stored to",
                        f.tmps[to_id.0 as usize].name
                    );
                }
                let new_op = match l.op {
                    Op::Loadsw | Op::Loaduw => {
                        if k_cls == Cls::Kl {
                            // Need extension.
                            ext_op(l.op)
                        } else {
                            // Same base class?
                            if k_cls.base() != l.cls.base() {
                                Op::Cast
                            } else {
                                Op::Copy
                            }
                        }
                    }
                    Op::Load => {
                        if k_cls.base() != l.cls.base() {
                            Op::Cast
                        } else {
                            Op::Copy
                        }
                    }
                    _ => ext_op(l.op),
                };
                f.blks[bid].ins[use_idx].op = new_op;
            }
        }
    }
}

/// Map a load opcode to its corresponding extension opcode.
fn ext_op(op: Op) -> Op {
    match op {
        Op::Loadsb => Op::Extsb,
        Op::Loadub => Op::Extub,
        Op::Loadsh => Op::Extsh,
        Op::Loaduh => Op::Extuh,
        Op::Loadsw => Op::Extsw,
        Op::Loaduw => Op::Extuw,
        _ => panic!("ext_op: not a sub-word load: {:?}", op),
    }
}

// ---------------------------------------------------------------------------
// coalesce — slot liveness, dead store elimination, fusion
// ---------------------------------------------------------------------------

/// Half-open range `[a, b)` with `0 <= a`. A range with `b == 0` is empty.
#[derive(Clone, Copy, Default)]
struct Range {
    a: i32,
    b: i32,
}

impl Range {
    #[inline]
    fn contains(self, n: i32) -> bool {
        self.a <= n && n < self.b
    }

    #[inline]
    fn overlaps(self, other: Range) -> bool {
        self.b != 0 && other.b != 0 && self.a < other.b && other.a < self.b
    }

    fn add(&mut self, n: i32) {
        if self.b == 0 {
            self.a = n;
            self.b = n + 1;
        } else if n < self.a {
            self.a = n;
        } else if n >= self.b {
            self.b = n + 1;
        }
    }
}

/// A dead store record.
#[derive(Clone)]
struct Store {
    ip: i32,
    blk_idx: usize,
    ins_idx: usize,
}

/// A stack slot being analyzed.
#[derive(Clone)]
struct Slot {
    /// Tmp index of the alloc.
    t: u32,
    /// Slot size in bytes.
    sz: i32,
    /// Bitmask of accessed bytes.
    m: u64,
    /// Current liveness bitmask (during backward scan).
    l: u64,
    /// Live range.
    r: Range,
    /// Fused-to slot index, or own index if representative.
    fused: Option<usize>,
    /// Dead store candidates.
    stores: Vec<Store>,
}

/// Resolve a ref to a slot index and offset.
fn resolve_slot(
    r: Ref,
    f: &Fn,
    slot_map: &[i32], // indexed by tmp, -1 for no slot
) -> Option<(usize, i64)> {
    let a = getalias(f, r);
    if a.typ != AliasType::Loc {
        return None;
    }
    let base_tmp = a.base;
    let visit = slot_map.get(base_tmp as usize).copied().unwrap_or(-1);
    if visit < 0 {
        return None;
    }
    Some((visit as usize, a.offset))
}

/// Load-side liveness update.
fn slot_load(r: Ref, x: u64, ip: i32, f: &Fn, slots: &mut [Slot], slot_map: &[i32]) {
    if let Some((si, off)) = resolve_slot(r, f, slot_map) {
        let s = &mut slots[si];
        s.l |= x << off;
        s.l &= s.m;
        if s.l != 0 {
            s.r.add(ip);
        }
    }
}

/// Store-side liveness update.
fn slot_store(
    r: Ref,
    x: u64,
    ip: i32,
    blk_idx: usize,
    ins_idx: usize,
    f: &Fn,
    slots: &mut [Slot],
    slot_map: &[i32],
) {
    if let Some((si, off)) = resolve_slot(r, f, slot_map) {
        let s = &mut slots[si];
        if s.l != 0 {
            s.r.add(ip);
            s.l &= !(x << off);
        } else {
            s.stores.push(Store {
                ip,
                blk_idx,
                ins_idx,
            });
        }
    }
}

/// Coalesce memory operations: liveness analysis, dead store elimination,
/// and greedy size-sorted fusion of non-overlapping slot live ranges.
///
/// Minimizes stack usage by merging slots with non-overlapping live ranges.
///
/// Port of C QBE's `coalesce()`.
pub fn coalesce(f: &mut Fn) {
    let ntmp = f.tmps.len();

    // Build slot list — only ALoc allocs in the start block with known size.
    let mut slots: Vec<Slot> = Vec::new();
    let mut slot_map: Vec<i32> = vec![-1; ntmp]; // tmp -> slot index
    let start_bid = f.start;

    for n in 0..ntmp {
        let t = &f.tmps[n];
        if t.alias.typ != AliasType::Loc {
            continue;
        }
        // Check that this is the slot itself (base == self).
        if t.alias.base != n as i32 {
            continue;
        }
        if t.bid != start_bid.0 {
            continue;
        }
        if t.alias.loc.sz < 0 {
            continue;
        }

        slot_map[n] = slots.len() as i32;
        slots.push(Slot {
            t: n as u32,
            sz: t.alias.loc.sz,
            m: t.alias.loc.m,
            l: 0,
            r: Range::default(),
            fused: None,
            stores: Vec::new(),
        });
    }

    let nsl = slots.len();
    if nsl == 0 {
        return;
    }

    // One-pass backward liveness analysis.
    // First, compute max RPO id for loop headers.
    let nblk = f.rpo.len();
    let mut blk_loop: Vec<i32> = vec![-1; f.blks.len()];

    {
        // Use loopiter to find natural loops.
        // Collect back-edges and compute loop bounds.
        // This replicates loopiter's maxrpo callback without
        // needing a mutable Fn borrow.
        for ni in 0..nblk {
            let bid = f.rpo[ni];
            let b = &f.blks[bid.0 as usize];
            let preds: Vec<BlkId> = b.pred.clone();
            for &p in &preds {
                let p_id = f.blks[p.0 as usize].id;
                if p_id >= ni as u32 {
                    // Back-edge: p -> bid. bid is a loop header.
                    // We need to mark the max RPO of blocks in this loop.
                    // For simplicity, use p's RPO id as a conservative bound.
                    if blk_loop[bid.0 as usize] < p_id as i32 {
                        blk_loop[bid.0 as usize] = p_id as i32;
                    }
                }
            }
        }
    }

    // Track blit instructions for later fixup.
    let mut blit_locs: Vec<(usize, usize)> = Vec::new(); // (blk_idx, ins_idx)

    // Block ranges for IP mapping, indexed by RPO id (matching C's br[n]).
    let mut blk_range: Vec<Range> = vec![Range::default(); nblk];
    let mut ip = i32::MAX - 1;

    for n in (0..nblk).rev() {
        let bid = f.rpo[n];
        let b_idx = bid.0 as usize;

        blk_range[n].b = ip;
        ip -= 1;

        // Initialize slot liveness from successors.
        let succ: Vec<Option<BlkId>> = {
            let b = &f.blks[b_idx];
            vec![b.s1, b.s2]
        };

        for s in &mut slots {
            s.l = 0;
            for succ_opt in &succ {
                if let Some(succ_id) = succ_opt {
                    let m = f.blks[succ_id.0 as usize].id as usize;
                    if m > n && s.r.contains(blk_range[m].a) {
                        s.l = s.m;
                        s.r.add(ip);
                    }
                }
            }
        }

        // Handle jump argument.
        let jmp = f.blks[b_idx].jmp;
        if jmp.typ == Jmp::Retc {
            ip -= 1;
            slot_load(jmp.arg, !0u64, ip, f, &mut slots, &slot_map);
        }

        // Walk instructions backward.
        let ins_list: Vec<Ins> = f.blks[b_idx].ins.clone();
        let nins = ins_list.len();
        let mut ii = nins;
        while ii > 0 {
            ii -= 1;
            let i = &ins_list[ii];

            if i.op == Op::Argc {
                ip -= 1;
                slot_load(i.arg[1], !0u64, ip, f, &mut slots, &slot_map);
            }

            if i.op.is_load() {
                let x = (1u64 << loadsz(i)) - 1;
                ip -= 1;
                slot_load(i.arg[0], x, ip, f, &mut slots, &slot_map);
            }

            if i.op.is_store() {
                let x = (1u64 << storesz(i)) - 1;
                slot_store(i.arg[1], x, ip, b_idx, ii, f, &mut slots, &slot_map);
                ip -= 1;
            }

            if i.op == Op::Blit0 {
                // blit0 must be followed by blit1
                assert!(ii + 1 < nins);
                let i2 = &ins_list[ii + 1];
                assert!(i2.op == Op::Blit1);
                let sz = match i2.arg[0] {
                    Ref::Int(v) => (v as i32).abs(),
                    _ => panic!("blit1: expected Int arg"),
                };
                let x = if sz as u32 >= N_BIT {
                    !0u64
                } else {
                    (1u64 << sz) - 1
                };
                slot_store(i.arg[1], x, ip, b_idx, ii, f, &mut slots, &slot_map);
                ip -= 1;
                slot_load(i.arg[0], x, ip, f, &mut slots, &slot_map);
                blit_locs.push((b_idx, ii));
            }
        }

        // Extend live ranges for loop headers.
        for s in &mut slots {
            if s.l != 0 {
                s.r.add(ip);
                let loop_end = blk_loop[b_idx];
                if loop_end != -1 {
                    assert!(loop_end as usize > n);
                    s.r.add(blk_range[loop_end as usize].b - 1);
                }
            }
        }

        blk_range[n].a = ip;
    }

    // Kill dead stores.
    for s in &slots {
        for st in &s.stores {
            if !s.r.contains(st.ip) {
                let ins = &mut f.blks[st.blk_idx].ins;
                if ins[st.ins_idx].op == Op::Blit0 {
                    ins[st.ins_idx + 1] = Ins {
                        op: Op::Nop,
                        cls: Cls::Kx,
                        to: Ref::R,
                        arg: [Ref::R, Ref::R],
                    };
                }
                ins[st.ins_idx] = Ins {
                    op: Op::Nop,
                    cls: Cls::Kx,
                    to: Ref::R,
                    arg: [Ref::R, Ref::R],
                };
            }
        }
    }

    // Kill slots with empty live ranges.
    let mut dead_tmps: Vec<u32> = Vec::new();
    let mut live_slots: Vec<Slot> = Vec::new();

    for s in slots.drain(..) {
        if s.r.b == 0 {
            dead_tmps.push(s.t);
        } else {
            live_slots.push(s);
        }
    }

    // Process dead temps — recursively kill their defs and uses.
    let mut stk = dead_tmps;
    while let Some(t) = stk.pop() {
        let tmp = &f.tmps[t as usize];
        assert!(tmp.ndef == 1);

        // Find and kill the definition.
        if let Some(def_idx) = tmp.def {
            let def_bid = tmp.bid as usize;
            let def_ins = f.blks[def_bid].ins[def_idx];
            if def_ins.op.is_load() {
                // Replace load with copy of UNDEF.
                f.blks[def_bid].ins[def_idx] = Ins {
                    op: Op::Copy,
                    cls: def_ins.cls,
                    to: def_ins.to,
                    arg: [Ref::UNDEF, Ref::R],
                };
                continue;
            }
            f.blks[def_bid].ins[def_idx] = Ins {
                op: Op::Nop,
                cls: Cls::Kx,
                to: Ref::R,
                arg: [Ref::R, Ref::R],
            };
        }

        // Kill uses.
        let uses = f.tmps[t as usize].uses.clone();
        for u in &uses {
            match u.typ {
                crate::ir::UseType::Jmp => {
                    let b = &mut f.blks[u.bid as usize];
                    if b.jmp.typ.is_ret() {
                        b.jmp.typ = Jmp::Ret0;
                        b.jmp.arg = Ref::R;
                    }
                }
                crate::ir::UseType::Ins => {
                    let idx = match u.detail {
                        crate::ir::UseDetail::InsIdx(i) => i as usize,
                        _ => continue,
                    };
                    let ins = &f.blks[u.bid as usize].ins[idx];
                    if ins.to != Ref::R {
                        if let Ref::Tmp(to_id) = ins.to {
                            stk.push(to_id.0);
                        }
                    } else if ins.op.is_arg() {
                        if ins.op == Op::Argc {
                            f.blks[u.bid as usize].ins[idx].arg[1] = Ref::CON_Z;
                        }
                    } else {
                        if ins.op == Op::Blit0 {
                            f.blks[u.bid as usize].ins[idx + 1] = Ins {
                                op: Op::Nop,
                                cls: Cls::Kx,
                                to: Ref::R,
                                arg: [Ref::R, Ref::R],
                            };
                        }
                        f.blks[u.bid as usize].ins[idx] = Ins {
                            op: Op::Nop,
                            cls: Cls::Kx,
                            to: Ref::R,
                            arg: [Ref::R, Ref::R],
                        };
                    }
                }
                _ => {}
            }
        }
    }

    // Update slot_map for remaining live slots.
    let nsl = live_slots.len();
    for (i, s) in live_slots.iter().enumerate() {
        slot_map[s.t as usize] = i as i32;
    }

    // Fuse slots by decreasing size.
    live_slots.sort_by(|a, b| {
        if a.sz != b.sz {
            b.sz.cmp(&a.sz) // larger first
        } else {
            a.r.a.cmp(&b.r.a)
        }
    });

    for n in 0..nsl {
        if live_slots[n].fused.is_some() {
            continue;
        }
        live_slots[n].fused = Some(n); // representative = self
        let mut r = live_slots[n].r;

        for si in (n + 1)..nsl {
            if live_slots[si].fused.is_some() || live_slots[si].r.b == 0 {
                continue;
            }
            // Check overlap with all slots already fused to this group.
            let mut can_fuse = true;
            if r.overlaps(live_slots[si].r) {
                // Check pairwise with all members of the group.
                for mi in n..si {
                    if live_slots[mi].fused == Some(n) {
                        if live_slots[mi].r.overlaps(live_slots[si].r) {
                            can_fuse = false;
                            break;
                        }
                    }
                }
            }
            if !can_fuse {
                continue;
            }
            r.add(live_slots[si].r.a);
            r.add(live_slots[si].r.b - 1);
            live_slots[si].fused = Some(n);
        }
    }

    // Substitute fused slots.
    // Update visit (slot_map) for the new ordering.
    for (i, s) in live_slots.iter().enumerate() {
        slot_map[s.t as usize] = i as i32;
    }

    for si in 0..nsl {
        let fused_to = match live_slots[si].fused {
            Some(f) => f,
            None => continue,
        };
        if fused_to == si {
            continue; // representative, no substitution needed
        }

        let src_t = live_slots[si].t;
        let dst_t = live_slots[fused_to].t;

        // Nop the definition of the source slot.
        let tmp = &f.tmps[src_t as usize];
        if let Some(def_idx) = tmp.def {
            let def_bid = tmp.bid as usize;
            let dst_tmp = &f.tmps[dst_t as usize];

            // Ensure the representative's def dominates.
            if let Some(dst_def_idx) = dst_tmp.def {
                let dst_def_bid = dst_tmp.bid as usize;
                if def_bid == dst_def_bid && def_idx < dst_def_idx {
                    // src def comes before dst def — swap defs.
                    let dst_ins = f.blks[dst_def_bid].ins[dst_def_idx];
                    f.blks[def_bid].ins[def_idx] = dst_ins;
                    f.blks[dst_def_bid].ins[dst_def_idx] = Ins {
                        op: Op::Nop,
                        cls: Cls::Kx,
                        to: Ref::R,
                        arg: [Ref::R, Ref::R],
                    };
                    f.tmps[dst_t as usize].def = Some(def_idx);
                } else {
                    f.blks[def_bid].ins[def_idx] = Ins {
                        op: Op::Nop,
                        cls: Cls::Kx,
                        to: Ref::R,
                        arg: [Ref::R, Ref::R],
                    };
                }
            } else {
                f.blks[def_bid].ins[def_idx] = Ins {
                    op: Op::Nop,
                    cls: Cls::Kx,
                    to: Ref::R,
                    arg: [Ref::R, Ref::R],
                };
            }
        }

        // Rewrite uses of the source to use the destination.
        let uses = f.tmps[src_t as usize].uses.clone();
        let src_ref = Ref::Tmp(TmpId(src_t));
        let dst_ref = Ref::Tmp(TmpId(dst_t));

        for u in &uses {
            match u.typ {
                crate::ir::UseType::Jmp => {
                    let b = &mut f.blks[u.bid as usize];
                    if b.jmp.arg == src_ref {
                        b.jmp.arg = dst_ref;
                    }
                }
                crate::ir::UseType::Ins => {
                    let idx = match u.detail {
                        crate::ir::UseDetail::InsIdx(i) => i as usize,
                        _ => continue,
                    };
                    let ins = &mut f.blks[u.bid as usize].ins[idx];
                    for a in 0..2 {
                        if ins.arg[a] == src_ref {
                            ins.arg[a] = dst_ref;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Fix newly overlapping blits.
    for &(blk_idx, ins_idx) in &blit_locs {
        let ins = &f.blks[blk_idx].ins;
        if ins[ins_idx].op != Op::Blit0 {
            continue;
        }

        let src = ins[ins_idx].arg[0];
        let dst = ins[ins_idx].arg[1];

        if let (Some((s_si, off0)), Some((d_si, off1))) = (
            resolve_slot(src, f, &slot_map),
            resolve_slot(dst, f, &slot_map),
        ) {
            let s_fused = live_slots.get(s_si).and_then(|s| s.fused);
            let d_fused = live_slots.get(d_si).and_then(|s| s.fused);
            if s_fused == d_fused && off0 < off1 {
                // Overlapping blit — negate the size to indicate backward copy.
                let i2 = &f.blks[blk_idx].ins[ins_idx + 1];
                if let Ref::Int(v) = i2.arg[0] {
                    let sz = v as i32;
                    assert!(sz >= 0);
                    f.blks[blk_idx].ins[ins_idx + 1].arg[0] = Ref::Int(-sz);
                }
            }
        }
    }
}
