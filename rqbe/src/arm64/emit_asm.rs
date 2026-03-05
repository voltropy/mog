// ARM64 assembly emission — port of QBE's arm64/emit.c (644 lines).
//
// Emits ARM64 assembly text for a function after register allocation.

use std::fmt::Write;

use crate::ir::*;

use super::isel_pass::logimm;
use super::regs::*;

// ---------------------------------------------------------------------------
// Comparison condition strings
// ---------------------------------------------------------------------------

/// ARM64 condition code strings, indexed by CmpI/CmpF kind.
static CTOA: [&str; N_CMP] = [
    // CmpI: Cieq..Ciult (0..9)
    "eq", "ne", "ge", "gt", "le", "lt", "cs", "hi", "ls", "cc",
    // CmpF: Cfeq..Cfuo (10..17)
    "eq", "ge", "gt", "ls", "mi", "ne", "vc", "vs",
];

// ---------------------------------------------------------------------------
// Opcode → assembly mapping table
// ---------------------------------------------------------------------------

/// A map entry: (op, cls_match, asm_format).
///
/// `cls_match`: Ki(-1) matches Kw/Kl, Ka(-2) matches all, else exact Cls.
const KI: i8 = -1;
const KA: i8 = -2;

struct OmapEntry {
    op: Op,
    cls: i8,
    asm: &'static str,
}

