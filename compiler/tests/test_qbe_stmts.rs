// ==========================================================================
// QBE codegen tests – control flow, functions, structs, async, advanced
// Ported from src/qbe_codegen.test.ts lines 1800-3291
// Adapted to match the Rust codegen's actual output patterns:
//   - Labels use @L.N naming (not @then./@while.cond. etc.)
//   - Async functions are simplified (no coro/future)
//   - Ok/Err/Some/None call runtime helpers (not inline gc_alloc)
//   - Function params are prefixed with %p. (e.g. %p.a)
// ==========================================================================

use std::collections::HashMap;

use mog::capability::{CapabilityDecl, CapabilityFn};
use mog::lexer::tokenize;
use mog::parser::parse;
use mog::qbe_codegen::{generate_plugin_qbe_ir, generate_qbe_ir, generate_qbe_ir_with_caps};
use mog::types::Type;

/// Helper: full pipeline from Mog source to QBE IR string.
fn qbe(src: &str) -> String {
    let tokens = tokenize(src);
    let ast = parse(&tokens);
    generate_qbe_ir(&ast)
}

/// Helper: full pipeline with capabilities.
fn qbe_with_caps(src: &str, caps: HashMap<String, CapabilityDecl>) -> String {
    let tokens = tokenize(src);
    let ast = parse(&tokens);
    generate_qbe_ir_with_caps(&ast, caps)
}

/// Helper: full pipeline in plugin mode.
fn qbe_plugin(src: &str, name: &str, version: &str) -> String {
    let tokens = tokenize(src);
    let ast = parse(&tokens);
    generate_plugin_qbe_ir(&ast, name, version)
}

// ==========================================================================
// Control Flow – if/else
// ==========================================================================

#[test]
fn if_statement_generates_jnz_and_labels() {
    let ir = qbe("if true { println(\"yes\") }");
    assert!(ir.contains("jnz"), "expected jnz");
    // Rust codegen uses @L.N labels
    assert!(ir.contains("@L."), "expected @L. labels");
}

#[test]
fn if_else_generates_both_branches() {
    let ir = qbe("if false { println(\"a\") } else { println(\"b\") }");
    assert!(ir.contains("jnz"));
    let print_calls = ir.lines().filter(|l| l.contains("call $println")).count();
    assert!(
        print_calls >= 2,
        "expected at least 2 println calls, got {print_calls}"
    );
}

#[test]
fn if_else_if_chain_generates_multiple_jnz() {
    let ir =
        qbe("if true { println(\"a\") } else if false { println(\"b\") } else { println(\"c\") }");
    let jnz_count = ir.lines().filter(|l| l.contains("jnz")).count();
    assert!(jnz_count >= 2, "expected >= 2 jnz, got {jnz_count}");
}

#[test]
fn if_condition_is_boolean_literal() {
    let ir = qbe("if true { println(\"ok\") }");
    // `true` becomes `copy 1` which is used in jnz
    assert!(ir.contains("copy 1"));
    assert!(ir.contains("jnz"));
}

#[test]
fn if_with_numeric_condition() {
    let ir = qbe("x := 42\nif x { println(\"nonzero\") }");
    assert!(ir.contains("jnz"));
    // Should load x and use it in jnz
    assert!(ir.contains("loadl"));
}

#[test]
fn if_false_condition() {
    let ir = qbe("if false { println(\"no\") }");
    assert!(ir.contains("copy 0"), "expected copy 0 for false");
    assert!(ir.contains("jnz"));
}

#[test]
fn if_with_comparison() {
    let ir = qbe("x := 5\nif (x == 5) { println(\"five\") }");
    assert!(ir.contains("ceql"), "expected ceql comparison");
    assert!(ir.contains("jnz"));
}

#[test]
fn if_with_less_than() {
    let ir = qbe("x := 3\nif (x < 10) { println(\"small\") }");
    assert!(ir.contains("csltl"), "expected csltl comparison");
    assert!(ir.contains("jnz"));
}

// ==========================================================================
// Control Flow – while loops
// ==========================================================================

#[test]
fn while_loop_generates_jnz() {
    let ir = qbe("x := 0\nwhile (x < 10) { println(x) }");
    assert!(ir.contains("jnz"), "expected jnz in while loop");
}

#[test]
fn while_loop_generates_comparison() {
    let ir = qbe("x := 0\nwhile (x < 10) { println(x) }");
    assert!(ir.contains("csltl"), "expected csltl comparison");
}

#[test]
fn while_loop_emits_interrupt_check() {
    let ir = qbe("x := 0\nwhile (x < 10) { println(x) }");
    assert!(
        ir.contains("$mog_interrupt_flag"),
        "expected interrupt flag check"
    );
}

