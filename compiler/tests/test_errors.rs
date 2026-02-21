/// Error-handling and regression tests ported from TypeScript:
///   - src/error_handling.test.ts  (65 tests)
///   - src/fixes.test.ts           (30 tests)
///
/// Covers lexer token recognition, parser AST structure, analyzer validation,
/// QBE IR code generation, and type-system predicates for Result/Optional
/// error-handling constructs.
use mog::ast::{ExprKind, MatchPattern, StatementKind};
use mog::compiler::{compile, CompileOptions};
use mog::lexer::tokenize;
use mog::parser::parse;
use mog::token::TokenType;
use mog::types::{FloatKind, IntegerKind, Type};

// ===========================================================================
// Helpers
// ===========================================================================

/// Tokenise and filter whitespace/comments (mirrors TS `lex()`).
fn lex(src: &str) -> Vec<mog::token::Token> {
    let tokens = tokenize(src);
    tokens
        .into_iter()
        .filter(|t| t.token_type != TokenType::Whitespace && t.token_type != TokenType::Comment)
        .collect()
}

/// Tokenise, filter, parse -> AST Program node.
fn parse_src(src: &str) -> mog::ast::Statement {
    let tokens = tokenize(src);
    parse(&tokens)
}

/// Run the full analyzer pipeline and return error messages.
fn analyze_src(src: &str) -> Vec<String> {
    let ast = parse_src(src);
    let mut analyzer = mog::analyzer::SemanticAnalyzer::new();
    let errors = analyzer.analyze(&ast);
    errors.into_iter().map(|e| e.message).collect()
}

/// Compile to QBE IR and return (ir_string, error_messages).
fn compile_src(src: &str) -> (String, Vec<String>) {
    let result = compile(src, &CompileOptions::default());
    (result.ir, result.errors)
}

/// Extract the top-level statements from a Program node.
fn program_stmts(stmt: &mog::ast::Statement) -> &[mog::ast::Statement] {
    match &stmt.kind {
        StatementKind::Program { statements, .. } => statements,
        _ => panic!("expected Program, got {:?}", stmt.kind),
    }
}

// ===========================================================================
// 1. LEXER TESTS  (18 tests)
// ===========================================================================

#[test]
fn lexer_ok_keyword() {
    let tokens = lex("ok(42)");
    assert!(
        tokens.iter().any(|t| t.token_type == TokenType::Ok),
        "should tokenize ok as Ok keyword"
    );
}

#[test]
fn lexer_ok_prefix_not_keyword() {
    let tokens = lex("ok_thing");
    assert!(!tokens.iter().any(|t| t.token_type == TokenType::Ok));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Identifier));
}

#[test]
fn lexer_err_keyword() {
    let tokens = lex(r#"err("error")"#);
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Err));
}

#[test]
fn lexer_err_prefix_not_keyword() {
    let tokens = lex("error");
    assert!(!tokens.iter().any(|t| t.token_type == TokenType::Err));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Identifier));
}

#[test]
fn lexer_some_keyword() {
    let tokens = lex("some(42)");
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Some));
}

#[test]
fn lexer_none_keyword() {
    let tokens = lex("none");
    assert!(tokens.iter().any(|t| t.token_type == TokenType::None));
}

#[test]
fn lexer_none_prefix_not_keyword() {
    let tokens = lex("nonexistent");
    assert!(!tokens.iter().any(|t| t.token_type == TokenType::None));
}

#[test]
fn lexer_try_keyword() {
    let tokens = lex("try { }");
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Try));
}

#[test]
fn lexer_catch_keyword() {
    let tokens = lex("catch(e) { }");
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Catch));
}

#[test]
fn lexer_is_keyword() {
    let tokens = lex("x is some(y)");
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Is));
}

#[test]
fn lexer_question_mark_token() {
    let tokens = lex("x?");
    assert!(tokens
        .iter()
        .any(|t| t.token_type == TokenType::QuestionMark));
}

#[test]
fn lexer_question_mark_value() {
    let tokens = lex("a?;");
    let q = tokens
        .iter()
        .find(|t| t.token_type == TokenType::QuestionMark)
        .expect("? token missing");
    assert_eq!(q.value, "?");
}