/// The omap[] table mapping QBE ops to ARM64 mnemonics.
static OMAP: &[OmapEntry] = &[
    OmapEntry {
        op: Op::Add,
        cls: KI,
        asm: "add\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Add,
        cls: KA,
        asm: "fadd\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Sub,
        cls: KI,
        asm: "sub\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Sub,
        cls: KA,
        asm: "fsub\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Neg,
        cls: KI,
        asm: "neg\t%=, %0",
    },
    OmapEntry {
        op: Op::Neg,
        cls: KA,
        asm: "fneg\t%=, %0",
    },
    OmapEntry {
        op: Op::And,
        cls: KI,
        asm: "and\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Or,
        cls: KI,
        asm: "orr\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Xor,
        cls: KI,
        asm: "eor\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Sar,
        cls: KI,
        asm: "asr\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Shr,
        cls: KI,
        asm: "lsr\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Shl,
        cls: KI,
        asm: "lsl\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Mul,
        cls: KI,
        asm: "mul\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Mul,
        cls: KA,
        asm: "fmul\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Div,
        cls: KI,
        asm: "sdiv\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Div,
        cls: KA,
        asm: "fdiv\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Udiv,
        cls: KI,
        asm: "udiv\t%=, %0, %1",
    },
    OmapEntry {
        op: Op::Rem,
        cls: KI,
        asm: "sdiv\t%?, %0, %1\n\tmsub\t%=, %?, %1, %0",
    },
    OmapEntry {
        op: Op::Urem,
        cls: KI,
        asm: "udiv\t%?, %0, %1\n\tmsub\t%=, %?, %1, %0",
    },
    OmapEntry {
        op: Op::Copy,
        cls: KI,
        asm: "mov\t%=, %0",
    },
    OmapEntry {
        op: Op::Copy,
        cls: KA,
        asm: "fmov\t%=, %0",
    },
    OmapEntry {
        op: Op::Swap,
        cls: KI,
        asm: "mov\t%?, %0\n\tmov\t%0, %1\n\tmov\t%1, %?",
    },
    OmapEntry {
        op: Op::Swap,
        cls: KA,
        asm: "fmov\t%?, %0\n\tfmov\t%0, %1\n\tfmov\t%1, %?",
    },
    OmapEntry {
        op: Op::Storeb,
        cls: Cls::Kw as i8,
        asm: "strb\t%W0, %M1",
    },
    OmapEntry {
        op: Op::Storeh,
        cls: Cls::Kw as i8,
        asm: "strh\t%W0, %M1",
    },
    OmapEntry {
        op: Op::Storew,
        cls: Cls::Kw as i8,
        asm: "str\t%W0, %M1",
    },
    OmapEntry {
        op: Op::Storel,
        cls: Cls::Kw as i8,
        asm: "str\t%L0, %M1",
    },
    OmapEntry {
        op: Op::Stores,
        cls: Cls::Kw as i8,
        asm: "str\t%S0, %M1",
    },
    OmapEntry {
        op: Op::Stored,
        cls: Cls::Kw as i8,
        asm: "str\t%D0, %M1",
    },
    OmapEntry {
        op: Op::Loadsb,
        cls: KI,
        asm: "ldrsb\t%=, %M0",
    },
    OmapEntry {
        op: Op::Loadub,
        cls: KI,
        asm: "ldrb\t%W=, %M0",
    },
    OmapEntry {
        op: Op::Loadsh,
        cls: KI,
        asm: "ldrsh\t%=, %M0",
    },
    OmapEntry {
        op: Op::Loaduh,
        cls: KI,
        asm: "ldrh\t%W=, %M0",
    },
    OmapEntry {
        op: Op::Loadsw,
        cls: Cls::Kw as i8,
        asm: "ldr\t%=, %M0",
    },
    OmapEntry {
        op: Op::Loadsw,
        cls: Cls::Kl as i8,
        asm: "ldrsw\t%=, %M0",
    },
    OmapEntry {
        op: Op::Loaduw,
        cls: KI,
        asm: "ldr\t%W=, %M0",
    },
    OmapEntry {
        op: Op::Load,
        cls: KA,
        asm: "ldr\t%=, %M0",
    },
    OmapEntry {
        op: Op::Extsb,
        cls: KI,
        asm: "sxtb\t%=, %W0",
    },
    OmapEntry {
        op: Op::Extub,
        cls: KI,
        asm: "uxtb\t%W=, %W0",
    },
    OmapEntry {
        op: Op::Extsh,
        cls: KI,
        asm: "sxth\t%=, %W0",
    },
    OmapEntry {
        op: Op::Extuh,
        cls: KI,
        asm: "uxth\t%W=, %W0",
    },
    OmapEntry {
        op: Op::Extsw,
        cls: KI,
        asm: "sxtw\t%L=, %W0",
    },
    OmapEntry {
        op: Op::Extuw,
        cls: KI,
        asm: "mov\t%W=, %W0",
    },
    OmapEntry {
        op: Op::Exts,
        cls: Cls::Kd as i8,
        asm: "fcvt\t%=, %S0",
    },
    OmapEntry {
        op: Op::Truncd,
        cls: Cls::Ks as i8,
        asm: "fcvt\t%=, %D0",
    },
    OmapEntry {
        op: Op::Cast,
        cls: Cls::Kw as i8,
        asm: "fmov\t%=, %S0",
    },
    OmapEntry {
        op: Op::Cast,
        cls: Cls::Kl as i8,
        asm: "fmov\t%=, %D0",
    },
    OmapEntry {
        op: Op::Cast,
        cls: Cls::Ks as i8,
        asm: "fmov\t%=, %W0",
    },
    OmapEntry {
        op: Op::Cast,
        cls: Cls::Kd as i8,
        asm: "fmov\t%=, %L0",
    },
    OmapEntry {
        op: Op::Stosi,
        cls: KA,
        asm: "fcvtzs\t%=, %S0",
    },
    OmapEntry {
        op: Op::Stoui,
        cls: KA,
        asm: "fcvtzu\t%=, %S0",
    },
    OmapEntry {
        op: Op::Dtosi,
        cls: KA,
        asm: "fcvtzs\t%=, %D0",
    },
    OmapEntry {
        op: Op::Dtoui,
        cls: KA,
        asm: "fcvtzu\t%=, %D0",
    },
    OmapEntry {
        op: Op::Swtof,
        cls: KA,
        asm: "scvtf\t%=, %W0",
    },
    OmapEntry {
        op: Op::Uwtof,
        cls: KA,
        asm: "ucvtf\t%=, %W0",
    },
    OmapEntry {
        op: Op::Sltof,
        cls: KA,
        asm: "scvtf\t%=, %L0",
    },
    OmapEntry {
        op: Op::Ultof,
        cls: KA,
        asm: "ucvtf\t%=, %L0",
    },
    OmapEntry {
        op: Op::Call,
        cls: Cls::Kw as i8,
        asm: "blr\t%L0",
    },
    OmapEntry {
        op: Op::Acmp,
        cls: KI,
        asm: "cmp\t%0, %1",
    },
    OmapEntry {
        op: Op::Acmn,
        cls: KI,
        asm: "cmn\t%0, %1",
    },
    OmapEntry {
        op: Op::Afcmp,
        cls: KA,
        asm: "fcmpe\t%0, %1",
    },
    // Flag/cset instructions: Flagieq + N_CMP_I integer + N_CMP_F float
    OmapEntry {
        op: Op::Flagieq,
        cls: KI,
        asm: "cset\t%=, eq",
    },
    OmapEntry {
        op: Op::Flagine,
        cls: KI,
        asm: "cset\t%=, ne",
    },
    OmapEntry {
        op: Op::Flagisge,
        cls: KI,
        asm: "cset\t%=, ge",
    },
    OmapEntry {
        op: Op::Flagisgt,
        cls: KI,
        asm: "cset\t%=, gt",
    },
    OmapEntry {
        op: Op::Flagisle,
        cls: KI,
        asm: "cset\t%=, le",
    },
    OmapEntry {
        op: Op::Flagislt,
        cls: KI,
        asm: "cset\t%=, lt",
    },
    OmapEntry {
        op: Op::Flagiuge,
        cls: KI,
        asm: "cset\t%=, cs",
    },
    OmapEntry {
        op: Op::Flagiugt,
        cls: KI,
        asm: "cset\t%=, hi",
    },
    OmapEntry {
        op: Op::Flagiule,
        cls: KI,
        asm: "cset\t%=, ls",
    },
    OmapEntry {
        op: Op::Flagiult,
        cls: KI,
        asm: "cset\t%=, cc",
    },
    OmapEntry {
        op: Op::Flagfeq,
        cls: KI,
        asm: "cset\t%=, eq",
    },
    OmapEntry {
        op: Op::Flagfge,
        cls: KI,
        asm: "cset\t%=, ge",
    },
    OmapEntry {
        op: Op::Flagfgt,
        cls: KI,
        asm: "cset\t%=, gt",
    },
    OmapEntry {
        op: Op::Flagfle,
        cls: KI,
        asm: "cset\t%=, ls",
    },
    OmapEntry {
        op: Op::Flagflt,
        cls: KI,
        asm: "cset\t%=, mi",
    },
    OmapEntry {
        op: Op::Flagfne,
        cls: KI,
        asm: "cset\t%=, ne",
    },
    OmapEntry {
        op: Op::Flagfo,
        cls: KI,
        asm: "cset\t%=, vc",
    },
    OmapEntry {
        op: Op::Flagfuo,
        cls: KI,
        asm: "cset\t%=, vs",
    },
];

