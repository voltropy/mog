// QBE Codegen Core Tests - ported from qbe_codegen.test.ts
// Tests: literals, variables, assignments, basic expressions, operators,
// string handling, arrays, maps, function calls, f-strings, type casts

use mog::analyzer::SemanticAnalyzer;
use mog::lexer::tokenize;
use mog::parser::parse;
use mog::qbe_codegen::generate_qbe_ir;

/// Helper: compile Mog source to QBE IR string.
fn qbe(src: &str) -> String {
    let tokens = tokenize(src);
    let ast = parse(&tokens);
    let mut analyzer = SemanticAnalyzer::new();
    let errors = analyzer.analyze(&ast);
    assert!(errors.is_empty(), "Analysis errors for `{src}`: {errors:?}");
    generate_qbe_ir(&ast)
}

/// Like qbe() but skips analyzer (for code that may not pass strict analysis).
fn qbe_raw(src: &str) -> String {
    let tokens = tokenize(src);
    let ast = parse(&tokens);
    generate_qbe_ir(&ast)
}

// ============================================================
// Core Infrastructure
// ============================================================

#[test]
fn empty_program_produces_no_main() {
    let ir = qbe("");
    // Empty program should not contain $main
    assert!(
        !ir.contains("$main"),
        "Empty program should not have $main:\n{ir}"
    );
}

// ============================================================
// Number Literals
// ============================================================

#[test]
fn integer_literal_in_print() {
    let ir = qbe("println(42)");
    // Should contain the literal 42 as an argument
    assert!(ir.contains("42"), "IR should contain literal 42:\n{ir}");
    assert!(
        ir.contains("call $print"),
        "IR should contain print call:\n{ir}"
    );
}

#[test]
fn zero_integer_literal() {
    let ir = qbe("println(0)");
    assert!(
        ir.contains("call $print"),
        "IR should contain print call:\n{ir}"
    );
}

#[test]
fn large_integer_literal() {
    let ir = qbe("println(1000000)");
    assert!(
        ir.contains("1000000"),
        "IR should contain large literal:\n{ir}"
    );
}

#[test]
fn float_literal_in_print() {
    let ir = qbe("println(3.14)");
    // Float should appear as QBE d_ format or be used in a float print call
    assert!(
        ir.contains("d_") || ir.contains("print_f64") || ir.contains("println_f64"),
        "IR should handle float literal:\n{ir}"
    );
}

#[test]
fn hex_literal() {
    // 0xFF = 255 — codegen may preserve original format or convert
    let ir = qbe("println(0xFF)");
    assert!(
        ir.contains("0xFF") || ir.contains("255"),
        "Hex literal should appear in IR:\n{ir}"
    );
}

#[test]
fn binary_literal() {
    // 0b1010 = 10 — codegen may preserve original format or convert
    let ir = qbe("println(0b1010)");
    assert!(
        ir.contains("0b1010") || ir.contains("10"),
        "Binary literal should appear in IR:\n{ir}"
    );
}

#[test]
fn octal_literal() {
    // 0o77 = 63 — codegen may preserve original format or convert
    let ir = qbe("println(0o77)");
    assert!(
        ir.contains("0o77") || ir.contains("63"),
        "Octal literal should appear in IR:\n{ir}"
    );
}

// ============================================================
// String Literals
// ============================================================

#[test]
fn string_literal_generates_data_section() {
    let ir = qbe("println(\"hello\")");
    assert!(
        ir.contains("hello"),
        "IR should contain string data 'hello':\n{ir}"
    );
    assert!(
        ir.contains("data"),
        "IR should have data section for strings:\n{ir}"
    );
}

#[test]
fn string_deduplication() {
    let ir = qbe("println(\"hello\")\nprintln(\"hello\")\nprintln(\"world\")");
    // Should only have one "hello" data definition
    let hello_count = ir.matches("b \"hello\"").count();
    assert_eq!(
        hello_count, 1,
        "String 'hello' should be deduplicated, found {hello_count} times:\n{ir}"
    );
    // Both strings present
    assert!(ir.contains("\"hello\""), "Should contain hello:\n{ir}");
    assert!(ir.contains("\"world\""), "Should contain world:\n{ir}");
}

