//! IR type tests for rqbe.
//!
//! Tests the IR types, their constructors, comparisons, and helper methods.
//! These complement the inline unit tests in ir.rs with integration-level
//! tests that exercise the public API.

use rqbe::ir::*;

// ---------------------------------------------------------------------------
// Ref creation and comparison
// ---------------------------------------------------------------------------

#[test]
fn ref_r_is_default() {
    assert_eq!(Ref::default(), Ref::R);
}

#[test]
fn ref_r_is_none() {
    assert!(Ref::R.is_none());
    assert!(!Ref::R.is_tmp());
}

#[test]
fn ref_tmp_creation() {
    let r = Ref::Tmp(TmpId(5));
    assert!(r.is_tmp());
    assert!(!r.is_none());
    assert_eq!(r.val(), 5);
}

#[test]
fn ref_con_creation() {
    let r = Ref::Con(ConId(3));
    assert!(!r.is_tmp());
    assert!(!r.is_none());
    assert_eq!(r.val(), 3);
}

#[test]
fn ref_int_creation() {
    let r = Ref::Int(42);
    assert!(!r.is_tmp());
    assert!(!r.is_none());
    assert_eq!(r.sval(), 42);
}

#[test]
fn ref_int_negative() {
    // Ref::Int uses a 29-bit masked encoding internally.
    // sval() sign-extends from bit 29, so small negative values
    // are preserved through the val()/sval() roundtrip.
    let r = Ref::Int(-1);
    // -1 as u32 is 0xFFFF_FFFF, masked to 29 bits: 0x1FFF_FFFF
    // sval sign-extends from bit 29 of the val() result.
    let v = r.val();
    assert_eq!(v, 0x1FFF_FFFF);
    // sval() sign-extends from bit 29 using i32 arithmetic:
    // ((val as i64) << 3) as i32 >> 3
    // For -1: val=0x1FFF_FFFF, shifted=0xFFFF_FFF8, as i32=-8, >>3 = -1
    let sv = r.sval();
    assert_eq!(sv, -1);
}

#[test]
fn ref_slot_creation() {
    let r = Ref::Slot(7);
    assert!(!r.is_tmp());
    assert!(!r.is_none());
    assert_eq!(r.val(), 7);
}

#[test]
fn ref_equality() {
    assert_eq!(Ref::Tmp(TmpId(1)), Ref::Tmp(TmpId(1)));
    assert_ne!(Ref::Tmp(TmpId(1)), Ref::Tmp(TmpId(2)));
    assert_ne!(Ref::Tmp(TmpId(1)), Ref::Con(ConId(1)));
    assert_ne!(Ref::R, Ref::Int(0));
}

#[test]
fn ref_special_constants() {
    // UNDEF is Con(0), CON_Z is Con(1)
    assert_eq!(Ref::UNDEF, Ref::Con(ConId(0)));
    assert_eq!(Ref::CON_Z, Ref::Con(ConId(1)));
    assert_ne!(Ref::UNDEF, Ref::CON_Z);
}

#[test]
fn ref_rtype_values() {
    assert_eq!(Ref::R.rtype(), -1);
    assert_eq!(Ref::Tmp(TmpId(0)).rtype(), 0);
    assert_eq!(Ref::Con(ConId(0)).rtype(), 1);
    assert_eq!(Ref::Int(0).rtype(), 2);
}

// ---------------------------------------------------------------------------
// BSet operations
// ---------------------------------------------------------------------------

#[test]
fn bset_new_empty() {
    let bs = BSet::new(64);
    assert_eq!(bs.count(), 0);
    for i in 0..64 {
        assert!(!bs.has(i));
    }
}

#[test]
fn bset_set_and_has() {
    let mut bs = BSet::new(128);
    bs.set(0);
    bs.set(63);
    bs.set(64);
    bs.set(127);

    assert!(bs.has(0));
    assert!(bs.has(63));
    assert!(bs.has(64));
    assert!(bs.has(127));
    assert!(!bs.has(1));
    assert!(!bs.has(62));
    assert!(!bs.has(65));
}

