//! Parser unit tests for rqbe.
//!
//! Tests the QBE IL parser with small hand-crafted snippets.
//! These exercise parse() and verify that the resulting IR structures
//! are correct.

use rqbe::parse::{parse, ParseResult};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse QBE IL and assert it produces the expected number of types,
/// data blocks, and functions.
fn parse_expect(input: &str, n_types: usize, n_data: usize, n_funcs: usize) -> ParseResult {
    let result = parse(input);
    assert_eq!(
        result.types.len(),
        n_types,
        "expected {} types, got {}",
        n_types,
        result.types.len()
    );
    assert_eq!(
        result.data.len(),
        n_data,
        "expected {} data groups, got {}",
        n_data,
        result.data.len()
    );
    assert_eq!(
        result.functions.len(),
        n_funcs,
        "expected {} functions, got {}",
        n_funcs,
        result.functions.len()
    );
    result
}

// ---------------------------------------------------------------------------
// Empty / minimal inputs
// ---------------------------------------------------------------------------

#[test]
fn parse_empty_input() {
    let result = parse("");
    assert!(result.types.is_empty());
    assert!(result.data.is_empty());
    assert!(result.functions.is_empty());
}

#[test]
fn parse_whitespace_only() {
    let result = parse("  \n\n  \t  \n");
    assert!(result.types.is_empty());
    assert!(result.data.is_empty());
    assert!(result.functions.is_empty());
}

#[test]
fn parse_comments_only() {
    let result = parse("# this is a comment\n# another comment\n");
    assert!(result.types.is_empty());
    assert!(result.data.is_empty());
    assert!(result.functions.is_empty());
}

// ---------------------------------------------------------------------------
// Function parsing
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn parse_empty_function() {
    let input = "function $empty() {\n@start\n    ret\n}\n";
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "empty");
}

#[test]
#[ignore]
fn parse_function_with_return_type() {
    let input = "function w $retw() {\n@start\n    ret 42\n}\n";
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "retw");
}

#[test]
#[ignore]
fn parse_function_with_params() {
    let input = "function w $add(w %a, w %b) {\n@start\n    %c =w add %a, %b\n    ret %c\n}\n";
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "add");
    // Should have temporaries for %a, %b, %c
    assert!(
        f.tmps.len() >= 3,
        "expected at least 3 tmps, got {}",
        f.tmps.len()
    );
}

#[test]
#[ignore]
fn parse_function_with_arithmetic() {
    let input = r#"
function w $arith(w %x) {
@start
    %a =w add %x, 1
    %b =w sub %a, 2
    %c =w mul %b, 3
    %d =w div %c, 4
    %e =w rem %d, 5
    ret %e
}
"#;
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "arith");

    // Verify that instructions were parsed. The start block should have
    // instructions for add, sub, mul, div, rem.
    let start_blk = &f.blks[f.start.0 as usize];
    assert!(
        start_blk.ins.len() >= 5,
        "expected at least 5 instructions, got {}",
        start_blk.ins.len()
    );
}

#[test]
#[ignore]
fn parse_function_with_control_flow() {
    let input = r#"
function w $max(w %a, w %b) {
@start
    %c =w csgtw %a, %b
    jnz %c, @left, @right
@left
    ret %a
@right
    ret %b
}
"#;
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "max");
    // Should have 3 blocks: start, left, right
    assert!(
        f.blks.len() >= 3,
        "expected at least 3 blocks, got {}",
        f.blks.len()
    );
}

#[test]
#[ignore]
fn parse_phi_nodes() {
    let input = r#"
function w $loop_sum(w %n) {
@start
    jmp @loop
@loop
    %i =w phi @start 0, @body %i1
    %s =w phi @start 0, @body %s1
    %c =w csltw %i, %n
    jnz %c, @body, @end
@body
    %s1 =w add %s, %i
    %i1 =w add %i, 1
    jmp @loop
@end
    ret %s
}
"#;
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "loop_sum");

    // The @loop block should have 2 phi nodes
    let mut found_phi_block = false;
    for blk in &f.blks {
        if blk.name == "loop" {
            assert_eq!(
                blk.phi.len(),
                2,
                "expected 2 phi nodes in @loop, got {}",
                blk.phi.len()
            );
            found_phi_block = true;
        }
    }
    assert!(found_phi_block, "did not find @loop block");
}

#[test]
#[ignore]
fn parse_call_instruction() {
    let input = r#"
function $caller() {
@start
    %r =w call $add(w 1, w 2)
    ret
}
"#;
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "caller");
}

#[test]
#[ignore]
fn parse_jmp_instruction() {
    let input = r#"
function $jumpy() {
@start
    jmp @end
@end
    ret
}
"#;
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "jumpy");
    assert!(f.blks.len() >= 2, "expected at least 2 blocks");
}

// ---------------------------------------------------------------------------
// Type definitions
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn parse_simple_type() {
    let input = "type :pair = { w, w }\n";
    let result = parse_expect(input, 1, 0, 0);
    let typ = &result.types[0];
    assert_eq!(typ.name, "pair");
}

#[test]
#[ignore]
fn parse_type_with_alignment() {
    let input = "type :vec = align 16 { w, w, w, w }\n";
    let result = parse_expect(input, 1, 0, 0);
    let typ = &result.types[0];
    assert_eq!(typ.name, "vec");
    assert_eq!(typ.align, 4); // stored as log2(16) = 4
}

#[test]
#[ignore]
fn parse_type_with_byte_array() {
    let input = "type :mem = { b 17 }\n";
    let result = parse_expect(input, 1, 0, 0);
    let typ = &result.types[0];
    assert_eq!(typ.name, "mem");
}

