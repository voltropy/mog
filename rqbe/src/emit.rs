use std::fmt::Write as _;

use crate::ir::{
    Cls, Con, ConType, Dat, DatItem, Fn, Jmp, Lnk, Op, Ref, SymType, Target, Typ, OP_TABLE, TMP0,
};

// ---------------------------------------------------------------------------
// FPStash — deduplicated floating-point constant pool
// ---------------------------------------------------------------------------

/// Holds deduplicated floating-point bit patterns for later emission as
/// literal data sections.
pub struct FPStash {
    /// Each entry is `(bytes, id)` where `bytes` is 4, 8, or 16 bytes and
    /// `id` is the stash index.
    pub entries: Vec<(Vec<u8>, usize)>,
}

impl FPStash {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Deduplicate a floating-point constant bit pattern.  `bits` must be
    /// exactly 4, 8, or 16 bytes.  Returns the stash index (reused if the
    /// same pattern at the same or larger size already exists).
    pub fn stash(&mut self, bits: &[u8]) -> usize {
        let size = bits.len();
        assert!(size == 4 || size == 8 || size == 16);
        for (i, (stored, _)) in self.entries.iter().enumerate() {
            if size <= stored.len() && stored[..size] == *bits {
                return i;
            }
        }
        let id = self.entries.len();
        self.entries.push((bits.to_vec(), id));
        id
    }
}

impl Default for FPStash {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    static FP_STASH: std::cell::RefCell<FPStash> = std::cell::RefCell::new(FPStash::new());
}

/// Free-standing convenience wrapper matching the C signature.  Uses a
/// thread-local stash so that callers that cannot thread state through can
/// still use it (mirrors the C global `stash`).
pub fn stashbits(bits: &[u8]) -> usize {
    FP_STASH.with(|s| s.borrow_mut().stash(bits))
}

/// Access the thread-local FP stash for finalization.
pub fn with_fp_stash<F, R>(f: F) -> R
where
    F: FnOnce(&FPStash) -> R,
{
    FP_STASH.with(|s| f(&s.borrow()))
}

// ---------------------------------------------------------------------------
// DatState — tracks BSS auto-detection across emitdat calls
// ---------------------------------------------------------------------------

/// Tracks state across a sequence of `emitdat` calls for a single data
/// definition (Start … End).  Mirrors the C `static int64_t zero` variable.
pub struct DatState {
    /// Accumulated zero bytes.  `-1` means "already committed as .data".
    zero: i64,
    /// Name from the DatItem::Start item, persisted for later items.
    name: String,
    /// Linkage from the DatItem::Start item.
    lnk: Option<Lnk>,
}

impl DatState {
    pub fn new() -> Self {
        Self {
            zero: 0,
            name: String::new(),
            lnk: None,
        }
    }
}

impl Default for DatState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Linkage / section helpers
// ---------------------------------------------------------------------------

/// Section kinds matching the C `SecText/SecData/SecBss` enum.
#[derive(Copy, Clone)]
enum Sec {
    Text = 0,
    Data = 1,
    Bss = 2,
}