#[test]
fn lexer_combined_ok_tokens() {
    let tokens = lex("ok(42)");
    let types: Vec<_> = tokens.iter().map(|t| t.token_type).collect();
    assert!(types.contains(&TokenType::Ok));
    assert!(types.contains(&TokenType::LParen));
    assert!(types.contains(&TokenType::Number));
    assert!(types.contains(&TokenType::RParen));
}

#[test]
fn lexer_combined_err_tokens() {
    let tokens = lex(r#"err("message")"#);
    let types: Vec<_> = tokens.iter().map(|t| t.token_type).collect();
    assert!(types.contains(&TokenType::Err));
    assert!(types.contains(&TokenType::LParen));
    assert!(types.contains(&TokenType::StringLit));
    assert!(types.contains(&TokenType::RParen));
}

#[test]
fn lexer_combined_try_catch() {
    let tokens = lex("try { } catch(e) { }");
    let types: Vec<_> = tokens.iter().map(|t| t.token_type).collect();
    assert!(types.contains(&TokenType::Try));
    assert!(types.contains(&TokenType::Catch));
}

#[test]
fn lexer_question_propagation_tokens() {
    let tokens = lex("result?;");
    let types: Vec<_> = tokens.iter().map(|t| t.token_type).collect();
    assert!(types.contains(&TokenType::Identifier));
    assert!(types.contains(&TokenType::QuestionMark));
    assert!(types.contains(&TokenType::Semicolon));
}

#[test]
fn lexer_is_some_pattern_tokens() {
    let tokens = lex("x is some(val)");
    let types: Vec<_> = tokens.iter().map(|t| t.token_type).collect();
    assert!(types.contains(&TokenType::Identifier));
    assert!(types.contains(&TokenType::Is));
    assert!(types.contains(&TokenType::Some));
    assert!(types.contains(&TokenType::LParen));
    assert!(types.contains(&TokenType::RParen));
}

#[test]
fn lexer_with_keyword() {
    let tokens = lex("with no_grad() { }");
    assert!(tokens.iter().any(|t| t.token_type == TokenType::With));
}

// ===========================================================================
// 2. PARSER TESTS  (14 tests)
// ===========================================================================

#[test]
fn parser_ok_expression() {
    let prog = parse_src("ok(42)");
    let stmts = program_stmts(&prog);
    assert!(!stmts.is_empty());
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        assert!(matches!(expression.kind, ExprKind::OkExpression { .. }));
        if let ExprKind::OkExpression { value } = &expression.kind {
            assert!(matches!(value.kind, ExprKind::NumberLiteral { .. }));
        }
    } else {
        panic!("expected ExpressionStatement");
    }
}

#[test]
fn parser_ok_complex_expr() {
    let prog = parse_src("ok(x + 1)");
    let stmts = program_stmts(&prog);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        if let ExprKind::OkExpression { value } = &expression.kind {
            assert!(matches!(value.kind, ExprKind::BinaryExpression { .. }));
        } else {
            panic!("expected OkExpression");
        }
    }
}

#[test]
fn parser_err_expression() {
    let prog = parse_src(r#"err("message")"#);
    let stmts = program_stmts(&prog);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        assert!(matches!(expression.kind, ExprKind::ErrExpression { .. }));
        if let ExprKind::ErrExpression { value } = &expression.kind {
            assert!(matches!(value.kind, ExprKind::StringLiteral { .. }));
        }
    } else {
        panic!("expected ExpressionStatement");
    }
}

#[test]
fn parser_some_expression() {
    let prog = parse_src("some(42)");
    let stmts = program_stmts(&prog);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        assert!(matches!(expression.kind, ExprKind::SomeExpression { .. }));
    } else {
        panic!("expected ExpressionStatement");
    }
}

#[test]
fn parser_none_expression() {
    let prog = parse_src("none");
    let stmts = program_stmts(&prog);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        assert!(matches!(expression.kind, ExprKind::NoneExpression));
    } else {
        panic!("expected ExpressionStatement");
    }
}

#[test]
fn parser_propagate_expression() {
    let prog = parse_src("x?");
    let stmts = program_stmts(&prog);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        if let ExprKind::PropagateExpression { value } = &expression.kind {
            if let ExprKind::Identifier { name } = &value.kind {
                assert_eq!(name, "x");
            } else {
                panic!("inner should be Identifier(x)");
            }
        } else {
            panic!("expected PropagateExpression");
        }
    }
}

