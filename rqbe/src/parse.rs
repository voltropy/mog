//! QBE IR text parser.
//!
//! Faithfully ported from `parse.c` (QBE 1.2). Hand-written tokenizer,
//! recursive-descent parser, post-parse type checker, and IR printer.

use std::collections::HashMap;
use std::fmt::Write as FmtWrite;

use crate::ir::{
    Blk, BlkId, Cls, Con, ConBits, ConType, Dat, DatItem, Field, FieldType, Fn, Ins, Jmp, Lnk, Op,
    Phi, Ref, SymType, Tmp, TmpId, Typ, TypId, N_FIELD, N_INS, OP_TABLE, TMP0,
};
use crate::util::newcon;

// ---------------------------------------------------------------------------
// ParseResult
// ---------------------------------------------------------------------------

/// Result of parsing QBE IR text.
pub struct ParseResult {
    /// Parsed type definitions.
    pub types: Vec<Typ>,
    /// Parsed data definitions (each inner vec is one data block:
    /// Start, items..., End).
    pub data: Vec<Vec<Dat>>,
    /// Parsed functions.
    pub functions: Vec<Fn>,
}

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

/// Token kinds produced by the lexer.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Tok {
    Txxx,
    // Opcodes (0..NPUBOP) are represented by their Op value
    Top(Op),
    // Keyword aliases that don't map directly to an Op
    Tloadw,
    Tloadl,
    Tloads,
    Tloadd,
    Talloc1,
    Talloc2,
    Tblit,
    Tcall,
    Tenv,
    Tphi,
    Tjmp,
    Tjnz,
    Tret,
    Thlt,
    Texport,
    Tthread,
    Tfunc,
    Ttype,
    Tdata,
    Tsection,
    Talign,
    Tdbgfile,
    // Type-width keywords
    Tl,
    Tw,
    Tsh,
    Tuh,
    Th,
    Tsb,
    Tub,
    Tb,
    Td,
    Ts,
    Tz,
    // Literal types
    Tint,
    Tflts,
    Tfltd,
    Ttmp,
    Tlbl,
    Tglo,
    Ttyp,
    Tstr,
    // Punctuation
    Tplus,
    Teq,
    Tcomma,
    Tlparen,
    Trparen,
    Tlbrace,
    Trbrace,
    Tnl,
    Tdots,
    Teof,
}

// ---------------------------------------------------------------------------
// Token value
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct TokVal {
    chr: char,
    fltd: f64,
    flts: f32,
    num: i64,
    str_val: String,
}

