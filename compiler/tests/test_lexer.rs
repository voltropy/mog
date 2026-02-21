use mog::lexer::tokenize;
use mog::token::TokenType;

// Helper: filter out whitespace tokens, return (token_type, value) pairs
fn non_ws(source: &str) -> Vec<(TokenType, String)> {
    tokenize(source)
        .into_iter()
        .filter(|t| t.token_type != TokenType::Whitespace)
        .map(|t| (t.token_type, t.value))
        .collect()
}

// Helper: get just the token types, excluding whitespace
fn types(source: &str) -> Vec<TokenType> {
    tokenize(source)
        .into_iter()
        .filter(|t| t.token_type != TokenType::Whitespace)
        .map(|t| t.token_type)
        .collect()
}

// Helper: get just the values, excluding whitespace
fn values(source: &str) -> Vec<String> {
    tokenize(source)
        .into_iter()
        .filter(|t| t.token_type != TokenType::Whitespace)
        .map(|t| t.value)
        .collect()
}

// ==========================================================================
// Multiline comment edge cases
// ==========================================================================

#[test]
fn multiline_comment_spanning_several_lines() {
    let tokens = tokenize("/* line1\nline2\nline3 */");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Comment);
    assert_eq!(tokens[0].value, "/* line1\nline2\nline3 */");
    // Start at line 1, end at line 3
    assert_eq!(tokens[0].span.start.line, 1);
    assert_eq!(tokens[0].span.start.column, 1);
    assert_eq!(tokens[0].span.end.line, 3);
}

#[test]
fn multiline_comment_after_code() {
    let toks = non_ws("x /* comment */");
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0], (TokenType::Identifier, "x".into()));
    assert_eq!(toks[1], (TokenType::Comment, "/* comment */".into()));
}

#[test]
fn multiline_comment_between_tokens() {
    let toks = non_ws("x /* comment */ y");
    assert_eq!(toks.len(), 3);
    assert_eq!(toks[0].1, "x");
    assert_eq!(toks[1].0, TokenType::Comment);
    assert_eq!(toks[2].1, "y");
}

#[test]
fn multiline_comment_empty() {
    let tokens = tokenize("/**/");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Comment);
    assert_eq!(tokens[0].value, "/**/");
}

#[test]
fn multiline_comment_special_characters() {
    let tokens = tokenize("/* @#$%^&*()!~`|\\{}[]<>?;:'\",. */");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Comment);
    assert_eq!(tokens[0].value, "/* @#$%^&*()!~`|\\{}[]<>?;:'\",. */");
}

#[test]
fn multiline_comment_unterminated_produces_unknown() {
    let tokens = tokenize("/* unterminated");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Unknown);
    assert_eq!(tokens[0].value, "/* unterminated");
}

#[test]
fn multiline_comment_unterminated_nested_produces_unknown() {
    // outer opened, inner opened + closed, but outer never closed
    let tokens = tokenize("/* outer /* inner */");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Unknown);
    assert_eq!(tokens[0].value, "/* outer /* inner */");
}

#[test]
fn multiline_comment_deeply_nested() {
    let tokens = tokenize("/* a /* b /* c */ d */ e */");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Comment);
    assert_eq!(tokens[0].value, "/* a /* b /* c */ d */ e */");
}

#[test]
fn multiline_comment_with_code_on_different_lines() {
    let toks = non_ws("x\n/* comment */\ny");
    assert_eq!(toks.len(), 3);
    assert_eq!(toks[0], (TokenType::Identifier, "x".into()));
    assert_eq!(toks[1], (TokenType::Comment, "/* comment */".into()));
    assert_eq!(toks[2], (TokenType::Identifier, "y".into()));
}

#[test]
fn single_and_multiline_comments_together() {
    let tokens = tokenize("// single\n/* multi */");
    let comments: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Comment)
        .collect();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].value, "// single");
    assert_eq!(comments[1].value, "/* multi */");
}

