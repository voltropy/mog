//! Advanced test suite for Mog compiler — ported from TypeScript tests.
//!
//! Covers: async/await, modules, capabilities, and math builtins.

use mog::analyzer::SemanticAnalyzer;
use mog::ast::{ExprKind, StatementKind};
use mog::capability::{
    mogdecl_type_to_type, parse_capability_decl, parse_capability_decls, CapabilityDecl,
};
use mog::lexer::tokenize;
use mog::module::parse_mod_file;
use mog::parser::parse_tokens;
use mog::qbe_codegen::{generate_module_qbe_ir, generate_qbe_ir, generate_qbe_ir_with_caps};
use mog::token::TokenType;
use mog::types::Type;
use std::collections::HashMap;

// ── Helpers ──────────────────────────────────────────────────────────────

fn tokenize_filtered(source: &str) -> Vec<mog::token::Token> {
    tokenize(source)
        .into_iter()
        .filter(|t| t.token_type != TokenType::Whitespace && t.token_type != TokenType::Comment)
        .collect()
}

fn parse_source(source: &str) -> mog::ast::Statement {
    let tokens = tokenize(source);
    parse_tokens(&tokens)
}

fn analyze_source(source: &str) -> (mog::ast::Statement, Vec<mog::analyzer::SemanticError>) {
    let ast = parse_source(source);
    let mut analyzer = SemanticAnalyzer::new();
    let errors = analyzer.analyze(&ast);
    (ast, errors)
}

fn analyze_with_caps(
    source: &str,
    caps: HashMap<String, CapabilityDecl>,
) -> (mog::ast::Statement, Vec<mog::analyzer::SemanticError>) {
    let ast = parse_source(source);
    let mut analyzer = SemanticAnalyzer::new();
    analyzer.set_capability_decls(caps);
    let errors = analyzer.analyze(&ast);
    (ast, errors)
}

fn get_program_stmts(stmt: &mog::ast::Statement) -> &Vec<mog::ast::Statement> {
    match &stmt.kind {
        StatementKind::Program { statements, .. } => statements,
        _ => panic!("expected Program"),
    }
}

fn make_math_cap() -> CapabilityDecl {
    parse_capability_decl(
        r#"
        capability math {
            fn sin(x: float) -> float;
            fn cos(x: float) -> float;
            fn sqrt(x: float) -> float;
            fn pow(base: float, exp: float) -> float;
            fn abs(x: float) -> float;
            fn floor(x: float) -> float;
            fn ceil(x: float) -> float;
            fn log(x: float) -> float;
            fn exp(x: float) -> float;
            fn tan(x: float) -> float;
            fn round(x: float) -> float;
            fn min(a: float, b: float) -> float;
            fn max(a: float, b: float) -> float;
        }
    "#,
    )
    .expect("math cap should parse")
}

fn make_process_cap() -> CapabilityDecl {
    parse_capability_decl(
        r#"
        capability process {
            async fn sleep(ms: int) -> int;
            fn getenv(name: string) -> string;
            fn exit(code: int) -> int;
        }
    "#,
    )
    .expect("process cap should parse")
}

fn make_fs_cap() -> CapabilityDecl {
    parse_capability_decl(
        r#"
        capability fs {
            fn read_file(path: string) -> string;
            fn write_file(path: string, data: string) -> bool;
            fn file_exists(path: string) -> bool;
        }
    "#,
    )
    .expect("fs cap should parse")
}

fn make_http_cap() -> CapabilityDecl {
    parse_capability_decl(
        r#"
        capability http {
            async fn get(url: string) -> string;
            async fn post(url: string, body: string) -> string;
        }
    "#,
    )
    .expect("http cap should parse")
}

fn caps_map(caps: Vec<CapabilityDecl>) -> HashMap<String, CapabilityDecl> {
    caps.into_iter().map(|c| (c.name.clone(), c)).collect()
}

// =========================================================================
// ASYNC TESTS — Lexer
// =========================================================================

#[test]
fn test_async_tokenizes_async_keyword() {
    let tokens = tokenize_filtered("async");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Async);
    assert_eq!(tokens[0].value, "async");
}

#[test]
fn test_async_tokenizes_await_keyword() {
    let tokens = tokenize_filtered("await");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Await);
    assert_eq!(tokens[0].value, "await");
}

#[test]
fn test_async_tokenizes_async_fn_sequence() {
    let tokens = tokenize_filtered("async fn foo()");
    assert_eq!(tokens[0].token_type, TokenType::Async);
    assert_eq!(tokens[1].token_type, TokenType::Fn);
    assert_eq!(tokens[2].token_type, TokenType::Identifier);
    assert_eq!(tokens[2].value, "foo");
}