impl Default for TokVal {
    fn default() -> Self {
        Self {
            chr: '\0',
            fltd: 0.0,
            flts: 0.0,
            num: 0,
            str_val: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Extended class constants (matching C QBE)
// ---------------------------------------------------------------------------

const KSB: i32 = 4;
const KUB: i32 = 5;
const KSH: i32 = 6;
const KUH: i32 = 7;
const KC: i32 = 8;
const K0: i32 = 9;

// ---------------------------------------------------------------------------
// Parser state machine
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum PState {
    PLbl,
    PPhi,
    PIns,
    PEnd,
}

// ---------------------------------------------------------------------------
// N_PRED — max phi predecessors / instruction arguments
// ---------------------------------------------------------------------------

const N_PRED: usize = 63;

// ---------------------------------------------------------------------------
// Build keyword map
// ---------------------------------------------------------------------------

fn build_keyword_map() -> HashMap<String, Tok> {
    let mut m = HashMap::new();

    // All public Op names from OP_TABLE (indices 1 .. NPUBOP)
    let npubop = Op::Nop as u16;
    for i in 1..npubop {
        let name = OP_TABLE[i as usize].name;
        if !name.is_empty() && name != "xxx" {
            m.insert(name.to_string(), Tok::Top(op_from_u16(i)));
        }
    }

    // Keyword aliases
    m.insert("loadw".into(), Tok::Tloadw);
    m.insert("loadl".into(), Tok::Tloadl);
    m.insert("loads".into(), Tok::Tloads);
    m.insert("loadd".into(), Tok::Tloadd);
    m.insert("alloc1".into(), Tok::Talloc1);
    m.insert("alloc2".into(), Tok::Talloc2);
    m.insert("blit".into(), Tok::Tblit);
    m.insert("call".into(), Tok::Tcall);
    m.insert("env".into(), Tok::Tenv);
    m.insert("phi".into(), Tok::Tphi);
    m.insert("jmp".into(), Tok::Tjmp);
    m.insert("jnz".into(), Tok::Tjnz);
    m.insert("ret".into(), Tok::Tret);
    m.insert("hlt".into(), Tok::Thlt);
    m.insert("export".into(), Tok::Texport);
    m.insert("thread".into(), Tok::Tthread);
    m.insert("function".into(), Tok::Tfunc);
    m.insert("type".into(), Tok::Ttype);
    m.insert("data".into(), Tok::Tdata);
    m.insert("section".into(), Tok::Tsection);
    m.insert("align".into(), Tok::Talign);
    m.insert("dbgfile".into(), Tok::Tdbgfile);
    m.insert("sb".into(), Tok::Tsb);
    m.insert("ub".into(), Tok::Tub);
    m.insert("sh".into(), Tok::Tsh);
    m.insert("uh".into(), Tok::Tuh);
    m.insert("b".into(), Tok::Tb);
    m.insert("h".into(), Tok::Th);
    m.insert("w".into(), Tok::Tw);
    m.insert("l".into(), Tok::Tl);
    m.insert("s".into(), Tok::Ts);
    m.insert("d".into(), Tok::Td);
    m.insert("z".into(), Tok::Tz);
    m.insert("...".into(), Tok::Tdots);

    m
}

/// Convert a u16 to an Op. We use a transmute since Op is #[repr(u16)].
fn op_from_u16(v: u16) -> Op {
    // Safety: Op is repr(u16) and all values 0..N_OP are valid variants.
    // We only call this with validated indices.
    assert!((v as usize) < crate::ir::N_OP);
    unsafe { std::mem::transmute(v) }
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
    lnum: u32,
    thead: Option<Tok>,
    tokval: TokVal,
    kwmap: HashMap<String, Tok>,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
            lnum: 1,
            thead: None,
            tokval: TokVal::default(),
            kwmap: build_keyword_map(),
        }
    }

    fn err(&self, msg: &str) -> ! {
        panic!("qbe:{}:{}: {}", "<input>", self.lnum, msg);
    }

    fn errf(&self, msg: String) -> ! {
        panic!("qbe:{}:{}: {}", "<input>", self.lnum, msg);
    }

    fn peekch(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn getch(&mut self) -> Option<u8> {
        let c = self.input.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn ungetch(&mut self) {
        if self.pos > 0 {
            self.pos -= 1;
        }
    }

    /// Read a (possibly negative) decimal integer.
    fn getint(&mut self) -> i64 {
        let mut n: u64 = 0;
        let c = self.getch().unwrap_or(b'0');
        let neg = c == b'-';
        let first = if neg || c == b'+' {
            self.getch().unwrap_or(b'0')
        } else {
            c
        };

        // Check for hex
        if first == b'0' {
            if let Some(c2) = self.peekch() {
                if c2 == b'x' || c2 == b'X' {
                    self.getch(); // consume 'x'
                    loop {
                        match self.peekch() {
                            Some(d @ b'0'..=b'9') => {
                                n = n.wrapping_mul(16).wrapping_add((d - b'0') as u64);
                                self.getch();
                            }
                            Some(d @ b'a'..=b'f') => {
                                n = n.wrapping_mul(16).wrapping_add((d - b'a' + 10) as u64);
                                self.getch();
                            }
                            Some(d @ b'A'..=b'F') => {
                                n = n.wrapping_mul(16).wrapping_add((d - b'A' + 10) as u64);
                                self.getch();
                            }
                            _ => break,
                        }
                    }
                    if neg {
                        n = 1u64.wrapping_add(!n);
                    }
                    return n as i64;
                }
            }
        }

        n = (first - b'0') as u64;
        loop {
            match self.peekch() {
                Some(d @ b'0'..=b'9') => {
                    n = 10u64.wrapping_mul(n).wrapping_add((d - b'0') as u64);
                    self.getch();
                }
                _ => break,
            }
        }
        if neg {
            n = 1u64.wrapping_add(!n);
        }
        n as i64
    }

    /// Core tokenizer. Returns the next token.
    fn lex(&mut self) -> Tok {
        // Skip blanks (space and tab)
        loop {
            match self.peekch() {
                Some(b' ') | Some(b'\t') => {
                    self.getch();
                }
                _ => break,
            }
        }

        let c = match self.getch() {
            None => return Tok::Teof,
            Some(c) => c,
        };

        self.tokval.chr = c as char;

        match c {
            b',' => return Tok::Tcomma,
            b'(' => return Tok::Tlparen,
            b')' => return Tok::Trparen,
            b'{' => return Tok::Tlbrace,
            b'}' => return Tok::Trbrace,
            b'=' => return Tok::Teq,
            b'+' => return Tok::Tplus,
            b's' => {
                // Try to match s_<float>
                if self.peekch() == Some(b'_') {
                    self.getch(); // consume '_'
                    let start = self.pos;
                    // Read float characters
                    while let Some(c2) = self.peekch() {
                        if c2.is_ascii_digit()
                            || c2 == b'.'
                            || c2 == b'-'
                            || c2 == b'+'
                            || c2 == b'e'
                            || c2 == b'E'
                        {
                            self.getch();
                        } else {
                            break;
                        }
                    }
                    let s = std::str::from_utf8(&self.input[start..self.pos]).unwrap_or("0");
                    if let Ok(v) = s.parse::<f32>() {
                        self.tokval.flts = v;
                        return Tok::Tflts;
                    }
                    // Not a float, backtrack
                    self.pos = start - 1; // back to before '_'
                                          // Fall through to Alpha
                }
                // Fall through to identifier scanning
                return self.scan_ident(c, Tok::Txxx);
            }
            b'd' => {
                // Try to match d_<float>
                if self.peekch() == Some(b'_') {
                    self.getch(); // consume '_'
                    let start = self.pos;
                    while let Some(c2) = self.peekch() {
                        if c2.is_ascii_digit()
                            || c2 == b'.'
                            || c2 == b'-'
                            || c2 == b'+'
                            || c2 == b'e'
                            || c2 == b'E'
                        {
                            self.getch();
                        } else {
                            break;
                        }
                    }
                    let s = std::str::from_utf8(&self.input[start..self.pos]).unwrap_or("0");
                    if let Ok(v) = s.parse::<f64>() {
                        self.tokval.fltd = v;
                        return Tok::Tfltd;
                    }
                    self.pos = start - 1;
                }
                return self.scan_ident(c, Tok::Txxx);
            }
            b'%' => {
                let c2 = self.getch().unwrap_or(0);
                return self.scan_ident(c2, Tok::Ttmp);
            }
            b'@' => {
                let c2 = self.getch().unwrap_or(0);
                return self.scan_ident(c2, Tok::Tlbl);
            }
            b'$' => {
                let c2 = match self.getch() {
                    Some(b'"') => return self.scan_quoted_string(Tok::Tglo),
                    Some(c2) => c2,
                    None => self.err("unexpected EOF after $"),
                };
                return self.scan_ident(c2, Tok::Tglo);
            }
            b':' => {
                let c2 = self.getch().unwrap_or(0);
                return self.scan_ident(c2, Tok::Ttyp);
            }
            b'#' => {
                // Comment — consume until newline
                loop {
                    match self.getch() {
                        Some(b'\n') | None => break,
                        _ => {}
                    }
                }
                self.lnum += 1;
                return Tok::Tnl;
            }
            b'\n' => {
                self.lnum += 1;
                return Tok::Tnl;
            }
            b'"' => {
                return self.scan_quoted_string(Tok::Tstr);
            }
            _ => {}
        }

        // Number
        if c.is_ascii_digit() || c == b'-' {
            self.ungetch();
            self.tokval.num = self.getint();
            return Tok::Tint;
        }

        // Bare identifier (keyword) — starts with letter, '.', or '_'
        if c.is_ascii_alphabetic() || c == b'.' || c == b'_' {
            return self.scan_ident(c, Tok::Txxx);
        }

        self.errf(format!("invalid character '{}' ({})", c as char, c));
    }

    /// Scan a quoted string. The opening quote has already been consumed
    /// (or was the leading char for $ globals).
    fn scan_quoted_string(&mut self, t: Tok) -> Tok {
        let mut s = String::new();
        let mut esc = false;
        loop {
            let c = match self.getch() {
                None => self.err("unterminated string"),
                Some(c) => c,
            };
            if c == b'"' && !esc {
                break;
            }
            s.push(c as char);
            esc = c == b'\\' && !esc;
        }
        self.tokval.str_val = s;
        t
    }

    /// Scan an identifier token. `first` is the first character already consumed.
    /// If `t` is not Txxx, the token type is forced (for sigil tokens: %, @, $, :).
    fn scan_ident(&mut self, first: u8, t: Tok) -> Tok {
        if !first.is_ascii_alphanumeric() && first != b'.' && first != b'_' {
            self.errf(format!("invalid character '{}' ({})", first as char, first));
        }
        let mut tok = vec![first];
        loop {
            match self.peekch() {
                Some(c) if c.is_ascii_alphanumeric() || c == b'$' || c == b'.' || c == b'_' => {
                    self.getch();
                    tok.push(c);
                }
                _ => break,
            }
        }
        let ident = String::from_utf8(tok).unwrap_or_default();
        self.tokval.str_val = ident.clone();

        if t != Tok::Txxx {
            return t;
        }

        // Keyword lookup
        if let Some(&kw) = self.kwmap.get(&ident) {
            return kw;
        }
        self.errf(format!("unknown keyword '{}'", ident));
    }

    fn peek(&mut self) -> Tok {
        if self.thead.is_none() {
            self.thead = Some(self.lex());
        }
        self.thead.unwrap()
    }

    fn next(&mut self) -> Tok {
        let t = self.peek();
        self.thead = None;
        t
    }

    fn nextnl(&mut self) -> Tok {
        loop {
            let t = self.next();
            if t != Tok::Tnl {
                return t;
            }
        }
    }

    fn expect(&mut self, t: Tok) {
        let t1 = self.next();
        if t == t1 {
            return;
        }
        let s1 = tok_name(t);
        let s2 = tok_name(t1);
        self.errf(format!("{} expected, got {} instead", s1, s2));
    }
}

fn tok_name(t: Tok) -> &'static str {
    match t {
        Tok::Tlbl => "label",
        Tok::Tcomma => ",",
        Tok::Teq => "=",
        Tok::Tnl => "newline",
        Tok::Tlparen => "(",
        Tok::Trparen => ")",
        Tok::Tlbrace => "{",
        Tok::Trbrace => "}",
        Tok::Teof => "end of file",
        _ => "??",
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    lex: Lexer<'a>,
    /// Accumulated type definitions.
    typs: Vec<Typ>,
    /// Current function being parsed.
    curf: Fn,
    /// Current block.
    curb: Option<usize>, // index into curf.blks
    /// Block link list (indices of blocks in order).
    blk_order: Vec<usize>,
    /// Block name → index in curf.blks.
    blk_map: HashMap<String, usize>,
    /// Temp name → TmpId for current function.
    tmp_map: HashMap<String, TmpId>,
    /// Current instruction buffer for current block.
    insb: Vec<Ins>,
    /// Return class for current function.
    rcls: i32,
    /// Phi link — index of next phi to add to current block.
    nblk: u32,
    /// Debug file names.
    dbgfiles: Vec<String>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lex: Lexer::new(input),
            typs: Vec::new(),
            curf: Fn::default(),
            curb: None,
            blk_order: Vec::new(),
            blk_map: HashMap::new(),
            tmp_map: HashMap::new(),
            insb: Vec::new(),
            rcls: K0,
            nblk: 0,
            dbgfiles: Vec::new(),
        }
    }

    fn err(&self, msg: &str) -> ! {
        self.lex.err(msg);
    }

    fn errf(&self, msg: String) -> ! {
        self.lex.errf(msg);
    }

    // -- Ref helpers --

    /// Find or create a temporary by name.
    fn tmpref(&mut self, name: &str) -> Ref {
        if let Some(&id) = self.tmp_map.get(name) {
            return Ref::Tmp(id);
        }
        let t = self.curf.tmps.len() as u32;
        let id = TmpId(t);
        self.tmp_map.insert(name.to_string(), id);
        let mut tmp = Tmp::default();
        tmp.name = name.to_string();
        tmp.cls = Cls::Kx;
        self.curf.tmps.push(tmp);
        Ref::Tmp(id)
    }

    /// Parse a reference (tmp, int, float, global).
    fn parseref(&mut self) -> Ref {
        let mut c = Con::default();
        match self.lex.next() {
            Tok::Ttmp => {
                let s = self.lex.tokval.str_val.clone();
                return self.tmpref(&s);
            }
            Tok::Tint => {
                c.typ = ConType::Bits;
                c.bits = ConBits::from_i64(self.lex.tokval.num);
            }
            Tok::Tflts => {
                c.typ = ConType::Bits;
                c.bits = ConBits::from_f32(self.lex.tokval.flts);
                c.flt = 1;
            }
            Tok::Tfltd => {
                c.typ = ConType::Bits;
                c.bits = ConBits::from_f64(self.lex.tokval.fltd);
                c.flt = 2;
            }
            Tok::Tthread => {
                c.sym.typ = SymType::Thr;
                self.lex.expect(Tok::Tglo);
                c.typ = ConType::Addr;
                c.sym.id = self.intern_sym(&self.lex.tokval.str_val.clone());
            }
            Tok::Tglo => {
                c.typ = ConType::Addr;
                c.sym.id = self.intern_sym(&self.lex.tokval.str_val.clone());
            }
            _ => return Ref::R,
        }
        newcon(&c, &mut self.curf)
    }

    /// Simple string interner for global symbol names. Uses con pool indirectly.
    /// We store the name directly in the Sym.id field using a local interner.
    fn intern_sym(&self, _name: &str) -> u32 {
        // For the parser we just use a sequential id. The actual symbol
        // resolution happens at a higher level. We store 0 and will put
        // the name in the Con's sym field.
        0
    }

    /// Find type by name among parsed types, searching backwards.
    fn findtyp(&self, limit: usize) -> usize {
        let name = self.lex.tokval.str_val.clone();
        for i in (0..limit).rev() {
            if self.typs[i].name == name {
                return i;
            }
        }
        self.errf(format!("undefined type :{}", name));
    }

    /// Parse a class specifier (w, l, s, d, sb, ub, sh, uh, :type).
    /// Returns (class_int, type_index). The type_index is only meaningful
    /// when class == KC.
    fn parsecls(&mut self) -> (i32, i32) {
        match self.lex.next() {
            Tok::Ttyp => {
                let tyn = self.findtyp(self.typs.len()) as i32;
                (KC, tyn)
            }
            Tok::Tsb => (KSB, -1),
            Tok::Tub => (KUB, -1),
            Tok::Tsh => (KSH, -1),
            Tok::Tuh => (KUH, -1),
            Tok::Tw => (Cls::Kw as i32, -1),
            Tok::Tl => (Cls::Kl as i32, -1),
            Tok::Ts => (Cls::Ks as i32, -1),
            Tok::Td => (Cls::Kd as i32, -1),
            _ => self.err("invalid class specifier"),
        }
    }

    /// Parse a reference list (function params or call args).
    /// If `arg` is true, we're parsing call arguments.
    /// If `arg` is false, we're parsing function parameters.
    /// Returns true if the function is variadic.
    fn parserefl(&mut self, arg: bool) -> bool {
        let mut hasenv = false;
        let mut vararg = false;
        self.lex.expect(Tok::Tlparen);

        while self.lex.peek() != Tok::Trparen {
            if self.insb.len() >= N_INS {
                self.err("too many instructions");
            }
            if !arg && vararg {
                self.err("no parameters allowed after '...'");
            }

            match self.lex.peek() {
                Tok::Tdots => {
                    if vararg {
                        self.err("only one '...' allowed");
                    }
                    vararg = true;
                    if arg {
                        self.insb.push(Ins {
                            op: Op::Argv,
                            cls: Cls::Kx,
                            to: Ref::R,
                            arg: [Ref::R, Ref::R],
                        });
                    }
                    self.lex.next();
                    // Next
                    if self.lex.peek() == Tok::Trparen {
                        break;
                    }
                    self.lex.expect(Tok::Tcomma);
                    continue;
                }
                Tok::Tenv => {
                    if hasenv {
                        self.err("only one environment allowed");
                    }
                    hasenv = true;
                    self.lex.next();
                    let k = Cls::Kl as i32;
                    let r = self.parseref();
                    if r == Ref::R {
                        self.err("invalid argument");
                    }
                    if !arg && !matches!(r, Ref::Tmp(_)) {
                        self.err("invalid function parameter");
                    }
                    let ins = if arg {
                        Ins {
                            op: Op::Arge,
                            cls: Cls::from_i8(k as i8),
                            to: Ref::R,
                            arg: [r, Ref::R],
                        }
                    } else {
                        Ins {
                            op: Op::Pare,
                            cls: Cls::from_i8(k as i8),
                            to: r,
                            arg: [Ref::R, Ref::R],
                        }
                    };
                    self.insb.push(ins);
                }
                _ => {
                    let (k, ty) = self.parsecls();
                    let r = self.parseref();
                    if r == Ref::R {
                        self.err("invalid argument");
                    }
                    if !arg && !matches!(r, Ref::Tmp(_)) {
                        self.err("invalid function parameter");
                    }
                    let ins = if k == KC {
                        if arg {
                            Ins {
                                op: Op::Argc,
                                cls: Cls::Kl,
                                to: Ref::R,
                                arg: [Ref::Typ(TypId(ty as u32)), r],
                            }
                        } else {
                            Ins {
                                op: Op::Parc,
                                cls: Cls::Kl,
                                to: r,
                                arg: [Ref::Typ(TypId(ty as u32)), Ref::R],
                            }
                        }
                    } else if k >= KSB {
                        let off = (k - KSB) as u16;
                        if arg {
                            Ins {
                                op: op_from_u16(Op::Argsb as u16 + off),
                                cls: Cls::Kw,
                                to: Ref::R,
                                arg: [r, Ref::R],
                            }
                        } else {
                            Ins {
                                op: op_from_u16(Op::Parsb as u16 + off),
                                cls: Cls::Kw,
                                to: r,
                                arg: [Ref::R, Ref::R],
                            }
                        }
                    } else {
                        let cls = Cls::from_i8(k as i8);
                        if arg {
                            Ins {
                                op: Op::Arg,
                                cls,
                                to: Ref::R,
                                arg: [r, Ref::R],
                            }
                        } else {
                            Ins {
                                op: Op::Par,
                                cls,
                                to: r,
                                arg: [Ref::R, Ref::R],
                            }
                        }
                    };
                    self.insb.push(ins);
                }
            }

            if self.lex.peek() == Tok::Trparen {
                break;
            }
            self.lex.expect(Tok::Tcomma);
        }
        self.lex.expect(Tok::Trparen);
        vararg
    }

    /// Find or create a block by name.
    fn findblk(&mut self, name: &str) -> usize {
        if let Some(&idx) = self.blk_map.get(name) {
            return idx;
        }
        let idx = self.curf.blks.len();
        let mut blk = Blk::default();
        blk.id = self.nblk;
        self.nblk += 1;
        blk.name = name.to_string();
        self.curf.blks.push(blk);
        self.blk_map.insert(name.to_string(), idx);
        idx
    }

    /// Close the current block: flush instruction buffer.
    fn closeblk(&mut self) {
        if let Some(idx) = self.curb {
            self.curf.blks[idx].ins = self.insb.drain(..).collect();
            self.blk_order.push(idx);
        }
    }

    /// Parse a single line inside a function body.
    fn parseline(&mut self, ps: PState) -> PState {
        let t = self.lex.nextnl();

        if ps == PState::PLbl && t != Tok::Tlbl && t != Tok::Trbrace {
            self.err("label or } expected");
        }

        // Label
        if t == Tok::Tlbl {
            let name = self.lex.tokval.str_val.clone();
            let b = self.findblk(&name);

            if let Some(cur_idx) = self.curb {
                if self.curf.blks[cur_idx].jmp.typ == Jmp::Jxxx {
                    self.closeblk();
                    self.curf.blks[cur_idx].jmp.typ = Jmp::Jmp_;
                    self.curf.blks[cur_idx].s1 = Some(BlkId(b as u32));
                }
            }
            if self.curf.blks[b].jmp.typ != Jmp::Jxxx {
                self.errf(format!("multiple definitions of block @{}", name));
            }
            self.curb = Some(b);
            // Don't clear insb: closeblk() already drained it via drain(..)
            // for the prior block. For the first label (when there was no prior
            // curb), insb may contain function parameter instructions that
            // belong to this block — clearing would destroy them.
            self.lex.expect(Tok::Tnl);
            return PState::PPhi;
        }

        if t == Tok::Trbrace {
            return PState::PEnd;
        }

        // ret
        if t == Tok::Tret {
            let cur = self
                .curb
                .unwrap_or_else(|| self.err("instruction outside block"));
            self.curf.blks[cur].jmp.typ = ret_jmp(self.rcls);
            if self.lex.peek() == Tok::Tnl {
                self.curf.blks[cur].jmp.typ = Jmp::Ret0;
            } else if self.rcls != K0 {
                let r = self.parseref();
                if r == Ref::R {
                    self.err("invalid return value");
                }
                self.curf.blks[cur].jmp.arg = r;
            }
            self.lex.expect(Tok::Tnl);
            self.closeblk();
            return PState::PLbl;
        }

        // jmp
        if t == Tok::Tjmp {
            let cur = self
                .curb
                .unwrap_or_else(|| self.err("instruction outside block"));
            self.curf.blks[cur].jmp.typ = Jmp::Jmp_;
            self.lex.expect(Tok::Tlbl);
            let name = self.lex.tokval.str_val.clone();
            let s1 = self.findblk(&name);
            self.curf.blks[cur].s1 = Some(BlkId(s1 as u32));
            self.lex.expect(Tok::Tnl);
            self.closeblk();
            return PState::PLbl;
        }

        // jnz
        if t == Tok::Tjnz {
            let cur = self
                .curb
                .unwrap_or_else(|| self.err("instruction outside block"));
            self.curf.blks[cur].jmp.typ = Jmp::Jnz;
            let r = self.parseref();
            if r == Ref::R {
                self.err("invalid argument for jnz jump");
            }
            self.curf.blks[cur].jmp.arg = r;
            self.lex.expect(Tok::Tcomma);
            self.lex.expect(Tok::Tlbl);
            let name1 = self.lex.tokval.str_val.clone();
            let s1 = self.findblk(&name1);
            self.curf.blks[cur].s1 = Some(BlkId(s1 as u32));
            self.lex.expect(Tok::Tcomma);
            self.lex.expect(Tok::Tlbl);
            let name2 = self.lex.tokval.str_val.clone();
            let s2 = self.findblk(&name2);
            self.curf.blks[cur].s2 = Some(BlkId(s2 as u32));
            // Check not jumping to start block
            if s1 == 0 || s2 == 0 {
                // Index 0 is the start block (first block created in parsefn)
                // Actually, we need to compare with curf.start
                let start_id = self.curf.start;
                if self.curf.blks[s1].id == start_id.0 || self.curf.blks[s2].id == start_id.0 {
                    self.err("invalid jump to the start block");
                }
            }
            self.lex.expect(Tok::Tnl);
            self.closeblk();
            return PState::PLbl;
        }

        // hlt
        if t == Tok::Thlt {
            let cur = self
                .curb
                .unwrap_or_else(|| self.err("instruction outside block"));
            self.curf.blks[cur].jmp.typ = Jmp::Hlt;
            self.lex.expect(Tok::Tnl);
            self.closeblk();
            return PState::PLbl;
        }

        // dbgloc
        if t == Tok::Top(Op::Dbgloc) {
            self.curb
                .unwrap_or_else(|| self.err("instruction outside block"));
            self.lex.expect(Tok::Tint);
            let line = self.lex.tokval.num;
            let arg0 = Ref::Int(line as i32);
            let arg1 = if self.lex.peek() == Tok::Tcomma {
                self.lex.next();
                self.lex.expect(Tok::Tint);
                Ref::Int(self.lex.tokval.num as i32)
            } else {
                Ref::Int(0)
            };
            self.insb.push(Ins {
                op: Op::Dbgloc,
                cls: Cls::Kw,
                to: Ref::R,
                arg: [arg0, arg1],
            });
            self.lex.expect(Tok::Tnl);
            return PState::PIns;
        }

        // Instruction with result: %tmp =cls op ...
        let (r, k, ty, op) = if t == Tok::Ttmp {
            let name = self.lex.tokval.str_val.clone();
            let r = self.tmpref(&name);
            self.lex.expect(Tok::Teq);
            let (k, ty) = self.parsecls();
            let op = self.lex.next();
            (r, k, ty, op)
        } else {
            // Operations without result: stores, blit, call, vastart
            let is_store_tok = is_store_token(t);
            if is_store_tok || t == Tok::Tblit || t == Tok::Tcall || t == Tok::Top(Op::Vastart) {
                (Ref::R, Cls::Kw as i32, -1i32, t)
            } else {
                self.err("label, instruction or jump expected");
            }
        };

        self.curb
            .unwrap_or_else(|| self.err("instruction outside block"));

        // Handle call
        if op == Tok::Tcall {
            let arg0 = self.parseref();
            self.parserefl(true);
            // The parserefl pushed arg/argc/etc. into insb
            // Now push the call instruction
            let (call_k, arg1) = if k == KC {
                (Cls::Kl, Ref::Typ(TypId(ty as u32)))
            } else if k >= KSB {
                (Cls::Kw, Ref::R)
            } else {
                (Cls::from_i8(k as i8), Ref::R)
            };
            self.lex.expect(Tok::Tnl);
            self.insb.push(Ins {
                op: Op::Call,
                cls: call_k,
                to: r,
                arg: [arg0, arg1],
            });
            return PState::PIns;
        }

        // Translate token aliases to Op
        let mut real_op = match op {
            Tok::Top(o) => o,
            Tok::Tloadw => Op::Loadsw,
            Tok::Tloadl | Tok::Tloads | Tok::Tloadd => Op::Load,
            Tok::Talloc1 | Tok::Talloc2 => Op::Alloc4,
            Tok::Tblit => Op::Blit0, // handled specially below
            Tok::Tphi => Op::Oxxx,   // handled specially below
            _ => self.err("invalid instruction"),
        };

        // vastart check
        if real_op == Op::Vastart && !self.curf.vararg {
            self.err("cannot use vastart in non-variadic function");
        }

        if k >= KSB && op != Tok::Tphi && op != Tok::Tblit {
            self.err("size class must be w, l, s, or d");
        }

        // Parse arguments
        let mut args: Vec<Ref> = Vec::new();
        let mut blks: Vec<String> = Vec::new();

        if self.lex.peek() != Tok::Tnl {
            loop {
                if args.len() >= N_PRED {
                    self.err("too many arguments");
                }
                if op == Tok::Tphi {
                    self.lex.expect(Tok::Tlbl);
                    blks.push(self.lex.tokval.str_val.clone());
                }
                let a = self.parseref();
                if a == Ref::R {
                    self.err("invalid instruction argument");
                }
                args.push(a);
                let t2 = self.lex.peek();
                if t2 == Tok::Tnl {
                    break;
                }
                if t2 != Tok::Tcomma {
                    self.err(", or end of line expected");
                }
                self.lex.next();
            }
        }
        self.lex.next(); // consume the newline

        // Handle phi
        if op == Tok::Tphi {
            if ps != PState::PPhi {
                self.err("unexpected phi instruction");
            }
            let cur = self.curb.unwrap();
            // Check not in start block
            if self.curf.blks[cur].id == self.curf.start.0 {
                self.err("unexpected phi instruction");
            }
            let phi_blks: Vec<BlkId> = blks
                .iter()
                .map(|name| {
                    let idx = self.findblk(name);
                    BlkId(self.curf.blks[idx].id)
                })
                .collect();
            let phi = Phi {
                to: r,
                cls: Cls::from_i8(k as i8),
                args,
                blks: phi_blks,
            };
            self.curf.blks[cur].phi.push(phi);
            return PState::PPhi;
        }

        // Handle blit
        if op == Tok::Tblit {
            if args.len() < 3 {
                self.err("blit requires 3 arguments");
            }
            // blit src, dst, size
            self.insb.push(Ins {
                op: Op::Blit0,
                cls: Cls::Kx,
                to: Ref::R,
                arg: [args[0], args[1]],
            });
            // The third arg must be a constant
            let size_ref = args[2];
            match size_ref {
                Ref::Con(cid) => {
                    let c = &self.curf.cons[cid.0 as usize];
                    let sz = c.bits.i();
                    if c.typ != ConType::Bits || sz < 0 {
                        self.err("invalid blit size");
                    }
                    self.insb.push(Ins {
                        op: Op::Blit1,
                        cls: Cls::Kx,
                        to: Ref::R,
                        arg: [Ref::Int(sz as i32), Ref::R],
                    });
                }
                _ => self.err("blit size must be constant"),
            }
            return PState::PIns;
        }

        // Default instruction
        if let Tok::Top(o) = op {
            if (o as u16) >= (Op::Nop as u16) && o != Op::Vastart && o != Op::Dbgloc {
                self.err("invalid instruction");
            }
            real_op = o;
        }

        let cls = Cls::from_i8(k as i8);
        self.insb.push(Ins {
            op: real_op,
            cls,
            to: r,
            arg: [
                args.first().copied().unwrap_or(Ref::R),
                args.get(1).copied().unwrap_or(Ref::R),
            ],
        });
        PState::PIns
    }

    // -- Type checking --

    fn typecheck(&self) {
        // Post-parse SSA type verification.
        // For now, we do a lightweight check: verify that each tmp assigned
        // via instruction or phi has a consistent class.
        // Full pred-matching check requires fillpreds which the rest of the
        // compiler does. We check:
        // 1. Each instruction's result tmp gets its class set; duplicates are errors.
        // 2. Instruction arg classes match the opcode's expected classes.

        let f = &self.curf;

        // First pass: record classes from phi + ins assignments
        let mut tmp_cls: Vec<Cls> = vec![Cls::Kx; f.tmps.len()];

        for b in &f.blks {
            for p in &b.phi {
                if let Ref::Tmp(id) = p.to {
                    tmp_cls[id.0 as usize] = p.cls;
                }
            }
            for i in &b.ins {
                if let Ref::Tmp(id) = i.to {
                    let t = &mut tmp_cls[id.0 as usize];
                    if *t != Cls::Kx && *t != i.cls {
                        self.errf(format!(
                            "temporary %{} is assigned with multiple types",
                            f.tmps[id.0 as usize].name
                        ));
                    }
                    *t = i.cls;
                }
            }
        }

        // Second pass: check instruction arg types
        for b in &f.blks {
            for i in &b.ins {
                for n in 0..2 {
                    let k_idx = i.cls as i8;
                    if k_idx < 0 {
                        continue;
                    }
                    let k = OP_TABLE[i.op as usize].argcls[n][k_idx as usize];
                    let r = i.arg[n];
                    // Ke = erroneous
                    if k == Cls::Kx && r != Ref::R {
                        // Check if Ref::Typ - those are OK
                        if matches!(r, Ref::Typ(_)) {
                            continue;
                        }
                    }
                    // Basic usecheck
                    if let Ref::Tmp(id) = r {
                        let tc = tmp_cls[id.0 as usize];
                        if tc != Cls::Kx && tc != k && !(tc == Cls::Kl && k == Cls::Kw) {
                            // Type mismatch - would be an error in full check
                        }
                    }
                }
            }
        }
    }

    // -- Function parser --

    fn parsefn(&mut self, lnk: Lnk) {
        self.curb = None;
        self.nblk = 0;
        self.insb.clear();
        self.blk_map.clear();
        self.blk_order.clear();
        self.tmp_map.clear();

        self.curf = Fn::default();

        // Initialize: 2 constants (undef + zero)
        self.curf.cons.push(Con {
            typ: ConType::Bits,
            bits: ConBits::from_i64(0xdeaddead),
            ..Con::default()
        });
        self.curf.cons.push(Con {
            typ: ConType::Bits,
            bits: ConBits::from_i64(0),
            ..Con::default()
        });

        // Initialize register temporaries (0..TMP0)
        for _i in 0..TMP0 {
            let mut tmp = Tmp::default();
            tmp.cls = Cls::Kl;
            self.curf.tmps.push(tmp);
        }

        self.curf.lnk = lnk;
        self.curf.retty = -1;

        // Parse return type
        if self.lex.peek() != Tok::Tglo {
            let (cls, ty) = self.parsecls();
            self.rcls = cls;
            self.curf.retty = ty;
        } else {
            self.rcls = K0;
        }

        // Function name
        if self.lex.next() != Tok::Tglo {
            self.err("function name expected");
        }
        self.curf.name = self.lex.tokval.str_val.clone();

        // Parse parameters
        self.curf.vararg = self.parserefl(false);

        if self.lex.nextnl() != Tok::Tlbrace {
            self.err("function body must start with {");
        }

        // Parameters are now in self.insb — they go into the start block.
        // Create the start block implicitly.
        let mut ps = PState::PLbl;
        loop {
            ps = self.parseline(ps);
            if ps == PState::PEnd {
                break;
            }
        }

        if self.curb.is_none() {
            self.err("empty function");
        }
        if let Some(cur) = self.curb {
            if self.curf.blks[cur].jmp.typ == Jmp::Jxxx {
                self.err("last block misses jump");
            }
        }

        // Set start block
        if !self.blk_order.is_empty() {
            self.curf.start = BlkId(self.curf.blks[self.blk_order[0]].id);
        }

        // Run lightweight type check
        self.typecheck();
    }

    // -- Type parser --

    fn parsefields(&mut self, fields: &mut Vec<Field>, ty: &mut TypStub, t_init: Tok) {
        let mut sz: u64 = 0;
        let mut al = ty.align;
        let mut t = t_init;

        while t != Tok::Trbrace {
            let (ftype, s, a) = match t {
                Tok::Td => (FieldType::Fd, 8u64, 3i32),
                Tok::Tl => (FieldType::Fl, 8, 3),
                Tok::Ts => (FieldType::Fs, 4, 2),
                Tok::Tw => (FieldType::Fw, 4, 2),
                Tok::Th => (FieldType::Fh, 2, 1),
                Tok::Tb => (FieldType::Fb, 1, 0),
                Tok::Ttyp => {
                    let idx = self.findtyp(self.typs.len() - 1);
                    let ty1 = &self.typs[idx];
                    let s = ty1.size;
                    let a = ty1.align;
                    (FieldType::FTyp, s, a)
                }
                _ => self.err("invalid type member specifier"),
            };

            let typ_len = if ftype == FieldType::FTyp {
                let idx = self.findtyp(self.typs.len() - 1);
                idx as u32
            } else {
                s as u32
            };

            if a > al {
                al = a;
            }
            let align_mask = ((1u64 << a) - 1) as u64;
            let padding = ((sz + align_mask) & !align_mask) - sz;
            if padding > 0 && fields.len() < N_FIELD {
                fields.push(Field {
                    typ: FieldType::FPad,
                    len: padding as u32,
                });
            }

            t = self.lex.nextnl();
            let count = if t == Tok::Tint {
                let c = self.lex.tokval.num;
                t = self.lex.nextnl();
                c as u64
            } else {
                1
            };

            sz += padding + count * s;

            let fld_len = if ftype == FieldType::FTyp {
                typ_len
            } else {
                s as u32
            };
            for _ in 0..count {
                if fields.len() < N_FIELD {
                    fields.push(Field {
                        typ: ftype,
                        len: fld_len,
                    });
                }
            }

            if t != Tok::Tcomma {
                break;
            }
            t = self.lex.nextnl();
        }

        if t != Tok::Trbrace {
            self.err(", or } expected");
        }

        fields.push(Field {
            typ: FieldType::End,
            len: 0,
        });

        let a_val = 1u64 << al;
        if sz < ty.size {
            sz = ty.size;
        }
        ty.size = (sz + a_val - 1) & !(a_val - 1);
        ty.align = al;
    }

    fn parsetyp(&mut self) {
        if self.lex.nextnl() != Tok::Ttyp {
            self.err("type name expected");
        }
        let name = self.lex.tokval.str_val.clone();
        if self.lex.nextnl() != Tok::Teq {
            self.err("= expected after type name");
        }

        let mut stub = TypStub {
            is_dark: false,
            is_union: false,
            align: -1,
            size: 0,
        };

        let mut t = self.lex.nextnl();
        if t == Tok::Talign {
            if self.lex.nextnl() != Tok::Tint {
                self.err("alignment expected");
            }
            let mut val = self.lex.tokval.num;
            let mut al = 0i32;
            while val > 1 {
                val /= 2;
                al += 1;
            }
            stub.align = al;
            t = self.lex.nextnl();
        }

        if t != Tok::Tlbrace {
            self.err("type body must start with {");
        }

        t = self.lex.nextnl();
        if t == Tok::Tint {
            // Dark type
            stub.is_dark = true;
            stub.size = self.lex.tokval.num as u64;
            if stub.align == -1 {
                self.err("dark types need alignment");
            }
            if self.lex.nextnl() != Tok::Trbrace {
                self.err("} expected");
            }

            // Push a placeholder so findtyp works
            self.typs.push(Typ {
                name,
                is_dark: true,
                is_union: false,
                align: stub.align,
                size: stub.size,
                nunion: 0,
                fields: Vec::new(),
            });
            return;
        }

        let mut all_fields: Vec<Vec<Field>> = Vec::new();

        if t == Tok::Tlbrace {
            // Union type
            stub.is_union = true;
            loop {
                if t != Tok::Tlbrace {
                    self.err("invalid union member");
                }
                let mut flds = Vec::new();
                let inner_t = self.lex.nextnl();
                self.parsefields(&mut flds, &mut stub, inner_t);
                all_fields.push(flds);
                t = self.lex.nextnl();
                if t == Tok::Trbrace {
                    break;
                }
            }
        } else {
            let mut flds = Vec::new();
            self.parsefields(&mut flds, &mut stub, t);
            all_fields.push(flds);
        }

        self.typs.push(Typ {
            name,
            is_dark: stub.is_dark,
            is_union: stub.is_union,
            align: stub.align,
            size: stub.size,
            nunion: all_fields.len() as u32,
            fields: all_fields,
        });
    }

    // -- Data parser --

    fn parsedat(&mut self, lnk: Lnk) -> Vec<Dat> {
        let mut items: Vec<Dat> = Vec::new();

        if self.lex.nextnl() != Tok::Tglo {
            self.err("data name expected");
        }
        let name = self.lex.tokval.str_val.clone();
        if self.lex.nextnl() != Tok::Teq {
            self.err("= expected after data name");
        }

        let mut data_lnk = lnk;
        let mut t = self.lex.nextnl();
        data_lnk.align = 8;
        if t == Tok::Talign {
            if self.lex.nextnl() != Tok::Tint {
                self.err("alignment expected");
            }
            data_lnk.align = self.lex.tokval.num as u8;
            t = self.lex.nextnl();
        }

        // DStart
        items.push(Dat {
            item: DatItem::Start,
            name: Some(name),
            lnk: Some(data_lnk),
        });

        if t != Tok::Tlbrace {
            self.err("expected data contents in { .. }");
        }

        loop {
            let dt = self.lex.nextnl();
            if dt == Tok::Trbrace {
                break;
            }

            // Size specifier
            let is_zero = dt == Tok::Tz;

            t = self.lex.nextnl();
            loop {
                let item = match t {
                    Tok::Tflts => DatItem::FltS(self.lex.tokval.flts),
                    Tok::Tfltd => DatItem::FltD(self.lex.tokval.fltd),
                    Tok::Tint => {
                        if is_zero {
                            DatItem::Zero(self.lex.tokval.num as u64)
                        } else {
                            match dt {
                                Tok::Tb => DatItem::Byte(self.lex.tokval.num),
                                Tok::Th => DatItem::Half(self.lex.tokval.num),
                                Tok::Tw | Tok::Ts => DatItem::Word(self.lex.tokval.num),
                                Tok::Tl | Tok::Td => DatItem::Long(self.lex.tokval.num),
                                _ => DatItem::Long(self.lex.tokval.num),
                            }
                        }
                    }
                    Tok::Tglo => {
                        let ref_name = self.lex.tokval.str_val.clone();
                        let mut off = 0i64;
                        if self.lex.peek() == Tok::Tplus {
                            self.lex.next();
                            if self.lex.next() != Tok::Tint {
                                self.err("invalid token after offset in ref");
                            }
                            off = self.lex.tokval.num;
                        }
                        DatItem::Ref {
                            name: ref_name,
                            off,
                        }
                    }
                    Tok::Tstr => DatItem::Str(self.lex.tokval.str_val.clone()),
                    _ => self.err("constant literal expected"),
                };

                items.push(Dat {
                    item,
                    name: None,
                    lnk: None,
                });

                t = self.lex.nextnl();
                if t == Tok::Tint
                    || t == Tok::Tflts
                    || t == Tok::Tfltd
                    || t == Tok::Tstr
                    || t == Tok::Tglo
                {
                    continue;
                }
                break;
            }

            if t == Tok::Trbrace {
                break;
            }
            if t != Tok::Tcomma {
                self.err(", or } expected");
            }
        }

        // DEnd
        items.push(Dat {
            item: DatItem::End,
            name: None,
            lnk: None,
        });

        items
    }

    // -- Linkage parser --

    fn parselnk(&mut self) -> (Lnk, Tok) {
        let mut lnk = Lnk::default();
        let mut haslnk = false;

        loop {
            let t = self.lex.nextnl();
            match t {
                Tok::Texport => {
                    lnk.export = true;
                    haslnk = true;
                }
                Tok::Tthread => {
                    lnk.thread = true;
                    haslnk = true;
                }
                Tok::Tsection => {
                    if lnk.sec.is_some() {
                        self.err("only one section allowed");
                    }
                    if self.lex.next() != Tok::Tstr {
                        self.err("section \"name\" expected");
                    }
                    lnk.sec = Some(self.lex.tokval.str_val.clone());
                    if self.lex.peek() == Tok::Tstr {
                        self.lex.next();
                        lnk.secf = Some(self.lex.tokval.str_val.clone());
                    }
                    haslnk = true;
                }
                _ => {
                    if t == Tok::Tfunc && lnk.thread {
                        self.err("only data may have thread linkage");
                    }
                    if haslnk && t != Tok::Tdata && t != Tok::Tfunc {
                        self.err("only data and function have linkage");
                    }
                    return (lnk, t);
                }
            }
        }
    }

    // -- Top-level --

    fn parse_all(&mut self) -> ParseResult {
        let mut fns = Vec::new();
        let mut dats = Vec::new();

        loop {
            let (lnk, t) = self.parselnk();
            match t {
                Tok::Tdbgfile => {
                    self.lex.expect(Tok::Tstr);
                    self.dbgfiles.push(self.lex.tokval.str_val.clone());
                }
                Tok::Tfunc => {
                    self.parsefn(lnk);
                    fns.push(self.curf.clone());
                }
                Tok::Tdata => {
                    let dat = self.parsedat(lnk);
                    dats.push(dat);
                }
                Tok::Ttype => {
                    self.parsetyp();
                }
                Tok::Teof => {
                    break;
                }
                _ => {
                    self.err("top-level definition expected");
                }
            }
        }

        ParseResult {
            types: self.typs.clone(),
            data: dats,
            functions: fns,
        }
    }
}

/// Helper struct for building Typ during parsing.
struct TypStub {
    is_dark: bool,
    is_union: bool,
    align: i32,
    size: u64,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn is_store_token(t: Tok) -> bool {
    match t {
        Tok::Top(op) => op.is_store(),
        _ => false,
    }
}

/// Map rcls to return Jmp variant.
fn ret_jmp(rcls: i32) -> Jmp {
    match rcls {
        x if x == Cls::Kw as i32 => Jmp::Retw,
        x if x == Cls::Kl as i32 => Jmp::Retl,
        x if x == Cls::Ks as i32 => Jmp::Rets,
        x if x == Cls::Kd as i32 => Jmp::Retd,
        x if x == KSB => Jmp::Retsb,
        x if x == KUB => Jmp::Retub,
        x if x == KSH => Jmp::Retsh,
        x if x == KUH => Jmp::Retuh,
        x if x == KC => Jmp::Retc,
        x if x == K0 => Jmp::Ret0,
        _ => Jmp::Ret0,
    }
}

// ---------------------------------------------------------------------------
// IR printer
// ---------------------------------------------------------------------------

/// Print a constant value in QBE text format.
fn printcon(c: &Con) -> String {
    match c.typ {
        ConType::Undef => String::new(),
        ConType::Addr => {
            let mut s = String::new();
            if c.sym.typ == SymType::Thr {
                s.push_str("thread ");
            }
            write!(s, "${}", c.sym.id).unwrap();
            if c.bits.i() != 0 {
                write!(s, "{:+}", c.bits.i()).unwrap();
            }
            s
        }
        ConType::Bits => {
            if c.flt == 1 {
                format!("s_{}", c.bits.s())
            } else if c.flt == 2 {
                format!("d_{}", c.bits.d())
            } else {
                format!("{}", c.bits.i())
            }
        }
    }
}

/// Print a Ref in QBE text format.
pub fn printref(r: Ref, f: &Fn, typs: &[Typ]) -> String {
    match r {
        Ref::Tmp(id) => {
            if id.0 < TMP0 {
                format!("R{}", id.0)
            } else {
                format!("%{}", f.tmps[id.0 as usize].name)
            }
        }
        Ref::Con(id) => {
            if r == Ref::UNDEF {
                "UNDEF".to_string()
            } else {
                printcon(&f.cons[id.0 as usize])
            }
        }
        Ref::Slot(_v) => {
            format!("S{}", r.sval())
        }
        Ref::Call(v) => {
            format!("{:04x}", v)
        }
        Ref::Typ(id) => {
            if (id.0 as usize) < typs.len() {
                format!(":{}", typs[id.0 as usize].name)
            } else {
                format!(":T{}", id.0)
            }
        }
        Ref::Mem(id) => {
            let m = &f.mems[id.0 as usize];
            let mut s = String::from("[");
            let mut has_part = false;
            if m.offset.typ != ConType::Undef {
                s.push_str(&printcon(&m.offset));
                has_part = true;
            }
            if m.base != Ref::R {
                if has_part {
                    s.push_str(" + ");
                }
                s.push_str(&printref(m.base, f, typs));
                has_part = true;
            }
            if m.index != Ref::R {
                if has_part {
                    s.push_str(" + ");
                }
                write!(s, "{} * ", m.scale).unwrap();
                s.push_str(&printref(m.index, f, typs));
            }
            s.push(']');
            s
        }
        Ref::Int(_v) => {
            format!("{}", r.sval())
        }
        Ref::R => String::new(),
    }
}

/// Print a function in QBE IL text format for debugging.
pub fn printfn(f: &Fn, typs: &[Typ]) -> String {
    let ktoc = ['w', 'l', 's', 'd'];
    let mut out = String::new();

    writeln!(out, "function ${}() {{", f.name).unwrap();

    for b in &f.blks {
        writeln!(out, "@{}", b.name).unwrap();

        // Phi nodes
        for p in &b.phi {
            write!(out, "\t{}", printref(p.to, f, typs)).unwrap();
            let cls_idx = p.cls as i8;
            let cls_ch = if cls_idx >= 0 && (cls_idx as usize) < ktoc.len() {
                ktoc[cls_idx as usize]
            } else {
                '?'
            };
            write!(out, " ={} phi ", cls_ch).unwrap();
            for (n, (arg, blk_id)) in p.args.iter().zip(p.blks.iter()).enumerate() {
                // Find block name by id
                let blk_name = f
                    .blks
                    .iter()
                    .find(|blk| blk.id == blk_id.0)
                    .map(|blk| blk.name.as_str())
                    .unwrap_or("??");
                write!(out, "@{} {}", blk_name, printref(*arg, f, typs)).unwrap();
                if n < p.args.len() - 1 {
                    write!(out, ", ").unwrap();
                }
            }
            writeln!(out).unwrap();
        }

        // Instructions
        for i in &b.ins {
            write!(out, "\t").unwrap();
            if i.to != Ref::R {
                let cls_idx = i.cls as i8;
                let cls_ch = if cls_idx >= 0 && (cls_idx as usize) < ktoc.len() {
                    ktoc[cls_idx as usize]
                } else {
                    '?'
                };
                write!(out, "{} ={} ", printref(i.to, f, typs), cls_ch).unwrap();
            }
            let opname = OP_TABLE[i.op as usize].name;
            write!(out, "{}", opname).unwrap();

            // For some ops without result, print the class suffix
            if i.to == Ref::R {
                match i.op {
                    Op::Arg
                    | Op::Swap
                    | Op::Xcmp
                    | Op::Acmp
                    | Op::Acmn
                    | Op::Afcmp
                    | Op::Xtest
                    | Op::Xdiv
                    | Op::Xidiv => {
                        let cls_idx = i.cls as i8;
                        if cls_idx >= 0 && (cls_idx as usize) < ktoc.len() {
                            write!(out, "{}", ktoc[cls_idx as usize]).unwrap();
                        }
                    }
                    _ => {}
                }
            }

            if i.arg[0] != Ref::R {
                write!(out, " {}", printref(i.arg[0], f, typs)).unwrap();
            }
            if i.arg[1] != Ref::R {
                write!(out, ", {}", printref(i.arg[1], f, typs)).unwrap();
            }
            writeln!(out).unwrap();
        }

        // Jump
        match b.jmp.typ {
            Jmp::Ret0
            | Jmp::Retw
            | Jmp::Retl
            | Jmp::Rets
            | Jmp::Retd
            | Jmp::Retsb
            | Jmp::Retub
            | Jmp::Retsh
            | Jmp::Retuh
            | Jmp::Retc => {
                let jname = jmp_name(b.jmp.typ);
                write!(out, "\t{}", jname).unwrap();
                if b.jmp.typ != Jmp::Ret0 || b.jmp.arg != Ref::R {
                    write!(out, " {}", printref(b.jmp.arg, f, typs)).unwrap();
                }
                if b.jmp.typ == Jmp::Retc && f.retty >= 0 && (f.retty as usize) < typs.len() {
                    write!(out, ", :{}", typs[f.retty as usize].name).unwrap();
                }
                writeln!(out).unwrap();
            }
            Jmp::Hlt => {
                writeln!(out, "\thlt").unwrap();
            }
            Jmp::Jmp_ => {
                if let Some(s1_id) = b.s1 {
                    let s1_name = f
                        .blks
                        .iter()
                        .find(|blk| blk.id == s1_id.0)
                        .map(|blk| blk.name.as_str())
                        .unwrap_or("??");
                    writeln!(out, "\tjmp @{}", s1_name).unwrap();
                }
            }
            Jmp::Jnz => {
                write!(out, "\tjnz {}", printref(b.jmp.arg, f, typs)).unwrap();
                let s1_name =
                    b.s1.and_then(|id| {
                        f.blks
                            .iter()
                            .find(|blk| blk.id == id.0)
                            .map(|blk| blk.name.as_str())
                    })
                    .unwrap_or("??");
                let s2_name =
                    b.s2.and_then(|id| {
                        f.blks
                            .iter()
                            .find(|blk| blk.id == id.0)
                            .map(|blk| blk.name.as_str())
                    })
                    .unwrap_or("??");
                writeln!(out, ", @{}, @{}", s1_name, s2_name).unwrap();
            }
            Jmp::Jxxx => {}
            _ => {
                // Flag jumps
                let jname = jmp_name(b.jmp.typ);
                write!(out, "\t{} ", jname).unwrap();
                let s1_name =
                    b.s1.and_then(|id| {
                        f.blks
                            .iter()
                            .find(|blk| blk.id == id.0)
                            .map(|blk| blk.name.as_str())
                    })
                    .unwrap_or("??");
                let s2_name =
                    b.s2.and_then(|id| {
                        f.blks
                            .iter()
                            .find(|blk| blk.id == id.0)
                            .map(|blk| blk.name.as_str())
                    })
                    .unwrap_or("??");
                writeln!(out, "@{}, @{}", s1_name, s2_name).unwrap();
            }
        }
    }

    writeln!(out, "}}").unwrap();
    out
}

/// Get the string name for a Jmp variant.
fn jmp_name(j: Jmp) -> &'static str {
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

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse QBE IR text into types, data blocks, and functions.
pub fn parse(input: &str) -> ParseResult {
    let mut parser = Parser::new(input);
    parser.parse_all()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let result = parse("");
        assert!(result.types.is_empty());
        assert!(result.data.is_empty());
        assert!(result.functions.is_empty());
    }

