use crate::cfg;
use crate::ir::*;
use crate::util::*;

// ---------------------------------------------------------------------------
// Lattice values for SCCP
// ---------------------------------------------------------------------------

/// Lattice value for a temporary.
///   - `Top`: not yet determined (matches UNDEF)
///   - `Bot`: known to be non-constant
///   - `Con(u32)`: known constant, value is a `ConId` index
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum LatVal {
    Top,
    Bot,
    Con(u32),
}

// ---------------------------------------------------------------------------
// Edge in the flow graph
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Edge {
    dest: i32, // RPO index of destination block, or -1
    dead: bool,
}

// ---------------------------------------------------------------------------
// Helper: check if a constant matches a specific integer value
// ---------------------------------------------------------------------------

fn iscon(c: &Con, w: bool, k: u64) -> bool {
    if c.typ != ConType::Bits {
        return false;
    }
    if w {
        c.bits.i() as u64 == k
    } else {
        (c.bits.i() as u32) == (k as u32)
    }
}

// ---------------------------------------------------------------------------
// Lattice operations
// ---------------------------------------------------------------------------

fn latval(r: Ref, val: &[LatVal]) -> LatVal {
    match r {
        Ref::Tmp(t) => val[t.0 as usize],
        Ref::Con(c) => LatVal::Con(c.0),
        _ => panic!("latval: unexpected ref {:?}", r),
    }
}

fn latmerge(v: LatVal, m: LatVal) -> LatVal {
    if m == LatVal::Top {
        v
    } else if v == LatVal::Top || v == m {
        m
    } else {
        LatVal::Bot
    }
}

// ---------------------------------------------------------------------------
// Update a temporary's lattice value, enqueuing uses if changed
// ---------------------------------------------------------------------------

fn update(t: u32, m: LatVal, val: &mut [LatVal], usewrk: &mut Vec<Use>, f: &Fn) {
    let m = latmerge(val[t as usize], m);
    if m != val[t as usize] {
        let tmp = &f.tmps[t as usize];
        for u in &tmp.uses {
            usewrk.push(*u);
        }
        val[t as usize] = m;
    }
}

// ---------------------------------------------------------------------------
// Check if all edges from block s to block d are dead
// ---------------------------------------------------------------------------