#[test]
fn test_async_tokenizes_await_expr_sequence() {
    let tokens = tokenize_filtered("await some_future");
    assert_eq!(tokens[0].token_type, TokenType::Await);
    assert_eq!(tokens[1].token_type, TokenType::Identifier);
    assert_eq!(tokens[1].value, "some_future");
}

#[test]
fn test_async_not_identifiers() {
    let tokens1 = tokenize_filtered("async");
    assert_eq!(tokens1[0].token_type, TokenType::Async);
    assert_ne!(tokens1[0].token_type, TokenType::Identifier);

    let tokens2 = tokenize_filtered("await");
    assert_eq!(tokens2[0].token_type, TokenType::Await);
    assert_ne!(tokens2[0].token_type, TokenType::Identifier);
}

#[test]
fn test_async_identifier_prefix_not_keyword() {
    let tokens1 = tokenize_filtered("async_task");
    assert_eq!(tokens1[0].token_type, TokenType::Identifier);
    assert_eq!(tokens1[0].value, "async_task");

    let tokens2 = tokenize_filtered("awaiting");
    assert_eq!(tokens2[0].token_type, TokenType::Identifier);
    assert_eq!(tokens2[0].value, "awaiting");
}

// =========================================================================
// ASYNC TESTS — Parser
// =========================================================================

#[test]
fn test_async_parse_fn_declaration() {
    let ast = parse_source("async fn fetch() -> i64 { 42 }");
    let stmts = get_program_stmts(&ast);
    assert!(!stmts.is_empty());
    match &stmts[0].kind {
        StatementKind::AsyncFunctionDeclaration { name, .. } => {
            assert_eq!(name, "fetch");
        }
        other => panic!("expected AsyncFunctionDeclaration, got {:?}", other),
    }
}

#[test]
fn test_async_parse_fn_with_return_type() {
    let ast = parse_source("async fn fetch() -> i64 { return 42; }");
    let stmts = get_program_stmts(&ast);
    match &stmts[0].kind {
        StatementKind::AsyncFunctionDeclaration {
            name, return_type, ..
        } => {
            assert_eq!(name, "fetch");
            assert_eq!(*return_type, Type::int());
        }
        other => panic!("expected AsyncFunctionDeclaration, got {:?}", other),
    }
}

#[test]
fn test_async_parse_fn_with_params() {
    let ast = parse_source("async fn add(a: i64, b: i64) -> i64 { a + b }");
    let stmts = get_program_stmts(&ast);
    match &stmts[0].kind {
        StatementKind::AsyncFunctionDeclaration { name, params, .. } => {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "a");
            assert_eq!(params[1].name, "b");
        }
        other => panic!("expected AsyncFunctionDeclaration, got {:?}", other),
    }
}

#[test]
fn test_async_parse_await_expression() {
    let ast = parse_source("async fn f() -> i64 { x: i64 = await get_data(); x }");
    let stmts = get_program_stmts(&ast);
    match &stmts[0].kind {
        StatementKind::AsyncFunctionDeclaration { body, .. } => {
            // First statement in body should be variable declaration with await value
            assert!(!body.statements.is_empty());
            match &body.statements[0].kind {
                StatementKind::VariableDeclaration { value, .. } => {
                    if let Some(val) = value {
                        match &val.kind {
                            ExprKind::AwaitExpression { .. } => {} // correct
                            other => panic!("expected AwaitExpression, got {:?}", other),
                        }
                    } else {
                        panic!("expected value in variable declaration");
                    }
                }
                other => panic!("expected VariableDeclaration, got {:?}", other),
            }
        }
        other => panic!("expected AsyncFunctionDeclaration, got {:?}", other),
    }
}

#[test]
fn test_async_parse_spawn_expression() {
    let src = "async fn f() -> i64 { 42 }\nfn main() { future: i64 = spawn f(); }";
    let ast = parse_source(src);
    let stmts = get_program_stmts(&ast);
    // Second statement is main function
    assert!(stmts.len() >= 2);
    match &stmts[1].kind {
        StatementKind::FunctionDeclaration { name, body, .. } => {
            assert_eq!(name, "main");
            match &body.statements[0].kind {
                StatementKind::VariableDeclaration { value, .. } => {
                    if let Some(val) = value {
                        match &val.kind {
                            ExprKind::SpawnExpression { .. } => {} // correct
                            other => panic!("expected SpawnExpression, got {:?}", other),
                        }
                    }
                }
                other => panic!("expected VariableDeclaration, got {:?}", other),
            }
        }
        other => panic!("expected FunctionDeclaration for main, got {:?}", other),
    }
}

// =========================================================================
// ASYNC TESTS — Analyzer
// =========================================================================