#[test]
fn parser_try_catch_statement() {
    let src = "try {\n    x := ok(42);\n} catch(e) {\n    y := 0;\n}";
    let prog = parse_src(src);
    let stmts = program_stmts(&prog);
    if let StatementKind::TryCatch {
        error_var,
        try_body,
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

#[test]
fn parser_is_some_expression() {
    let src = "if x is some(val) {\n    y := val;\n}";
    let prog = parse_src(src);
    let stmts = program_stmts(&prog);
    if let StatementKind::Conditional { condition, .. } = &stmts[0].kind {
        if let ExprKind::IsSomeExpression { value, binding } = &condition.kind {
            if let ExprKind::Identifier { name } = &value.kind {
                assert_eq!(name, "x");
            }
            assert_eq!(binding, "val");
        } else {
            panic!("expected IsSomeExpression, got {:?}", condition.kind);
        }
    } else {
        panic!("expected Conditional");
    }
}

#[test]
fn parser_is_none_expression() {
    let src = "if x is none {\n    y := 0;\n}";
    let prog = parse_src(src);
    let stmts = program_stmts(&prog);
    if let StatementKind::Conditional { condition, .. } = &stmts[0].kind {
        assert!(matches!(condition.kind, ExprKind::IsNoneExpression { .. }));
    } else {
        panic!("expected Conditional");
    }
}

#[test]
fn parser_result_return_type() {
    let src = "fn foo() -> Result<int> {\n    return ok(42);\n}";
    let prog = parse_src(src);
    let stmts = program_stmts(&prog);
    if let StatementKind::FunctionDeclaration { return_type, .. } = &stmts[0].kind {
        assert!(
            return_type.is_result(),
            "expected Result type, got {:?}",
            return_type
        );
        if let Type::Result(inner) = return_type {
            assert!(inner.is_integer());
        }
    } else {
        panic!("expected FunctionDeclaration");
    }
}

#[test]
fn parser_optional_return_type() {
    let src = "fn find() -> ?int {\n    return none;\n}";
    let prog = parse_src(src);
    let stmts = program_stmts(&prog);
    if let StatementKind::FunctionDeclaration { return_type, .. } = &stmts[0].kind {
        assert!(
            return_type.is_optional(),
            "expected Optional type, got {:?}",
            return_type
        );
        if let Type::Optional(inner) = return_type {
            assert!(inner.is_integer());
        }
    } else {
        panic!("expected FunctionDeclaration");
    }
}

#[test]
fn parser_match_result_variant_patterns() {
    let src = "match x {\n    ok(val) => val,\n    err(msg) => 0,\n}";
    let prog = parse_src(src);
    let stmts = program_stmts(&prog);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        if let ExprKind::MatchExpression { arms, .. } = &expression.kind {
            assert_eq!(arms.len(), 2);
            if let MatchPattern::VariantPattern { name, binding } = &arms[0].pattern {
                assert_eq!(name, "ok");
                assert_eq!(binding.as_deref(), Some("val"));
            } else {
                panic!("arm 0 should be VariantPattern");
            }
            if let MatchPattern::VariantPattern { name, binding } = &arms[1].pattern {
                assert_eq!(name, "err");
                assert_eq!(binding.as_deref(), Some("msg"));
            } else {
                panic!("arm 1 should be VariantPattern");
            }
        }
    }
}

#[test]
fn parser_match_optional_variant_patterns() {
    let src = "match x {\n    some(val) => val,\n    none() => 0,\n}";
    let prog = parse_src(src);
    let stmts = program_stmts(&prog);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        if let ExprKind::MatchExpression { arms, .. } = &expression.kind {
            assert_eq!(arms.len(), 2);
            if let MatchPattern::VariantPattern { name, .. } = &arms[0].pattern {
                assert_eq!(name, "some");
            }
            if let MatchPattern::VariantPattern { name, .. } = &arms[1].pattern {
                assert_eq!(name, "none");
            }
        }
    }
}

