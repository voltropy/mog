//! Feature tests ported from the TypeScript test suite.
//!
//! Covers closures, functions, strings, and arrays across all compiler stages:
//! lexer → parser → analyzer → QBE codegen.

use mog::ast::{ExprKind, StatementKind};
use mog::compiler::{compile, CompileOptions};
use mog::lexer::tokenize;
use mog::parser::parse;
use mog::token::TokenType;
use mog::types::{FloatKind, IntegerKind, Type};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Tokenise, filter whitespace/comments, parse.
fn parse_src(src: &str) -> mog::ast::Statement {
    let tokens = tokenize(src);
    parse(&tokens)
}

/// Full pipeline through the analyzer.
fn analyze_src(src: &str) -> (mog::ast::Statement, Vec<mog::analyzer::SemanticError>) {
    let ast = parse_src(src);
    let mut analyzer = mog::analyzer::SemanticAnalyzer::new();
    let errors = analyzer.analyze(&ast);
    (ast, errors)
}

/// Compile to QBE IR and return (ir, errors).
fn compile_src(src: &str) -> (String, Vec<String>) {
    let result = compile(src, &CompileOptions::default());
    (result.ir, result.errors)
}

/// Extract the list of statements from a parsed Program node.
fn program_stmts(stmt: &mog::ast::Statement) -> &[mog::ast::Statement] {
    match &stmt.kind {
        StatementKind::Program { statements, .. } => statements,
        _ => panic!("expected Program, got {:?}", stmt.kind),
    }
}

/// Get the inner statements, handling the parser's unwrap_statements behaviour.
/// When `{ ... }` contains multiple statements it stays as Block; with a single
/// statement the Block gets unwrapped by parse_program.
fn inner_stmts(ast: &mog::ast::Statement) -> &[mog::ast::Statement] {
    let stmts = program_stmts(ast);
    if stmts.len() == 1 {
        if let StatementKind::Block { statements, .. } = &stmts[0].kind {
            return statements;
        }
    }
    stmts
}

/// Find a function declaration by name in top-level statements.
fn find_fn_decl<'a>(stmts: &'a [mog::ast::Statement], name: &str) -> &'a mog::ast::Statement {
    stmts
        .iter()
        .find(
            |s| matches!(&s.kind, StatementKind::FunctionDeclaration { name: n, .. } if n == name),
        )
        .unwrap_or_else(|| panic!("no FunctionDeclaration named '{name}'"))
}

fn has_error_containing(errors: &[mog::analyzer::SemanticError], substr: &str) -> bool {
    errors
        .iter()
        .any(|e| e.message.to_lowercase().contains(&substr.to_lowercase()))
}

// ===========================================================================
//  CLOSURES  — Parser
// ===========================================================================

#[test]
fn closure_parse_lambda_with_params() {
    let ast = parse_src("{ f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x * 2; }; }");
    let stmts = inner_stmts(&ast);
    if let StatementKind::VariableDeclaration {
        value: Some(val), ..
    } = &stmts[0].kind
    {
        assert!(
            matches!(val.kind, ExprKind::Lambda { .. }),
            "expected Lambda, got {:?}",
            val.kind
        );
        if let ExprKind::Lambda { params, .. } = &val.kind {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
        }
    } else {
        panic!(
            "expected VariableDeclaration with value, got {:?}",
            stmts[0].kind
        );
    }
}