#[test]
fn bset_clr() {
    let mut bs = BSet::new(64);
    bs.set(10);
    assert!(bs.has(10));
    bs.clr(10);
    assert!(!bs.has(10));
}

#[test]
fn bset_count() {
    let mut bs = BSet::new(256);
    assert_eq!(bs.count(), 0);
    bs.set(0);
    assert_eq!(bs.count(), 1);
    bs.set(100);
    assert_eq!(bs.count(), 2);
    bs.set(255);
    assert_eq!(bs.count(), 3);
    bs.set(0); // setting again shouldn't change count
    assert_eq!(bs.count(), 3);
}

#[test]
fn bset_zero() {
    let mut bs = BSet::new(64);
    bs.set(0);
    bs.set(31);
    bs.set(63);
    assert_eq!(bs.count(), 3);
    bs.zero();
    assert_eq!(bs.count(), 0);
    assert!(!bs.has(0));
    assert!(!bs.has(31));
    assert!(!bs.has(63));
}

#[test]
fn bset_union() {
    let mut a = BSet::new(64);
    let mut b = BSet::new(64);
    a.set(0);
    a.set(10);
    b.set(10);
    b.set(20);
    a.union(&b);
    assert!(a.has(0));
    assert!(a.has(10));
    assert!(a.has(20));
    assert_eq!(a.count(), 3);
}

#[test]
fn bset_inter() {
    let mut a = BSet::new(64);
    let mut b = BSet::new(64);
    a.set(0);
    a.set(10);
    a.set(20);
    b.set(10);
    b.set(20);
    b.set(30);
    a.inter(&b);
    assert!(!a.has(0));
    assert!(a.has(10));
    assert!(a.has(20));
    assert!(!a.has(30));
    assert_eq!(a.count(), 2);
}

#[test]
fn bset_diff() {
    let mut a = BSet::new(64);
    let mut b = BSet::new(64);
    a.set(0);
    a.set(10);
    a.set(20);
    b.set(10);
    b.set(30);
    a.diff(&b);
    assert!(a.has(0));
    assert!(!a.has(10));
    assert!(a.has(20));
    assert_eq!(a.count(), 2);
}

#[test]
fn bset_equal() {
    let mut a = BSet::new(64);
    let mut b = BSet::new(64);
    assert!(a.equal(&b));

    a.set(5);
    assert!(!a.equal(&b));

    b.set(5);
    assert!(a.equal(&b));
}

#[test]
fn bset_copy_from() {
    let mut a = BSet::new(64);
    let mut b = BSet::new(64);
    b.set(3);
    b.set(42);
    a.copy_from(&b);
    assert!(a.has(3));
    assert!(a.has(42));
    assert!(a.equal(&b));
}

#[test]
fn bset_iter() {
    let mut bs = BSet::new(128);
    bs.set(5);
    bs.set(63);
    bs.set(64);
    bs.set(100);

    let bits: Vec<u32> = bs.iter().collect();
    assert_eq!(bits, vec![5, 63, 64, 100]);
}

#[test]
fn bset_iter_empty() {
    let bs = BSet::new(64);
    let bits: Vec<u32> = bs.iter().collect();
    assert!(bits.is_empty());
}

// ---------------------------------------------------------------------------
// Op range helpers
// ---------------------------------------------------------------------------

#[test]
fn op_is_store() {
    assert!(Op::Storeb.is_store());
    assert!(Op::Storeh.is_store());
    assert!(Op::Storew.is_store());
    assert!(Op::Storel.is_store());
    assert!(Op::Stores.is_store());
    assert!(Op::Stored.is_store());
    assert!(!Op::Add.is_store());
    assert!(!Op::Loadsb.is_store());
}