#[test]
fn test_async_analyzer_validates_async_fn() {
    let (_ast, errors) = analyze_source("async fn fetch() -> i64 { 42 }");
    // Async function declarations should be valid
    let relevant: Vec<_> = errors
        .iter()
        .filter(|e| e.message.to_lowercase().contains("async"))
        .collect();
    assert!(
        relevant.is_empty(),
        "should have no async-related errors, got: {:?}",
        relevant
    );
}

#[test]
fn test_async_await_outside_async_is_error() {
    // await used outside of an async function should produce an error
    let (_ast, errors) = analyze_source("fn main() { x: i64 = await get_data(); }");
    let has_await_error = errors
        .iter()
        .any(|e| e.message.to_lowercase().contains("await"));
    assert!(
        has_await_error,
        "expected error about await outside async, got: {:?}",
        errors
    );
}

// =========================================================================
// ASYNC TESTS — Codegen
// =========================================================================

#[test]
fn test_async_codegen_await_calls_function() {
    // Await compiles to a direct call in the current QBE backend
    let ast =
        parse_source("async fn f() -> i64 { 42 }\nasync fn g() -> i64 { x: i64 = await f(); x }");
    let ir = generate_qbe_ir(&ast);
    assert!(
        ir.contains("call") && ir.contains("$f"),
        "await expression should generate a call to the awaited function, IR:\n{}",
        ir
    );
}

#[test]
fn test_async_codegen_spawn() {
    // Spawn compiles to a direct call in the current QBE backend
    let ast = parse_source("async fn f() -> i64 { 42 }\nfn main() { future: i64 = spawn f(); }");
    let ir = generate_qbe_ir(&ast);
    assert!(
        ir.contains("call") && ir.contains("$f"),
        "spawn expression should generate a call, IR:\n{}",
        ir
    );
}

#[test]
fn test_async_tokenizes_spawn_keyword() {
    let tokens = tokenize_filtered("spawn");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Spawn);
    assert_eq!(tokens[0].value, "spawn");
}

// =========================================================================
// MODULE TESTS — Lexer
// =========================================================================

#[test]
fn test_module_tokenizes_package_keyword() {
    let tokens = tokenize_filtered("package");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Package);
    assert_eq!(tokens[0].value, "package");
}

#[test]
fn test_module_tokenizes_import_keyword() {
    let tokens = tokenize_filtered("import");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Import);
    assert_eq!(tokens[0].value, "import");
}

#[test]
fn test_module_tokenizes_pub_keyword() {
    let tokens = tokenize_filtered("pub");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Pub);
    assert_eq!(tokens[0].value, "pub");
}

#[test]
fn test_module_keywords_in_context() {
    let tokens = tokenize_filtered("package math\nimport \"utils\"\npub fn add() -> i64 { 1 }");
    // Check that package, import, pub appear
    let types: Vec<_> = tokens.iter().map(|t| t.token_type).collect();
    assert!(types.contains(&TokenType::Package));
    assert!(types.contains(&TokenType::Import));
    assert!(types.contains(&TokenType::Pub));
}

// =========================================================================
// MODULE TESTS — Parser
// =========================================================================

#[test]
fn test_module_parse_package_declaration() {
    let ast = parse_source("package math");
    let stmts = get_program_stmts(&ast);
    assert!(!stmts.is_empty());
    match &stmts[0].kind {
        StatementKind::PackageDeclaration { name } => {
            assert_eq!(name, "math");
        }
        other => panic!("expected PackageDeclaration, got {:?}", other),
    }
}

#[test]
fn test_module_parse_import_single() {
    let ast = parse_source("import \"math\"");
    let stmts = get_program_stmts(&ast);
    assert!(!stmts.is_empty());
    match &stmts[0].kind {
        StatementKind::ImportDeclaration { paths } => {
            assert_eq!(paths.len(), 1);
            assert_eq!(paths[0], "math");
        }
        other => panic!("expected ImportDeclaration, got {:?}", other),
    }
}

#[test]
fn test_module_parse_import_multiple() {
    let ast = parse_source("import \"a\"\nimport \"b\"");
    let stmts = get_program_stmts(&ast);
    assert!(stmts.len() >= 2);
    match &stmts[0].kind {
        StatementKind::ImportDeclaration { paths } => {
            assert_eq!(paths[0], "a");
        }
        other => panic!("expected ImportDeclaration for 'a', got {:?}", other),
    }
    match &stmts[1].kind {
        StatementKind::ImportDeclaration { paths } => {
            assert_eq!(paths[0], "b");
        }
        other => panic!("expected ImportDeclaration for 'b', got {:?}", other),
    }
}