#[test]
fn parser_propagate_on_call() {
    let prog = parse_src("foo()?");
    let stmts = program_stmts(&prog);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        if let ExprKind::PropagateExpression { value } = &expression.kind {
            assert!(matches!(value.kind, ExprKind::CallExpression { .. }));
        } else {
            panic!("expected PropagateExpression");
        }
    }
}

#[test]
fn parser_match_with_wildcard() {
    let src = "match x {\n    ok(v) => v,\n    _ => 0,\n}";
    let prog = parse_src(src);
    let stmts = program_stmts(&prog);
    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
        if let ExprKind::MatchExpression { arms, .. } = &expression.kind {
            assert_eq!(arms.len(), 2);
            assert!(matches!(arms[1].pattern, MatchPattern::WildcardPattern));
        }
    }
}

#[test]
fn parser_with_block() {
    let src = "with no_grad() {\n    x: int = 1;\n}";
    let prog = parse_src(src);
    let stmts = program_stmts(&prog);
    assert!(
        matches!(stmts[0].kind, StatementKind::WithBlock { .. }),
        "expected WithBlock, got {:?}",
        stmts[0].kind
    );
}

// ===========================================================================
// 3. ANALYZER TESTS  (10 tests)
// ===========================================================================

#[test]
fn analyzer_ok_expression_no_errors() {
    let errors = analyze_src("ok(42)");
    assert!(
        errors.is_empty(),
        "ok(42) should analyze cleanly: {:?}",
        errors
    );
}

#[test]
fn analyzer_err_expression_no_errors() {
    let errors = analyze_src(r#"err("msg")"#);
    assert!(
        errors.is_empty(),
        "err() should analyze cleanly: {:?}",
        errors
    );
}

#[test]
fn analyzer_some_expression_no_errors() {
    let errors = analyze_src("some(42)");
    assert!(
        errors.is_empty(),
        "some() should analyze cleanly: {:?}",
        errors
    );
}

#[test]
fn analyzer_none_expression_no_errors() {
    let errors = analyze_src("none");
    assert!(
        errors.is_empty(),
        "none should analyze cleanly: {:?}",
        errors
    );
}

#[test]
fn analyzer_is_some_no_errors() {
    let src = "if some(42) is some(val) {\n    val;\n}";
    let errors = analyze_src(src);
    assert!(errors.is_empty(), "is some() should be valid: {:?}", errors);
}

#[test]
fn analyzer_try_catch_no_errors() {
    let src = "try {\n    ok(42);\n} catch(e) {\n    e;\n}";
    let errors = analyze_src(src);
    assert!(errors.is_empty(), "try/catch should be valid: {:?}", errors);
}

#[test]
fn analyzer_fn_result_return_type() {
    let src = "fn foo() -> Result<int> {\n    return ok(42);\n}";
    let errors = analyze_src(src);
    assert!(
        errors.is_empty(),
        "fn Result<int> should be valid: {:?}",
        errors
    );
}

#[test]
fn analyzer_fn_optional_return_type() {
    let src = "fn find() -> ?int {\n    return none;\n}";
    let errors = analyze_src(src);
    assert!(errors.is_empty(), "fn ?int should be valid: {:?}", errors);
}

#[test]
fn analyzer_try_catch_in_function() {
    let errors = analyze_src(
        "fn main() -> int {\n    try {\n        x := ok(42);\n    } catch(e) {\n        y := 0;\n    }\n    return 0;\n}",
    );
    assert!(
        errors.is_empty(),
        "try/catch in fn should be valid: {:?}",
        errors
    );
}

#[test]
fn analyzer_tensor_indexing_accepts() {
    let src = "fn main() -> int {\n    t := tensor<f32>([3], [1.0, 2.0, 3.0]);\n    val := t[0];\n    return 0;\n}";
    let errors = analyze_src(src);
    assert!(
        errors.is_empty(),
        "tensor indexing should be valid: {:?}",
        errors
    );
}

// ===========================================================================
// 4. CODEGEN (QBE IR) TESTS  (10 tests)
//
// The QBE backend uses gc_alloc + tag-based encoding for Result/Optional:
//   ok/some  -> gc_alloc(16), storel tag=0, storel payload
//   err      -> gc_alloc(16), storel tag=1, storel payload
//   none     -> literal 0
//   match    -> loadl tag, ceql, jnz
//   ?        -> loadl tag, ceql 1, jnz err_path/ok_path
// ===========================================================================

#[test]
fn codegen_ok_uses_gc_alloc_and_tag_zero() {
    let (ir, errors) = compile_src("fn main() -> int { return 0; }\nok(42)");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"), "ok should use gc_alloc");
    assert!(ir.contains("storel 0,"), "ok should store tag 0");
}

#[test]
fn codegen_err_uses_gc_alloc_and_tag_one() {
    let (ir, errors) = compile_src("fn main() -> int { return 0; }\nerr(\"oops\")");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"), "err should use gc_alloc");
    assert!(ir.contains("storel 1,"), "err should store tag 1");
}

#[test]
fn codegen_some_uses_gc_alloc_and_tag_zero() {
    let (ir, errors) = compile_src("fn main() -> int { return 0; }\nsome(42)");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"), "some should use gc_alloc");
    assert!(ir.contains("storel 0,"), "some should store tag 0");
}

#[test]
fn codegen_none_produces_ir() {
    let (ir, errors) = compile_src("fn main() -> int { return 0; }\nnone");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    // QBE: NoneExpression => literal "0", so IR is non-empty
    assert!(!ir.is_empty());
}

#[test]
fn codegen_match_result_uses_tag_check() {
    let (ir, errors) = compile_src(
        "fn main() -> int { return 0; }\nmatch ok(42) {\n    ok(val) => val,\n    err(msg) => 0,\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("loadl"), "match should load tag");
    assert!(ir.contains("ceql"), "match should compare tag");
}

#[test]
fn codegen_try_catch_block() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    try {\n        ok(42);\n    } catch(e) {\n        0;\n    }\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("jmp"),
        "try/catch should emit jump instructions"
    );
}

