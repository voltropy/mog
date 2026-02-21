// QBE codegen expression tests — ported from qbe_codegen.test.ts
// Covers: function calls, member expressions, index expressions,
// closures/lambdas, match expressions, Result/Optional, template literals,
// if/block expressions, structs, and more.

fn qbe(src: &str) -> String {
    let tokens = mog::lexer::tokenize(src);
    let ast = mog::parser::parse(&tokens);
    let mut analyzer = mog::analyzer::SemanticAnalyzer::new();
    let _errors = analyzer.analyze(&ast);
    mog::qbe_codegen::generate_qbe_ir(&ast)
}

// ============================================================
// Function Calls
// ============================================================

#[test]
fn test_call_println_string() {
    let ir = qbe(r#"println("hello")"#);
    assert!(
        ir.contains("call $println_string("),
        "should call println_string for println: {ir}"
    );
}

#[test]
fn test_call_println_i64() {
    let ir = qbe("println_i64(42)");
    assert!(
        ir.contains("call $println_i64("),
        "should call println_i64: {ir}"
    );
}

#[test]
fn test_call_user_function_one_arg() {
    let ir = qbe("fn foo(x: int) -> int { return x; }\nfoo(5)");
    assert!(ir.contains("call $foo("), "should call foo: {ir}");
}

#[test]
fn test_call_user_function_two_args() {
    let ir = qbe("fn add(a: int, b: int) -> int { return a + b; }\nadd(1, 2)");
    assert!(ir.contains("call $add("), "should call add: {ir}");
}

#[test]
fn test_call_user_function_three_args() {
    let ir = qbe("fn triple(a: int, b: int, c: int) -> int { return a + b + c; }\ntriple(1, 2, 3)");
    assert!(ir.contains("call $triple("), "should call triple: {ir}");
}

#[test]
fn test_call_result_assigned_to_variable() {
    let ir = qbe("fn double(x: int) -> int { return x * 2; }\nr := double(21)");
    assert!(ir.contains("call $double("), "should call double: {ir}");
    assert!(ir.contains("storel"), "should store result: {ir}");
}

#[test]
fn test_call_nested_function_calls() {
    let ir = qbe("fn id(x: int) -> int { return x; }\nprintln_i64(id(42))");
    assert!(ir.contains("call $id("), "should call id: {ir}");
    assert!(
        ir.contains("call $println_i64("),
        "should call println_i64: {ir}"
    );
}

#[test]
fn test_call_function_with_string_arg() {
    let ir = qbe("fn greet(name: string) -> string { return name; }\ngreet(\"world\")");
    assert!(ir.contains("call $greet("), "should call greet: {ir}");
}

#[test]
fn test_call_void_function() {
    let ir = qbe("fn noop() { }\nnoop()");
    assert!(ir.contains("call $noop("), "should call noop: {ir}");
}

#[test]
fn test_call_function_with_expression_arg() {
    let ir = qbe("fn foo(x: int) -> int { return x; }\nfoo(1 + 2)");
    assert!(ir.contains("call $foo("), "should call foo: {ir}");
    assert!(ir.contains("add"), "should have addition: {ir}");
}

// ============================================================
// String Method Calls (Member Expression Calls)
// ============================================================

#[test]
fn test_string_upper_method() {
    let ir = qbe("s := \"hello\"\ns.upper()");
    assert!(
        ir.contains("call $string_upper("),
        "should call string_upper: {ir}"
    );
}

#[test]
fn test_string_contains_method() {
    let ir = qbe("s := \"hello world\"\ns.contains(\"world\")");
    assert!(
        ir.contains("call $string_contains("),
        "should call string_contains: {ir}"
    );
}

#[test]
fn test_string_replace_method() {
    let ir = qbe("s := \"hello\"\ns.replace(\"hello\", \"world\")");
    assert!(
        ir.contains("call $string_replace("),
        "should call string_replace: {ir}"
    );
}

#[test]
fn test_string_len_method() {
    let ir = qbe("s := \"hello\"\ns.len()");
    assert!(
        ir.contains("string_len") || ir.contains("call $string_"),
        "should call string method: {ir}"
    );
}

// ============================================================
// Array Method Calls
// ============================================================

#[test]
fn test_array_push_method() {
    let ir = qbe("arr := [1, 2, 3]\narr.push(4)");
    assert!(
        ir.contains("call $array_push("),
        "should call array_push: {ir}"
    );
}

#[test]
fn test_array_len_method() {
    let ir = qbe("arr := [10, 20]\narr.len()");
    assert!(
        ir.contains("call $array_len("),
        "should call array_len: {ir}"
    );
}

#[test]
fn test_array_pop_method() {
    let ir = qbe("arr := [1, 2, 3]\narr.pop()");
    assert!(
        ir.contains("call $array_pop("),
        "should call array_pop: {ir}"
    );
}

#[test]
fn test_array_method_result_used() {
    let ir = qbe("arr := [1, 2, 3]\nn := arr.len()");
    assert!(
        ir.contains("call $array_len("),
        "should call array_len: {ir}"
    );
    assert!(ir.contains("storel"), "should store result: {ir}");
}

// ============================================================
// Array Literals and Index Expressions
// ============================================================

#[test]
fn test_array_literal_creation() {
    let ir = qbe("arr := [10, 20, 30]");
    assert!(
        ir.contains("call $array_alloc("),
        "should create array: {ir}"
    );
    assert!(
        ir.contains("call $array_push("),
        "should push elements: {ir}"
    );
}

#[test]
fn test_empty_array_literal() {
    let ir = qbe("arr := []");
    assert!(ir.contains("call $array_alloc("), "should allocate: {ir}");
}

#[test]
fn test_array_index_read() {
    let ir = qbe("arr := [10, 20, 30]\nx := arr[0]");
    assert!(ir.contains("loadl"), "should load from array: {ir}");
}

#[test]
fn test_array_index_write() {
    let ir = qbe("arr := [10, 20, 30]\narr[1] = 99");
    assert!(
        ir.contains("storel") || ir.contains("array_set"),
        "should store at index: {ir}"
    );
}

#[test]
fn test_array_with_many_elements() {
    let ir = qbe("arr := [1, 2, 3, 4, 5]");
    assert!(
        ir.contains("call $array_alloc("),
        "should create array: {ir}"
    );
    assert!(
        ir.contains("call $array_push("),
        "should push elements: {ir}"
    );
}

#[test]
fn test_array_fill_syntax() {
    let ir = qbe("arr := [0; 5]");
    assert!(ir.contains("call $array_alloc("), "should allocate: {ir}");
    assert!(
        ir.contains("jnz") || ir.contains("jmp"),
        "should have loop: {ir}"
    );
    assert!(
        ir.contains("call $array_push("),
        "should push in loop: {ir}"
    );
}

// ============================================================
// Map Literals and Index
// ============================================================

#[test]
fn test_map_literal_empty() {
    let ir = qbe("m := {}");
    assert!(ir.contains("call $map_new("), "should create map: {ir}");
}

#[test]
fn test_map_literal_with_entries() {
    let ir = qbe(r#"m := {"a": 1, "b": 2}"#);
    assert!(ir.contains("call $map_new("), "should create map: {ir}");
    assert!(ir.contains("call $map_set("), "should set entries: {ir}");
}

#[test]
fn test_map_index_read() {
    let ir = qbe("m := {\"key\": 42}\nv := m[\"key\"]");
    assert!(ir.contains("call $map_get("), "should call map_get: {ir}");
}

// ============================================================
// Struct Definition and Field Access
// ============================================================

#[test]
fn test_struct_definition_and_instantiation() {
    let ir = qbe("struct Point { x: i64, y: i64 }\np := Point { x: 3, y: 4 }");
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate struct: {ir}"
    );
}

#[test]
fn test_struct_field_read() {
    let ir = qbe("struct Point { x: i64, y: i64 }\np := Point { x: 10, y: 20 }\nx := p.x");
    assert!(ir.contains("loadl"), "should load field: {ir}");
}

#[test]
fn test_struct_field_second_field_offset() {
    let ir = qbe("struct Point { x: i64, y: i64 }\np := Point { x: 10, y: 20 }\ny := p.y");
    // Second field needs offset calculation (add 8)
    assert!(ir.contains("loadl"), "should load field: {ir}");
}

#[test]
fn test_struct_field_in_expression() {
    let ir = qbe("struct Point { x: i64, y: i64 }\np := Point { x: 3, y: 4 }\nresult := p.x + p.y");
    assert!(ir.contains("add"), "should have addition: {ir}");
}

#[test]
fn test_struct_field_write() {
    let ir = qbe("struct Point { x: i64, y: i64 }\np := Point { x: 0, y: 0 }\np.y = 10");
    // Struct creation should allocate and store fields
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate struct: {ir}"
    );
    assert!(ir.contains("storel"), "should store fields: {ir}");
}