#[test]
fn division_not_confused_with_multiline_comment() {
    let toks = types("a / b");
    assert_eq!(
        toks,
        vec![
            TokenType::Identifier,
            TokenType::Divide,
            TokenType::Identifier
        ]
    );
}

#[test]
fn multiline_comment_with_stars_inside() {
    let tokens = tokenize("/* * ** *** */");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Comment);
    assert_eq!(tokens[0].value, "/* * ** *** */");
}

#[test]
fn multiline_comment_with_slashes_inside() {
    let tokens = tokenize("/* // not a line comment */");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Comment);
}

// ==========================================================================
// F-string / template string edge cases
// ==========================================================================

#[test]
fn fstring_simple_no_interp() {
    let tokens = tokenize(r#"f"hello world""#);
    let types: Vec<_> = tokens.iter().map(|t| t.token_type).collect();
    assert_eq!(types[0], TokenType::TemplateStringStart);
    // The last should be TemplateStringEnd
    assert_eq!(*types.last().unwrap(), TokenType::TemplateStringEnd);
    // Should have a part with "hello world"
    let parts: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::TemplateStringPart)
        .collect();
    assert_eq!(parts[0].value, "hello world");
}

#[test]
fn fstring_empty() {
    let tokens = tokenize(r#"f"""#);
    assert_eq!(tokens[0].token_type, TokenType::TemplateStringStart);
    assert_eq!(tokens[1].token_type, TokenType::TemplateStringEnd);
}

#[test]
fn fstring_with_expression() {
    let tokens = tokenize(r#"f"Result: {x + y}""#);
    let parts: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::TemplateStringPart)
        .collect();
    // Should contain "Result: " as a text part and "x + y" as an expression part
    assert!(parts.iter().any(|p| p.value == "Result: "));
    assert!(parts.iter().any(|p| p.value == "x + y"));
}

#[test]
fn fstring_multiple_interpolations() {
    let tokens = tokenize(r#"f"{a} and {b}""#);
    let interp_starts: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::TemplateInterpStart)
        .collect();
    let interp_ends: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::TemplateInterpEnd)
        .collect();
    assert_eq!(interp_starts.len(), 2);
    assert_eq!(interp_ends.len(), 2);
}

#[test]
fn fstring_escaped_braces() {
    let tokens = tokenize(r#"f"{{literal}}""#);
    let parts: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::TemplateStringPart)
        .collect();
    // Escaped {{ -> "{" and "literal" and }} -> "}"
    assert!(parts.iter().any(|p| p.value == "{"));
    assert!(parts.iter().any(|p| p.value == "literal"));
    assert!(parts.iter().any(|p| p.value == "}"));
}

#[test]
fn fstring_escape_sequences() {
    let tokens = tokenize(r#"f"line1\nline2""#);
    let parts: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::TemplateStringPart)
        .collect();
    // \n should be processed into actual newline
    assert!(parts.iter().any(|p| p.value.contains('\n')));
}

#[test]
fn fstring_single_quoted() {
    let tokens = tokenize("f'hello {name}'");
    assert_eq!(tokens[0].token_type, TokenType::TemplateStringStart);
    assert_eq!(
        tokens.last().unwrap().token_type,
        TokenType::TemplateStringEnd
    );
    let parts: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::TemplateStringPart)
        .collect();
    assert!(parts.iter().any(|p| p.value == "hello "));
    assert!(parts.iter().any(|p| p.value == "name"));
}

#[test]
fn fstring_tab_escape() {
    let tokens = tokenize(r#"f"col1\tcol2""#);
    let parts: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::TemplateStringPart)
        .collect();
    assert!(parts.iter().any(|p| p.value.contains('\t')));
}

// ==========================================================================
// Numeric edge cases
// ==========================================================================

#[test]
fn hex_uppercase() {
    let tokens = tokenize("0xFF 0XAB");
    let nums: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Number)
        .collect();
    assert_eq!(nums.len(), 2);
    assert_eq!(nums[0].value, "0xFF");
    assert_eq!(nums[1].value, "0XAB");
}