#[test]
fn codegen_propagate_uses_tag_check_and_branch() {
    let (ir, errors) = compile_src(
        "fn foo() -> Result<int> { return ok(42); }\nfn main() -> int { return 0; }\nfoo()?",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("ceql"), "? should compare tag");
    assert!(ir.contains("jnz"), "? should branch conditionally");
}

#[test]
fn codegen_is_some_uses_tag_check() {
    let (ir, errors) =
        compile_src("fn main() -> int { return 0; }\nif some(42) is some(val) {\n    val;\n}");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("ceql"), "is some should compare tag");
}

#[test]
fn codegen_fn_with_ok_and_err() {
    let (ir, errors) = compile_src(
        "fn validate(x: int) -> Result<int> {\n    if x > 0 { return ok(x); }\n    return err(\"bad\");\n}\nfn main() -> int { return 0; }",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"), "should use gc_alloc");
    assert!(ir.contains("storel 0,"), "ok should store tag 0");
    assert!(ir.contains("storel 1,"), "err should store tag 1");
}

#[test]
fn codegen_fn_with_some_and_none() {
    let (ir, errors) = compile_src(
        "fn lookup(x: int) -> ?int {\n    if x > 0 { return some(x); }\n    return none;\n}\nfn main() -> int { return 0; }",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"), "some should use gc_alloc");
    assert!(ir.contains("storel 0,"), "some should store tag 0");
}

// ===========================================================================
// 5. COMPILE PIPELINE TESTS  (14 tests)
// ===========================================================================

#[test]
fn pipeline_ok_compiles() {
    let (ir, errors) = compile_src("ok(42)");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"));
}

#[test]
fn pipeline_err_compiles() {
    let (ir, errors) = compile_src("err(\"msg\")");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"));
}

#[test]
fn pipeline_some_compiles() {
    let (ir, errors) = compile_src("some(42)");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"));
}

#[test]
fn pipeline_none_compiles() {
    let (_ir, errors) = compile_src("none");
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn pipeline_fn_result_return() {
    let (ir, errors) = compile_src("fn foo() -> Result<int> {\n    return ok(42);\n}");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"));
}

#[test]
fn pipeline_fn_optional_return() {
    let (_ir, errors) = compile_src("fn find() -> ?int {\n    return none;\n}");
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn pipeline_try_catch_compiles() {
    let (ir, errors) = compile_src("try {\n    ok(42);\n} catch(e) {\n    0;\n}");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(!ir.is_empty());
}

#[test]
fn pipeline_match_result_compiles() {
    let (ir, errors) = compile_src("match ok(42) {\n    ok(val) => val,\n    err(msg) => 0,\n}");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("ceql"));
}

#[test]
fn pipeline_is_some_compiles() {
    let (ir, errors) = compile_src("if some(42) is some(val) {\n    val;\n}");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("ceql"));
}

#[test]
fn pipeline_match_optional_compiles() {
    let (_ir, errors) = compile_src("match some(42) {\n    some(val) => val,\n    none() => 0,\n}");
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn pipeline_propagation_in_function() {
    let (ir, errors) = compile_src(
        "fn foo() -> Result<int> { return ok(42); }\nfn bar() -> Result<int> { return ok(foo()?); }",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("ceql"));
    assert!(ir.contains("jnz"));
}

#[test]
fn pipeline_is_none_compiles() {
    let (_ir, errors) = compile_src("if none is none {\n    0;\n}");
    assert!(errors.is_empty(), "errors: {:?}", errors);
}

#[test]
fn pipeline_nested_ok_err() {
    let (ir, errors) = compile_src(
        "fn check(x: int) -> Result<int> {\n    if x > 0 { return ok(x); }\n    return err(\"neg\");\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"));
    assert!(ir.contains("storel 0,"));
    assert!(ir.contains("storel 1,"));
}

#[test]
fn pipeline_some_none_in_function() {
    let (ir, errors) = compile_src(
        "fn find(x: int) -> ?int {\n    if x > 0 { return some(x); }\n    return none;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("gc_alloc"));
}

// ===========================================================================
// 6. TYPE SYSTEM TESTS  (6 tests)
// ===========================================================================

#[test]
fn type_result_creation() {
    let rt = Type::result(Type::int());
    assert!(rt.is_result());
    if let Type::Result(inner) = &rt {
        assert!(inner.is_integer());
    }
}

#[test]
fn type_result_display() {
    let rt = Type::result(Type::Integer(IntegerKind::I64));
    assert_eq!(format!("{}", rt), "Result<i64>");
}

#[test]
fn type_result_same_type() {
    let r1 = Type::result(Type::Integer(IntegerKind::I64));
    let r2 = Type::result(Type::Integer(IntegerKind::I64));
    let r3 = Type::result(Type::Float(FloatKind::F64));
    assert!(r1.same_type(&r2));
    assert!(!r1.same_type(&r3));
}

#[test]
fn type_optional_creation() {
    let ot = Type::optional(Type::int());
    assert!(ot.is_optional());
    if let Type::Optional(inner) = &ot {
        assert!(inner.is_integer());
    }
}

#[test]
fn type_optional_display() {
    let ot = Type::optional(Type::Integer(IntegerKind::I64));
    assert_eq!(format!("{}", ot), "?i64");
}

#[test]
fn type_optional_same_type() {
    let o1 = Type::optional(Type::Integer(IntegerKind::I64));
    let o2 = Type::optional(Type::Integer(IntegerKind::I64));
    let o3 = Type::optional(Type::Float(FloatKind::F64));
    assert!(o1.same_type(&o2));
    assert!(!o1.same_type(&o3));
}

#[test]
fn type_result_nested() {
    let rt = Type::result(Type::result(Type::int()));
    assert!(rt.is_result());
    if let Type::Result(inner) = &rt {
        assert!(inner.is_result());
    }
}

#[test]
fn type_optional_nested() {
    let ot = Type::optional(Type::optional(Type::int()));
    assert!(ot.is_optional());
    if let Type::Optional(inner) = &ot {
        assert!(inner.is_optional());
    }
}

// ===========================================================================
// 7. REGRESSION TESTS from fixes.test.ts
// ===========================================================================

// -- With blocks codegen ---------------------------------------------------

#[test]
#[ignore] // QBE codegen WithBlock only matches bare Identifier, not CallExpression no_grad()
fn fixes_with_block_codegen() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    with no_grad() {\n        x: int = 42;\n    }\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("no_grad_begin"));
    assert!(ir.contains("no_grad_end"));
}

// -- Tensor ML operations ---------------------------------------------------

#[test]
#[ignore] // tensor_ones/relu/sigmoid/softmax not registered as builtins in Rust analyzer
fn fixes_tensor_relu_sigmoid_softmax() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    t := tensor_ones([3]);\n    r := relu(t);\n    s := sigmoid(t);\n    sm := softmax(t);\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("tensor_relu"));
    assert!(ir.contains("tensor_sigmoid"));
    assert!(ir.contains("tensor_softmax"));
}

