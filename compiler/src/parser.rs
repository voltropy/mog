//! Mog recursive-descent parser.
//!
//! Faithfully ported from the TypeScript reference implementation
//! (`src/parser.ts`).  The grammar is described informally in the TS
//! source; this module produces the same AST shape defined in
//! `crate::ast`.

use crate::ast::*;
use crate::lexer::tokenize;
use crate::token::{Token, TokenType};
use crate::types::{FloatKind, IntegerKind, Type, UnsignedKind};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Parse a stream of tokens into a `Program` statement.
pub fn parse_tokens(tokens: &[Token]) -> Statement {
    // Filter whitespace and comments before parsing (matches TS compiler pipeline)
    let filtered: Vec<Token> = tokens
        .iter()
        .filter(|t| t.token_type != TokenType::Whitespace && t.token_type != TokenType::Comment)
        .cloned()
        .collect();
    let mut parser = Parser::new(&filtered);
    parser.parse_program()
}

/// Convenience alias used by other modules.
pub fn parse(tokens: &[Token]) -> Statement {
    parse_tokens(tokens)
}

// ---------------------------------------------------------------------------
// Escape-sequence helper (mirrors `decodeEscapeSequences` in TS)
// ---------------------------------------------------------------------------

fn decode_escape_sequences(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('\'') => out.push('\''),
                Some('x') => {
                    let mut hex = String::new();
                    for _ in 0..2 {
                        if let Some(&ch) = chars.peek() {
                            if ch.is_ascii_hexdigit() {
                                hex.push(ch);
                                chars.next();
                            }
                        }
                    }
                    if hex.len() == 2 {
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            out.push(byte as char);
                        }
                    } else {
                        out.push('\\');
                        out.push('x');
                        out.push_str(&hex);
                    }
                }
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Associativity sets (mirrors the TS statics)
// ---------------------------------------------------------------------------

fn is_associative_op(op: &str) -> bool {
    matches!(op, "+" | "*" | "&&" | "||" | "&" | "|")
}