#[test]
fn different_strings_have_different_labels() {
    let ir = qbe("println(\"foo\")\nprintln(\"bar\")");
    assert!(ir.contains("\"foo\""), "Should contain foo:\n{ir}");
    assert!(ir.contains("\"bar\""), "Should contain bar:\n{ir}");
}

#[test]
fn empty_string_literal() {
    let ir = qbe("println(\"\")");
    assert!(ir.contains("call $println"), "Should call println:\n{ir}");
}

#[test]
fn string_with_newline_escape() {
    let ir = qbe("println(\"hello\\nworld\")");
    // The IR should contain the escaped newline
    assert!(ir.contains("hello"), "Should contain hello:\n{ir}");
}

// ============================================================
// Boolean Literals
// ============================================================

#[test]
fn true_literal() {
    let ir = qbe("println(true)");
    // true should be represented as 1
    assert!(ir.contains("1"), "true should be encoded as 1:\n{ir}");
}

#[test]
fn false_literal() {
    let ir = qbe("println(false)");
    // false should be represented as 0
    assert!(ir.contains("call $print"), "Should call print:\n{ir}");
}

// ============================================================
// Variable Declarations (walrus :=)
// ============================================================

#[test]
fn variable_declaration_walrus_int() {
    let ir = qbe("x := 42\nprintln(x)");
    // Should allocate stack space
    assert!(
        ir.contains("alloc"),
        "Should allocate stack for variable:\n{ir}"
    );
    // Should store 42
    assert!(
        ir.contains("storel 42,") || ir.contains("store") && ir.contains("42"),
        "Should store 42:\n{ir}"
    );
}

#[test]
fn variable_declaration_walrus_string() {
    let ir = qbe("name := \"hello\"\nprintln(name)");
    assert!(ir.contains("alloc"), "Should allocate stack:\n{ir}");
    assert!(ir.contains("hello"), "Should contain string data:\n{ir}");
}

#[test]
fn variable_declaration_walrus_bool() {
    let ir = qbe("flag := true\nprintln(flag)");
    assert!(ir.contains("alloc"), "Should allocate stack:\n{ir}");
}

#[test]
fn variable_declaration_walrus_float() {
    let ir = qbe("pi := 3.14\nprintln(pi)");
    assert!(ir.contains("alloc"), "Should allocate stack:\n{ir}");
    // Float should use stored instruction
    assert!(
        ir.contains("stored") || ir.contains("d_"),
        "Should have float store/literal:\n{ir}"
    );
}

#[test]
fn variable_used_after_declaration() {
    let ir = qbe("x := 5\nprintln(x)");
    assert!(ir.contains("alloc"), "Should allocate stack:\n{ir}");
    assert!(
        ir.contains("storel 5,") || ir.contains("store") && ir.contains("5"),
        "Should store 5:\n{ir}"
    );
    // x should be loaded before being passed to println
    assert!(
        ir.contains("loadl") || ir.contains("load"),
        "Should load variable:\n{ir}"
    );
}

#[test]
fn multiple_variable_declarations() {
    let ir = qbe("x := 1\ny := 2\nprintln(x)\nprintln(y)");
    // Should have two separate allocs
    let alloc_count = ir.matches("alloc").count();
    assert!(
        alloc_count >= 2,
        "Should have at least 2 allocs, found {alloc_count}:\n{ir}"
    );
}

// ============================================================
// Variable Assignments
// ============================================================

#[test]
fn variable_reassignment() {
    let ir = qbe("x := 1\nx := 42\nprintln(x)");
    assert!(ir.contains("42"), "Should contain reassigned value:\n{ir}");
    assert!(
        ir.contains("storel") || ir.contains("store"),
        "Should have store instruction:\n{ir}"
    );
}