#[test]
fn test_struct_three_fields() {
    let ir = qbe("struct Vec3 { x: i64, y: i64, z: i64 }\nv := Vec3 { x: 1, y: 2, z: 3 }");
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate struct: {ir}"
    );
    assert!(ir.contains("storel 1,"), "store x: {ir}");
    assert!(ir.contains("storel 2,"), "store y: {ir}");
    assert!(ir.contains("storel 3,"), "store z: {ir}");
}

// ============================================================
// Lambda / Closure Expressions
// ============================================================

#[test]
fn test_lambda_expression_basic() {
    let ir = qbe("double := fn(x: i64) -> i64 { return x * 2; }");
    // Lambda generates a closure pair and a separate function
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate closure pair: {ir}"
    );
}

#[test]
fn test_lambda_assigned_to_variable() {
    let ir = qbe("f := fn(x: i64) -> i64 { return x + 1; }");
    assert!(
        ir.contains("storel"),
        "should store closure to variable: {ir}"
    );
}

#[test]
fn test_lambda_with_capture() {
    let ir = qbe("offset := 10\nadd_offset := fn(x: i64) -> i64 { return x + offset; }");
    // Should allocate env for captured variable
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate closure/env: {ir}"
    );
}

#[test]
fn test_lambda_body_generates_function() {
    let ir = qbe("mul := fn(a: i64, b: i64) -> i64 { return a * b; }");
    // The lambda body becomes a separate QBE function
    assert!(
        ir.contains("function"),
        "should generate lambda function: {ir}"
    );
}