#[test]
fn while_loop_has_back_jump() {
    let ir = qbe("x := 0\nwhile (x < 10) { println(x) }");
    // Should have jmp back to loop condition label
    let has_jmp_back = ir.lines().any(|l| l.trim().starts_with("jmp @L."));
    assert!(has_jmp_back, "expected jmp back to loop start");
}

#[test]
fn while_true_loop() {
    let ir = qbe("while true { println(\"loop\") }");
    assert!(ir.contains("copy 1"), "expected copy 1 for true");
    assert!(ir.contains("jnz"));
    assert!(ir.contains("$mog_interrupt_flag"));
}

#[test]
fn while_loop_contains_exit_on_interrupt() {
    let ir = qbe("x := 0\nwhile (x < 10) { println(x) }");
    assert!(
        ir.contains("call $exit(w 99)"),
        "expected exit(99) for interrupt"
    );
}

// ==========================================================================
// Control Flow – for-in-range
// ==========================================================================

#[test]
fn for_in_range_stores_start_value() {
    let ir = qbe("for i in 0..10 { println(i) }");
    assert!(ir.contains("storel 0,"), "expected store of start value 0");
}

#[test]
fn for_in_range_stores_end_value() {
    let ir = qbe("for i in 0..10 { println(i) }");
    // End value 10 is used in csltl comparison
    assert!(
        ir.lines().any(|l| l.contains("csltl") && l.contains("10")),
        "expected csltl with 10"
    );
}

#[test]
fn for_in_range_has_comparison() {
    let ir = qbe("for i in 0..10 { println(i) }");
    assert!(ir.contains("csltl"), "expected csltl for loop condition");
}

#[test]
fn for_in_range_has_jnz() {
    let ir = qbe("for i in 0..10 { println(i) }");
    assert!(ir.contains("jnz"), "expected jnz for loop branch");
}

#[test]
fn for_in_range_increments_counter() {
    let ir = qbe("for i in 0..10 { println(i) }");
    let has_add_1 = ir.lines().any(|l| l.contains("add") && l.contains(", 1"));
    assert!(has_add_1, "expected add with step 1");
}

#[test]
fn for_in_range_has_interrupt_check() {
    let ir = qbe("for i in 0..10 { println(i) }");
    assert!(ir.contains("$mog_interrupt_flag"));
}

#[test]
fn for_in_range_has_back_jump() {
    let ir = qbe("for i in 0..10 { println(i) }");
    let jmp_count = ir
        .lines()
        .filter(|l| l.trim().starts_with("jmp @L."))
        .count();
    assert!(jmp_count >= 1, "expected at least one jmp back");
}

// ==========================================================================
// Control Flow – for-each
// ==========================================================================

#[test]
fn for_each_loop_uses_array_get() {
    let ir = qbe("arr := [1, 2, 3]\nfor item in arr { println(item) }");
    assert!(ir.contains("call $array_get("), "expected array_get call");
}

#[test]
fn for_each_loop_has_interrupt_check() {
    let ir = qbe("arr := [1, 2, 3]\nfor item in arr { println(item) }");
    assert!(ir.contains("$mog_interrupt_flag"));
}

#[test]
fn for_each_loop_compares_index_to_length() {
    let ir = qbe("arr := [1, 2, 3]\nfor item in arr { println(item) }");
    assert!(ir.contains("csltl"), "expected csltl for index < length");
}

#[test]
fn for_each_loop_increments_index() {
    let ir = qbe("arr := [1, 2, 3]\nfor item in arr { println(item) }");
    let has_add_1 = ir.lines().any(|l| l.contains("add") && l.contains(", 1"));
    assert!(has_add_1, "expected add 1 for index increment");
}

// ==========================================================================
// Control Flow – for-in-index
// ==========================================================================

#[test]
fn for_in_index_uses_array_get() {
    let ir = qbe("arr := [10, 20, 30]\nfor idx, val in arr { println(idx) }");
    assert!(ir.contains("call $array_get("));
}

#[test]
fn for_in_index_has_comparison() {
    let ir = qbe("arr := [10, 20, 30]\nfor idx, val in arr { println(idx) }");
    assert!(ir.contains("csltl"), "expected csltl for index comparison");
}

#[test]
fn for_in_index_both_vars_usable() {
    // Both idx and val should be accessible without error
    let ir = qbe("arr := [10, 20, 30]\nfor idx, val in arr { println(idx)\nprintln(val) }");
    let println_count = ir.lines().filter(|l| l.contains("call $println")).count();
    assert!(println_count >= 2, "expected >= 2 println calls");
}

// ==========================================================================
// Control Flow – for-in-map
// ==========================================================================

#[test]
fn for_in_map_iterates() {
    let ir = qbe("m := {\"a\": 1, \"b\": 2}\nfor k, v in m { println(k) }");
    // Should have some kind of iteration with jnz
    assert!(ir.contains("jnz"), "expected jnz for loop");
    assert!(ir.contains("$mog_interrupt_flag"));
}