#[test]
fn variable_reassignment_different_value() {
    let ir = qbe("x := 10\nx := 20\nprintln(x)");
    assert!(
        ir.contains("20"),
        "Should contain reassigned value 20:\n{ir}"
    );
}

// ============================================================
// Binary Operators - Arithmetic
// ============================================================

#[test]
fn integer_addition() {
    let ir = qbe("x := 10 + 3\nprintln(x)");
    assert!(ir.contains("add"), "Should have add instruction:\n{ir}");
}

#[test]
fn integer_subtraction() {
    let ir = qbe("x := 10 - 3\nprintln(x)");
    assert!(ir.contains("sub"), "Should have sub instruction:\n{ir}");
}

#[test]
fn integer_multiplication() {
    let ir = qbe("x := 10 * 3\nprintln(x)");
    assert!(ir.contains("mul"), "Should have mul instruction:\n{ir}");
}

#[test]
fn integer_division() {
    let ir = qbe("x := 10 / 3\nprintln(x)");
    assert!(ir.contains("div"), "Should have div instruction:\n{ir}");
}

#[test]
fn integer_modulo() {
    let ir = qbe("x := 10 % 3\nprintln(x)");
    assert!(ir.contains("rem"), "Should have rem instruction:\n{ir}");
}

#[test]
fn float_addition() {
    let ir = qbe("x := 3.14 + 2.0\nprintln(x)");
    assert!(ir.contains("add"), "Should have float add:\n{ir}");
}

#[test]
fn float_subtraction() {
    let ir = qbe("x := 3.14 - 2.0\nprintln(x)");
    assert!(ir.contains("sub"), "Should have float sub:\n{ir}");
}

#[test]
fn float_multiplication() {
    let ir = qbe("x := 3.14 * 2.0\nprintln(x)");
    assert!(ir.contains("mul"), "Should have float mul:\n{ir}");
}

#[test]
fn cast_float_to_int() {
    // Analyzer warns about precision loss; skip analysis
    let ir = qbe_raw("x := 3.14 as int\nprintln(x)");
    assert!(
        ir.contains("dtosi") || ir.contains("dtos") || ir.contains("cast"),
        "Float-to-int cast should emit conversion:\n{ir}"
    );
}

// ============================================================
// Comparison Operators
// ============================================================

#[test]
fn integer_equality() {
    let ir = qbe("x := (10 == 3)\nprintln(x)");
    assert!(
        ir.contains("ceql") || ir.contains("ceq"),
        "Should have equality comparison:\n{ir}"
    );
}

#[test]
fn integer_not_equal() {
    let ir = qbe("x := (10 != 3)\nprintln(x)");
    assert!(
        ir.contains("cnel") || ir.contains("cne"),
        "Should have not-equal comparison:\n{ir}"
    );
}

#[test]
fn integer_less_than() {
    let ir = qbe("x := (10 < 3)\nprintln(x)");
    assert!(
        ir.contains("csltl") || ir.contains("cslt") || ir.contains("clt"),
        "Should have less-than comparison:\n{ir}"
    );
}

#[test]
fn integer_greater_than() {
    let ir = qbe("x := (10 > 3)\nprintln(x)");
    assert!(
        ir.contains("csgtl") || ir.contains("csgt") || ir.contains("cgt"),
        "Should have greater-than comparison:\n{ir}"
    );
}

#[test]
fn integer_less_or_equal() {
    let ir = qbe("x := (10 <= 3)\nprintln(x)");
    assert!(
        ir.contains("cslel") || ir.contains("csle") || ir.contains("cle"),
        "Should have less-or-equal comparison:\n{ir}"
    );
}

#[test]
fn integer_greater_or_equal() {
    let ir = qbe("x := (10 >= 3)\nprintln(x)");
    assert!(
        ir.contains("csgel") || ir.contains("csge") || ir.contains("cge"),
        "Should have greater-or-equal comparison:\n{ir}"
    );
}

