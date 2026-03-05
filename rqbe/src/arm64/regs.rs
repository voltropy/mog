// ARM64 register constants and target definitions.
// Port of QBE's arm64/targ.c + register definitions from arm64/all.h.

use crate::ir::Target;

// ---------------------------------------------------------------------------
// Physical register indices  (mirrors the C `enum { RXX, R0, … }`)
// ---------------------------------------------------------------------------

/// RXX — the null physical register (index 0 — never used as a real reg).
pub const RXX: u32 = 0;

// General-purpose registers: R0–R30, FP, LR, SP
pub const R0: u32 = 1;
pub const R1: u32 = 2;
pub const R2: u32 = 3;
pub const R3: u32 = 4;
pub const R4: u32 = 5;
pub const R5: u32 = 6;
pub const R6: u32 = 7;
pub const R7: u32 = 8;
pub const R8: u32 = 9;
pub const R9: u32 = 10;
pub const R10: u32 = 11;
pub const R11: u32 = 12;
pub const R12: u32 = 13;
pub const R13: u32 = 14;
pub const R14: u32 = 15;
pub const R15: u32 = 16;
pub const IP0: u32 = 17; // x16
pub const IP1: u32 = 18; // x17
pub const R18: u32 = 19; // platform register
pub const R19: u32 = 20;
pub const R20: u32 = 21;
pub const R21: u32 = 22;
pub const R22: u32 = 23;
pub const R23: u32 = 24;
pub const R24: u32 = 25;
pub const R25: u32 = 26;
pub const R26: u32 = 27;
pub const R27: u32 = 28;
pub const R28: u32 = 29;
pub const FP: u32 = 30;
pub const LR: u32 = 31;
pub const SP: u32 = 32;

// SIMD/FP registers: V0–V30  (V31 reserved as scratch)
pub const V0: u32 = 33;
pub const V1: u32 = 34;
pub const V2: u32 = 35;
pub const V3: u32 = 36;
pub const V4: u32 = 37;
pub const V5: u32 = 38;
pub const V6: u32 = 39;
pub const V7: u32 = 40;
pub const V8: u32 = 41;
pub const V9: u32 = 42;
pub const V10: u32 = 43;
pub const V11: u32 = 44;
pub const V12: u32 = 45;
pub const V13: u32 = 46;
pub const V14: u32 = 47;
pub const V15: u32 = 48;
pub const V16: u32 = 49;
pub const V17: u32 = 50;
pub const V18: u32 = 51;
pub const V19: u32 = 52;
pub const V20: u32 = 53;
pub const V21: u32 = 54;
pub const V22: u32 = 55;
pub const V23: u32 = 56;
pub const V24: u32 = 57;
pub const V25: u32 = 58;
pub const V26: u32 = 59;
pub const V27: u32 = 60;
pub const V28: u32 = 61;
pub const V29: u32 = 62;
pub const V30: u32 = 63;

// ---------------------------------------------------------------------------
// Derived counts  (match C QBE's NGPR, NFPR, NGPS, NFPS, NCLR)
// ---------------------------------------------------------------------------

/// Total GP registers (R0..SP inclusive).
pub const NGPR: i32 = (SP - R0 + 1) as i32; // 32

/// Total FP registers (V0..V30 inclusive).
pub const NFPR: i32 = (V30 - V0 + 1) as i32; // 31

/// GP caller-save count: R0..R18 (19) + LR (1) = 20.
pub const NGPS: i32 = (R18 - R0 + 1 + 1) as i32; // 20

/// FP caller-save count: V0..V7 (8) + V16..V30 (15) = 23.
pub const NFPS: i32 = ((V7 - V0 + 1) + (V30 - V16 + 1)) as i32; // 23

/// Callee-save count: R19..R28 (10) + V8..V15 (8) = 18.
pub const NCLR: i32 = ((R28 - R19 + 1) + (V15 - V8 + 1)) as i32; // 18

// ---------------------------------------------------------------------------
// Register bitmask helpers
// ---------------------------------------------------------------------------

#[inline]
pub const fn bit(n: u32) -> u64 {
    1u64 << n
}

/// Globally-live registers that the allocator must not touch.
pub const RGLOB: u64 = bit(FP) | bit(SP) | bit(R18);

// ---------------------------------------------------------------------------
// Caller-save / callee-save register lists  (match C QBE's arm64_rsave / arm64_rclob)
// ---------------------------------------------------------------------------

/// Caller-save registers (registers we may clobber in a call).
/// Terminates with -1 sentinel in C; we just use the slice length.
pub static ARM64_RSAVE: &[i32] = &[
    R0 as i32, R1 as i32, R2 as i32, R3 as i32, R4 as i32, R5 as i32, R6 as i32, R7 as i32,
    R8 as i32, R9 as i32, R10 as i32, R11 as i32, R12 as i32, R13 as i32, R14 as i32, R15 as i32,
    IP0 as i32, IP1 as i32, R18 as i32, LR as i32, V0 as i32, V1 as i32, V2 as i32, V3 as i32,
    V4 as i32, V5 as i32, V6 as i32, V7 as i32, V16 as i32, V17 as i32, V18 as i32, V19 as i32,
    V20 as i32, V21 as i32, V22 as i32, V23 as i32, V24 as i32, V25 as i32, V26 as i32, V27 as i32,
    V28 as i32, V29 as i32, V30 as i32,
];

/// Callee-save registers (we must save/restore these if used).
pub static ARM64_RCLOB: &[i32] = &[
    R19 as i32, R20 as i32, R21 as i32, R22 as i32, R23 as i32, R24 as i32, R25 as i32, R26 as i32,
    R27 as i32, R28 as i32, V8 as i32, V9 as i32, V10 as i32, V11 as i32, V12 as i32, V13 as i32,
    V14 as i32, V15 as i32,
];

// GP argument registers for AAPCS64.
pub static GP_REG: [i32; 8] = [
    R0 as i32, R1 as i32, R2 as i32, R3 as i32, R4 as i32, R5 as i32, R6 as i32, R7 as i32,
];

// FP argument registers for AAPCS64.
pub static FP_REG: [i32; 8] = [
    V0 as i32, V1 as i32, V2 as i32, V3 as i32, V4 as i32, V5 as i32, V6 as i32, V7 as i32,
];

// ---------------------------------------------------------------------------
// Target definitions  (port of targ.c)
// ---------------------------------------------------------------------------

/// ELF ARM64 target.
pub static T_ARM64: Target = Target {
    name: "arm64",
    apple: false,
    gpr0: R0 as i32,
    ngpr: NGPR,
    fpr0: V0 as i32,
    nfpr: NFPR,
    rglob: RGLOB,
    nrglob: 3,
    rsave: ARM64_RSAVE,
    nrsave: [NGPS, NFPS],
};

/// Apple ARM64 target (Mach-O).
pub static T_ARM64_APPLE: Target = Target {
    name: "arm64_apple",
    apple: true,
    gpr0: R0 as i32,
    ngpr: NGPR,
    fpr0: V0 as i32,
    nfpr: NFPR,
    rglob: RGLOB,
    nrglob: 3,
    rsave: ARM64_RSAVE,
    nrsave: [NGPS, NFPS],
};

/// `arm64_memargs()` — ARM64 never folds memory addressing into instruction operands.
pub fn memargs(_op: u16) -> i32 {
    0
}