fn deadedge(s: usize, d: usize, edge: &[[Edge; 2]]) -> bool {
    let e = &edge[s];
    if e[0].dest == d as i32 && !e[0].dead {
        return false;
    }
    if e[1].dest == d as i32 && !e[1].dead {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Visit a phi node: merge lattice values from live incoming edges
// ---------------------------------------------------------------------------

fn visitphi(
    p: &Phi,
    n: usize,
    val: &mut [LatVal],
    usewrk: &mut Vec<Use>,
    edge: &[[Edge; 2]],
    f: &Fn,
) {
    let mut v = LatVal::Top;
    for a in 0..p.narg() {
        if !deadedge(p.blks[a].0 as usize, n, edge) {
            v = latmerge(v, latval(p.args[a], val));
        }
    }
    if let Ref::Tmp(t) = p.to {
        update(t.0, v, val, usewrk, f);
    }
}

// ---------------------------------------------------------------------------
// Visit an instruction: fold if possible, otherwise mark as Bot
// ---------------------------------------------------------------------------

fn visitins(i: &Ins, val: &mut [LatVal], usewrk: &mut Vec<Use>, f: &mut Fn) {
    if !i.to.is_tmp() {
        return;
    }
    let v;
    if i.op.can_fold() {
        let l = latval(i.arg[0], val);
        let r = if i.arg[1].is_none() {
            // CON_Z is ConId(1)
            LatVal::Con(1)
        } else {
            latval(i.arg[1], val)
        };
        if l == LatVal::Bot || r == LatVal::Bot {
            v = LatVal::Bot;
        } else if l == LatVal::Top || r == LatVal::Top {
            v = LatVal::Top;
        } else {
            let lc = match l {
                LatVal::Con(c) => c,
                _ => unreachable!(),
            };
            let rc = match r {
                LatVal::Con(c) => c,
                _ => unreachable!(),
            };
            v = opfold(i.op, i.cls, lc, rc, f);
        }
    } else {
        v = LatVal::Bot;
    }
    if let Ref::Tmp(t) = i.to {
        update(t.0, v, val, usewrk, f);
    }
}

// ---------------------------------------------------------------------------
// Visit a block's jump: enqueue successor edges as appropriate
// ---------------------------------------------------------------------------

fn visitjmp(
    b: &Blk,
    n: usize,
    val: &[LatVal],
    edge: &mut [[Edge; 2]],
    flowrk: &mut Vec<usize>,
    f: &Fn,
) {
    if b.jmp.typ.is_jnz() {
        let l = latval(b.jmp.arg, val);
        if l == LatVal::Bot {
            // Both edges are reachable.
            flowrk.push(n * 2 + 1);
            flowrk.push(n * 2);
        } else if let LatVal::Con(c) = l {
            if iscon(&f.cons[c as usize], false, 0) {
                // Condition is false: only false-branch (s2 = edge[n][1]) is reachable.
                debug_assert!(edge[n][0].dead);
                flowrk.push(n * 2 + 1);
            } else {
                // Condition is true: only true-branch (s1 = edge[n][0]) is reachable.
                debug_assert!(edge[n][1].dead);
                flowrk.push(n * 2);
            }
        }
        // Top: don't enqueue anything yet.
    } else if b.jmp.typ.is_jmp() {
        flowrk.push(n * 2);
    } else if b.jmp.typ == Jmp::Hlt || b.jmp.typ.is_ret() {
        // No successors.
    } else {
        panic!("visitjmp: unexpected jmp type {:?}", b.jmp.typ);
    }
}

// ---------------------------------------------------------------------------
// Initialize an edge
// ---------------------------------------------------------------------------

fn initedge(s: Option<BlkId>) -> Edge {
    Edge {
        dest: match s {
            Some(id) => id.0 as i32,
            None => -1,
        },
        dead: true,
    }
}

// ---------------------------------------------------------------------------
// Replace a Tmp ref with its constant if the lattice says it's constant.
// Returns true if replacement was made.
// ---------------------------------------------------------------------------

fn renref(r: &mut Ref, val: &[LatVal]) -> bool {
    if let Ref::Tmp(t) = *r {
        if let LatVal::Con(c) = val[t.0 as usize] {
            *r = Ref::Con(ConId(c));
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// The main SCCP pass
// ---------------------------------------------------------------------------

/// Sparse Conditional Constant Propagation pass.
/// Requires: RPO, use info, pred info.
pub fn fold(f: &mut Fn) {
    let ntmp = f.tmps.len();
    let nblk = f.rpo.len();

    // Initialize lattice: all temps start at Top.
    let mut val: Vec<LatVal> = vec![LatVal::Top; ntmp];

    // Initialize edges: two per block (s1, s2).
    let mut edge: Vec<[Edge; 2]> = Vec::with_capacity(nblk);
    for n in 0..nblk {
        let bid = f.rpo[n];
        let s1 = f.blks[bid.0 as usize].s1;
        let s2 = f.blks[bid.0 as usize].s2;
        f.blks[bid.0 as usize].visit = 0;
        edge.push([initedge(s1), initedge(s2)]);
    }

    // Start edge: points to the start block (RPO index 0).
    let mut flowrk: Vec<usize> = Vec::new();
    // The start block is rpo[0], which has RPO index 0.
    // Push a "start" edge -- we simulate it by making edge[0] get activated.
    // In C, `initedge(&start, fn->start)` creates an edge with dest = start->id.
    // start->id is the RPO index of the start block, which is 0.
    // We push a synthetic edge index; use a special encoding.
    // Actually, let's just handle the start edge inline: push a synthetic entry.
    let start_edge_idx = nblk * 2; // sentinel index for the start edge
    let start_edge = Edge {
        dest: 0, // RPO index 0 = start block
        dead: true,
    };
    // Instead of a complex sentinel, let's add the start edge to our edge vec
    // and handle it. Actually, simpler: just manually activate it.
    // The simplest approach: add start edge to the edge array as a virtual entry.
    edge.push([
        start_edge.clone(),
        Edge {
            dest: -1,
            dead: true,
        },
    ]);
    flowrk.push(start_edge_idx); // index into edge array, slot 0

    let mut usewrk: Vec<Use> = Vec::new();

    // -----------------------------------------------------------------------
    // Phase 1: Find constants and dead CFG edges
    // -----------------------------------------------------------------------
    loop {
        if let Some(eidx) = flowrk.pop() {
            let slot = eidx & 1;
            let block_idx = eidx >> 1;
            let e = &edge[block_idx][slot];
            if e.dest == -1 || !e.dead {
                continue;
            }
            let dest = e.dest as usize;
            edge[block_idx][slot].dead = false;

            let bid = f.rpo[dest];
            let n = dest;

            // Visit phis -- need to borrow carefully.
            let nphi = f.blks[bid.0 as usize].phi.len();
            for pi in 0..nphi {
                let p = f.blks[bid.0 as usize].phi[pi].clone();
                visitphi(&p, n, &mut val, &mut usewrk, &edge, f);
            }

            let visit = f.blks[bid.0 as usize].visit;
            if visit == 0 {
                let nins = f.blks[bid.0 as usize].ins.len();
                for ii in 0..nins {
                    let ins = f.blks[bid.0 as usize].ins[ii];
                    visitins(&ins, &mut val, &mut usewrk, f);
                }
                let b_copy = f.blks[bid.0 as usize].clone();
                visitjmp(&b_copy, n, &val, &mut edge, &mut flowrk, f);
            }
            f.blks[bid.0 as usize].visit += 1;
        } else if let Some(u) = usewrk.pop() {
            let n = u.bid as usize;
            // Find the RPO index for this block id.
            // u.bid is a block index (BlkId.0), and the block's id field
            // stores its RPO index.
            // Actually, in QBE, b->id IS the RPO number after fillrpo.
            // So we look up rpo[n] to get the BlkId, but u.bid is the
            // RPO index directly (since Tmp.bid = block's rpo index).
            // Wait -- Use.bid stores the block id as set by filluse, which
            // is the RPO number (b->id). So n = u.bid is the RPO index.
            if n >= nblk {
                continue;
            }
            let bid = f.rpo[n];
            if f.blks[bid.0 as usize].visit == 0 {
                continue;
            }
            match u.typ {
                UseType::Phi => {
                    if let UseDetail::PhiIdx(pidx) = u.detail {
                        let p = f.blks[bid.0 as usize].phi[pidx as usize].clone();
                        visitphi(&p, n, &mut val, &mut usewrk, &edge, f);
                    }
                }
                UseType::Ins => {
                    if let UseDetail::InsIdx(iidx) = u.detail {
                        let ins = f.blks[bid.0 as usize].ins[iidx as usize];
                        visitins(&ins, &mut val, &mut usewrk, f);
                    }
                }
                UseType::Jmp => {
                    let b_copy = f.blks[bid.0 as usize].clone();
                    visitjmp(&b_copy, n, &val, &mut edge, &mut flowrk, f);
                }
                _ => panic!("fold: unexpected use type {:?}", u.typ),
            }
        } else {
            break;
        }
    }

    // -----------------------------------------------------------------------
    // Phase 2: Trim dead code, replace constants
    // -----------------------------------------------------------------------

    // Collect the set of dead blocks (visit == 0).
    let mut dead_blks: Vec<bool> = vec![false; f.blks.len()];
    let mut any_dead = false;
    for n in 0..nblk {
        let bid = f.rpo[n];
        let b = &f.blks[bid.0 as usize];
        if b.visit == 0 {
            dead_blks[bid.0 as usize] = true;
            any_dead = true;
        }
    }

    // Remove edges from dead blocks first.
    if any_dead {
        for n in 0..nblk {
            let bid = f.rpo[n];
            if !dead_blks[bid.0 as usize] {
                continue;
            }
            let s1 = f.blks[bid.0 as usize].s1;
            let s2 = f.blks[bid.0 as usize].s2;
            if s1.is_some() {
                cfg::edgedel(f, bid, 0);
            }
            if s2.is_some() {
                cfg::edgedel(f, bid, 1);
            }
        }
    }

    // Process live blocks: rewrite phis, instructions, and jumps.
    for n in 0..nblk {
        let bid = f.rpo[n];
        if dead_blks[bid.0 as usize] {
            continue;
        }

        let bi = bid.0 as usize;

        // Rewrite phi nodes.
        let mut new_phis: Vec<Phi> = Vec::new();
        let phis = std::mem::take(&mut f.blks[bi].phi);
        for mut p in phis {
            if let Ref::Tmp(t) = p.to {
                if val[t.0 as usize] != LatVal::Bot {
                    // This phi produces a constant; remove it.
                    continue;
                }
            }
            // Rename constant args on live edges.
            for a in 0..p.narg() {
                if !deadedge(p.blks[a].0 as usize, n, &edge) {
                    renref(&mut p.args[a], &val);
                }
            }
            new_phis.push(p);
        }
        f.blks[bi].phi = new_phis;

        // Rewrite instructions.
        let nins = f.blks[bi].ins.len();
        for ii in 0..nins {
            let mut ins = f.blks[bi].ins[ii];
            if renref(&mut ins.to, &val) {
                // Instruction produces a constant; nop it out.
                ins = Ins::default(); // Nop
            } else {
                renref(&mut ins.arg[0], &val);
                renref(&mut ins.arg[1], &val);
                if ins.op.is_store() && ins.arg[0] == Ref::UNDEF {
                    ins = Ins::default(); // Nop
                }
            }
            f.blks[bi].ins[ii] = ins;
        }

        // Rewrite jump argument.
        let mut jmp = f.blks[bi].jmp;
        renref(&mut jmp.arg, &val);
        if jmp.typ.is_jnz() {
            if let Ref::Con(c) = jmp.arg {
                if iscon(&f.cons[c.0 as usize], false, 0) {
                    // Condition is false: remove true branch (s1), keep false (s2).
                    cfg::edgedel(f, bid, 0);
                    let s2 = f.blks[bi].s2;
                    f.blks[bi].s1 = s2;
                    f.blks[bi].s2 = None;
                } else {
                    // Condition is true: remove false branch (s2).
                    cfg::edgedel(f, bid, 1);
                }
                jmp.typ = Jmp::Jmp_;
                jmp.arg = Ref::R;
            }
        }
        f.blks[bi].jmp = jmp;
    }

    // Remove dead blocks from the block list and rebuild rpo.
    if any_dead {
        // Mark dead blocks: clear their successors so they are truly disconnected.
        for n in 0..nblk {
            let bid = f.rpo[n];
            if dead_blks[bid.0 as usize] {
                f.blks[bid.0 as usize].jmp = JmpInfo::default();
                f.blks[bid.0 as usize].s1 = None;
                f.blks[bid.0 as usize].s2 = None;
                f.blks[bid.0 as usize].phi.clear();
                f.blks[bid.0 as usize].ins.clear();
            }
        }
        // Rebuild RPO to exclude dead blocks.
        cfg::fillrpo(f);
    }
}

// ---------------------------------------------------------------------------
// Constant folding: integer operations
// ---------------------------------------------------------------------------

/// Fold an integer operation on two constants.
/// Returns `None` if the operation cannot be folded (e.g., address arithmetic
/// that isn't supported), or `Some(con)` with the result constant.
fn foldint(op: Op, w: bool, cl: &Con, cr: &Con) -> Option<Con> {
    let mut sym = Sym::default();
    let mut typ = ConType::Bits;

    let l_u = cl.bits.i() as u64;
    let r_u = cr.bits.i() as u64;
    let l_s = cl.bits.i();
    let r_s = cr.bits.i();

    // Address arithmetic checks.
    if op == Op::Add {
        if cl.typ == ConType::Addr {
            if cr.typ == ConType::Addr {
                return None;
            }
            typ = ConType::Addr;
            sym = cl.sym;
        } else if cr.typ == ConType::Addr {
            typ = ConType::Addr;
            sym = cr.sym;
        }
    } else if op == Op::Sub {
        if cl.typ == ConType::Addr {
            if cr.typ != ConType::Addr {
                typ = ConType::Addr;
                sym = cl.sym;
            } else if cl.sym != cr.sym {
                return None;
            }
            // else: Addr - Addr with same sym => Bits (offset difference)
        } else if cr.typ == ConType::Addr {
            return None;
        }
    } else if cl.typ == ConType::Addr || cr.typ == ConType::Addr {
        return None;
    }

    // Division/remainder by zero and overflow checks.
    if op == Op::Div || op == Op::Rem || op == Op::Udiv || op == Op::Urem {
        if iscon(cr, w, 0) {
            return None;
        }
        if op == Op::Div || op == Op::Rem {
            let min = if w { i64::MIN as u64 } else { i32::MIN as u64 };
            if iscon(cr, w, u64::MAX) {
                // -1 in unsigned representation
                if iscon(cl, w, min) {
                    return None;
                }
            }
        }
    }

    let x: u64 = match op {
        Op::Add => l_u.wrapping_add(r_u),
        Op::Sub => l_u.wrapping_sub(r_u),
        Op::Neg => 0u64.wrapping_sub(l_u),
        Op::Div => {
            if w {
                (l_s / r_s) as u64
            } else {
                ((l_s as i32) / (r_s as i32)) as u64
            }
        }
        Op::Rem => {
            if w {
                (l_s % r_s) as u64
            } else {
                ((l_s as i32) % (r_s as i32)) as u64
            }
        }
        Op::Udiv => {
            if w {
                l_u / r_u
            } else {
                ((l_u as u32) / (r_u as u32)) as u64
            }
        }
        Op::Urem => {
            if w {
                l_u % r_u
            } else {
                ((l_u as u32) % (r_u as u32)) as u64
            }
        }
        Op::Mul => l_u.wrapping_mul(r_u),
        Op::And => l_u & r_u,
        Op::Or => l_u | r_u,
        Op::Xor => l_u ^ r_u,
        Op::Sar => {
            let shift = r_u & (31 | if w { 32 } else { 0 });
            if w {
                (l_s >> shift) as u64
            } else {
                ((l_s as i32) >> shift) as u64
            }
        }
        Op::Shr => {
            let shift = r_u & (31 | if w { 32 } else { 0 });
            if w {
                l_u >> shift
            } else {
                ((l_u as u32) >> shift) as u64
            }
        }
        Op::Shl => {
            let shift = r_u & (31 | if w { 32 } else { 0 });
            l_u.wrapping_shl(shift as u32)
        }
        Op::Extsb => (l_u as i8) as i64 as u64,
        Op::Extub => (l_u as u8) as u64,
        Op::Extsh => (l_u as i16) as i64 as u64,
        Op::Extuh => (l_u as u16) as u64,
        Op::Extsw => (l_u as i32) as i64 as u64,
        Op::Extuw => (l_u as u32) as u64,
        Op::Stosi => {
            if w {
                (cl.bits.s() as i64) as u64
            } else {
                (cl.bits.s() as i32) as u64
            }
        }
        Op::Stoui => {
            if w {
                cl.bits.s() as u64
            } else {
                cl.bits.s() as u32 as u64
            }
        }
        Op::Dtosi => {
            if w {
                (cl.bits.d() as i64) as u64
            } else {
                (cl.bits.d() as i32) as u64
            }
        }
        Op::Dtoui => {
            if w {
                cl.bits.d() as u64
            } else {
                cl.bits.d() as u32 as u64
            }
        }
        Op::Cast => {
            if cl.typ == ConType::Addr {
                typ = ConType::Addr;
                sym = cl.sym;
            }
            l_u
        }
        _ => {
            let opi = op as u16;
            let ocmpw = Op::OCMPW as u16;
            let ocmpw1 = Op::OCMPW1 as u16;
            let ocmpl = Op::OCMPL as u16;
            let ocmpl1 = Op::OCMPL1 as u16;
            let ocmps = Op::OCMPS as u16;
            let ocmps1 = Op::OCMPS1 as u16;
            let ocmpd = Op::OCMPD as u16;
            let ocmpd1 = Op::OCMPD1 as u16;

            if opi >= ocmpw && opi <= ocmpl1 {
                // Integer comparison (word or long).
                let mut ll = l_u;
                let mut rr = r_u;
                let mut cmp_op = opi;
                if cmp_op <= ocmpw1 {
                    // Word comparison: sign-extend 32-bit values.
                    ll = (l_u as i32) as i64 as u64;
                    rr = (r_u as i32) as i64 as u64;
                } else {
                    // Long comparison: shift op down to word range.
                    cmp_op -= ocmpl - ocmpw;
                }
                let ll_s = ll as i64;
                let rr_s = rr as i64;
                let kind = (cmp_op - ocmpw) as u8;
                match kind {
                    x if x == CmpI::Ciule as u8 => {
                        if ll <= rr {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpI::Ciult as u8 => {
                        if ll < rr {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpI::Cisle as u8 => {
                        if ll_s <= rr_s {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpI::Cislt as u8 => {
                        if ll_s < rr_s {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpI::Cisgt as u8 => {
                        if ll_s > rr_s {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpI::Cisge as u8 => {
                        if ll_s >= rr_s {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpI::Ciugt as u8 => {
                        if ll > rr {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpI::Ciuge as u8 => {
                        if ll >= rr {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpI::Cieq as u8 => {
                        if ll == rr {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpI::Cine as u8 => {
                        if ll != rr {
                            1
                        } else {
                            0
                        }
                    }
                    _ => panic!("foldint: unreachable int cmp"),
                }
            } else if opi >= ocmps && opi <= ocmps1 {
                // Single-precision float comparison.
                let ls = cl.bits.s();
                let rs = cr.bits.s();
                let kind = (opi - ocmps) as u8;
                match kind {
                    x if x == CmpF::Cfle as u8 => {
                        if ls <= rs {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cflt as u8 => {
                        if ls < rs {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfgt as u8 => {
                        if ls > rs {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfge as u8 => {
                        if ls >= rs {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfne as u8 => {
                        if ls != rs {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfeq as u8 => {
                        if ls == rs {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfo as u8 => {
                        if ls < rs || ls >= rs {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfuo as u8 => {
                        if !(ls < rs || ls >= rs) {
                            1
                        } else {
                            0
                        }
                    }
                    _ => panic!("foldint: unreachable float cmp"),
                }
            } else if opi >= ocmpd && opi <= ocmpd1 {
                // Double-precision float comparison.
                let ld = cl.bits.d();
                let rd = cr.bits.d();
                let kind = (opi - ocmpd) as u8;
                match kind {
                    x if x == CmpF::Cfle as u8 => {
                        if ld <= rd {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cflt as u8 => {
                        if ld < rd {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfgt as u8 => {
                        if ld > rd {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfge as u8 => {
                        if ld >= rd {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfne as u8 => {
                        if ld != rd {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfeq as u8 => {
                        if ld == rd {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfo as u8 => {
                        if ld < rd || ld >= rd {
                            1
                        } else {
                            0
                        }
                    }
                    x if x == CmpF::Cfuo as u8 => {
                        if !(ld < rd || ld >= rd) {
                            1
                        } else {
                            0
                        }
                    }
                    _ => panic!("foldint: unreachable double cmp"),
                }
            } else {
                panic!("foldint: unreachable op {:?}", op);
            }
        }
    };

    Some(Con {
        typ,
        sym,
        bits: ConBits::from_i64(x as i64),
        flt: 0,
    })
}

// ---------------------------------------------------------------------------
// Constant folding: float operations
// ---------------------------------------------------------------------------

fn foldflt(op: Op, w: bool, cl: &Con, cr: &Con) -> Con {
    if cl.typ != ConType::Bits || cr.typ != ConType::Bits {
        panic!(
            "invalid address operand for '{}'",
            OP_TABLE[op as usize].name
        );
    }
    let mut res = Con {
        typ: ConType::Bits,
        sym: Sym::default(),
        bits: ConBits::from_i64(0),
        flt: 0,
    };
    if w {
        // Double precision.
        let ld = cl.bits.d();
        let rd = cr.bits.d();
        let xd: f64 = match op {
            Op::Add => ld + rd,
            Op::Sub => ld - rd,
            Op::Neg => -ld,
            Op::Div => ld / rd,
            Op::Mul => ld * rd,
            Op::Swtof => (cl.bits.i() as i32) as f64,
            Op::Uwtof => (cl.bits.i() as u32) as f64,
            Op::Sltof => (cl.bits.i()) as f64,
            Op::Ultof => (cl.bits.i() as u64) as f64,
            Op::Exts => cl.bits.s() as f64,
            Op::Cast => ld,
            _ => panic!("foldflt: unreachable op {:?}", op),
        };
        res.bits = ConBits::from_f64(xd);
        res.flt = 2;
    } else {
        // Single precision.
        let ls = cl.bits.s();
        let rs = cr.bits.s();
        let xs: f32 = match op {
            Op::Add => ls + rs,
            Op::Sub => ls - rs,
            Op::Neg => -ls,
            Op::Div => ls / rs,
            Op::Mul => ls * rs,
            Op::Swtof => (cl.bits.i() as i32) as f32,
            Op::Uwtof => (cl.bits.i() as u32) as f32,
            Op::Sltof => (cl.bits.i()) as f32,
            Op::Ultof => (cl.bits.i() as u64) as f32,
            Op::Truncd => cl.bits.d() as f32,
            Op::Cast => ls,
            _ => panic!("foldflt: unreachable op {:?}", op),
        };
        res.bits = ConBits::from_f32(xs);
        res.flt = 1;
    }
    res
}

// ---------------------------------------------------------------------------
// Top-level fold dispatch
// ---------------------------------------------------------------------------

fn opfold(op: Op, cls: Cls, cl_id: u32, cr_id: u32, f: &mut Fn) -> LatVal {
    let cl = f.cons[cl_id as usize];
    let cr = f.cons[cr_id as usize];

    let mut c = if cls == Cls::Kw || cls == Cls::Kl {
        match foldint(op, cls == Cls::Kl, &cl, &cr) {
            Some(c) => c,
            None => return LatVal::Bot,
        }
    } else {
        foldflt(op, cls == Cls::Kd, &cl, &cr)
    };

    if !cls.is_wide() {
        // Truncate to 32 bits for word/single classes.
        let bits = c.bits.i() as u64 & 0xffffffff;
        c.bits = ConBits::from_i64(bits as i64);
    }

    let r = newcon(&c, f);
    debug_assert!(!(cls == Cls::Ks || cls == Cls::Kd) || c.flt != 0);
    match r {
        Ref::Con(id) => LatVal::Con(id.0),
        _ => unreachable!(),
    }
}