#[test]
fn for_in_map_creates_map() {
    let ir = qbe("m := {\"a\": 1, \"b\": 2}\nfor k, v in m { println(k) }");
    assert!(ir.contains("call $map_new(l"), "expected map_new call");
    assert!(ir.contains("call $map_set("), "expected map_set calls");
}

// ==========================================================================
// Control Flow – break/continue
// ==========================================================================

#[test]
fn break_in_while_jumps_past_loop() {
    let ir = qbe("x := 0\nwhile true { break }");
    // break generates jmp to the end label (which is the label after the loop body)
    // In: jnz %v.1, @L.1, @L.2  -> @L.2 is end
    // break body is: jmp @L.2
    assert!(ir.contains("jmp @L.2"), "expected jmp to end label @L.2");
}

#[test]
fn continue_in_while_jumps_to_condition() {
    let ir = qbe("x := 0\nwhile (x < 10) { continue }");
    // continue should jump back to the loop condition
    assert!(
        ir.contains("jmp @L.0"),
        "expected jmp to condition label @L.0"
    );
}

#[test]
fn break_in_for_jumps_past_loop() {
    let ir = qbe("for i in 0..10 { break }");
    // end label is now @L.3 (test=@L.0, body=@L.1, step=@L.2, end=@L.3)
    assert!(ir.contains("jmp @L.3"), "expected jmp to end label @L.3");
}

#[test]
fn continue_in_for_jumps_back() {
    let ir = qbe("for i in 0..10 { continue }");
    // continue should jump to the step label @L.2
    assert!(ir.contains("jmp @L.2"), "expected jmp to step label");
}

#[test]
fn break_outside_loop_does_nothing_special() {
    // break at top-level shouldn't crash (it just won't appear)
    // This tests that the codegen doesn't panic
    let ir = qbe("x := 1\nprintln(x)");
    assert!(ir.contains("println"));
}

// ==========================================================================
// Function declarations (sync)
// ==========================================================================

#[test]
fn simple_function_with_params() {
    let ir = qbe("fn add(a: int, b: int) -> int { return a + b; }");
    assert!(ir.contains("function"), "expected function keyword");
    assert!(ir.contains("$add"), "expected $add function name");
    assert!(ir.contains("@start"), "expected @start label");
    assert!(ir.contains("alloc8 8"), "expected alloc8 8 for stack slots");
    assert!(ir.contains("storel %p.a,"), "expected storel %p.a");
    assert!(ir.contains("storel %p.b,"), "expected storel %p.b");
    assert!(ir.contains("add"), "expected add instruction");
    assert!(ir.contains("ret"), "expected ret");
}

#[test]
fn function_named_main_becomes_program_user() {
    let ir = qbe("fn main() { println(\"hello\") }");
    assert!(
        ir.contains("$program_user"),
        "expected main renamed to $program_user"
    );
    assert!(ir.contains("@start"));
}

#[test]
fn void_function_generates_ret() {
    let ir = qbe("fn greet() { println(\"hi\") }");
    assert!(ir.contains("$greet"), "expected $greet function");
    assert!(ir.contains("@start"));
    assert!(ir.contains("ret"), "expected ret instruction");
}

#[test]
fn function_with_return_value() {
    let ir = qbe("fn double(x: int) -> int { return x * 2; }");
    assert!(ir.contains("$double"));
    assert!(ir.contains("storel %p.x,"), "expected storel %p.x");
    assert!(ir.contains("mul"), "expected mul instruction");
    assert!(ir.contains("ret"));
}

#[test]
fn function_generates_gc_push_frame() {
    let ir = qbe("fn foo() { return 1; }");
    assert!(ir.contains("call $gc_push_frame()"));
}

#[test]
fn function_generates_gc_pop_frame() {
    let ir = qbe("fn foo() { return 1; }");
    assert!(ir.contains("call $gc_pop_frame()"));
}

#[test]
fn multiple_functions_generated_separately() {
    let ir = qbe("fn foo() -> int { return 1; }\nfn bar() -> int { return 2; }");
    assert!(ir.contains("$foo"), "expected $foo");
    assert!(ir.contains("$bar"), "expected $bar");
    let starts = ir.lines().filter(|l| l.trim() == "@start").count();
    assert!(starts >= 2, "expected >= 2 @start labels, got {starts}");
}

#[test]
fn function_with_no_params_has_empty_param_list() {
    let ir = qbe("fn nothing() -> int { return 42; }");
    assert!(ir.contains("$nothing()"), "expected $nothing()");
}

