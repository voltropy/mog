//! Register allocation.
//!
//! Faithfully ported from QBE 1.2 `rega.c` (698 lines).
//!
//! Assigns physical registers to virtual temporaries, resolves parallel
//! moves, inserts bridge blocks for phi resolution, and tracks register
//! usage for the function.

use crate::ir::{BSet, Cls, Fn, Ins, Jmp, Op, Ref, Target, TmpId, TMP0};
use crate::util::{phicls, InsBuffer};

// ---------------------------------------------------------------------------
// RMap — register ↔ temporary mapping
// ---------------------------------------------------------------------------

/// Maximum number of simultaneous register mappings.
/// In C QBE this is Tmp0 (= NBit = 64).
const RMAP_MAX: usize = TMP0 as usize;

/// Tracks the current mapping between temporaries and physical registers.
///
/// Port of C QBE's `struct RMap`.
#[derive(Clone)]
struct RMap {
    /// Temporary for each mapping slot.
    t: [i32; RMAP_MAX],
    /// Register for each mapping slot.
    r: [i32; RMAP_MAX],
    /// Wait list: `w[reg]` = tmp that wants this register (for hints).
    w: [i32; RMAP_MAX],
    /// Bitset tracking which tmps and registers are allocated.
    b: BSet,
    /// Number of active mappings.
    n: usize,
}

impl RMap {
    fn new(ntmp: u32) -> Self {
        Self {
            t: [0; RMAP_MAX],
            r: [0; RMAP_MAX],
            w: [0; RMAP_MAX],
            b: BSet::new(ntmp),
            n: 0,
        }
    }

    fn copy_from(&mut self, other: &RMap) {
        self.t = other.t;
        self.r = other.r;
        self.w = other.w;
        self.b.copy_from(&other.b);
        self.n = other.n;
    }
}

// ---------------------------------------------------------------------------
// Parallel move buffer
// ---------------------------------------------------------------------------

struct PMEntry {
    src: Ref,
    dst: Ref,
    cls: Cls,
}

/// Parallel move state.
struct PMState {
    entries: Vec<PMEntry>,
}