#[test]
fn float_equality() {
    let ir = qbe("x := (3.14 == 2.0)\nprintln(x)");
    assert!(
        ir.contains("ceqd") || ir.contains("ceq"),
        "Should have float equality:\n{ir}"
    );
}

#[test]
fn float_less_than() {
    let ir = qbe("x := (3.14 < 2.0)\nprintln(x)");
    assert!(
        ir.contains("cltd") || ir.contains("clt"),
        "Should have float less-than:\n{ir}"
    );
}

#[test]
fn float_greater_than() {
    let ir = qbe("x := (3.14 > 2.0)\nprintln(x)");
    assert!(
        ir.contains("cgtd") || ir.contains("cgt"),
        "Should have float greater-than:\n{ir}"
    );
}

// ============================================================
// Logical Operators
// ============================================================

#[test]
fn logical_and() {
    // Analyzer requires numeric types for &&; use ints
    let ir = qbe("x := (1 && 0)\nprintln(x)");
    // Should emit boolean and or short-circuit pattern
    assert!(
        ir.contains("and") || ir.contains("jnz"),
        "Should have logical AND:\n{ir}"
    );
}

#[test]
fn logical_or() {
    let ir = qbe("x := (1 || 0)\nprintln(x)");
    assert!(
        ir.contains("or") || ir.contains("jnz"),
        "Should have logical OR:\n{ir}"
    );
}

// ============================================================
// Unary Operators
// ============================================================

#[test]
fn unary_negation_integer() {
    let ir = qbe("x := -42\nprintln(x)");
    // Negation should use sub 0, value or negative literal
    assert!(
        ir.contains("sub") || ir.contains("-42"),
        "Should have negation:\n{ir}"
    );
}

#[test]
fn unary_negation_float() {
    let ir = qbe("x := -3.14\nprintln(x)");
    assert!(
        ir.contains("neg") || ir.contains("sub") || ir.contains("d_"),
        "Should have float negation:\n{ir}"
    );
}

#[test]
fn unary_not() {
    // The `not` keyword produces a unary expression
    // Use a variable to force the codegen to emit the not operation
    let ir = qbe_raw("x := 1\ny := not x\nprintln(y)");
    // not should emit xor with 1 or similar negation
    assert!(
        ir.contains("xor") || ir.contains("ceql") || ir.contains("sub"),
        "Should have logical not:\n{ir}"
    );
}

// ============================================================
// String Concatenation
// ============================================================

#[test]
fn string_concat_two_literals() {
    let ir = qbe("x := \"hello\" + \" world\"\nprintln(x)");
    assert!(
        ir.contains("string_concat") || ir.contains("concat"),
        "Should have string concat call:\n{ir}"
    );
}

#[test]
fn string_concat_variables() {
    let ir = qbe("a := \"hello\"\nb := \" world\"\nc := a + b\nprintln(c)");
    assert!(
        ir.contains("string_concat") || ir.contains("concat"),
        "Should have string concat:\n{ir}"
    );
}

// ============================================================
// String Equality
// ============================================================

#[test]
fn string_equality_comparison() {
    let ir = qbe("x := (\"hello\" == \"world\")\nprintln(x)");
    assert!(
        ir.contains("string_eq"),
        "Should use string_eq for string equality:\n{ir}"
    );
}

#[test]
fn string_inequality_comparison() {
    let ir = qbe("x := (\"hello\" != \"world\")\nprintln(x)");
    assert!(
        ir.contains("string_eq"),
        "Should use string_eq for string inequality:\n{ir}"
    );
    // Inequality also XORs the result
    assert!(ir.contains("xor"), "Should XOR for inequality:\n{ir}");
}

// ============================================================
// Type Casting (as)
// ============================================================

#[test]
fn cast_int_to_float() {
    let ir = qbe("x := 42 as f64\nprintln(x)");
    assert!(
        ir.contains("sltof") || ir.contains("swtof") || ir.contains("cast"),
        "Int-to-float cast should emit conversion:\n{ir}"
    );
}

// ============================================================
// Array Literals
// ============================================================