#[test]
fn closure_parse_lambda_no_params() {
    let ast = parse_src("{ f: fn() -> i64 = fn() -> i64 { return 42; }; }");
    let stmts = inner_stmts(&ast);
    if let StatementKind::VariableDeclaration {
        value: Some(val), ..
    } = &stmts[0].kind
    {
        if let ExprKind::Lambda { params, .. } = &val.kind {
            assert_eq!(params.len(), 0);
        } else {
            panic!("expected Lambda");
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

#[test]
fn closure_parse_lambda_multiple_params() {
    let ast =
        parse_src("{ f: fn(i64, i64) -> i64 = fn(a: i64, b: i64) -> i64 { return a + b; }; }");
    let stmts = inner_stmts(&ast);
    if let StatementKind::VariableDeclaration {
        value: Some(val), ..
    } = &stmts[0].kind
    {
        if let ExprKind::Lambda { params, .. } = &val.kind {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "a");
            assert_eq!(params[1].name, "b");
        } else {
            panic!("expected Lambda");
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// ===========================================================================
//  CLOSURES  — Analyzer (capture analysis)
// ===========================================================================

#[test]
fn closure_analyzer_no_captures() {
    let (_, errors) = analyze_src("{ f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x * 2; }; }");
    assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
}

#[test]
fn closure_analyzer_capturing_one_variable() {
    let (_, errors) =
        analyze_src("{ n: i64 = 10; f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + n; }; }");
    assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
}

#[test]
fn closure_analyzer_capturing_multiple_variables() {
    let (_, errors) = analyze_src(
        "{ a: i64 = 1; b: i64 = 2; f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + a + b; }; }",
    );
    assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
}

#[test]
fn closure_analyzer_does_not_error_on_params() {
    let (_, errors) = analyze_src("{ f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x * 2; }; }");
    assert!(errors.is_empty());
}

#[test]
fn closure_analyzer_does_not_error_on_local_vars() {
    let (_, errors) =
        analyze_src("{ f: fn(i64) -> i64 = fn(x: i64) -> i64 { y: i64 = x * 2; return y; }; }");
    assert!(errors.is_empty());
}

// ===========================================================================
//  CLOSURES  — Codegen (compile to QBE IR)
// ===========================================================================

#[test]
fn closure_codegen_non_capturing_lambda() {
    let src = r#"
        fn main() -> i64 {
            f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x * 2; };
            return f(21);
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(!ir.is_empty(), "IR should not be empty");
}

#[test]
fn closure_codegen_capturing_one_var() {
    let src = r#"
        fn main() -> i64 {
            n: i64 = 10;
            f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + n; };
            return f(5);
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(!ir.is_empty());
}

#[test]
fn closure_codegen_capturing_multiple_vars() {
    let src = r#"
        fn main() -> i64 {
            a: i64 = 1;
            b: i64 = 2;
            f: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + a + b; };
            return f(10);
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(!ir.is_empty());
}

#[test]
fn closure_codegen_named_function_still_works() {
    let src = r#"
        fn add(a: i64, b: i64) -> i64 {
            return a + b;
        }
        fn main() -> i64 {
            return add(30, 12);
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("$add"), "IR should contain add function");
}

#[test]
fn closure_codegen_non_capturing_no_params() {
    let src = r#"
        fn main() -> i64 {
            f: fn() -> i64 = fn() -> i64 { return 42; };
            return f();
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(!ir.is_empty());
}

#[test]
fn closure_codegen_higher_order_passing_as_arg() {
    let src = r#"
        fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
            return f(x);
        }
        fn main() -> i64 {
            n: i64 = 10;
            adder: fn(i64) -> i64 = fn(x: i64) -> i64 { return x + n; };
            return apply(adder, 5);
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("$apply"), "IR should contain apply function");
}

#[test]
fn closure_codegen_make_adder_pattern() {
    let src = r#"
        fn make_adder(n: i64) -> fn(i64) -> i64 {
            return fn(x: i64) -> i64 { return x + n; };
        }
        fn main() -> i64 {
            add10: fn(i64) -> i64 = make_adder(10);
            return add10(5);
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("$make_adder"), "IR should contain make_adder");
}

#[test]
fn closure_codegen_multiple_closures_different_captures() {
    let src = r#"
        fn main() -> i64 {
            a: i64 = 10;
            b: i64 = 20;
            f: fn() -> i64 = fn() -> i64 { return a; };
            g: fn() -> i64 = fn() -> i64 { return b; };
            return f() + g();
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(!ir.is_empty());
}

// ===========================================================================
//  CLOSURES  — Array filter/map with closures
// ===========================================================================

#[test]
fn closure_codegen_array_filter_inline_lambda() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1, 2, 3, 4, 5, 6];
            evens: [i64] = arr.filter(fn(x: i64) -> i64 { return (x % 2) == 0; });
            return evens.len();
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_filter"), "IR should call array_filter");
}

#[test]
fn closure_codegen_array_map_inline_lambda() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1, 2, 3];
            doubled: [i64] = arr.map(fn(x: i64) -> i64 { return x * 2; });
            return doubled.pop();
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_map"), "IR should call array_map");
}

#[test]
fn closure_codegen_array_filter_with_capture() {
    let src = r#"
        fn main() -> i64 {
            threshold: i64 = 3;
            arr: [i64] = [1, 2, 3, 4, 5];
            big: [i64] = arr.filter(fn(x: i64) -> i64 { return x > threshold; });
            return big.len();
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_filter"));
}

#[test]
fn closure_codegen_array_sort_with_comparator() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [3, 1, 4, 1, 5, 9, 2, 6];
            arr.sort(fn(a: i64, b: i64) -> i64 { return b - a; });
            return arr[0];
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("array_sort_with_comparator") || ir.contains("array_sort"),
        "IR should call sort"
    );
}

// ===========================================================================
//  FUNCTIONS  — Type system
// ===========================================================================

#[test]
fn function_type_construction() {
    let ft = Type::function(
        vec![
            Type::Integer(IntegerKind::I64),
            Type::Integer(IntegerKind::I64),
        ],
        Type::Integer(IntegerKind::I64),
    );
    assert!(ft.is_function());
    if let Type::Function {
        param_types,
        return_type,
    } = &ft
    {
        assert_eq!(param_types.len(), 2);
        assert!(return_type.is_integer());
    } else {
        panic!("expected Function type");
    }
}

#[test]
fn function_type_display() {
    let ft = Type::function(vec![Type::Integer(IntegerKind::I64)], Type::Bool);
    let s = format!("{ft}");
    assert!(
        s.contains("fn") && s.contains("i64") && s.contains("bool"),
        "display: {s}"
    );
}

#[test]
fn function_type_same_type() {
    let ft1 = Type::function(
        vec![Type::Integer(IntegerKind::I64)],
        Type::Integer(IntegerKind::I64),
    );
    let ft2 = Type::function(
        vec![Type::Integer(IntegerKind::I64)],
        Type::Integer(IntegerKind::I64),
    );
    let ft3 = Type::function(
        vec![Type::Float(FloatKind::F64)],
        Type::Integer(IntegerKind::I64),
    );
    assert!(ft1.same_type(&ft2));
    assert!(!ft1.same_type(&ft3));
}

#[test]
fn function_type_is_function_predicate() {
    let ft = Type::function(vec![], Type::Integer(IntegerKind::I64));
    assert!(ft.is_function());
    assert!(!Type::Integer(IntegerKind::I64).is_function());
}

#[test]
fn function_type_nested() {
    // fn(i64) -> fn(i64) -> i64
    let inner = Type::function(
        vec![Type::Integer(IntegerKind::I64)],
        Type::Integer(IntegerKind::I64),
    );
    let outer = Type::function(vec![Type::Integer(IntegerKind::I64)], inner);
    assert!(outer.is_function());
    if let Type::Function { return_type, .. } = &outer {
        assert!(return_type.is_function());
    }
}

// ===========================================================================
//  FUNCTIONS  — Parser
// ===========================================================================

#[test]
fn function_parse_basic_declaration() {
    let ast = parse_src("{ fn add(a: i64, b: i64) -> i64 { return a + b; } add(1, 2); }");
    let stmts = inner_stmts(&ast);
    let has_fn = stmts
        .iter()
        .any(|s| matches!(&s.kind, StatementKind::FunctionDeclaration { .. }));
    assert!(has_fn, "should contain FunctionDeclaration");
}

#[test]
fn function_parse_single_expression_body() {
    let ast = parse_src("{ fn double(x: i64) -> i64 { x * 2 } }");
    let stmts = inner_stmts(&ast);
    if let StatementKind::FunctionDeclaration { body, .. } = &stmts[0].kind {
        assert_eq!(body.statements.len(), 1);
        // Single expression should be wrapped as Return
        assert!(
            matches!(body.statements[0].kind, StatementKind::Return { .. }),
            "expected Return, got {:?}",
            body.statements[0].kind
        );
    } else {
        panic!("expected FunctionDeclaration, got {:?}", stmts[0].kind);
    }
}

#[test]
fn function_parse_explicit_return() {
    let ast = parse_src("{ fn double(x: i64) -> i64 { return x * 2; } }");
    let stmts = inner_stmts(&ast);
    if let StatementKind::FunctionDeclaration { body, .. } = &stmts[0].kind {
        assert!(matches!(
            body.statements[0].kind,
            StatementKind::Return { .. }
        ));
    } else {
        panic!("expected FunctionDeclaration, got {:?}", stmts[0].kind);
    }
}

#[test]
fn function_parse_lambda_single_expr_body() {
    let ast = parse_src("{ f := fn(x: i64) -> i64 { x + 1 }; }");
    let stmts = inner_stmts(&ast);
    // Navigate into ExpressionStatement -> AssignmentExpression -> Lambda
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        if let ExprKind::AssignmentExpression { value, .. } = &expression.kind {
            if let ExprKind::Lambda { body, .. } = &value.kind {
                assert_eq!(body.statements.len(), 1);
                assert!(matches!(
                    body.statements[0].kind,
                    StatementKind::Return { .. }
                ));
            } else {
                panic!("expected Lambda, got {:?}", value.kind);
            }
        } else {
            panic!("expected AssignmentExpression, got {:?}", expression.kind);
        }
    } else {
        panic!("expected ExpressionStatement, got {:?}", stmts[0].kind);
    }
}