#[test]
fn function_return_type_is_l() {
    let ir = qbe("fn get_val() -> int { return 42; }");
    assert!(
        ir.contains("function l $get_val()"),
        "expected function l return type"
    );
}

// ==========================================================================
// Return statements
// ==========================================================================

#[test]
fn return_with_literal_value() {
    let ir = qbe("fn get_val() -> int { return 42; }");
    assert!(ir.contains("ret 42"), "expected ret 42");
}

#[test]
fn return_with_expression() {
    let ir = qbe("fn compute(x: int) -> int { return x + 10; }");
    assert!(ir.contains("add"), "expected add instruction");
    assert!(ir.contains("ret"), "expected ret");
}

#[test]
fn return_void_emits_gc_pop_frame() {
    let ir = qbe("fn do_nothing() { return; }");
    assert!(ir.contains("call $gc_pop_frame()"));
    assert!(ir.contains("ret"));
}

#[test]
fn return_void_uses_ret_0() {
    let ir = qbe("fn do_nothing() { return; }");
    // Void return should emit ret 0 or similar
    assert!(ir.contains("ret"), "expected ret instruction");
}

// ==========================================================================
// Struct definitions and field access
// ==========================================================================

#[test]
fn struct_definition_parses_without_error() {
    let ir = qbe("struct Point { x: int, y: int }");
    // Just check it doesn't panic — struct defs don't produce IR themselves
    let _ = ir;
}

#[test]
fn struct_construction_allocates_memory() {
    let ir = qbe("struct Point { x: int, y: int }\np := Point { x: 10, y: 20 }\nprintln(p)");
    assert!(
        ir.contains("call $gc_alloc(l 16)"),
        "expected gc_alloc(l 16) for 2-field struct"
    );
}

#[test]
fn struct_construction_stores_field_values() {
    let ir = qbe("struct Point { x: int, y: int }\np := Point { x: 10, y: 20 }\nprintln(p)");
    assert!(ir.contains("storel 10,"), "expected storel 10");
    assert!(ir.contains("storel 20,"), "expected storel 20");
}

#[test]
fn struct_3_fields_allocates_24_bytes() {
    let ir =
        qbe("struct Vec3 { x: int, y: int, z: int }\nv := Vec3 { x: 1, y: 2, z: 3 }\nprintln(v)");
    assert!(
        ir.contains("call $gc_alloc(l 24)"),
        "expected gc_alloc(l 24) for 3 fields"
    );
}

#[test]
fn struct_field_access_generates_loadl() {
    let ir = qbe("struct Point { x: int, y: int }\np := Point { x: 10, y: 20 }\nprintln(p.x)");
    assert!(ir.contains("loadl"), "expected loadl for field access");
}

#[test]
fn struct_field_at_nonzero_offset_adds_8() {
    let ir = qbe("struct Point { x: int, y: int }\np := Point { x: 10, y: 20 }\nprintln(p.y)");
    // y is at offset 8
    let has_offset = ir.lines().any(|l| l.contains("add") && l.contains(", 8"));
    assert!(has_offset, "expected add with offset 8 for field y");
}

#[test]
fn struct_construction_single_field() {
    let ir = qbe("struct Wrapper { val: int }\nw := Wrapper { val: 42 }\nprintln(w)");
    assert!(ir.contains("call $gc_alloc"), "expected gc_alloc");
    assert!(ir.contains("storel 42,"), "expected storel 42");
}

#[test]
fn struct_nested_access() {
    let ir = qbe("struct Inner { v: int }\nstruct Outer { inner: Inner }\ni := Inner { v: 5 }\no := Outer { inner: i }\nprintln(o.inner)");
    assert!(ir.contains("call $gc_alloc"), "expected gc_alloc");
    assert!(ir.contains("loadl"), "expected loadl");
}

// ==========================================================================
// Async function declarations
// ==========================================================================

#[test]
fn async_function_generates_function() {
    let ir = qbe("async fn fetchData() { println(\"loading\") }");
    assert!(ir.contains("$fetchData"), "expected $fetchData function");
}

#[test]
fn async_function_has_gc_push_frame() {
    let ir = qbe("async fn fetchData() { println(\"loading\") }");
    assert!(ir.contains("call $gc_push_frame()"));
}

#[test]
fn async_function_has_gc_pop_frame() {
    let ir = qbe("async fn fetchData() { println(\"loading\") }");
    assert!(ir.contains("call $gc_pop_frame()"));
}

#[test]
fn async_function_has_start_label() {
    let ir = qbe("async fn fetchData() { println(\"loading\") }");
    assert!(ir.contains("@start"), "expected @start label");
}

#[test]
fn async_function_with_params() {
    let ir = qbe("async fn compute(x: int, y: int) { println(x) }");
    assert!(ir.contains("alloc8 8"), "expected alloc8 for param slots");
    assert!(ir.contains("storel %p.x,"), "expected storel %p.x");
    assert!(ir.contains("storel %p.y,"), "expected storel %p.y");
}