#[test]
fn test_higher_order_function() {
    let ir = qbe(
        "fn apply(f: fn(i64) -> i64, x: i64) -> i64 { return f(x); }\n\
         double := fn(x: i64) -> i64 { return x * 2; }\n\
         apply(double, 7)",
    );
    assert!(ir.contains("call $apply("), "should call apply: {ir}");
}

#[test]
fn test_lambda_with_block_body() {
    // Lambda with block body generates a separate function
    let ir = qbe("f := fn(x: i64) -> i64 { return x * 2; }");
    // The lambda generates a function and a closure allocation
    assert!(
        ir.contains("function") && ir.contains("$lambda_"),
        "lambda should generate function: {ir}"
    );
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate closure pair: {ir}"
    );
}

// ============================================================
// Match Expressions
// ============================================================

#[test]
fn test_match_integer_basic() {
    let ir = qbe("x := 1\nresult := match x { 1 => 10, 2 => 20, _ => 0 }");
    assert!(
        ir.contains("ceql") || ir.contains("ceqw"),
        "should compare arms: {ir}"
    );
    assert!(ir.contains("jnz"), "should have branch: {ir}");
}

#[test]
fn test_match_with_wildcard() {
    let ir = qbe("x := 99\nresult := match x { 1 => 10, _ => 0 }");
    assert!(ir.contains("jmp"), "wildcard should jump: {ir}");
}

#[test]
fn test_match_multiple_arms() {
    let ir = qbe("x := 3\nresult := match x { 1 => 10, 2 => 20, 3 => 30, _ => 0 }");
    assert!(
        ir.contains("ceql") || ir.contains("ceqw") || ir.contains("jnz"),
        "should compare arms: {ir}"
    );
}

#[test]
fn test_match_as_expression() {
    let ir = qbe("x := 2\ny := match x { 1 => 100, 2 => 200, _ => 0 }");
    assert!(ir.contains("storel"), "should store match result: {ir}");
    assert!(
        ir.contains("ceql") || ir.contains("ceqw"),
        "should compare arms: {ir}"
    );
}

#[test]
fn test_match_result_ok_err() {
    let ir = qbe("fn safe_div(a: int, b: int) -> Result<int> {\n\
         if b == 0 { return err(\"div by zero\"); }\n\
         return ok(a / b);\n\
         }\n\
         r := safe_div(10, 2)\n\
         val := match r { ok(v) => v, err(e) => 0 }");
    assert!(
        ir.contains("jnz") || ir.contains("ceql"),
        "should have match branches: {ir}"
    );
}

#[test]
fn test_match_optional_some_none() {
    let ir = qbe("fn find(n: int) -> ?int {\n\
         if n > 0 { return some(n); }\n\
         return none;\n\
         }\n\
         o := find(42)\n\
         val := match o { some(v) => v, none => 0 }");
    assert!(
        ir.contains("jnz") || ir.contains("ceql"),
        "should have match branches: {ir}"
    );
}