/// Emit linkage directives for a symbol (section, alignment, globl, label).
///
/// `target` supplies the assembler-symbol prefix (e.g. `_` on Mach-O).
/// If `target` is `None`, no prefix is used and thread-local is treated as
/// ELF-style.
fn emitlnk(name: &str, lnk: &Lnk, sec: Sec, target: Option<&Target>, out: &mut String) {
    let is_apple = target.map_or(false, |t| t.apple);
    // Assembler symbol prefix — on Apple `_`, else empty, unless the name
    // is already a quoted string.
    let pfx = if name.starts_with('"') {
        ""
    } else if is_apple {
        "_"
    } else {
        ""
    };

    let mut sfx = "";

    // Apple thread-local data needs special TLV bootstrap glue.
    if is_apple && lnk.thread {
        let _ = writeln!(out, ".section __DATA,__thread_vars,thread_local_variables");
        let _ = writeln!(out, "{pfx}{name}:");
        let _ = writeln!(out, "\t.quad __tlv_bootstrap");
        let _ = writeln!(out, "\t.quad 0");
        sfx = "$tlv$init";
        let _ = writeln!(out, "\t.quad {pfx}{name}{sfx}\n");
        // The actual data goes into __DATA,__thread_data.
        let _ = write!(out, ".section __DATA,__thread_data,thread_local_regular");
    } else if let Some(s) = &lnk.sec {
        let _ = write!(out, ".section {s}");
        if let Some(sf) = &lnk.secf {
            let _ = write!(out, ",{sf}");
        }
    } else {
        // Default section from thread × kind table.
        let sec_name = if lnk.thread {
            match sec {
                Sec::Text => ".abort \"unreachable\"",
                Sec::Data => ".section .tdata,\"awT\"",
                Sec::Bss => ".section .tbss,\"awT\"",
            }
        } else {
            match sec {
                Sec::Text => ".text",
                Sec::Data => ".data",
                Sec::Bss => ".bss",
            }
        };
        let _ = write!(out, "{sec_name}");
    }
    let _ = writeln!(out);

    if lnk.align > 0 {
        let _ = writeln!(out, ".balign {}", lnk.align);
    }
    if lnk.export {
        let _ = writeln!(out, ".globl {pfx}{name}");
    }
    let _ = writeln!(out, "{pfx}{name}{sfx}:");
}

/// Emit function linkage (`.text` section, `.globl`, alignment, label).
pub fn emitfnlnk(name: &str, lnk: &Lnk, out: &mut String) {
    emitlnk(name, lnk, Sec::Text, None, out);
}

/// Emit function linkage using a target for platform-specific prefix.
pub fn emitfnlnk_target(name: &str, lnk: &Lnk, target: &Target, out: &mut String) {
    emitlnk(name, lnk, Sec::Text, Some(target), out);
}

// ---------------------------------------------------------------------------
// Data emission
// ---------------------------------------------------------------------------

/// Emit one data definition item.  Call repeatedly with each `Dat` in a
/// `Start … End` sequence.  The `state` must persist across the sequence.
pub fn emitdat(dat: &Dat, state: &mut DatState, target: Option<&Target>, out: &mut String) {
    let is_apple = target.map_or(false, |t| t.apple);
    let pfx = if is_apple { "_" } else { "" };

    match &dat.item {
        DatItem::Start => {
            state.zero = 0;
            state.name = dat.name.clone().unwrap_or_default();
            state.lnk = dat.lnk.clone();
        }
        DatItem::End => {
            if state.zero != -1 {
                // Entire definition was zero — emit as BSS.
                if let Some(l) = &state.lnk {
                    emitlnk(&state.name, l, Sec::Bss, target, out);
                }
                let _ = writeln!(out, "\t.fill {},1,0", state.zero);
            }
        }
        DatItem::Zero(n) => {
            if state.zero != -1 {
                state.zero += *n as i64;
            } else {
                let _ = writeln!(out, "\t.fill {},1,0", n);
            }
        }
        DatItem::Str(s) => {
            commit_data_section(
                &state.name.clone(),
                state.lnk.as_ref(),
                &mut state.zero,
                target,
                out,
            );
            let _ = writeln!(out, "\t.ascii \"{s}\"");
        }
        DatItem::Ref {
            name: ref_name,
            off,
        } => {
            commit_data_section(
                &state.name.clone(),
                state.lnk.as_ref(),
                &mut state.zero,
                target,
                out,
            );
            let rp = if ref_name.starts_with('"') { "" } else { pfx };
            if *off != 0 {
                let _ = writeln!(out, "\t.quad {rp}{ref_name}{off:+}");
            } else {
                let _ = writeln!(out, "\t.quad {rp}{ref_name}");
            }
        }
        DatItem::FltS(f) => {
            commit_data_section(
                &state.name.clone(),
                state.lnk.as_ref(),
                &mut state.zero,
                target,
                out,
            );
            let _ = writeln!(out, "\t.int {}", f.to_bits() as i32);
        }
        DatItem::FltD(f) => {
            commit_data_section(
                &state.name.clone(),
                state.lnk.as_ref(),
                &mut state.zero,
                target,
                out,
            );
            let _ = writeln!(out, "\t.quad {}", f.to_bits() as i64);
        }
        item => {
            commit_data_section(
                &state.name.clone(),
                state.lnk.as_ref(),
                &mut state.zero,
                target,
                out,
            );
            let (directive, val) = match item {
                DatItem::Byte(v) => ("\t.byte", *v),
                DatItem::Half(v) => ("\t.short", *v),
                DatItem::Word(v) => ("\t.int", *v),
                DatItem::Long(v) => ("\t.quad", *v),
                _ => unreachable!(),
            };
            let _ = writeln!(out, "{directive} {val}");
        }
    }
}

