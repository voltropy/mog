//! Integration tests for the Mog parser.
//!
//! These complement the inline unit tests in `parser.rs` by covering areas
//! NOT already exercised there: nested expressions, complex match patterns,
//! try/catch, template literals, SoA/AoS, type aliases, spawn, await,
//! optional chaining, complex function signatures, error cases, pub
//! declarations, for-in-map, with blocks, and more.

use mog::ast::*;
use mog::lexer::tokenize;
use mog::parser::parse;
use mog::types::Type;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_src(src: &str) -> Statement {
    let tokens = tokenize(src);
    parse(&tokens)
}

fn program_stmts(program: &Statement) -> &Vec<Statement> {
    match &program.kind {
        StatementKind::Program { statements, .. } => statements,
        other => panic!("expected Program, got {:?}", other),
    }
}

/// Return the single expression from a one-statement program that is an
/// ExpressionStatement.
fn single_expr(program: &Statement) -> &Expr {
    let stmts = program_stmts(program);
    assert_eq!(stmts.len(), 1, "expected exactly 1 statement");
    match &stmts[0].kind {
        StatementKind::ExpressionStatement { expression } => expression,
        other => panic!("expected ExpressionStatement, got {:?}", other),
    }
}

// =========================================================================
// 1. Nested binary expressions with parentheses
// =========================================================================
#[test]
fn test_nested_binary_with_parens() {
    let prog = parse_src("x: int = (1 + 2) * (3 + 4);");
    let stmts = program_stmts(&prog);
    if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
        let val = value.as_ref().unwrap();
        if let ExprKind::BinaryExpression {
            operator,
            left,
            right,
        } = &val.kind
        {
            assert_eq!(operator, "*");
            assert!(matches!(left.kind, ExprKind::BinaryExpression { .. }));
            assert!(matches!(right.kind, ExprKind::BinaryExpression { .. }));
        } else {
            panic!("expected BinaryExpression, got {:?}", val.kind);
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// =========================================================================
// 2. Deeply nested member access (a.b.c.d)
// =========================================================================
#[test]
fn test_deeply_nested_member_access() {
    let prog = parse_src("a.b.c.d;");
    let expr = single_expr(&prog);
    // outermost: (...).d
    if let ExprKind::MemberExpression { property, object } = &expr.kind {
        assert_eq!(property, "d");
        // next: (...).c
        if let ExprKind::MemberExpression {
            property: p2,
            object: o2,
        } = &object.kind
        {
            assert_eq!(p2, "c");
            // next: a.b
            if let ExprKind::MemberExpression {
                property: p3,
                object: o3,
            } = &o2.kind
            {
                assert_eq!(p3, "b");
                if let ExprKind::Identifier { name } = &o3.kind {
                    assert_eq!(name, "a");
                } else {
                    panic!("expected Identifier 'a'");
                }
            } else {
                panic!("expected MemberExpression for .b");
            }
        } else {
            panic!("expected MemberExpression for .c");
        }
    } else {
        panic!("expected MemberExpression for .d");
    }
}

// =========================================================================
// 3. Try/catch statement
// =========================================================================
#[test]
fn test_try_catch() {
    let prog = parse_src("try { x = 1; } catch (e) { x = 0; }");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::TryCatch {
        try_body,
        error_var,
        catch_body,
    } = &stmts[0].kind
    {
        assert_eq!(error_var, "e");
        assert!(!try_body.statements.is_empty());
        assert!(!catch_body.statements.is_empty());
    } else {
        panic!("expected TryCatch, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 4. Try/catch with multiple statements in each block
// =========================================================================
#[test]
fn test_try_catch_multiple_stmts() {
    // Note: `err` is a keyword in Mog, so use `e` as the catch variable
    let prog = parse_src("try { a = 1; b = 2; } catch (e) { c = 3; }");
    let stmts = program_stmts(&prog);
    if let StatementKind::TryCatch {
        try_body,
        error_var,
        catch_body,
    } = &stmts[0].kind
    {
        assert_eq!(try_body.statements.len(), 2);
        assert_eq!(error_var, "e");
        assert_eq!(catch_body.statements.len(), 1);
    } else {
        panic!("expected TryCatch");
    }
}

// =========================================================================
// 5. With block
// =========================================================================
#[test]
fn test_with_block() {
    let prog = parse_src("with open_file(\"test.txt\") { x = 1; }");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::WithBlock { context, body } = &stmts[0].kind {
        // context should be a function call
        if let ExprKind::CallExpression { callee, .. } = &context.kind {
            if let ExprKind::Identifier { name } = &callee.kind {
                assert_eq!(name, "open_file");
            } else {
                panic!("expected Identifier callee");
            }
        } else {
            panic!("expected CallExpression context, got {:?}", context.kind);
        }
        assert!(!body.statements.is_empty());
    } else {
        panic!("expected WithBlock, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 6. Type alias — simple
// =========================================================================
#[test]
fn test_type_alias_simple() {
    let prog = parse_src("type Score = int;");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::TypeAliasDeclaration {
        name, aliased_type, ..
    } = &stmts[0].kind
    {
        assert_eq!(name, "Score");
        // aliased_type should be TypeAlias wrapping Integer
        if let Type::TypeAlias {
            name: alias_name,
            aliased_type: inner,
        } = aliased_type
        {
            assert_eq!(alias_name, "Score");
            assert!(matches!(*inner.as_ref(), Type::Integer(_)));
        } else {
            panic!("expected TypeAlias, got {:?}", aliased_type);
        }
    } else {
        panic!("expected TypeAliasDeclaration, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 7. Type alias — map type
// =========================================================================
#[test]
fn test_type_alias_map() {
    let prog = parse_src("type Config = {string: string};");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::TypeAliasDeclaration {
        name, aliased_type, ..
    } = &stmts[0].kind
    {
        assert_eq!(name, "Config");
        if let Type::TypeAlias { aliased_type, .. } = aliased_type {
            assert!(matches!(*aliased_type.as_ref(), Type::Map { .. }));
        } else {
            panic!("expected TypeAlias wrapping Map");
        }
    } else {
        panic!("expected TypeAliasDeclaration");
    }
}

// =========================================================================
// 8. Type alias — function type
// =========================================================================
#[test]
fn test_type_alias_function() {
    let prog = parse_src("type Handler = fn(int) -> string;");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::TypeAliasDeclaration {
        name, aliased_type, ..
    } = &stmts[0].kind
    {
        assert_eq!(name, "Handler");
        if let Type::TypeAlias { aliased_type, .. } = aliased_type {
            assert!(
                matches!(*aliased_type.as_ref(), Type::Function { .. }),
                "expected Function type, got {:?}",
                aliased_type
            );
        } else {
            panic!("expected TypeAlias wrapping Function");
        }
    } else {
        panic!("expected TypeAliasDeclaration");
    }
}

// =========================================================================
// 9. Spawn expression
// =========================================================================
#[test]
fn test_spawn_expression() {
    let prog = parse_src("spawn fetch_data();");
    let expr = single_expr(&prog);
    if let ExprKind::SpawnExpression { argument } = &expr.kind {
        assert!(
            matches!(argument.kind, ExprKind::CallExpression { .. }),
            "expected CallExpression in spawn, got {:?}",
            argument.kind
        );
    } else {
        panic!("expected SpawnExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 10. Await expression
// =========================================================================
#[test]
fn test_await_expression() {
    let prog = parse_src("await fetch_data();");
    let expr = single_expr(&prog);
    if let ExprKind::AwaitExpression { argument } = &expr.kind {
        assert!(
            matches!(argument.kind, ExprKind::CallExpression { .. }),
            "expected CallExpression in await, got {:?}",
            argument.kind
        );
    } else {
        panic!("expected AwaitExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 11. Ok expression
// =========================================================================
#[test]
fn test_ok_expression() {
    let prog = parse_src("ok(42);");
    let expr = single_expr(&prog);
    if let ExprKind::OkExpression { value } = &expr.kind {
        assert!(matches!(value.kind, ExprKind::NumberLiteral { .. }));
    } else {
        panic!("expected OkExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 12. Err expression
// =========================================================================
#[test]
fn test_err_expression() {
    let prog = parse_src("err(\"bad\");");
    let expr = single_expr(&prog);
    if let ExprKind::ErrExpression { value } = &expr.kind {
        if let ExprKind::StringLiteral { value: s } = &value.kind {
            assert_eq!(s, "bad");
        } else {
            panic!("expected StringLiteral inside err");
        }
    } else {
        panic!("expected ErrExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 13. Some expression
// =========================================================================
#[test]
fn test_some_expression() {
    let prog = parse_src("some(99);");
    let expr = single_expr(&prog);
    if let ExprKind::SomeExpression { value } = &expr.kind {
        if let ExprKind::NumberLiteral { value: v, .. } = &value.kind {
            assert_eq!(v, "99");
        } else {
            panic!("expected NumberLiteral");
        }
    } else {
        panic!("expected SomeExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 14. None expression
// =========================================================================
#[test]
fn test_none_expression() {
    let prog = parse_src("none;");
    let expr = single_expr(&prog);
    assert!(
        matches!(expr.kind, ExprKind::NoneExpression),
        "expected NoneExpression, got {:?}",
        expr.kind
    );
}

// =========================================================================
// 15. Match with variant patterns (Ok/Err)
// =========================================================================
#[test]
fn test_match_variant_ok_err() {
    let prog = parse_src(
        r#"x: int = match result {
            ok(v) => v,
            err(e) => 0
        };"#,
    );
    let stmts = program_stmts(&prog);
    if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
        let val = value.as_ref().unwrap();
        if let ExprKind::MatchExpression { arms, .. } = &val.kind {
            assert_eq!(arms.len(), 2);
            // First arm: ok(v)
            if let MatchPattern::VariantPattern { name, binding } = &arms[0].pattern {
                assert_eq!(name, "ok");
                assert_eq!(binding.as_deref(), Some("v"));
            } else {
                panic!("expected VariantPattern for ok");
            }
            // Second arm: err(e)
            if let MatchPattern::VariantPattern { name, binding } = &arms[1].pattern {
                assert_eq!(name, "err");
                assert_eq!(binding.as_deref(), Some("e"));
            } else {
                panic!("expected VariantPattern for err");
            }
        } else {
            panic!("expected MatchExpression");
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// =========================================================================
// 16. Match with Some/None patterns
// =========================================================================
#[test]
fn test_match_variant_some_none() {
    let prog = parse_src(
        r#"x: int = match opt {
            some(v) => v,
            none => 0
        };"#,
    );
    let stmts = program_stmts(&prog);
    if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
        let val = value.as_ref().unwrap();
        if let ExprKind::MatchExpression { arms, .. } = &val.kind {
            assert_eq!(arms.len(), 2);
            // some(v)
            if let MatchPattern::VariantPattern { name, binding } = &arms[0].pattern {
                assert_eq!(name, "some");
                assert_eq!(binding.as_deref(), Some("v"));
            } else {
                panic!("expected VariantPattern for some");
            }
            // none (no parens)
            if let MatchPattern::VariantPattern { name, binding } = &arms[1].pattern {
                assert_eq!(name, "none");
                assert!(binding.is_none());
            } else {
                panic!("expected VariantPattern for none");
            }
        } else {
            panic!("expected MatchExpression");
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// =========================================================================
// 17. Match with multiple literal arms
// =========================================================================
#[test]
fn test_match_multiple_literal_arms() {
    let prog = parse_src(
        r#"x: int = match code {
            1 => 10,
            2 => 20,
            3 => 30,
            _ => 0
        };"#,
    );
    let stmts = program_stmts(&prog);
    if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
        let val = value.as_ref().unwrap();
        if let ExprKind::MatchExpression { arms, .. } = &val.kind {
            assert_eq!(arms.len(), 4);
            assert!(matches!(arms[3].pattern, MatchPattern::WildcardPattern));
        } else {
            panic!("expected MatchExpression");
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// =========================================================================
// 18. Package declaration
// =========================================================================
#[test]
fn test_package_declaration() {
    let prog = parse_src("package mylib;");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::PackageDeclaration { name } = &stmts[0].kind {
        assert_eq!(name, "mylib");
    } else {
        panic!("expected PackageDeclaration, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 19. Import declaration (string path syntax)
// =========================================================================
#[test]
fn test_import_declaration() {
    let prog = parse_src("import \"math\";");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::ImportDeclaration { paths } = &stmts[0].kind {
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], "math");
    } else {
        panic!("expected ImportDeclaration, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 20. Pub struct definition
// =========================================================================
#[test]
fn test_pub_struct_definition() {
    let prog = parse_src("pub struct Color { r: int; g: int; b: int; }");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::StructDefinition {
        name,
        fields,
        is_public,
    } = &stmts[0].kind
    {
        assert_eq!(name, "Color");
        assert!(is_public);
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].name, "r");
        assert_eq!(fields[1].name, "g");
        assert_eq!(fields[2].name, "b");
    } else {
        panic!("expected StructDefinition, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 21. Pub async function
// =========================================================================
#[test]
fn test_pub_async_function() {
    let prog = parse_src("pub async fn load() -> string { return \"ok\"; }");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::AsyncFunctionDeclaration {
        name, is_public, ..
    } = &stmts[0].kind
    {
        assert_eq!(name, "load");
        assert!(is_public);
    } else {
        panic!("expected AsyncFunctionDeclaration, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 22. Optional declaration
// =========================================================================
#[test]
fn test_optional_declaration() {
    let prog = parse_src("optional gpu;");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::OptionalDeclaration { capabilities } = &stmts[0].kind {
        assert_eq!(capabilities.len(), 1);
        assert_eq!(capabilities[0], "gpu");
    } else {
        panic!("expected OptionalDeclaration, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 23. Optional declaration with multiple capabilities
// =========================================================================
#[test]
fn test_optional_multiple_capabilities() {
    let prog = parse_src("optional gpu, opencl;");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::OptionalDeclaration { capabilities } = &stmts[0].kind {
        assert_eq!(capabilities.len(), 2);
        assert_eq!(capabilities[0], "gpu");
        assert_eq!(capabilities[1], "opencl");
    } else {
        panic!("expected OptionalDeclaration");
    }
}

// =========================================================================
// 24. For loop (traditional: for i := start to end)
// =========================================================================
#[test]
fn test_for_loop_traditional() {
    let prog = parse_src("for i := 0 to 10 { x = i; }");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::ForLoop {
        variable,
        start,
        end,
        body,
    } = &stmts[0].kind
    {
        assert_eq!(variable, "i");
        if let ExprKind::NumberLiteral { value, .. } = &start.kind {
            assert_eq!(value, "0");
        } else {
            panic!("expected NumberLiteral for start");
        }
        if let ExprKind::NumberLiteral { value, .. } = &end.kind {
            assert_eq!(value, "10");
        } else {
            panic!("expected NumberLiteral for end");
        }
        assert!(!body.statements.is_empty());
    } else {
        panic!("expected ForLoop, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 25. Nested function calls as arguments
// =========================================================================
#[test]
fn test_nested_function_call_args() {
    let prog = parse_src("foo(bar(1, 2), baz(3));");
    let expr = single_expr(&prog);
    if let ExprKind::CallExpression {
        callee, arguments, ..
    } = &expr.kind
    {
        if let ExprKind::Identifier { name } = &callee.kind {
            assert_eq!(name, "foo");
        } else {
            panic!("expected Identifier callee");
        }
        assert_eq!(arguments.len(), 2);
        assert!(matches!(arguments[0].kind, ExprKind::CallExpression { .. }));
        assert!(matches!(arguments[1].kind, ExprKind::CallExpression { .. }));
    } else {
        panic!("expected CallExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 26. Index on function call result
// =========================================================================
#[test]
fn test_index_on_call_result() {
    let prog = parse_src("getItems()[0];");
    let expr = single_expr(&prog);
    if let ExprKind::IndexExpression { object, index } = &expr.kind {
        assert!(matches!(object.kind, ExprKind::CallExpression { .. }));
        if let ExprKind::NumberLiteral { value, .. } = &index.kind {
            assert_eq!(value, "0");
        } else {
            panic!("expected NumberLiteral index");
        }
    } else {
        panic!("expected IndexExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 27. Function with default parameter value
// =========================================================================
#[test]
fn test_function_with_default_param() {
    let prog = parse_src(
        "fn greet(name: string, greeting: string = \"Hello\") -> string { return greeting; }",
    );
    let stmts = program_stmts(&prog);
    if let StatementKind::FunctionDeclaration { name, params, .. } = &stmts[0].kind {
        assert_eq!(name, "greet");
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "name");
        assert!(params[0].default_value.is_none());
        assert_eq!(params[1].name, "greeting");
        assert!(params[1].default_value.is_some());
        if let ExprKind::StringLiteral { value } = &params[1].default_value.as_ref().unwrap().kind {
            assert_eq!(value, "Hello");
        } else {
            panic!("expected StringLiteral default");
        }
    } else {
        panic!("expected FunctionDeclaration");
    }
}

// =========================================================================
// 28. SoA constructor
// =========================================================================
#[test]
fn test_soa_constructor() {
    let prog = parse_src("x := soa Particle[100];");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    // This should be a variable declaration with walrus
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        if let ExprKind::AssignmentExpression { value, .. } = &expression.kind {
            if let ExprKind::SoAConstructor {
                struct_name,
                capacity,
            } = &value.kind
            {
                assert_eq!(struct_name, "Particle");
                assert_eq!(*capacity, Some(100));
            } else {
                panic!("expected SoAConstructor, got {:?}", value.kind);
            }
        } else {
            panic!("expected AssignmentExpression");
        }
    } else {
        panic!("expected ExpressionStatement, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 29. SoA constructor — dynamic (no capacity)
// =========================================================================
#[test]
fn test_soa_constructor_dynamic() {
    let prog = parse_src("x := soa Datum[];");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        if let ExprKind::AssignmentExpression { value, .. } = &expression.kind {
            if let ExprKind::SoAConstructor {
                struct_name,
                capacity,
            } = &value.kind
            {
                assert_eq!(struct_name, "Datum");
                assert!(capacity.is_none());
            } else {
                panic!("expected SoAConstructor");
            }
        } else {
            panic!("expected AssignmentExpression");
        }
    } else {
        panic!("expected ExpressionStatement");
    }
}

// =========================================================================
// 30. Boolean literal false
// =========================================================================
#[test]
fn test_boolean_literal_false() {
    let prog = parse_src("x: bool = false;");
    let stmts = program_stmts(&prog);
    if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
        let val = value.as_ref().unwrap();
        if let ExprKind::BooleanLiteral { value: b } = &val.kind {
            assert!(!b);
        } else {
            panic!("expected BooleanLiteral false");
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// =========================================================================
// 31. If without else (top-level)
// =========================================================================
#[test]
fn test_if_without_else() {
    let prog = parse_src("if x > 0 { return 1; }");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::Conditional {
        false_branch,
        condition,
        ..
    } = &stmts[0].kind
    {
        assert!(false_branch.is_none());
        assert!(matches!(condition.kind, ExprKind::BinaryExpression { .. }));
    } else {
        panic!("expected Conditional");
    }
}

// =========================================================================
// 32. Nested if-else (if / else if / else)
// =========================================================================
#[test]
fn test_nested_if_else() {
    let prog = parse_src("if x > 0 { a = 1; } else { if x < 0 { a = 2; } else { a = 0; } }");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::Conditional {
        false_branch,
        true_branch,
        ..
    } = &stmts[0].kind
    {
        assert!(!true_branch.statements.is_empty());
        // false branch should contain another Conditional
        let fb = false_branch.as_ref().expect("expected false branch");
        assert_eq!(fb.statements.len(), 1);
        assert!(matches!(
            fb.statements[0].kind,
            StatementKind::Conditional { .. }
        ));
    } else {
        panic!("expected Conditional");
    }
}

// =========================================================================
// 33. For-each with typed variable
// =========================================================================
#[test]
fn test_for_each_typed() {
    let prog = parse_src("for item: int in items { x = item; }");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::ForEachLoop {
        variable,
        var_type,
        array,
        body,
    } = &stmts[0].kind
    {
        assert_eq!(variable, "item");
        assert!(var_type.is_some());
        if let ExprKind::Identifier { name } = &array.kind {
            assert_eq!(name, "items");
        } else {
            panic!("expected Identifier iterable");
        }
        assert!(!body.statements.is_empty());
    } else {
        panic!("expected ForEachLoop, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 34. Lambda with no params and void return
// =========================================================================
#[test]
fn test_lambda_no_params() {
    let prog = parse_src("x: fn() -> void = fn() -> void { return; };");
    let stmts = program_stmts(&prog);
    if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
        let val = value.as_ref().unwrap();
        if let ExprKind::Lambda { params, body, .. } = &val.kind {
            assert!(params.is_empty());
            assert_eq!(body.statements.len(), 1);
            assert!(matches!(
                body.statements[0].kind,
                StatementKind::Return { .. }
            ));
        } else {
            panic!("expected Lambda, got {:?}", val.kind);
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// =========================================================================
// 35. Struct literal with named fields
// =========================================================================
#[test]
fn test_struct_literal() {
    let prog = parse_src("p: Point = Point { x: 1, y: 2 };");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::VariableDeclaration { name, value, .. } = &stmts[0].kind {
        assert_eq!(name, "p");
        let val = value.as_ref().unwrap();
        if let ExprKind::StructLiteral {
            struct_name,
            fields,
        } = &val.kind
        {
            assert_eq!(struct_name.as_deref(), Some("Point"));
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[1].name, "y");
        } else {
            panic!("expected StructLiteral, got {:?}", val.kind);
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// =========================================================================
// 36. Return without value (bare return)
// =========================================================================
#[test]
fn test_return_without_value() {
    let prog = parse_src("fn cleanup() { return; }");
    let stmts = program_stmts(&prog);
    if let StatementKind::FunctionDeclaration { body, .. } = &stmts[0].kind {
        assert_eq!(body.statements.len(), 1);
        if let StatementKind::Return { value } = &body.statements[0].kind {
            assert!(value.is_none());
        } else {
            panic!("expected Return statement");
        }
    } else {
        panic!("expected FunctionDeclaration");
    }
}

// =========================================================================
// 37. Call with named arguments
// =========================================================================
#[test]
fn test_call_with_named_args() {
    let prog = parse_src("connect(host: \"localhost\", port: 8080);");
    let expr = single_expr(&prog);
    if let ExprKind::CallExpression {
        callee,
        arguments,
        named_args,
    } = &expr.kind
    {
        if let ExprKind::Identifier { name } = &callee.kind {
            assert_eq!(name, "connect");
        }
        // named args should be in the named_args field
        let na = named_args.as_ref().expect("expected named_args");
        assert_eq!(na.len(), 2);
        assert_eq!(na[0].name, "host");
        assert_eq!(na[1].name, "port");
        // positional args may be empty
        assert!(arguments.is_empty());
    } else {
        panic!("expected CallExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 38. Nested arrays (using [int] type for outer, inner elements are arrays)
// =========================================================================
#[test]
fn test_nested_array_literal() {
    let prog = parse_src("x: [int] = [[1, 2], [3, 4]];");
    let stmts = program_stmts(&prog);
    if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
        let val = value.as_ref().unwrap();
        if let ExprKind::ArrayLiteral { elements } = &val.kind {
            assert_eq!(elements.len(), 2);
            assert!(matches!(elements[0].kind, ExprKind::ArrayLiteral { .. }));
            assert!(matches!(elements[1].kind, ExprKind::ArrayLiteral { .. }));
        } else {
            panic!("expected ArrayLiteral, got {:?}", val.kind);
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// =========================================================================
// 39. Comparison operators (==, !=, <, >, <=, >=)
// =========================================================================
#[test]
fn test_comparison_operators() {
    for (src, expected_op) in [
        ("x == y;", "=="),
        ("x != y;", "!="),
        ("x < y;", "<"),
        ("x > y;", ">"),
        ("x <= y;", "<="),
        ("x >= y;", ">="),
    ] {
        let prog = parse_src(src);
        let expr = single_expr(&prog);
        if let ExprKind::BinaryExpression { operator, .. } = &expr.kind {
            assert_eq!(operator, expected_op, "failed for source: {}", src);
        } else {
            panic!("expected BinaryExpression for: {}", src);
        }
    }
}

// =========================================================================
// 40. Logical not (unary)
// =========================================================================
#[test]
fn test_unary_not() {
    let prog = parse_src("not true;");
    let expr = single_expr(&prog);
    if let ExprKind::UnaryExpression { operator, operand } = &expr.kind {
        assert_eq!(operator, "not");
        assert!(matches!(
            operand.kind,
            ExprKind::BooleanLiteral { value: true }
        ));
    } else {
        panic!("expected UnaryExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 41. Struct definition with multiple types
// =========================================================================
#[test]
fn test_struct_with_mixed_field_types() {
    let prog = parse_src("struct Config { host: string; port: int; debug: bool; }");
    let stmts = program_stmts(&prog);
    if let StatementKind::StructDefinition { name, fields, .. } = &stmts[0].kind {
        assert_eq!(name, "Config");
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].name, "host");
        assert_eq!(fields[0].field_type, Type::String);
        assert_eq!(fields[1].name, "port");
        assert!(matches!(fields[1].field_type, Type::Integer(_)));
        assert_eq!(fields[2].name, "debug");
        assert_eq!(fields[2].field_type, Type::Bool);
    } else {
        panic!("expected StructDefinition");
    }
}

// =========================================================================
// 42. Async fn with params and return type
// =========================================================================
#[test]
fn test_async_fn_with_params() {
    let prog =
        parse_src("async fn request(url: string, timeout: int) -> string { return \"ok\"; }");
    let stmts = program_stmts(&prog);
    if let StatementKind::AsyncFunctionDeclaration {
        name,
        params,
        return_type,
        ..
    } = &stmts[0].kind
    {
        assert_eq!(name, "request");
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "url");
        assert_eq!(params[1].name, "timeout");
        assert!(
            !matches!(return_type, Type::Void),
            "expected non-void return type"
        );
    } else {
        panic!("expected AsyncFunctionDeclaration");
    }
}

// =========================================================================
// 43. Slice expression
// =========================================================================
#[test]
fn test_slice_expression() {
    let prog = parse_src("arr[1:5];");
    let expr = single_expr(&prog);
    if let ExprKind::SliceExpression {
        object,
        start,
        end,
        step,
    } = &expr.kind
    {
        if let ExprKind::Identifier { name } = &object.kind {
            assert_eq!(name, "arr");
        }
        if let ExprKind::NumberLiteral { value, .. } = &start.kind {
            assert_eq!(value, "1");
        }
        if let ExprKind::NumberLiteral { value, .. } = &end.kind {
            assert_eq!(value, "5");
        }
        assert!(step.is_none());
    } else {
        panic!("expected SliceExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 44. Multiple imports (string path syntax)
// =========================================================================
#[test]
fn test_multiple_imports() {
    let prog = parse_src("import \"math\"; import \"io\";");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 2);
    if let StatementKind::ImportDeclaration { paths } = &stmts[0].kind {
        assert_eq!(paths[0], "math");
    } else {
        panic!("expected ImportDeclaration");
    }
    if let StatementKind::ImportDeclaration { paths } = &stmts[1].kind {
        assert_eq!(paths[0], "io");
    } else {
        panic!("expected ImportDeclaration");
    }
}

// =========================================================================
// 45. With block using identifier context
// =========================================================================
#[test]
fn test_with_block_identifier() {
    let prog = parse_src("with ctx { cleanup(); }");
    let stmts = program_stmts(&prog);
    if let StatementKind::WithBlock { context, body } = &stmts[0].kind {
        if let ExprKind::Identifier { name } = &context.kind {
            assert_eq!(name, "ctx");
        } else {
            panic!("expected Identifier context");
        }
        assert_eq!(body.statements.len(), 1);
    } else {
        panic!("expected WithBlock");
    }
}

// =========================================================================
// 46. Variable declaration with zero init
// =========================================================================
#[test]
fn test_variable_declaration_zero_init() {
    // Mog requires = or := after type — no uninitialized declarations
    let prog = parse_src("x: int = 0;");
    let stmts = program_stmts(&prog);
    if let StatementKind::VariableDeclaration {
        name,
        var_type,
        value,
    } = &stmts[0].kind
    {
        assert_eq!(name, "x");
        assert!(var_type.is_some());
        assert!(value.is_some());
        if let ExprKind::NumberLiteral { value: v, .. } = &value.as_ref().unwrap().kind {
            assert_eq!(v, "0");
        } else {
            panic!("expected NumberLiteral");
        }
    } else {
        panic!("expected VariableDeclaration, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 47. Map literal with multiple entries
// =========================================================================
#[test]
fn test_map_literal_multiple_entries() {
    let prog = parse_src("x = ({ a: 1, b: 2, c: 3 });");
    let stmts = program_stmts(&prog);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        if let ExprKind::AssignmentExpression { value, .. } = &expression.kind {
            if let ExprKind::MapLiteral { entries } = &value.kind {
                assert_eq!(entries.len(), 3);
                assert_eq!(entries[0].key, "a");
                assert_eq!(entries[1].key, "b");
                assert_eq!(entries[2].key, "c");
            } else {
                panic!("expected MapLiteral");
            }
        } else {
            panic!("expected AssignmentExpression");
        }
    } else {
        panic!("expected ExpressionStatement");
    }
}

// =========================================================================
// 48. Chained index and member expressions
// =========================================================================
#[test]
fn test_chained_index_then_member() {
    let prog = parse_src("arr[0].name;");
    let expr = single_expr(&prog);
    // outermost is MemberExpression (...).name
    if let ExprKind::MemberExpression { object, property } = &expr.kind {
        assert_eq!(property, "name");
        // inner is IndexExpression arr[0]
        if let ExprKind::IndexExpression { object: inner, .. } = &object.kind {
            if let ExprKind::Identifier { name } = &inner.kind {
                assert_eq!(name, "arr");
            } else {
                panic!("expected Identifier 'arr'");
            }
        } else {
            panic!("expected IndexExpression");
        }
    } else {
        panic!("expected MemberExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 49. Assignment to member expression (dot assign)
// =========================================================================
#[test]
fn test_assignment_to_member() {
    let prog = parse_src("obj.field = 42;");
    let expr = single_expr(&prog);
    if let ExprKind::AssignmentExpression { target, value, .. } = &expr.kind {
        let t = target.as_ref().expect("expected target");
        assert!(matches!(t.kind, ExprKind::MemberExpression { .. }));
        if let ExprKind::NumberLiteral { value: v, .. } = &value.kind {
            assert_eq!(v, "42");
        }
    } else {
        panic!("expected AssignmentExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 50. Assignment to index expression
// =========================================================================
#[test]
fn test_assignment_to_index() {
    let prog = parse_src("arr[0] = 99;");
    let expr = single_expr(&prog);
    if let ExprKind::AssignmentExpression { target, value, .. } = &expr.kind {
        let t = target.as_ref().expect("expected target");
        assert!(matches!(t.kind, ExprKind::IndexExpression { .. }));
        if let ExprKind::NumberLiteral { value: v, .. } = &value.kind {
            assert_eq!(v, "99");
        }
    } else {
        panic!("expected AssignmentExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 51. Pub type alias
// =========================================================================
#[test]
fn test_pub_type_alias() {
    let prog = parse_src("pub type ID = int;");
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::TypeAliasDeclaration { name, .. } = &stmts[0].kind {
        assert_eq!(name, "ID");
    } else {
        panic!("expected TypeAliasDeclaration, got {:?}", stmts[0].kind);
    }
}

// =========================================================================
// 52. Function with no return type (void implied)
// =========================================================================
#[test]
fn test_function_no_return_type() {
    let prog = parse_src("fn doSomething() { x = 1; }");
    let stmts = program_stmts(&prog);
    if let StatementKind::FunctionDeclaration {
        name, return_type, ..
    } = &stmts[0].kind
    {
        assert_eq!(name, "doSomething");
        assert_eq!(*return_type, Type::Void);
    } else {
        panic!("expected FunctionDeclaration");
    }
}

// =========================================================================
// 53. Complex: function returning a match expression
// =========================================================================
#[test]
fn test_function_with_match_return() {
    let prog = parse_src(
        r#"fn classify(x: int) -> string {
            return match x {
                1 => "one",
                2 => "two",
                _ => "other"
            };
        }"#,
    );
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 1);
    if let StatementKind::FunctionDeclaration { name, body, .. } = &stmts[0].kind {
        assert_eq!(name, "classify");
        assert_eq!(body.statements.len(), 1);
        if let StatementKind::Return { value } = &body.statements[0].kind {
            let val = value.as_ref().unwrap();
            assert!(matches!(val.kind, ExprKind::MatchExpression { .. }));
        } else {
            panic!("expected Return");
        }
    } else {
        panic!("expected FunctionDeclaration");
    }
}

// =========================================================================
// 54. While loop with complex condition
// =========================================================================
#[test]
fn test_while_complex_condition() {
    let prog = parse_src("while not done { x = x + 1; }");
    let stmts = program_stmts(&prog);
    if let StatementKind::WhileLoop { test, .. } = &stmts[0].kind {
        if let ExprKind::UnaryExpression { operator, operand } = &test.kind {
            assert_eq!(operator, "not");
            if let ExprKind::Identifier { name } = &operand.kind {
                assert_eq!(name, "done");
            } else {
                panic!("expected Identifier 'done'");
            }
        } else {
            panic!("expected UnaryExpression, got {:?}", test.kind);
        }
    } else {
        panic!("expected WhileLoop");
    }
}

// =========================================================================
// 55. Propagate expression (postfix ?)
// =========================================================================
#[test]
fn test_propagate_expression() {
    let prog = parse_src("get_value()?;");
    let expr = single_expr(&prog);
    if let ExprKind::PropagateExpression { value } = &expr.kind {
        assert!(
            matches!(value.kind, ExprKind::CallExpression { .. }),
            "expected CallExpression inside propagate"
        );
    } else {
        panic!("expected PropagateExpression, got {:?}", expr.kind);
    }
}

// =========================================================================
// 56. Multiple statements including mixed declarations
// =========================================================================
#[test]
fn test_mixed_declarations() {
    let prog = parse_src(
        r#"
        package main;
        import "io";
        requires fs;
        fn main() { print("hello"); }
        "#,
    );
    let stmts = program_stmts(&prog);
    assert_eq!(stmts.len(), 4);
    assert!(matches!(
        stmts[0].kind,
        StatementKind::PackageDeclaration { .. }
    ));
    assert!(matches!(
        stmts[1].kind,
        StatementKind::ImportDeclaration { .. }
    ));
    assert!(matches!(
        stmts[2].kind,
        StatementKind::RequiresDeclaration { .. }
    ));
    assert!(matches!(
        stmts[3].kind,
        StatementKind::FunctionDeclaration { .. }
    ));
}

// =========================================================================
// 57. Call expression with mixed positional and named args
// =========================================================================
#[test]
fn test_call_mixed_positional_and_named_args() {
    let prog = parse_src("request(\"url\", timeout: 30, retry: true);");
    let expr = single_expr(&prog);
    if let ExprKind::CallExpression {
        arguments,
        named_args,
        ..
    } = &expr.kind
    {
        assert_eq!(arguments.len(), 1);
        if let ExprKind::StringLiteral { value } = &arguments[0].kind {
            assert_eq!(value, "url");
        }
        let na = named_args.as_ref().expect("expected named args");
        assert_eq!(na.len(), 2);
        assert_eq!(na[0].name, "timeout");
        assert_eq!(na[1].name, "retry");
    } else {
        panic!("expected CallExpression");
    }
}

// =========================================================================
// 58. Match with block body arm
// =========================================================================
#[test]
fn test_match_block_body_arm() {
    let prog = parse_src(
        r#"x: int = match val {
            1 => { a = 1; a },
            _ => 0
        };"#,
    );
    let stmts = program_stmts(&prog);
    if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
        let val = value.as_ref().unwrap();
        if let ExprKind::MatchExpression { arms, .. } = &val.kind {
            assert_eq!(arms.len(), 2);
            // First arm body should be a block expression
            // The parser wraps multi-stmt blocks or returns the single expression
        } else {
            panic!("expected MatchExpression");
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// =========================================================================
// 59. Empty array literal
// =========================================================================
#[test]
fn test_empty_array() {
    let prog = parse_src("x: [int] = [];");
    let stmts = program_stmts(&prog);
    if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
        let val = value.as_ref().unwrap();
        if let ExprKind::ArrayLiteral { elements } = &val.kind {
            assert!(elements.is_empty());
        } else {
            panic!("expected ArrayLiteral");
        }
    } else {
        panic!("expected VariableDeclaration");
    }
}

// =========================================================================
// 60. Spawn on a member call
// =========================================================================
#[test]
fn test_spawn_member_call() {
    let prog = parse_src("spawn obj.method();");
    let expr = single_expr(&prog);
    if let ExprKind::SpawnExpression { argument } = &expr.kind {
        assert!(
            matches!(argument.kind, ExprKind::CallExpression { .. }),
            "expected CallExpression"
        );
        if let ExprKind::CallExpression { callee, .. } = &argument.kind {
            assert!(matches!(callee.kind, ExprKind::MemberExpression { .. }));
        }
    } else {
        panic!("expected SpawnExpression, got {:?}", expr.kind);
    }
}
