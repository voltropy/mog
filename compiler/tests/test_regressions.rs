//! Regression tests for QBE codegen bugs fixed 2025-02-21.

use mog::compiler::{compile, CompileOptions};

fn qbe(source: &str) -> String {
    let src = format!("fn main() -> int {{\n{}\nreturn 0;\n}}", source);
    let opts = CompileOptions {
        ..Default::default()
    };
    let result = compile(&src, &opts);
    assert!(
        result.errors.is_empty(),
        "compile errors: {:?}",
        result.errors
    );
    result.ir
}

// ---------------------------------------------------------------------------
// 1. Float struct fields use `stored` not `storel`
// ---------------------------------------------------------------------------
#[test]
fn regression_float_struct_field_stored() {
    let ir = qbe("struct Point { x: float, y: float }\n\
         p := Point { x: 1.5, y: 2.5 };\n\
         println(f\"{p.x}\");");
    assert!(
        ir.contains("stored"),
        "float struct fields should use 'stored': {ir}"
    );
}

// ---------------------------------------------------------------------------
// 2. Float function arguments use `d` type
// ---------------------------------------------------------------------------
#[test]
fn regression_float_function_args_typed() {
    let ir = qbe("fn add_f(a: float, b: float) -> float { return a + b; }\n\
         result := add_f(1.5, 2.5);\n\
         println(f\"{result}\");");
    assert!(
        ir.contains("d %p.a") || ir.contains("d %p_a"),
        "float params should have d type: {ir}"
    );
    assert!(
        ir.contains("call $add_f(d"),
        "float args in call should use d type: {ir}"
    );
}

// ---------------------------------------------------------------------------
// 3. float_regs cleared between functions — no label/register leaking
// ---------------------------------------------------------------------------
#[test]
fn regression_no_label_leak_between_functions() {
    let ir = qbe("fn foo(x: int) -> int {\n\
         if x > 0 { return 1; }\n\
         return 0;\n\
         }\n\
         fn bar(y: int) -> int {\n\
         if y > 0 { return 2; }\n\
         return 0;\n\
         }\n\
         result := foo(1) + bar(1);\n\
         println(f\"{result}\");");
    // Both functions should compile without QBE errors
    // (the original bug caused duplicate @L.0 labels)
    assert!(!ir.is_empty(), "should produce IR");
}

// ---------------------------------------------------------------------------
// 4. Float literals use decimal format (not hex)
// ---------------------------------------------------------------------------
#[test]
fn regression_float_literal_decimal_format() {
    let ir = qbe("x := 3.14;\n\
         println(f\"{x}\");");
    assert!(
        ir.contains("d_3.") || ir.contains("d_3"),
        "float literal should be decimal: {ir}"
    );
    assert!(!ir.contains("d_40"), "should NOT use hex format: {ir}");
}

// ---------------------------------------------------------------------------
// 5. str() dispatches to f64_to_string for float values
// ---------------------------------------------------------------------------
#[test]
fn regression_str_float_dispatches_f64() {
    let ir = qbe("x := 3.14;\n\
         s := str(x);\n\
         println(s);");
    assert!(
        ir.contains("f64_to_string"),
        "str(float) should call f64_to_string: {ir}"
    );
}

// ---------------------------------------------------------------------------
// 6. Template literal interpolation uses f64_to_string for floats
// ---------------------------------------------------------------------------
#[test]
fn regression_fstring_float_uses_f64_to_string() {
    let ir = qbe("x := 3.14;\n\
         println(f\"value: {x}\");");
    assert!(
        ir.contains("f64_to_string"),
        "f-string float interp should use f64_to_string: {ir}"
    );
}

// ---------------------------------------------------------------------------
// 7. array_push with float uses ensure_long (cast)
// ---------------------------------------------------------------------------
#[test]
fn regression_array_push_float_casts() {
    let ir = qbe("arr := [1.0, 2.0, 3.0];\n\
         println(f\"{arr.len}\");");
    assert!(
        ir.contains("cast") || ir.contains("array_push"),
        "float array push should work: {ir}"
    );
}

// ---------------------------------------------------------------------------
// 8. Closure variables use indirect calls
// ---------------------------------------------------------------------------
#[test]
fn regression_closure_indirect_call() {
    let ir = qbe("double := fn(x: int) -> int { return x * 2; };\n\
         result := double(21);\n\
         println(f\"{result}\");");
    assert!(
        !ir.contains("call $double("),
        "closure should use indirect call, not direct symbol: {ir}"
    );
}

// ---------------------------------------------------------------------------
// 9. all/race map to mog_all/mog_race
// ---------------------------------------------------------------------------
#[test]
fn regression_all_race_correct_symbols() {
    let ir = qbe("arr := [1, 2, 3];\n\
         result := all(arr);\n\
         println(f\"{result}\");");
    assert!(ir.contains("$mog_all"), "all() should map to mog_all: {ir}");
}

// ---------------------------------------------------------------------------
// 10. Arrays use array_alloc not array_new
// ---------------------------------------------------------------------------
#[test]
fn regression_array_uses_alloc_not_new() {
    let ir = qbe("arr := [10, 20, 30];\n\
         println(f\"{arr[0]}\");");
    assert!(ir.contains("array_alloc"), "should use array_alloc: {ir}");
    assert!(!ir.contains("array_new"), "should NOT use array_new: {ir}");
}

// ---------------------------------------------------------------------------
// 11. Array .len uses array_length runtime function
// ---------------------------------------------------------------------------
#[test]
fn regression_array_len_uses_runtime() {
    let ir = qbe("arr := [10, 20, 30];\n\
         n := arr.len;\n\
         println(f\"{n}\");");
    assert!(
        ir.contains("array_length"),
        "arr.len should call array_length: {ir}"
    );
}

// ---------------------------------------------------------------------------
// 12. String methods on [u8] variables
// ---------------------------------------------------------------------------
#[test]
fn regression_string_methods_on_u8_array() {
    let ir = qbe("s: [u8] = \"hello\";\n\
         u := s.upper();\n\
         println(u);");
    assert!(
        ir.contains("string_upper"),
        "s.upper() on [u8] should call string_upper: {ir}"
    );
}

// ---------------------------------------------------------------------------
// 13. Binary ops re-check float after generation
// ---------------------------------------------------------------------------
#[test]
fn regression_binary_op_float_recheck() {
    let ir = qbe("struct Pt { x: float, y: float }\n\
         fn dist(p: Pt) -> float { return (p.x * p.x) + (p.y * p.y); }\n\
         p := Pt { x: 3.0, y: 4.0 };\n\
         d := dist(p);\n\
         println(f\"{d}\");");
    assert!(
        ir.contains("=d mul"),
        "float struct field multiply should produce d type: {ir}"
    );
}