#[test]
fn hex_with_underscores() {
    let tokens = tokenize("0xFF_FF");
    let nums: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Number)
        .collect();
    assert_eq!(nums.len(), 1);
    assert_eq!(nums[0].value, "0xFF_FF");
}

#[test]
fn binary_with_underscores() {
    let tokens = tokenize("0b1010_0101");
    let nums: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Number)
        .collect();
    assert_eq!(nums.len(), 1);
    assert_eq!(nums[0].value, "0b1010_0101");
}

#[test]
fn octal_with_underscores() {
    let tokens = tokenize("0o77_77");
    let nums: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Number)
        .collect();
    assert_eq!(nums.len(), 1);
    assert_eq!(nums[0].value, "0o77_77");
}

#[test]
fn scientific_notation_with_minus_exponent() {
    let tokens = tokenize("2.5e-3");
    let nums: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Number)
        .collect();
    assert_eq!(nums.len(), 1);
    assert_eq!(nums[0].value, "2.5e-3");
}

#[test]
fn scientific_notation_with_plus_exponent() {
    let tokens = tokenize("1e+5");
    let nums: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Number)
        .collect();
    assert_eq!(nums.len(), 1);
    assert_eq!(nums[0].value, "1e+5");
}

#[test]
fn scientific_notation_uppercase_e() {
    let tokens = tokenize("3.14E10");
    let nums: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Number)
        .collect();
    assert_eq!(nums.len(), 1);
    assert_eq!(nums[0].value, "3.14E10");
}

#[test]
fn number_zero_standalone() {
    let tokens = tokenize("0");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Number);
    assert_eq!(tokens[0].value, "0");
}

#[test]
fn large_integer() {
    let tokens = tokenize("999999999999999999");
    assert_eq!(tokens[0].token_type, TokenType::Number);
    assert_eq!(tokens[0].value, "999999999999999999");
}

#[test]
fn negative_number_is_minus_plus_number() {
    let toks = types("-42");
    assert_eq!(toks, vec![TokenType::Minus, TokenType::Number]);
}

#[test]
fn float_with_many_decimal_digits() {
    let tokens = tokenize("3.141592653589793");
    assert_eq!(tokens[0].token_type, TokenType::Number);
    assert_eq!(tokens[0].value, "3.141592653589793");
}

#[test]
fn number_before_range_is_separate() {
    // "5..10" should be Number(5), Range(..), Number(10)
    let toks = non_ws("5..10");
    assert_eq!(toks.len(), 3);
    assert_eq!(toks[0], (TokenType::Number, "5".into()));
    assert_eq!(toks[1], (TokenType::Range, "..".into()));
    assert_eq!(toks[2], (TokenType::Number, "10".into()));
}

#[test]
fn float_before_range() {
    // "3.0..5" -> Number(3.0), Range(..), Number(5)
    let toks = non_ws("3.0..5");
    assert_eq!(toks[0], (TokenType::Number, "3.0".into()));
    assert_eq!(toks[1], (TokenType::Range, "..".into()));
    assert_eq!(toks[2], (TokenType::Number, "5".into()));
}

#[test]
fn binary_uppercase_b() {
    let tokens = tokenize("0B1100");
    let nums: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Number)
        .collect();
    assert_eq!(nums.len(), 1);
    assert_eq!(nums[0].value, "0B1100");
}

#[test]
fn octal_uppercase_o() {
    let tokens = tokenize("0O77");
    let nums: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Number)
        .collect();
    assert_eq!(nums.len(), 1);
    assert_eq!(nums[0].value, "0O77");
}

#[test]
fn very_large_exponent() {
    let tokens = tokenize("1e308");
    assert_eq!(tokens[0].token_type, TokenType::Number);
    assert_eq!(tokens[0].value, "1e308");
}

// ==========================================================================
// String edge cases
// ==========================================================================