#[test]
#[ignore]
fn parse_type_with_multiple_fields() {
    // struct { int x; long y; float z; double w; }
    let input = "type :mixed = { w, l, s, d }\n";
    let result = parse_expect(input, 1, 0, 0);
    let typ = &result.types[0];
    assert_eq!(typ.name, "mixed");
}

// ---------------------------------------------------------------------------
// Data definitions
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn parse_data_definition() {
    let input = r#"data $msg = { b "hello\n", b 0 }
"#;
    let result = parse_expect(input, 0, 1, 0);
    assert!(!result.data[0].is_empty());
}

#[test]
#[ignore]
fn parse_data_with_numbers() {
    let input = "data $arr = { w 1, w 2, w 3 }\n";
    let result = parse_expect(input, 0, 1, 0);
    assert!(!result.data[0].is_empty());
}

#[test]
#[ignore]
fn parse_data_with_zero_fill() {
    let input = "data $buf = { z 1024 }\n";
    let result = parse_expect(input, 0, 1, 0);
    assert!(!result.data[0].is_empty());
}

// ---------------------------------------------------------------------------
// Linkage attributes
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn parse_export_function() {
    let input = "export function $visible() {\n@start\n    ret\n}\n";
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "visible");
    assert!(f.lnk.export, "function should be exported");
}

#[test]
#[ignore]
fn parse_non_export_function() {
    let input = "function $hidden() {\n@start\n    ret\n}\n";
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "hidden");
    assert!(!f.lnk.export, "function should not be exported");
}

#[test]
#[ignore]
fn parse_section_data() {
    let input = r#"section ".rodata" data $ro = { b "readonly", b 0 }
"#;
    let _result = parse_expect(input, 0, 1, 0);
}

// ---------------------------------------------------------------------------
// Multiple definitions
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn parse_multiple_functions() {
    let input = r#"
function w $f1() {
@start
    ret 1
}

function w $f2() {
@start
    ret 2
}

function w $f3() {
@start
    ret 3
}
"#;
    let result = parse_expect(input, 0, 0, 3);
    assert_eq!(result.functions[0].name, "f1");
    assert_eq!(result.functions[1].name, "f2");
    assert_eq!(result.functions[2].name, "f3");
}

#[test]
#[ignore]
fn parse_mixed_definitions() {
    let input = r#"
type :pair = { w, w }

data $msg = { b "hi", b 0 }

export
function w $main() {
@start
    ret 0
}
"#;
    let result = parse_expect(input, 1, 1, 1);
    assert_eq!(result.types[0].name, "pair");
    assert_eq!(result.functions[0].name, "main");
}

// ---------------------------------------------------------------------------
// Memory operations
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn parse_alloc_and_store_load() {
    let input = r#"
function w $mem() {
@start
    %p =l alloc4 4
    storew 42, %p
    %v =w loadw %p
    ret %v
}
"#;
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "mem");
}

// ---------------------------------------------------------------------------
// 64-bit and float operations
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn parse_long_operations() {
    let input = r#"
function l $longadd(l %a, l %b) {
@start
    %c =l add %a, %b
    ret %c
}
"#;
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "longadd");
}

#[test]
#[ignore]
fn parse_float_operations() {
    let input = r#"
function s $fadd(s %a, s %b) {
@start
    %c =s add %a, %b
    ret %c
}
"#;
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "fadd");
}

#[test]
#[ignore]
fn parse_double_operations() {
    let input = r#"
function d $dadd(d %a, d %b) {
@start
    %c =d add %a, %b
    ret %c
}
"#;
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "dadd");
}

// ---------------------------------------------------------------------------
// Extension / conversion operations
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn parse_extension_ops() {
    let input = r#"
function l $extend(w %x) {
@start
    %a =l extsw %x
    %b =l extuw %x
    ret %a
}
"#;
    let result = parse_expect(input, 0, 0, 1);
    let f = &result.functions[0];
    assert_eq!(f.name, "extend");
}

// ---------------------------------------------------------------------------
// Real QBE test file parsing
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn parse_sum_ssa() {
    let contents =
        std::fs::read_to_string("../vendor/qbe-1.2/test/sum.ssa").expect("sum.ssa not found");
    // Extract just the QBE IL (before driver section)
    let qbe_source: String = contents
        .lines()
        .take_while(|l| !l.starts_with("# >>> driver"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = parse(&qbe_source);
    assert_eq!(result.functions.len(), 1, "sum.ssa should have 1 function");
    assert_eq!(result.functions[0].name, "sum");
}

#[test]
#[ignore]
fn parse_eucl_ssa() {
    let contents =
        std::fs::read_to_string("../vendor/qbe-1.2/test/eucl.ssa").expect("eucl.ssa not found");
    let qbe_source: String = contents
        .lines()
        .take_while(|l| !l.starts_with("# >>> driver"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = parse(&qbe_source);
    assert_eq!(result.functions.len(), 1);
    assert_eq!(result.functions[0].name, "test");
}

#[test]
#[ignore]
fn parse_abi1_ssa() {
    let contents =
        std::fs::read_to_string("../vendor/qbe-1.2/test/abi1.ssa").expect("abi1.ssa not found");
    let qbe_source: String = contents
        .lines()
        .take_while(|l| !l.starts_with("# >>> driver"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = parse(&qbe_source);
    // abi1.ssa has a type definition and 2 functions ($alpha and $test)
    assert_eq!(result.types.len(), 1, "abi1.ssa should have 1 type");
    assert_eq!(
        result.functions.len(),
        2,
        "abi1.ssa should have 2 functions"
    );
}