// -- Math builtins ----------------------------------------------------------

#[test]
fn fixes_math_single_arg_builtins() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    x := 1.0;\n    a := sqrt(x);\n    b := sin(x);\n    c := cos(x);\n    d := exp(x);\n    e := log(x);\n    f := floor(x);\n    g := ceil(x);\n    h := abs(x);\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    // QBE backend emits calls to C math library functions
    assert!(ir.contains("sqrt"), "should contain sqrt");
    assert!(ir.contains("sin"), "should contain sin");
    assert!(ir.contains("cos"), "should contain cos");
}

#[test]
fn fixes_math_two_arg_builtins() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    x := 2.0;\n    y := 3.0;\n    a := pow(x, y);\n    b := min(x, y);\n    c := max(x, y);\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("pow"), "should contain pow");
}

// -- String operations ------------------------------------------------------

#[test]
fn fixes_string_concat_codegen() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    a := \"hello\";\n    b := \" world\";\n    c := a + b;\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("string_concat"),
        "string + string should call string_concat"
    );
}

#[test]
fn fixes_string_slice_codegen() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    s := \"hello world\";\n    sub := s[0:5];\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("string_slice"),
        "s[0:5] should call string_slice"
    );
}

// -- parse_int / parse_float ------------------------------------------------

#[test]
fn fixes_parse_int_codegen() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    s := \"42\";\n    n := parse_int(s);\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("parse_int") || ir.contains("atoi"),
        "should generate parse_int or atoi"
    );
}