#[test]
fn empty_double_quoted_string() {
    let tokens = tokenize(r#""""#);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::StringLit);
    assert_eq!(tokens[0].value, r#""""#);
}

#[test]
fn empty_single_quoted_string() {
    let tokens = tokenize("''");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::StringLit);
    assert_eq!(tokens[0].value, "''");
}

#[test]
fn string_with_spaces() {
    let toks = non_ws(r#""hello world" "foo bar""#);
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0].1, "\"hello world\"");
    assert_eq!(toks[1].1, "\"foo bar\"");
}

#[test]
fn string_with_escaped_quotes() {
    let tokens = tokenize(r#""he said \"hi\"""#);
    assert_eq!(tokens[0].token_type, TokenType::StringLit);
    assert_eq!(tokens[0].value, r#""he said \"hi\"""#);
}

#[test]
fn string_with_backslash_sequences() {
    let tokens = tokenize(r#""path\\to\\file""#);
    assert_eq!(tokens[0].token_type, TokenType::StringLit);
    assert_eq!(tokens[0].value, r#""path\\to\\file""#);
}

#[test]
fn single_quoted_string_with_escaped_quotes() {
    let tokens = tokenize(r"'it\'s'");
    assert_eq!(tokens[0].token_type, TokenType::StringLit);
    assert_eq!(tokens[0].value, r"'it\'s'");
}

#[test]
fn string_with_newline_escape() {
    // In a regular string, escapes are stored verbatim
    let tokens = tokenize(r#""line1\nline2""#);
    assert_eq!(tokens[0].token_type, TokenType::StringLit);
    assert_eq!(tokens[0].value, r#""line1\nline2""#);
}

// ==========================================================================
// Operator combinations and multi-token sequences
// ==========================================================================

#[test]
fn assignment_expression() {
    let toks = non_ws("x := 42");
    assert_eq!(toks.len(), 3);
    assert_eq!(toks[0], (TokenType::Identifier, "x".into()));
    assert_eq!(toks[1], (TokenType::Assign, ":=".into()));
    assert_eq!(toks[2], (TokenType::Number, "42".into()));
}

#[test]
fn function_call_syntax() {
    let toks = non_ws("foo(bar, baz)");
    assert_eq!(toks.len(), 6);
    assert_eq!(toks[0].0, TokenType::Identifier);
    assert_eq!(toks[1].0, TokenType::LParen);
    assert_eq!(toks[2].0, TokenType::Identifier);
    assert_eq!(toks[3].0, TokenType::Comma);
    assert_eq!(toks[4].0, TokenType::Identifier);
    assert_eq!(toks[5].0, TokenType::RParen);
}

#[test]
fn array_indexing() {
    let toks = non_ws("arr[0]");
    assert_eq!(toks.len(), 4);
    assert_eq!(toks[0].0, TokenType::Identifier);
    assert_eq!(toks[1].0, TokenType::LBracket);
    assert_eq!(toks[2].0, TokenType::Number);
    assert_eq!(toks[3].0, TokenType::RBracket);
}

#[test]
fn nested_array_indexing() {
    let vals = values("arr[1][2]");
    assert_eq!(vals, vec!["arr", "[", "1", "]", "[", "2", "]"]);
}

#[test]
fn table_syntax() {
    let vals = values("{key:value}");
    assert_eq!(vals, vec!["{", "key", ":", "value", "}"]);
}

#[test]
fn complex_arithmetic_expression() {
    let toks = non_ws("result := ((a + b) * (c - d)) / 2");
    assert_eq!(toks[0].1, "result");
    assert_eq!(toks[1].0, TokenType::Assign);
    assert_eq!(toks[2].0, TokenType::LParen);
    assert_eq!(toks[3].0, TokenType::LParen);
    assert_eq!(toks[4].1, "a");
    assert_eq!(toks[5].0, TokenType::Plus);
    assert_eq!(toks[6].1, "b");
    assert_eq!(toks[7].0, TokenType::RParen);
    assert_eq!(toks[8].0, TokenType::Times);
    assert_eq!(toks[14].0, TokenType::RParen);
    assert_eq!(toks[15].0, TokenType::Divide);
    assert_eq!(toks[16], (TokenType::Number, "2".into()));
}

#[test]
fn modulo_operator() {
    let toks = types("a % b");
    assert_eq!(
        toks,
        vec![
            TokenType::Identifier,
            TokenType::Modulo,
            TokenType::Identifier
        ]
    );
}

#[test]
fn arrow_operator() {
    let toks = types("fn foo -> i32");
    assert_eq!(
        toks,
        vec![
            TokenType::Fn,
            TokenType::Identifier,
            TokenType::Arrow,
            TokenType::TypeToken
        ]
    );
}

#[test]
fn fat_arrow_operator() {
    let toks = types("x => y");
    assert_eq!(
        toks,
        vec![
            TokenType::Identifier,
            TokenType::FatArrow,
            TokenType::Identifier
        ]
    );
}

#[test]
fn not_equal_operator() {
    let toks = types("a != b");
    assert_eq!(
        toks,
        vec![
            TokenType::Identifier,
            TokenType::NotEqual,
            TokenType::Identifier
        ]
    );
}

#[test]
fn chained_comparisons() {
    let toks = types("a < b > c <= d >= e");
    assert_eq!(
        toks,
        vec![
            TokenType::Identifier,
            TokenType::Less,
            TokenType::Identifier,
            TokenType::Greater,
            TokenType::Identifier,
            TokenType::LessEqual,
            TokenType::Identifier,
            TokenType::GreaterEqual,
            TokenType::Identifier,
        ]
    );
}

#[test]
fn shift_operators() {
    let toks = types("a << 2 >> 3");
    assert_eq!(
        toks,
        vec![
            TokenType::Identifier,
            TokenType::LShift,
            TokenType::Number,
            TokenType::RShift,
            TokenType::Number,
        ]
    );
}

#[test]
fn bitwise_xor_not() {
    let toks = types("~a ^ b");
    assert_eq!(
        toks,
        vec![
            TokenType::BitwiseNot,
            TokenType::Identifier,
            TokenType::BitwiseXor,
            TokenType::Identifier,
        ]
    );
}

#[test]
fn single_equal_is_equal() {
    // Single = is also Equal in Mog
    let tokens = tokenize("=");
    assert_eq!(tokens[0].token_type, TokenType::Equal);
    assert_eq!(tokens[0].value, "=");
}

#[test]
fn colon_vs_assign() {
    // ":" alone is Colon, ":=" is Assign
    let t1 = tokenize(":");
    assert_eq!(t1[0].token_type, TokenType::Colon);
    let t2 = tokenize(":=");
    assert_eq!(t2[0].token_type, TokenType::Assign);
}

// ==========================================================================
// Unknown characters
// ==========================================================================

#[test]
fn at_sign_is_unknown() {
    let tokens = tokenize("@");
    assert_eq!(tokens[0].token_type, TokenType::Unknown);
    assert_eq!(tokens[0].value, "@");
}

#[test]
fn dollar_sign_is_unknown() {
    let tokens = tokenize("$");
    assert_eq!(tokens[0].token_type, TokenType::Unknown);
    assert_eq!(tokens[0].value, "$");
}

#[test]
fn unknown_between_valid_tokens() {
    let toks = non_ws("foo @ bar");
    assert_eq!(toks.len(), 3);
    assert_eq!(toks[0].0, TokenType::Identifier);
    assert_eq!(toks[1].0, TokenType::Unknown);
    assert_eq!(toks[2].0, TokenType::Identifier);
}

#[test]
fn exclamation_without_equal_is_unknown() {
    // "!" alone should be Unknown (since != is handled separately)
    let tokens = tokenize("!");
    // The lexer checks "!=" first; lone "!" falls through to unknown
    assert_eq!(tokens[0].token_type, TokenType::Unknown);
}

// ==========================================================================
// Identifier edge cases
// ==========================================================================

#[test]
fn identifiers_with_numbers() {
    let ids: Vec<_> = tokenize("var1 temp123 x99")
        .into_iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0].value, "var1");
    assert_eq!(ids[1].value, "temp123");
    assert_eq!(ids[2].value, "x99");
}

#[test]
fn uppercase_identifiers() {
    let ids: Vec<_> = tokenize("MyClass CONSTANT_VALUE")
        .into_iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0].value, "MyClass");
    assert_eq!(ids[1].value, "CONSTANT_VALUE");
}

#[test]
fn single_char_identifiers() {
    let ids: Vec<_> = tokenize("a b c x y z")
        .into_iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .collect();
    assert_eq!(ids.len(), 6);
}

#[test]
fn very_long_identifier() {
    let long_id = "a".repeat(1000);
    let tokens = tokenize(&long_id);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Identifier);
    assert_eq!(tokens[0].value, long_id);
}

#[test]
fn dunder_identifiers() {
    let ids: Vec<_> = tokenize("__init__ __foo__")
        .into_iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0].value, "__init__");
    assert_eq!(ids[1].value, "__foo__");
}