/// If we haven't committed to a .data section yet, do so now and emit any
/// leading zeros.
fn commit_data_section(
    name: &str,
    lnk: Option<&Lnk>,
    zero: &mut i64,
    target: Option<&Target>,
    out: &mut String,
) {
    if *zero != -1 {
        if let Some(l) = lnk {
            emitlnk(name, l, Sec::Data, target, out);
        }
        if *zero > 0 {
            let _ = writeln!(out, "\t.fill {},1,0", *zero);
        }
        *zero = -1;
    }
}

// ---------------------------------------------------------------------------
// Floating-point constant finalization
// ---------------------------------------------------------------------------

/// Emit all stashed FP constants.  `sec` is `[literal4_section,
/// literal8_section, literal16_section]`.
fn emitfin_inner(stash: &FPStash, sec: &[&str; 3], asloc: &str, out: &mut String) {
    if stash.entries.is_empty() {
        return;
    }
    let _ = writeln!(out, "/* floating point constants */");

    // Iterate from largest to smallest alignment: 16, 8, 4 bytes.
    for lg in (2..=4).rev() {
        let size = 1usize << lg;
        for (i, (bits, _)) in stash.entries.iter().enumerate() {
            if bits.len() == size {
                let _ = writeln!(out, ".section {}", sec[lg - 2]);
                let _ = writeln!(out, ".p2align {lg}");
                let _ = write!(out, "{asloc}fp{i}:");
                // Emit 32-bit ints.
                for chunk in bits.chunks(4) {
                    let val = i32::from_le_bytes(chunk.try_into().unwrap());
                    let _ = write!(out, "\n\t.int {val}");
                }
                // Comment with floating-point value for 4/8 byte constants.
                if lg <= 3 {
                    let fval: f64 = if lg == 2 {
                        f32::from_le_bytes(bits[..4].try_into().unwrap()) as f64
                    } else {
                        f64::from_le_bytes(bits[..8].try_into().unwrap())
                    };
                    let _ = writeln!(out, " /* {fval} */");
                } else {
                    let _ = writeln!(out);
                }
                let _ = writeln!(out);
            }
        }
    }
}

/// Emit stashed FP constants for Mach-O targets.
pub fn macho_emitfin(stash: &FPStash, out: &mut String) {
    let sec = [
        "__TEXT,__literal4,4byte_literals",
        "__TEXT,__literal8,8byte_literals",
        ".abort \"unreachable\"",
    ];
    emitfin_inner(stash, &sec, "L", out);
}

/// Emit stashed FP constants for ELF targets.
pub fn elf_emitfin(stash: &FPStash, out: &mut String) {
    let sec = [".rodata", ".rodata", ".rodata"];
    emitfin_inner(stash, &sec, ".L", out);
    let _ = writeln!(out, ".section .note.GNU-stack,\"\",@progbits");
}

/// Emit FP constants and finalization for the given target.
pub fn emitfin(stash: &FPStash, target: &Target, out: &mut String) {
    if target.apple {
        macho_emitfin(stash, out);
    } else {
        elf_emitfin(stash, out);
    }
}

/// Emit ELF function type/size directives (called after each function body).
pub fn elf_emitfnfin(name: &str, out: &mut String) {
    let _ = writeln!(out, ".type {name}, @function");
    let _ = writeln!(out, ".size {name}, .-{name}");
}

// ---------------------------------------------------------------------------
// DWARF debug info
// ---------------------------------------------------------------------------

/// State for DWARF `.file` directive deduplication.
pub struct DbgFileState {
    files: Vec<String>,
    curfile: u32,
}