#[test]
fn op_is_load() {
    assert!(Op::Loadsb.is_load());
    assert!(Op::Loadub.is_load());
    assert!(Op::Loadsh.is_load());
    assert!(Op::Loaduh.is_load());
    assert!(Op::Loadsw.is_load());
    assert!(Op::Loaduw.is_load());
    assert!(Op::Load.is_load());
    assert!(!Op::Add.is_load());
    assert!(!Op::Storew.is_load());
}

#[test]
fn op_is_ext() {
    assert!(Op::Extsb.is_ext());
    assert!(Op::Extub.is_ext());
    assert!(Op::Extsh.is_ext());
    assert!(Op::Extuh.is_ext());
    assert!(Op::Extsw.is_ext());
    assert!(Op::Extuw.is_ext());
    assert!(!Op::Add.is_ext());
}

#[test]
fn op_is_par() {
    assert!(Op::Par.is_par());
    assert!(Op::Parc.is_par());
    assert!(Op::Pare.is_par());
    assert!(!Op::Add.is_par());
    assert!(!Op::Arg.is_par());
}

#[test]
fn op_is_arg() {
    assert!(Op::Arg.is_arg());
    assert!(Op::Argc.is_arg());
    assert!(Op::Arge.is_arg());
    assert!(Op::Argv.is_arg());
    assert!(!Op::Add.is_arg());
    assert!(!Op::Par.is_arg());
}

#[test]
fn op_is_parbh() {
    assert!(Op::Parsb.is_parbh());
    assert!(Op::Parub.is_parbh());
    assert!(Op::Parsh.is_parbh());
    assert!(Op::Paruh.is_parbh());
    assert!(!Op::Par.is_parbh());
    assert!(!Op::Add.is_parbh());
}

#[test]
fn op_is_argbh() {
    assert!(Op::Argsb.is_argbh());
    assert!(Op::Argub.is_argbh());
    assert!(Op::Argsh.is_argbh());
    assert!(Op::Arguh.is_argbh());
    assert!(!Op::Arg.is_argbh());
    assert!(!Op::Add.is_argbh());
}

#[test]
fn op_is_cmp() {
    assert!(Op::Ceqw.is_cmp());
    assert!(Op::Cslew.is_cmp());
    assert!(Op::Ceqd.is_cmp());
    assert!(!Op::Add.is_cmp());
    assert!(!Op::Storew.is_cmp());
}

#[test]
fn op_is_flag() {
    assert!(Op::Flagieq.is_flag());
    assert!(Op::Flagfuo.is_flag());
    assert!(!Op::Add.is_flag());
    assert!(!Op::Ceqw.is_flag());
}

#[test]
fn op_is_alloc() {
    assert!(Op::Alloc4.is_alloc());
    assert!(Op::Alloc8.is_alloc());
    assert!(Op::Alloc16.is_alloc());
    assert!(!Op::Add.is_alloc());
}

#[test]
fn op_can_fold() {
    assert!(Op::Add.can_fold());
    assert!(Op::Sub.can_fold());
    assert!(Op::Mul.can_fold());
    // Stores/loads typically cannot fold
    assert!(!Op::Storew.can_fold());
}

// ---------------------------------------------------------------------------
// Cls helpers
// ---------------------------------------------------------------------------

#[test]
fn cls_is_wide() {
    assert!(!Cls::Kw.is_wide());
    assert!(Cls::Kl.is_wide());
    assert!(!Cls::Ks.is_wide());
    assert!(Cls::Kd.is_wide());
    assert!(!Cls::Kx.is_wide());
}

#[test]
fn cls_is_float() {
    assert!(!Cls::Kw.is_float());
    assert!(!Cls::Kl.is_float());
    assert!(Cls::Ks.is_float());
    assert!(Cls::Kd.is_float());
    assert!(!Cls::Kx.is_float());
}