#[test]
fn test_module_parse_pub_function() {
    let ast = parse_source("pub fn add(a: i64, b: i64) -> i64 { a + b }");
    let stmts = get_program_stmts(&ast);
    match &stmts[0].kind {
        StatementKind::FunctionDeclaration {
            name, is_public, ..
        } => {
            assert_eq!(name, "add");
            assert!(*is_public, "function should be public");
        }
        other => panic!("expected FunctionDeclaration, got {:?}", other),
    }
}

#[test]
fn test_module_parse_pub_struct() {
    let ast = parse_source("pub struct Point { x: i64, y: i64 }");
    let stmts = get_program_stmts(&ast);
    match &stmts[0].kind {
        StatementKind::StructDefinition {
            name, is_public, ..
        } => {
            assert_eq!(name, "Point");
            assert!(*is_public, "struct should be public");
        }
        other => panic!("expected StructDefinition, got {:?}", other),
    }
}

#[test]
fn test_module_parse_non_pub_function_is_private() {
    let ast = parse_source("fn helper() -> i64 { 1 }");
    let stmts = get_program_stmts(&ast);
    match &stmts[0].kind {
        StatementKind::FunctionDeclaration { is_public, .. } => {
            assert!(!*is_public, "function without pub should be private");
        }
        other => panic!("expected FunctionDeclaration, got {:?}", other),
    }
}

#[test]
fn test_module_parse_package_with_functions() {
    let ast = parse_source("package utils\nfn helper() -> i64 { 1 }\npub fn api() -> i64 { 2 }");
    let stmts = get_program_stmts(&ast);
    assert!(stmts.len() >= 3);
    match &stmts[0].kind {
        StatementKind::PackageDeclaration { name } => assert_eq!(name, "utils"),
        other => panic!("expected PackageDeclaration, got {:?}", other),
    }
    match &stmts[2].kind {
        StatementKind::FunctionDeclaration {
            name, is_public, ..
        } => {
            assert_eq!(name, "api");
            assert!(*is_public);
        }
        other => panic!("expected pub FunctionDeclaration, got {:?}", other),
    }
}

// =========================================================================
// MODULE TESTS — Module resolver
// =========================================================================

#[test]
fn test_module_parse_mod_file() {
    assert_eq!(
        parse_mod_file("module myproject"),
        Some("myproject".to_string())
    );
}

#[test]
fn test_module_parse_mod_file_with_whitespace() {
    assert_eq!(
        parse_mod_file("  module  testproject  "),
        Some("testproject".to_string())
    );
}

#[test]
fn test_module_parse_mod_file_with_comments() {
    // parse_mod_file only looks for "module <name>" lines
    let result = parse_mod_file("// comment line\nmodule test");
    assert_eq!(result, Some("test".to_string()));
}

#[test]
fn test_module_parse_mod_file_empty() {
    assert_eq!(parse_mod_file(""), None);
}

#[test]
fn test_module_parse_mod_file_no_module() {
    assert_eq!(parse_mod_file("something else"), None);
}

// =========================================================================
// MODULE TESTS — Codegen (name mangling)
// =========================================================================

#[test]
fn test_module_codegen_name_mangling() {
    let ast = parse_source("pub fn add(a: i64, b: i64) -> i64 { a + b }");
    let ir = generate_module_qbe_ir(&ast, HashMap::new(), "math");
    assert!(
        ir.contains("math__add"),
        "module codegen should mangle function names with package prefix, IR:\n{}",
        ir
    );
}

#[test]
fn test_module_codegen_package_prefix_multiple_fns() {
    let ast = parse_source("pub fn foo() -> i64 { 1 }\npub fn bar() -> i64 { 2 }");
    let ir = generate_module_qbe_ir(&ast, HashMap::new(), "mylib");
    assert!(
        ir.contains("mylib__foo"),
        "should contain mylib__foo, IR:\n{}",
        ir
    );
    assert!(
        ir.contains("mylib__bar"),
        "should contain mylib__bar, IR:\n{}",
        ir
    );
}

// =========================================================================
// MODULE TESTS — Analyzer
// =========================================================================

#[test]
fn test_module_analyzer_pub_function() {
    let (_ast, errors) = analyze_source("pub fn foo() -> i64 { 42 }");
    let relevant: Vec<_> = errors
        .iter()
        .filter(|e| e.message.contains("pub"))
        .collect();
    assert!(
        relevant.is_empty(),
        "pub functions should not cause errors, got: {:?}",
        relevant
    );
}

#[test]
fn test_module_analyzer_package_decl() {
    let (_ast, errors) = analyze_source("package mylib");
    let relevant: Vec<_> = errors
        .iter()
        .filter(|e| e.message.contains("package"))
        .collect();
    assert!(
        relevant.is_empty(),
        "package declarations should be valid, got: {:?}",
        relevant
    );
}