#[test]
fn function_parse_multiple_statements_preserved() {
    let ast = parse_src("{ fn foo(x: i64) -> i64 { y := x + 1; return y * 2; } }");
    let stmts = inner_stmts(&ast);
    if let StatementKind::FunctionDeclaration { body, .. } = &stmts[0].kind {
        assert_eq!(body.statements.len(), 2);
    } else {
        panic!("expected FunctionDeclaration, got {:?}", stmts[0].kind);
    }
}

#[test]
fn function_parse_default_parameter() {
    let ast = parse_src("{ fn greet(name: i64, times: i64 = 1) -> i64 { return name + times; } }");
    let stmts = inner_stmts(&ast);
    if let StatementKind::FunctionDeclaration { params, .. } = &stmts[0].kind {
        assert_eq!(params.len(), 2);
        assert!(params[0].default_value.is_none());
        assert!(params[1].default_value.is_some());
    } else {
        panic!("expected FunctionDeclaration, got {:?}", stmts[0].kind);
    }
}

#[test]
fn function_parse_multiple_defaults() {
    let ast =
        parse_src("{ fn config(x: i64, y: i64 = 10, z: i64 = 20) -> i64 { return x + y + z; } }");
    let stmts = inner_stmts(&ast);
    if let StatementKind::FunctionDeclaration { params, .. } = &stmts[0].kind {
        assert!(params[0].default_value.is_none());
        assert!(params[1].default_value.is_some());
        assert!(params[2].default_value.is_some());
    } else {
        panic!("expected FunctionDeclaration, got {:?}", stmts[0].kind);
    }
}