#[test]
fn cls_base() {
    // Integer classes have base 0, float classes have base 1
    assert_eq!(Cls::Kw.base(), 0);
    assert_eq!(Cls::Kl.base(), 0);
    assert_eq!(Cls::Ks.base(), 1);
    assert_eq!(Cls::Kd.base(), 1);
    assert_eq!(Cls::Kx.base(), -1);
}

#[test]
fn cls_from_i8() {
    assert_eq!(Cls::from_i8(0), Cls::Kw);
    assert_eq!(Cls::from_i8(1), Cls::Kl);
    assert_eq!(Cls::from_i8(2), Cls::Ks);
    assert_eq!(Cls::from_i8(3), Cls::Kd);
    assert_eq!(Cls::from_i8(-1), Cls::Kx);
    assert_eq!(Cls::from_i8(99), Cls::Kx);
}

#[test]
fn cls_as_index() {
    assert_eq!(Cls::Kw.as_index(), 0);
    assert_eq!(Cls::Kl.as_index(), 1);
    assert_eq!(Cls::Ks.as_index(), 2);
    assert_eq!(Cls::Kd.as_index(), 3);
}

#[test]
#[should_panic]
fn cls_kx_as_index_panics() {
    let _ = Cls::Kx.as_index();
}

#[test]
fn cls_default_is_kx() {
    assert_eq!(Cls::default(), Cls::Kx);
}

// ---------------------------------------------------------------------------
// Jmp helpers
// ---------------------------------------------------------------------------

#[test]
fn jmp_is_ret() {
    assert!(Jmp::Retw.is_ret());
    assert!(Jmp::Retl.is_ret());
    assert!(Jmp::Rets.is_ret());
    assert!(Jmp::Retd.is_ret());
    assert!(Jmp::Retc.is_ret());
    assert!(Jmp::Ret0.is_ret());
    assert!(!Jmp::Jmp_.is_ret());
    assert!(!Jmp::Jnz.is_ret());
}

#[test]
fn jmp_default() {
    assert_eq!(Jmp::default(), Jmp::Jxxx);
}

// ---------------------------------------------------------------------------
// Ins struct
// ---------------------------------------------------------------------------

#[test]
fn ins_default() {
    let ins = Ins::default();
    assert_eq!(ins.to, Ref::R);
    assert_eq!(ins.op, Op::Nop);
}

// ---------------------------------------------------------------------------
// Id newtypes
// ---------------------------------------------------------------------------

#[test]
fn blk_id_basics() {
    let id = BlkId(5);
    assert_eq!(id.index(), 5);
    assert!(!id.is_none());
    assert!(BlkId::NONE.is_none());
}

#[test]
fn tmp_id_basics() {
    let id = TmpId(10);
    assert_eq!(id.0, 10);
}

#[test]
fn con_id_basics() {
    let id = ConId(3);
    assert_eq!(id.0, 3);
}

#[test]
fn mem_id_basics() {
    let id = MemId(7);
    assert_eq!(id.0, 7);
}

// ---------------------------------------------------------------------------
// ConBits
// ---------------------------------------------------------------------------

#[test]
fn con_bits_from_i64() {
    let bits = ConBits::from_i64(42);
    assert_eq!(bits.i(), 42);
}

#[test]
fn con_bits_from_f64() {
    let bits = ConBits::from_f64(3.14);
    let val = bits.d();
    assert!((val - 3.14).abs() < 1e-15);
}

#[test]
fn con_bits_from_f32() {
    let bits = ConBits::from_f32(2.5);
    let val = bits.s();
    assert!((val - 2.5).abs() < 1e-7);
}

#[test]
fn con_bits_zero() {
    let bits = ConBits::from_i64(0);
    assert_eq!(bits.i(), 0);
    assert_eq!(bits.d(), 0.0);
}

// ---------------------------------------------------------------------------
// isreg helper
// ---------------------------------------------------------------------------