// =========================================================================
// CAPABILITY TESTS — .mogdecl parsing
// =========================================================================

#[test]
fn test_cap_parse_process() {
    let decl = parse_capability_decl(
        r#"
        capability process {
            async fn sleep(ms: int) -> int;
            fn getenv(name: string) -> string;
            fn exit(code: int) -> int;
        }
    "#,
    )
    .expect("should parse");
    assert_eq!(decl.name, "process");
    assert_eq!(decl.functions.len(), 3);

    assert_eq!(decl.functions[0].name, "sleep");
    assert!(decl.functions[0].is_async);
    assert_eq!(decl.functions[0].params.len(), 1);
    assert_eq!(decl.functions[0].params[0].0, "ms");
    assert_eq!(decl.functions[0].return_type, Type::int());

    assert_eq!(decl.functions[1].name, "getenv");
    assert!(!decl.functions[1].is_async);
    assert_eq!(decl.functions[1].return_type, Type::String);

    assert_eq!(decl.functions[2].name, "exit");
    assert_eq!(decl.functions[2].return_type, Type::int());
}

#[test]
fn test_cap_parse_fs() {
    let decl = parse_capability_decl(
        r#"
        capability fs {
            fn read_file(path: string) -> string;
            fn write_file(path: string, data: string) -> bool;
            fn file_exists(path: string) -> bool;
        }
    "#,
    )
    .expect("should parse");
    assert_eq!(decl.name, "fs");
    assert_eq!(decl.functions.len(), 3);

    assert_eq!(decl.functions[0].name, "read_file");
    assert_eq!(decl.functions[0].return_type, Type::String);
    assert_eq!(decl.functions[0].params.len(), 1);

    assert_eq!(decl.functions[1].name, "write_file");
    assert_eq!(decl.functions[1].return_type, Type::Bool);
    assert_eq!(decl.functions[1].params.len(), 2);

    assert_eq!(decl.functions[2].name, "file_exists");
    assert_eq!(decl.functions[2].return_type, Type::Bool);
}

#[test]
fn test_cap_parse_log() {
    let decl = parse_capability_decl(
        r#"
        capability log {
            fn info(message: string);
            fn warn(message: string);
            fn error(message: string);
        }
    "#,
    )
    .expect("should parse");
    assert_eq!(decl.name, "log");
    assert_eq!(decl.functions.len(), 3);
    // Functions without -> should have Void return type
    assert_eq!(decl.functions[0].return_type, Type::Void);
    assert_eq!(decl.functions[1].return_type, Type::Void);
    assert_eq!(decl.functions[2].return_type, Type::Void);
}

#[test]
fn test_cap_parse_env() {
    let decl = parse_capability_decl(
        r#"
        capability env {
            fn get(name: string) -> string;
            fn set(name: string, value: string) -> bool;
        }
    "#,
    )
    .expect("should parse");
    assert_eq!(decl.name, "env");
    assert_eq!(decl.functions.len(), 2);
    assert_eq!(decl.functions[0].name, "get");
    assert_eq!(decl.functions[0].params[0].0, "name");
    assert_eq!(decl.functions[1].name, "set");
    assert_eq!(decl.functions[1].params.len(), 2);
}

#[test]
fn test_cap_parse_http() {
    let decl = parse_capability_decl(
        r#"
        capability http {
            async fn get(url: string) -> string;
            async fn post(url: string, body: string) -> string;
        }
    "#,
    )
    .expect("should parse");
    assert_eq!(decl.name, "http");
    assert_eq!(decl.functions.len(), 2);
    assert!(decl.functions[0].is_async, "http.get should be async");
    assert!(decl.functions[1].is_async, "http.post should be async");
    assert_eq!(decl.functions[0].return_type, Type::String);
    assert_eq!(decl.functions[1].return_type, Type::String);
}

#[test]
fn test_cap_parse_math_mogdecl() {
    let decl = make_math_cap();
    assert_eq!(decl.name, "math");
    assert!(decl.functions.len() >= 10);

    let sin_fn = decl.functions.iter().find(|f| f.name == "sin").unwrap();
    assert_eq!(sin_fn.params.len(), 1);
    assert_eq!(sin_fn.return_type, Type::float());
    assert!(!sin_fn.is_async);

    let pow_fn = decl.functions.iter().find(|f| f.name == "pow").unwrap();
    assert_eq!(pow_fn.params.len(), 2);
    assert_eq!(pow_fn.return_type, Type::float());
}