#[test]
fn function_parse_void_no_return_type() {
    let ast = parse_src("{ fn greet(name: i64) { print_i64(name); } }");
    let stmts = inner_stmts(&ast);
    if let StatementKind::FunctionDeclaration { return_type, .. } = &stmts[0].kind {
        assert!(
            return_type.is_void(),
            "expected Void return type, got: {return_type}"
        );
    } else {
        panic!("expected FunctionDeclaration, got {:?}", stmts[0].kind);
    }
}

#[test]
fn function_parse_named_arguments() {
    let ast = parse_src("{ fn add(a: i64, b: i64) -> i64 { return a + b; } add(a: 1, b: 2); }");
    let stmts = inner_stmts(&ast);
    // Find the call expression
    let call_stmt = stmts
        .iter()
        .find(|s| matches!(&s.kind, StatementKind::ExpressionStatement { .. }))
        .expect("no ExpressionStatement found");
    if let StatementKind::ExpressionStatement { expression } = &call_stmt.kind {
        if let ExprKind::CallExpression { named_args, .. } = &expression.kind {
            let na = named_args.as_ref().expect("expected named_args");
            assert_eq!(na.len(), 2);
            assert_eq!(na[0].name, "a");
            assert_eq!(na[1].name, "b");
        } else {
            panic!("expected CallExpression");
        }
    }
}

#[test]
fn function_parse_function_type_in_param() {
    let ast = parse_src("{ fn apply(f: fn(i64) -> i64, x: i64) -> i64 { return f(x); } }");
    let stmts = inner_stmts(&ast);
    if let StatementKind::FunctionDeclaration { params, .. } = &stmts[0].kind {
        assert!(
            params[0].param_type.is_function(),
            "first param should be fn type"
        );
    } else {
        panic!("expected FunctionDeclaration, got {:?}", stmts[0].kind);
    }
}