#[test]
fn isreg_function() {
    // R is not a register
    assert!(!isreg(Ref::R));
    // Con is not a register
    assert!(!isreg(Ref::Con(ConId(0))));
    // Int is not a register
    assert!(!isreg(Ref::Int(0)));
    // Tmp(0) is the special Tmp0, not a register
    assert!(!isreg(Ref::Tmp(TmpId(0))));
}

// ---------------------------------------------------------------------------
// Phi struct
// ---------------------------------------------------------------------------

#[test]
fn phi_narg() {
    let phi = Phi {
        cls: Cls::Kw,
        to: Ref::Tmp(TmpId(1)),
        args: vec![Ref::Int(0), Ref::Tmp(TmpId(2))],
        blks: vec![BlkId(0), BlkId(1)],
    };
    assert_eq!(phi.narg(), 2);
}

// ---------------------------------------------------------------------------
// Blk struct
// ---------------------------------------------------------------------------

#[test]
fn blk_default_fields() {
    let blk = Blk::default();
    assert!(blk.phi.is_empty());
    assert!(blk.ins.is_empty());
    assert_eq!(blk.s1, None);
    assert_eq!(blk.s2, None);
    assert!(blk.pred.is_empty());
    assert!(blk.fron.is_empty());
}

// ---------------------------------------------------------------------------
// CmpI / CmpF enums
// ---------------------------------------------------------------------------

#[test]
fn cmpneg_involution() {
    // cmpneg(cmpneg(x)) == x for comparison operations
    use rqbe::ir::cmpneg;
    let cmps = [
        CmpI::Cieq,
        CmpI::Cine,
        CmpI::Cisle,
        CmpI::Cisgt,
        CmpI::Cislt,
        CmpI::Cisge,
        CmpI::Ciule,
        CmpI::Ciugt,
        CmpI::Ciult,
        CmpI::Ciuge,
    ];
    for &c in &cmps {
        let n = cmpneg(c as u16);
        let nn = cmpneg(n);
        assert_eq!(nn, c as u16, "cmpneg is not an involution for {:?}", c);
    }
}

#[test]
fn cmpop_involution() {
    // cmpop(cmpop(x)) == x for comparison operations (swap operands)
    use rqbe::ir::cmpop;
    let cmps = [
        CmpI::Cieq,
        CmpI::Cine,
        CmpI::Cisle,
        CmpI::Cisgt,
        CmpI::Cislt,
        CmpI::Cisge,
        CmpI::Ciule,
        CmpI::Ciugt,
        CmpI::Ciult,
        CmpI::Ciuge,
    ];
    for &c in &cmps {
        let s = cmpop(c as u16);
        let ss = cmpop(s);
        assert_eq!(ss, c as u16, "cmpop is not an involution for {:?}", c);
    }
}

// ---------------------------------------------------------------------------
// argcls helper
// ---------------------------------------------------------------------------

#[test]
fn argcls_for_add() {
    // The add operation should return an appropriate class for its operands.
    // argcls takes an Ins and an operand index.
    let ins = Ins {
        op: Op::Add,
        cls: Cls::Kw,
        to: Ref::Tmp(TmpId(1)),
        arg: [Ref::Tmp(TmpId(2)), Ref::Tmp(TmpId(3))],
    };
    let cls_0 = argcls(&ins, 0);
    assert_eq!(cls_0, Cls::Kw);
}

// ---------------------------------------------------------------------------
// clsmerge helper
// ---------------------------------------------------------------------------

#[test]
fn clsmerge_same() {
    let mut c = Cls::Kw;
    assert!(clsmerge(&mut c, Cls::Kw));
    assert_eq!(c, Cls::Kw);
}

#[test]
fn clsmerge_from_kx() {
    let mut c = Cls::Kx;
    assert!(clsmerge(&mut c, Cls::Kl));
    assert_eq!(c, Cls::Kl);
}

#[test]
fn clsmerge_conflict() {
    // Merging incompatible classes should fail
    let mut c = Cls::Kw;
    assert!(!clsmerge(&mut c, Cls::Kd));
}