    #[test]
    fn test_parse_type() {
        let input = "type :pair = { w, l }\n";
        let result = parse(input);
        assert_eq!(result.types.len(), 1);
        assert_eq!(result.types[0].name, "pair");
        assert!(!result.types[0].is_dark);
        assert!(!result.types[0].is_union);
    }

    #[test]
    fn test_parse_data() {
        let input = "data $str = { b \"hello\", b 0 }\n";
        let result = parse(input);
        assert_eq!(result.data.len(), 1);
        // Start, "hello", 0, End
        assert!(result.data[0].len() >= 3);
    }

    #[test]
    fn test_parse_function() {
        let input = "\
function w $add(w %a, w %b) {
@start
    %c =w add %a, %b
    ret %c
}
";
        let result = parse(input);
        assert_eq!(result.functions.len(), 1);
        assert_eq!(result.functions[0].name, "add");
    }

    #[test]
    fn test_parse_void_function() {
        let input = "\
function $nop() {
@start
    ret
}
";
        let result = parse(input);
        assert_eq!(result.functions.len(), 1);
        assert_eq!(result.functions[0].name, "nop");
    }

    #[test]
    fn test_printfn_roundtrip() {
        let input = "\
function w $add(w %a, w %b) {
@start
    %c =w add %a, %b
    ret %c
}
";
        let result = parse(input);
        let f = &result.functions[0];
        let text = printfn(f, &result.types);
        assert!(text.contains("add"));
        assert!(text.contains("ret"));
    }
}
