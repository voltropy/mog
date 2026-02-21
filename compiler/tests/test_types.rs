//! Comprehensive type system tests — ported from types.test.ts & type_system.test.ts

use mog::types::*;
use std::collections::BTreeMap;

// ── helpers ──────────────────────────────────────────────────────────────────

fn fields(entries: &[(&str, Type)]) -> BTreeMap<String, Type> {
    entries
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn point_struct() -> Type {
    Type::struct_type(
        "Point",
        fields(&[("x", Type::float()), ("y", Type::float())]),
    )
}

fn particle_struct() -> Type {
    Type::struct_type(
        "Particle",
        fields(&[
            ("x", Type::Float(FloatKind::F64)),
            ("y", Type::Float(FloatKind::F64)),
            ("mass", Type::Float(FloatKind::F64)),
            ("id", Type::Integer(IntegerKind::I32)),
            ("active", Type::Unsigned(UnsignedKind::U8)),
        ]),
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// 1.  TYPE GUARD / PREDICATE FUNCTIONS — every variant
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn is_integer_true_for_all_integer_kinds() {
    for k in [
        IntegerKind::I8,
        IntegerKind::I16,
        IntegerKind::I32,
        IntegerKind::I64,
        IntegerKind::I128,
        IntegerKind::I256,
    ] {
        assert!(
            Type::Integer(k).is_integer(),
            "I{} should be integer",
            k.bits()
        );
    }
}

#[test]
fn is_integer_false_for_non_integers() {
    assert!(!Type::float().is_integer());
    assert!(!Type::Unsigned(UnsignedKind::U32).is_integer());
    assert!(!Type::string().is_integer());
    assert!(!Type::bool().is_integer());
    assert!(!Type::void().is_integer());
    assert!(!Type::array(Type::int()).is_integer());
    assert!(!Type::map(Type::string(), Type::int()).is_integer());
    assert!(!point_struct().is_integer());
    assert!(!Type::result(Type::int()).is_integer());
    assert!(!Type::optional(Type::int()).is_integer());
    assert!(!Type::future(Type::int()).is_integer());
    assert!(!Type::pointer(None).is_integer());
    assert!(!Type::custom("X").is_integer());
    assert!(!Type::tensor(Type::float(), None).is_integer());
    assert!(!Type::function(vec![], Type::void()).is_integer());
}

#[test]
fn is_unsigned_true_for_all_unsigned_kinds() {
    for k in [
        UnsignedKind::U8,
        UnsignedKind::U16,
        UnsignedKind::U32,
        UnsignedKind::U64,
        UnsignedKind::U128,
        UnsignedKind::U256,
    ] {
        assert!(
            Type::Unsigned(k).is_unsigned(),
            "U{} should be unsigned",
            k.bits()
        );
    }
}

#[test]
fn is_unsigned_false_for_non_unsigned() {
    assert!(!Type::int().is_unsigned());
    assert!(!Type::float().is_unsigned());
    assert!(!Type::string().is_unsigned());
    assert!(!Type::bool().is_unsigned());
    assert!(!Type::void().is_unsigned());
    assert!(!Type::array(Type::int()).is_unsigned());
    assert!(!Type::result(Type::int()).is_unsigned());
}

#[test]
fn is_float_true_for_all_float_kinds() {
    for k in [
        FloatKind::F8,
        FloatKind::F16,
        FloatKind::F32,
        FloatKind::F64,
        FloatKind::F128,
        FloatKind::F256,
        FloatKind::Bf16,
    ] {
        assert!(Type::Float(k).is_float(), "{:?} should be float", k);
    }
}

#[test]
fn is_float_false_for_non_floats() {
    assert!(!Type::int().is_float());
    assert!(!Type::Unsigned(UnsignedKind::U32).is_float());
    assert!(!Type::string().is_float());
    assert!(!Type::bool().is_float());
    assert!(!Type::void().is_float());
    assert!(!Type::array(Type::float()).is_float());
}

#[test]
fn is_bool_only_for_bool() {
    assert!(Type::bool().is_bool());
    assert!(!Type::int().is_bool());
    assert!(!Type::float().is_bool());
    assert!(!Type::string().is_bool());
    assert!(!Type::void().is_bool());
}

#[test]
fn is_string_only_for_string() {
    assert!(Type::string().is_string());
    assert!(!Type::int().is_string());
    assert!(!Type::bool().is_string());
    assert!(!Type::void().is_string());
}

#[test]
fn is_void_only_for_void() {
    assert!(Type::void().is_void());
    assert!(!Type::int().is_void());
    assert!(!Type::string().is_void());
    assert!(!Type::bool().is_void());
}

#[test]
fn is_pointer_for_pointer_variants() {
    assert!(Type::pointer(None).is_pointer());
    assert!(Type::pointer(Some(Type::int())).is_pointer());
    assert!(!Type::int().is_pointer());
    assert!(!Type::array(Type::int()).is_pointer());
}

#[test]
fn is_array_only_for_arrays() {
    assert!(Type::array(Type::int()).is_array());
    assert!(Type::array_with_dims(Type::int(), vec![3, 4]).is_array());
    assert!(!Type::int().is_array());
    assert!(!Type::map(Type::string(), Type::int()).is_array());
}

#[test]
fn is_map_only_for_maps() {
    assert!(Type::map(Type::string(), Type::int()).is_map());
    assert!(Type::map(Type::int(), Type::float()).is_map());
    assert!(!Type::int().is_map());
    assert!(!Type::array(Type::int()).is_map());
}

#[test]
fn is_struct_only_for_structs() {
    assert!(point_struct().is_struct());
    assert!(Type::struct_type("Empty", BTreeMap::new()).is_struct());
    assert!(!Type::int().is_struct());
    assert!(!Type::array(Type::int()).is_struct());
    assert!(!Type::map(Type::int(), Type::float()).is_struct());
}

#[test]
fn is_aos_only_for_aos() {
    let aos = Type::AOS {
        element_type: Box::new(point_struct()),
        size: None,
    };
    assert!(aos.is_aos());
    assert!(!Type::int().is_aos());
    assert!(!point_struct().is_aos());
    assert!(!Type::array(Type::int()).is_aos());
}

#[test]
fn is_soa_only_for_soa() {
    let soa = Type::SOA {
        struct_type: Box::new(point_struct()),
        capacity: Some(100),
    };
    assert!(soa.is_soa());
    assert!(!Type::int().is_soa());
    assert!(!point_struct().is_soa());
}

#[test]
fn is_custom_only_for_custom() {
    assert!(Type::custom("Foo").is_custom());
    assert!(Type::custom("MyType").is_custom());
    assert!(!Type::int().is_custom());
    assert!(!Type::string().is_custom());
}

#[test]
fn is_type_alias_only_for_aliases() {
    assert!(Type::type_alias("MyInt", Type::int()).is_type_alias());
    assert!(!Type::int().is_type_alias());
    assert!(!Type::custom("X").is_type_alias());
}

#[test]
fn is_function_only_for_functions() {
    assert!(Type::function(vec![Type::int()], Type::bool()).is_function());
    assert!(Type::function(vec![], Type::void()).is_function());
    assert!(!Type::int().is_function());
    assert!(!Type::custom("fn").is_function());
}

#[test]
fn is_tensor_only_for_tensors() {
    assert!(Type::tensor(Type::float(), None).is_tensor());
    assert!(Type::tensor(Type::Float(FloatKind::Bf16), Some(vec![3, 4])).is_tensor());
    assert!(!Type::int().is_tensor());
    assert!(!Type::array(Type::float()).is_tensor());
}

#[test]
fn is_result_only_for_results() {
    assert!(Type::result(Type::int()).is_result());
    assert!(Type::result(Type::string()).is_result());
    assert!(!Type::int().is_result());
    assert!(!Type::optional(Type::int()).is_result());
}

#[test]
fn is_optional_only_for_optionals() {
    assert!(Type::optional(Type::int()).is_optional());
    assert!(Type::optional(Type::string()).is_optional());
    assert!(!Type::int().is_optional());
    assert!(!Type::result(Type::int()).is_optional());
}

#[test]
fn is_future_only_for_futures() {
    assert!(Type::future(Type::int()).is_future());
    assert!(Type::future(Type::string()).is_future());
    assert!(!Type::int().is_future());
    assert!(!Type::optional(Type::int()).is_future());
}

// ═══════════════════════════════════════════════════════════════════════════
// 2.  is_numeric & is_signed
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn is_numeric_true_for_all_numeric_kinds() {
    // integers
    assert!(Type::Integer(IntegerKind::I8).is_numeric());
    assert!(Type::Integer(IntegerKind::I64).is_numeric());
    assert!(Type::Integer(IntegerKind::I256).is_numeric());
    // unsigned
    assert!(Type::Unsigned(UnsignedKind::U8).is_numeric());
    assert!(Type::Unsigned(UnsignedKind::U64).is_numeric());
    assert!(Type::Unsigned(UnsignedKind::U256).is_numeric());
    // float
    assert!(Type::Float(FloatKind::F8).is_numeric());
    assert!(Type::Float(FloatKind::F64).is_numeric());
    assert!(Type::Float(FloatKind::Bf16).is_numeric());
}

#[test]
fn is_numeric_false_for_non_numerics() {
    assert!(!Type::string().is_numeric());
    assert!(!Type::bool().is_numeric());
    assert!(!Type::void().is_numeric());
    assert!(!Type::array(Type::int()).is_numeric());
    assert!(!Type::map(Type::string(), Type::int()).is_numeric());
    assert!(!point_struct().is_numeric());
    assert!(!Type::pointer(None).is_numeric());
    assert!(!Type::custom("X").is_numeric());
    assert!(!Type::result(Type::int()).is_numeric());
    assert!(!Type::optional(Type::int()).is_numeric());
    assert!(!Type::future(Type::int()).is_numeric());
    assert!(!Type::tensor(Type::float(), None).is_numeric());
    assert!(!Type::function(vec![], Type::void()).is_numeric());
}

#[test]
fn is_signed_for_integer_variants() {
    assert!(Type::Integer(IntegerKind::I8).is_signed());
    assert!(Type::Integer(IntegerKind::I16).is_signed());
    assert!(Type::Integer(IntegerKind::I32).is_signed());
    assert!(Type::Integer(IntegerKind::I64).is_signed());
    assert!(Type::Integer(IntegerKind::I128).is_signed());
    assert!(Type::Integer(IntegerKind::I256).is_signed());
}

#[test]
fn is_signed_false_for_unsigned_and_float() {
    assert!(!Type::Unsigned(UnsignedKind::U8).is_signed());
    assert!(!Type::Unsigned(UnsignedKind::U16).is_signed());
    assert!(!Type::Unsigned(UnsignedKind::U32).is_signed());
    assert!(!Type::Unsigned(UnsignedKind::U64).is_signed());
    assert!(!Type::Unsigned(UnsignedKind::U128).is_signed());
    assert!(!Type::Unsigned(UnsignedKind::U256).is_signed());
    assert!(!Type::Float(FloatKind::F32).is_signed());
    assert!(!Type::Float(FloatKind::F64).is_signed());
    assert!(!Type::Float(FloatKind::Bf16).is_signed());
}

// ═══════════════════════════════════════════════════════════════════════════
// 3.  bits() — Type-level
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bits_for_all_integer_kinds() {
    assert_eq!(Type::Integer(IntegerKind::I8).bits(), Some(8));
    assert_eq!(Type::Integer(IntegerKind::I16).bits(), Some(16));
    assert_eq!(Type::Integer(IntegerKind::I32).bits(), Some(32));
    assert_eq!(Type::Integer(IntegerKind::I64).bits(), Some(64));
    assert_eq!(Type::Integer(IntegerKind::I128).bits(), Some(128));
    assert_eq!(Type::Integer(IntegerKind::I256).bits(), Some(256));
}

#[test]
fn bits_for_all_unsigned_kinds() {
    assert_eq!(Type::Unsigned(UnsignedKind::U8).bits(), Some(8));
    assert_eq!(Type::Unsigned(UnsignedKind::U16).bits(), Some(16));
    assert_eq!(Type::Unsigned(UnsignedKind::U32).bits(), Some(32));
    assert_eq!(Type::Unsigned(UnsignedKind::U64).bits(), Some(64));
    assert_eq!(Type::Unsigned(UnsignedKind::U128).bits(), Some(128));
    assert_eq!(Type::Unsigned(UnsignedKind::U256).bits(), Some(256));
}

#[test]
fn bits_for_all_float_kinds() {
    assert_eq!(Type::Float(FloatKind::F8).bits(), Some(8));
    assert_eq!(Type::Float(FloatKind::F16).bits(), Some(16));
    assert_eq!(Type::Float(FloatKind::F32).bits(), Some(32));
    assert_eq!(Type::Float(FloatKind::F64).bits(), Some(64));
    assert_eq!(Type::Float(FloatKind::F128).bits(), Some(128));
    assert_eq!(Type::Float(FloatKind::F256).bits(), Some(256));
    assert_eq!(Type::Float(FloatKind::Bf16).bits(), Some(16));
}

#[test]
fn bits_for_bool_is_one() {
    assert_eq!(Type::Bool.bits(), Some(1));
}

#[test]
fn bits_none_for_non_numeric_types() {
    assert_eq!(Type::string().bits(), None);
    assert_eq!(Type::void().bits(), None);
    assert_eq!(Type::array(Type::int()).bits(), None);
    assert_eq!(Type::map(Type::string(), Type::int()).bits(), None);
    assert_eq!(point_struct().bits(), None);
    assert_eq!(Type::pointer(None).bits(), None);
    assert_eq!(Type::custom("X").bits(), None);
    assert_eq!(Type::result(Type::int()).bits(), None);
    assert_eq!(Type::optional(Type::int()).bits(), None);
    assert_eq!(Type::future(Type::int()).bits(), None);
    assert_eq!(Type::tensor(Type::float(), None).bits(), None);
    assert_eq!(Type::function(vec![], Type::void()).bits(), None);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4.  from_str for all numeric kinds — including edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn integer_kind_from_str_all() {
    assert_eq!(IntegerKind::from_str("i8"), Some(IntegerKind::I8));
    assert_eq!(IntegerKind::from_str("i16"), Some(IntegerKind::I16));
    assert_eq!(IntegerKind::from_str("i32"), Some(IntegerKind::I32));
    assert_eq!(IntegerKind::from_str("i64"), Some(IntegerKind::I64));
    assert_eq!(IntegerKind::from_str("i128"), Some(IntegerKind::I128));
    assert_eq!(IntegerKind::from_str("i256"), Some(IntegerKind::I256));
}

#[test]
fn integer_kind_from_str_rejects_invalid() {
    assert_eq!(IntegerKind::from_str("i512"), None);
    assert_eq!(IntegerKind::from_str("u32"), None);
    assert_eq!(IntegerKind::from_str("f64"), None);
    assert_eq!(IntegerKind::from_str("int"), None);
    assert_eq!(IntegerKind::from_str(""), None);
    assert_eq!(IntegerKind::from_str("I32"), None); // case-sensitive
    assert_eq!(IntegerKind::from_str("i0"), None);
    assert_eq!(IntegerKind::from_str("i"), None);
}

#[test]
fn unsigned_kind_from_str_all() {
    assert_eq!(UnsignedKind::from_str("u8"), Some(UnsignedKind::U8));
    assert_eq!(UnsignedKind::from_str("u16"), Some(UnsignedKind::U16));
    assert_eq!(UnsignedKind::from_str("u32"), Some(UnsignedKind::U32));
    assert_eq!(UnsignedKind::from_str("u64"), Some(UnsignedKind::U64));
    assert_eq!(UnsignedKind::from_str("u128"), Some(UnsignedKind::U128));
    assert_eq!(UnsignedKind::from_str("u256"), Some(UnsignedKind::U256));
}

#[test]
fn unsigned_kind_from_str_rejects_invalid() {
    assert_eq!(UnsignedKind::from_str("u512"), None);
    assert_eq!(UnsignedKind::from_str("i32"), None);
    assert_eq!(UnsignedKind::from_str("f32"), None);
    assert_eq!(UnsignedKind::from_str("uint"), None);
    assert_eq!(UnsignedKind::from_str(""), None);
    assert_eq!(UnsignedKind::from_str("U32"), None); // case-sensitive
}

#[test]
fn float_kind_from_str_all() {
    assert_eq!(FloatKind::from_str("f8"), Some(FloatKind::F8));
    assert_eq!(FloatKind::from_str("f16"), Some(FloatKind::F16));
    assert_eq!(FloatKind::from_str("f32"), Some(FloatKind::F32));
    assert_eq!(FloatKind::from_str("f64"), Some(FloatKind::F64));
    assert_eq!(FloatKind::from_str("f128"), Some(FloatKind::F128));
    assert_eq!(FloatKind::from_str("f256"), Some(FloatKind::F256));
    assert_eq!(FloatKind::from_str("bf16"), Some(FloatKind::Bf16));
}

#[test]
fn float_kind_from_str_rejects_invalid() {
    assert_eq!(FloatKind::from_str("f512"), None);
    assert_eq!(FloatKind::from_str("i32"), None);
    assert_eq!(FloatKind::from_str("float"), None);
    assert_eq!(FloatKind::from_str(""), None);
    assert_eq!(FloatKind::from_str("BF16"), None); // case-sensitive
    assert_eq!(FloatKind::from_str("bfloat16"), None);
    assert_eq!(FloatKind::from_str("f0"), None);
}

// ═══════════════════════════════════════════════════════════════════════════
// 5.  BF16-specific behaviour
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bf16_has_16_bits() {
    assert_eq!(FloatKind::Bf16.bits(), 16);
    assert_eq!(Type::Float(FloatKind::Bf16).bits(), Some(16));
}

#[test]
fn bf16_is_float_not_integer() {
    let bf = Type::Float(FloatKind::Bf16);
    assert!(bf.is_float());
    assert!(bf.is_numeric());
    assert!(!bf.is_integer());
    assert!(!bf.is_unsigned());
    assert!(!bf.is_signed());
}

#[test]
fn bf16_display() {
    assert_eq!(format!("{}", FloatKind::Bf16), "bf16");
    assert_eq!(format!("{}", Type::Float(FloatKind::Bf16)), "bf16");
}

#[test]
fn bf16_same_type_with_itself() {
    let a = Type::Float(FloatKind::Bf16);
    let b = Type::Float(FloatKind::Bf16);
    assert!(a.same_type(&b));
}

#[test]
fn bf16_not_same_as_f16() {
    let bf = Type::Float(FloatKind::Bf16);
    let f16 = Type::Float(FloatKind::F16);
    assert!(!bf.same_type(&f16));
}

#[test]
fn bf16_compatible_with_other_floats() {
    let bf = Type::Float(FloatKind::Bf16);
    assert!(bf.compatible_with(&Type::Float(FloatKind::F32)));
    assert!(bf.compatible_with(&Type::Float(FloatKind::F64)));
    assert!(bf.compatible_with(&Type::Float(FloatKind::F16)));
}

// ═══════════════════════════════════════════════════════════════════════════
// 6.  same_type — complex nested types
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn same_type_identical_primitives() {
    assert!(Type::int().same_type(&Type::int()));
    assert!(Type::float().same_type(&Type::float()));
    assert!(Type::string().same_type(&Type::string()));
    assert!(Type::bool().same_type(&Type::bool()));
    assert!(Type::void().same_type(&Type::void()));
}

#[test]
fn same_type_different_primitives() {
    assert!(!Type::int().same_type(&Type::float()));
    assert!(!Type::string().same_type(&Type::bool()));
    assert!(!Type::int().same_type(&Type::void()));
}

#[test]
fn same_type_different_bit_widths() {
    assert!(!Type::Integer(IntegerKind::I32).same_type(&Type::Integer(IntegerKind::I64)));
    assert!(!Type::Unsigned(UnsignedKind::U8).same_type(&Type::Unsigned(UnsignedKind::U16)));
    assert!(!Type::Float(FloatKind::F32).same_type(&Type::Float(FloatKind::F64)));
}

#[test]
fn same_type_integer_vs_unsigned_same_bits() {
    assert!(!Type::Integer(IntegerKind::I32).same_type(&Type::Unsigned(UnsignedKind::U32)));
    assert!(!Type::Integer(IntegerKind::I64).same_type(&Type::Unsigned(UnsignedKind::U64)));
}

#[test]
fn same_type_arrays_same_element() {
    let a = Type::array(Type::int());
    let b = Type::array(Type::int());
    assert!(a.same_type(&b));
}

#[test]
fn same_type_arrays_different_element() {
    let a = Type::array(Type::int());
    let b = Type::array(Type::float());
    assert!(!a.same_type(&b));
}

#[test]
fn same_type_array_of_arrays() {
    let inner_a = Type::array(Type::Integer(IntegerKind::I32));
    let inner_b = Type::array(Type::Integer(IntegerKind::I32));
    let a = Type::array(inner_a);
    let b = Type::array(inner_b);
    assert!(a.same_type(&b));
}

#[test]
fn same_type_array_of_arrays_different_inner() {
    let a = Type::array(Type::array(Type::Integer(IntegerKind::I32)));
    let b = Type::array(Type::array(Type::Float(FloatKind::F32)));
    assert!(!a.same_type(&b));
}

#[test]
fn same_type_maps_same_kv() {
    let a = Type::map(Type::string(), Type::int());
    let b = Type::map(Type::string(), Type::int());
    assert!(a.same_type(&b));
}

#[test]
fn same_type_maps_different_value() {
    let a = Type::map(Type::string(), Type::int());
    let b = Type::map(Type::string(), Type::float());
    assert!(!a.same_type(&b));
}

#[test]
fn same_type_maps_different_key() {
    let a = Type::map(Type::Integer(IntegerKind::I32), Type::float());
    let b = Type::map(Type::Unsigned(UnsignedKind::U32), Type::float());
    assert!(!a.same_type(&b));
}

#[test]
fn same_type_map_of_maps() {
    let inner = Type::map(Type::string(), Type::int());
    let a = Type::map(Type::string(), inner.clone());
    let b = Type::map(Type::string(), inner);
    assert!(a.same_type(&b));
}

#[test]
fn same_type_structs_same_name() {
    let a = point_struct();
    let b = point_struct();
    assert!(a.same_type(&b));
}

#[test]
fn same_type_structs_different_name() {
    let a = Type::struct_type("Point", fields(&[("x", Type::float())]));
    let b = Type::struct_type("Vec2", fields(&[("x", Type::float())]));
    assert!(!a.same_type(&b));
}

#[test]
fn same_type_struct_with_struct_fields() {
    let inner = point_struct();
    let a = Type::struct_type("Nested", fields(&[("pt", inner.clone())]));
    let b = Type::struct_type("Nested", fields(&[("pt", inner)]));
    assert!(a.same_type(&b));
}

#[test]
fn same_type_result_types() {
    assert!(Type::result(Type::int()).same_type(&Type::result(Type::int())));
    assert!(!Type::result(Type::int()).same_type(&Type::result(Type::float())));
}

#[test]
fn same_type_optional_types() {
    assert!(Type::optional(Type::string()).same_type(&Type::optional(Type::string())));
    assert!(!Type::optional(Type::int()).same_type(&Type::optional(Type::float())));
}

#[test]
fn same_type_future_types() {
    assert!(Type::future(Type::int()).same_type(&Type::future(Type::int())));
    assert!(!Type::future(Type::int()).same_type(&Type::future(Type::string())));
}

#[test]
fn same_type_function_types() {
    let a = Type::function(vec![Type::int(), Type::float()], Type::bool());
    let b = Type::function(vec![Type::int(), Type::float()], Type::bool());
    assert!(a.same_type(&b));
}

#[test]
fn same_type_function_different_params() {
    let a = Type::function(vec![Type::int()], Type::bool());
    let b = Type::function(vec![Type::float()], Type::bool());
    assert!(!a.same_type(&b));
}

#[test]
fn same_type_function_different_return() {
    let a = Type::function(vec![Type::int()], Type::bool());
    let b = Type::function(vec![Type::int()], Type::void());
    assert!(!a.same_type(&b));
}

#[test]
fn same_type_function_different_arity() {
    let a = Type::function(vec![Type::int()], Type::void());
    let b = Type::function(vec![Type::int(), Type::int()], Type::void());
    assert!(!a.same_type(&b));
}

#[test]
fn same_type_tensor_types() {
    let a = Type::tensor(Type::float(), Some(vec![3, 4]));
    let b = Type::tensor(Type::float(), Some(vec![3, 4]));
    assert!(a.same_type(&b));
}

#[test]
fn same_type_tensor_different_dtype() {
    let a = Type::tensor(Type::float(), None);
    let b = Type::tensor(Type::Float(FloatKind::F32), None);
    assert!(!a.same_type(&b));
}

#[test]
fn same_type_custom_types() {
    assert!(Type::custom("Foo").same_type(&Type::custom("Foo")));
    assert!(!Type::custom("Foo").same_type(&Type::custom("Bar")));
}

#[test]
fn same_type_pointer_types() {
    assert!(Type::pointer(None).same_type(&Type::pointer(None)));
    assert!(Type::pointer(Some(Type::int())).same_type(&Type::pointer(Some(Type::int()))));
    assert!(!Type::pointer(Some(Type::int())).same_type(&Type::pointer(Some(Type::float()))));
    assert!(!Type::pointer(None).same_type(&Type::pointer(Some(Type::int()))));
}

#[test]
fn same_type_cross_category_always_false() {
    assert!(!Type::int().same_type(&Type::array(Type::int())));
    assert!(!Type::array(Type::int()).same_type(&Type::map(Type::int(), Type::int())));
    assert!(!Type::result(Type::int()).same_type(&Type::optional(Type::int())));
    assert!(!Type::future(Type::int()).same_type(&Type::result(Type::int())));
    assert!(!Type::function(vec![], Type::void()).same_type(&Type::int()));
    assert!(!Type::tensor(Type::float(), None).same_type(&Type::array(Type::float())));
    assert!(!Type::custom("X").same_type(&Type::string()));
}

// ═══════════════════════════════════════════════════════════════════════════
// 7.  compatible_with — edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compatible_same_type_always_true() {
    assert!(Type::int().compatible_with(&Type::int()));
    assert!(Type::float().compatible_with(&Type::float()));
    assert!(Type::string().compatible_with(&Type::string()));
    assert!(Type::bool().compatible_with(&Type::bool()));
    assert!(Type::void().compatible_with(&Type::void()));
}

#[test]
fn compatible_int_widths() {
    // int<->int same signedness = compatible
    assert!(Type::Integer(IntegerKind::I8).compatible_with(&Type::Integer(IntegerKind::I64)));
    assert!(Type::Integer(IntegerKind::I64).compatible_with(&Type::Integer(IntegerKind::I8)));
    assert!(Type::Integer(IntegerKind::I32).compatible_with(&Type::Integer(IntegerKind::I256)));
}

#[test]
fn compatible_unsigned_widths() {
    assert!(Type::Unsigned(UnsignedKind::U8).compatible_with(&Type::Unsigned(UnsignedKind::U64)));
    assert!(Type::Unsigned(UnsignedKind::U64).compatible_with(&Type::Unsigned(UnsignedKind::U8)));
    assert!(Type::Unsigned(UnsignedKind::U128).compatible_with(&Type::Unsigned(UnsignedKind::U256)));
}

#[test]
fn compatible_float_widths() {
    assert!(Type::Float(FloatKind::F32).compatible_with(&Type::Float(FloatKind::F64)));
    assert!(Type::Float(FloatKind::F64).compatible_with(&Type::Float(FloatKind::F32)));
    assert!(Type::Float(FloatKind::F8).compatible_with(&Type::Float(FloatKind::F256)));
    assert!(Type::Float(FloatKind::Bf16).compatible_with(&Type::Float(FloatKind::F64)));
}

#[test]
fn compatible_int_to_float() {
    assert!(Type::int().compatible_with(&Type::float()));
    assert!(Type::Integer(IntegerKind::I32).compatible_with(&Type::Float(FloatKind::F32)));
    assert!(Type::Integer(IntegerKind::I8).compatible_with(&Type::Float(FloatKind::F64)));
}

#[test]
fn compatible_unsigned_to_float() {
    assert!(Type::Unsigned(UnsignedKind::U32).compatible_with(&Type::float()));
    assert!(Type::Unsigned(UnsignedKind::U8).compatible_with(&Type::Float(FloatKind::F64)));
}

#[test]
fn not_compatible_float_to_int() {
    assert!(!Type::float().compatible_with(&Type::int()));
    assert!(!Type::Float(FloatKind::F64).compatible_with(&Type::Integer(IntegerKind::I64)));
}

#[test]
fn not_compatible_int_to_unsigned() {
    assert!(!Type::int().compatible_with(&Type::Unsigned(UnsignedKind::U32)));
    assert!(!Type::Integer(IntegerKind::I32).compatible_with(&Type::Unsigned(UnsignedKind::U32)));
}

#[test]
fn not_compatible_unsigned_to_int() {
    assert!(!Type::Unsigned(UnsignedKind::U32).compatible_with(&Type::int()));
}

#[test]
fn not_compatible_string_to_numeric() {
    assert!(!Type::string().compatible_with(&Type::int()));
    assert!(!Type::string().compatible_with(&Type::float()));
    assert!(!Type::string().compatible_with(&Type::Unsigned(UnsignedKind::U32)));
}

#[test]
fn not_compatible_bool_to_numeric() {
    assert!(!Type::bool().compatible_with(&Type::int()));
    assert!(!Type::bool().compatible_with(&Type::float()));
}

#[test]
fn not_compatible_array_to_int() {
    assert!(!Type::array(Type::int()).compatible_with(&Type::int()));
}

#[test]
fn not_compatible_struct_to_map() {
    assert!(!point_struct().compatible_with(&Type::map(Type::string(), Type::float())));
}

#[test]
fn compatible_through_alias() {
    let alias = Type::type_alias("MyInt", Type::int());
    assert!(alias.compatible_with(&Type::int()));
    assert!(alias.compatible_with(&Type::Integer(IntegerKind::I32))); // int<->int
    assert!(alias.compatible_with(&Type::float())); // int -> float
}

// ═══════════════════════════════════════════════════════════════════════════
// 8.  Type compatibility matrix — every pair
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compatibility_matrix_exhaustive() {
    let types = vec![
        ("int", Type::int()),
        ("uint", Type::Unsigned(UnsignedKind::U32)),
        ("float", Type::float()),
        ("bool", Type::bool()),
        ("string", Type::string()),
        ("void", Type::void()),
        ("array", Type::array(Type::int())),
        ("map", Type::map(Type::string(), Type::int())),
        ("struct", point_struct()),
        ("result", Type::result(Type::int())),
        ("optional", Type::optional(Type::int())),
        ("future", Type::future(Type::int())),
        ("pointer", Type::pointer(None)),
        ("custom", Type::custom("X")),
        ("tensor", Type::tensor(Type::float(), None)),
        ("function", Type::function(vec![], Type::void())),
    ];

    for (name_a, a) in &types {
        for (name_b, b) in &types {
            let compat = a.compatible_with(b);
            // Same type should always be compatible with itself
            if name_a == name_b {
                assert!(compat, "{name_a} should be compatible with itself");
            }
            // These specific cross-type compatibilities are true
            let expected_compat = name_a == name_b
                || (*name_a == "int" && *name_b == "float")
                || (*name_a == "uint" && *name_b == "float");
            assert_eq!(
                compat, expected_compat,
                "{name_a} -> {name_b}: expected {expected_compat}, got {compat}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 9.  resolve_alias — including nested alias chains
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn resolve_alias_simple() {
    let alias = Type::type_alias("MyInt", Type::int());
    assert_eq!(*alias.resolve_alias(), Type::int());
}

#[test]
fn resolve_alias_nested_chain() {
    let inner = Type::type_alias("InnerInt", Type::Integer(IntegerKind::I32));
    let outer = Type::type_alias("OuterInt", inner);
    assert_eq!(*outer.resolve_alias(), Type::Integer(IntegerKind::I32));
}

#[test]
fn resolve_alias_triple_chain() {
    let a = Type::type_alias("A", Type::Float(FloatKind::F32));
    let b = Type::type_alias("B", a);
    let c = Type::type_alias("C", b);
    assert_eq!(*c.resolve_alias(), Type::Float(FloatKind::F32));
}

#[test]
fn resolve_alias_owned_consumes() {
    let alias = Type::type_alias("MyFloat", Type::float());
    assert_eq!(alias.resolve_alias_owned(), Type::float());
}

#[test]
fn resolve_alias_owned_nested() {
    let a = Type::type_alias("A", Type::Integer(IntegerKind::I16));
    let b = Type::type_alias("B", a);
    assert_eq!(b.resolve_alias_owned(), Type::Integer(IntegerKind::I16));
}

#[test]
fn same_type_sees_through_aliases() {
    let alias = Type::type_alias("Alias64", Type::int());
    assert!(alias.same_type(&Type::int()));
    assert!(Type::int().same_type(&alias));
}

#[test]
fn same_type_two_aliases_same_underlying() {
    let a = Type::type_alias("A", Type::Integer(IntegerKind::I64));
    let b = Type::type_alias("B", Type::Integer(IntegerKind::I64));
    assert!(a.same_type(&b));
}

#[test]
fn same_type_two_aliases_different_underlying() {
    let a = Type::type_alias("A", Type::int());
    let b = Type::type_alias("B", Type::float());
    assert!(!a.same_type(&b));
}

#[test]
fn resolve_alias_non_alias_returns_self() {
    let t = Type::int();
    assert_eq!(*t.resolve_alias(), Type::int());
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. ResultType construction and comparison
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn result_type_construction() {
    let r = Type::result(Type::int());
    assert!(r.is_result());
    assert!(!r.is_optional());
    assert!(!r.is_future());
}

#[test]
fn result_type_nested() {
    let r = Type::result(Type::result(Type::string()));
    assert!(r.is_result());
    if let Type::Result(inner) = &r {
        assert!(inner.is_result());
    }
}

#[test]
fn result_same_type_inner_match() {
    let a = Type::result(Type::Integer(IntegerKind::I32));
    let b = Type::result(Type::Integer(IntegerKind::I32));
    assert!(a.same_type(&b));
}

#[test]
fn result_not_same_different_inner() {
    let a = Type::result(Type::int());
    let b = Type::result(Type::string());
    assert!(!a.same_type(&b));
}

#[test]
fn result_display() {
    assert_eq!(format!("{}", Type::result(Type::int())), "Result<i64>");
    assert_eq!(
        format!("{}", Type::result(Type::string())),
        "Result<string>"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. OptionalType
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn optional_construction() {
    let o = Type::optional(Type::int());
    assert!(o.is_optional());
    assert!(!o.is_result());
    assert!(!o.is_future());
}

#[test]
fn optional_nested() {
    let o = Type::optional(Type::optional(Type::string()));
    assert!(o.is_optional());
    if let Type::Optional(inner) = &o {
        assert!(inner.is_optional());
    }
}

#[test]
fn optional_display() {
    assert_eq!(format!("{}", Type::optional(Type::int())), "?i64");
    assert_eq!(format!("{}", Type::optional(Type::string())), "?string");
    assert_eq!(
        format!("{}", Type::optional(Type::optional(Type::int()))),
        "??i64"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. FutureType
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn future_construction() {
    let f = Type::future(Type::int());
    assert!(f.is_future());
    assert!(!f.is_result());
    assert!(!f.is_optional());
}

#[test]
fn future_display() {
    assert_eq!(format!("{}", Type::future(Type::int())), "Future<i64>");
    assert_eq!(
        format!("{}", Type::future(Type::string())),
        "Future<string>"
    );
}

#[test]
fn future_nested() {
    let f = Type::future(Type::future(Type::int()));
    assert!(f.is_future());
    assert_eq!(format!("{}", f), "Future<Future<i64>>");
}

// ═══════════════════════════════════════════════════════════════════════════
// 13. FunctionType with various param/return types
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn function_no_params_void_return() {
    let f = Type::function(vec![], Type::void());
    assert!(f.is_function());
    assert_eq!(format!("{f}"), "fn() -> void");
}

#[test]
fn function_single_param() {
    let f = Type::function(vec![Type::int()], Type::bool());
    assert_eq!(format!("{f}"), "fn(i64) -> bool");
}

#[test]
fn function_multiple_params() {
    let f = Type::function(
        vec![Type::int(), Type::float(), Type::string()],
        Type::bool(),
    );
    assert_eq!(format!("{f}"), "fn(i64, f64, string) -> bool");
}

#[test]
fn function_returning_array() {
    let f = Type::function(vec![Type::int()], Type::array(Type::float()));
    assert_eq!(format!("{f}"), "fn(i64) -> [f64]");
}

#[test]
fn function_returning_optional() {
    let f = Type::function(vec![Type::string()], Type::optional(Type::int()));
    assert_eq!(format!("{f}"), "fn(string) -> ?i64");
}

#[test]
fn function_returning_result() {
    let f = Type::function(vec![], Type::result(Type::string()));
    assert_eq!(format!("{f}"), "fn() -> Result<string>");
}

#[test]
fn function_with_function_param() {
    let callback = Type::function(vec![Type::int()], Type::void());
    let f = Type::function(vec![callback], Type::bool());
    assert_eq!(format!("{f}"), "fn(fn(i64) -> void) -> bool");
}

// ═══════════════════════════════════════════════════════════════════════════
// 14. TensorType with dtype and shapes
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn tensor_no_shape() {
    let t = Type::tensor(Type::float(), None);
    assert!(t.is_tensor());
    assert_eq!(format!("{t}"), "tensor<f64>");
}

#[test]
fn tensor_with_shape() {
    let t = Type::tensor(Type::Float(FloatKind::F32), Some(vec![3, 224, 224]));
    assert!(t.is_tensor());
    assert_eq!(format!("{t}"), "tensor<f32>");
}

#[test]
fn tensor_bf16_dtype() {
    let t = Type::tensor(Type::Float(FloatKind::Bf16), Some(vec![1, 768]));
    assert!(t.is_tensor());
    assert_eq!(format!("{t}"), "tensor<bf16>");
}

#[test]
fn tensor_same_type_checks_dtype() {
    let a = Type::tensor(Type::Float(FloatKind::F32), Some(vec![3, 4]));
    let b = Type::tensor(Type::Float(FloatKind::F32), Some(vec![5, 6]));
    // same_type only checks dtype, not shape
    assert!(a.same_type(&b));
}

#[test]
fn tensor_different_dtype_not_same() {
    let a = Type::tensor(Type::Float(FloatKind::F32), None);
    let b = Type::tensor(Type::Float(FloatKind::F64), None);
    assert!(!a.same_type(&b));
}

// ═══════════════════════════════════════════════════════════════════════════
// 15. SOAType and AOSType
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn aos_dynamic_construction() {
    let aos = Type::AOS {
        element_type: Box::new(point_struct()),
        size: None,
    };
    assert!(aos.is_aos());
    assert!(!aos.is_soa());
}

#[test]
fn aos_fixed_size() {
    let aos = Type::AOS {
        element_type: Box::new(point_struct()),
        size: Some(100),
    };
    assert!(aos.is_aos());
}

#[test]
fn aos_display_dynamic() {
    let aos = Type::AOS {
        element_type: Box::new(point_struct()),
        size: None,
    };
    assert_eq!(format!("{aos}"), "[Point]");
}

#[test]
fn aos_display_fixed() {
    let aos = Type::AOS {
        element_type: Box::new(point_struct()),
        size: Some(50),
    };
    assert_eq!(format!("{aos}"), "[Point; 50]");
}

#[test]
fn soa_construction_with_capacity() {
    let soa = Type::SOA {
        struct_type: Box::new(point_struct()),
        capacity: Some(100),
    };
    assert!(soa.is_soa());
    assert!(!soa.is_aos());
}

#[test]
fn soa_dynamic_no_capacity() {
    let soa = Type::SOA {
        struct_type: Box::new(point_struct()),
        capacity: None,
    };
    assert!(soa.is_soa());
}

#[test]
fn soa_display_with_capacity() {
    let soa = Type::SOA {
        struct_type: Box::new(point_struct()),
        capacity: Some(100),
    };
    assert_eq!(format!("{soa}"), "soa Point[100]");
}

#[test]
fn soa_display_no_capacity() {
    let soa = Type::SOA {
        struct_type: Box::new(point_struct()),
        capacity: None,
    };
    assert_eq!(format!("{soa}"), "soa Point");
}

#[test]
fn aos_not_compatible_with_soa() {
    let aos = Type::AOS {
        element_type: Box::new(point_struct()),
        size: None,
    };
    let soa = Type::SOA {
        struct_type: Box::new(point_struct()),
        capacity: Some(100),
    };
    assert!(!aos.same_type(&soa));
    assert!(!aos.compatible_with(&soa));
}

#[test]
fn struct_not_compatible_with_aos() {
    let st = point_struct();
    let aos = Type::AOS {
        element_type: Box::new(point_struct()),
        size: None,
    };
    assert!(!st.same_type(&aos));
    assert!(!st.compatible_with(&aos));
}

// ═══════════════════════════════════════════════════════════════════════════
// 16. Display for all complex type combinations
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn display_all_integer_kinds() {
    assert_eq!(format!("{}", Type::Integer(IntegerKind::I8)), "i8");
    assert_eq!(format!("{}", Type::Integer(IntegerKind::I16)), "i16");
    assert_eq!(format!("{}", Type::Integer(IntegerKind::I32)), "i32");
    assert_eq!(format!("{}", Type::Integer(IntegerKind::I64)), "i64");
    assert_eq!(format!("{}", Type::Integer(IntegerKind::I128)), "i128");
    assert_eq!(format!("{}", Type::Integer(IntegerKind::I256)), "i256");
}

#[test]
fn display_all_unsigned_kinds() {
    assert_eq!(format!("{}", Type::Unsigned(UnsignedKind::U8)), "u8");
    assert_eq!(format!("{}", Type::Unsigned(UnsignedKind::U16)), "u16");
    assert_eq!(format!("{}", Type::Unsigned(UnsignedKind::U32)), "u32");
    assert_eq!(format!("{}", Type::Unsigned(UnsignedKind::U64)), "u64");
    assert_eq!(format!("{}", Type::Unsigned(UnsignedKind::U128)), "u128");
    assert_eq!(format!("{}", Type::Unsigned(UnsignedKind::U256)), "u256");
}

#[test]
fn display_all_float_kinds() {
    assert_eq!(format!("{}", Type::Float(FloatKind::F8)), "f8");
    assert_eq!(format!("{}", Type::Float(FloatKind::F16)), "f16");
    assert_eq!(format!("{}", Type::Float(FloatKind::F32)), "f32");
    assert_eq!(format!("{}", Type::Float(FloatKind::F64)), "f64");
    assert_eq!(format!("{}", Type::Float(FloatKind::F128)), "f128");
    assert_eq!(format!("{}", Type::Float(FloatKind::F256)), "f256");
    assert_eq!(format!("{}", Type::Float(FloatKind::Bf16)), "bf16");
}

#[test]
fn display_primitives() {
    assert_eq!(format!("{}", Type::bool()), "bool");
    assert_eq!(format!("{}", Type::string()), "string");
    assert_eq!(format!("{}", Type::void()), "void");
}

#[test]
fn display_pointer_variants() {
    assert_eq!(format!("{}", Type::pointer(None)), "ptr");
    assert_eq!(format!("{}", Type::pointer(Some(Type::int()))), "ptr<i64>");
    assert_eq!(
        format!("{}", Type::pointer(Some(Type::string()))),
        "ptr<string>"
    );
}

#[test]
fn display_array() {
    assert_eq!(format!("{}", Type::array(Type::int())), "[i64]");
    assert_eq!(format!("{}", Type::array(Type::float())), "[f64]");
    assert_eq!(format!("{}", Type::array(Type::string())), "[string]");
}

#[test]
fn display_map() {
    assert_eq!(
        format!("{}", Type::map(Type::string(), Type::int())),
        "{string: i64}"
    );
    assert_eq!(
        format!(
            "{}",
            Type::map(
                Type::Unsigned(UnsignedKind::U8),
                Type::Integer(IntegerKind::I32)
            )
        ),
        "{u8: i32}"
    );
}

#[test]
fn display_nested_array_of_maps() {
    let nested = Type::array(Type::map(Type::string(), Type::int()));
    assert_eq!(format!("{nested}"), "[{string: i64}]");
}

#[test]
fn display_nested_map_of_arrays() {
    let nested = Type::map(Type::string(), Type::array(Type::float()));
    assert_eq!(format!("{nested}"), "{string: [f64]}");
}

#[test]
fn display_struct() {
    assert_eq!(format!("{}", point_struct()), "Point");
}

#[test]
fn display_custom() {
    assert_eq!(format!("{}", Type::custom("MyType")), "MyType");
}

#[test]
fn display_type_alias() {
    assert_eq!(
        format!("{}", Type::type_alias("MyInt", Type::int())),
        "MyInt"
    );
}

#[test]
fn display_function() {
    assert_eq!(
        format!(
            "{}",
            Type::function(vec![Type::int(), Type::float()], Type::bool())
        ),
        "fn(i64, f64) -> bool"
    );
}

#[test]
fn display_tensor() {
    assert_eq!(
        format!("{}", Type::tensor(Type::float(), None)),
        "tensor<f64>"
    );
    assert_eq!(
        format!(
            "{}",
            Type::tensor(Type::Float(FloatKind::Bf16), Some(vec![3, 4]))
        ),
        "tensor<bf16>"
    );
}

#[test]
fn display_result_optional_future() {
    assert_eq!(format!("{}", Type::result(Type::int())), "Result<i64>");
    assert_eq!(format!("{}", Type::optional(Type::int())), "?i64");
    assert_eq!(format!("{}", Type::future(Type::int())), "Future<i64>");
}

#[test]
fn display_deeply_nested() {
    // Result<Optional<Future<[{string: i64}]>>>
    let inner = Type::array(Type::map(Type::string(), Type::int()));
    let f = Type::future(inner);
    let o = Type::optional(f);
    let r = Type::result(o);
    assert_eq!(format!("{r}"), "Result<?Future<[{string: i64}]>>");
}

// ═══════════════════════════════════════════════════════════════════════════
// 17. Predicate on compound types — is_numeric etc. on wrapped types
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn compound_types_not_numeric() {
    assert!(!Type::array(Type::int()).is_numeric());
    assert!(!Type::map(Type::int(), Type::float()).is_numeric());
    assert!(!Type::result(Type::int()).is_numeric());
    assert!(!Type::optional(Type::float()).is_numeric());
    assert!(!Type::future(Type::int()).is_numeric());
    assert!(!Type::tensor(Type::float(), None).is_numeric());
    assert!(!Type::function(vec![Type::int()], Type::int()).is_numeric());
    let aos = Type::AOS {
        element_type: Box::new(point_struct()),
        size: None,
    };
    assert!(!aos.is_numeric());
    let soa = Type::SOA {
        struct_type: Box::new(point_struct()),
        capacity: Some(10),
    };
    assert!(!soa.is_numeric());
}

#[test]
fn compound_types_not_signed() {
    assert!(!Type::array(Type::int()).is_signed());
    assert!(!Type::result(Type::int()).is_signed());
    assert!(!Type::optional(Type::int()).is_signed());
    assert!(!Type::future(Type::int()).is_signed());
}

// ═══════════════════════════════════════════════════════════════════════════
// 18. Struct fields
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn struct_with_mixed_fields() {
    let p = particle_struct();
    if let Type::Struct { name, fields } = &p {
        assert_eq!(name, "Particle");
        assert_eq!(fields.len(), 5);
        assert_eq!(fields.get("x"), Some(&Type::Float(FloatKind::F64)));
        assert_eq!(fields.get("id"), Some(&Type::Integer(IntegerKind::I32)));
        assert_eq!(
            fields.get("active"),
            Some(&Type::Unsigned(UnsignedKind::U8))
        );
    } else {
        panic!("Expected Struct");
    }
}

#[test]
fn struct_empty_fields() {
    let s = Type::struct_type("Empty", BTreeMap::new());
    if let Type::Struct { fields, .. } = &s {
        assert_eq!(fields.len(), 0);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 19. Constructors via convenience functions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn convenience_constructors() {
    assert_eq!(Type::int(), Type::Integer(IntegerKind::I64));
    assert_eq!(Type::float(), Type::Float(FloatKind::F64));
    assert_eq!(Type::string(), Type::String);
    assert_eq!(Type::bool(), Type::Bool);
    assert_eq!(Type::void(), Type::Void);
}

#[test]
fn array_constructor_no_dims() {
    if let Type::Array {
        element_type,
        dimensions,
    } = Type::array(Type::int())
    {
        assert_eq!(*element_type, Type::int());
        assert!(dimensions.is_empty());
    } else {
        panic!("Expected Array");
    }
}

#[test]
fn array_constructor_with_dims() {
    if let Type::Array {
        element_type,
        dimensions,
    } = Type::array_with_dims(Type::int(), vec![3, 4])
    {
        assert_eq!(*element_type, Type::int());
        assert_eq!(dimensions, vec![3, 4]);
    } else {
        panic!("Expected Array");
    }
}

#[test]
fn map_constructor() {
    if let Type::Map {
        key_type,
        value_type,
    } = Type::map(Type::string(), Type::float())
    {
        assert_eq!(*key_type, Type::string());
        assert_eq!(*value_type, Type::float());
    } else {
        panic!("Expected Map");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 20. Kind-level Display formatting
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn integer_kind_display() {
    assert_eq!(format!("{}", IntegerKind::I8), "i8");
    assert_eq!(format!("{}", IntegerKind::I16), "i16");
    assert_eq!(format!("{}", IntegerKind::I32), "i32");
    assert_eq!(format!("{}", IntegerKind::I64), "i64");
    assert_eq!(format!("{}", IntegerKind::I128), "i128");
    assert_eq!(format!("{}", IntegerKind::I256), "i256");
}

#[test]
fn unsigned_kind_display() {
    assert_eq!(format!("{}", UnsignedKind::U8), "u8");
    assert_eq!(format!("{}", UnsignedKind::U16), "u16");
    assert_eq!(format!("{}", UnsignedKind::U32), "u32");
    assert_eq!(format!("{}", UnsignedKind::U64), "u64");
    assert_eq!(format!("{}", UnsignedKind::U128), "u128");
    assert_eq!(format!("{}", UnsignedKind::U256), "u256");
}

#[test]
fn float_kind_display() {
    assert_eq!(format!("{}", FloatKind::F8), "f8");
    assert_eq!(format!("{}", FloatKind::F16), "f16");
    assert_eq!(format!("{}", FloatKind::F32), "f32");
    assert_eq!(format!("{}", FloatKind::F64), "f64");
    assert_eq!(format!("{}", FloatKind::F128), "f128");
    assert_eq!(format!("{}", FloatKind::F256), "f256");
    assert_eq!(format!("{}", FloatKind::Bf16), "bf16");
}

// ═══════════════════════════════════════════════════════════════════════════
// 21. Equality and Clone
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn type_clone_equals_original() {
    let types = vec![
        Type::int(),
        Type::float(),
        Type::string(),
        Type::bool(),
        Type::void(),
        Type::array(Type::int()),
        Type::map(Type::string(), Type::int()),
        point_struct(),
        Type::result(Type::int()),
        Type::optional(Type::string()),
        Type::future(Type::float()),
        Type::pointer(Some(Type::int())),
        Type::custom("Foo"),
        Type::type_alias("A", Type::int()),
        Type::function(vec![Type::int()], Type::bool()),
        Type::tensor(Type::float(), Some(vec![3, 4])),
    ];
    for t in types {
        let cloned = t.clone();
        assert_eq!(t, cloned);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 22. Edge: same_type vs PartialEq (dimensions ignored by same_type)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn same_type_ignores_array_dimensions() {
    let a = Type::array_with_dims(Type::int(), vec![3, 4]);
    let b = Type::array_with_dims(Type::int(), vec![5, 6]);
    // same_type only checks element_type for arrays
    assert!(a.same_type(&b));
    // but PartialEq checks everything
    assert_ne!(a, b);
}

#[test]
fn same_type_ignores_tensor_shape() {
    let a = Type::tensor(Type::Float(FloatKind::F32), Some(vec![1, 2, 3]));
    let b = Type::tensor(Type::Float(FloatKind::F32), Some(vec![4, 5]));
    assert!(a.same_type(&b));
}
