//! Utility functions ported from QBE's `util.c`.
//!
//! In C QBE these rely on global state (instruction buffer `insb`/`curi`,
//! intern table, pool allocator). In Rust we use local structs instead.

use std::collections::HashMap;

use crate::ir::{Cls, Con, ConBits, ConId, ConType, Fn, Ins, Op, Ref, Sym, Tmp, TmpId, N_CMP_I};

// ---------------------------------------------------------------------------
// String interning
// ---------------------------------------------------------------------------

/// A string interner mapping strings to `u32` IDs and back.
///
/// Port of C QBE's `intern()` / `str()` which use a global hash table
/// (`itbl[]`) with bucket chains. Here we use a `HashMap` for simplicity.
pub struct StringInterner {
    map: HashMap<String, u32>,
    strings: Vec<String>,
}

impl StringInterner {
    /// Create a new, empty interner.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            strings: Vec::new(),
        }
    }

    /// Intern a string, returning its unique ID.
    /// If the string has already been interned, returns the existing ID.
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.map.get(s) {
            return id;
        }
        let id = self.strings.len() as u32;
        self.strings.push(s.to_owned());
        self.map.insert(s.to_owned(), id);
        id
    }

    /// Retrieve a previously interned string by its ID.
    ///
    /// # Panics
    /// Panics if the ID is out of range.
    pub fn get(&self, id: u32) -> &str {
        &self.strings[id as usize]
    }

    /// Number of interned strings.
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Whether the interner is empty.
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Hash function
// ---------------------------------------------------------------------------

/// Port of QBE's `hash()` from util.c.
///
/// ```c
/// for (h=0; *s; ++s)
///     h = *s + 17*h;
/// ```
///
/// This is used for symbol lookup (the intern table uses it modulo a mask).
pub fn hash(s: &str) -> u32 {
    let mut h: u32 = 0;
    for &b in s.as_bytes() {
        h = (b as u32).wrapping_add(h.wrapping_mul(17));
    }
    h
}

// ---------------------------------------------------------------------------
// Instruction buffer
// ---------------------------------------------------------------------------

/// A buffer for emitting instructions.
///
/// In C QBE, instructions are emitted in *reverse* order into a global buffer
/// (`insb[]` / `curi`), then copied out. We preserve the reverse-emit
/// semantics: `emit()` prepends and `as_slice()` returns instructions in
/// program order.
pub struct InsBuffer {
    buf: Vec<Ins>,
}