#[test]
fn keyword_prefix_is_identifier() {
    // "foobar" should NOT be keyword "for" + "bar"
    let tokens = tokenize("foobar ifx return_value");
    let ids: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0].value, "foobar");
    assert_eq!(ids[1].value, "ifx");
    assert_eq!(ids[2].value, "return_value");
}

// ==========================================================================
// Type edge cases
// ==========================================================================

#[test]
fn invalid_type_is_identifier() {
    // i999, u1 should be identifiers, not types
    let tokens = tokenize("i999 u1");
    let ids: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0].value, "i999");
    assert_eq!(ids[1].value, "u1");
}

#[test]
fn all_integer_types() {
    let types_found: Vec<_> = tokenize("i8 i16 i32 i64 i128 i256")
        .into_iter()
        .filter(|t| t.token_type == TokenType::TypeToken)
        .collect();
    assert_eq!(types_found.len(), 6);
}

#[test]
fn all_unsigned_types() {
    let types_found: Vec<_> = tokenize("u8 u16 u32 u64 u128 u256")
        .into_iter()
        .filter(|t| t.token_type == TokenType::TypeToken)
        .collect();
    assert_eq!(types_found.len(), 6);
}

#[test]
fn all_float_types() {
    let types_found: Vec<_> = tokenize("f8 f16 f32 f64 f128 f256")
        .into_iter()
        .filter(|t| t.token_type == TokenType::TypeToken)
        .collect();
    assert_eq!(types_found.len(), 6);
}