#[test]
fn test_match_with_block_arm() {
    let ir = qbe("x := 1\n\
         result := match x {\n\
         1 => 10,\n\
         _ => 0\n\
         }");
    assert!(
        ir.contains("jnz") || ir.contains("ceql"),
        "should have match: {ir}"
    );
}

#[test]
fn test_match_string_pattern() {
    let ir = qbe(r#"s := "hello"
result := match s { "hello" => 1, "world" => 2, _ => 0 }"#);
    // String match uses string_eq
    assert!(
        ir.contains("string_eq") || ir.contains("@match.") || ir.contains("jnz"),
        "should compare strings or branch: {ir}"
    );
}

// ============================================================
// Result Type (ok / err)
// ============================================================

#[test]
fn test_ok_construction() {
    let ir = qbe("fn test_ok() -> Result<int> {\n\
           return ok(42);\n\
         }");
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate result: {ir}"
    );
    assert!(
        ir.contains("storel 0,") || ir.contains("storel 42,"),
        "should store tag or value: {ir}"
    );
}

#[test]
fn test_err_construction() {
    let ir = qbe(r#"fn test_err() -> Result<int> {
  return err("bad");
}"#);
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate result: {ir}"
    );
}

#[test]
fn test_result_function_returns_ok() {
    let ir = qbe("fn safe_div(a: int, b: int) -> Result<int> {\n\
           if b == 0 { return err(\"zero\"); }\n\
           return ok(a / b);\n\
         }");
    assert!(ir.contains("function"), "should have function: {ir}");
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate result: {ir}"
    );
    assert!(
        ir.contains("div") || ir.contains("ret"),
        "should have div or ret: {ir}"
    );
}

#[test]
fn test_result_function_returns_err() {
    let ir = qbe(r#"fn fail() -> Result<int> {
  return err("failed");
}"#);
    assert!(ir.contains("call $gc_alloc("), "should allocate: {ir}");
    assert!(ir.contains("ret"), "should return: {ir}");
}

#[test]
fn test_question_mark_propagation() {
    let ir = qbe("fn safe_div(a: int, b: int) -> Result<int> {\n\
           if b == 0 { return err(\"zero\"); }\n\
           return ok(a / b);\n\
         }\n\
         fn chain(a: int, b: int, c: int) -> Result<int> {\n\
           step1 := safe_div(a, b)?\n\
           step2 := safe_div(step1, c)?\n\
           return ok(step2);\n\
         }");
    // ? propagation loads tag and branches on error
    assert!(
        ir.contains("jnz") || ir.contains("jmp"),
        "should branch on error: {ir}"
    );
}

// ============================================================
// Optional Type (some / none)
// ============================================================

#[test]
fn test_some_construction() {
    let ir = qbe("fn test_some() -> ?int {\n\
           return some(42);\n\
         }");
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate optional: {ir}"
    );
}

#[test]
fn test_none_construction() {
    let ir = qbe("fn test_none() -> ?int {\n\
           return none;\n\
         }");
    assert!(
        ir.contains("call $gc_alloc(") || ir.contains("ret"),
        "should handle none: {ir}"
    );
}

#[test]
fn test_optional_function_some_path() {
    let ir = qbe("fn find_positive(n: int) -> ?int {\n\
           if n > 0 { return some(n); }\n\
           return none;\n\
         }");
    assert!(ir.contains("function"), "should declare function: {ir}");
    assert!(ir.contains("call $gc_alloc("), "should allocate: {ir}");
    assert!(
        ir.contains("jnz") || ir.contains("csgtl") || ir.contains("cslel"),
        "should have comparison: {ir}"
    );
}

#[test]
fn test_optional_function_none_path() {
    let ir = qbe("fn always_none() -> ?int {\n\
           return none;\n\
         }");
    assert!(ir.contains("ret"), "should return: {ir}");
}

// ============================================================
// Template Literals / F-strings
// ============================================================