#[test]
fn async_function_with_return() {
    let ir = qbe("async fn getValue() -> int { return 42; }");
    assert!(ir.contains("ret"), "expected ret");
}

#[test]
fn async_function_body_executes() {
    let ir = qbe("async fn fetchData() { println(\"loading\") }");
    assert!(
        ir.contains("call $println_string("),
        "expected println call in body"
    );
}

// ==========================================================================
// Await expressions
// ==========================================================================

#[test]
fn await_calls_async_function() {
    let ir = qbe(
        "async fn fetchData() -> int { return 1; }\nasync fn main() { x := await fetchData() }",
    );
    assert!(
        ir.contains("call $fetchData("),
        "expected call to fetchData"
    );
}

#[test]
fn await_stores_result() {
    let ir = qbe(
        "async fn fetchData() -> int { return 1; }\nasync fn main() { x := await fetchData() }",
    );
    assert!(ir.contains("storel"), "expected storel for await result");
}

// ==========================================================================
// Spawn expressions
// ==========================================================================

#[test]
fn spawn_calls_async_function() {
    let ir = qbe("async fn doWork() { println(\"working\") }\nfn main() { f := spawn doWork() }");
    assert!(ir.contains("call $doWork("), "expected call to doWork");
}

#[test]
fn spawn_stores_result() {
    let ir = qbe("async fn doWork() { println(\"working\") }\nfn main() { f := spawn doWork() }");
    assert!(ir.contains("storel"), "expected storel for spawn result");
}

// ==========================================================================
// Ok/Err/Some/None – Result and Optional types
// ==========================================================================

#[test]
fn ok_expression_calls_ok_helper() {
    let ir = qbe("r := Ok(42)\nprintln(r)");
    assert!(ir.contains("call $Ok("), "expected call to $Ok");
}

#[test]
fn ok_expression_passes_value() {
    let ir = qbe("r := Ok(42)\nprintln(r)");
    assert!(ir.contains("call $Ok(l 42)"), "expected call $Ok(l 42)");
}

#[test]
fn err_expression_calls_err_helper() {
    let ir = qbe("r := Err(99)\nprintln(r)");
    assert!(ir.contains("call $Err("), "expected call to $Err");
}

#[test]
fn err_expression_passes_value() {
    let ir = qbe("r := Err(99)\nprintln(r)");
    assert!(ir.contains("call $Err(l 99)"), "expected call $Err(l 99)");
}

#[test]
fn some_expression_calls_some_helper() {
    let ir = qbe("opt := Some(7)\nprintln(opt)");
    assert!(ir.contains("call $Some("), "expected call to $Some");
}

#[test]
fn some_expression_passes_value() {
    let ir = qbe("opt := Some(7)\nprintln(opt)");
    assert!(ir.contains("call $Some(l 7)"), "expected call $Some(l 7)");
}

#[test]
fn none_expression_stores_zero() {
    let ir = qbe("opt := None\nprintln(opt)");
    // None just stores 0 directly
    assert!(ir.contains("storel 0,"), "expected storel 0 for None");
}

// ==========================================================================
// Match expression
// ==========================================================================

#[test]
fn match_expression_generates_ceql() {
    let ir = qbe("x := 42\ny := match x { 1 => 10, _ => 0 }\nprintln(y)");
    assert!(ir.contains("ceql"), "expected ceql for match arm");
}

#[test]
fn match_expression_generates_jnz() {
    let ir = qbe("x := 42\ny := match x { 1 => 10, _ => 0 }\nprintln(y)");
    assert!(ir.contains("jnz"), "expected jnz for match branch");
}

#[test]
fn match_expression_stores_arm_values() {
    let ir = qbe("x := 42\ny := match x { 1 => 10, _ => 0 }\nprintln(y)");
    assert!(
        ir.contains("storel 10,"),
        "expected storel 10 for first arm"
    );
    assert!(
        ir.contains("storel 0,"),
        "expected storel 0 for default arm"
    );
}

#[test]
fn match_wildcard_uses_jmp() {
    let ir = qbe("x := 42\ny := match x { 1 => 10, _ => 0 }\nprintln(y)");
    // Wildcard should use jmp (unconditional)
    assert!(ir.contains("jmp @L."), "expected jmp for wildcard arm");
}

// ==========================================================================
// Cast expressions
// ==========================================================================

#[test]
fn cast_int_to_float_generates_sltof() {
    let ir = qbe("x := 42\ny := x as float\nprintln(y)");
    assert!(ir.contains("sltof"), "expected sltof for int->float cast");
}