#[test]
fn bool_type() {
    let tokens = tokenize("bool");
    assert_eq!(tokens[0].token_type, TokenType::TypeToken);
    assert_eq!(tokens[0].value, "bool");
}

#[test]
fn string_type() {
    let tokens = tokenize("string");
    assert_eq!(tokens[0].token_type, TokenType::TypeToken);
    assert_eq!(tokens[0].value, "string");
}

#[test]
fn ptr_type() {
    let tokens = tokenize("ptr");
    assert_eq!(tokens[0].token_type, TokenType::TypeToken);
    assert_eq!(tokens[0].value, "ptr");
}

#[test]
fn int_type() {
    let tokens = tokenize("int");
    assert_eq!(tokens[0].token_type, TokenType::TypeToken);
    assert_eq!(tokens[0].value, "int");
}

// ==========================================================================
// Position tracking edge cases
// ==========================================================================

#[test]
fn position_multiline_program() {
    let tokens = tokenize("{\n  x := 42\n}");
    let lbrace = tokens
        .iter()
        .find(|t| t.token_type == TokenType::LBrace)
        .unwrap();
    let rbrace = tokens
        .iter()
        .find(|t| t.token_type == TokenType::RBrace)
        .unwrap();
    assert_eq!(lbrace.span.start.line, 1);
    assert_eq!(lbrace.span.start.column, 1);
    assert_eq!(rbrace.span.start.line, 3);
    assert_eq!(rbrace.span.start.column, 1);
}

#[test]
fn position_after_multiple_newlines() {
    let tokens = tokenize("line1\n\nline3");
    let ids: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .collect();
    assert_eq!(ids[0].span.start.line, 1);
    assert_eq!(ids[1].span.start.line, 3);
}