#[test]
fn function_parse_function_type_as_return() {
    let ast = parse_src(
        "{ fn make_adder(n: i64) -> fn(i64) -> i64 { return fn(x: i64) -> i64 { x + n }; } }",
    );
    let stmts = inner_stmts(&ast);
    if let StatementKind::FunctionDeclaration { return_type, .. } = &stmts[0].kind {
        assert!(
            return_type.is_function(),
            "return type should be fn type, got: {return_type}"
        );
    } else {
        panic!("expected FunctionDeclaration, got {:?}", stmts[0].kind);
    }
}

// ===========================================================================
//  FUNCTIONS  — Analyzer
// ===========================================================================

#[test]
fn function_analyzer_default_param_valid() {
    let (_, errors) =
        analyze_src("{ fn foo(x: i64, y: i64 = 10) -> i64 { return x + y; } foo(5); }");
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn function_analyzer_named_args_valid() {
    let (_, errors) =
        analyze_src("{ fn add(a: i64, b: i64) -> i64 { return a + b; } add(a: 1, b: 2); }");
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn function_analyzer_mixed_positional_and_named() {
    let (_, errors) = analyze_src(
        "{ fn config(x: i64, y: i64, z: i64) -> i64 { return x + y + z; } config(1, y: 2, z: 3); }",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn function_analyzer_errors_on_unknown_named_arg() {
    let (_, errors) =
        analyze_src("{ fn add(a: i64, b: i64) -> i64 { return a + b; } add(a: 1, c: 2); }");
    assert!(
        has_error_containing(&errors, "unknown named argument"),
        "expected error about unknown named arg, got: {:?}",
        errors
    );
}

#[test]
fn function_analyzer_errors_on_missing_required() {
    let (_, errors) = analyze_src("{ fn add(a: i64, b: i64) -> i64 { return a + b; } add(a: 1); }");
    assert!(
        has_error_containing(&errors, "missing") || has_error_containing(&errors, "argument"),
        "expected error about missing arg, got: {:?}",
        errors
    );
}

#[test]
fn function_analyzer_default_fills_missing_named() {
    let (_, errors) =
        analyze_src("{ fn greet(x: i64, y: i64 = 10) -> i64 { return x + y; } greet(x: 5); }");
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn function_analyzer_positional_covers_required() {
    let (_, errors) = analyze_src("{ fn add(a: i64, b: i64) -> i64 { return a + b; } add(1, 2); }");
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn function_analyzer_closure_captures_accepted() {
    let (_, errors) = analyze_src("{ n: i64 = 5; f := fn(x: i64) -> i64 { x + n }; }");
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn function_analyzer_return_lambda_accepted() {
    let (_, errors) = analyze_src(
        "{ fn make_adder(n: i64) -> fn(i64) -> i64 { return fn(x: i64) -> i64 { x + n }; } }",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn function_analyzer_higher_order_accepted() {
    let (_, errors) = analyze_src(
        "{ fn apply(f: fn(i64) -> i64, x: i64) -> i64 { return f(x); } fn double(x: i64) -> i64 { x * 2 } }",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

// ===========================================================================
//  FUNCTIONS  — Codegen
// ===========================================================================

#[test]
fn function_codegen_single_expression_body() {
    let src = r#"{ fn double(x: i64) -> i64 { x * 2 } double(5); }"#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("$double"), "IR should contain double function");
    assert!(ir.contains("mul"), "IR should contain mul instruction");
}

#[test]
fn function_codegen_default_parameters() {
    let src = r#"{ fn add(a: i64, b: i64 = 10) -> i64 { return a + b; } add(5); }"#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("$add"));
}

#[test]
fn function_codegen_void_function() {
    let src = r#"{ fn greet(x: i64) { print_i64(x); } greet(42); }"#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("$greet"), "IR should contain greet function");
}

#[test]
fn function_codegen_named_arguments() {
    let src = r#"
        { fn config(x: i64, y: i64 = 10, z: i64 = 20) -> i64 { return x + y + z; } config(1, y: 5, z: 15); }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("$config"));
}

#[test]
fn function_codegen_type_alias() {
    let src = r#"
        {
            type Transformer = fn(i64) -> i64;
            fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
                return x;
            }
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn function_codegen_recursive() {
    let src = r#"
        fn factorial(n: i64) -> i64 {
            if (n <= 1) { return 1; }
            return n * factorial(n - 1);
        }
        fn main() -> i64 {
            return factorial(5);
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("$factorial"), "IR should contain factorial");
}

// ===========================================================================
//  STRINGS  — Type system
// ===========================================================================

#[test]
fn string_type_construction() {
    let st = Type::String;
    assert!(st.is_string());
    assert!(!st.is_integer());
}

#[test]
fn string_type_display() {
    let st = Type::String;
    assert_eq!(format!("{st}"), "string");
}

#[test]
fn string_type_same_type() {
    assert!(Type::String.same_type(&Type::String));
    assert!(!Type::String.same_type(&Type::Pointer(None)));
}

#[test]
fn string_type_convenience_constructor() {
    let s = Type::string();
    assert!(s.is_string());
}

// ===========================================================================
//  STRINGS  — Lexer
// ===========================================================================

#[test]
fn string_lexer_type_token() {
    let tokens = tokenize("name: string");
    let type_tok = tokens.iter().find(|t| t.value == "string");
    assert!(type_tok.is_some(), "should find 'string' token");
    assert_eq!(type_tok.unwrap().token_type, TokenType::TypeToken);
}

#[test]
fn string_lexer_string_literal() {
    let tokens = tokenize(r#""hello world""#);
    let str_tok = tokens.iter().find(|t| t.token_type == TokenType::StringLit);
    assert!(str_tok.is_some(), "should find StringLit token");
    // Value should be 'hello world' (without quotes)
    assert!(
        str_tok.unwrap().value.contains("hello world"),
        "value: {}",
        str_tok.unwrap().value
    );
}

#[test]
fn string_lexer_empty_string() {
    let tokens = tokenize(r#""""#);
    let str_tok = tokens.iter().find(|t| t.token_type == TokenType::StringLit);
    assert!(str_tok.is_some(), "should find empty StringLit token");
}

// ===========================================================================
//  STRINGS  — Analyzer
// ===========================================================================

#[test]
fn string_analyzer_literal_infers() {
    let src = r#"fn main() -> i64 { name := "Alice"; return 0; }"#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn string_analyzer_fstring_infers() {
    let src = r#"fn main() -> i64 { name := "Alice"; greeting := f"Hello, {name}!"; return 0; }"#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn string_analyzer_methods_compile() {
    let src = r#"fn main() -> i64 {
        name: string = "Alice";
        upper := name.upper();
        lower := name.lower();
        trimmed := name.trim();
        return 0;
    }"#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn string_analyzer_str_builtin() {
    let src = r#"fn main() -> i64 { s := str(42); return 0; }"#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

// ===========================================================================
//  STRINGS  — Codegen (string methods in IR)
// ===========================================================================

#[test]
fn string_codegen_upper() {
    let src = r#"
        fn main() -> i64 {
            s: string = "hello";
            result: string = s.upper();
            println_string(result);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("string_upper"), "IR should call string_upper");
}

#[test]
fn string_codegen_lower() {
    let src = r#"
        fn main() -> i64 {
            s: string = "HELLO";
            result: string = s.lower();
            println_string(result);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("string_lower"), "IR should call string_lower");
}

#[test]
fn string_codegen_trim() {
    let src = r#"
        fn main() -> i64 {
            s: string = "  hello  ";
            result: string = s.trim();
            println_string(result);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("string_trim"), "IR should call string_trim");
}

#[test]
fn string_codegen_contains() {
    let src = r#"
        fn main() -> i64 {
            s: string = "hello world";
            if (s.contains("world")) {
                println_string("found");
            } else {
                println_string("not found");
            }
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("string_contains"),
        "IR should call string_contains"
    );
}

#[test]
fn string_codegen_starts_with() {
    let src = r#"
        fn main() -> i64 {
            s: string = "hello world";
            if (s.starts_with("hello")) {
                println_string("yes");
            } else {
                println_string("no");
            }
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("string_starts_with"),
        "IR should call string_starts_with"
    );
}

#[test]
fn string_codegen_ends_with() {
    let src = r#"
        fn main() -> i64 {
            s: string = "hello world";
            if (s.ends_with("world")) {
                println_string("yes");
            } else {
                println_string("no");
            }
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("string_ends_with"),
        "IR should call string_ends_with"
    );
}

#[test]
fn string_codegen_replace() {
    let src = r#"
        fn main() -> i64 {
            s: string = "hello world";
            result: string = s.replace("world", "mog");
            println_string(result);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("string_replace"),
        "IR should call string_replace"
    );
}

#[test]
fn string_codegen_split() {
    let src = r#"
        fn main() -> i64 {
            s: string = "a,b,c";
            parts = s.split(",");
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("string_split"), "IR should call string_split");
}

#[test]
#[ignore] // .len property access on strings not yet supported in Rust compiler
fn string_codegen_len_property() {
    let src = r#"
        fn main() -> i64 {
            s: string = "hello";
            len := s.len;
            println_i64(len);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(!ir.is_empty());
}

#[test]
fn string_codegen_concatenation() {
    let src = r#"
        fn main() -> i64 {
            a := "hello ";
            b := "world";
            c := string_concat(a, b);
            println_string(c);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("string_concat"), "IR should call string_concat");
}

#[test]
fn string_codegen_fstring() {
    let src = r#"
        fn main() -> i64 {
            name: string = "World";
            greeting := f"Hello, {name}!";
            println_string(greeting);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn string_codegen_str_int_conversion() {
    let src = r#"
        fn main() -> i64 {
            s := str(42);
            println_string(s);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
#[ignore] // method chaining on strings not fully implemented in Rust QBE codegen
fn string_codegen_method_chaining() {
    let src = r#"
        fn main() -> i64 {
            s: string = "Hello World";
            result: string = s.upper().lower();
            println_string(result);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("string_upper"),
        "IR should contain string_upper"
    );
    assert!(
        ir.contains("string_lower"),
        "IR should contain string_lower"
    );
}

#[test]
fn string_codegen_utf8_literal() {
    let src = r#"
        fn main() -> i64 {
            s := "café";
            println_string(s);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn string_codegen_utf8_cjk() {
    let src = r#"
        fn main() -> i64 {
            s := "hello 世界";
            println_string(s);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
#[ignore] // ptr-to-string backward compat not yet implemented in Rust compiler
fn string_codegen_backward_compat_ptr() {
    let src = r#"
        fn main() -> i64 {
            s: ptr = "hello";
            println_string(s);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

// ===========================================================================
//  ARRAYS  — Codegen (array methods in IR)
// ===========================================================================

#[test]
fn array_codegen_push() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1, 2, 3];
            arr.push(42);
            return 0;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_push"), "IR should call array_push");
}

#[test]
fn array_codegen_pop() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1, 2, 3];
            last := arr.pop();
            return last;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_pop"), "IR should call array_pop");
}

#[test]
fn array_codegen_contains() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1, 2, 3];
            result := arr.contains(2);
            return result;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("array_contains"),
        "IR should call array_contains"
    );
}

#[test]
fn array_codegen_sort_no_args() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [5, 3, 8, 1, 2];
            arr.sort();
            return arr[0];
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_sort"), "IR should call array_sort");
}

#[test]
fn array_codegen_reverse() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1, 2, 3];
            arr.reverse();
            return arr[0];
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_reverse"), "IR should call array_reverse");
}

#[test]
fn array_codegen_len() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1, 2, 3];
            return arr.len();
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_len"), "IR should call array_len");
}

#[test]
fn array_codegen_filter() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1, 2, 3, 4, 5];
            evens: [i64] = arr.filter(fn(x: i64) -> i64 { return (x % 2) == 0; });
            return evens.len();
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_filter"), "IR should call array_filter");
}

#[test]
fn array_codegen_map() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1, 2, 3];
            doubled: [i64] = arr.map(fn(x: i64) -> i64 { return x * 2; });
            return doubled.len();
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_map"), "IR should call array_map");
}

#[test]
fn array_codegen_declaration_and_indexing() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [10, 20, 30];
            return arr[1];
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(!ir.is_empty());
}

#[test]
fn array_codegen_push_then_pop() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1];
            arr.push(99);
            last := arr.pop();
            return last;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("array_push"), "IR should call array_push");
    assert!(ir.contains("array_pop"), "IR should call array_pop");
}

#[test]
fn array_codegen_len_property() {
    // .len property access on arrays uses inline loadl (no function call)
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [1, 2];
            length := arr.len;
            return length;
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    // Property access generates inline load, not a function call
    assert!(!ir.is_empty(), "IR should not be empty");
}

#[test]
fn array_codegen_sort_with_closure_comparator() {
    let src = r#"
        fn main() -> i64 {
            arr: [i64] = [5, 3, 8, 1, 2];
            arr.sort(fn(a: i64, b: i64) -> i64 { return a - b; });
            return arr[0];
        }
    "#;
    let (ir, errors) = compile_src(src);
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("array_sort_with_comparator") || ir.contains("array_sort"),
        "IR should call array sort"
    );
}