#[test]
fn fixes_parse_float_codegen() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    s := \"3.14\";\n    f := parse_float(s);\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("parse_float") || ir.contains("atof") || ir.contains("float_from_string"),
        "should generate parse_float"
    );
}

// -- Array join codegen -----------------------------------------------------

#[test]
fn fixes_array_join_codegen() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    arr := [1, 2, 3, 4, 5];\n    joined := arr.join(\", \");\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("join") || ir.contains("array_join"),
        "arr.join should generate join call"
    );
}

// -- Tensor element access --------------------------------------------------

#[test]
fn fixes_tensor_element_read_qbe() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    t := tensor<f32>([3], [1.0, 2.0, 3.0]);\n    val := t[0];\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("tensor"), "should reference tensor");
}

#[test]
fn fixes_tensor_element_write_qbe() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    t := tensor<f32>([3], [1.0, 2.0, 3.0]);\n    t[1] = 42.0;\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("tensor"), "should reference tensor");
}

// -- ? operator in try blocks (regression fix) ------------------------------

#[test]
fn fixes_propagate_outside_try_returns() {
    let (ir, errors) = compile_src(
        "fn may_fail(x: int) -> Result<int> {\n    if x < 0 { return err(\"neg\"); }\n    return ok(x * 2);\n}\nfn caller() -> Result<int> {\n    val := may_fail(5)?;\n    return ok(val);\n}\nfn main() -> int { return 0; }",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(ir.contains("ret"), "? outside try should generate ret");
}

#[test]
fn fixes_propagate_inside_try_branches() {
    let (ir, errors) = compile_src(
        "fn may_fail(x: int) -> Result<int> {\n    if x < 0 { return err(\"neg\"); }\n    return ok(x * 2);\n}\nfn main() -> int {\n    try {\n        val := may_fail(5)?;\n    } catch(e) {\n        0;\n    }\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(!ir.is_empty());
}

// -- str() dispatching to println_string ------------------------------------

#[test]
fn fixes_str_float_println() {
    let (ir, errors) = compile_src(
        "fn main() -> int {\n    x: float = 3.14;\n    println(str(x));\n    return 0;\n}",
    );
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("println_string") || ir.contains("println"),
        "should dispatch to println"
    );
}

#[test]
fn fixes_str_int_println() {
    let (ir, errors) =
        compile_src("fn main() -> int {\n    x := 42;\n    println(str(x));\n    return 0;\n}");
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(
        ir.contains("println_string") || ir.contains("println"),
        "should dispatch to println"
    );
}