#[test]
fn position_column_tracking() {
    let tokens = tokenize("  foo  bar");
    let ids: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .collect();
    assert_eq!(ids[0].span.start.column, 3);
    assert_eq!(ids[1].span.start.column, 8);
}

#[test]
fn position_index_tracking() {
    let tokens = tokenize("abc def");
    let ids: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .collect();
    assert_eq!(ids[0].span.start.index, 0);
    assert_eq!(ids[0].span.end.index, 3);
    assert_eq!(ids[1].span.start.index, 4);
    assert_eq!(ids[1].span.end.index, 7);
}

// ==========================================================================
// Multi-line programs
// ==========================================================================

#[test]
fn multiline_program_structure() {
    let code = "{\n  x := 42\n  result := x * 2\n}";
    let tokens = tokenize(code);
    assert!(tokens
        .iter()
        .any(|t| t.token_type == TokenType::LBrace && t.span.start.line == 1));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Identifier
        && t.value == "x"
        && t.span.start.line == 2));
    assert!(tokens
        .iter()
        .any(|t| t.token_type == TokenType::RBrace && t.span.start.line == 4));
}

#[test]
fn comments_in_multiline_code() {
    let tokens = tokenize("x := 5 # this is a comment\ny := 10");
    let comment = tokens
        .iter()
        .find(|t| t.token_type == TokenType::Comment)
        .unwrap();
    assert_eq!(comment.value, "# this is a comment");
}

#[test]
fn line_numbers_across_three_lines() {
    let ids: Vec<_> = tokenize("line1\nline2\nline3")
        .into_iter()
        .filter(|t| t.token_type == TokenType::Identifier)
        .collect();
    assert_eq!(ids[0].span.start.line, 1);
    assert_eq!(ids[1].span.start.line, 2);
    assert_eq!(ids[2].span.start.line, 3);
}

// ==========================================================================
// Whitespace handling
// ==========================================================================

#[test]
fn only_whitespace_input() {
    let tokens = tokenize("   \t\n   ");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Whitespace);
}

#[test]
fn tabs_only() {
    let tokens = tokenize("\t\t");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Whitespace);
    assert_eq!(tokens[0].value, "\t\t");
}

#[test]
fn newlines_only() {
    let tokens = tokenize("\n\n");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Whitespace);
    assert_eq!(tokens[0].value, "\n\n");
}

#[test]
fn mixed_whitespace_collapsed() {
    let tokens = tokenize("  \t\n  ");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_type, TokenType::Whitespace);
    assert_eq!(tokens[0].value, "  \t\n  ");
}

// ==========================================================================
// Keyword-specific tests not already covered
// ==========================================================================

#[test]
fn keyword_true_false() {
    let toks = types("true false");
    assert_eq!(toks, vec![TokenType::True, TokenType::False]);
}

#[test]
fn keyword_async_await_spawn() {
    let toks = types("async await spawn");
    assert_eq!(
        toks,
        vec![TokenType::Async, TokenType::Await, TokenType::Spawn]
    );
}

#[test]
fn keyword_package_import_pub() {
    let toks = types("package import pub");
    assert_eq!(
        toks,
        vec![TokenType::Package, TokenType::Import, TokenType::Pub]
    );
}

#[test]
fn keyword_try_catch() {
    let toks = types("try catch");
    assert_eq!(toks, vec![TokenType::Try, TokenType::Catch]);
}

#[test]
fn keyword_ok_err_some_none() {
    let toks = types("ok err some none");
    assert_eq!(
        toks,
        vec![
            TokenType::Ok,
            TokenType::Err,
            TokenType::Some,
            TokenType::None
        ]
    );
}

#[test]
fn keyword_match_is() {
    let toks = types("match is");
    assert_eq!(toks, vec![TokenType::Match, TokenType::Is]);
}

#[test]
fn keyword_struct_soa() {
    let toks = types("struct soa");
    assert_eq!(toks, vec![TokenType::Struct, TokenType::Soa]);
}