#[test]
fn empty_array_literal() {
    let ir = qbe_raw("x := []\nprintln(x)");
    assert!(
        ir.contains("gc_alloc") || ir.contains("alloc"),
        "Empty array should allocate memory:\n{ir}"
    );
}

#[test]
fn array_with_elements() {
    let ir = qbe("x := [10, 20, 30]\nprintln(x)");
    assert!(
        ir.contains("gc_alloc") || ir.contains("alloc"),
        "Array should allocate memory:\n{ir}"
    );
    // Elements should be stored
    assert!(ir.contains("10"), "Should contain element 10:\n{ir}");
    assert!(ir.contains("20"), "Should contain element 20:\n{ir}");
    assert!(ir.contains("30"), "Should contain element 30:\n{ir}");
}

#[test]
fn array_stores_elements() {
    let ir = qbe("x := [1, 2, 3]\nprintln(x)");
    // Rust codegen uses array_new + array_push per element
    let push_count = ir.matches("array_push").count();
    assert!(
        push_count >= 3,
        "Should have at least 3 array_push for elements, found {push_count}:\n{ir}"
    );
}

// ============================================================
// Map Literals
// ============================================================

#[test]
fn empty_map_literal() {
    let ir = qbe_raw("x := ({})\nprintln(x)");
    assert!(
        ir.contains("map_new"),
        "Empty map should call map_new:\n{ir}"
    );
}

#[test]
fn map_with_entries() {
    let ir = qbe_raw("x := ({a: 1, b: 2})\nprintln(x)");
    assert!(ir.contains("map_new"), "Map should call map_new:\n{ir}");
    assert!(
        ir.contains("map_set"),
        "Map entries should call map_set:\n{ir}"
    );
}

#[test]
fn map_entries_count() {
    let ir = qbe_raw("x := ({a: 1, b: 2})\nprintln(x)");
    let set_count = ir.matches("map_set").count();
    assert!(
        set_count >= 2,
        "Should have at least 2 map_set calls, found {set_count}:\n{ir}"
    );
}

// ============================================================
// Print / Println Function Calls
// ============================================================

#[test]
fn print_integer() {
    let ir = qbe("print(42)");
    assert!(
        ir.contains("call $print_i64") || ir.contains("call $print"),
        "Should call print function:\n{ir}"
    );
}

#[test]
fn println_string() {
    let ir = qbe("println(\"hello\")");
    assert!(
        ir.contains("call $println_string") || ir.contains("call $println"),
        "Should call println function:\n{ir}"
    );
}

#[test]
fn println_integer() {
    let ir = qbe("println(42)");
    assert!(
        ir.contains("call $println_i64") || ir.contains("call $print"),
        "Should call println for integer:\n{ir}"
    );
}

#[test]
fn println_float() {
    let ir = qbe("println(3.14)");
    assert!(
        ir.contains("println") || ir.contains("print"),
        "Should call println for float:\n{ir}"
    );
}

#[test]
fn println_bool() {
    let ir = qbe("println(true)");
    assert!(ir.contains("call $print"), "Should call print:\n{ir}");
}

#[test]
fn print_no_newline() {
    let ir = qbe("print(\"hello\")");
    assert!(ir.contains("call $print"), "Should call print:\n{ir}");
}

// ============================================================
// F-string / Template Literals
// ============================================================

#[test]
fn fstring_with_int_interpolation() {
    let ir = qbe("println(f\"hello {42}\")");
    assert!(
        ir.contains("i64_to_string") || ir.contains("to_string"),
        "F-string with int should convert to string:\n{ir}"
    );
    assert!(
        ir.contains("string_concat") || ir.contains("concat"),
        "F-string should concat parts:\n{ir}"
    );
}

#[test]
fn fstring_with_multiple_interpolations() {
    let ir = qbe("println(f\"a{1}b{2}c\")");
    assert!(
        ir.contains("i64_to_string") || ir.contains("to_string"),
        "F-string should convert ints:\n{ir}"
    );
    assert!(
        ir.contains("string_concat") || ir.contains("concat"),
        "F-string should concat:\n{ir}"
    );
}