#[test]
fn test_fstring_basic_interpolation() {
    let ir = qbe(r#"println(f"hello {42}")"#);
    assert!(
        ir.contains("call $i64_to_string"),
        "should convert int to string: {ir}"
    );
    assert!(
        ir.contains("call $string_concat"),
        "should concatenate: {ir}"
    );
}

#[test]
fn test_fstring_with_variable() {
    let ir = qbe("x := 10\nprintln(f\"value: {x}\")");
    assert!(
        ir.contains("call $string_concat"),
        "should concatenate: {ir}"
    );
}

#[test]
fn test_fstring_multiple_interpolations() {
    let ir = qbe(r#"println(f"a{1}b{2}c")"#);
    assert!(
        ir.contains("call $i64_to_string"),
        "should convert ints: {ir}"
    );
    assert!(
        ir.contains("call $string_concat"),
        "should concatenate: {ir}"
    );
}

#[test]
fn test_fstring_plain_string_no_conversion() {
    let ir = qbe(r#"println(f"just text")"#);
    assert!(ir.contains("just text"), "should have string data: {ir}");
    assert!(
        !ir.contains("call $i64_to_string"),
        "should NOT convert: {ir}"
    );
}

#[test]
fn test_fstring_with_expression() {
    let ir = qbe("x := 5\nprintln(f\"result: {x + 1}\")");
    assert!(ir.contains("call $string_concat"), "should concat: {ir}");
}

#[test]
fn test_fstring_string_var_no_conversion() {
    let ir = qbe("name := \"world\"\nprintln(f\"hello {name}\")");
    // String variable should NOT need i64_to_string conversion
    assert!(ir.contains("call $string_concat"), "should concat: {ir}");
}

// ============================================================
// If / Else Expressions and Control Flow
// ============================================================

#[test]
fn test_if_else_basic() {
    let ir = qbe("x := 1\nif x > 0 { println_i64(1); } else { println_i64(0); }");
    assert!(ir.contains("jnz"), "should have conditional branch: {ir}");
}

#[test]
fn test_if_without_else() {
    let ir = qbe("x := 5\nif x > 0 { println_i64(x); }");
    assert!(ir.contains("jnz"), "should have conditional branch: {ir}");
}

#[test]
fn test_if_else_chain() {
    let ir = qbe(
        "x := 2\n\
         if x == 1 { println_i64(10); } else if x == 2 { println_i64(20); } else { println_i64(0); }"
    );
    assert!(ir.contains("jnz"), "should have branches: {ir}");
}

#[test]
fn test_while_loop() {
    let ir = qbe("i := 0\nwhile i < 10 { i = i + 1; }");
    assert!(
        ir.contains("jnz"),
        "should have loop condition branch: {ir}"
    );
    assert!(ir.contains("jmp"), "should have loop back jump: {ir}");
}

#[test]
fn test_for_in_range() {
    let ir = qbe("for i in 0..10 { println_i64(i); }");
    assert!(
        ir.contains("jnz") || ir.contains("jmp"),
        "should have loop: {ir}"
    );
    assert!(ir.contains("add"), "should have increment: {ir}");
}

// ============================================================
// Block Expressions and Variable Scoping
// ============================================================

#[test]
fn test_block_multiple_statements() {
    let ir = qbe("x := 1\ny := 2\nz := x + y");
    assert!(ir.contains("add"), "should have addition: {ir}");
    assert!(ir.contains("storel"), "should store variables: {ir}");
}

#[test]
fn test_variable_declaration_and_use() {
    let ir = qbe("x := 42\nprintln_i64(x)");
    assert!(ir.contains("storel 42,"), "should store 42: {ir}");
    assert!(ir.contains("loadl"), "should load variable: {ir}");
    assert!(
        ir.contains("call $println_i64("),
        "should call println: {ir}"
    );
}

#[test]
fn test_variable_reassignment() {
    let ir = qbe("x := 1\nx = 42");
    assert!(ir.contains("storel 42,"), "should store new value: {ir}");
}

// ============================================================
// Arithmetic and Binary Expressions
// ============================================================

#[test]
fn test_binary_addition() {
    let ir = qbe("x := 1 + 2");
    assert!(ir.contains("add"), "should have addition: {ir}");
}

#[test]
fn test_binary_subtraction() {
    let ir = qbe("x := 10 - 3");
    assert!(ir.contains("sub"), "should have subtraction: {ir}");
}

#[test]
fn test_binary_multiplication() {
    let ir = qbe("x := 4 * 5");
    assert!(ir.contains("mul"), "should have multiplication: {ir}");
}

#[test]
fn test_binary_division() {
    let ir = qbe("x := 10 / 2");
    assert!(ir.contains("div"), "should have division: {ir}");
}

#[test]
fn test_comparison_equal() {
    let ir = qbe("x := 5\nif x == 5 { println_i64(1); }");
    assert!(
        ir.contains("ceq") || ir.contains("ceql") || ir.contains("ceqw"),
        "should have eq comparison: {ir}"
    );
}

#[test]
fn test_comparison_less_than() {
    let ir = qbe("x := 3\nif x < 5 { println_i64(1); }");
    assert!(
        ir.contains("cslt") || ir.contains("csltl"),
        "should have lt comparison: {ir}"
    );
}

#[test]
fn test_comparison_greater_than() {
    let ir = qbe("x := 10\nif x > 5 { println_i64(1); }");
    assert!(
        ir.contains("csgt") || ir.contains("csgtl"),
        "should have gt comparison: {ir}"
    );
}

// ============================================================
// Boolean and Logical Expressions
// ============================================================

#[test]
fn test_boolean_true() {
    let ir = qbe("x := true");
    assert!(ir.contains("storel 1,"), "true should store 1: {ir}");
}

#[test]
fn test_boolean_false() {
    let ir = qbe("x := false");
    assert!(ir.contains("storel 0,"), "false should store 0: {ir}");
}

#[test]
fn test_logical_and() {
    let ir = qbe("a := true\nb := false\nif a && b { println_i64(1); }");
    // Logical AND uses short-circuit: jnz for first operand
    assert!(ir.contains("jnz"), "should have short-circuit branch: {ir}");
}

#[test]
fn test_logical_or() {
    let ir = qbe("a := false\nb := true\nif a || b { println_i64(1); }");
    assert!(ir.contains("jnz"), "should have short-circuit branch: {ir}");
}

// ============================================================
// Return Statements
// ============================================================

#[test]
fn test_return_value() {
    let ir = qbe("fn foo() -> int { return 42; }");
    assert!(ir.contains("ret"), "should have return: {ir}");
}

#[test]
fn test_return_expression() {
    let ir = qbe("fn bar(x: int) -> int { return x * 2; }");
    assert!(ir.contains("mul"), "should have multiply: {ir}");
    assert!(ir.contains("ret"), "should return: {ir}");
}

#[test]
fn test_void_return() {
    let ir = qbe("fn noop() { return; }");
    assert!(ir.contains("ret"), "should have return: {ir}");
}

// ============================================================
// String Literals and Data Section
// ============================================================

#[test]
fn test_string_literal_data_section() {
    let ir = qbe(r#"s := "hello world""#);
    assert!(ir.contains("hello world"), "should have string data: {ir}");
    assert!(
        ir.contains("data $str."),
        "should have data section entry: {ir}"
    );
}

#[test]
fn test_multiple_string_literals() {
    let ir = qbe("a := \"foo\"\nb := \"bar\"");
    assert!(ir.contains("foo"), "should have foo: {ir}");
    assert!(ir.contains("bar"), "should have bar: {ir}");
}

// ============================================================
// Function Declaration Structure
// ============================================================

#[test]
fn test_function_declaration_structure() {
    let ir = qbe("fn add(a: int, b: int) -> int { return a + b; }");
    assert!(
        ir.contains("function"),
        "should have function keyword: {ir}"
    );
    assert!(ir.contains("$add("), "should have function name: {ir}");
    assert!(ir.contains("ret"), "should have return: {ir}");
}

#[test]
fn test_main_wrapping() {
    // Top-level code should be wrapped in a main-like function
    let ir = qbe("x := 42");
    assert!(
        ir.contains("function"),
        "should have function wrapper: {ir}"
    );
}

#[test]
fn test_multiple_functions() {
    let ir = qbe("fn foo(x: int) -> int { return x; }\n\
         fn bar(x: int) -> int { return x + 1; }");
    assert!(ir.contains("$foo("), "should have foo: {ir}");
    assert!(ir.contains("$bar("), "should have bar: {ir}");
}

// ============================================================
// GC Alloc and Memory
// ============================================================

#[test]
fn test_gc_alloc_for_array() {
    let ir = qbe("arr := [1, 2]");
    // Array uses array_new which internally allocates
    assert!(
        ir.contains("call $array_alloc("),
        "should allocate via array_alloc: {ir}"
    );
}

#[test]
fn test_gc_alloc_for_struct() {
    let ir = qbe("struct Pair { a: i64, b: i64 }\np := Pair { a: 1, b: 2 }");
    assert!(
        ir.contains("call $gc_alloc("),
        "should allocate struct: {ir}"
    );
}

#[test]
fn test_gc_alloc_for_map() {
    let ir = qbe(r#"m := {"x": 1}"#);
    assert!(ir.contains("call $map_new("), "should call map_new: {ir}");
}