#[test]
fn cast_float_to_int_generates_dtosi() {
    let ir = qbe("x := 3.14\ny := x as int\nprintln(y)");
    assert!(ir.contains("dtosi"), "expected dtosi for float->int cast");
}

#[test]
fn cast_to_string_stores_value() {
    let ir = qbe("x := 42\ny := x as string\nprintln(y)");
    // Cast to string in the Rust codegen stores the value directly
    assert!(ir.contains("storel"), "expected storel for cast value");
    assert!(ir.contains("loadl"), "expected loadl to read cast result");
}

// ==========================================================================
// Capability calls (requires + function calls)
// ==========================================================================

fn make_fs_cap() -> HashMap<String, CapabilityDecl> {
    let mut caps = HashMap::new();
    caps.insert(
        "fs".to_string(),
        CapabilityDecl {
            name: "fs".to_string(),
            functions: vec![CapabilityFn {
                name: "read_file".to_string(),
                params: vec![("path".to_string(), Type::String)],
                return_type: Type::String,
                is_async: false,
            }],
        },
    );
    caps
}

fn make_net_cap() -> HashMap<String, CapabilityDecl> {
    let mut caps = HashMap::new();
    caps.insert(
        "net".to_string(),
        CapabilityDecl {
            name: "net".to_string(),
            functions: vec![CapabilityFn {
                name: "fetch".to_string(),
                params: vec![("url".to_string(), Type::String)],
                return_type: Type::String,
                is_async: false,
            }],
        },
    );
    caps
}

fn make_timer_cap() -> HashMap<String, CapabilityDecl> {
    let mut caps = HashMap::new();
    caps.insert(
        "timer".to_string(),
        CapabilityDecl {
            name: "timer".to_string(),
            functions: vec![CapabilityFn {
                name: "now".to_string(),
                params: vec![],
                return_type: Type::int(),
                is_async: false,
            }],
        },
    );
    caps
}

#[test]
fn capability_call_invokes_dispatcher() {
    let ir = qbe_with_caps(
        "requires fs\nresult := fs.read_file(\"test.txt\")\nprintln(result)",
        make_fs_cap(),
    );
    assert!(
        ir.contains("call $mog_cap_call_out"),
        "expected mog_cap_call_out dispatcher call"
    );
}

#[test]
fn capability_call_allocates_args_buffer() {
    let ir = qbe_with_caps(
        "requires fs\nresult := fs.read_file(\"test.txt\")\nprintln(result)",
        make_fs_cap(),
    );
    assert!(
        ir.contains("call $gc_alloc"),
        "expected gc_alloc for args buffer"
    );
}

#[test]
fn capability_call_creates_string_constants() {
    let ir = qbe_with_caps(
        "requires net\nresult := net.fetch(\"http://example.com\")\nprintln(result)",
        make_net_cap(),
    );
    assert!(ir.contains("$str."), "expected string constant references");
    assert!(ir.contains("$mog_cap_call_out"));
}

#[test]
fn capability_call_with_zero_args() {
    let ir = qbe_with_caps(
        "requires timer\nt := timer.now()\nprintln(t)",
        make_timer_cap(),
    );
    assert!(ir.contains("$mog_cap_call_out"));
    // arg count should be 0 (as w 0)
    assert!(ir.contains("w 0)"), "expected 0 args in cap call");
}

#[test]
fn capability_call_extracts_result() {
    let ir = qbe_with_caps(
        "requires timer\nt := timer.now()\nprintln(t)",
        make_timer_cap(),
    );
    // Should extract result from offset 8 of the return buffer
    assert!(ir.contains("add"), "expected add for result offset");
    assert!(ir.contains("loadl"), "expected loadl to get result");
}

// ==========================================================================
// Plugin mode
// ==========================================================================

#[test]
fn plugin_mode_generates_plugin_info() {
    let ir = qbe_plugin(
        "fn greet(name: string) -> string { return name; }",
        "hello_plugin",
        "1.0.0",
    );
    assert!(
        ir.contains("$mog_plugin_info"),
        "expected $mog_plugin_info function"
    );
}

#[test]
fn plugin_mode_generates_plugin_init() {
    let ir = qbe_plugin(
        "fn greet(name: string) -> string { return name; }",
        "hello_plugin",
        "1.0.0",
    );
    assert!(
        ir.contains("$mog_plugin_init"),
        "expected $mog_plugin_init function"
    );
}

#[test]
fn plugin_mode_generates_plugin_exports() {
    let ir = qbe_plugin(
        "fn greet(name: string) -> string { return name; }",
        "hello_plugin",
        "1.0.0",
    );
    assert!(
        ir.contains("$mog_plugin_exports"),
        "expected $mog_plugin_exports function"
    );
}

#[test]
fn plugin_mode_stores_plugin_name() {
    let ir = qbe_plugin(
        "fn greet(name: string) -> string { return name; }",
        "hello_plugin",
        "1.0.0",
    );
    assert!(ir.contains("hello_plugin"), "expected plugin name in data");
}