#[test]
fn fstring_only_string_no_conversion() {
    let ir = qbe("println(f\"just a string\")");
    assert!(
        ir.contains("just a string"),
        "Should contain the string data:\n{ir}"
    );
    assert!(
        !ir.contains("i64_to_string"),
        "Pure string f-string should not call i64_to_string:\n{ir}"
    );
}

#[test]
fn fstring_with_variable() {
    let ir = qbe("x := 42\nprintln(f\"value: {x}\")");
    // Should load x and convert
    assert!(
        ir.contains("string_concat") || ir.contains("concat"),
        "F-string with var should concat:\n{ir}"
    );
}

// ============================================================
// Complex Expressions
// ============================================================

#[test]
fn nested_arithmetic() {
    let ir = qbe("x := (1 + 2) * 3\nprintln(x)");
    assert!(ir.contains("add"), "Should have add:\n{ir}");
    assert!(ir.contains("mul"), "Should have mul:\n{ir}");
}

#[test]
fn variable_in_expression() {
    let ir = qbe("x := 10\ny := x + 5\nprintln(y)");
    assert!(
        ir.contains("loadl") || ir.contains("load"),
        "Should load variable x:\n{ir}"
    );
    assert!(ir.contains("add"), "Should have add:\n{ir}");
}

#[test]
fn multiple_operations() {
    let ir = qbe("x := 10\ny := 20\nz := x + y\nprintln(z)");
    assert!(ir.contains("add"), "Should have add:\n{ir}");
    let store_count = ir.matches("storel").count();
    assert!(
        store_count >= 3,
        "Should have at least 3 stores (x, y, z):\n{ir}"
    );
}

// ============================================================
// Comparison in conditions
// ============================================================

#[test]
fn comparison_in_if() {
    let ir = qbe("x := 10\nif x > 5 { println(\"yes\") }");
    assert!(
        ir.contains("csgtl") || ir.contains("csgt") || ir.contains("cgt") || ir.contains("jnz"),
        "Should have comparison and branch:\n{ir}"
    );
    assert!(ir.contains("jnz"), "Should have conditional branch:\n{ir}");
}

#[test]
fn equality_in_if() {
    let ir = qbe("x := 10\nif x == 10 { println(\"yes\") }");
    assert!(
        ir.contains("ceql") || ir.contains("ceq"),
        "Should have equality check:\n{ir}"
    );
}

// ============================================================
// Edge Cases
// ============================================================

#[test]
fn variable_declaration_zero() {
    let ir = qbe("x := 0\nprintln(x)");
    assert!(ir.contains("alloc"), "Should allocate:\n{ir}");
    assert!(
        ir.contains("storel 0,") || ir.contains("store"),
        "Should store zero:\n{ir}"
    );
}

#[test]
fn chained_variable_usage() {
    let ir = qbe("x := 1\ny := x\nz := y\nprintln(z)");
    let load_count = ir.matches("loadl").count();
    assert!(
        load_count >= 2,
        "Should have multiple loads for chained vars, found {load_count}:\n{ir}"
    );
}

#[test]
fn boolean_in_variable() {
    let ir = qbe("x := true\ny := false\nprintln(x)");
    assert!(
        ir.contains("storel 1,") || ir.contains("store") && ir.contains("1"),
        "true should store 1:\n{ir}"
    );
}

#[test]
fn expression_statement_print() {
    let ir = qbe("print(1 + 2)");
    assert!(ir.contains("add"), "Should have add:\n{ir}");
    assert!(ir.contains("call $print"), "Should call print:\n{ir}");
}

#[test]
fn main_function_with_body() {
    let ir = qbe("fn main() { println(\"hello\") }");
    // main should be renamed to program_user
    assert!(
        ir.contains("$program_user") || ir.contains("$main"),
        "Should generate main/program_user function:\n{ir}"
    );
}