impl InsBuffer {
    /// Create a new, empty instruction buffer.
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Emit a single instruction (prepend — matching C QBE's `*--curi = …`).
    ///
    /// Instructions are stored in reverse emission order internally and
    /// reversed on [`as_slice`] / [`finish`].
    pub fn emit(&mut self, op: Op, cls: Cls, to: Ref, a0: Ref, a1: Ref) {
        self.buf.push(Ins {
            op,
            cls,
            to,
            arg: [a0, a1],
        });
    }

    /// Emit an already-formed instruction (prepend).
    pub fn emiti(&mut self, ins: Ins) {
        self.buf.push(ins);
    }

    /// Return the instructions in program order (reverse of emission order).
    ///
    /// This allocates a new `Vec`. For a non-allocating view, use
    /// [`as_slice_reversed`] which gives emission order.
    pub fn finish(&self) -> Vec<Ins> {
        let mut out = self.buf.clone();
        out.reverse();
        out
    }

    /// Return the instructions in emission order (most-recently-emitted first).
    pub fn as_slice(&self) -> &[Ins] {
        &self.buf
    }

    /// Number of buffered instructions.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    /// Get a mutable reference to the most recently emitted instruction.
    ///
    /// Since instructions are stored in emission order (reverse program order),
    /// this returns the last element of the internal buffer — i.e. the
    /// instruction most recently pushed via `emit()` or `emiti()`.
    pub fn last_mut(&mut self) -> &mut Ins {
        self.buf.last_mut().expect("InsBuffer is empty")
    }

    /// Get a mutable reference to the instruction at absolute index `idx`.
    ///
    /// Use this instead of `last_mut()` when `fixarg()` calls may have pushed
    /// additional instructions after the instruction you want to patch.
    pub fn at_mut(&mut self, idx: usize) -> &mut Ins {
        &mut self.buf[idx]
    }
}

impl Default for InsBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Duplicate a slice of instructions (port of C QBE's `idup`).
///
/// Returns a new `Vec<Ins>` containing a copy of `src`.
pub fn idup(src: &[Ins]) -> Vec<Ins> {
    src.to_vec()
}

/// Copy instructions from `src` into `dst`, appending them.
/// Returns the number of instructions copied.
///
/// Port of C QBE's `icpy(d, s, n)` which returns `d + n`.
pub fn icpy(dst: &mut Vec<Ins>, src: &[Ins]) -> usize {
    dst.extend_from_slice(src);
    src.len()
}

// ---------------------------------------------------------------------------
// Tmp helpers
// ---------------------------------------------------------------------------

/// Create a new temporary in the function with the given name prefix and class.
///
/// Port of C QBE's `newtmp()`. The C version uses a static counter for unique
/// naming (`prfx.N`); we replicate that with a counter embedded in the name.
///
/// Returns a `Ref::Tmp` referring to the new temporary.
pub fn newtmp(prefix: &str, cls: Cls, f: &mut Fn) -> Ref {
    // Use a thread-local counter for unique naming, matching C QBE's static int.
    use std::cell::Cell;
    thread_local! {
        static COUNTER: Cell<u32> = const { Cell::new(0) };
    }

    let n = COUNTER.with(|c| {
        let v = c.get() + 1;
        c.set(v);
        v
    });

    let t = f.tmps.len();
    let mut tmp = Tmp::default();
    if !prefix.is_empty() {
        tmp.name = format!("{}.{}", prefix, n);
    }
    tmp.cls = cls;
    tmp.slot = -1;
    tmp.nuse = 1;
    tmp.ndef = 1;
    f.tmps.push(tmp);

    Ref::Tmp(TmpId(t as u32))
}

/// Adjust the use count of a temporary.
///
/// Port of C QBE's `chuse(r, du, fn)`.
/// If `r` is a `Ref::Tmp`, adds `delta` to its `nuse` count.
pub fn chuse(r: Ref, delta: i32, f: &mut Fn) {
    if let Ref::Tmp(id) = r {
        let t = &mut f.tmps[id.0 as usize];
        t.nuse = (t.nuse as i32 + delta) as u32;
    }
}

/// Follow phi-class chains to find the representative temporary.
///
/// Port of C QBE's `phicls(t, tmp)` with path compression.
/// Returns the index of the representative temporary.
pub fn phicls(t: usize, tmps: &mut [Tmp]) -> usize {
    let t1 = tmps[t].phi;
    if t1 == 0 {
        return t;
    }
    let t1 = t1 as usize;
    let result = phicls(t1, tmps);
    tmps[t].phi = result as i32;
    result
}

// ---------------------------------------------------------------------------
// Constant helpers
// ---------------------------------------------------------------------------

/// Check if two symbols refer to the same entity.
///
/// Port of C QBE's `symeq()`.
#[inline]
fn symeq(s0: &Sym, s1: &Sym) -> bool {
    s0.typ == s1.typ && s0.id == s1.id
}

/// Add a constant to the function, deduplicating against existing constants.
///
/// Port of C QBE's `newcon()`. Skips index 0 (the undef sentinel).
/// Returns a `Ref::Con` referring to the (possibly existing) constant.
pub fn newcon(c: &Con, f: &mut Fn) -> Ref {
    // Search existing constants (skip index 0 = undef sentinel)
    for i in 1..f.cons.len() {
        let c1 = &f.cons[i];
        if c.typ == c1.typ && symeq(&c.sym, &c1.sym) && c.bits.i() == c1.bits.i() {
            return Ref::Con(ConId(i as u32));
        }
    }
    let i = f.cons.len();
    f.cons.push(*c);
    Ref::Con(ConId(i as u32))
}

/// Get or create an integer constant with the given value.
///
/// Port of C QBE's `getcon()`. Searches existing `CBits` constants first.
pub fn getcon(val: i64, f: &mut Fn) -> Ref {
    for c in 1..f.cons.len() {
        if f.cons[c].typ == ConType::Bits && f.cons[c].bits.i() == val {
            return Ref::Con(ConId(c as u32));
        }
    }
    let c = f.cons.len();
    f.cons.push(Con {
        typ: ConType::Bits,
        sym: Sym::default(),
        bits: ConBits::from_i64(val),
        flt: 0,
    });
    Ref::Con(ConId(c as u32))
}

/// Try to add two constants together.
///
/// Port of C QBE's `addcon()`. Returns `true` on success.
///
/// Rules:
/// - If `a` is `Undef`, it becomes a copy of `b`.
/// - If `b` is `Addr`, then `a` must not already be `Addr` (can't add two
///   addresses); if `a` is `Bits`, it becomes `Addr` with `b`'s symbol.
/// - The integer bits are always summed.
pub fn addcon(a: &mut Con, b: &Con) -> bool {
    if a.typ == ConType::Undef {
        *a = *b;
        return true;
    }

    if b.typ == ConType::Addr {
        if a.typ == ConType::Addr {
            // Can't add two addresses.
            return false;
        }
        a.typ = ConType::Addr;
        a.sym = b.sym;
    }
    a.bits = ConBits::from_i64(a.bits.i().wrapping_add(b.bits.i()));
    true
}

// ---------------------------------------------------------------------------
// Comparison helpers
// ---------------------------------------------------------------------------

/// Swap a comparison opcode's operands.
///
/// This is a thin wrapper that converts `Op` to its discriminant, calls the
/// `cmpop` already defined in `ir.rs`, and converts back.
///
/// Note: the canonical `cmpop(op: u16) -> u16` lives in `ir.rs` and operates
/// on raw discriminants. This wrapper provides a typed `Op -> Op` interface.
pub fn cmpop(op: Op) -> Op {
    let raw = crate::ir::cmpop(op as u16);
    // SAFETY: we trust cmpop returns a valid Op discriminant.
    debug_assert!((raw as usize) < crate::ir::N_OP);
    // Convert raw u16 back to Op via transmute (repr(u16) enum).
    // This is safe because cmpop only returns values within the Op range.
    unsafe { std::mem::transmute::<u16, Op>(raw) }
}

/// Negate a comparison opcode.
///
/// Typed `Op -> Op` wrapper around `ir::cmpneg`.
pub fn cmpneg(op: Op) -> Op {
    let raw = crate::ir::cmpneg(op as u16);
    debug_assert!((raw as usize) < crate::ir::N_OP);
    unsafe { std::mem::transmute::<u16, Op>(raw) }
}

/// Merge two value classes.
///
/// Typed wrapper around `ir::clsmerge`. Returns `true` if there is a conflict
/// (the classes are incompatible). This matches the C convention where a
/// nonzero return signals an error.
///
/// Special case from C QBE: `Kw` and `Kl` merge to `Kw` (not an error).
pub fn clsmerge(dest: &mut Cls, src: Cls) -> bool {
    if src == Cls::Kx {
        return false;
    }
    if *dest == Cls::Kx {
        *dest = src;
        return false;
    }
    // C QBE special case: Kw + Kl => Kw (not a conflict)
    if (*dest == Cls::Kw && src == Cls::Kl) || (*dest == Cls::Kl && src == Cls::Kw) {
        *dest = Cls::Kw;
        return false;
    }
    // Return true if mismatch (conflict)
    *dest != src
}

// ---------------------------------------------------------------------------
// Ref helpers
// ---------------------------------------------------------------------------

/// Check if a reference is a physical register.
///
/// Typed wrapper around `ir::isreg`.
#[inline]
pub fn isreg(r: Ref) -> bool {
    crate::ir::isreg(r)
}

/// Check if an opcode is a comparison. If so, returns `Some((kind, class))`:
/// - `kind`: the comparison kind index (0..N_CMP_I for integer, N_CMP_I.. for float)
/// - `class`: the class as an i32 (`Kw`=0, `Kl`=1, `Ks`=2, `Kd`=3)
///
/// Port of C QBE's `iscmp(op, &pk, &pc)`.
pub fn iscmp(op: Op) -> Option<(i32, i32)> {
    let o = op as u16;

    let ocmpw = Op::OCMPW as u16;
    let ocmpw1 = Op::OCMPW1 as u16;
    let ocmpl = Op::OCMPL as u16;
    let ocmpl1 = Op::OCMPL1 as u16;
    let ocmps = Op::OCMPS as u16;
    let ocmps1 = Op::OCMPS1 as u16;
    let ocmpd = Op::OCMPD as u16;
    let ocmpd1 = Op::OCMPD1 as u16;

    if o >= ocmpw && o <= ocmpw1 {
        Some(((o - ocmpw) as i32, Cls::Kw as i8 as i32))
    } else if o >= ocmpl && o <= ocmpl1 {
        Some(((o - ocmpl) as i32, Cls::Kl as i8 as i32))
    } else if o >= ocmps && o <= ocmps1 {
        Some((N_CMP_I as i32 + (o - ocmps) as i32, Cls::Ks as i8 as i32))
    } else if o >= ocmpd && o <= ocmpd1 {
        Some((N_CMP_I as i32 + (o - ocmpd) as i32, Cls::Kd as i8 as i32))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Stack allocation helper
// ---------------------------------------------------------------------------

/// Emit a stack allocation, ensuring 16-byte alignment.
///
/// Port of C QBE's `salloc()`. If the size is a known constant, it is
/// rounded up at compile time. Otherwise, emits runtime alignment code:
/// ```text
///   r1 = add rs, 15
///   r0 = and r1, -16
///   rt = salloc r0
/// ```
///
/// Note: This emits into an `InsBuffer` rather than a global buffer. The
/// caller must provide the buffer and the function context.
pub fn salloc(to: Ref, sz: Ref, f: &mut Fn, buf: &mut InsBuffer) {
    f.dynalloc = true;

    if let Ref::Con(id) = sz {
        let raw_sz = f.cons[id.0 as usize].bits.i();
        if raw_sz < 0 || raw_sz >= (i32::MAX as i64) - 15 {
            panic!("invalid alloc size {}", raw_sz);
        }
        let aligned = (raw_sz + 15) & !15;
        let con_ref = getcon(aligned, f);
        buf.emit(Op::Salloc, Cls::Kl, to, con_ref, Ref::R);
    } else {
        // Runtime alignment: r0 = (sz + 15) & -16
        let r0 = newtmp("isel", Cls::Kl, f);
        let r1 = newtmp("isel", Cls::Kl, f);
        let neg16 = getcon(-16, f);
        let fifteen = getcon(15, f);

        // Emit in reverse order (matching C QBE's emit pattern):
        // These are pushed in reverse program order.
        buf.emit(Op::Salloc, Cls::Kl, to, r0, Ref::R);
        buf.emit(Op::And, Cls::Kl, r0, r1, neg16);
        buf.emit(Op::Add, Cls::Kl, r1, sz, fifteen);

        // Verify the source isn't a stack slot (C QBE checks this).
        if let Ref::Tmp(tid) = sz {
            if f.tmps[tid.0 as usize].slot != -1 {
                let to_name = if let Ref::Tmp(to_id) = to {
                    f.tmps[to_id.0 as usize].name.clone()
                } else {
                    "?".to_string()
                };
                panic!(
                    "unlikely alloc argument %{} for %{}",
                    f.tmps[tid.0 as usize].name, to_name,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Cls, Con, ConBits, ConType, Op, Ref, Sym, TmpId};

    #[test]
    fn test_hash_basic() {
        // Deterministic: same input -> same output
        assert_eq!(hash("hello"), hash("hello"));
        assert_ne!(hash("hello"), hash("world"));
        // Empty string -> 0
        assert_eq!(hash(""), 0);
    }

    #[test]
    fn test_string_interner() {
        let mut si = StringInterner::new();
        let id_a = si.intern("alpha");
        let id_b = si.intern("beta");
        let id_a2 = si.intern("alpha");

        assert_eq!(id_a, id_a2);
        assert_ne!(id_a, id_b);
        assert_eq!(si.get(id_a), "alpha");
        assert_eq!(si.get(id_b), "beta");
        assert_eq!(si.len(), 2);
    }

    #[test]
    fn test_ins_buffer() {
        let mut buf = InsBuffer::new();
        buf.emit(Op::Add, Cls::Kw, Ref::R, Ref::R, Ref::R);
        buf.emit(Op::Sub, Cls::Kl, Ref::R, Ref::R, Ref::R);
        assert_eq!(buf.len(), 2);

        // finish() reverses: first emitted = last in program order
        let ins = buf.finish();
        assert_eq!(ins[0].op, Op::Sub);
        assert_eq!(ins[1].op, Op::Add);
    }

    #[test]
    fn test_newtmp() {
        let mut f = Fn::default();
        // Seed with the undef constant
        f.cons.push(Con::default());

        let r = newtmp("test", Cls::Kw, &mut f);
        assert!(r.is_tmp());
        if let Ref::Tmp(id) = r {
            assert!(f.tmps[id.0 as usize].name.starts_with("test."));
            assert_eq!(f.tmps[id.0 as usize].cls, Cls::Kw);
            assert_eq!(f.tmps[id.0 as usize].slot, -1);
        }
    }

    #[test]
    fn test_chuse() {
        let mut f = Fn::default();
        let mut tmp = Tmp::default();
        tmp.nuse = 5;
        f.tmps.push(tmp);

        let r = Ref::Tmp(TmpId(0));
        chuse(r, 3, &mut f);
        assert_eq!(f.tmps[0].nuse, 8);
        chuse(r, -2, &mut f);
        assert_eq!(f.tmps[0].nuse, 6);

        // Non-tmp refs are no-ops
        chuse(Ref::R, 10, &mut f);
        chuse(Ref::Int(42), 10, &mut f);
    }

    #[test]
    fn test_getcon() {
        let mut f = Fn::default();
        // Index 0 = undef sentinel
        f.cons.push(Con::default());

        let r1 = getcon(42, &mut f);
        let r2 = getcon(42, &mut f);
        let r3 = getcon(99, &mut f);

        // Same value -> same ref
        assert_eq!(r1, r2);
        // Different value -> different ref
        assert_ne!(r1, r3);
        // Should be Con refs
        assert!(matches!(r1, Ref::Con(_)));
        // Total constants: sentinel + 42 + 99
        assert_eq!(f.cons.len(), 3);
    }

    #[test]
    fn test_newcon() {
        let mut f = Fn::default();
        f.cons.push(Con::default()); // undef sentinel

        let c1 = Con {
            typ: ConType::Bits,
            sym: Sym::default(),
            bits: ConBits::from_i64(100),
            flt: 0,
        };
        let r1 = newcon(&c1, &mut f);
        let r2 = newcon(&c1, &mut f);
        assert_eq!(r1, r2);
        assert_eq!(f.cons.len(), 2); // sentinel + one constant
    }

    #[test]
    fn test_addcon() {
        // Undef + Bits = Bits
        let mut a = Con {
            typ: ConType::Undef,
            ..Con::default()
        };
        let b = Con {
            typ: ConType::Bits,
            bits: ConBits::from_i64(42),
            ..Con::default()
        };
        assert!(addcon(&mut a, &b));
        assert_eq!(a.typ, ConType::Bits);
        assert_eq!(a.bits.i(), 42);

        // Bits + Bits = sum
        let c = Con {
            typ: ConType::Bits,
            bits: ConBits::from_i64(8),
            ..Con::default()
        };
        assert!(addcon(&mut a, &c));
        assert_eq!(a.bits.i(), 50);

        // Addr + Addr = fail
        let mut d = Con {
            typ: ConType::Addr,
            bits: ConBits::from_i64(10),
            ..Con::default()
        };
        let e = Con {
            typ: ConType::Addr,
            bits: ConBits::from_i64(20),
            ..Con::default()
        };
        assert!(!addcon(&mut d, &e));
    }

    #[test]
    fn test_clsmerge() {
        let mut k = Cls::Kx;
        assert!(!clsmerge(&mut k, Cls::Kw));
        assert_eq!(k, Cls::Kw);

        // Same class -> no conflict
        assert!(!clsmerge(&mut k, Cls::Kw));
        assert_eq!(k, Cls::Kw);

        // Kw + Kl -> Kw (C QBE special case)
        let mut kl = Cls::Kl;
        assert!(!clsmerge(&mut kl, Cls::Kw));
        assert_eq!(kl, Cls::Kw);

        // Kx src -> no-op
        let mut k2 = Cls::Ks;
        assert!(!clsmerge(&mut k2, Cls::Kx));
        assert_eq!(k2, Cls::Ks);

        // Mismatch (conflict)
        let mut k3 = Cls::Kw;
        assert!(clsmerge(&mut k3, Cls::Ks));
    }

    #[test]
    fn test_iscmp() {
        // Ceqw is the first word comparison
        let r = iscmp(Op::Ceqw);
        assert!(r.is_some());
        let (kind, cls) = r.unwrap();
        assert_eq!(kind, 0); // Cieq
        assert_eq!(cls, Cls::Kw as i8 as i32);

        // Ceql is the first long comparison
        let r = iscmp(Op::Ceql);
        assert!(r.is_some());
        let (kind, cls) = r.unwrap();
        assert_eq!(kind, 0); // Cieq
        assert_eq!(cls, Cls::Kl as i8 as i32);

        // Non-comparison
        assert!(iscmp(Op::Add).is_none());
    }

    #[test]
    fn test_cmpop_cmpneg_roundtrip() {
        // cmpop(cmpop(x)) == x for all comparisons
        let ops = [
            Op::Ceqw,
            Op::Cnew,
            Op::Csgew,
            Op::Csgtw,
            Op::Cslew,
            Op::Csltw,
            Op::Ceqs,
            Op::Cges,
            Op::Cgts,
            Op::Cles,
            Op::Clts,
            Op::Cnes,
        ];
        for &op in &ops {
            assert_eq!(cmpop(cmpop(op)), op, "cmpop roundtrip failed for {:?}", op);
        }

        // cmpneg(cmpneg(x)) == x for all comparisons
        for &op in &ops {
            assert_eq!(
                cmpneg(cmpneg(op)),
                op,
                "cmpneg roundtrip failed for {:?}",
                op
            );
        }
    }

    #[test]
    fn test_phicls() {
        let mut tmps = vec![Tmp::default(); 5];
        // Chain: 4 -> 3 -> 2, where 2 has phi=0 (sentinel = "I am the representative")
        // In C QBE, phi==0 means "no link", so tmp 2 is the representative.
        tmps[4].phi = 3;
        tmps[3].phi = 2;
        // tmps[2].phi = 0 (default) — sentinel, 2 is representative

        let rep = phicls(4, &mut tmps);
        assert_eq!(rep, 2);
        // Path compression should have updated intermediaries
        assert_eq!(tmps[4].phi, 2);
        assert_eq!(tmps[3].phi, 2);
    }

    #[test]
    fn test_salloc_constant() {
        let mut f = Fn::default();
        f.cons.push(Con::default()); // undef sentinel

        let sz_ref = getcon(100, &mut f);
        let to = newtmp("out", Cls::Kl, &mut f);

        let mut buf = InsBuffer::new();
        salloc(to, sz_ref, &mut f, &mut buf);

        assert!(f.dynalloc);
        let ins = buf.finish();
        assert_eq!(ins.len(), 1);
        assert_eq!(ins[0].op, Op::Salloc);
        // Size should be rounded to 112 (100 rounded up to next 16)
        if let Ref::Con(id) = ins[0].arg[0] {
            assert_eq!(f.cons[id.0 as usize].bits.i(), 112);
        } else {
            panic!("expected Con ref for aligned size");
        }
    }

    #[test]
    fn test_idup() {
        let ins = vec![
            Ins {
                op: Op::Add,
                cls: Cls::Kw,
                to: Ref::R,
                arg: [Ref::R, Ref::R],
            },
            Ins {
                op: Op::Sub,
                cls: Cls::Kl,
                to: Ref::R,
                arg: [Ref::R, Ref::R],
            },
        ];
        let dup = idup(&ins);
        assert_eq!(dup.len(), 2);
        assert_eq!(dup[0].op, Op::Add);
        assert_eq!(dup[1].op, Op::Sub);
    }
}