impl DbgFileState {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            curfile: 0,
        }
    }

    /// Emit `.file N "path"` if this is the first time we've seen `file`,
    /// then set the current file index.
    pub fn emitdbgfile(&mut self, file: &str, out: &mut String) {
        for (n, f) in self.files.iter().enumerate() {
            if f == file {
                // GAS requires positive file numbers.
                self.curfile = (n + 1) as u32;
                return;
            }
        }
        self.files.push(file.to_string());
        self.curfile = self.files.len() as u32;
        let _ = writeln!(out, ".file {} {}", self.curfile, file);
    }

    /// Emit `.loc fileno line [col]`.
    pub fn emitdbgloc(&self, line: u32, col: u32, out: &mut String) {
        if col != 0 {
            let _ = writeln!(out, "\t.loc {} {} {}", self.curfile, line, col);
        } else {
            let _ = writeln!(out, "\t.loc {} {}", self.curfile, line);
        }
    }
}

impl Default for DbgFileState {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience free-standing wrappers that use a thread-local debug state.
pub fn emitdbgfile(file: &str, out: &mut String) {
    thread_local! {
        static STATE: std::cell::RefCell<DbgFileState> =
            std::cell::RefCell::new(DbgFileState::new());
    }
    STATE.with(|s| s.borrow_mut().emitdbgfile(file, out));
}

pub fn emitdbgloc(line: u32, col: u32, out: &mut String) {
    thread_local! {
        static STATE: std::cell::RefCell<DbgFileState> =
            std::cell::RefCell::new(DbgFileState::new());
    }
    STATE.with(|s| s.borrow().emitdbgloc(line, col, out));
}

// ---------------------------------------------------------------------------
// IR printer — printcon
// ---------------------------------------------------------------------------

/// Print a `Con` value in QBE IL syntax.
fn printcon(c: &Con, f: &Fn, out: &mut String) {
    match c.typ {
        ConType::Undef => {}
        ConType::Addr => {
            if c.sym.typ == SymType::Thr {
                let _ = write!(out, "thread ");
            }
            let sym_name = if (c.sym.id as usize) < f.strs.len() {
                f.strs[c.sym.id as usize].as_str()
            } else {
                "???"
            };
            let _ = write!(out, "${sym_name}");
            if c.bits.i() != 0 {
                let _ = write!(out, "{:+}", c.bits.i());
            }
        }
        ConType::Bits => {
            if c.flt == 1 {
                let _ = write!(out, "s_{}", c.bits.s());
            } else if c.flt == 2 {
                let _ = write!(out, "d_{}", c.bits.d());
            } else {
                let _ = write!(out, "{}", c.bits.i());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// IR printer — printref
// ---------------------------------------------------------------------------

/// Print a single `Ref` in QBE IL syntax.
pub fn printref(r: Ref, f: &Fn, out: &mut String) {
    match r {
        Ref::R => {}
        Ref::Tmp(id) => {
            if id.0 < TMP0 {
                let _ = write!(out, "R{}", id.0);
            } else {
                let name = &f.tmps[id.0 as usize].name;
                let _ = write!(out, "%{name}");
            }
        }
        Ref::Con(id) => {
            if r == Ref::UNDEF {
                let _ = write!(out, "UNDEF");
            } else {
                printcon(&f.cons[id.0 as usize], f, out);
            }
        }
        Ref::Slot(n) => {
            let _ = write!(out, "S{}", n as i32);
        }
        Ref::Call(v) => {
            let _ = write!(out, "{v:04x}");
        }
        Ref::Typ(id) => {
            // We'd need a global typ table to print the name.
            let _ = write!(out, ":typ{}", id.0);
        }
        Ref::Mem(id) => {
            let m = &f.mems[id.0 as usize];
            let _ = write!(out, "[");
            let mut need_plus = false;
            if m.offset.typ != ConType::Undef {
                printcon(&m.offset, f, out);
                need_plus = true;
            }
            if !m.base.is_none() {
                if need_plus {
                    let _ = write!(out, " + ");
                }
                printref(m.base, f, out);
                need_plus = true;
            }
            if !m.index.is_none() {
                if need_plus {
                    let _ = write!(out, " + ");
                }
                let _ = write!(out, "{} * ", m.scale);
                printref(m.index, f, out);
            }
            let _ = write!(out, "]");
        }
        Ref::Int(v) => {
            let _ = write!(out, "{v}");
        }
    }
}

/// Print a single `Ref` in QBE IL syntax, with access to a global `Typ`
/// table for resolving type names.
pub fn printref_with_typs(r: Ref, f: &Fn, typs: &[Typ], out: &mut String) {
    if let Ref::Typ(id) = r {
        let _ = write!(out, ":{}", typs[id.0 as usize].name);
    } else {
        printref(r, f, out);
    }
}

// ---------------------------------------------------------------------------
// IR printer — printfn
// ---------------------------------------------------------------------------

/// Map a `Cls` to its QBE IL character: `w`, `l`, `s`, `d`.
fn ktoc(cls: Cls) -> char {
    match cls {
        Cls::Kw => 'w',
        Cls::Kl => 'l',
        Cls::Ks => 's',
        Cls::Kd => 'd',
        Cls::Kx => 'x',
    }
}

/// Map a `Jmp` to its QBE IL name.
fn jtoa(j: Jmp) -> &'static str {
    match j {
        Jmp::Jxxx => "xxx",
        Jmp::Retw => "retw",
        Jmp::Retl => "retl",
        Jmp::Rets => "rets",
        Jmp::Retd => "retd",
        Jmp::Retsb => "retsb",
        Jmp::Retub => "retub",
        Jmp::Retsh => "retsh",
        Jmp::Retuh => "retuh",
        Jmp::Retc => "retc",
        Jmp::Ret0 => "ret",
        Jmp::Jmp_ => "jmp",
        Jmp::Jnz => "jnz",
        Jmp::Jfieq => "jfieq",
        Jmp::Jfine => "jfine",
        Jmp::Jfisge => "jfisge",
        Jmp::Jfisgt => "jfisgt",
        Jmp::Jfisle => "jfisle",
        Jmp::Jfislt => "jfislt",
        Jmp::Jfiuge => "jfiuge",
        Jmp::Jfiugt => "jfiugt",
        Jmp::Jfiule => "jfiule",
        Jmp::Jfiult => "jfiult",
        Jmp::Jffeq => "jffeq",
        Jmp::Jffge => "jffge",
        Jmp::Jffgt => "jffgt",
        Jmp::Jffle => "jffle",
        Jmp::Jfflt => "jfflt",
        Jmp::Jffne => "jffne",
        Jmp::Jffo => "jffo",
        Jmp::Jffuo => "jffuo",
        Jmp::Hlt => "hlt",
    }
}

/// Ops that print their class suffix even when `to` is `R` (no destination).
fn needs_cls_suffix(op: Op) -> bool {
    matches!(
        op,
        Op::Arg
            | Op::Swap
            | Op::Xcmp
            | Op::Acmp
            | Op::Acmn
            | Op::Afcmp
            | Op::Xtest
            | Op::Xdiv
            | Op::Xidiv
    )
}

/// Print a function in QBE IL format.
///
/// Blocks are traversed in RPO order (via `f.rpo`).  If `f.rpo` is empty,
/// blocks are traversed in index order starting from `f.start`.
pub fn printfn(f: &Fn, typs: &[Typ], out: &mut String) {
    let _ = writeln!(out, "function ${}() {{", f.name);

    // Build the block iteration order.
    let order: Vec<usize> = if !f.rpo.is_empty() {
        f.rpo.iter().map(|bid| bid.0 as usize).collect()
    } else if !f.blks.is_empty() {
        // Fallback: linear from start.
        let mut v = Vec::new();
        let idx = f.start.0 as usize;
        // Just iterate all blocks in order.  A more faithful traversal
        // would follow a link chain, but the Rust port uses Vec indices.
        for i in 0..f.blks.len() {
            // Start from f.start, then wrap around.
            let bi = (idx + i) % f.blks.len();
            v.push(bi);
        }
        v
    } else {
        Vec::new()
    };

    for (order_idx, &bi) in order.iter().enumerate() {
        let b = &f.blks[bi];
        let _ = writeln!(out, "@{}", b.name);

        // --- Phi nodes ---
        for p in &b.phi {
            let _ = write!(out, "\t");
            printref(p.to, f, out);
            let _ = write!(out, " ={} phi ", ktoc(p.cls));
            assert!(p.narg() > 0);
            for n in 0..p.narg() {
                let pred_name = &f.blks[p.blks[n].0 as usize].name;
                let _ = write!(out, "@{pred_name} ");
                printref(p.args[n], f, out);
                if n == p.narg() - 1 {
                    let _ = writeln!(out);
                } else {
                    let _ = write!(out, ", ");
                }
            }
        }

        // --- Instructions ---
        for i in &b.ins {
            let _ = write!(out, "\t");
            if !i.to.is_none() {
                printref(i.to, f, out);
                let _ = write!(out, " ={} ", ktoc(i.cls));
            }
            let name = OP_TABLE[i.op as usize].name;
            let _ = write!(out, "{name}");
            // Some ops print a class suffix even without a destination.
            if i.to.is_none() && needs_cls_suffix(i.op) {
                let _ = write!(out, "{}", ktoc(i.cls));
            }
            if !i.arg[0].is_none() {
                let _ = write!(out, " ");
                printref(i.arg[0], f, out);
            }
            if !i.arg[1].is_none() {
                let _ = write!(out, ", ");
                printref(i.arg[1], f, out);
            }
            let _ = writeln!(out);
        }

        // --- Jump ---
        let jmp = &b.jmp;
        match jmp.typ {
            Jmp::Retw
            | Jmp::Retl
            | Jmp::Rets
            | Jmp::Retd
            | Jmp::Retsb
            | Jmp::Retub
            | Jmp::Retsh
            | Jmp::Retuh
            | Jmp::Retc
            | Jmp::Ret0 => {
                let _ = write!(out, "\t{}", jtoa(jmp.typ));
                if jmp.typ != Jmp::Ret0 || !jmp.arg.is_none() {
                    let _ = write!(out, " ");
                    printref(jmp.arg, f, out);
                }
                if jmp.typ == Jmp::Retc {
                    if f.retty >= 0 && (f.retty as usize) < typs.len() {
                        let _ = write!(out, ", :{}", typs[f.retty as usize].name);
                    }
                }
                let _ = writeln!(out);
            }
            Jmp::Hlt => {
                let _ = writeln!(out, "\thlt");
            }
            Jmp::Jmp_ => {
                // Omit unconditional jump to the next block in layout.
                let s1 = b.s1;
                let next_bid = order.get(order_idx + 1).copied();
                if s1.map(|b| b.0 as usize) != next_bid {
                    if let Some(s1_id) = s1 {
                        let _ = writeln!(out, "\tjmp @{}", f.blks[s1_id.0 as usize].name);
                    }
                }
            }
            Jmp::Jnz => {
                let _ = write!(out, "\tjnz ");
                printref(jmp.arg, f, out);
                let _ = write!(out, ", ");
                let s1_name =
                    b.s1.map(|id| f.blks[id.0 as usize].name.as_str())
                        .unwrap_or("???");
                let s2_name =
                    b.s2.map(|id| f.blks[id.0 as usize].name.as_str())
                        .unwrap_or("???");
                let _ = writeln!(out, "@{s1_name}, @{s2_name}");
            }
            Jmp::Jxxx => {
                // No terminator yet (shouldn't happen in well-formed IR).
            }
            _ => {
                // Flag-based conditional jumps (jfieq, jfine, etc.)
                let _ = write!(out, "\t{} ", jtoa(jmp.typ));
                if jmp.typ == Jmp::Jnz {
                    printref(jmp.arg, f, out);
                    let _ = write!(out, ", ");
                }
                let s1_name =
                    b.s1.map(|id| f.blks[id.0 as usize].name.as_str())
                        .unwrap_or("???");
                let s2_name =
                    b.s2.map(|id| f.blks[id.0 as usize].name.as_str())
                        .unwrap_or("???");
                let _ = writeln!(out, "@{s1_name}, @{s2_name}");
            }
        }
    }

    let _ = writeln!(out, "}}");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Lnk;

    #[test]
    fn test_fpstash_dedup() {
        let mut stash = FPStash::new();
        let a = stash.stash(&[1, 2, 3, 4]);
        let b = stash.stash(&[1, 2, 3, 4]);
        assert_eq!(a, b);
        let c = stash.stash(&[5, 6, 7, 8]);
        assert_ne!(a, c);
    }

    #[test]
    fn test_fpstash_size_dedup() {
        let mut stash = FPStash::new();
        // 8-byte entry.
        let a = stash.stash(&[1, 2, 3, 4, 5, 6, 7, 8]);
        // 4-byte prefix matches — should reuse.
        let b = stash.stash(&[1, 2, 3, 4]);
        assert_eq!(a, b);
    }

    #[test]
    fn test_emitfnlnk_basic() {
        let lnk = Lnk {
            export: true,
            thread: false,
            align: 4,
            sec: None,
            secf: None,
        };
        let mut out = String::new();
        emitfnlnk("main", &lnk, &mut out);
        assert!(out.contains(".text"));
        assert!(out.contains(".globl main"));
        assert!(out.contains(".balign 4"));
        assert!(out.contains("main:"));
    }

    #[test]
    fn test_emitdat_bss() {
        let lnk = Lnk::default();
        let mut state = DatState::new();
        let mut out = String::new();

        // Start.
        emitdat(
            &Dat {
                item: DatItem::Start,
                name: Some("x".into()),
                lnk: Some(lnk.clone()),
            },
            &mut state,
            None,
            &mut out,
        );
        // Only zeros.
        emitdat(
            &Dat {
                item: DatItem::Zero(16),
                name: Some("x".into()),
                lnk: Some(lnk.clone()),
            },
            &mut state,
            None,
            &mut out,
        );
        // End.
        emitdat(
            &Dat {
                item: DatItem::End,
                name: Some("x".into()),
                lnk: Some(lnk),
            },
            &mut state,
            None,
            &mut out,
        );

        assert!(out.contains(".bss"));
        assert!(out.contains(".fill 16,1,0"));
    }

    #[test]
    fn test_emitdat_data() {
        let lnk = Lnk::default();
        let mut state = DatState::new();
        let mut out = String::new();

        emitdat(
            &Dat {
                item: DatItem::Start,
                name: Some("y".into()),
                lnk: Some(lnk.clone()),
            },
            &mut state,
            None,
            &mut out,
        );
        emitdat(
            &Dat {
                item: DatItem::Word(42),
                name: Some("y".into()),
                lnk: Some(lnk.clone()),
            },
            &mut state,
            None,
            &mut out,
        );
        emitdat(
            &Dat {
                item: DatItem::End,
                name: Some("y".into()),
                lnk: Some(lnk),
            },
            &mut state,
            None,
            &mut out,
        );

        assert!(out.contains(".data"));
        assert!(out.contains(".int 42"));
    }

    #[test]
    fn test_macho_emitfin() {
        let mut stash = FPStash::new();
        let _ = stash.stash(&1.0f32.to_le_bytes());
        let mut out = String::new();
        macho_emitfin(&stash, &mut out);
        assert!(out.contains("__TEXT,__literal4,4byte_literals"));
        assert!(out.contains("Lfp0:"));
    }

    #[test]
    fn test_ktoc() {
        assert_eq!(ktoc(Cls::Kw), 'w');
        assert_eq!(ktoc(Cls::Kl), 'l');
        assert_eq!(ktoc(Cls::Ks), 's');
        assert_eq!(ktoc(Cls::Kd), 'd');
    }

    #[test]
    fn test_dbgfile_loc() {
        let mut dbg = DbgFileState::new();
        let mut out = String::new();
        dbg.emitdbgfile("\"test.c\"", &mut out);
        assert!(out.contains(".file 1 \"test.c\""));
        dbg.emitdbgloc(10, 5, &mut out);
        assert!(out.contains(".loc 1 10 5"));
        dbg.emitdbgloc(20, 0, &mut out);
        assert!(out.contains(".loc 1 20\n"));
    }

    #[test]
    fn test_elf_emitfin() {
        let mut stash = FPStash::new();
        let _ = stash.stash(&2.0f64.to_le_bytes());
        let mut out = String::new();
        elf_emitfin(&stash, &mut out);
        assert!(out.contains(".rodata"));
        assert!(out.contains(".Lfp0:"));
        assert!(out.contains("GNU-stack"));
    }
}