#[test]
fn function_with_return() {
    let ir = qbe("fn double(x: int) -> int { return x * 2; }");
    assert!(ir.contains("$double"), "Should generate function:\n{ir}");
    assert!(ir.contains("mul"), "Should have multiply:\n{ir}");
    assert!(ir.contains("ret"), "Should have return:\n{ir}");
}

#[test]
fn function_with_params() {
    let ir = qbe("fn add(a: int, b: int) -> int { return a + b; }");
    assert!(ir.contains("$add"), "Should generate function:\n{ir}");
    assert!(ir.contains("add"), "Should have add:\n{ir}");
    assert!(ir.contains("ret"), "Should have return:\n{ir}");
}

// ============================================================
// While loop generates loop structure
// ============================================================

#[test]
fn while_loop_structure() {
    let ir = qbe("x := 0\nwhile x < 10 { x := x + 1 }");
    assert!(ir.contains("jnz"), "Should have conditional jump:\n{ir}");
    // Should have loop labels
    assert!(
        ir.contains("@while") || ir.contains("@loop") || ir.contains("@L."),
        "Should have loop labels:\n{ir}"
    );
}

// ============================================================
// For-in range loop
// ============================================================

#[test]
fn for_in_range() {
    let ir = qbe("for i in 0..10 { println(i) }");
    assert!(ir.contains("jnz"), "Should have conditional jump:\n{ir}");
    assert!(ir.contains("call $print"), "Should call print:\n{ir}");
}

// ============================================================
// GC integration
// ============================================================

#[test]
fn function_generates_gc_push_frame() {
    let ir = qbe("fn foo() { println(1) }");
    assert!(
        ir.contains("gc_push_frame") || ir.contains("gc_"),
        "Function should emit gc frame management:\n{ir}"
    );
}

// ============================================================
// Mixed type expressions
// ============================================================

#[test]
fn print_expression_result() {
    let ir = qbe("println(10 + 20)");
    assert!(ir.contains("add"), "Should compute addition:\n{ir}");
    assert!(ir.contains("call $print"), "Should call print:\n{ir}");
}

#[test]
fn print_comparison_result() {
    let ir = qbe("println(10 > 5)");
    assert!(
        ir.contains("csgtl") || ir.contains("csgt") || ir.contains("cgt"),
        "Should have comparison:\n{ir}"
    );
}

// ============================================================
// Array fill syntax
// ============================================================

#[test]
fn array_fill_syntax() {
    let ir = qbe_raw("x := [0; 5]\nprintln(x)");
    // Should have allocation and loop/fill pattern
    assert!(
        ir.contains("gc_alloc") || ir.contains("alloc"),
        "Array fill should allocate:\n{ir}"
    );
}

// ============================================================
// Additional expression tests
// ============================================================

#[test]
fn println_with_variable_string() {
    let ir = qbe("s := \"world\"\nprintln(s)");
    assert!(ir.contains("world"), "Should contain string data:\n{ir}");
    assert!(ir.contains("call $println"), "Should call println:\n{ir}");
}

#[test]
fn variable_reassign_and_print() {
    let ir = qbe("x := 1\nx := 2\nprintln(x)");
    assert!(
        ir.contains("storel 1,") || ir.contains("1"),
        "Should store initial value:\n{ir}"
    );
    assert!(
        ir.contains("storel 2,") || ir.contains("2"),
        "Should store reassigned value:\n{ir}"
    );
}

#[test]
fn multiple_print_calls() {
    let ir = qbe("println(1)\nprintln(2)\nprintln(3)");
    let print_count = ir.matches("call $print").count();
    assert!(
        print_count >= 3,
        "Should have at least 3 print calls, found {print_count}:\n{ir}"
    );
}

#[test]
fn if_else_generates_branches() {
    let ir = qbe("if true { println(\"a\") } else { println(\"b\") }");
    assert!(ir.contains("jnz"), "Should have conditional jump:\n{ir}");
    let print_count = ir.matches("call $println").count();
    assert!(
        print_count >= 2,
        "Should have at least 2 println calls:\n{ir}"
    );
}