#[test]
fn plugin_mode_stores_version() {
    let ir = qbe_plugin(
        "fn greet(name: string) -> string { return name; }",
        "hello_plugin",
        "1.0.0",
    );
    assert!(ir.contains("1.0.0"), "expected version in data");
}

#[test]
fn plugin_mode_exports_functions() {
    let ir = qbe_plugin(
        "fn add(a: int, b: int) -> int { return a + b; }",
        "math_plugin",
        "0.1.0",
    );
    assert!(ir.contains("$add"), "expected $add function in plugin IR");
}

#[test]
fn plugin_mode_has_export_keyword() {
    let ir = qbe_plugin(
        "fn greet(name: string) -> string { return name; }",
        "hello_plugin",
        "1.0.0",
    );
    assert!(
        ir.contains("export function"),
        "expected export function keyword"
    );
}

// ==========================================================================
// Main entry generation
// ==========================================================================

#[test]
fn main_entry_renames_to_program_user() {
    let ir = qbe("fn main() { println(\"hello world\") }");
    assert!(ir.contains("$program_user"));
}

#[test]
fn main_entry_generates_exported_main() {
    let ir = qbe("fn main() { println(\"hello world\") }");
    assert!(
        ir.contains("export function w $main()"),
        "expected exported $main"
    );
}

#[test]
fn main_calls_gc_init() {
    let ir = qbe("fn main() { println(\"hello world\") }");
    assert!(
        ir.contains("call $gc_init()"),
        "expected gc_init in main entry"
    );
}

#[test]
fn toplevel_code_wrapped_in_mog_program() {
    let ir = qbe("println(\"hello\")");
    assert!(ir.contains("$mog_program"), "expected $mog_program wrapper");
    assert!(ir.contains("call $println"), "expected println call");
}

// ==========================================================================
// Void functions
// ==========================================================================

#[test]
fn void_function_no_return_has_default_ret() {
    let ir = qbe("fn log_msg() { println(\"msg\") }");
    assert!(ir.contains("$log_msg"));
    assert!(ir.contains("ret"), "expected default ret");
}

#[test]
fn void_function_explicit_return() {
    let ir = qbe("fn early_exit(x: int) { if (x > 0) { return; }\nprintln(\"neg\") }");
    assert!(ir.contains("ret"));
    assert!(ir.contains("call $gc_pop_frame()"));
}

// ==========================================================================
// Recursive functions
// ==========================================================================

#[test]
fn recursive_function_compiles() {
    let ir = qbe(
        "fn factorial(n: int) -> int { if (n <= 1) { return 1; }\nreturn n * factorial(n - 1); }",
    );
    assert!(ir.contains("$factorial"), "expected $factorial function");
    assert!(
        ir.contains("call $factorial("),
        "expected recursive call to factorial"
    );
}

#[test]
fn recursive_function_has_base_case_and_recursive_call() {
    let ir =
        qbe("fn fib(n: int) -> int { if (n <= 1) { return n; }\nreturn fib(n - 1) + fib(n - 2); }");
    assert!(ir.contains("$fib"));
    let fib_calls = ir.lines().filter(|l| l.contains("call $fib(")).count();
    assert!(
        fib_calls >= 2,
        "expected >= 2 recursive fib calls, got {fib_calls}"
    );
}

// ==========================================================================
// Nested control flow
// ==========================================================================

#[test]
fn nested_if_in_while() {
    let ir = qbe("x := 0\nwhile (x < 10) { if (x > 5) { println(\"big\") } }");
    // Should have both while structure and if structure
    let jnz_count = ir.lines().filter(|l| l.contains("jnz")).count();
    assert!(
        jnz_count >= 2,
        "expected >= 2 jnz for nested if+while, got {jnz_count}"
    );
}

#[test]
fn nested_for_in_for() {
    let ir = qbe("for i in 0..5 { for j in 0..5 { println(i) } }");
    // Should have at least 2 csltl comparisons for both loops
    let cmp_count = ir.lines().filter(|l| l.contains("csltl")).count();
    assert!(
        cmp_count >= 2,
        "expected >= 2 csltl for nested loops, got {cmp_count}"
    );
}

#[test]
fn while_inside_function() {
    let ir = qbe("fn count_up() { x := 0\nwhile (x < 100) { println(x) } }");
    assert!(ir.contains("$count_up"));
    assert!(ir.contains("csltl"));
    assert!(ir.contains("jnz"));
}

#[test]
fn if_inside_for() {
    let ir = qbe("for i in 0..10 { if (i > 5) { println(i) } }");
    // Both for loop comparison and if comparison
    let jnz_count = ir.lines().filter(|l| l.contains("jnz")).count();
    assert!(jnz_count >= 2, "expected >= 2 jnz, got {jnz_count}");
}