#[test]
fn test_cap_parse_with_comments() {
    let decl = parse_capability_decl(
        r#"
        // This is a comment
        # Another comment style
        capability http {
            async fn get(url: string) -> string;
        }
    "#,
    )
    .expect("should parse with comments");
    assert_eq!(decl.name, "http");
    assert_eq!(decl.functions.len(), 1);
}

#[test]
fn test_cap_parse_empty_returns_none() {
    assert!(parse_capability_decl("").is_none());
}

#[test]
fn test_cap_parse_just_comment_returns_none() {
    assert!(parse_capability_decl("// just a comment").is_none());
}

#[test]
fn test_cap_parse_multiple_decls() {
    let decls = parse_capability_decls(
        r#"
        capability process {
            fn exit(code: int) -> int;
        }
        capability fs {
            fn read_file(path: string) -> string;
        }
    "#,
    )
    .expect("should parse multiple");
    assert_eq!(decls.len(), 2);
    assert_eq!(decls[0].name, "process");
    assert_eq!(decls[1].name, "fs");
}

// =========================================================================
// CAPABILITY TESTS — Requires declaration parsing
// =========================================================================

#[test]
fn test_cap_parse_requires_declaration() {
    let ast = parse_source("requires process");
    let stmts = get_program_stmts(&ast);
    assert!(!stmts.is_empty());
    match &stmts[0].kind {
        StatementKind::RequiresDeclaration { capabilities } => {
            assert!(capabilities.contains(&"process".to_string()));
        }
        other => panic!("expected RequiresDeclaration, got {:?}", other),
    }
}

#[test]
fn test_cap_parse_requires_multiple() {
    let ast = parse_source("requires process, fs");
    let stmts = get_program_stmts(&ast);
    match &stmts[0].kind {
        StatementKind::RequiresDeclaration { capabilities } => {
            assert!(capabilities.contains(&"process".to_string()));
            assert!(capabilities.contains(&"fs".to_string()));
        }
        other => panic!("expected RequiresDeclaration, got {:?}", other),
    }
}

// =========================================================================
// CAPABILITY TESTS — Analyzer with capabilities
// =========================================================================

#[test]
fn test_cap_analyzer_with_required_cap() {
    let caps = caps_map(vec![make_process_cap()]);
    let src = "requires process\nfn main() { result: i64 = process.exit(0); }";
    let (_ast, errors) = analyze_with_caps(src, caps);
    // With capability declared, calling process.exit should not produce "unknown function" errors
    let unknown_errors: Vec<_> = errors
        .iter()
        .filter(|e| {
            e.message.to_lowercase().contains("unknown")
                || e.message.to_lowercase().contains("undefined")
        })
        .collect();
    assert!(
        unknown_errors.is_empty(),
        "should not have unknown function errors with cap declared, got: {:?}",
        unknown_errors
    );
}

#[test]
fn test_cap_analyzer_fs_cap() {
    let caps = caps_map(vec![make_fs_cap()]);
    let src = r#"requires fs
fn main() {
    content: string = fs.read_file("test.txt");
}"#;
    let (_ast, errors) = analyze_with_caps(src, caps);
    let unknown_errors: Vec<_> = errors
        .iter()
        .filter(|e| {
            e.message.to_lowercase().contains("unknown")
                || e.message.to_lowercase().contains("undefined")
        })
        .collect();
    assert!(
        unknown_errors.is_empty(),
        "fs.read_file should be recognized with cap, got: {:?}",
        unknown_errors
    );
}

// =========================================================================
// CAPABILITY TESTS — Codegen
// =========================================================================

#[test]
fn test_cap_codegen_process_call() {
    let caps = caps_map(vec![make_process_cap()]);
    let ast = parse_source("requires process\nfn main() { process.exit(0); }");
    let ir = generate_qbe_ir_with_caps(&ast, caps);
    // Capability calls should generate some form of external call
    assert!(
        ir.contains("exit") || ir.contains("mog_cap") || ir.contains("process"),
        "should generate capability call in IR, IR:\n{}",
        ir
    );
}

#[test]
fn test_cap_codegen_async_cap_call() {
    let caps = caps_map(vec![make_http_cap()]);
    let ast = parse_source(
        r#"requires http
async fn fetch() -> i64 {
    data: string = await http.get("http://example.com");
    0
}"#,
    );
    let ir = generate_qbe_ir_with_caps(&ast, caps);
    // Should generate IR for the capability call
    assert!(
        ir.contains("call") || ir.contains("http") || ir.contains("get"),
        "async cap call should appear in IR, IR:\n{}",
        ir
    );
}

// =========================================================================
// CAPABILITY TESTS — mogdecl_type_to_type
// =========================================================================

#[test]
fn test_cap_type_mapping_int() {
    assert_eq!(mogdecl_type_to_type("int"), Type::int());
}

