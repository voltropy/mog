use crate::ir::{Cls, Fn, Op, Ref};
use crate::util::{getcon, newtmp, InsBuffer};

/// Emit load/store pairs for a blit (block copy) operation.
///
/// `sd[0]` is the source address, `sd[1]` is the destination address.
/// `sz` is signed: positive means forward copy, negative means backward.
fn blit(sd: [Ref; 2], sz: i32, f: &mut Fn, buf: &mut InsBuffer) {
    struct Entry {
        st: Op,
        ld: Op,
        cls: Cls,
        size: i32,
    }

    let tbl = [
        Entry {
            st: Op::Storel,
            ld: Op::Load,
            cls: Cls::Kl,
            size: 8,
        },
        Entry {
            st: Op::Storew,
            ld: Op::Load,
            cls: Cls::Kw,
            size: 4,
        },
        Entry {
            st: Op::Storeh,
            ld: Op::Loaduh,
            cls: Cls::Kw,
            size: 2,
        },
        Entry {
            st: Op::Storeb,
            ld: Op::Loadub,
            cls: Cls::Kw,
            size: 1,
        },
    ];

    let fwd = sz >= 0;
    let mut sz = sz.unsigned_abs() as i32;
    let mut off: i32 = if fwd { sz } else { 0 };

    for p in &tbl {
        let n = p.size;
        while sz >= n {
            if fwd {
                off -= n;
            }
            let r = newtmp("blt", Cls::Kl, f);
            let r1 = newtmp("blt", Cls::Kl, f);
            let ro = getcon(off as i64, f);
            buf.emit(p.st, Cls::Kw, Ref::R, r, r1);
            buf.emit(Op::Add, Cls::Kl, r1, sd[1], ro);
            let r1 = newtmp("blt", Cls::Kl, f);
            buf.emit(p.ld, p.cls, r, r1, Ref::R);
            buf.emit(Op::Add, Cls::Kl, r1, sd[0], ro);
            if !fwd {
                off += n;
            }
            sz -= n;
        }
    }
}

/// Blit lowering pass — expand `Blit0`/`Blit1` pairs into explicit
/// load/store sequences.
pub fn simpl(f: &mut Fn) {
    for blk_idx in 0..f.blks.len() {
        let nins = f.blks[blk_idx].ins.len();
        if nins == 0 {
            continue;
        }

        let mut buf = InsBuffer::new();
        let mut started = false;
        let mut i = nins;

        // Walk instructions backward (matching C QBE's pattern).
        while i > 0 {
            i -= 1;
            let ins = f.blks[blk_idx].ins[i];

            match ins.op {
                Op::Blit1 => {
                    // Blit1 must be preceded by Blit0.
                    assert!(i > 0, "Blit1 without preceding Blit0");
                    let prev = f.blks[blk_idx].ins[i - 1];
                    assert!(
                        prev.op == Op::Blit0,
                        "instruction before Blit1 must be Blit0"
                    );

                    if !started {
                        // Copy all instructions after the blit pair into
                        // the buffer in REVERSE order (since the buffer
                        // stores in reverse and finish() reverses).
                        for j in (i + 1..nins).rev() {
                            buf.emiti(f.blks[blk_idx].ins[j]);
                        }
                        started = true;
                    }

                    // Expand the blit: source/dest come from Blit0's
                    // args, size from Blit1's first arg.
                    let sd = prev.arg;
                    let sz = ins.arg[0].sval();
                    blit(sd, sz, f, &mut buf);

                    // Skip the Blit0 instruction.
                    i -= 1;
                }
                _ => {
                    if started {
                        buf.emiti(ins);
                    }
                }
            }
        }

        if started {
            f.blks[blk_idx].ins = buf.finish();
        }
    }
}