fn is_binary_op_token(tt: TokenType, value: &str) -> bool {
    matches!(
        tt,
        TokenType::Plus
            | TokenType::Minus
            | TokenType::Times
            | TokenType::Divide
            | TokenType::Modulo
            | TokenType::LogicalAnd
            | TokenType::LogicalOr
            | TokenType::BitwiseAnd
            | TokenType::BitwiseOr
            | TokenType::BitwiseXor
            | TokenType::LShift
            | TokenType::RShift
            | TokenType::Less
            | TokenType::LessEqual
            | TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::NotEqual
    ) || (tt == TokenType::Equal && value == "==")
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    scope_counter: u32,
    allow_struct_literal: bool,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            scope_counter: 0,
            allow_struct_literal: true,
        }
    }

    fn next_scope_id(&mut self) -> String {
        self.scope_counter += 1;
        format!("scope_{}", self.scope_counter)
    }

    // -----------------------------------------------------------------------
    // Token helpers
    // -----------------------------------------------------------------------

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_next(&self) -> Option<&Token> {
        if self.pos + 1 < self.tokens.len() {
            Some(&self.tokens[self.pos + 1])
        } else {
            None
        }
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.pos - 1]
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        self.pos += 1;
        tok
    }

    fn check(&self, tt: TokenType) -> bool {
        !self.is_at_end() && self.peek().token_type == tt
    }

    fn match_token(&mut self, tt: TokenType) -> bool {
        if self.check(tt) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, tt: TokenType, msg: &str) -> &Token {
        if self.check(tt) {
            self.advance();
            return self.previous();
        }
        let tok = if self.is_at_end() {
            self.tokens.last().unwrap()
        } else {
            self.peek()
        };
        panic!("{} at line {}", msg, tok.span.start.line);
    }

    fn last_position(&self) -> Position {
        if let Some(last) = self.tokens.last() {
            last.span.end.into()
        } else {
            Position {
                line: 1,
                column: 1,
                index: 0,
            }
        }
    }

    /// Get the span of the previous token, already converted to `ast::Span`.
    fn prev_span(&self) -> Span {
        self.previous().span.into()
    }

    fn combine_spans(&self, a: impl Into<Span>, b: impl Into<Span>) -> Span {
        let a = a.into();
        let b = b.into();
        Span {
            start: a.start,
            end: b.end,
        }
    }

    // -----------------------------------------------------------------------
    // Program (top-level)
    // -----------------------------------------------------------------------

    fn parse_program(&mut self) -> Statement {
        let mut stmts: Vec<Statement> = Vec::new();

        while !self.is_at_end() {
            if let Some(s) = self.parse_statement() {
                stmts.push(s);
            }
        }

        // Unwrap single-block pattern (mirrors TS getUnwrappedStatements)
        stmts = unwrap_statements(stmts);

        let scope_id = self.next_scope_id();
        Statement {
            kind: StatementKind::Program {
                statements: stmts,
                scope_id,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Statement dispatch
    // -----------------------------------------------------------------------

    fn parse_statement(&mut self) -> Option<Statement> {
        // Skip stray semicolons
        while self.match_token(TokenType::Semicolon) {}

        if self.check(TokenType::RBrace) || self.is_at_end() {
            return None;
        }

        // Package declaration
        if self.match_token(TokenType::Package) {
            return Some(self.parse_package_declaration());
        }

        // Import declaration
        if self.match_token(TokenType::Import) {
            return Some(self.parse_import_declaration());
        }

        // pub modifier
        if self.check(TokenType::Pub) {
            return Some(self.parse_pub_declaration());
        }

        // requires / optional
        if self.match_token(TokenType::Requires) {
            return Some(self.parse_requires_declaration());
        }
        if self.match_token(TokenType::Optional) {
            return Some(self.parse_optional_declaration());
        }

        // Block statement
        if self.match_token(TokenType::LBrace) {
            return Some(self.parse_block_statement());
        }

        // async fn
        if self.check(TokenType::Async)
            && self.peek_next().map(|t| t.token_type) == Some(TokenType::Fn)
        {
            self.advance(); // consume async
            self.advance(); // consume fn
            return Some(self.parse_async_function_declaration(false));
        }

        // fn name(...)
        if self.check(TokenType::Fn)
            && self.peek_next().map(|t| t.token_type) == Some(TokenType::Identifier)
        {
            self.advance(); // consume fn
            return Some(self.parse_function_declaration(false));
        }

        // struct
        if self.match_token(TokenType::Struct) {
            return Some(self.parse_struct_definition(false));
        }

        // type alias
        if self.match_token(TokenType::TypeKw) {
            return Some(self.parse_type_alias_declaration(false));
        }

        // return
        if self.match_token(TokenType::Return) {
            return Some(self.parse_return_statement());
        }

        // if
        if self.match_token(TokenType::If) {
            return Some(self.parse_if_statement());
        }

        // while
        if self.match_token(TokenType::While) {
            return Some(self.parse_while_loop());
        }

        // for
        if self.match_token(TokenType::For) {
            return Some(self.parse_for_loop());
        }

        // try/catch
        if self.match_token(TokenType::Try) {
            return Some(self.parse_try_catch());
        }

        // with
        if self.match_token(TokenType::With) {
            return Some(self.parse_with_statement());
        }

        // break
        if self.match_token(TokenType::Break) {
            self.match_token(TokenType::Semicolon);
            return Some(Statement {
                span: Span {
                    start: Position {
                        line: 1,
                        column: 1,
                        index: 0,
                    },
                    end: self.last_position(),
                },
                kind: StatementKind::Break,
            });
        }

        // continue
        if self.match_token(TokenType::Continue) {
            self.match_token(TokenType::Semicolon);
            return Some(Statement {
                span: Span {
                    start: Position {
                        line: 1,
                        column: 1,
                        index: 0,
                    },
                    end: self.last_position(),
                },
                kind: StatementKind::Continue,
            });
        }

        // Variable declaration:  IDENTIFIER : type ... or TYPE ...
        // Checking for:  IDENTIFIER COLON  (name: type = ...)
        if self.check(TokenType::Identifier)
            && self.peek_next().map(|t| t.token_type) == Some(TokenType::Colon)
        {
            return Some(self.parse_variable_declaration());
        }
        // TYPE token can also start a variable declaration in some forms
        if self.check(TokenType::TypeToken) {
            return Some(self.parse_variable_declaration());
        }

        // Expression statement
        let expr = self.parse_expression();
        let span = expr.span;
        self.match_token(TokenType::Semicolon);
        Some(Statement {
            kind: StatementKind::ExpressionStatement {
                expression: Box::new(expr),
            },
            span,
        })
    }

    // -----------------------------------------------------------------------
    // Declarations
    // -----------------------------------------------------------------------

    fn parse_package_declaration(&mut self) -> Statement {
        let name = self
            .consume(TokenType::Identifier, "Expected package name")
            .value
            .clone();
        self.match_token(TokenType::Semicolon);
        Statement {
            kind: StatementKind::PackageDeclaration { name },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    fn parse_import_declaration(&mut self) -> Statement {
        let mut paths: Vec<String> = Vec::new();

        if self.match_token(TokenType::LParen) {
            // Grouped import: import ( "path1" "path2" )
            while !self.check(TokenType::RParen) && !self.is_at_end() {
                let path_tok = self.consume(TokenType::StringLit, "Expected import path string");
                let raw = &path_tok.value;
                let path_value = raw[1..raw.len() - 1].to_string();
                paths.push(path_value);
                // Allow optional semicolons or commas between imports
                if !self.match_token(TokenType::Semicolon) {
                    self.match_token(TokenType::Comma);
                }
            }
            self.consume(TokenType::RParen, "Expected ) after grouped imports");
        } else {
            // Single import: import "path"
            let path_tok = self.consume(TokenType::StringLit, "Expected import path string");
            let raw = &path_tok.value;
            let path_value = raw[1..raw.len() - 1].to_string();
            paths.push(path_value);
        }

        self.match_token(TokenType::Semicolon);

        Statement {
            kind: StatementKind::ImportDeclaration { paths },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    fn parse_pub_declaration(&mut self) -> Statement {
        self.advance(); // consume 'pub'

        // pub fn
        if self.check(TokenType::Fn)
            && self.peek_next().map(|t| t.token_type) == Some(TokenType::Identifier)
        {
            self.advance(); // consume fn
            return self.parse_function_declaration(true);
        }

        // pub async fn
        if self.check(TokenType::Async)
            && self.peek_next().map(|t| t.token_type) == Some(TokenType::Fn)
        {
            self.advance(); // consume async
            self.advance(); // consume fn
            return self.parse_async_function_declaration(true);
        }

        // pub struct
        if self.match_token(TokenType::Struct) {
            return self.parse_struct_definition(true);
        }

        // pub type
        if self.match_token(TokenType::TypeKw) {
            return self.parse_type_alias_declaration(true);
        }

        panic!(
            "Expected fn, struct, or type after pub at line {}",
            self.peek().span.start.line
        );
    }

    fn parse_function_declaration(&mut self, is_public: bool) -> Statement {
        let name = self
            .consume(TokenType::Identifier, "Expected function name")
            .value
            .clone();
        self.consume(TokenType::LParen, "Expected ( after function name");
        let params = self.parse_function_params();
        self.consume(TokenType::RParen, "Expected ) after parameters");

        let return_type = if self.match_token(TokenType::Arrow) {
            self.parse_return_type()
        } else {
            Type::Void
        };

        let body = self.parse_function_body();

        Statement {
            kind: StatementKind::FunctionDeclaration {
                name,
                params,
                return_type,
                body,
                is_public,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    fn parse_async_function_declaration(&mut self, is_public: bool) -> Statement {
        let name = self
            .consume(TokenType::Identifier, "Expected function name")
            .value
            .clone();
        self.consume(TokenType::LParen, "Expected ( after function name");
        let params = self.parse_function_params();
        self.consume(TokenType::RParen, "Expected ) after parameters");

        let return_type = if self.match_token(TokenType::Arrow) {
            self.parse_return_type()
        } else {
            Type::Void
        };

        let body = self.parse_function_body();

        Statement {
            kind: StatementKind::AsyncFunctionDeclaration {
                name,
                params,
                return_type,
                body,
                is_public,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    fn parse_struct_definition(&mut self, is_public: bool) -> Statement {
        let name = self
            .consume(TokenType::Identifier, "Expected struct name")
            .value
            .clone();
        self.consume(TokenType::LBrace, "Expected { after struct name");

        let mut fields: Vec<FieldDef> = Vec::new();
        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            let field_name = self
                .consume(TokenType::Identifier, "Expected field name")
                .value
                .clone();
            self.consume(TokenType::Colon, "Expected : after field name");

            let field_type = if self.check(TokenType::LBracket) {
                let type_name = self.parse_array_type_annotation();
                self.resolve_type_name(&type_name)
            } else if self.check(TokenType::TypeToken) {
                let type_name = self
                    .consume(TokenType::TypeToken, "Expected type after :")
                    .value
                    .clone();
                self.resolve_type_name(&type_name)
            } else {
                let type_name = self
                    .consume(TokenType::Identifier, "Expected type after :")
                    .value
                    .clone();
                self.resolve_type_name(&type_name)
            };

            fields.push(FieldDef {
                name: field_name,
                field_type,
            });
            if !self.match_token(TokenType::Comma) {
                self.match_token(TokenType::Semicolon);
            }
        }

        self.consume(TokenType::RBrace, "Expected } after struct fields");

        Statement {
            kind: StatementKind::StructDefinition {
                name,
                fields,
                is_public,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Variable declaration
    // -----------------------------------------------------------------------

    fn parse_variable_declaration(&mut self) -> Statement {
        let name = self
            .consume(TokenType::Identifier, "Expected variable name")
            .value
            .clone();
        self.consume(TokenType::Colon, "Expected : after variable name");

        let var_type = self.parse_type_annotation();

        // Handle both `:=` and `=` assignment operators
        if self.match_token(TokenType::Assign) {
            // Walrus operator :=
        } else {
            self.consume(TokenType::Equal, "Expected = or := after type");
            // In the TS parser, if `=` is seen after type, it also tries to consume `:=`
            self.match_token(TokenType::Assign);
        }

        let initializer = self.parse_expression();

        self.consume(
            TokenType::Semicolon,
            "Expected ; after variable declaration",
        );

        Statement {
            kind: StatementKind::VariableDeclaration {
                name,
                var_type: Some(var_type),
                value: Some(Box::new(initializer)),
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    /// Parse a type annotation in variable-declaration position.
    ///
    /// Handles: fn(...)->T, tensor<T>, [T], [T; N], ?T, soa Struct[N],
    /// Result<T>, primitive types, custom names, trailing `[]`.
    fn parse_type_annotation(&mut self) -> Type {
        // fn(...)  -> T
        if self.check(TokenType::Fn) {
            return self.parse_function_type_annotation();
        }

        // tensor<T>
        if self.check(TokenType::Tensor) {
            return self.parse_tensor_type_annotation();
        }

        // [T] or [T; N] or [[T; N]; M]
        if self.check(TokenType::LBracket) {
            let type_name = self.parse_array_type_annotation();
            return self.resolve_type_name(&type_name);
        }

        // soa Struct[N]
        if self.check(TokenType::Soa) {
            self.advance(); // consume soa
            let struct_name = self
                .consume(TokenType::Identifier, "Expected struct name after soa")
                .value
                .clone();
            self.consume(TokenType::LBracket, "Expected [ after struct name");
            let capacity = if self.check(TokenType::Number) {
                let v = self
                    .consume(TokenType::Number, "Expected capacity")
                    .value
                    .clone();
                Some(v.parse::<usize>().unwrap())
            } else {
                None
            };
            self.consume(TokenType::RBracket, "Expected ] after capacity");
            return Type::SOA {
                struct_type: Box::new(Type::Custom(struct_name)),
                capacity,
            };
        }

        // ?T (optional)
        if self.check(TokenType::QuestionMark) {
            self.advance(); // consume ?
            let inner = self.parse_return_type();
            return Type::Optional(Box::new(inner));
        }

        // TYPE token (primitive)
        if self.check(TokenType::TypeToken) {
            let mut type_name = self
                .consume(TokenType::TypeToken, "Expected type annotation")
                .value
                .clone();
            while self.match_token(TokenType::LBracket) {
                type_name.push_str("[]");
                self.consume(TokenType::RBracket, "Expected ] after array bracket");
            }
            return self.resolve_type_name(&type_name);
        }

        // IDENTIFIER (custom type, or Result<T>)
        if self.check(TokenType::Identifier) {
            let mut type_name = self
                .consume(TokenType::Identifier, "Expected type annotation")
                .value
                .clone();

            // Result<T>
            if type_name == "Result" && self.match_token(TokenType::Less) {
                let inner = self.parse_return_type();
                self.consume(TokenType::Greater, "Expected > after Result<T>");
                return Type::Result(Box::new(inner));
            }

            // trailing []
            while self.match_token(TokenType::LBracket) {
                type_name.push_str("[]");
                self.consume(TokenType::RBracket, "Expected ] after array bracket");
            }
            return self.resolve_type_name(&type_name);
        }

        panic!("Expected type annotation");
    }

    // -----------------------------------------------------------------------
    // Type alias
    // -----------------------------------------------------------------------

    fn parse_type_alias_declaration(&mut self, is_public: bool) -> Statement {
        let _ = is_public; // stored on the type alias via the AST if needed
        let name = self
            .consume(TokenType::Identifier, "Expected type alias name")
            .value
            .clone();
        self.consume(TokenType::Equal, "Expected = after type alias name");

        // Map type: type Config = {string: string};
        if self.check(TokenType::LBrace) {
            self.advance(); // consume {
            let key_type_name = if self.check(TokenType::TypeToken) {
                self.consume(TokenType::TypeToken, "Expected key type")
                    .value
                    .clone()
            } else {
                self.consume(TokenType::Identifier, "Expected key type")
                    .value
                    .clone()
            };
            self.consume(TokenType::Colon, "Expected : after key type");
            let value_type_name = if self.check(TokenType::TypeToken) {
                self.consume(TokenType::TypeToken, "Expected value type")
                    .value
                    .clone()
            } else {
                self.consume(TokenType::Identifier, "Expected value type")
                    .value
                    .clone()
            };
            self.consume(TokenType::RBrace, "Expected } after map type");
            self.match_token(TokenType::Semicolon);

            let key_type = self.resolve_type_name(&key_type_name);
            let value_type = self.resolve_type_name(&value_type_name);
            let map_type = Type::Map {
                key_type: Box::new(key_type),
                value_type: Box::new(value_type),
            };

            return Statement {
                kind: StatementKind::TypeAliasDeclaration {
                    name: name.clone(),
                    aliased_type: Type::TypeAlias {
                        name,
                        aliased_type: Box::new(map_type),
                    },
                },
                span: Span {
                    start: Position {
                        line: 1,
                        column: 1,
                        index: 0,
                    },
                    end: self.last_position(),
                },
            };
        }

        // fn(...) -> T
        if self.check(TokenType::Fn) {
            let func_type = self.parse_function_type_annotation();
            self.match_token(TokenType::Semicolon);
            return Statement {
                kind: StatementKind::TypeAliasDeclaration {
                    name: name.clone(),
                    aliased_type: Type::TypeAlias {
                        name,
                        aliased_type: Box::new(func_type),
                    },
                },
                span: Span {
                    start: Position {
                        line: 1,
                        column: 1,
                        index: 0,
                    },
                    end: self.last_position(),
                },
            };
        }

        let type_name = if self.check(TokenType::LBracket) {
            self.parse_array_type_annotation()
        } else if self.check(TokenType::TypeToken) {
            self.consume(TokenType::TypeToken, "Expected type")
                .value
                .clone()
        } else {
            self.consume(TokenType::Identifier, "Expected type")
                .value
                .clone()
        };

        let mut full_name = type_name;
        while self.match_token(TokenType::LBracket) {
            full_name.push_str("[]");
            self.consume(TokenType::RBracket, "Expected ] after array bracket");
        }
        self.match_token(TokenType::Semicolon);

        let aliased = self.resolve_type_name(&full_name);

        Statement {
            kind: StatementKind::TypeAliasDeclaration {
                name: name.clone(),
                aliased_type: Type::TypeAlias {
                    name,
                    aliased_type: Box::new(aliased),
                },
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Requires / Optional
    // -----------------------------------------------------------------------

    fn parse_requires_declaration(&mut self) -> Statement {
        let mut capabilities = Vec::new();
        capabilities.push(
            self.consume(TokenType::Identifier, "Expected capability name")
                .value
                .clone(),
        );
        while self.match_token(TokenType::Comma) {
            capabilities.push(
                self.consume(TokenType::Identifier, "Expected capability name")
                    .value
                    .clone(),
            );
        }
        self.match_token(TokenType::Semicolon);
        Statement {
            kind: StatementKind::RequiresDeclaration { capabilities },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    fn parse_optional_declaration(&mut self) -> Statement {
        let mut capabilities = Vec::new();
        capabilities.push(
            self.consume(TokenType::Identifier, "Expected capability name")
                .value
                .clone(),
        );
        while self.match_token(TokenType::Comma) {
            capabilities.push(
                self.consume(TokenType::Identifier, "Expected capability name")
                    .value
                    .clone(),
            );
        }
        self.match_token(TokenType::Semicolon);
        Statement {
            kind: StatementKind::OptionalDeclaration { capabilities },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Return
    // -----------------------------------------------------------------------

    fn parse_return_statement(&mut self) -> Statement {
        let value = if !self.check(TokenType::Semicolon) {
            Some(Box::new(self.parse_expression()))
        } else {
            None
        };
        self.consume(TokenType::Semicolon, "Expected ; after return");
        Statement {
            kind: StatementKind::Return { value },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // If statement
    // -----------------------------------------------------------------------

    fn parse_if_statement(&mut self) -> Statement {
        let condition = if self.match_token(TokenType::LParen) {
            let cond = self.parse_expression();
            self.consume(TokenType::RParen, "Expected ) after condition");
            cond
        } else {
            let prev = self.allow_struct_literal;
            self.allow_struct_literal = false;
            let cond = self.parse_expression();
            self.allow_struct_literal = prev;
            cond
        };

        self.consume(TokenType::LBrace, "Expected { after if condition");
        let true_branch = self.parse_block_body("Expected } after if body");

        let false_branch = if self.match_token(TokenType::Else) {
            if self.check(TokenType::If) {
                self.advance(); // consume 'if'
                let nested = self.parse_if_statement();
                Some(Block {
                    statements: vec![nested],
                    scope_id: self.next_scope_id(),
                    span: Span {
                        start: Position {
                            line: 1,
                            column: 1,
                            index: 0,
                        },
                        end: self.last_position(),
                    },
                })
            } else {
                self.consume(TokenType::LBrace, "Expected { after else");
                Some(self.parse_block_body("Expected } after else body"))
            }
        } else {
            None
        };

        Statement {
            kind: StatementKind::Conditional {
                condition: Box::new(condition),
                true_branch,
                false_branch,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // While loop
    // -----------------------------------------------------------------------

    fn parse_while_loop(&mut self) -> Statement {
        let has_parens = self.match_token(TokenType::LParen);
        let prev = self.allow_struct_literal;
        self.allow_struct_literal = false;
        let test = self.parse_expression();
        self.allow_struct_literal = prev;
        if has_parens {
            self.consume(TokenType::RParen, "Expected ) after while condition");
        }

        self.consume(TokenType::LBrace, "Expected { after while condition");
        let body = self.parse_block_body("Expected } after while loop");

        Statement {
            kind: StatementKind::WhileLoop {
                test: Box::new(test),
                body,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // For loop (all variants)
    // -----------------------------------------------------------------------

    fn parse_for_loop(&mut self) -> Statement {
        let variable = self
            .consume(TokenType::Identifier, "Expected variable name after for")
            .value
            .clone();

        // for item: type in arr { ... }
        if self.check(TokenType::Colon) {
            self.consume(TokenType::Colon, "Expected : after variable name");
            let mut type_name = self
                .consume(TokenType::TypeToken, "Expected type after :")
                .value
                .clone();
            while self.match_token(TokenType::LBracket) {
                type_name.push('[');
                self.consume(TokenType::RBracket, "Expected ] after array bracket");
                type_name.push(']');
            }
            let var_type = self.resolve_type_name(&type_name);
            self.consume(TokenType::In, "Expected 'in' after type annotation");
            let prev = self.allow_struct_literal;
            self.allow_struct_literal = false;
            let array = self.parse_expression();
            self.allow_struct_literal = prev;
            let body = self.parse_for_body();
            return Statement {
                kind: StatementKind::ForEachLoop {
                    variable,
                    var_type: Some(var_type),
                    array: Box::new(array),
                    body,
                },
                span: Span {
                    start: Position {
                        line: 1,
                        column: 1,
                        index: 0,
                    },
                    end: self.last_position(),
                },
            };
        }

        // for i, item in ... (index+value or map key+value)
        if self.match_token(TokenType::Comma) {
            let second_var = self
                .consume(
                    TokenType::Identifier,
                    "Expected second variable name after comma",
                )
                .value
                .clone();
            self.consume(TokenType::In, "Expected 'in' after variable names");
            let prev = self.allow_struct_literal;
            self.allow_struct_literal = false;
            let iterable = self.parse_expression();
            self.allow_struct_literal = prev;
            let body = self.parse_for_body();
            return Statement {
                kind: StatementKind::ForInIndex {
                    index_variable: variable,
                    value_variable: second_var,
                    iterable: Box::new(iterable),
                    body,
                },
                span: Span {
                    start: Position {
                        line: 1,
                        column: 1,
                        index: 0,
                    },
                    end: self.last_position(),
                },
            };
        }

        // for item in ... (foreach without type annotation, or range)
        if self.match_token(TokenType::In) {
            let prev = self.allow_struct_literal;
            self.allow_struct_literal = false;
            let iterable_expr = self.parse_expression();
            self.allow_struct_literal = prev;

            // for i in start..end
            if self.match_token(TokenType::Range) {
                let prev2 = self.allow_struct_literal;
                self.allow_struct_literal = false;
                let end_expr = self.parse_expression();
                self.allow_struct_literal = prev2;
                let body = self.parse_for_body();
                return Statement {
                    kind: StatementKind::ForInRange {
                        variable,
                        start: Box::new(iterable_expr),
                        end: Box::new(end_expr),
                        body,
                    },
                    span: Span {
                        start: Position {
                            line: 1,
                            column: 1,
                            index: 0,
                        },
                        end: self.last_position(),
                    },
                };
            }

            // for item in array
            let body = self.parse_for_body();
            return Statement {
                kind: StatementKind::ForEachLoop {
                    variable,
                    var_type: None,
                    array: Box::new(iterable_expr),
                    body,
                },
                span: Span {
                    start: Position {
                        line: 1,
                        column: 1,
                        index: 0,
                    },
                    end: self.last_position(),
                },
            };
        }

        // Traditional for loop: for i := start to end { ... }
        self.consume(TokenType::Assign, "Expected := after variable name");
        let start_expr = self.parse_expression();
        self.consume(TokenType::To, "Expected to after start value");
        let end_expr = self.parse_expression();
        let body = self.parse_for_body();

        Statement {
            kind: StatementKind::ForLoop {
                variable,
                start: Box::new(start_expr),
                end: Box::new(end_expr),
                body,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    fn parse_for_body(&mut self) -> Block {
        self.consume(TokenType::LBrace, "Expected { after for header");
        self.parse_block_body("Expected } after for loop")
    }

    // -----------------------------------------------------------------------
    // Try / catch
    // -----------------------------------------------------------------------

    fn parse_try_catch(&mut self) -> Statement {
        self.consume(TokenType::LBrace, "Expected { after try");
        let try_body = self.parse_block_body("Expected } after try body");

        self.consume(TokenType::Catch, "Expected catch after try block");
        self.consume(TokenType::LParen, "Expected ( after catch");
        let error_var = self
            .consume(TokenType::Identifier, "Expected error variable name")
            .value
            .clone();
        self.consume(TokenType::RParen, "Expected ) after catch variable");

        self.consume(TokenType::LBrace, "Expected { after catch(...)");
        let catch_body = self.parse_block_body("Expected } after catch body");

        Statement {
            kind: StatementKind::TryCatch {
                try_body,
                error_var,
                catch_body,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // With statement
    // -----------------------------------------------------------------------

    fn parse_with_statement(&mut self) -> Statement {
        let prev = self.allow_struct_literal;
        self.allow_struct_literal = false;
        let context = self.parse_expression();
        self.allow_struct_literal = prev;

        self.consume(TokenType::LBrace, "Expected { after with expression");
        let body = self.parse_block_body("Expected } after with body");

        Statement {
            kind: StatementKind::WithBlock {
                context: Box::new(context),
                body,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Block helpers
    // -----------------------------------------------------------------------

    fn parse_block_statement(&mut self) -> Statement {
        let mut stmts = Vec::new();
        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            if let Some(s) = self.parse_statement() {
                stmts.push(s);
            }
        }
        self.consume(TokenType::RBrace, "Expected } after block");
        let scope_id = self.next_scope_id();
        Statement {
            kind: StatementKind::Block {
                statements: stmts,
                scope_id,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    /// Parse statements until `}`, consume the `}`, and return a `Block`.
    fn parse_block_body(&mut self, close_msg: &str) -> Block {
        let mut stmts = Vec::new();
        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            if let Some(s) = self.parse_statement() {
                stmts.push(s);
            }
        }
        self.consume(TokenType::RBrace, close_msg);
        let scope_id = self.next_scope_id();
        Block {
            statements: stmts,
            scope_id,
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    /// Parse `{ stmts }` for a function body with implicit-return rewriting.
    fn parse_function_body(&mut self) -> Block {
        self.consume(TokenType::LBrace, "Expected { after function signature");

        let mut stmts = Vec::new();
        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            if let Some(s) = self.parse_statement() {
                stmts.push(s);
            }
        }
        self.consume(TokenType::RBrace, "Expected } after function body");

        // Single-expression body → implicit return
        if stmts.len() == 1 {
            if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
                let span = stmts[0].span;
                let value = expression.clone();
                stmts = vec![Statement {
                    kind: StatementKind::Return { value: Some(value) },
                    span,
                }];
            }
        }

        let scope_id = self.next_scope_id();
        Block {
            statements: stmts,
            scope_id,
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Function parameters
    // -----------------------------------------------------------------------

    fn parse_function_params(&mut self) -> Vec<FunctionParam> {
        let mut params = Vec::new();
        if self.check(TokenType::RParen) {
            return params;
        }
        loop {
            let param_name = self
                .consume(TokenType::Identifier, "Expected parameter name")
                .value
                .clone();
            self.consume(TokenType::Colon, "Expected : after parameter name");

            let param_type = if self.check(TokenType::Fn) {
                self.parse_function_type_annotation()
            } else if self.check(TokenType::LBracket) {
                let arr_name = self.parse_array_type_annotation();
                self.resolve_type_name(&arr_name)
            } else {
                let type_name_str = if self.check(TokenType::TypeToken) {
                    self.consume(TokenType::TypeToken, "Expected parameter type")
                        .value
                        .clone()
                } else {
                    self.consume(TokenType::Identifier, "Expected parameter type")
                        .value
                        .clone()
                };
                let mut full = type_name_str;
                while self.match_token(TokenType::LBracket) {
                    full.push('[');
                    self.consume(TokenType::RBracket, "Expected ] after array bracket");
                    full.push(']');
                }
                self.resolve_type_name(&full)
            };

            // Default value
            let default_value = if self.check(TokenType::Equal) && self.peek().value == "=" {
                self.advance(); // consume =
                Some(self.parse_expression())
            } else {
                None
            };

            params.push(FunctionParam {
                name: param_name,
                param_type,
                default_value,
            });

            if !self.match_token(TokenType::Comma) {
                break;
            }
        }
        params
    }

    // -----------------------------------------------------------------------
    // Return type
    // -----------------------------------------------------------------------

    fn parse_return_type(&mut self) -> Type {
        // fn(...) -> T
        if self.check(TokenType::Fn) {
            return self.parse_function_type_annotation();
        }

        // ?T
        if self.match_token(TokenType::QuestionMark) {
            let inner = self.parse_return_type();
            return Type::Optional(Box::new(inner));
        }

        // tensor<T>
        if self.check(TokenType::Tensor) {
            return self.parse_tensor_type_annotation();
        }

        // [T], [T; N]
        if self.check(TokenType::LBracket) {
            self.advance(); // consume [
            let inner_name = if self.check(TokenType::TypeToken) {
                self.consume(TokenType::TypeToken, "Expected type inside array brackets")
                    .value
                    .clone()
            } else {
                self.consume(TokenType::Identifier, "Expected type inside array brackets")
                    .value
                    .clone()
            };
            let mut arr_type_name = format!("[{}", inner_name);
            if self.match_token(TokenType::Semicolon) {
                let size = self
                    .consume(TokenType::Number, "Expected size after ;")
                    .value
                    .clone();
                arr_type_name.push_str("; ");
                arr_type_name.push_str(&size);
            }
            self.consume(TokenType::RBracket, "Expected ] after array type");
            arr_type_name.push(']');
            return self.resolve_type_name(&arr_type_name);
        }

        // TYPE or IDENTIFIER
        let return_name = if self.check(TokenType::TypeToken) {
            self.consume(TokenType::TypeToken, "Expected return type")
                .value
                .clone()
        } else {
            self.consume(TokenType::Identifier, "Expected return type")
                .value
                .clone()
        };

        // Result<T>
        if return_name == "Result" && self.match_token(TokenType::Less) {
            let inner = self.parse_return_type();
            self.consume(TokenType::Greater, "Expected > after Result<T>");
            return Type::Result(Box::new(inner));
        }
        // Future<T>
        if return_name == "Future" && self.match_token(TokenType::Less) {
            let inner = self.parse_return_type();
            self.consume(TokenType::Greater, "Expected > after Future<T>");
            return Type::Future(Box::new(inner));
        }

        let mut full = return_name;
        while self.match_token(TokenType::LBracket) {
            full.push('[');
            self.consume(TokenType::RBracket, "Expected ] after array bracket");
            full.push(']');
        }
        self.resolve_type_name(&full)
    }

    // -----------------------------------------------------------------------
    // Function type annotation:  fn(int, float) -> bool
    // -----------------------------------------------------------------------

    fn parse_function_type_annotation(&mut self) -> Type {
        self.consume(TokenType::Fn, "Expected fn keyword");
        self.consume(TokenType::LParen, "Expected ( after fn");

        let mut param_types = Vec::new();
        if !self.check(TokenType::RParen) {
            loop {
                let pt = if self.check(TokenType::Fn) {
                    self.parse_function_type_annotation()
                } else if self.check(TokenType::LBracket) {
                    let arr = self.parse_array_type_annotation();
                    self.resolve_type_name(&arr)
                } else {
                    let t_name = if self.check(TokenType::TypeToken) {
                        self.consume(TokenType::TypeToken, "Expected type")
                            .value
                            .clone()
                    } else {
                        self.consume(TokenType::Identifier, "Expected type")
                            .value
                            .clone()
                    };
                    let mut full = t_name;
                    while self.match_token(TokenType::LBracket) {
                        full.push('[');
                        self.consume(TokenType::RBracket, "Expected ] after array bracket");
                        full.push(']');
                    }
                    self.resolve_type_name(&full)
                };
                param_types.push(pt);
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }

        self.consume(TokenType::RParen, "Expected ) after fn param types");
        self.consume(TokenType::Arrow, "Expected -> after fn param types");
        let ret = self.parse_return_type();

        Type::Function {
            param_types,
            return_type: Box::new(ret),
        }
    }

    // -----------------------------------------------------------------------
    // Tensor type annotation:  tensor<f32>
    // -----------------------------------------------------------------------

    fn parse_tensor_type_annotation(&mut self) -> Type {
        self.consume(TokenType::Tensor, "Expected tensor keyword");
        self.consume(TokenType::Less, "Expected < after tensor");
        let dtype_name = if self.check(TokenType::TypeToken) {
            self.consume(TokenType::TypeToken, "Expected dtype")
                .value
                .clone()
        } else {
            self.consume(TokenType::Identifier, "Expected dtype")
                .value
                .clone()
        };
        self.consume(TokenType::Greater, "Expected > after tensor dtype");
        let dtype = self
            .parse_primitive_type(&dtype_name)
            .unwrap_or_else(|| panic!("Invalid tensor dtype: {}", dtype_name));
        Type::Tensor {
            dtype: Box::new(dtype),
            shape: None,
        }
    }

    // -----------------------------------------------------------------------
    // Array type annotation: [u8], [i32; 5], [[f64; 3]; 2]
    // -----------------------------------------------------------------------

    fn parse_array_type_annotation(&mut self) -> String {
        self.consume(TokenType::LBracket, "Expected [ to start array type");

        // Nested array: [[inner; n]; m]
        if self.check(TokenType::LBracket) {
            let inner = self.parse_array_type_annotation();
            self.consume(TokenType::Semicolon, "Expected ; after inner array type");
            let outer_size = self
                .consume(TokenType::Number, "Expected size after ; in array type")
                .value
                .clone();
            self.consume(TokenType::RBracket, "Expected ] after array type");
            return format!("[{}; {}]", inner, outer_size);
        }

        let inner_type = if self.check(TokenType::TypeToken) {
            self.consume(TokenType::TypeToken, "Expected type inside array brackets")
                .value
                .clone()
        } else if self.check(TokenType::Identifier) {
            self.consume(
                TokenType::Identifier,
                "Expected type name inside array brackets",
            )
            .value
            .clone()
        } else {
            panic!("Expected type name inside array brackets");
        };

        if self.match_token(TokenType::Semicolon) {
            let size = self
                .consume(TokenType::Number, "Expected size after ; in array type")
                .value
                .clone();
            self.consume(TokenType::RBracket, "Expected ] after array type");
            format!("[{}; {}]", inner_type, size)
        } else {
            self.consume(TokenType::RBracket, "Expected ] after array type");
            format!("[{}]", inner_type)
        }
    }

    // -----------------------------------------------------------------------
    // Type resolution from string name (mirrors `parseType` in TS)
    // -----------------------------------------------------------------------

    fn resolve_type_name(&self, name: &str) -> Type {
        // Fixed-size array: [T; N]
        if let Some(caps) = parse_fixed_array(name) {
            let (inner_name, size) = caps;
            if let Some(prim) = self.parse_primitive_type(&inner_name) {
                return Type::Array {
                    element_type: Box::new(prim),
                    dimensions: vec![size],
                };
            }
            // Nested fixed array like [[f64; 3]; 2]
            if inner_name.starts_with('[') {
                let inner_type = self.resolve_type_name(&inner_name);
                return Type::Array {
                    element_type: Box::new(inner_type),
                    dimensions: vec![size],
                };
            }
            // AOS type with struct: [Point; 100] → Custom for now
            return Type::AOS {
                element_type: Box::new(Type::Custom(inner_name)),
                size: Some(size),
            };
        }

        // Dynamic array: [T]
        if name.starts_with('[') && name.ends_with(']') && !name.contains(';') {
            let inner = &name[1..name.len() - 1];
            if let Some(prim) = self.parse_primitive_type(inner) {
                return Type::Array {
                    element_type: Box::new(prim),
                    dimensions: vec![],
                };
            }
            return Type::AOS {
                element_type: Box::new(Type::Custom(inner.to_string())),
                size: None,
            };
        }

        // Trailing-bracket arrays: f64[], f64[][], etc.
        if let Some(base_end) = name.find("[]") {
            let base = &name[..base_end];
            let suffix = &name[base_end..];
            if let Some(element_type) = self.parse_primitive_type(base) {
                let depth = suffix.matches('[').count();
                let mut current = element_type;
                for _ in 0..depth {
                    current = Type::Array {
                        element_type: Box::new(current),
                        dimensions: vec![],
                    };
                }
                return current;
            }
            return Type::Void;
        }

        // Primitive
        if let Some(prim) = self.parse_primitive_type(name) {
            return prim;
        }

        // Custom/struct type
        Type::Custom(name.to_string())
    }

    fn parse_primitive_type(&self, name: &str) -> Option<Type> {
        match name {
            "int" => Some(Type::Integer(IntegerKind::I64)),
            "float" => Some(Type::Float(FloatKind::F64)),
            "bool" => Some(Type::Bool),
            "string" => Some(Type::String),
            "ptr" => Some(Type::Pointer(None)),
            "bf16" => Some(Type::Float(FloatKind::Bf16)),
            _ => {
                if let Some(k) = IntegerKind::from_str(name) {
                    return Some(Type::Integer(k));
                }
                if let Some(k) = UnsignedKind::from_str(name) {
                    return Some(Type::Unsigned(k));
                }
                if let Some(k) = FloatKind::from_str(name) {
                    return Some(Type::Float(k));
                }
                None
            }
        }
    }

    // =======================================================================
    // EXPRESSIONS
    // =======================================================================

    fn parse_expression(&mut self) -> Expr {
        self.parse_assignment()
    }

    // -----------------------------------------------------------------------
    // Assignment
    // -----------------------------------------------------------------------

    fn parse_assignment(&mut self) -> Expr {
        let expr = self.parse_conditional();

        // := or =
        let is_assign = self.match_token(TokenType::Assign);
        let is_equal_assign =
            if !is_assign && self.check(TokenType::Equal) && self.peek().value == "=" {
                self.advance();
                true
            } else {
                false
            };

        if is_assign || is_equal_assign {
            let value = self.parse_assignment();
            let span = self.combine_spans(expr.span, value.span);
            match &expr.kind {
                ExprKind::Identifier { name } => {
                    return Expr {
                        kind: ExprKind::AssignmentExpression {
                            name: Some(name.clone()),
                            target: None,
                            value: Box::new(value),
                        },
                        span,
                    };
                }
                ExprKind::IndexExpression { .. } | ExprKind::MemberExpression { .. } => {
                    return Expr {
                        kind: ExprKind::AssignmentExpression {
                            name: None,
                            target: Some(Box::new(expr)),
                            value: Box::new(value),
                        },
                        span,
                    };
                }
                _ => panic!("Invalid assignment target"),
            }
        }

        expr
    }

    // -----------------------------------------------------------------------
    // Conditional (ternary)
    // -----------------------------------------------------------------------

    fn parse_conditional(&mut self) -> Expr {
        let expr = self.parse_binary_expression();

        // Not using QUESTION here — the TS parser uses a special "QUESTION" token type.
        // In the Rust token set, QuestionMark is `?` and is used for error propagation.
        // The ternary `?` in the TS parser checks for "QUESTION" which is a different token.
        // We don't have a separate ternary question token, so we skip ternary for now.
        // (The TS `QUESTION` is never actually emitted by the lexer for ternary in practice.)

        expr
    }

    // -----------------------------------------------------------------------
    // Binary expression — FLAT precedence
    // -----------------------------------------------------------------------

    fn parse_binary_expression(&mut self) -> Expr {
        let mut expr = self.parse_unary();

        // Handle `expr is some(name)` / `expr is none` / `expr is ok(name)` / `expr is err(name)`
        if self.match_token(TokenType::Is) {
            if self.match_token(TokenType::Some) {
                self.consume(TokenType::LParen, "Expected ( after some in is-pattern");
                let binding = self
                    .consume(
                        TokenType::Identifier,
                        "Expected binding name in is some(...)",
                    )
                    .value
                    .clone();
                self.consume(
                    TokenType::RParen,
                    "Expected ) after binding name in is some(...)",
                );
                let span = self.combine_spans(expr.span, self.previous().span);
                return Expr {
                    kind: ExprKind::IsSomeExpression {
                        value: Box::new(expr),
                        binding,
                    },
                    span,
                };
            } else if self.match_token(TokenType::None) {
                let span = self.combine_spans(expr.span, self.previous().span);
                return Expr {
                    kind: ExprKind::IsNoneExpression {
                        value: Box::new(expr),
                    },
                    span,
                };
            } else if self.match_token(TokenType::Ok) {
                self.consume(TokenType::LParen, "Expected ( after ok in is-pattern");
                let binding = self
                    .consume(TokenType::Identifier, "Expected binding name in is ok(...)")
                    .value
                    .clone();
                self.consume(
                    TokenType::RParen,
                    "Expected ) after binding name in is ok(...)",
                );
                let span = self.combine_spans(expr.span, self.previous().span);
                return Expr {
                    kind: ExprKind::IsOkExpression {
                        value: Box::new(expr),
                        binding,
                    },
                    span,
                };
            } else if self.match_token(TokenType::Err) {
                self.consume(TokenType::LParen, "Expected ( after err in is-pattern");
                let binding = self
                    .consume(
                        TokenType::Identifier,
                        "Expected binding name in is err(...)",
                    )
                    .value
                    .clone();
                self.consume(
                    TokenType::RParen,
                    "Expected ) after binding name in is err(...)",
                );
                let span = self.combine_spans(expr.span, self.previous().span);
                return Expr {
                    kind: ExprKind::IsErrExpression {
                        value: Box::new(expr),
                        binding,
                    },
                    span,
                };
            }
        }

        if !self.is_binary_operator() {
            return expr;
        }

        // First binary op
        self.advance();
        let first_op_value = self.previous().value.clone();
        let right = self.parse_unary();
        let span = self.combine_spans(expr.span, right.span);
        expr = Expr {
            kind: ExprKind::BinaryExpression {
                operator: first_op_value.clone(),
                left: Box::new(expr),
                right: Box::new(right),
            },
            span,
        };

        // Chaining
        while self.is_binary_operator() {
            let next_op_value = self.peek().value.clone();

            if next_op_value != first_op_value {
                panic!(
                    "Cannot mix operators '{}' and '{}' without parentheses at line {}",
                    first_op_value,
                    next_op_value,
                    self.peek().span.start.line
                );
            }

            if !is_associative_op(&first_op_value) {
                panic!(
                    "Operator '{}' is not associative; use parentheses at line {}",
                    first_op_value,
                    self.peek().span.start.line
                );
            }

            self.advance();
            let chain_right = self.parse_unary();
            let span = self.combine_spans(expr.span, chain_right.span);
            expr = Expr {
                kind: ExprKind::BinaryExpression {
                    operator: first_op_value.clone(),
                    left: Box::new(expr),
                    right: Box::new(chain_right),
                },
                span,
            };
        }

        expr
    }

    fn is_binary_operator(&self) -> bool {
        if self.is_at_end() {
            return false;
        }
        let tok = self.peek();
        is_binary_op_token(tok.token_type, &tok.value)
    }

    // -----------------------------------------------------------------------
    // Unary
    // -----------------------------------------------------------------------

    fn parse_unary(&mut self) -> Expr {
        // await
        if self.match_token(TokenType::Await) {
            let start_span = self.previous().span;
            let argument = self.parse_unary();
            let span = self.combine_spans(start_span, argument.span);
            return Expr {
                kind: ExprKind::AwaitExpression {
                    argument: Box::new(argument),
                },
                span,
            };
        }

        // spawn
        if self.match_token(TokenType::Spawn) {
            let start_span = self.previous().span;
            let argument = self.parse_unary();
            let span = self.combine_spans(start_span, argument.span);
            return Expr {
                kind: ExprKind::SpawnExpression {
                    argument: Box::new(argument),
                },
                span,
            };
        }

        // - ! not ~
        if self.match_token(TokenType::Minus)
            || self.match_token(TokenType::NotEqual) && self.previous().value == "!"
            || self.match_token(TokenType::Not)
            || self.match_token(TokenType::BitwiseNot)
        {
            let op = self.previous().value.clone();
            let start_span = self.previous().span;
            let argument = self.parse_unary();
            let span = self.combine_spans(start_span, argument.span);
            return Expr {
                kind: ExprKind::UnaryExpression {
                    operator: op,
                    operand: Box::new(argument),
                },
                span,
            };
        }

        // cast<T>(expr)
        if self.match_token(TokenType::Cast) {
            let start_span = self.previous().span;
            self.consume(TokenType::Less, "Expected < after cast");
            let mut type_name = self
                .consume(TokenType::TypeToken, "Expected type name after cast<")
                .value
                .clone();
            while self.match_token(TokenType::LBracket) {
                type_name.push('[');
                self.consume(TokenType::RBracket, "Expected ] after array bracket");
                type_name.push(']');
            }
            self.consume(TokenType::Greater, "Expected > after type name");
            self.consume(TokenType::LParen, "Expected ( after cast<type>");
            let value = self.parse_expression();
            self.consume(TokenType::RParen, "Expected ) after cast value");
            let target_type = self.resolve_type_name(&type_name);
            let span = self.combine_spans(start_span, value.span);
            return Expr {
                kind: ExprKind::CastExpression {
                    target_type,
                    value: Box::new(value),
                    source_type: None,
                },
                span,
            };
        }

        let mut expr = self.parse_primary();

        // ? postfix (error propagation)
        if self.match_token(TokenType::QuestionMark) {
            let span = self.combine_spans(expr.span, self.previous().span);
            expr = Expr {
                kind: ExprKind::PropagateExpression {
                    value: Box::new(expr),
                },
                span,
            };
        }

        // `as Type` postfix cast
        while self.match_token(TokenType::As) {
            let type_name = if self.check(TokenType::TypeToken) {
                self.consume(TokenType::TypeToken, "Expected type name after as")
                    .value
                    .clone()
            } else {
                self.consume(TokenType::Identifier, "Expected type name after as")
                    .value
                    .clone()
            };
            let mut full = type_name;
            while self.match_token(TokenType::LBracket) {
                full.push('[');
                self.consume(TokenType::RBracket, "Expected ] after array bracket");
                full.push(']');
            }
            let target_type = self.resolve_type_name(&full);
            let span = expr.span; // use existing span
            expr = Expr {
                kind: ExprKind::CastExpression {
                    target_type,
                    value: Box::new(expr),
                    source_type: None,
                },
                span,
            };
        }

        expr
    }

    // -----------------------------------------------------------------------
    // Primary
    // -----------------------------------------------------------------------

    fn parse_primary(&mut self) -> Expr {
        // SoA constructor: soa Struct[N]
        if self.match_token(TokenType::Soa) {
            let struct_name = self
                .consume(TokenType::Identifier, "Expected struct name after soa")
                .value
                .clone();
            self.consume(TokenType::LBracket, "Expected [ after struct name");
            let capacity = if self.check(TokenType::Number) {
                let v = self
                    .consume(TokenType::Number, "Expected capacity")
                    .value
                    .clone();
                Some(v.parse::<usize>().unwrap())
            } else {
                None
            };
            self.consume(TokenType::RBracket, "Expected ] after capacity");
            return Expr {
                kind: ExprKind::SoAConstructor {
                    struct_name,
                    capacity,
                },
                span: Span {
                    start: Position {
                        line: 1,
                        column: 1,
                        index: 0,
                    },
                    end: self.last_position(),
                },
            };
        }

        // If expression
        if self.match_token(TokenType::If) {
            return self.parse_if_expression();
        }

        // Match expression
        if self.match_token(TokenType::Match) {
            return self.parse_match_expression();
        }

        // Anonymous function: fn(params) -> type { body }
        if self.match_token(TokenType::Fn) {
            return self.parse_lambda();
        }

        // ok(expr)
        if self.match_token(TokenType::Ok) {
            let start_span = self.previous().span;
            self.consume(TokenType::LParen, "Expected ( after ok");
            let value = self.parse_expression();
            self.consume(TokenType::RParen, "Expected ) after ok value");
            let span = self.combine_spans(start_span, value.span);
            return Expr {
                kind: ExprKind::OkExpression {
                    value: Box::new(value),
                },
                span,
            };
        }

        // err(expr)
        if self.match_token(TokenType::Err) {
            let start_span = self.previous().span;
            self.consume(TokenType::LParen, "Expected ( after err");
            let value = self.parse_expression();
            self.consume(TokenType::RParen, "Expected ) after err value");
            let span = self.combine_spans(start_span, value.span);
            return Expr {
                kind: ExprKind::ErrExpression {
                    value: Box::new(value),
                },
                span,
            };
        }

        // some(expr)
        if self.match_token(TokenType::Some) {
            let start_span = self.previous().span;
            self.consume(TokenType::LParen, "Expected ( after some");
            let value = self.parse_expression();
            self.consume(TokenType::RParen, "Expected ) after some value");
            let span = self.combine_spans(start_span, value.span);
            return Expr {
                kind: ExprKind::SomeExpression {
                    value: Box::new(value),
                },
                span,
            };
        }

        // none
        if self.match_token(TokenType::None) {
            return Expr {
                kind: ExprKind::NoneExpression,
                span: self.prev_span(),
            };
        }

        // false
        if self.match_token(TokenType::False) {
            return Expr {
                kind: ExprKind::BooleanLiteral { value: false },
                span: self.prev_span(),
            };
        }

        // true
        if self.match_token(TokenType::True) {
            return Expr {
                kind: ExprKind::BooleanLiteral { value: true },
                span: self.prev_span(),
            };
        }

        // Number literal
        if self.match_token(TokenType::Number) {
            let value = self.previous().value.clone();
            return Expr {
                kind: ExprKind::NumberLiteral {
                    value,
                    literal_type: None,
                },
                span: self.prev_span(),
            };
        }

        // String literal
        if self.match_token(TokenType::StringLit) {
            let raw = self.previous().value.clone();
            let inner = &raw[1..raw.len() - 1];
            let value = decode_escape_sequences(inner);
            return Expr {
                kind: ExprKind::StringLiteral { value },
                span: self.prev_span(),
            };
        }

        // Template string
        if self.check(TokenType::TemplateStringStart) {
            return self.parse_template_string();
        }

        // tensor<dtype>(args)
        if self.match_token(TokenType::Tensor) {
            let start_span = self.previous().span;
            self.consume(TokenType::Less, "Expected < after tensor");
            let dtype_name = if self.check(TokenType::TypeToken) {
                self.consume(TokenType::TypeToken, "Expected dtype")
                    .value
                    .clone()
            } else {
                self.consume(TokenType::Identifier, "Expected dtype")
                    .value
                    .clone()
            };
            self.consume(TokenType::Greater, "Expected > after tensor dtype");
            let dtype = self
                .parse_primitive_type(&dtype_name)
                .unwrap_or_else(|| panic!("Invalid tensor dtype: {}", dtype_name));

            let mut args = Vec::new();
            if self.match_token(TokenType::LParen) {
                if !self.check(TokenType::RParen) {
                    loop {
                        args.push(self.parse_expression());
                        if !self.match_token(TokenType::Comma) {
                            break;
                        }
                    }
                }
                self.consume(TokenType::RParen, "Expected ) after tensor arguments");
            }

            let mut object = Expr {
                kind: ExprKind::TensorConstruction { dtype, args },
                span: self.combine_spans(
                    start_span,
                    Span {
                        start: self.last_position(),
                        end: self.last_position(),
                    },
                ),
            };

            // Chained postfix on tensor
            loop {
                if self.match_token(TokenType::Dot) {
                    let property = self
                        .consume(TokenType::Identifier, "Expected property name")
                        .value
                        .clone();
                    let member_span = self.combine_spans(object.span, self.previous().span);
                    object = Expr {
                        kind: ExprKind::MemberExpression {
                            object: Box::new(object),
                            property,
                        },
                        span: member_span,
                    };
                    if self.match_token(TokenType::LParen) {
                        let mut call_args = Vec::new();
                        if !self.check(TokenType::RParen) {
                            loop {
                                call_args.push(self.parse_expression());
                                if !self.match_token(TokenType::Comma) {
                                    break;
                                }
                            }
                        }
                        self.consume(TokenType::RParen, "Expected ) after arguments");
                        let call_span = self.combine_spans(object.span, self.previous().span);
                        object = Expr {
                            kind: ExprKind::CallExpression {
                                callee: Box::new(object),
                                arguments: call_args,
                                named_args: None,
                            },
                            span: call_span,
                        };
                    }
                } else {
                    break;
                }
            }

            return object;
        }

        // Identifier (with postfix chaining and struct literal)
        if self.match_token(TokenType::Identifier) {
            let tok_value = self.previous().value.clone();
            let tok_span = self.previous().span;

            // Struct literal: TypeName { field: value, ... }
            if self.allow_struct_literal && self.match_token(TokenType::LBrace) {
                let mut fields = Vec::new();
                if !self.check(TokenType::RBrace) {
                    loop {
                        let field_name = self
                            .consume(TokenType::Identifier, "Expected field name")
                            .value
                            .clone();
                        self.consume(TokenType::Colon, "Expected ':' after field name");
                        let field_value = self.parse_expression();
                        fields.push(StructFieldInit {
                            name: field_name,
                            value: field_value,
                        });
                        if !self.match_token(TokenType::Comma) {
                            break;
                        }
                    }
                }
                self.consume(
                    TokenType::RBrace,
                    "Expected '}' after struct literal fields",
                );
                let span = self.combine_spans(tok_span, self.previous().span);
                return Expr {
                    kind: ExprKind::StructLiteral {
                        struct_name: Some(tok_value),
                        fields,
                    },
                    span,
                };
            }

            let mut object = Expr {
                kind: ExprKind::Identifier {
                    name: tok_value.clone(),
                },
                span: tok_span.into(),
            };

            // Postfix chaining: [], (), .
            loop {
                if self.match_token(TokenType::LBracket) {
                    let first_expr = self.parse_expression();

                    if self.match_token(TokenType::Colon) {
                        // Slice: obj[start:end]
                        let end_expr = self.parse_expression();
                        self.consume(TokenType::RBracket, "Expected ] after slice expression");
                        let span = self.combine_spans(object.span, end_expr.span);
                        object = Expr {
                            kind: ExprKind::SliceExpression {
                                object: Box::new(object),
                                start: Box::new(first_expr),
                                end: Box::new(end_expr),
                                step: None,
                            },
                            span,
                        };
                    } else {
                        // Index: obj[index]
                        self.consume(TokenType::RBracket, "Expected ] after index");
                        let span = self.combine_spans(object.span, first_expr.span);
                        object = Expr {
                            kind: ExprKind::IndexExpression {
                                object: Box::new(object),
                                index: Box::new(first_expr),
                            },
                            span,
                        };
                    }
                } else if self.match_token(TokenType::LParen) {
                    let mut call_args = Vec::new();
                    let mut named_args: Vec<NamedArg> = Vec::new();
                    if !self.check(TokenType::RParen) {
                        loop {
                            // Check for named argument: identifier followed by COLON
                            if self.check(TokenType::Identifier)
                                && self.peek_next().map(|t| t.token_type) == Some(TokenType::Colon)
                            {
                                let saved_pos = self.pos;
                                let arg_name =
                                    self.consume(TokenType::Identifier, "").value.clone();
                                self.consume(TokenType::Colon, "");
                                let arg_value = self.parse_expression();
                                named_args.push(NamedArg {
                                    name: arg_name,
                                    value: arg_value,
                                });
                                let _ = saved_pos; // used in TS for backtracking but not needed here
                            } else {
                                call_args.push(self.parse_expression());
                            }
                            if !self.match_token(TokenType::Comma) {
                                break;
                            }
                        }
                    }
                    self.consume(TokenType::RParen, "Expected ) after arguments");
                    let call_span = self.combine_spans(object.span, self.previous().span);
                    object = Expr {
                        kind: ExprKind::CallExpression {
                            callee: Box::new(object),
                            arguments: call_args,
                            named_args: if named_args.is_empty() {
                                None
                            } else {
                                Some(named_args)
                            },
                        },
                        span: call_span,
                    };
                } else if self.match_token(TokenType::Dot) {
                    let property = self
                        .consume(TokenType::Identifier, "Expected property name")
                        .value
                        .clone();
                    let span = self.combine_spans(object.span, self.previous().span);
                    object = Expr {
                        kind: ExprKind::MemberExpression {
                            object: Box::new(object),
                            property,
                        },
                        span,
                    };
                } else {
                    break;
                }
            }

            return object;
        }

        // Array literal: [...]
        if self.match_token(TokenType::LBracket) {
            return self.parse_array_literal();
        }

        // Map literal: {...}
        if self.match_token(TokenType::LBrace) {
            return self.parse_map_literal();
        }

        // Parenthesized expression
        if self.match_token(TokenType::LParen) {
            let expr = self.parse_expression();
            self.consume(TokenType::RParen, "Expected ) after expression");
            return expr;
        }

        let tok = self.peek();
        panic!(
            "Unexpected token: {} at line {}",
            tok.token_type, tok.span.start.line
        );
    }

    // -----------------------------------------------------------------------
    // Lambda (anonymous function)
    // -----------------------------------------------------------------------

    fn parse_lambda(&mut self) -> Expr {
        self.consume(TokenType::LParen, "Expected ( after fn");
        let params = self.parse_function_params();
        self.consume(TokenType::RParen, "Expected ) after parameters");

        let return_type = if self.match_token(TokenType::Arrow) {
            self.parse_return_type()
        } else {
            Type::Void
        };

        let body = self.parse_function_body();

        Expr {
            kind: ExprKind::Lambda {
                params,
                return_type,
                body,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // If expression
    // -----------------------------------------------------------------------

    fn parse_if_expression(&mut self) -> Expr {
        let condition = if self.match_token(TokenType::LParen) {
            let cond = self.parse_expression();
            self.consume(TokenType::RParen, "Expected ) after condition");
            cond
        } else {
            let prev = self.allow_struct_literal;
            self.allow_struct_literal = false;
            let cond = self.parse_expression();
            self.allow_struct_literal = prev;
            cond
        };

        self.consume(TokenType::LBrace, "Expected { after if condition");
        let true_branch = self.parse_block_body("Expected } after if body");

        let false_branch = if self.match_token(TokenType::Else) {
            if self.match_token(TokenType::If) {
                // else if — recursive
                let nested = self.parse_if_expression();
                IfElseBranch::IfExpression(Box::new(nested))
            } else {
                self.consume(TokenType::LBrace, "Expected { after else");
                let block = self.parse_block_body("Expected } after else body");
                IfElseBranch::Block(block)
            }
        } else {
            // No else branch — produce an empty block
            IfElseBranch::Block(Block {
                statements: vec![],
                scope_id: self.next_scope_id(),
                span: Span {
                    start: Position {
                        line: 1,
                        column: 1,
                        index: 0,
                    },
                    end: self.last_position(),
                },
            })
        };

        Expr {
            kind: ExprKind::IfExpression {
                condition: Box::new(condition),
                true_branch,
                false_branch,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Match expression
    // -----------------------------------------------------------------------

    fn parse_match_expression(&mut self) -> Expr {
        let prev = self.allow_struct_literal;
        self.allow_struct_literal = false;
        let subject = self.parse_expression();
        self.allow_struct_literal = prev;

        self.consume(TokenType::LBrace, "Expected { after match subject");

        let mut arms = Vec::new();
        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            // Parse pattern
            let pattern = if self.match_token(TokenType::Underscore) {
                MatchPattern::WildcardPattern
            } else if (self.check(TokenType::Identifier)
                || self.check(TokenType::Ok)
                || self.check(TokenType::Err)
                || self.check(TokenType::Some)
                || self.check(TokenType::None))
                && self.peek_next().map(|t| t.token_type) == Some(TokenType::LParen)
            {
                let name = self.advance().value.clone();
                self.consume(TokenType::LParen, "Expected ( after variant name");
                let binding = if !self.check(TokenType::RParen) {
                    Some(
                        self.consume(TokenType::Identifier, "Expected binding variable")
                            .value
                            .clone(),
                    )
                } else {
                    None
                };
                self.consume(TokenType::RParen, "Expected ) after variant binding");
                MatchPattern::VariantPattern { name, binding }
            } else if self.check(TokenType::None)
                && self.peek_next().map(|t| t.token_type) != Some(TokenType::LParen)
            {
                self.advance();
                MatchPattern::VariantPattern {
                    name: "none".to_string(),
                    binding: None,
                }
            } else {
                let value = self.parse_expression();
                MatchPattern::LiteralPattern(value)
            };

            self.consume(TokenType::FatArrow, "Expected => after pattern");

            // Parse arm body
            let body = if self.check(TokenType::LBrace) {
                self.advance(); // consume {
                let mut stmts = Vec::new();
                while !self.check(TokenType::RBrace) && !self.is_at_end() {
                    if let Some(s) = self.parse_statement() {
                        stmts.push(s);
                    }
                }
                self.consume(TokenType::RBrace, "Expected } after match arm body");

                if stmts.len() == 1 {
                    if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
                        *expression.clone()
                    } else {
                        // Wrap multi-statement body in a BlockExpression
                        let scope_id = self.next_scope_id();
                        Expr {
                            kind: ExprKind::BlockExpression {
                                block: Block {
                                    statements: stmts,
                                    scope_id,
                                    span: Span {
                                        start: Position {
                                            line: 1,
                                            column: 1,
                                            index: 0,
                                        },
                                        end: self.last_position(),
                                    },
                                },
                            },
                            span: Span {
                                start: Position {
                                    line: 1,
                                    column: 1,
                                    index: 0,
                                },
                                end: self.last_position(),
                            },
                        }
                    }
                } else if !stmts.is_empty() {
                    let scope_id = self.next_scope_id();
                    Expr {
                        kind: ExprKind::BlockExpression {
                            block: Block {
                                statements: stmts,
                                scope_id,
                                span: Span {
                                    start: Position {
                                        line: 1,
                                        column: 1,
                                        index: 0,
                                    },
                                    end: self.last_position(),
                                },
                            },
                        },
                        span: Span {
                            start: Position {
                                line: 1,
                                column: 1,
                                index: 0,
                            },
                            end: self.last_position(),
                        },
                    }
                } else {
                    Expr {
                        kind: ExprKind::NumberLiteral {
                            value: "0".to_string(),
                            literal_type: None,
                        },
                        span: subject.span,
                    }
                }
            } else {
                self.parse_expression()
            };

            arms.push(MatchArm { pattern, body });
            self.match_token(TokenType::Comma);
        }

        self.consume(TokenType::RBrace, "Expected } after match expression");

        Expr {
            kind: ExprKind::MatchExpression {
                subject: Box::new(subject),
                arms,
            },
            span: Span {
                start: Position {
                    line: 1,
                    column: 1,
                    index: 0,
                },
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Template string
    // -----------------------------------------------------------------------

    fn parse_template_string(&mut self) -> Expr {
        let start = self.peek().span.start;
        self.advance(); // consume TEMPLATE_STRING_START

        let mut parts = Vec::new();

        while !self.check(TokenType::TemplateStringEnd) && !self.is_at_end() {
            if self.check(TokenType::TemplateStringPart) {
                let str_part = self.advance().value.clone();
                if !str_part.is_empty() {
                    parts.push(TemplatePart::String(str_part));
                }
            } else if self.check(TokenType::TemplateInterpStart) {
                self.advance(); // consume {

                if self.check(TokenType::TemplateStringPart) {
                    let expr_text = self.advance().value.clone();
                    // Tokenize the interpolation expression and parse it
                    let expr_tokens = tokenize(&expr_text);
                    let expr_tokens: Vec<Token> = expr_tokens
                        .into_iter()
                        .filter(|t| t.token_type != TokenType::Whitespace)
                        .collect();
                    let mut sub_parser = Parser::new(&expr_tokens);
                    let expr = sub_parser.parse_expression();
                    parts.push(TemplatePart::Expr(expr));
                }

                self.consume(TokenType::TemplateInterpEnd, "Expected } after expression");
            } else {
                break;
            }
        }

        self.consume(
            TokenType::TemplateStringEnd,
            "Expected \" at end of template string",
        );

        let end: Position = self.previous().span.end.into();
        Expr {
            kind: ExprKind::TemplateLiteral { parts },
            span: Span {
                start: start.into(),
                end,
            },
        }
    }

    // -----------------------------------------------------------------------
    // Array literal
    // -----------------------------------------------------------------------

    fn parse_array_literal(&mut self) -> Expr {
        let start: Position = self.previous().span.start.into();

        // Empty array
        if self.check(TokenType::RBracket) {
            self.consume(TokenType::RBracket, "Expected ] after array elements");
            return Expr {
                kind: ExprKind::ArrayLiteral { elements: vec![] },
                span: Span {
                    start,
                    end: self.last_position(),
                },
            };
        }

        let first = self.parse_expression();

        // Array fill: [value; count]
        if self.match_token(TokenType::Semicolon) {
            let count = self.parse_expression();
            self.consume(TokenType::RBracket, "Expected ] after array fill count");
            return Expr {
                kind: ExprKind::ArrayFill {
                    value: Box::new(first),
                    count: Box::new(count),
                },
                span: Span {
                    start,
                    end: self.last_position(),
                },
            };
        }

        // Regular array literal
        let mut elements = vec![first];
        while self.match_token(TokenType::Comma) {
            elements.push(self.parse_expression());
        }
        self.consume(TokenType::RBracket, "Expected ] after array elements");

        Expr {
            kind: ExprKind::ArrayLiteral { elements },
            span: Span {
                start,
                end: self.last_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Map literal
    // -----------------------------------------------------------------------

    fn parse_map_literal(&mut self) -> Expr {
        let start: Position = self.previous().span.start.into();
        let mut entries = Vec::new();

        while !self.check(TokenType::RBrace) && !self.is_at_end() {
            let key = if self.check(TokenType::StringLit) {
                let raw = self.advance().value.clone();
                if raw.starts_with('"') || raw.starts_with('\'') {
                    raw[1..raw.len() - 1].to_string()
                } else {
                    raw
                }
            } else if self.check(TokenType::Number) {
                self.advance().value.clone()
            } else {
                self.consume(
                    TokenType::Identifier,
                    "Expected field name, string, or number in map literal",
                )
                .value
                .clone()
            };

            self.consume(TokenType::Colon, "Expected : after key");
            let value = self.parse_expression();
            entries.push(MapEntry { key, value });
            self.match_token(TokenType::Comma);
        }

        self.consume(TokenType::RBrace, "Expected } after map literal");

        Expr {
            kind: ExprKind::MapLiteral { entries },
            span: Span {
                start,
                end: self.last_position(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Position conversion from token::Position → ast::Position
// ---------------------------------------------------------------------------

impl From<crate::token::Position> for Position {
    fn from(p: crate::token::Position) -> Self {
        Position {
            line: p.line,
            column: p.column,
            index: p.index,
        }
    }
}

impl From<crate::token::Span> for Span {
    fn from(s: crate::token::Span) -> Self {
        Span {
            start: s.start.into(),
            end: s.end.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn unwrap_statements(stmts: Vec<Statement>) -> Vec<Statement> {
    if stmts.len() == 1 {
        if let StatementKind::Block { statements, .. } = &stmts[0].kind {
            if statements.len() == 1 {
                return statements.clone();
            }
        }
    }
    stmts
}

/// Parse `[TypeName; Size]` from a type-name string.
fn parse_fixed_array(name: &str) -> Option<(String, usize)> {
    if !name.starts_with('[') || !name.ends_with(']') {
        return None;
    }
    let inner = &name[1..name.len() - 1];
    // Find the last ';' — to handle nested types like [[f64; 3]; 2]
    let semi_pos = inner.rfind(';')?;
    let type_part = inner[..semi_pos].trim().to_string();
    let size_part = inner[semi_pos + 1..].trim();
    let size = size_part.parse::<usize>().ok()?;
    Some((type_part, size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn parse_src(src: &str) -> Statement {
        let tokens: Vec<Token> = tokenize(src)
            .into_iter()
            .filter(|t| t.token_type != TokenType::Whitespace && t.token_type != TokenType::Comment)
            .collect();
        parse(&tokens)
    }

    /// Extract the statements from a Program node.
    fn program_stmts(program: &Statement) -> &Vec<Statement> {
        match &program.kind {
            StatementKind::Program { statements, .. } => statements,
            _ => panic!("expected Program, got {:?}", program.kind),
        }
    }

    // 1. Variable declaration with type annotation: `x: int = 5;`
    #[test]
    fn test_variable_declaration_typed() {
        let program = parse_src("x: int = 5;");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::VariableDeclaration {
            name,
            var_type,
            value,
        } = &stmts[0].kind
        {
            assert_eq!(name, "x");
            assert!(var_type.is_some());
            assert!(value.is_some());
            if let ExprKind::NumberLiteral { value: val, .. } = &value.as_ref().unwrap().kind {
                assert_eq!(val, "5");
            } else {
                panic!("expected NumberLiteral");
            }
        } else {
            panic!("expected VariableDeclaration, got {:?}", stmts[0].kind);
        }
    }

    // 2. Variable declaration with walrus operator: `x: int := 5;`
    #[test]
    fn test_variable_declaration_walrus() {
        let program = parse_src("x: int := 5;");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::VariableDeclaration {
            name,
            var_type,
            value,
        } = &stmts[0].kind
        {
            assert_eq!(name, "x");
            assert!(var_type.is_some());
            assert!(value.is_some());
        } else {
            panic!("expected VariableDeclaration, got {:?}", stmts[0].kind);
        }
    }

    // 3. Assignment expression: `x = 10;`
    #[test]
    fn test_assignment() {
        let program = parse_src("x = 10;");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
            if let ExprKind::AssignmentExpression { name, value, .. } = &expression.kind {
                assert_eq!(name.as_deref(), Some("x"));
                if let ExprKind::NumberLiteral { value: val, .. } = &value.kind {
                    assert_eq!(val, "10");
                } else {
                    panic!("expected NumberLiteral in assignment value");
                }
            } else {
                panic!("expected AssignmentExpression, got {:?}", expression.kind);
            }
        } else {
            panic!("expected ExpressionStatement, got {:?}", stmts[0].kind);
        }
    }

    // 4. Function declaration
    #[test]
    fn test_function_declaration() {
        let program = parse_src("fn add(a: int, b: int) -> int { return a + b; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::FunctionDeclaration {
            name,
            params,
            body,
            is_public,
            ..
        } = &stmts[0].kind
        {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "a");
            assert_eq!(params[1].name, "b");
            assert!(!is_public);
            assert!(!body.statements.is_empty());
        } else {
            panic!("expected FunctionDeclaration, got {:?}", stmts[0].kind);
        }
    }

    // 5. Public function
    #[test]
    fn test_pub_function() {
        let program = parse_src("pub fn greet() -> string { return \"hi\"; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::FunctionDeclaration {
            name, is_public, ..
        } = &stmts[0].kind
        {
            assert_eq!(name, "greet");
            assert!(is_public);
        } else {
            panic!("expected FunctionDeclaration, got {:?}", stmts[0].kind);
        }
    }

    // 6. Struct definition
    #[test]
    fn test_struct_definition() {
        let program = parse_src("struct Point { x: int; y: int; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::StructDefinition {
            name,
            fields,
            is_public,
        } = &stmts[0].kind
        {
            assert_eq!(name, "Point");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[1].name, "y");
            assert!(!is_public);
        } else {
            panic!("expected StructDefinition, got {:?}", stmts[0].kind);
        }
    }

    // 7. If-else conditional
    #[test]
    fn test_if_else() {
        let program = parse_src("if x > 0 { return 1; } else { return 0; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::Conditional {
            condition,
            true_branch,
            false_branch,
        } = &stmts[0].kind
        {
            // condition is a binary expression
            if let ExprKind::BinaryExpression { operator, .. } = &condition.kind {
                assert_eq!(operator, ">");
            } else {
                panic!("expected BinaryExpression in condition");
            }
            assert!(!true_branch.statements.is_empty());
            assert!(false_branch.is_some());
        } else {
            panic!("expected Conditional, got {:?}", stmts[0].kind);
        }
    }

    // 8. While loop
    #[test]
    fn test_while_loop() {
        let program = parse_src("while x > 0 { x = x - 1; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::WhileLoop { test, body } = &stmts[0].kind {
            if let ExprKind::BinaryExpression { operator, .. } = &test.kind {
                assert_eq!(operator, ">");
            } else {
                panic!("expected BinaryExpression in while test");
            }
            assert!(!body.statements.is_empty());
        } else {
            panic!("expected WhileLoop, got {:?}", stmts[0].kind);
        }
    }

    // 9. For-in-range loop
    #[test]
    fn test_for_in_range() {
        let program = parse_src("for i in 0..10 { x = i; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::ForInRange {
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
                panic!("expected NumberLiteral for range start");
            }
            if let ExprKind::NumberLiteral { value, .. } = &end.kind {
                assert_eq!(value, "10");
            } else {
                panic!("expected NumberLiteral for range end");
            }
            assert!(!body.statements.is_empty());
        } else {
            panic!("expected ForInRange, got {:?}", stmts[0].kind);
        }
    }

    // 10. For-each loop
    #[test]
    fn test_for_each_loop() {
        let program = parse_src("for item in items { x = item; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::ForEachLoop {
            variable,
            array,
            body,
            ..
        } = &stmts[0].kind
        {
            assert_eq!(variable, "item");
            if let ExprKind::Identifier { name } = &array.kind {
                assert_eq!(name, "items");
            } else {
                panic!("expected Identifier for iterable");
            }
            assert!(!body.statements.is_empty());
        } else {
            panic!("expected ForEachLoop, got {:?}", stmts[0].kind);
        }
    }

    // 11. For-in-index loop
    #[test]
    fn test_for_in_index() {
        let program = parse_src("for i, item in items { x = item; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::ForInIndex {
            index_variable,
            value_variable,
            iterable,
            body,
        } = &stmts[0].kind
        {
            assert_eq!(index_variable, "i");
            assert_eq!(value_variable, "item");
            if let ExprKind::Identifier { name } = &iterable.kind {
                assert_eq!(name, "items");
            } else {
                panic!("expected Identifier for iterable");
            }
            assert!(!body.statements.is_empty());
        } else {
            panic!("expected ForInIndex, got {:?}", stmts[0].kind);
        }
    }

    // 12. Match expression
    #[test]
    fn test_match_expression() {
        let program = parse_src("x: int = match y { 1 => 10, _ => 0 };");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::VariableDeclaration { name, value, .. } = &stmts[0].kind {
            assert_eq!(name, "x");
            let val = value.as_ref().unwrap();
            if let ExprKind::MatchExpression { subject, arms } = &val.kind {
                if let ExprKind::Identifier { name } = &subject.kind {
                    assert_eq!(name, "y");
                } else {
                    panic!("expected Identifier as match subject");
                }
                assert_eq!(arms.len(), 2);
                assert_eq!(arms[1].pattern, MatchPattern::WildcardPattern);
            } else {
                panic!("expected MatchExpression, got {:?}", val.kind);
            }
        } else {
            panic!("expected VariableDeclaration, got {:?}", stmts[0].kind);
        }
    }

    // 13. Binary expression chaining (same operator, left-to-right)
    // Mog forbids mixing different operators without parentheses.
    // Same associative operators can chain: `1 + 2 + 3` is `(1 + 2) + 3`.
    #[test]
    fn test_binary_expression_chaining() {
        let program = parse_src("x: int = 1 + 2 + 3;");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
            let val = value.as_ref().unwrap();
            // Outermost: (1 + 2) + 3
            if let ExprKind::BinaryExpression {
                operator,
                left,
                right,
            } = &val.kind
            {
                assert_eq!(operator, "+");
                // right is 3
                if let ExprKind::NumberLiteral { value: v, .. } = &right.kind {
                    assert_eq!(v, "3");
                } else {
                    panic!("expected NumberLiteral on right");
                }
                // left is (1 + 2)
                if let ExprKind::BinaryExpression {
                    operator: inner_op, ..
                } = &left.kind
                {
                    assert_eq!(inner_op, "+");
                } else {
                    panic!("expected nested BinaryExpression on left side");
                }
            } else {
                panic!("expected BinaryExpression, got {:?}", val.kind);
            }
        } else {
            panic!("expected VariableDeclaration");
        }
    }

    // 14. Function call
    #[test]
    fn test_function_call() {
        let program = parse_src("print(\"hello\");");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
            if let ExprKind::CallExpression {
                callee, arguments, ..
            } = &expression.kind
            {
                if let ExprKind::Identifier { name } = &callee.kind {
                    assert_eq!(name, "print");
                } else {
                    panic!("expected Identifier callee");
                }
                assert_eq!(arguments.len(), 1);
                if let ExprKind::StringLiteral { value } = &arguments[0].kind {
                    assert_eq!(value, "hello");
                } else {
                    panic!("expected StringLiteral argument");
                }
            } else {
                panic!("expected CallExpression, got {:?}", expression.kind);
            }
        } else {
            panic!("expected ExpressionStatement, got {:?}", stmts[0].kind);
        }
    }

    // 15. Member expression
    #[test]
    fn test_member_expression() {
        let program = parse_src("point.x;");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
            if let ExprKind::MemberExpression { object, property } = &expression.kind {
                if let ExprKind::Identifier { name } = &object.kind {
                    assert_eq!(name, "point");
                } else {
                    panic!("expected Identifier object");
                }
                assert_eq!(property, "x");
            } else {
                panic!("expected MemberExpression, got {:?}", expression.kind);
            }
        } else {
            panic!("expected ExpressionStatement, got {:?}", stmts[0].kind);
        }
    }

    // 16. Array literal
    #[test]
    fn test_array_literal() {
        let program = parse_src("x: [int] = [1, 2, 3];");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::VariableDeclaration { name, value, .. } = &stmts[0].kind {
            assert_eq!(name, "x");
            let val = value.as_ref().unwrap();
            if let ExprKind::ArrayLiteral { elements } = &val.kind {
                assert_eq!(elements.len(), 3);
            } else {
                panic!("expected ArrayLiteral, got {:?}", val.kind);
            }
        } else {
            panic!("expected VariableDeclaration");
        }
    }

    // 17. Map literal inside parenthesized expression
    #[test]
    fn test_map_literal() {
        // Use parens to force expression context, since bare `{` starts a block
        let program = parse_src("x = ({ name: \"alice\", age: 30 });");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
            if let ExprKind::AssignmentExpression { value, .. } = &expression.kind {
                if let ExprKind::MapLiteral { entries } = &value.kind {
                    assert_eq!(entries.len(), 2);
                    assert_eq!(entries[0].key, "name");
                    assert_eq!(entries[1].key, "age");
                } else {
                    panic!("expected MapLiteral, got {:?}", value.kind);
                }
            } else {
                panic!("expected AssignmentExpression, got {:?}", expression.kind);
            }
        } else {
            panic!("expected ExpressionStatement, got {:?}", stmts[0].kind);
        }
    }

    // 18. Return statement
    #[test]
    fn test_return_statement() {
        let program = parse_src("fn foo() -> int { return 42; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::FunctionDeclaration { body, .. } = &stmts[0].kind {
            assert_eq!(body.statements.len(), 1);
            if let StatementKind::Return { value } = &body.statements[0].kind {
                assert!(value.is_some());
                if let ExprKind::NumberLiteral { value: val, .. } = &value.as_ref().unwrap().kind {
                    assert_eq!(val, "42");
                } else {
                    panic!("expected NumberLiteral in return value");
                }
            } else {
                panic!("expected Return statement");
            }
        } else {
            panic!("expected FunctionDeclaration");
        }
    }

    // 19. Async function declaration
    #[test]
    fn test_async_function() {
        let program = parse_src("async fn fetch() -> string { return \"data\"; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::AsyncFunctionDeclaration {
            name,
            params,
            is_public,
            ..
        } = &stmts[0].kind
        {
            assert_eq!(name, "fetch");
            assert!(params.is_empty());
            assert!(!is_public);
        } else {
            panic!("expected AsyncFunctionDeclaration, got {:?}", stmts[0].kind);
        }
    }

    // 20. Requires declaration
    #[test]
    fn test_requires_declaration() {
        let program = parse_src("requires fs;");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::RequiresDeclaration { capabilities } = &stmts[0].kind {
            assert_eq!(capabilities.len(), 1);
            assert_eq!(capabilities[0], "fs");
        } else {
            panic!("expected RequiresDeclaration, got {:?}", stmts[0].kind);
        }
    }

    // 21. Lambda expression
    #[test]
    fn test_lambda() {
        let program = parse_src("x: fn(int) -> int = fn(a: int) -> int { return a; };");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
            let val = value.as_ref().unwrap();
            if let ExprKind::Lambda { params, body, .. } = &val.kind {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "a");
                assert!(!body.statements.is_empty());
            } else {
                panic!("expected Lambda, got {:?}", val.kind);
            }
        } else {
            panic!("expected VariableDeclaration");
        }
    }

    // 22. String literal
    #[test]
    fn test_string_literal() {
        let program = parse_src("x: string = \"hello\";");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::VariableDeclaration { name, value, .. } = &stmts[0].kind {
            assert_eq!(name, "x");
            let val = value.as_ref().unwrap();
            if let ExprKind::StringLiteral { value: s } = &val.kind {
                assert_eq!(s, "hello");
            } else {
                panic!("expected StringLiteral, got {:?}", val.kind);
            }
        } else {
            panic!("expected VariableDeclaration");
        }
    }

    // 23. Boolean literal
    #[test]
    fn test_boolean_literal() {
        let program = parse_src("x: bool = true;");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
            let val = value.as_ref().unwrap();
            if let ExprKind::BooleanLiteral { value: b } = &val.kind {
                assert!(*b);
            } else {
                panic!("expected BooleanLiteral, got {:?}", val.kind);
            }
        } else {
            panic!("expected VariableDeclaration");
        }
    }

    // 24. Multiple statements in program
    #[test]
    fn test_multiple_statements() {
        let program = parse_src("x: int = 1; y: int = 2;");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 2);
        if let StatementKind::VariableDeclaration { name, .. } = &stmts[0].kind {
            assert_eq!(name, "x");
        } else {
            panic!("expected first VariableDeclaration");
        }
        if let StatementKind::VariableDeclaration { name, .. } = &stmts[1].kind {
            assert_eq!(name, "y");
        } else {
            panic!("expected second VariableDeclaration");
        }
    }

    // 25. Empty program
    #[test]
    fn test_empty_program() {
        let program = parse_src("");
        if let StatementKind::Program { statements, .. } = &program.kind {
            assert!(statements.is_empty());
        } else {
            panic!("expected Program");
        }
    }

    // 26. Break and continue
    #[test]
    fn test_break_and_continue() {
        let program = parse_src("while true { break; continue; }");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::WhileLoop { body, .. } = &stmts[0].kind {
            assert_eq!(body.statements.len(), 2);
            assert!(matches!(body.statements[0].kind, StatementKind::Break));
            assert!(matches!(body.statements[1].kind, StatementKind::Continue));
        } else {
            panic!("expected WhileLoop");
        }
    }

    // 27. Index expression
    #[test]
    fn test_index_expression() {
        let program = parse_src("arr[0];");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
            if let ExprKind::IndexExpression { object, index } = &expression.kind {
                if let ExprKind::Identifier { name } = &object.kind {
                    assert_eq!(name, "arr");
                } else {
                    panic!("expected Identifier for indexed object");
                }
                if let ExprKind::NumberLiteral { value, .. } = &index.kind {
                    assert_eq!(value, "0");
                } else {
                    panic!("expected NumberLiteral for index");
                }
            } else {
                panic!("expected IndexExpression, got {:?}", expression.kind);
            }
        } else {
            panic!("expected ExpressionStatement");
        }
    }

    // 28. Unary expression
    #[test]
    fn test_unary_expression() {
        let program = parse_src("x: int = -5;");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::VariableDeclaration { value, .. } = &stmts[0].kind {
            let val = value.as_ref().unwrap();
            if let ExprKind::UnaryExpression { operator, operand } = &val.kind {
                assert_eq!(operator, "-");
                if let ExprKind::NumberLiteral { value: v, .. } = &operand.kind {
                    assert_eq!(v, "5");
                } else {
                    panic!("expected NumberLiteral in unary operand");
                }
            } else {
                panic!("expected UnaryExpression, got {:?}", val.kind);
            }
        } else {
            panic!("expected VariableDeclaration");
        }
    }

    // 29. Requires with multiple capabilities
    #[test]
    fn test_requires_multiple_capabilities() {
        let program = parse_src("requires fs, net;");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::RequiresDeclaration { capabilities } = &stmts[0].kind {
            assert_eq!(capabilities.len(), 2);
            assert_eq!(capabilities[0], "fs");
            assert_eq!(capabilities[1], "net");
        } else {
            panic!("expected RequiresDeclaration");
        }
    }

    // 30. Nested function calls (method chaining)
    #[test]
    fn test_method_chaining() {
        let program = parse_src("foo.bar(1).baz(2);");
        let stmts = program_stmts(&program);
        assert_eq!(stmts.len(), 1);
        if let StatementKind::ExpressionStatement { expression } = &stmts[0].kind {
            // outermost should be a call to .baz(2)
            if let ExprKind::CallExpression {
                callee, arguments, ..
            } = &expression.kind
            {
                assert_eq!(arguments.len(), 1);
                // callee should be a MemberExpression: (...).baz
                if let ExprKind::MemberExpression { property, object } = &callee.kind {
                    assert_eq!(property, "baz");
                    // object should be a call to .bar(1)
                    if let ExprKind::CallExpression {
                        callee: inner_callee,
                        ..
                    } = &object.kind
                    {
                        if let ExprKind::MemberExpression { property: p, .. } = &inner_callee.kind {
                            assert_eq!(p, "bar");
                        } else {
                            panic!("expected inner MemberExpression");
                        }
                    } else {
                        panic!("expected inner CallExpression");
                    }
                } else {
                    panic!("expected MemberExpression, got {:?}", callee.kind);
                }
            } else {
                panic!("expected CallExpression, got {:?}", expression.kind);
            }
        } else {
            panic!("expected ExpressionStatement");
        }
    }
}