#[test]
fn keyword_with() {
    let toks = types("with");
    assert_eq!(toks, vec![TokenType::With]);
}

// ==========================================================================
// Complex real-world-like expressions
// ==========================================================================

#[test]
fn function_definition_tokens() {
    let toks = types("fn add(a: i32, b: i32) -> i32");
    assert_eq!(
        toks,
        vec![
            TokenType::Fn,
            TokenType::Identifier, // add
            TokenType::LParen,
            TokenType::Identifier, // a
            TokenType::Colon,
            TokenType::TypeToken, // i32
            TokenType::Comma,
            TokenType::Identifier, // b
            TokenType::Colon,
            TokenType::TypeToken, // i32
            TokenType::RParen,
            TokenType::Arrow,
            TokenType::TypeToken, // i32
        ]
    );
}

#[test]
fn if_else_expression() {
    let toks = types("if x > 0 { return x } else { return 0 }");
    assert_eq!(toks[0], TokenType::If);
    assert_eq!(toks[1], TokenType::Identifier); // x
    assert_eq!(toks[2], TokenType::Greater);
    assert_eq!(toks[3], TokenType::Number);
    assert_eq!(toks[4], TokenType::LBrace);
    assert_eq!(toks[5], TokenType::Return);
}

#[test]
fn for_loop_with_range() {
    let toks = types("for i in 0..10");
    assert_eq!(
        toks,
        vec![
            TokenType::For,
            TokenType::Identifier,
            TokenType::In,
            TokenType::Number,
            TokenType::Range,
            TokenType::Number,
        ]
    );
}

#[test]
fn struct_definition() {
    let toks = types("struct Point { x: f64, y: f64 }");
    assert_eq!(toks[0], TokenType::Struct);
    assert_eq!(toks[1], TokenType::Identifier); // Point
    assert_eq!(toks[2], TokenType::LBrace);
    assert_eq!(toks[3], TokenType::Identifier); // x
    assert_eq!(toks[4], TokenType::Colon);
    assert_eq!(toks[5], TokenType::TypeToken); // f64
}

#[test]
fn match_expression() {
    let toks = types("match x { 1 => a, 2 => b }");
    assert_eq!(toks[0], TokenType::Match);
    assert_eq!(toks[1], TokenType::Identifier);
    assert_eq!(toks[2], TokenType::LBrace);
    assert_eq!(toks[3], TokenType::Number);
    assert_eq!(toks[4], TokenType::FatArrow);
    assert_eq!(toks[5], TokenType::Identifier);
}

#[test]
fn logical_and_or_expression() {
    let toks = types("a && b || c");
    assert_eq!(
        toks,
        vec![
            TokenType::Identifier,
            TokenType::LogicalAnd,
            TokenType::Identifier,
            TokenType::LogicalOr,
            TokenType::Identifier,
        ]
    );
}

#[test]
fn cast_expression() {
    let toks = types("cast x as i32");
    assert_eq!(
        toks,
        vec![
            TokenType::Cast,
            TokenType::Identifier,
            TokenType::As,
            TokenType::TypeToken
        ]
    );
}

#[test]
fn dot_access_chain() {
    let toks = types("obj.field.method");
    assert_eq!(
        toks,
        vec![
            TokenType::Identifier,
            TokenType::Dot,
            TokenType::Identifier,
            TokenType::Dot,
            TokenType::Identifier,
        ]
    );
}

#[test]
fn semicolon_separated_statements() {
    let toks = types("x := 1; y := 2; z := 3");
    // Check semicolons are present
    let semis: Vec<_> = toks
        .iter()
        .filter(|t| **t == TokenType::Semicolon)
        .collect();
    assert_eq!(semis.len(), 2);
}

#[test]
fn question_mark_in_expression() {
    let toks = types("result?");
    assert_eq!(toks, vec![TokenType::Identifier, TokenType::QuestionMark]);
}

#[test]
fn underscore_in_pattern() {
    let toks = types("match x { _ => default }");
    assert!(toks.contains(&TokenType::Underscore));
}