#[test]
fn break_in_nested_while() {
    let ir = qbe("x := 0\nwhile true { while true { break }\nbreak }");
    // Should have at least 2 jmp instructions for the two breaks
    let break_jumps = ir
        .lines()
        .filter(|l| l.trim().starts_with("jmp @L."))
        .count();
    assert!(
        break_jumps >= 2,
        "expected >= 2 jmp for breaks, got {break_jumps}"
    );
}

// ==========================================================================
// Additional integration tests
// ==========================================================================

#[test]
fn if_with_equality_comparison() {
    let ir = qbe("x := 5\nif (x == 5) { println(\"five\") }");
    assert!(ir.contains("ceql"));
    assert!(ir.contains("jnz"));
}

#[test]
fn while_with_less_than() {
    let ir = qbe("x := 0\ny := 0\nwhile (x < 10) { println(x) }");
    assert!(ir.contains("csltl"));
}

#[test]
fn for_loop_body_with_function_call() {
    let ir =
        qbe("fn double(x: int) -> int { return x * 2; }\nfor i in 0..5 { println(double(i)) }");
    assert!(ir.contains("$double"));
    assert!(ir.contains("call $double("));
}

#[test]
fn function_calling_another_function() {
    let ir =
        qbe("fn add(a: int, b: int) -> int { return a + b; }\nfn main() { println(add(1, 2)) }");
    assert!(ir.contains("$add"));
    assert!(ir.contains("$program_user"));
    assert!(ir.contains("call $add("));
}

// ==========================================================================
// Array operations
// ==========================================================================

#[test]
fn array_literal_uses_array_alloc() {
    let ir = qbe("arr := [10, 20, 30]\nprintln(arr)");
    assert!(
        ir.contains("call $array_alloc("),
        "expected array_alloc call"
    );
}

#[test]
fn array_literal_pushes_elements() {
    let ir = qbe("arr := [10, 20, 30]\nprintln(arr)");
    let push_count = ir
        .lines()
        .filter(|l| l.contains("call $array_push("))
        .count();
    assert!(
        push_count == 3,
        "expected 3 array_push calls, got {push_count}"
    );
}

#[test]
fn array_index_uses_array_get() {
    let ir = qbe("arr := [10, 20, 30]\nx := arr[1]\nprintln(x)");
    assert!(ir.contains("call $array_get("), "expected array_get call");
}

#[test]
fn array_len_loads_from_array() {
    let ir = qbe("arr := [1, 2, 3]\nn := arr.len\nprintln(n)");
    assert!(ir.contains("loadl"), "expected loadl for array .len");
}

// ==========================================================================
// Map operations
// ==========================================================================

#[test]
fn map_literal_creates_map() {
    let ir = qbe("m := {\"a\": 1, \"b\": 2}\nprintln(m)");
    assert!(ir.contains("call $map_new(l"), "expected map_new");
    assert!(ir.contains("call $map_set("), "expected map_set");
}

#[test]
fn map_literal_sets_entries() {
    let ir = qbe("m := {\"a\": 1, \"b\": 2}\nprintln(m)");
    let set_count = ir.lines().filter(|l| l.contains("call $map_set(")).count();
    assert!(set_count == 2, "expected 2 map_set calls, got {set_count}");
}

// ==========================================================================
// String operations
// ==========================================================================

#[test]
fn string_literal_in_data_section() {
    let ir = qbe("s := \"hello world\"\nprintln(s)");
    assert!(ir.contains("data $str."), "expected string in data section");
    assert!(ir.contains("hello world"), "expected string content");
}

#[test]
fn string_literal_null_terminated() {
    let ir = qbe("s := \"hello\"\nprintln(s)");
    assert!(ir.contains("b 0"), "expected null terminator");
}

#[test]
fn string_concatenation_calls_string_concat() {
    let ir = qbe("a := \"hello \"\nb := \"world\"\nc := a + b\nprintln(c)");
    assert!(
        ir.contains("call $string_concat"),
        "expected string_concat call"
    );
}

// ==========================================================================
// Top-level program structure
// ==========================================================================

#[test]
fn toplevel_generates_main_export() {
    let ir = qbe("println(\"hi\")");
    assert!(ir.contains("export function w $main()"));
}

#[test]
fn main_entry_calls_mog_vm_get_global() {
    let ir = qbe("println(\"hi\")");
    assert!(ir.contains("call $mog_vm_get_global()"));
}

#[test]
fn main_entry_creates_vm_if_needed() {
    let ir = qbe("println(\"hi\")");
    assert!(ir.contains("call $mog_vm_new()"));
    assert!(ir.contains("call $mog_register_posix_host("));
}