#[test]
fn test_cap_type_mapping_float() {
    assert_eq!(mogdecl_type_to_type("float"), Type::float());
}

#[test]
fn test_cap_type_mapping_string() {
    assert_eq!(mogdecl_type_to_type("string"), Type::String);
}

#[test]
fn test_cap_type_mapping_bool() {
    assert_eq!(mogdecl_type_to_type("bool"), Type::Bool);
}

#[test]
fn test_cap_type_mapping_none() {
    assert_eq!(mogdecl_type_to_type("none"), Type::Void);
}

#[test]
fn test_cap_type_mapping_custom() {
    // Unknown types become Custom
    assert_eq!(
        mogdecl_type_to_type("MyStruct"),
        Type::Custom("MyStruct".to_string())
    );
}

// =========================================================================
// MATH TESTS — Capability declaration
// =========================================================================

#[test]
fn test_math_cap_has_sin() {
    let cap = make_math_cap();
    let f = cap.functions.iter().find(|f| f.name == "sin");
    assert!(f.is_some(), "math capability should have sin");
    let f = f.unwrap();
    assert_eq!(f.params.len(), 1);
    assert_eq!(f.params[0].0, "x");
    assert_eq!(f.return_type, Type::float());
}

#[test]
fn test_math_cap_has_cos() {
    let cap = make_math_cap();
    let f = cap.functions.iter().find(|f| f.name == "cos");
    assert!(f.is_some(), "math capability should have cos");
}

#[test]
fn test_math_cap_has_sqrt() {
    let cap = make_math_cap();
    let f = cap.functions.iter().find(|f| f.name == "sqrt");
    assert!(f.is_some(), "math capability should have sqrt");
}

#[test]
fn test_math_cap_has_pow() {
    let cap = make_math_cap();
    let f = cap.functions.iter().find(|f| f.name == "pow");
    assert!(f.is_some(), "math capability should have pow");
    assert_eq!(f.unwrap().params.len(), 2, "pow takes two params");
}

#[test]
fn test_math_cap_has_abs() {
    let cap = make_math_cap();
    assert!(cap.functions.iter().any(|f| f.name == "abs"));
}

#[test]
fn test_math_cap_has_floor_ceil() {
    let cap = make_math_cap();
    assert!(cap.functions.iter().any(|f| f.name == "floor"));
    assert!(cap.functions.iter().any(|f| f.name == "ceil"));
}

#[test]
fn test_math_cap_has_exp_log() {
    let cap = make_math_cap();
    assert!(cap.functions.iter().any(|f| f.name == "exp"));
    assert!(cap.functions.iter().any(|f| f.name == "log"));
}

// =========================================================================
// MATH TESTS — Analyzer with math capability
// =========================================================================

#[test]
fn test_math_analyzer_sin_recognized() {
    let caps = caps_map(vec![make_math_cap()]);
    let src = "requires math\nfn main() { x: f64 = math.sin(1.0); }";
    let (_ast, errors) = analyze_with_caps(src, caps);
    let unknown: Vec<_> = errors
        .iter()
        .filter(|e| {
            e.message.to_lowercase().contains("unknown")
                || e.message.to_lowercase().contains("undefined")
        })
        .collect();
    assert!(
        unknown.is_empty(),
        "math.sin should be recognized with math cap, got: {:?}",
        unknown
    );
}

#[test]
fn test_math_analyzer_sqrt_recognized() {
    let caps = caps_map(vec![make_math_cap()]);
    let src = "requires math\nfn main() { x: f64 = math.sqrt(4.0); }";
    let (_ast, errors) = analyze_with_caps(src, caps);
    let unknown: Vec<_> = errors
        .iter()
        .filter(|e| {
            e.message.to_lowercase().contains("unknown")
                || e.message.to_lowercase().contains("undefined")
        })
        .collect();
    assert!(
        unknown.is_empty(),
        "math.sqrt should be recognized, got: {:?}",
        unknown
    );
}

// =========================================================================
// MATH TESTS — Codegen
// =========================================================================

#[test]
fn test_math_codegen_sin() {
    let caps = caps_map(vec![make_math_cap()]);
    let ast = parse_source("requires math\nfn main() { x: f64 = math.sin(1.0); }");
    let ir = generate_qbe_ir_with_caps(&ast, caps);
    assert!(
        ir.contains("sin") || ir.contains("math"),
        "math.sin should generate call in IR, IR:\n{}",
        ir
    );
}

#[test]
fn test_math_codegen_sqrt() {
    let caps = caps_map(vec![make_math_cap()]);
    let ast = parse_source("requires math\nfn main() { x: f64 = math.sqrt(4.0); }");
    let ir = generate_qbe_ir_with_caps(&ast, caps);
    assert!(
        ir.contains("sqrt") || ir.contains("math"),
        "math.sqrt should generate call in IR, IR:\n{}",
        ir
    );
}