// ---------------------------------------------------------------------------
// Emission context
// ---------------------------------------------------------------------------

struct E<'a> {
    out: &'a mut String,
    f: &'a Fn,
    frame: u64,
    padding: u64,
    apple: bool,
    asloc: &'static str,
    assym: &'static str,
}

// ---------------------------------------------------------------------------
// Register naming
// ---------------------------------------------------------------------------

fn rname(r: u32, k: Cls) -> String {
    if r == SP {
        return "sp".to_string();
    }
    if r >= R0 && r <= LR {
        let n = r - R0;
        match k {
            Cls::Kw => format!("w{n}"),
            Cls::Kl | Cls::Kx => format!("x{n}"),
            _ => panic!("invalid class for GPR"),
        }
    } else if r >= V0 && r <= V30 {
        let n = r - V0;
        match k {
            Cls::Ks => format!("s{n}"),
            Cls::Kd | Cls::Kx => format!("d{n}"),
            _ => panic!("invalid class for FPR"),
        }
    } else {
        panic!("invalid register {r}");
    }
}

/// Compute the stack frame offset for a slot or spill reference.
fn slot_offset(r: Ref, e: &E) -> u64 {
    let s = r.sval();
    if s == -1 {
        16 + e.frame
    } else if s < 0 {
        if e.f.vararg && !e.apple {
            16 + e.frame + 192 - ((s + 2) as u64).wrapping_neg().wrapping_add(1)
        } else {
            let offset = (-(s + 2)) as u64;
            16 + e.frame + offset
        }
    } else {
        16 + e.padding + 4 * (s as u64)
    }
}

