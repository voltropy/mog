//! Capability declaration parser for `.mogdecl` files.
//!
//! Parses declarations like:
//! ```text
//! capability fs {
//!   fn read_file(path: string) -> string;
//!   async fn sleep(ms: int) -> int;
//! }
//! ```

use crate::types::Type;

// ── Public types ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CapabilityDecl {
    pub name: String,
    pub functions: Vec<CapabilityFn>,
}

#[derive(Debug, Clone)]
pub struct CapabilityFn {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub return_type: Type,
    pub is_async: bool,
}

// ── Tokenizer ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum TokKind {
    Keyword,
    Ident,
    Punct,
    Eof,
}

#[derive(Debug, Clone)]
struct Tok {
    kind: TokKind,
    value: String,
}

fn tokenize_mogdecl(source: &str) -> Vec<Tok> {
    let src: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < src.len() {
        // Skip whitespace
        if src[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Skip comments (// and #)
        if (src[i] == '/' && i + 1 < src.len() && src[i + 1] == '/') || src[i] == '#' {
            while i < src.len() && src[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Punctuation
        if "{};,()->?<>:".contains(src[i]) {
            // Handle -> as a single token
            if src[i] == '-' && i + 1 < src.len() && src[i + 1] == '>' {
                tokens.push(Tok {
                    kind: TokKind::Punct,
                    value: "->".into(),
                });
                i += 2;
                continue;
            }
            tokens.push(Tok {
                kind: TokKind::Punct,
                value: src[i].to_string(),
            });
            i += 1;
            continue;
        }

        // Identifiers / keywords
        if src[i].is_ascii_alphabetic() || src[i] == '_' {
            let start = i;
            while i < src.len() && (src[i].is_ascii_alphanumeric() || src[i] == '_') {
                i += 1;
            }
            let word: String = src[start..i].iter().collect();
            let kind = match word.as_str() {
                "capability" | "fn" | "async" | "Result" => TokKind::Keyword,
                _ => TokKind::Ident,
            };
            tokens.push(Tok { kind, value: word });
            continue;
        }

        // Skip unknown characters
        i += 1;
    }

    tokens.push(Tok {
        kind: TokKind::Eof,
        value: String::new(),
    });
    tokens
}

// ── Parser ───────────────────────────────────────────────────────────

struct DeclParser {
    tokens: Vec<Tok>,
    pos: usize,
    eof: Tok,
}

impl DeclParser {
    fn new(tokens: Vec<Tok>) -> Self {
        Self {
            tokens,
            pos: 0,
            eof: Tok {
                kind: TokKind::Eof,
                value: String::new(),
            },
        }
    }

    fn peek(&self) -> &Tok {
        self.tokens.get(self.pos).unwrap_or(&self.eof)
    }

    fn advance(&mut self) -> Tok {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Tok {
            kind: TokKind::Eof,
            value: String::new(),
        });
        self.pos += 1;
        tok
    }

    fn expect(&mut self, kind: TokKind, value: Option<&str>) -> Result<Tok, String> {
        let tok = self.advance();
        if tok.kind != kind {
            return Err(format!(
                "Expected {:?}{}, got {:?} '{}'",
                kind,
                value.map_or(String::new(), |v| format!(" '{}'", v)),
                tok.kind,
                tok.value,
            ));
        }
        if let Some(v) = value {
            if tok.value != v {
                return Err(format!("Expected '{}', got '{}'", v, tok.value));
            }
        }
        Ok(tok)
    }

    fn parse(&mut self) -> Result<Vec<CapabilityDecl>, String> {
        let mut decls = Vec::new();
        while self.peek().kind != TokKind::Eof {
            if self.peek().value == "capability" {
                decls.push(self.parse_capability()?);
            } else {
                self.advance();
            }
        }
        Ok(decls)
    }

    fn parse_capability(&mut self) -> Result<CapabilityDecl, String> {
        self.expect(TokKind::Keyword, Some("capability"))?;
        let name = self.expect(TokKind::Ident, None)?.value;
        self.expect(TokKind::Punct, Some("{"))?;

        let mut functions = Vec::new();
        while self.peek().value != "}" {
            if self.peek().kind == TokKind::Eof {
                break;
            }
            functions.push(self.parse_function()?);
        }

        self.expect(TokKind::Punct, Some("}"))?;
        Ok(CapabilityDecl { name, functions })
    }

    fn parse_function(&mut self) -> Result<CapabilityFn, String> {
        let mut is_async = false;
        if self.peek().value == "async" {
            self.advance();
            is_async = true;
        }

        self.expect(TokKind::Keyword, Some("fn"))?;
        let name = self.expect(TokKind::Ident, None)?.value;
        self.expect(TokKind::Punct, Some("("))?;

        let mut params = Vec::new();
        while self.peek().value != ")" {
            if !params.is_empty() {
                self.expect(TokKind::Punct, Some(","))?;
            }
            let param_name = self.expect(TokKind::Ident, None)?.value;
            self.expect(TokKind::Punct, Some(":"))?;

            // Check for ?Type (optional) — consume the ? but note it
            if self.peek().value == "?" {
                self.advance();
            }

            let param_type = self.parse_type_name()?;
            params.push((param_name, param_type));
        }

        self.expect(TokKind::Punct, Some(")"))?;

        // Return type
        let return_type = if self.peek().value == "->" {
            self.advance();
            self.parse_type_name()?
        } else {
            Type::Void
        };

        // Consume optional semicolon
        if self.peek().value == ";" {
            self.advance();
        }

        Ok(CapabilityFn {
            name,
            params,
            return_type,
            is_async,
        })
    }

    fn parse_type_name(&mut self) -> Result<Type, String> {
        // Handle Result<T>
        if self.peek().value == "Result" {
            self.advance();
            if self.peek().value == "<" {
                self.advance();
                let inner = self.parse_type_name()?;
                self.expect(TokKind::Punct, Some(">"))?;
                return Ok(Type::Result(Box::new(inner)));
            }
            return Ok(Type::Result(Box::new(Type::Void)));
        }

        let tok = self.advance();
        Ok(mogdecl_type_to_type(&tok.value))
    }
}

// ── Type mapping ─────────────────────────────────────────────────────

/// Convert a mogdecl type name to the Mog `Type`.
pub fn mogdecl_type_to_type(name: &str) -> Type {
    match name {
        "int" => Type::int(),
        "float" => Type::float(),
        "bool" => Type::Bool,
        "string" => Type::String,
        "none" => Type::Void,
        other => {
            // Handle Result<T> string form
            if let Some(inner) = other
                .strip_prefix("Result<")
                .and_then(|s| s.strip_suffix('>'))
            {
                return Type::Result(Box::new(mogdecl_type_to_type(inner)));
            }
            Type::Custom(other.to_string())
        }
    }
}

// ── Public API ───────────────────────────────────────────────────────

/// Parse a `.mogdecl` file and return all capability declarations found.
pub fn parse_capability_decls(content: &str) -> Result<Vec<CapabilityDecl>, String> {
    let tokens = tokenize_mogdecl(content);
    let mut parser = DeclParser::new(tokens);
    parser.parse()
}

/// Parse a `.mogdecl` file and return the first capability, or `None`.
pub fn parse_capability_decl(content: &str) -> Option<CapabilityDecl> {
    let decls = parse_capability_decls(content).ok()?;
    decls.into_iter().next()
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_process_capability() {
        let src = r#"
            capability process {
                async fn sleep(ms: int) -> int;
                fn getenv(name: string) -> string;
                fn exit(code: int) -> int;
            }
        "#;
        let decl = parse_capability_decl(src).expect("should parse");
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
    }

    #[test]
    fn parse_log_capability_no_return_types() {
        let src = r#"
            capability log {
                fn info(message: string);
                fn warn(message: string);
            }
        "#;
        let decl = parse_capability_decl(src).expect("should parse");
        assert_eq!(decl.name, "log");
        assert_eq!(decl.functions.len(), 2);
        assert_eq!(decl.functions[0].return_type, Type::Void);
    }

    #[test]
    fn parse_with_comments() {
        let src = r#"
            // This is a comment
            # Another comment style
            capability http {
                async fn get(url: string) -> string;
            }
        "#;
        let decl = parse_capability_decl(src).expect("should parse");
        assert_eq!(decl.name, "http");
        assert_eq!(decl.functions.len(), 1);
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(parse_capability_decl("").is_none());
        assert!(parse_capability_decl("// just a comment").is_none());
    }

    #[test]
    fn mogdecl_type_mapping() {
        assert_eq!(mogdecl_type_to_type("int"), Type::int());
        assert_eq!(mogdecl_type_to_type("float"), Type::float());
        assert_eq!(mogdecl_type_to_type("bool"), Type::Bool);
        assert_eq!(mogdecl_type_to_type("string"), Type::String);
        assert_eq!(mogdecl_type_to_type("none"), Type::Void);
        assert_eq!(
            mogdecl_type_to_type("Result<int>"),
            Type::Result(Box::new(Type::int()))
        );
    }
}