impl PMState {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    /// Add a parallel move entry.
    fn add(&mut self, src: Ref, dst: Ref, cls: Cls) {
        assert!(self.entries.len() < RMAP_MAX, "too many parallel moves");
        self.entries.push(PMEntry { src, dst, cls });
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum PMStat {
    ToMove,
    Moving,
    Moved,
}

// ---------------------------------------------------------------------------
// Hint helpers
// ---------------------------------------------------------------------------

fn hint_reg(t: usize, tmps: &mut [crate::ir::Tmp]) -> i32 {
    let pc = phicls(t, tmps);
    tmps[pc].hint.r
}

fn sethint(t: usize, r: i32, loop_depth: i32, tmps: &mut [crate::ir::Tmp]) {
    let pc = phicls(t, tmps);
    let p = &mut tmps[pc];
    if p.hint.r == -1 || p.hint.w > loop_depth {
        p.hint.r = r;
        p.hint.w = loop_depth;
        tmps[t].visit = -1;
    }
}

// ---------------------------------------------------------------------------
// RMap operations
// ---------------------------------------------------------------------------

/// Find the register assigned to temporary `t`, or -1 if not mapped.
fn rfind(m: &RMap, t: i32) -> i32 {
    for i in 0..m.n {
        if m.t[i] == t {
            return m.r[i];
        }
    }
    -1
}

/// Get a Ref for temporary `t`: either its assigned register or its spill slot.
fn rref(m: &RMap, t: i32, tmps: &[crate::ir::Tmp]) -> Ref {
    let r = rfind(m, t);
    if r == -1 {
        let s = tmps[t as usize].slot;
        assert!(s != -1, "should have spilled");
        Ref::Slot(s as u32)
    } else {
        Ref::Tmp(TmpId(r as u32))
    }
}

/// Add a mapping: temporary `t` → register `r`.
fn radd(m: &mut RMap, t: i32, r: i32, target: &Target, regu: &mut u64) {
    assert!(t >= TMP0 as i32 || t == r, "invalid temporary");
    assert!(
        (target.gpr0 <= r && r < target.gpr0 + target.ngpr)
            || (target.fpr0 <= r && r < target.fpr0 + target.nfpr),
        "invalid register {}",
        r
    );
    assert!(!m.b.has(t as u32), "temporary {} has mapping", t);
    assert!(!m.b.has(r as u32), "register {} already allocated", r);
    assert!(
        m.n <= (target.ngpr + target.nfpr) as usize,
        "too many mappings"
    );
    m.b.set(t as u32);
    m.b.set(r as u32);
    m.t[m.n] = t;
    m.r[m.n] = r;
    m.n += 1;
    *regu |= 1u64 << r;
}

/// Try to allocate a register for temporary `t`.
/// If `try_only` is true and no register is available, returns Ref::R instead of panicking.
fn ralloctry(
    m: &mut RMap,
    t: i32,
    try_only: bool,
    target: &Target,
    regu: &mut u64,
    tmps: &mut [crate::ir::Tmp],
) -> Ref {
    if t < TMP0 as i32 {
        // Physical register: must already be tracked.
        assert!(m.b.has(t as u32));
        return Ref::Tmp(TmpId(t as u32));
    }
    if m.b.has(t as u32) {
        let r = rfind(m, t);
        assert!(r != -1);
        return Ref::Tmp(TmpId(r as u32));
    }

    // Try visit hint, then phi-class hint.
    let mut r = tmps[t as usize].visit;
    if r == -1 || m.b.has(r as u32) {
        r = hint_reg(t as usize, tmps);
    }
    if r == -1 || m.b.has(r as u32) {
        if try_only {
            return Ref::R;
        }
        // Find a free register, preferring ones not in the hint mask.
        let pc = phicls(t as usize, tmps);
        let hint_mask = tmps[pc].hint.m;
        let used = m.b.bits_raw().first().copied().unwrap_or(0);
        let regs = hint_mask | used;

        let (r0, r1) = if tmps[t as usize].cls.base() == 0 {
            (target.gpr0, target.gpr0 + target.ngpr)
        } else {
            (target.fpr0, target.fpr0 + target.nfpr)
        };

        // First pass: avoid both used and hinted-against registers.
        let mut found = -1;
        for ri in r0..r1 {
            if regs & (1u64 << ri) == 0 {
                found = ri;
                break;
            }
        }
        if found == -1 {
            // Second pass: just avoid used registers.
            for ri in r0..r1 {
                if !m.b.has(ri as u32) {
                    found = ri;
                    break;
                }
            }
        }
        assert!(found != -1, "no more regs");
        r = found;
    }

    radd(m, t, r, target, regu);
    sethint(t as usize, r, i32::MAX, tmps);
    tmps[t as usize].visit = r;

    let h = hint_reg(t as usize, tmps);
    if h != -1 && h != r && (r as usize) < RMAP_MAX {
        m.w[r as usize] = t;
    }
    Ref::Tmp(TmpId(r as u32))
}

#[inline]
fn ralloc(
    m: &mut RMap,
    t: i32,
    target: &Target,
    regu: &mut u64,
    tmps: &mut [crate::ir::Tmp],
) -> Ref {
    ralloctry(m, t, false, target, regu, tmps)
}

/// Free the mapping for temporary `t`. Returns the register it was using, or -1.
fn rfree(m: &mut RMap, t: i32, target: &Target) -> i32 {
    assert!(
        t >= TMP0 as i32 || (1u64 << t) & target.rglob == 0,
        "cannot free global register"
    );
    if !m.b.has(t as u32) {
        return -1;
    }
    // Find the index.
    let mut idx = 0;
    while m.t[idx] != t {
        idx += 1;
        assert!(idx < m.n);
    }
    let r = m.r[idx];
    m.b.clr(t as u32);
    m.b.clr(r as u32);
    m.n -= 1;
    // Shift remaining entries down.
    for j in idx..m.n {
        m.t[j] = m.t[j + 1];
        m.r[j] = m.r[j + 1];
    }
    assert!(t >= TMP0 as i32 || t == r);
    r
}

// ---------------------------------------------------------------------------
// Parallel move resolution
// ---------------------------------------------------------------------------

/// Resolve one entry in the parallel move, handling cycles with swaps.
fn pmrec(pm: &[PMEntry], status: &mut [PMStat], i: usize, k: &mut Cls, buf: &mut InsBuffer) -> i32 {
    if pm[i].src == pm[i].dst {
        status[i] = PMStat::Moved;
        return -1;
    }

    assert!(pm[i].cls.base() == k.base());
    // Merge class: (Kw|Kl)==Kl, (Ks|Kd)==Kd
    let merged = (pm[i].cls as i8) | (*k as i8);
    *k = Cls::from_i8(merged);

    // Find if any other entry's dst matches our src.
    let npm = pm.len();
    let mut j = 0;
    while j < npm {
        if pm[j].dst == pm[i].src {
            break;
        }
        j += 1;
    }

    let st = if j == npm { PMStat::Moved } else { status[j] };

    let c;
    match st {
        PMStat::Moving => {
            // Cycle detected — emit swap.
            c = j as i32;
            buf.emit(Op::Swap, *k, Ref::R, pm[i].src, pm[i].dst);
        }
        PMStat::ToMove => {
            status[i] = PMStat::Moving;
            let ci = pmrec(pm, status, j, k, buf);
            if ci == i as i32 {
                // End of cycle.
                c = -1;
            } else if ci != -1 {
                // Still in cycle — emit swap.
                c = ci;
                buf.emit(Op::Swap, *k, Ref::R, pm[i].src, pm[i].dst);
            } else {
                // No cycle — emit copy.
                c = -1;
                buf.emit(Op::Copy, pm[i].cls, pm[i].dst, pm[i].src, Ref::R);
            }
        }
        PMStat::Moved => {
            c = -1;
            buf.emit(Op::Copy, pm[i].cls, pm[i].dst, pm[i].src, Ref::R);
        }
    }
    status[i] = PMStat::Moved;
    c
}

/// Generate instructions for all entries in the parallel move buffer.
fn pmgen(pm: &PMState, buf: &mut InsBuffer) {
    let npm = pm.entries.len();
    if npm == 0 {
        return;
    }
    let mut status = vec![PMStat::ToMove; npm];
    for i in 0..npm {
        if status[i] == PMStat::ToMove {
            let mut k = pm.entries[i].cls;
            pmrec(&pm.entries, &mut status, i, &mut k, buf);
        }
    }
}

// ---------------------------------------------------------------------------
// move — handle register moves during allocation
// ---------------------------------------------------------------------------

fn do_move(
    r: i32,
    to: Ref,
    m: &mut RMap,
    target: &Target,
    regu: &mut u64,
    tmps: &mut [crate::ir::Tmp],
) {
    let r1 = if to == Ref::R {
        -1
    } else {
        rfree(m, to.val() as i32, target)
    };

    if m.b.has(r as u32) {
        // r is used and not by `to`.
        assert!(r1 != r);
        let mut n = 0;
        while m.r[n] != r {
            n += 1;
            assert!(n < m.n);
        }
        let t = m.t[n];
        rfree(m, t, target);
        m.b.set(r as u32);
        ralloc(m, t, target, regu, tmps);
        m.b.clr(r as u32);
    }

    let t = if to == Ref::R { r } else { to.val() as i32 };
    radd(m, t, r, target, regu);
}

fn regcpy(ins: &Ins) -> bool {
    ins.op == Op::Copy && crate::ir::isreg(ins.arg[0])
}

// ---------------------------------------------------------------------------
// dopm — process parallel moves in rega
// ---------------------------------------------------------------------------

/// Process a block of consecutive register copy instructions.
///
/// Port of C QBE's `dopm()` in rega.c.
fn rega_dopm(
    f: &mut Fn,
    b_idx: usize,
    i_end: usize,
    m: &mut RMap,
    target: &Target,
    regu: &mut u64,
    pm: &mut PMState,
    buf: &mut InsBuffer,
    _stmov: &mut u32,
) -> usize {
    // Save current mapping state.
    let m0_t = m.t;
    let m0_r = m.r;
    let m0_n = m.n;

    // Find the start of the PM block.
    let mut start = i_end;
    while start > 0 && regcpy(&f.blks[b_idx].ins[start - 1]) {
        start -= 1;
    }

    // Process from i_end down to start, applying moves.
    let mut i = i_end;
    loop {
        let ins = f.blks[b_idx].ins[i];
        do_move(
            ins.arg[0].val() as i32,
            ins.to,
            m,
            target,
            regu,
            &mut f.tmps,
        );
        if i == start {
            break;
        }
        i -= 1;
    }

    assert!(m0_n <= m.n);

    // Handle call instruction preceding the PM block.
    if start > 0 && f.blks[b_idx].ins[start - 1].op == Op::Call {
        let call_ref = f.blks[b_idx].ins[start - 1].arg[1];
        let def = target.retregs(call_ref, None) | target.rglob;
        for &rs in target.rsave {
            if rs < 0 {
                break;
            }
            if (1u64 << rs) & def == 0 {
                do_move(rs, Ref::R, m, target, regu, &mut f.tmps);
            }
        }
    }

    // Build parallel move entries.
    pm.clear();
    for n in 0..m.n {
        let t = m.t[n];
        let s = f.tmps[t as usize].slot;
        let r1 = m.r[n];
        // Find what register t had in the old mapping.
        let mut r_old = -1;
        for j in 0..m0_n {
            if m0_t[j] == t {
                r_old = m0_r[j];
                break;
            }
        }
        if r_old != -1 {
            pm.add(
                Ref::Tmp(TmpId(r1 as u32)),
                Ref::Tmp(TmpId(r_old as u32)),
                f.tmps[t as usize].cls,
            );
        } else if s != -1 {
            pm.add(
                Ref::Tmp(TmpId(r1 as u32)),
                Ref::Slot(s as u32),
                f.tmps[t as usize].cls,
            );
        }
    }

    // Re-add physical register self-mappings.
    for ip in start..=i_end {
        let ins = f.blks[b_idx].ins[ip];
        if ins.to != Ref::R {
            rfree(m, ins.to.val() as i32, target);
        }
        let r = ins.arg[0].val() as i32;
        if rfind(m, r) == -1 {
            radd(m, r, r, target, regu);
        }
    }

    pmgen(pm, buf);
    start
}

// ---------------------------------------------------------------------------
// Priority / insertion helpers for doblk
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn prio1(r1: Ref, _r2: Ref, tmps: &mut [crate::ir::Tmp]) -> bool {
    if let Ref::Tmp(tid) = r1 {
        hint_reg(tid.0 as usize, tmps) != -1
    } else {
        false
    }
}

#[allow(dead_code)]
fn insert_sorted(ra: &mut Vec<*mut Ref>, r: *mut Ref, tmps: &mut [crate::ir::Tmp]) {
    let p = ra.len();
    ra.push(r);
    let mut i = p;
    while i > 0 {
        // SAFETY: We only use these pointers within this function scope
        // to reorder references. They point to valid Ins fields.
        let cur = unsafe { *r };
        let prev = unsafe { *ra[i - 1] };
        if prio1(cur, prev, tmps) {
            ra[i] = ra[i - 1];
            ra[i - 1] = r;
            i -= 1;
        } else {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// doblk — process one block bottom-to-top
// ---------------------------------------------------------------------------

fn doblk(
    f: &mut Fn,
    b_idx: usize,
    cur: &mut RMap,
    target: &Target,
    regu: &mut u64,
    pm: &mut PMState,
    stmov: &mut u32,
) {
    // Handle jump argument.
    if let Ref::Tmp(tid) = f.blks[b_idx].jmp.arg {
        f.blks[b_idx].jmp.arg = ralloc(cur, tid.0 as i32, target, regu, &mut f.tmps);
    }

    let mut buf = InsBuffer::new();
    let nins = f.blks[b_idx].ins.len();
    let mut i1 = nins;

    while i1 > 0 {
        i1 -= 1;
        let ins = f.blks[b_idx].ins[i1];
        buf.emiti(ins);

        let mut rf: i32 = -1;

        match ins.op {
            Op::Call => {
                let call_ref = f.blks[b_idx].ins[i1].arg[1];
                let rs = target.argregs(call_ref, None) | target.rglob;
                for &rsave in target.rsave {
                    if rsave < 0 {
                        break;
                    }
                    if (1u64 << rsave) & rs == 0 {
                        rfree(cur, rsave, target);
                    }
                }
            }
            Op::Copy if regcpy(&f.blks[b_idx].ins[i1]) => {
                // Remove the emitted copy — dopm will handle it.
                // InsBuffer stores in reverse, so the last push is the copy we just emitted.
                buf = {
                    let mut new_buf = InsBuffer::new();
                    let sl = buf.as_slice();
                    // Skip the last instruction (the regcpy we just pushed).
                    for idx in 0..sl.len() - 1 {
                        new_buf.emiti(sl[idx]);
                    }
                    new_buf
                };
                let old_len = buf.len();
                i1 = rega_dopm(f, b_idx, i1, cur, target, regu, pm, &mut buf, stmov);
                *stmov += (buf.len() - old_len) as u32;
                continue;
            }
            _ => {
                // Handle copy hint.
                if ins.op == Op::Copy
                    && crate::ir::isreg(ins.to)
                    && matches!(ins.arg[0], Ref::Tmp(tid) if tid.0 >= TMP0)
                {
                    if let Ref::Tmp(to_tid) = ins.to {
                        sethint(
                            ins.arg[0].val() as usize,
                            to_tid.0 as i32,
                            i32::MAX,
                            &mut f.tmps,
                        );
                    }
                }

                if ins.to != Ref::R {
                    let r = ins.to.val() as i32;
                    if r < TMP0 as i32 && (1u64 << r) & target.rglob != 0 {
                        // Global register — don't free.
                    } else {
                        rf = rfree(cur, r, target);
                        if rf == -1 {
                            assert!(!crate::ir::isreg(ins.to));
                            // Dead definition — remove the emitted instruction.
                            buf = {
                                let mut new_buf = InsBuffer::new();
                                let sl = buf.as_slice();
                                for idx in 0..sl.len() - 1 {
                                    new_buf.emiti(sl[idx]);
                                }
                                new_buf
                            };
                            continue;
                        }
                        // Patch the `to` field.
                        {
                            let last_idx = buf.len() - 1;
                            // We need to modify the last emitted instruction's `to`.
                            // InsBuffer stores in emission order (most recent = last).
                            let sl = buf.as_slice();
                            let mut patched = sl[last_idx];
                            patched.to = Ref::Tmp(TmpId(rf as u32));
                            // Rebuild buf with patched instruction.
                            let mut new_buf = InsBuffer::new();
                            for idx in 0..last_idx {
                                new_buf.emiti(sl[idx]);
                            }
                            new_buf.emiti(patched);
                            buf = new_buf;
                        }
                    }
                }
            }
        }

        // Allocate registers for arguments.
        // Collect argument references that need allocation.
        let cur_ins = *buf.as_slice().last().unwrap();
        let mut arg_refs: Vec<(usize, usize, i32)> = Vec::new(); // (arg_idx, mem_field, tmp_val)

        for x in 0..2 {
            match cur_ins.arg[x] {
                Ref::Mem(mid) => {
                    let m = &f.mems[mid.0 as usize];
                    if let Ref::Tmp(tid) = m.base {
                        arg_refs.push((x, 0, tid.0 as i32)); // 0 = base
                    }
                    if let Ref::Tmp(tid) = m.index {
                        arg_refs.push((x, 1, tid.0 as i32)); // 1 = index
                    }
                }
                Ref::Tmp(tid) => {
                    arg_refs.push((x, 2, tid.0 as i32)); // 2 = direct arg
                }
                _ => {}
            }
        }

        // Sort by priority (hinted first).
        arg_refs.sort_by(|a, b| {
            let ha = hint_reg(a.2 as usize, &mut f.tmps) != -1;
            let hb = hint_reg(b.2 as usize, &mut f.tmps) != -1;
            hb.cmp(&ha)
        });

        // Allocate and patch.
        for &(arg_idx, mem_field, tmp_val) in &arg_refs {
            let allocated = ralloc(cur, tmp_val, target, regu, &mut f.tmps);

            // Patch the instruction in the buffer.
            let last_idx = buf.len() - 1;
            let sl = buf.as_slice();
            let mut patched = sl[last_idx];

            match mem_field {
                0 | 1 => {
                    // Memory field — patch in f.mems.
                    if let Ref::Mem(mid) = patched.arg[arg_idx] {
                        if mem_field == 0 {
                            f.mems[mid.0 as usize].base = allocated;
                        } else {
                            f.mems[mid.0 as usize].index = allocated;
                        }
                    }
                }
                _ => {
                    // Direct argument.
                    patched.arg[arg_idx] = allocated;
                }
            }

            // Rebuild buffer with patched instruction.
            let mut new_buf = InsBuffer::new();
            for idx in 0..last_idx {
                new_buf.emiti(sl[idx]);
            }
            new_buf.emiti(patched);
            buf = new_buf;
        }

        // Eliminate identity copies.
        {
            let last = buf.as_slice().last().unwrap();
            if last.op == Op::Copy && last.to == last.arg[0] {
                buf = {
                    let mut new_buf = InsBuffer::new();
                    let sl = buf.as_slice();
                    for idx in 0..sl.len() - 1 {
                        new_buf.emiti(sl[idx]);
                    }
                    new_buf
                };
            }
        }

        // Try to satisfy register hints by moving.
        if rf != -1 {
            let rf_u = rf as usize;
            if rf_u < RMAP_MAX {
                let wt = cur.w[rf_u];
                if wt != 0 && !cur.b.has(rf as u32) && hint_reg(wt as usize, &mut f.tmps) == rf {
                    let rt = rfree(cur, wt, target);
                    if rt != -1 {
                        f.tmps[wt as usize].visit = -1;
                        ralloc(cur, wt, target, regu, &mut f.tmps);
                        assert!(cur.b.has(rf as u32));
                        buf.emit(
                            Op::Copy,
                            f.tmps[wt as usize].cls,
                            Ref::Tmp(TmpId(rt as u32)),
                            Ref::Tmp(TmpId(rf as u32)),
                            Ref::R,
                        );
                        *stmov += 1;
                        cur.w[rf_u] = 0;

                        // Patch any arguments that used rt to use rf instead.
                        // This mirrors the C code's ra[] loop.
                        let last_idx = buf.len() - 1;
                        // The instruction being processed is at last_idx - 1 (before the copy we just emitted).
                        if last_idx > 0 {
                            let sl = buf.as_slice();
                            let mut patched = sl[last_idx - 1];
                            for a in 0..2 {
                                if patched.arg[a] == Ref::Tmp(TmpId(rt as u32)) {
                                    patched.arg[a] = Ref::Tmp(TmpId(rf as u32));
                                }
                            }
                            let mut new_buf = InsBuffer::new();
                            for idx in 0..last_idx - 1 {
                                new_buf.emiti(sl[idx]);
                            }
                            new_buf.emiti(patched);
                            new_buf.emiti(sl[last_idx]); // the copy
                            buf = new_buf;
                        }
                    }
                }
            }
        }
    }

    // Replace block instructions with buffered version.
    f.blks[b_idx].ins = buf.finish();
}

// ---------------------------------------------------------------------------
// carve — block ordering by loop nesting
// ---------------------------------------------------------------------------

fn carve_order(f: &Fn) -> Vec<u32> {
    let mut blk_ids: Vec<u32> = f.rpo.iter().map(|b| b.0).collect();
    blk_ids.sort_by(|&a, &b| {
        let la = f.blks[a as usize].loop_depth;
        let lb = f.blks[b as usize].loop_depth;
        if la == lb {
            // Higher RPO id first (deeper blocks first within same loop level).
            let ia = f.blks[a as usize].id;
            let ib = f.blks[b as usize].id;
            ib.cmp(&ia)
        } else {
            // Higher loop depth first.
            lb.cmp(&la)
        }
    });
    blk_ids
}

// ---------------------------------------------------------------------------
// prio2 — priority for end-of-block allocation ordering
// ---------------------------------------------------------------------------

fn prio2(t1: i32, t2: i32, tmps: &mut [crate::ir::Tmp]) -> std::cmp::Ordering {
    let v1 = tmps[t1 as usize].visit;
    let v2 = tmps[t2 as usize].visit;
    // Different signs: prefer the one with visit != -1.
    if (v1 ^ v2) < 0 {
        return if v1 != -1 {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Less
        };
    }
    let h1 = hint_reg(t1 as usize, tmps);
    let h2 = hint_reg(t2 as usize, tmps);
    if (h1 ^ h2) < 0 {
        return if h1 != -1 {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Less
        };
    }
    tmps[t1 as usize].cost.cmp(&tmps[t2 as usize].cost)
}

// ---------------------------------------------------------------------------
// rega — main register allocation entry point
// ---------------------------------------------------------------------------

/// Register allocation pass.
///
/// Port of C QBE's `rega(Fn *fn)`.
///
/// Depends on: rpo, phi, cost, spill.
pub fn rega(f: &mut Fn, target: &Target) {
    let ntmp = f.tmps.len() as u32;
    let nblk = f.rpo.len();
    let mut regu: u64 = 0;
    let mut _stmov: u32 = 0;
    let mut pm = PMState::new();

    // 1. Setup
    let mut end: Vec<RMap> = (0..nblk).map(|_| RMap::new(ntmp)).collect();
    let mut beg: Vec<RMap> = (0..nblk).map(|_| RMap::new(ntmp)).collect();
    let mut cur = RMap::new(ntmp);

    // Initialize hints: physical registers hint to themselves.
    for t in 0..f.tmps.len() {
        f.tmps[t].hint.r = if t < TMP0 as usize { t as i32 } else { -1 };
        f.tmps[t].hint.w = i32::MAX;
        f.tmps[t].visit = -1;
    }

    // Set hints from initial copy instructions in the start block.
    let start_idx = f.start.0 as usize;
    for ii in 0..f.blks[start_idx].ins.len() {
        let ins = f.blks[start_idx].ins[ii];
        if ins.op != Op::Copy || !crate::ir::isreg(ins.arg[0]) {
            break;
        }
        if let Ref::Tmp(tid) = ins.to {
            sethint(
                tid.0 as usize,
                ins.arg[0].val() as i32,
                i32::MAX,
                &mut f.tmps,
            );
        }
    }

    // Get block ordering by loop nesting.
    let blk_order = carve_order(f);

    // 2. Assign registers — process blocks in carved order.
    for &bid in &blk_order {
        let b_idx = bid as usize;
        let n = f.blks[b_idx].id as usize;
        let _loop_depth = f.blks[b_idx].loop_depth;

        // Reset current mapping.
        cur.n = 0;
        cur.b.zero();
        cur.w = [0; RMAP_MAX];

        // Collect temporaries live at block exit, sorted by priority.
        let mut rl: Vec<i32> = Vec::new();
        for t in f.blks[b_idx].out.iter() {
            if t >= TMP0 {
                let tv = t as i32;
                // Insertion sort by prio2.
                let mut j = rl.len();
                rl.push(tv);
                while j > 0 && prio2(tv, rl[j - 1], &mut f.tmps) == std::cmp::Ordering::Greater {
                    rl[j] = rl[j - 1];
                    rl[j - 1] = tv;
                    j -= 1;
                }
            }
        }

        // Add physical registers that are live out.
        for r in f.blks[b_idx].out.iter() {
            if r < TMP0 {
                radd(&mut cur, r as i32, r as i32, target, &mut regu);
            }
        }

        // Allocate registers for live-out temporaries (try first, then force).
        for &t in &rl {
            ralloctry(&mut cur, t, true, target, &mut regu, &mut f.tmps);
        }
        for &t in &rl {
            ralloc(&mut cur, t, target, &mut regu, &mut f.tmps);
        }

        end[n].copy_from(&cur);

        // Process the block's instructions.
        doblk(f, b_idx, &mut cur, target, &mut regu, &mut pm, &mut _stmov);

        // Update in set and remove phi destinations.
        f.blks[b_idx].r#in.copy_from(&cur.b);
        for pi in 0..f.blks[b_idx].phi.len() {
            if let Ref::Tmp(tid) = f.blks[b_idx].phi[pi].to {
                f.blks[b_idx].r#in.clr(tid.0);
            }
        }

        beg[n].copy_from(&cur);
    }

    // 3. Emit copies shared by multiple edges to the same block.
    let rpo = f.rpo.clone();
    for &sid in &rpo {
        let s_idx = sid.0 as usize;
        let npred = f.blks[s_idx].pred.len();
        if npred <= 1 {
            continue;
        }
        let s_id = f.blks[s_idx].id as usize;
        let m = &beg[s_id];

        // rl maps a register live at the beginning of s to the one used in
        // all predecessors (if any, -1 otherwise).
        let mut rl_map = vec![0i32; RMAP_MAX];

        // Process phi nodes.
        let nphi = f.blks[s_idx].phi.len();
        for pi in 0..nphi {
            let phi_to = f.blks[s_idx].phi[pi].to;
            if let Ref::Tmp(tid) = phi_to {
                let r = rfind(m, tid.0 as i32);
                if r == -1 {
                    continue;
                }
                let narg = f.blks[s_idx].phi[pi].narg();
                for u in 0..narg {
                    let blk = f.blks[s_idx].phi[pi].blks[u];
                    let src = f.blks[s_idx].phi[pi].args[u];
                    if let Ref::Tmp(src_tid) = src {
                        let b_id = f.blks[blk.0 as usize].id as usize;
                        let x = rfind(&end[b_id], src_tid.0 as i32);
                        if x == -1 {
                            continue; // spilled
                        }
                        let cur_val = rl_map[r as usize];
                        rl_map[r as usize] = if cur_val == 0 || cur_val == x { x } else { -1 };
                    }
                }
                if rl_map[r as usize] == 0 {
                    rl_map[r as usize] = -1;
                }
            }
        }

        // Process non-phi temporaries.
        for j in 0..m.n {
            let t = m.t[j];
            let r = m.r[j];
            if rl_map[r as usize] != 0 || t < TMP0 as i32 {
                continue;
            }
            let preds = f.blks[s_idx].pred.clone();
            for &pred in &preds {
                let p_id = f.blks[pred.0 as usize].id as usize;
                let x = rfind(&end[p_id], t);
                if x == -1 {
                    continue; // spilled
                }
                let cur_val = rl_map[r as usize];
                rl_map[r as usize] = if cur_val == 0 || cur_val == x { x } else { -1 };
            }
            if rl_map[r as usize] == 0 {
                rl_map[r as usize] = -1;
            }
        }

        // Generate parallel move for shared copies.
        pm.clear();
        // We need a mutable reference to beg[s_id].b, but we also read beg[s_id].
        // Clone the needed data first.
        let m_n = beg[s_id].n;
        let m_t = beg[s_id].t;
        let m_r = beg[s_id].r;

        for j in 0..m_n {
            let t = m_t[j];
            let r = m_r[j];
            let x = rl_map[r as usize];
            assert!(x != 0 || t < TMP0 as i32);
            if x > 0 && !beg[s_id].b.has(x as u32) {
                pm.add(
                    Ref::Tmp(TmpId(x as u32)),
                    Ref::Tmp(TmpId(r as u32)),
                    f.tmps[t as usize].cls,
                );
                beg[s_id].r[j] = x;
                beg[s_id].b.set(x as u32);
            }
        }

        let mut buf = InsBuffer::new();
        pmgen(&pm, &mut buf);
        let new_ins = buf.finish();
        let j = new_ins.len();
        if j == 0 {
            continue;
        }
        // Prepend the generated copies to the block's instructions.
        let old_ins = f.blks[s_idx].ins.clone();
        let mut combined = new_ins;
        combined.extend(old_ins);
        f.blks[s_idx].ins = combined;
    }

    // 4. Emit remaining copies in new blocks (bridge blocks for phi resolution).
    let mut blist: Vec<usize> = Vec::new(); // indices of new blocks

    // We need to iterate over all blocks and their successors.
    // Collect (block_idx, successor_slot, successor_idx) triples first.
    let all_blks: Vec<u32> = {
        let mut v = Vec::new();
        // Walk the linked block list (all blocks reachable from start).
        for bid in &f.rpo {
            v.push(bid.0);
        }
        // Also include any blocks added during phase 3 (shouldn't be any yet).
        v
    };

    for &bid in &all_blks {
        let b_idx = bid as usize;
        let successors = [f.blks[b_idx].s1, f.blks[b_idx].s2];

        for (slot, s_opt) in successors.iter().enumerate() {
            let s_bid = match s_opt {
                Some(s) => *s,
                None => continue,
            };
            let s_idx = s_bid.0 as usize;
            let s_rpo = f.blks[s_idx].id as usize;
            let b_rpo = f.blks[b_idx].id as usize;

            pm.clear();

            // Process phis.
            let nphi = f.blks[s_idx].phi.len();
            for pi in 0..nphi {
                let dst = f.blks[s_idx].phi[pi].to;
                match dst {
                    Ref::Slot(_) | Ref::Tmp(_) => {}
                    _ => continue,
                }
                let dst_resolved = if let Ref::Tmp(tid) = dst {
                    let r = rfind(&beg[s_rpo], tid.0 as i32);
                    if r == -1 {
                        continue;
                    }
                    Ref::Tmp(TmpId(r as u32))
                } else {
                    dst
                };

                // Find the argument corresponding to block b.
                let narg = f.blks[s_idx].phi[pi].narg();
                let mut found_u = None;
                for u in 0..narg {
                    if f.blks[s_idx].phi[pi].blks[u].0 == bid {
                        found_u = Some(u);
                        break;
                    }
                }
                let u = match found_u {
                    Some(u) => u,
                    None => {
                        // This predecessor isn't in this phi.
                        continue;
                    }
                };

                let src = f.blks[s_idx].phi[pi].args[u];
                let src_resolved = if let Ref::Tmp(tid) = src {
                    rref(&end[b_rpo], tid.0 as i32, &f.tmps)
                } else {
                    src
                };
                let cls = f.blks[s_idx].phi[pi].cls;
                pm.add(src_resolved, dst_resolved, cls);
            }

            // Process non-phi temporaries live into s.
            for t in f.blks[s_idx].r#in.iter() {
                if t >= TMP0 {
                    let src = rref(&end[b_rpo], t as i32, &f.tmps);
                    let dst = rref(&beg[s_rpo], t as i32, &f.tmps);
                    pm.add(src, dst, f.tmps[t as usize].cls);
                }
            }

            let mut buf = InsBuffer::new();
            pmgen(&pm, &mut buf);
            let new_ins = buf.finish();
            if new_ins.is_empty() {
                continue;
            }

            // Create a bridge block.
            let new_bid = f.blks.len();
            let mut b1 = crate::ir::Blk::default();
            b1.loop_depth = (f.blks[b_idx].loop_depth + f.blks[s_idx].loop_depth) / 2;
            b1.name = format!("{}_{}", f.blks[b_idx].name, f.blks[s_idx].name);
            b1.ins = new_ins;
            b1.jmp.typ = Jmp::Jmp_;
            b1.s1 = Some(s_bid);
            f.blks.push(b1);
            blist.push(new_bid);

            // Redirect the edge.
            let new_blk_id = crate::ir::BlkId(new_bid as u32);
            if slot == 0 {
                f.blks[b_idx].s1 = Some(new_blk_id);
            } else {
                f.blks[b_idx].s2 = Some(new_blk_id);
            }
        }
    }

    // Clear all phi nodes — they've been resolved.
    for bid in &f.rpo {
        f.blks[bid.0 as usize].phi.clear();
    }
    for &new_idx in &blist {
        // New blocks don't have phis, but clear for safety.
        f.blks[new_idx].phi.clear();
    }

    f.reg = regu;
}