// ---------------------------------------------------------------------------
// Instruction format string emitter
// ---------------------------------------------------------------------------

/// Emit an instruction using a format string with escapes:
///   %=  destination register
///   %0  arg[0] register
///   %1  arg[1] (register or constant)
///   %?  scratch register (R18/x18 for GP, s31/d31 for FP)
///   %W  force 32-bit register name for next
///   %L  force 64-bit register name for next
///   %S  force single-float register name for next
///   %D  force double-float register name for next
///   %M  memory addressing mode for next (0/1/=)
fn emitf(s: &str, i: &Ins, e: &mut E) {
    e.out.push('\t');

    let bytes = s.as_bytes();
    let mut pos = 0;
    let mut sp = false;
    let len = bytes.len();

    loop {
        let mut k = i.cls;
        // Consume non-escape characters
        while pos < len && bytes[pos] != b'%' {
            let c = bytes[pos] as char;
            pos += 1;
            if c == ' ' && !sp {
                e.out.push('\t');
                sp = true;
            } else {
                e.out.push(c);
            }
        }
        if pos >= len {
            e.out.push('\n');
            return;
        }
        pos += 1; // skip '%'

        // Process escapes
        loop {
            if pos >= len {
                e.out.push('\n');
                return;
            }
            let c = bytes[pos] as char;
            pos += 1;
            match c {
                'W' => {
                    k = Cls::Kw;
                    continue;
                }
                'L' => {
                    k = Cls::Kl;
                    continue;
                }
                'S' => {
                    k = Cls::Ks;
                    continue;
                }
                'D' => {
                    k = Cls::Kd;
                    continue;
                }
                '?' => {
                    if k.base() == 0 {
                        let _ = write!(e.out, "{}", rname(R18, k));
                    } else if k == Cls::Ks {
                        e.out.push_str("s31");
                    } else {
                        e.out.push_str("d31");
                    }
                }
                '=' | '0' => {
                    let r = if c == '=' { i.to } else { i.arg[0] };
                    assert!(isreg(r), "expected register, got {:?}", r);
                    let _ = write!(e.out, "{}", rname(r.val(), k));
                }
                '1' => {
                    let r = i.arg[1];
                    match r {
                        Ref::Tmp(_) => {
                            assert!(isreg(r));
                            let _ = write!(e.out, "{}", rname(r.val(), k));
                        }
                        Ref::Con(cid) => {
                            let pc = &e.f.cons[cid.0 as usize];
                            let n = pc.bits.i() as u64 as u32;
                            assert_eq!(pc.typ, ConType::Bits);
                            if n & 0xfff000 != 0 {
                                let _ = write!(e.out, "#{}, lsl #12", n >> 12);
                            } else {
                                let _ = write!(e.out, "#{n}");
                            }
                        }
                        _ => panic!("invalid second argument {:?}", r),
                    }
                }
                'M' => {
                    // Next char is 0, 1, or =
                    if pos >= len {
                        panic!("unexpected end of format");
                    }
                    let mc = bytes[pos] as char;
                    pos += 1;
                    let r = match mc {
                        '=' => i.to,
                        '0' => i.arg[0],
                        '1' => i.arg[1],
                        _ => panic!("bad memory escape"),
                    };
                    match r {
                        Ref::Tmp(_) => {
                            assert!(isreg(r));
                            let _ = write!(e.out, "[{}]", rname(r.val(), Cls::Kl));
                        }
                        Ref::Slot(_) => {
                            let s = slot_offset(r, e);
                            let _ = write!(e.out, "[x29, {s}]");
                        }
                        _ => panic!("unhandled ref in memory operand: {:?}", r),
                    }
                }
                _ => panic!("invalid format escape '%{c}'"),
            }
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Address loading
// ---------------------------------------------------------------------------

fn loadaddr(c: &Con, rn: &str, e: &mut E) {
    let sym_name = format!("sym{}", c.sym.id); // simplified symbol lookup
    let pfx = if sym_name.starts_with('"') {
        ""
    } else {
        e.assym
    };

    let template: &[&str] = match c.sym.typ {
        SymType::Glo => {
            if e.apple {
                &["\tadrp\tR, S@PAGE", "\tadd\tR, R, S@PAGEOFF"]
            } else {
                &["\tadrp\tR, S", "\tadd\tR, R, #:lo12:S"]
            }
        }
        SymType::Thr => {
            if e.apple {
                &["\tadrp\tR, S@TLVPPAGE", "\tldr\tR, [R, S@TLVPPAGEOFF]"]
            } else {
                &[
                    "\tmrs\tR, tpidr_el0",
                    "\tadd\tR, R, #:tprel_hi12:S, lsl #12",
                    "\tadd\tR, R, #:tprel_lo12_nc:S",
                ]
            }
        }
    };

    for line in template {
        for ch in line.chars() {
            match ch {
                'R' => {
                    e.out.push_str(rn);
                }
                'S' => {
                    e.out.push_str(pfx);
                    e.out.push_str(&sym_name);
                }
                _ => {
                    e.out.push(ch);
                }
            }
        }
        if c.bits.i() != 0 {
            let _ = write!(e.out, "+{}", c.bits.i());
        }
        e.out.push('\n');
    }
}

/// Load a constant into a register.
fn loadcon(c: &Con, r: u32, k: Cls, e: &mut E) {
    let w = k.is_wide();
    let rn = rname(r, k);
    let mut n = c.bits.i();

    if c.typ == ConType::Addr {
        loadaddr(c, &rn, e);
        return;
    }
    assert_eq!(c.typ, ConType::Bits);
    if !w {
        n = n as i32 as i64;
    }
    if (n | 0xffff) == -1 || logimm(n as u64, k) {
        let _ = writeln!(e.out, "\tmov\t{rn}, #{n}");
    } else {
        let _ = writeln!(e.out, "\tmov\t{rn}, #{}", n as u16 as i16);
        let mut remaining = n;
        let mut sh = 16;
        loop {
            remaining >>= 16;
            if remaining == 0 || (!w && sh == 32) || sh == 64 {
                break;
            }
            let chunk = (remaining & 0xffff) as u16;
            if chunk != 0 {
                let _ = writeln!(e.out, "\tmovk\t{rn}, #0x{chunk:x}, lsl #{sh}");
            }
            sh += 16;
        }
    }
}

// ---------------------------------------------------------------------------
// loadsz / storesz helpers
// ---------------------------------------------------------------------------

fn loadsz(i: &Ins) -> u32 {
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
        _ => 4,
    }
}

fn storesz(i: &Ins) -> u32 {
    match i.op {
        Op::Storeb => 1,
        Op::Storeh => 2,
        Op::Storew | Op::Stores => 4,
        Op::Storel | Op::Stored => 8,
        _ => 4,
    }
}

// ---------------------------------------------------------------------------
// Fix up large slot offsets
// ---------------------------------------------------------------------------

fn fixarg_emit(pr: &mut Ref, sz: u32, e: &mut E) {
    if let Ref::Slot(_) = *pr {
        let s = slot_offset(*pr, e);
        if s > sz as u64 * 4095 {
            // Emit address computation into IP0
            let addr_ins = Ins {
                op: Op::Addr,
                cls: Cls::Kl,
                to: Ref::Tmp(TmpId(IP0)),
                arg: [*pr, Ref::R],
            };
            emitins_inner(&addr_ins, e);
            *pr = Ref::Tmp(TmpId(IP0));
        }
    }
}

// ---------------------------------------------------------------------------
// Main instruction emitter
// ---------------------------------------------------------------------------

fn emitins_inner(i: &Ins, e: &mut E) {
    match i.op {
        Op::Nop => {}
        Op::Copy => {
            if i.to == i.arg[0] {
                return;
            }
            if let Ref::Slot(_) = i.to {
                // Store to slot
                let r = i.to;
                let mut arg0 = i.arg[0];
                if !isreg(arg0) {
                    // Move to scratch, then store
                    let scratch_ins = Ins {
                        op: Op::Copy,
                        cls: i.cls,
                        to: Ref::Tmp(TmpId(R18)),
                        arg: [arg0, Ref::R],
                    };
                    emitins_inner(&scratch_ins, e);
                    arg0 = Ref::Tmp(TmpId(R18));
                }
                let store_op_val = Op::Storew as u16 + i.cls as i8 as u16;
                let store_op = unsafe { std::mem::transmute::<u16, Op>(store_op_val) };
                let store_ins = Ins {
                    op: store_op,
                    cls: Cls::Kw,
                    to: Ref::R,
                    arg: [arg0, r],
                };
                emitins_inner(&store_ins, e);
                return;
            }
            assert!(isreg(i.to));
            match i.arg[0] {
                Ref::Con(cid) => {
                    let c = e.f.cons[cid.0 as usize];
                    loadcon(&c, i.to.val(), i.cls, e);
                    return;
                }
                Ref::Slot(_) => {
                    // Load from slot
                    let load_ins = Ins {
                        op: Op::Load,
                        cls: i.cls,
                        to: i.to,
                        arg: [i.arg[0], Ref::R],
                    };
                    emitins_inner(&load_ins, e);
                    return;
                }
                _ => {
                    assert!(i.to.val() != R18, "copy to R18 via table");
                    // Fall through to table lookup
                    emit_from_table(i, e);
                    return;
                }
            }
        }
        Op::Addr => {
            assert!(matches!(i.arg[0], Ref::Slot(_)));
            let rn = rname(i.to.val(), Cls::Kl);
            let s = slot_offset(i.arg[0], e);
            if s <= 4095 {
                let _ = writeln!(e.out, "\tadd\t{rn}, x29, #{s}");
            } else if s <= 65535 {
                let _ = writeln!(e.out, "\tmov\t{rn}, #{s}");
                let _ = writeln!(e.out, "\tadd\t{rn}, x29, {rn}");
            } else {
                let _ = writeln!(e.out, "\tmov\t{rn}, #{}", s & 0xFFFF);
                let _ = writeln!(e.out, "\tmovk\t{rn}, #{}, lsl #16", s >> 16);
                let _ = writeln!(e.out, "\tadd\t{rn}, x29, {rn}");
            }
        }
        Op::Call => {
            if let Ref::Con(cid) = i.arg[0] {
                let c = &e.f.cons[cid.0 as usize];
                if c.typ == ConType::Addr && c.sym.typ == SymType::Glo && c.bits.i() == 0 {
                    let sym_name = format!("sym{}", c.sym.id);
                    let pfx = if sym_name.starts_with('"') {
                        ""
                    } else {
                        e.assym
                    };
                    let _ = writeln!(e.out, "\tbl\t{pfx}{sym_name}");
                    return;
                }
            }
            emit_from_table(i, e);
        }
        Op::Salloc => {
            emitf("sub sp, sp, %0", i, e);
            if !i.to.is_none() {
                emitf("mov %=, sp", i, e);
            }
        }
        Op::Dbgloc => {
            crate::emit::emitdbgloc(i.arg[0].val(), i.arg[1].val(), e.out);
        }
        _ => {
            // Handle loads/stores with large offsets
            let mut ins_copy = *i;
            if i.op.is_load() {
                fixarg_emit(&mut ins_copy.arg[0], loadsz(i), e);
            }
            if i.op.is_store() {
                fixarg_emit(&mut ins_copy.arg[1], storesz(i), e);
            }
            emit_from_table(&ins_copy, e);
        }
    }
}

/// Look up instruction in omap[] and emit via format string.
fn emit_from_table(i: &Ins, e: &mut E) {
    let k = i.cls as i8;
    for entry in OMAP {
        if entry.op == i.op {
            if entry.cls == k || entry.cls == KA || (entry.cls == KI && i.cls.base() == 0) {
                emitf(entry.asm, i, e);
                return;
            }
        }
    }
    let cls_ch = match i.cls {
        Cls::Kw => 'w',
        Cls::Kl => 'l',
        Cls::Ks => 's',
        Cls::Kd => 'd',
        Cls::Kx => '?',
    };
    panic!(
        "no omap match for {:?}({cls_ch})",
        OP_TABLE[i.op as usize].name
    );
}

// ---------------------------------------------------------------------------
// Frame layout computation
// ---------------------------------------------------------------------------

fn framelayout(e: &mut E) {
    let mut o: u32 = 0;
    for &r in ARM64_RCLOB {
        if e.f.reg & bit(r as u32) != 0 {
            o += 1;
        }
    }
    let mut f = e.f.slot as u64;
    f = (f + 3) & !3;
    o += o & 1; // round up to even
    e.padding = 4 * (f - e.f.slot as u64);
    e.frame = 4 * f + 8 * o as u64;
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Emit ARM64 assembly for one function.
pub fn emitfn(f: &mut Fn, t: &Target, out: &mut String) {
    let apple = t.apple;
    let asloc = if apple { "L" } else { ".L" };
    let assym = if apple { "_" } else { "" };

    // On Apple, force function alignment to 4
    if apple {
        f.lnk.align = 4;
    }

    crate::emit::emitfnlnk_target(&f.name, &f.lnk, t, out);

    let mut e = E {
        out,
        f,
        frame: 0,
        padding: 0,
        apple,
        asloc,
        assym,
    };
    framelayout(&mut e);

    // Non-Apple vararg: save register save area
    if e.f.vararg && !apple {
        for n in (0..=7).rev() {
            let _ = writeln!(e.out, "\tstr\tq{n}, [sp, -16]!");
        }
        for n in (1..=7).rev().step_by(2) {
            let _ = writeln!(e.out, "\tstp\tx{}, x{n}, [sp, -16]!", n - 1);
        }
    }

    // Function prologue: save frame pointer and link register
    let frame = e.frame;
    if frame + 16 <= 512 {
        let _ = writeln!(e.out, "\tstp\tx29, x30, [sp, -{}]!", frame + 16);
    } else if frame <= 4095 {
        let _ = writeln!(e.out, "\tsub\tsp, sp, #{frame}");
        let _ = writeln!(e.out, "\tstp\tx29, x30, [sp, -16]!");
    } else if frame <= 65535 {
        let _ = writeln!(e.out, "\tmov\tx16, #{frame}");
        let _ = writeln!(e.out, "\tsub\tsp, sp, x16");
        let _ = writeln!(e.out, "\tstp\tx29, x30, [sp, -16]!");
    } else {
        let _ = writeln!(e.out, "\tmov\tx16, #{}", frame & 0xFFFF);
        let _ = writeln!(e.out, "\tmovk\tx16, #{}, lsl #16", frame >> 16);
        let _ = writeln!(e.out, "\tsub\tsp, sp, x16");
        let _ = writeln!(e.out, "\tstp\tx29, x30, [sp, -16]!");
    }
    let _ = writeln!(e.out, "\tmov\tx29, sp");

    // Save callee-save registers
    let padding = e.padding;
    let mut s = ((frame - padding) / 4) as i32;
    for &r in ARM64_RCLOB {
        if e.f.reg & bit(r as u32) != 0 {
            s -= 2;
            let store_op = if r as u32 >= V0 {
                Op::Stored
            } else {
                Op::Storel
            };
            let save_ins = Ins {
                op: store_op,
                cls: Cls::Kw,
                to: Ref::R,
                arg: [Ref::Tmp(TmpId(r as u32)), Ref::Slot(s as u32 & 0x1fff_ffff)],
            };
            emitins_inner(&save_ins, &mut e);
        }
    }

    // Use a static block counter for label uniqueness
    thread_local! {
        static ID0: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    let id0 = ID0.with(|c| c.get());

    // Emit basic blocks
    let rpo: Vec<BlkId> = e.f.rpo.clone();
    let nblk = rpo.len();
    let mut lbl = false;

    for (block_idx, &bid) in rpo.iter().enumerate() {
        let blk = &e.f.blks[bid.0 as usize];

        if lbl || blk.pred.len() > 1 {
            let _ = writeln!(e.out, "{}{}:", e.asloc, id0 + blk.id);
        }

        // Emit instructions
        for i in &blk.ins {
            emitins_inner(i, &mut e);
        }

        lbl = true;
        match blk.jmp.typ {
            Jmp::Hlt => {
                let _ = writeln!(e.out, "\tbrk\t#1000");
            }
            Jmp::Ret0 => {
                // Epilogue: restore callee-saves
                let mut s2 = ((e.frame - e.padding) / 4) as i32;
                for &r in ARM64_RCLOB {
                    if e.f.reg & bit(r as u32) != 0 {
                        s2 -= 2;
                        let load_cls = if r as u32 >= V0 { Cls::Kd } else { Cls::Kl };
                        let restore_ins = Ins {
                            op: Op::Load,
                            cls: load_cls,
                            to: Ref::Tmp(TmpId(r as u32)),
                            arg: [Ref::Slot(s2 as u32 & 0x1fff_ffff), Ref::R],
                        };
                        emitins_inner(&restore_ins, &mut e);
                    }
                }
                if e.f.dynalloc {
                    let _ = writeln!(e.out, "\tmov sp, x29");
                }
                let mut o = e.frame + 16;
                if e.f.vararg && !e.apple {
                    o += 192;
                }
                if o <= 504 {
                    let _ = writeln!(e.out, "\tldp\tx29, x30, [sp], {o}");
                } else if o - 16 <= 4095 {
                    let _ = writeln!(e.out, "\tldp\tx29, x30, [sp], 16");
                    let _ = writeln!(e.out, "\tadd\tsp, sp, #{}", o - 16);
                } else if o - 16 <= 65535 {
                    let _ = writeln!(e.out, "\tldp\tx29, x30, [sp], 16");
                    let _ = writeln!(e.out, "\tmov\tx16, #{}", o - 16);
                    let _ = writeln!(e.out, "\tadd\tsp, sp, x16");
                } else {
                    let _ = writeln!(e.out, "\tldp\tx29, x30, [sp], 16");
                    let _ = writeln!(e.out, "\tmov\tx16, #{}", (o - 16) & 0xFFFF);
                    let _ = writeln!(e.out, "\tmovk\tx16, #{}, lsl #16", (o - 16) >> 16);
                    let _ = writeln!(e.out, "\tadd\tsp, sp, x16");
                }
                let _ = writeln!(e.out, "\tret");
            }
            Jmp::Jmp_ => {
                // Unconditional jump
                if let Some(s1) = blk.s1 {
                    let next_bid = if block_idx + 1 < nblk {
                        Some(rpo[block_idx + 1])
                    } else {
                        None
                    };
                    if Some(s1) != next_bid {
                        let _ = writeln!(
                            e.out,
                            "\tb\t{}{}",
                            e.asloc,
                            id0 + e.f.blks[s1.0 as usize].id
                        );
                    } else {
                        lbl = false;
                    }
                }
            }
            _ => {
                // Conditional branch
                let c = blk.jmp.typ as u16 - Jmp::Jfieq as u16;
                if c > N_CMP as u16 {
                    panic!("unhandled jump {:?}", blk.jmp.typ);
                }

                let (s1, s2_blk) = (blk.s1, blk.s2);
                let next_bid = if block_idx + 1 < nblk {
                    Some(rpo[block_idx + 1])
                } else {
                    None
                };

                let (cc, target_bid) = if s2_blk == next_bid {
                    // s2 is fallthrough, branch to s1 on condition
                    // But we need to negate the condition since
                    // the convention is: b.cc to s2, fallthrough to s1
                    // Actually: In QBE, b.cc jumps to s2, fall to s1.
                    // If s2 == next (link), swap: negate condition, branch to s2's "other".
                    // Wait: looking at C code: if link == s2 { swap s1/s2 } else { c = cmpneg(c) }
                    // Then b.c to s2, fall to s1 (via goto Jmp which checks s1 != link).
                    (c, s2_blk)
                } else {
                    let neg_c = cmpneg(c);
                    (neg_c, s2_blk)
                };

                if let Some(tb) = target_bid {
                    let cond_str = CTOA.get(cc as usize).unwrap_or(&"??");
                    let _ = writeln!(
                        e.out,
                        "\tb{cond_str}\t{}{}",
                        e.asloc,
                        id0 + e.f.blks[tb.0 as usize].id
                    );
                }
                // Fallthrough or unconditional jump to s1
                if let Some(s1) = s1 {
                    if Some(s1) != next_bid {
                        let _ = writeln!(
                            e.out,
                            "\tb\t{}{}",
                            e.asloc,
                            id0 + e.f.blks[s1.0 as usize].id
                        );
                    } else {
                        lbl = false;
                    }
                }
            }
        }
    }

    // Update label counter
    ID0.with(|c| c.set(id0 + nblk as u32));

    // ELF function footer
    if !apple {
        crate::emit::elf_emitfnfin(&f.name, &mut e.out);
    }
}