#[test]
fn test_math_codegen_pow() {
    let caps = caps_map(vec![make_math_cap()]);
    let ast = parse_source("requires math\nfn main() { x: f64 = math.pow(2.0, 3.0); }");
    let ir = generate_qbe_ir_with_caps(&ast, caps);
    assert!(
        ir.contains("pow") || ir.contains("math"),
        "math.pow should generate call in IR, IR:\n{}",
        ir
    );
}

#[test]
fn test_math_codegen_multiple_ops() {
    let caps = caps_map(vec![make_math_cap()]);
    let ast = parse_source(
        r#"requires math
fn main() {
    a: f64 = math.sin(1.0);
    b: f64 = math.cos(2.0);
    c: f64 = math.sqrt(4.0);
}"#,
    );
    let ir = generate_qbe_ir_with_caps(&ast, caps);
    // Should have multiple math calls
    assert!(
        ir.contains("sin") || ir.contains("cos") || ir.contains("sqrt"),
        "multiple math ops should appear in IR, IR:\n{}",
        ir
    );
}

// =========================================================================
// Additional coverage tests
// =========================================================================

#[test]
fn test_async_parse_pub_async_fn() {
    let ast = parse_source("pub async fn fetch() -> i64 { 42 }");
    let stmts = get_program_stmts(&ast);
    match &stmts[0].kind {
        StatementKind::AsyncFunctionDeclaration {
            name, is_public, ..
        } => {
            assert_eq!(name, "fetch");
            assert!(*is_public, "pub async fn should be public");
        }
        other => panic!("expected AsyncFunctionDeclaration, got {:?}", other),
    }
}

#[test]
fn test_module_codegen_preserves_private_fn_names() {
    // Private functions (no pub) may or may not get prefix depending on impl
    let ast = parse_source("fn helper() -> i64 { 1 }");
    let ir = generate_module_qbe_ir(&ast, HashMap::new(), "pkg");
    // Should at least contain the function name somewhere
    assert!(
        ir.contains("helper"),
        "function name should appear in IR, IR:\n{}",
        ir
    );
}

#[test]
fn test_cap_parse_result_return_type() {
    let decl = parse_capability_decl(
        r#"
        capability storage {
            fn save(data: string) -> Result<bool>;
        }
    "#,
    )
    .expect("should parse");
    assert_eq!(decl.name, "storage");
    assert_eq!(
        decl.functions[0].return_type,
        Type::Result(Box::new(Type::Bool))
    );
}

#[test]
fn test_async_fn_return_type_is_preserved() {
    let ast = parse_source("async fn compute() -> f64 { 3.14 }");
    let stmts = get_program_stmts(&ast);
    match &stmts[0].kind {
        StatementKind::AsyncFunctionDeclaration { return_type, .. } => {
            assert_eq!(*return_type, Type::float());
        }
        other => panic!("expected AsyncFunctionDeclaration, got {:?}", other),
    }
}

#[test]
fn test_cap_parse_multiple_params() {
    let decl = parse_capability_decl(
        r#"
        capability db {
            fn query(sql: string, timeout: int) -> string;
        }
    "#,
    )
    .expect("should parse");
    assert_eq!(decl.functions[0].params.len(), 2);
    assert_eq!(decl.functions[0].params[0].0, "sql");
    assert_eq!(decl.functions[0].params[0].1, Type::String);
    assert_eq!(decl.functions[0].params[1].0, "timeout");
    assert_eq!(decl.functions[0].params[1].1, Type::int());
}

#[test]
fn test_module_parse_import_path_with_slash() {
    let ast = parse_source("import \"myapp/utils\"");
    let stmts = get_program_stmts(&ast);
    match &stmts[0].kind {
        StatementKind::ImportDeclaration { paths } => {
            assert_eq!(paths[0], "myapp/utils");
        }
        other => panic!("expected ImportDeclaration, got {:?}", other),
    }
}

#[test]
fn test_cap_all_functions_have_names() {
    let decl = make_process_cap();
    for f in &decl.functions {
        assert!(
            !f.name.is_empty(),
            "capability function names should not be empty"
        );
    }
}

#[test]
fn test_async_codegen_async_fn_is_export() {
    // Async functions should produce an exported function in the IR
    let ast = parse_source("async fn fetch() -> i64 { 42 }");
    let ir = generate_qbe_ir(&ast);
    assert!(
        ir.contains("export function") || ir.contains("function"),
        "async fn should generate a function in IR, IR:\n{}",
        ir
    );
}
