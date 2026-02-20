use crate::token::{Position, Span, Token, TokenType};

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

struct Lexer<'a> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
    line: u32,
    column: u32,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn current_position(&self) -> Position {
        Position {
            line: self.line,
            column: self.column,
            index: self.pos as u32,
        }
    }

    fn peek(&self) -> u8 {
        if self.pos < self.bytes.len() {
            self.bytes[self.pos]
        } else {
            0
        }
    }

    fn peek_at(&self, offset: usize) -> u8 {
        let idx = self.pos + offset;
        if idx < self.bytes.len() {
            self.bytes[idx]
        } else {
            0
        }
    }

    fn advance(&mut self, n: usize) {
        for _ in 0..n {
            if self.pos < self.bytes.len() {
                if self.bytes[self.pos] == b'\n' {
                    self.line += 1;
                    self.column = 1;
                } else {
                    self.column += 1;
                }
                self.pos += 1;
            }
        }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn peek_char(&self) -> char {
        if self.pos < self.input.len() {
            self.input[self.pos..].chars().next().unwrap_or('\0')
        } else {
            '\0'
        }
    }

    /// Check if the remaining input starts with a keyword followed by a non-word char.
    fn match_keyword(&self, kw: &str) -> bool {
        let rem = self.remaining();
        if rem.starts_with(kw) {
            let next_idx = kw.len();
            if next_idx >= rem.len() {
                return true;
            }
            let next_byte = rem.as_bytes()[next_idx];
            !is_word_char(next_byte)
        } else {
            false
        }
    }

    /// Try to match a type token at the current position.
    /// Types: i8, i16, i32, i64, i128, i256, u8, u16, u32, u64, u128, u256,
    ///        f8, f16, f32, f64, f128, f256, bf16, int, float, bool, ptr, string
    /// Optionally followed by [] array suffixes.
    fn match_type(&self) -> Option<usize> {
        let rem = self.remaining();
        let bytes = rem.as_bytes();
        if bytes.is_empty() {
            return None;
        }

        let base_len = match bytes[0] {
            b'i' | b'u' | b'f' => {
                // Try i8, i16, i32, i64, i128, i256 etc.
                let num_start = 1;
                let mut i = num_start;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if i > num_start {
                    let num_str = &rem[num_start..i];
                    match num_str {
                        "8" | "16" | "32" | "64" | "128" | "256" => Some(i),
                        _ => None,
                    }
                } else {
                    // Check for "int"
                    if rem.starts_with("int") {
                        Some(3)
                    } else {
                        None
                    }
                }
            }
            b'b' => {
                if rem.starts_with("bf16") {
                    Some(4)
                } else if rem.starts_with("bool") {
                    Some(4)
                } else {
                    None
                }
            }
            b'p' => {
                if rem.starts_with("ptr") {
                    Some(3)
                } else {
                    None
                }
            }
            b's' => {
                if rem.starts_with("string") {
                    Some(6)
                } else {
                    None
                }
            }
            _ => None,
        };

        let base_len = base_len?;

        // Must be followed by non-word char (or end) to be a type
        if base_len < bytes.len() && is_word_char(bytes[base_len]) {
            // Exception: "float" starts with "f" but the digit check fails.
            // Also check for "float" explicitly.
            return None;
        }

        // Check for array suffixes []
        let mut end = base_len;
        while end + 1 < bytes.len() && bytes[end] == b'[' && bytes[end + 1] == b']' {
            end += 2;
        }

        // After array suffixes, must be followed by non-word char
        if end < bytes.len() && is_word_char(bytes[end]) {
            return None;
        }

        Some(end)
    }

    /// Match a number literal. Returns the length if matched.
    /// Supports: integers, floats with decimal point, scientific notation,
    /// hex (0x...), binary (0b...), octal (0o...)
    fn match_number(&self) -> Option<usize> {
        let rem = self.remaining();
        let bytes = rem.as_bytes();
        if bytes.is_empty() || !bytes[0].is_ascii_digit() {
            return None;
        }

        let mut i = 0;

        // Check for hex, binary, octal
        if bytes[0] == b'0' && i + 1 < bytes.len() {
            match bytes[1] {
                b'x' | b'X' => {
                    i = 2;
                    while i < bytes.len() && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
                        i += 1;
                    }
                    return if i > 2 { Some(i) } else { None };
                }
                b'b' | b'B' => {
                    i = 2;
                    while i < bytes.len()
                        && (bytes[i] == b'0' || bytes[i] == b'1' || bytes[i] == b'_')
                    {
                        i += 1;
                    }
                    return if i > 2 { Some(i) } else { None };
                }
                b'o' | b'O' => {
                    i = 2;
                    while i < bytes.len()
                        && ((bytes[i] >= b'0' && bytes[i] <= b'7') || bytes[i] == b'_')
                    {
                        i += 1;
                    }
                    return if i > 2 { Some(i) } else { None };
                }
                _ => {}
            }
        }

        // Regular integer/float
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }

        // Optional decimal point
        if i < bytes.len() && bytes[i] == b'.' {
            // Check it's not a range (..)
            if i + 1 < bytes.len() && bytes[i + 1] == b'.' {
                // It's a range, stop here
            } else {
                i += 1; // consume '.'
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
        }

        // Optional exponent
        if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
            let mut j = i + 1;
            if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
                j += 1;
            }
            if j < bytes.len() && bytes[j].is_ascii_digit() {
                i = j;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
        }

        if i > 0 {
            Some(i)
        } else {
            None
        }
    }

    /// Match an identifier [a-zA-Z_][a-zA-Z0-9_]*
    fn match_identifier(&self) -> Option<usize> {
        let bytes = self.remaining().as_bytes();
        if bytes.is_empty() {
            return None;
        }
        if !is_ident_start(bytes[0]) {
            return None;
        }
        let mut i = 1;
        while i < bytes.len() && is_word_char(bytes[i]) {
            i += 1;
        }
        Some(i)
    }

    fn make_token(&self, token_type: TokenType, value: String, start: Position) -> Token {
        Token {
            token_type,
            value,
            span: Span {
                start,
                end: self.current_position(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Main tokenize loop
    // -----------------------------------------------------------------------

    fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens: Vec<Token> = Vec::new();

        while !self.at_end() {
            let start_pos = self.current_position();
            let ch = self.peek();

            // ----- Whitespace -----
            if ch == b' ' || ch == b'\t' || ch == b'\n' || ch == b'\r' {
                let start = self.pos;
                while !self.at_end() {
                    let c = self.peek();
                    if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
                        self.advance(1);
                    } else {
                        break;
                    }
                }
                let value = self.input[start..self.pos].to_string();
                tokens.push(self.make_token(TokenType::Whitespace, value, start_pos));
                continue;
            }

            // ----- Single-line comment: // or # -----
            if (ch == b'/' && self.peek_at(1) == b'/') || ch == b'#' {
                let start = self.pos;
                while !self.at_end() && self.peek() != b'\n' {
                    self.advance(1);
                }
                let value = self.input[start..self.pos].to_string();
                tokens.push(self.make_token(TokenType::Comment, value, start_pos));
                continue;
            }

            // ----- Multi-line comment: /* ... */ (nested) -----
            if ch == b'/' && self.peek_at(1) == b'*' {
                let mut comment_value = String::from("/*");
                self.advance(2);
                let mut depth: u32 = 1;
                while !self.at_end() && depth > 0 {
                    if self.peek() == b'/' && self.peek_at(1) == b'*' {
                        comment_value.push_str("/*");
                        self.advance(2);
                        depth += 1;
                    } else if self.peek() == b'*' && self.peek_at(1) == b'/' {
                        comment_value.push_str("*/");
                        self.advance(2);
                        depth -= 1;
                    } else {
                        comment_value.push(self.peek_char());
                        self.advance(1);
                    }
                }
                let tt = if depth > 0 {
                    TokenType::Unknown
                } else {
                    TokenType::Comment
                };
                tokens.push(self.make_token(tt, comment_value, start_pos));
                continue;
            }

            // ----- Keywords (checked before identifiers) -----
            // We need to check that the current position is at a word boundary.
            // The TS lexer uses \b anchors in its regexes, and keywords are
            // checked before identifiers, so we replicate the same order.
            //
            // The TS lexer checks keywords *first* (before type/number/identifier).
            // Each keyword regex uses /keyword\b/y which matches only if the word
            // starts at the current position. We need to verify the char before is
            // not a word char (the sticky regex implicitly starts at pos).
            //
            // Actually, the TS regex /fn\b/y with lastIndex set to pos matches only
            // if input[pos..] starts with "fn" followed by a non-word char. There's
            // no lookbehind. So "xfn" at pos 1 would match "fn". But in the TS lexer,
            // identifiers are consumed fully, so we'd never be in the middle of a word.
            // For safety, we replicate the exact TS order.

            if is_ident_start(ch) {
                // Could be keyword, type, LLM, or identifier
                // Try keywords first (in the EXACT order the TS lexer checks them)
                if let Some(tok) = self.try_keyword_or_special(&mut tokens, start_pos) {
                    tokens.push(tok);
                    continue;
                }

                // ----- F-string: f"..." or f'...' -----
                if ch == b'f' && (self.peek_at(1) == b'"' || self.peek_at(1) == b'\'') {
                    self.lex_fstring(&mut tokens, start_pos);
                    continue;
                }

                // ----- Type tokens -----
                if let Some(len) = self.match_type() {
                    let value = self.input[self.pos..self.pos + len].to_string();
                    self.advance(len);
                    tokens.push(self.make_token(TokenType::TypeToken, value, start_pos));
                    continue;
                }

                // ----- Number starting with digit handled below -----
                // (identifiers can't start with digits, so no conflict)

                // ----- Identifier (or underscore) -----
                if let Some(len) = self.match_identifier() {
                    let value = self.input[self.pos..self.pos + len].to_string();
                    self.advance(len);
                    let tt = if value == "_" {
                        TokenType::Underscore
                    } else {
                        TokenType::Identifier
                    };
                    tokens.push(self.make_token(tt, value, start_pos));
                    continue;
                }
            }

            // ----- Multi-char operators (check longer ones first) -----
            // =>
            if ch == b'=' && self.peek_at(1) == b'>' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::FatArrow, "=>".into(), start_pos));
                continue;
            }
            // !=
            if ch == b'!' && self.peek_at(1) == b'=' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::NotEqual, "!=".into(), start_pos));
                continue;
            }
            // ==
            if ch == b'=' && self.peek_at(1) == b'=' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::Equal, "==".into(), start_pos));
                continue;
            }
            // :=
            if ch == b':' && self.peek_at(1) == b'=' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::Assign, ":=".into(), start_pos));
                continue;
            }
            // ->
            if ch == b'-' && self.peek_at(1) == b'>' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::Arrow, "->".into(), start_pos));
                continue;
            }
            // <=
            if ch == b'<' && self.peek_at(1) == b'=' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::LessEqual, "<=".into(), start_pos));
                continue;
            }
            // >=
            if ch == b'>' && self.peek_at(1) == b'=' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::GreaterEqual, ">=".into(), start_pos));
                continue;
            }

            // ----- Single-char operators -----
            // +
            if ch == b'+' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Plus, "+".into(), start_pos));
                continue;
            }
            // - (already checked ->)
            if ch == b'-' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Minus, "-".into(), start_pos));
                continue;
            }
            // *
            if ch == b'*' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Times, "*".into(), start_pos));
                continue;
            }
            // / (already checked // and /*)
            if ch == b'/' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Divide, "/".into(), start_pos));
                continue;
            }
            // %
            if ch == b'%' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Modulo, "%".into(), start_pos));
                continue;
            }
            // && (must be before &)
            if ch == b'&' && self.peek_at(1) == b'&' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::LogicalAnd, "&&".into(), start_pos));
                continue;
            }
            // || (must be before |)
            if ch == b'|' && self.peek_at(1) == b'|' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::LogicalOr, "||".into(), start_pos));
                continue;
            }
            // &
            if ch == b'&' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::BitwiseAnd, "&".into(), start_pos));
                continue;
            }
            // |
            if ch == b'|' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::BitwiseOr, "|".into(), start_pos));
                continue;
            }
            // ^
            if ch == b'^' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::BitwiseXor, "^".into(), start_pos));
                continue;
            }
            // ~
            if ch == b'~' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::BitwiseNot, "~".into(), start_pos));
                continue;
            }
            // << (must be before <)
            if ch == b'<' && self.peek_at(1) == b'<' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::LShift, "<<".into(), start_pos));
                continue;
            }
            // >> (must be before >)
            if ch == b'>' && self.peek_at(1) == b'>' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::RShift, ">>".into(), start_pos));
                continue;
            }
            // <
            if ch == b'<' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Less, "<".into(), start_pos));
                continue;
            }
            // >
            if ch == b'>' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Greater, ">".into(), start_pos));
                continue;
            }
            // = (single, already checked == and =>)
            if ch == b'=' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Equal, "=".into(), start_pos));
                continue;
            }

            // ----- Delimiters -----
            if ch == b'[' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::LBracket, "[".into(), start_pos));
                continue;
            }
            if ch == b']' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::RBracket, "]".into(), start_pos));
                continue;
            }
            if ch == b'{' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::LBrace, "{".into(), start_pos));
                continue;
            }
            if ch == b'}' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::RBrace, "}".into(), start_pos));
                continue;
            }
            if ch == b'(' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::LParen, "(".into(), start_pos));
                continue;
            }
            if ch == b')' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::RParen, ")".into(), start_pos));
                continue;
            }
            if ch == b',' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Comma, ",".into(), start_pos));
                continue;
            }
            // : (already checked :=)
            if ch == b':' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Colon, ":".into(), start_pos));
                continue;
            }
            if ch == b';' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Semicolon, ";".into(), start_pos));
                continue;
            }
            if ch == b'?' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::QuestionMark, "?".into(), start_pos));
                continue;
            }
            // .. (range, must be before .)
            if ch == b'.' && self.peek_at(1) == b'.' {
                self.advance(2);
                tokens.push(self.make_token(TokenType::Range, "..".into(), start_pos));
                continue;
            }
            // .
            if ch == b'.' {
                self.advance(1);
                tokens.push(self.make_token(TokenType::Dot, ".".into(), start_pos));
                continue;
            }

            // ----- String literals (double or single quoted) -----
            // Check for f-string first (already handled in ident branch above)
            if ch == b'"' || ch == b'\'' {
                // Check for template string: "{...}" pattern
                if ch == b'"' && self.peek_at(1) == b'{' {
                    self.lex_template_string(&mut tokens, start_pos);
                    continue;
                }
                let quote = ch;
                let start = self.pos;
                self.advance(1); // opening quote
                while !self.at_end() && self.peek() != quote {
                    if self.peek() == b'\\' {
                        self.advance(1); // backslash
                        if !self.at_end() {
                            self.advance(1); // escaped char
                        }
                    } else {
                        self.advance(1);
                    }
                }
                if !self.at_end() && self.peek() == quote {
                    self.advance(1); // closing quote
                }
                let value = self.input[start..self.pos].to_string();
                tokens.push(self.make_token(TokenType::StringLit, value, start_pos));
                continue;
            }

            // ----- Number literals -----
            if ch.is_ascii_digit() {
                if let Some(len) = self.match_number() {
                    let value = self.input[self.pos..self.pos + len].to_string();
                    self.advance(len);
                    tokens.push(self.make_token(TokenType::Number, value, start_pos));
                    continue;
                }
            }

            // ----- Unknown -----
            let c = self.peek_char();
            self.advance(1);
            tokens.push(self.make_token(TokenType::Unknown, c.to_string(), start_pos));
        }

        tokens
    }

    // -----------------------------------------------------------------------
    // Keyword / special matching (preserves TS order)
    // -----------------------------------------------------------------------

    /// Try to match a keyword at the current position. Returns Some(Token) if
    /// matched, advancing the position. Returns None if no keyword matches.
    ///
    /// The TS lexer checks keywords in a specific order before falling through
    /// to type/identifier matching. We replicate that order exactly.
    fn try_keyword_or_special(&mut self, _tokens: &[Token], start_pos: Position) -> Option<Token> {
        // The TS lexer checks keywords via sticky regexes like /fn\b/y.
        // The \b means keyword must be followed by a non-word character.
        // We check each keyword in the exact TS order.

        // Helper macro to reduce boilerplate
        macro_rules! try_kw {
            ($kw:expr, $tt:expr) => {
                if self.match_keyword($kw) {
                    let value = $kw.to_string();
                    self.advance($kw.len());
                    return Some(self.make_token($tt, value, start_pos));
                }
            };
        }

        try_kw!("fn", TokenType::Fn);
        try_kw!("return", TokenType::Return);
        try_kw!("if", TokenType::If);
        try_kw!("else", TokenType::Else);
        try_kw!("while", TokenType::While);
        try_kw!("for", TokenType::For);
        try_kw!("to", TokenType::To);
        try_kw!("in", TokenType::In);
        try_kw!("break", TokenType::Break);
        try_kw!("continue", TokenType::Continue);
        try_kw!("cast", TokenType::Cast);
        try_kw!("as", TokenType::As);
        try_kw!("not", TokenType::Not);
        try_kw!("struct", TokenType::Struct);
        try_kw!("soa", TokenType::Soa);
        try_kw!("requires", TokenType::Requires);
        try_kw!("optional", TokenType::Optional);
        // LLM is uppercase
        try_kw!("LLM", TokenType::Llm);
        try_kw!("type", TokenType::TypeKw);
        try_kw!("match", TokenType::Match);
        try_kw!("try", TokenType::Try);
        try_kw!("catch", TokenType::Catch);
        try_kw!("ok", TokenType::Ok);
        try_kw!("err", TokenType::Err);
        try_kw!("some", TokenType::Some);
        try_kw!("none", TokenType::None);
        try_kw!("is", TokenType::Is);
        try_kw!("tensor", TokenType::Tensor);
        try_kw!("async", TokenType::Async);
        try_kw!("await", TokenType::Await);
        try_kw!("spawn", TokenType::Spawn);
        try_kw!("package", TokenType::Package);
        try_kw!("import", TokenType::Import);
        try_kw!("pub", TokenType::Pub);
        try_kw!("with", TokenType::With);
        try_kw!("true", TokenType::True);
        try_kw!("false", TokenType::False);

        None
    }

    // -----------------------------------------------------------------------
    // F-string lexing: f"Hello {name}!" or f'Hello {name}!'
    // -----------------------------------------------------------------------

    fn lex_fstring(&mut self, tokens: &mut Vec<Token>, start_pos: Position) {
        self.advance(1); // consume 'f'
        let quote_char = self.peek();
        self.advance(1); // consume opening quote
        tokens.push(self.make_token(
            TokenType::TemplateStringStart,
            String::from(quote_char as char),
            start_pos,
        ));

        while !self.at_end() && self.peek() != quote_char {
            if self.peek() == b'{' {
                // Check for escaped brace {{ -> literal {
                if self.peek_at(1) == b'{' {
                    let part_start = self.current_position();
                    self.advance(2);
                    tokens.push(self.make_token(
                        TokenType::TemplateStringPart,
                        "{".into(),
                        part_start,
                    ));
                    continue;
                }
                let interp_start_pos = self.current_position();
                self.advance(1); // consume {
                tokens.push(self.make_token(
                    TokenType::TemplateInterpStart,
                    "{".into(),
                    interp_start_pos,
                ));

                // Parse the expression inside {} - collect all chars until balanced }
                let expr_start_pos = self.current_position();
                let mut brace_depth: u32 = 1;
                let mut expr_str = String::new();
                while brace_depth > 0 && !self.at_end() {
                    let c = self.peek();
                    if c == b'{' {
                        brace_depth += 1;
                        expr_str.push(c as char);
                        self.advance(1);
                    } else if c == b'}' {
                        brace_depth -= 1;
                        if brace_depth > 0 {
                            expr_str.push(c as char);
                        }
                        self.advance(1);
                    } else if c == quote_char {
                        // Unterminated expression
                        break;
                    } else {
                        expr_str.push(self.peek_char());
                        self.advance(1);
                    }
                }
                if !expr_str.is_empty() {
                    tokens.push(self.make_token(
                        TokenType::TemplateStringPart,
                        expr_str,
                        expr_start_pos,
                    ));
                }
                let interp_end_pos = self.current_position();
                tokens.push(self.make_token(
                    TokenType::TemplateInterpEnd,
                    "}".into(),
                    interp_end_pos,
                ));
            } else if self.peek() == b'}' {
                // Check for escaped brace }} -> literal }
                if self.peek_at(1) == b'}' {
                    let part_start = self.current_position();
                    self.advance(2);
                    tokens.push(self.make_token(
                        TokenType::TemplateStringPart,
                        "}".into(),
                        part_start,
                    ));
                    continue;
                }
                // Single } in f-string text - treat as literal
                let part_start = self.current_position();
                self.advance(1);
                tokens.push(self.make_token(TokenType::TemplateStringPart, "}".into(), part_start));
            } else {
                // Collect string literal part
                let part_start = self.current_position();
                let mut str_value = String::new();
                while !self.at_end() && self.peek() != b'{' && self.peek() != quote_char {
                    // Check for escaped brace }} in text - stop before it
                    if self.peek() == b'}' && self.peek_at(1) == b'}' {
                        break;
                    }
                    if self.peek() == b'\\' {
                        self.advance(1); // backslash
                        if !self.at_end() {
                            let escaped = self.peek();
                            match escaped {
                                b'n' => str_value.push('\n'),
                                b't' => str_value.push('\t'),
                                b'r' => str_value.push('\r'),
                                b'\\' => str_value.push('\\'),
                                b'{' => str_value.push('{'),
                                b'}' => str_value.push('}'),
                                _ if escaped == quote_char => {
                                    str_value.push(quote_char as char);
                                }
                                _ => {
                                    str_value.push(escaped as char);
                                }
                            }
                            self.advance(1);
                        }
                    } else {
                        str_value.push(self.peek_char());
                        self.advance(1);
                    }
                }
                if !str_value.is_empty() {
                    tokens.push(self.make_token(
                        TokenType::TemplateStringPart,
                        str_value,
                        part_start,
                    ));
                }
            }
        }

        // Consume closing quote
        if !self.at_end() && self.peek() == quote_char {
            let end_start = self.current_position();
            self.advance(1);
            tokens.push(self.make_token(
                TokenType::TemplateStringEnd,
                String::from(quote_char as char),
                end_start,
            ));
        }
    }

    // -----------------------------------------------------------------------
    // Template string lexing: "{expr}..."
    // -----------------------------------------------------------------------

    fn lex_template_string(&mut self, tokens: &mut Vec<Token>, _start_pos: Position) {
        self.advance(1); // consume opening "

        while !self.at_end() && self.peek() != b'"' {
            if self.peek() == b'{' {
                let interp_start = self.current_position();
                self.advance(1); // consume {
                tokens.push(self.make_token(
                    TokenType::TemplateInterpStart,
                    "{".into(),
                    interp_start,
                ));

                // Parse expression inside {}
                while !self.at_end() && self.peek() != b'}' {
                    let expr_start = self.current_position();
                    // Skip whitespace
                    while !self.at_end() && (self.peek() == b' ' || self.peek() == b'\t') {
                        self.advance(1);
                    }
                    // Parse identifier
                    if let Some(len) = self.match_identifier() {
                        let value = self.input[self.pos..self.pos + len].to_string();
                        self.advance(len);
                        tokens.push(self.make_token(TokenType::Identifier, value, expr_start));
                        break;
                    }
                    break;
                }
                if !self.at_end() && self.peek() == b'}' {
                    let interp_end = self.current_position();
                    self.advance(1);
                    tokens.push(self.make_token(
                        TokenType::TemplateInterpEnd,
                        "}".into(),
                        interp_end,
                    ));
                }
            } else {
                // Collect string literal part
                let part_start = self.current_position();
                let mut str_value = String::new();
                while !self.at_end() && self.peek() != b'{' && self.peek() != b'"' {
                    if self.peek() == b'\\' {
                        self.advance(1);
                        if !self.at_end() {
                            str_value.push(self.peek_char());
                            self.advance(1);
                        }
                    } else {
                        str_value.push(self.peek_char());
                        self.advance(1);
                    }
                }
                if !str_value.is_empty() {
                    tokens.push(self.make_token(
                        TokenType::TemplateStringPart,
                        str_value,
                        part_start,
                    ));
                }
            }
        }

        if !self.at_end() && self.peek() == b'"' {
            let end_start = self.current_position();
            self.advance(1);
            tokens.push(self.make_token(TokenType::TemplateStringEnd, "\"".into(), end_start));
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(input);
    lexer.tokenize()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_whitespace() {
        let tokens = tokenize("  \n\t");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Whitespace);
        assert_eq!(tokens[0].value, "  \n\t");
    }

    #[test]
    fn test_keywords() {
        let tokens = tokenize("fn return if else while for");
        let kw_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(kw_tokens.len(), 6);
        assert_eq!(kw_tokens[0].token_type, TokenType::Fn);
        assert_eq!(kw_tokens[1].token_type, TokenType::Return);
        assert_eq!(kw_tokens[2].token_type, TokenType::If);
        assert_eq!(kw_tokens[3].token_type, TokenType::Else);
        assert_eq!(kw_tokens[4].token_type, TokenType::While);
        assert_eq!(kw_tokens[5].token_type, TokenType::For);
    }

    #[test]
    fn test_operators() {
        let tokens = tokenize("+ - * / == != <= >= := -> =>");
        let op_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(op_tokens[0].token_type, TokenType::Plus);
        assert_eq!(op_tokens[1].token_type, TokenType::Minus);
        assert_eq!(op_tokens[2].token_type, TokenType::Times);
        assert_eq!(op_tokens[3].token_type, TokenType::Divide);
        assert_eq!(op_tokens[4].token_type, TokenType::Equal);
        assert_eq!(op_tokens[5].token_type, TokenType::NotEqual);
        assert_eq!(op_tokens[6].token_type, TokenType::LessEqual);
        assert_eq!(op_tokens[7].token_type, TokenType::GreaterEqual);
        assert_eq!(op_tokens[8].token_type, TokenType::Assign);
        assert_eq!(op_tokens[9].token_type, TokenType::Arrow);
        assert_eq!(op_tokens[10].token_type, TokenType::FatArrow);
    }

    #[test]
    fn test_numbers() {
        let tokens = tokenize("42 3.14 0xFF 0b1010 0o77 1e10");
        let num_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(num_tokens.len(), 6);
        for t in &num_tokens {
            assert_eq!(t.token_type, TokenType::Number);
        }
        assert_eq!(num_tokens[0].value, "42");
        assert_eq!(num_tokens[1].value, "3.14");
        assert_eq!(num_tokens[2].value, "0xFF");
        assert_eq!(num_tokens[3].value, "0b1010");
        assert_eq!(num_tokens[4].value, "0o77");
        assert_eq!(num_tokens[5].value, "1e10");
    }

    #[test]
    fn test_string_literals() {
        let tokens = tokenize(r#""hello" 'world'"#);
        let str_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(str_tokens.len(), 2);
        assert_eq!(str_tokens[0].token_type, TokenType::StringLit);
        assert_eq!(str_tokens[0].value, "\"hello\"");
        assert_eq!(str_tokens[1].token_type, TokenType::StringLit);
        assert_eq!(str_tokens[1].value, "'world'");
    }

    #[test]
    fn test_single_line_comment() {
        let tokens = tokenize("// this is a comment\n42");
        assert_eq!(tokens[0].token_type, TokenType::Comment);
        assert_eq!(tokens[0].value, "// this is a comment");
    }

    #[test]
    fn test_multi_line_comment() {
        let tokens = tokenize("/* hello */");
        assert_eq!(tokens[0].token_type, TokenType::Comment);
        assert_eq!(tokens[0].value, "/* hello */");
    }

    #[test]
    fn test_nested_comment() {
        let tokens = tokenize("/* outer /* inner */ still outer */");
        assert_eq!(tokens[0].token_type, TokenType::Comment);
        assert_eq!(tokens[0].value, "/* outer /* inner */ still outer */");
    }

    #[test]
    fn test_types() {
        let tokens = tokenize("i32 u64 f32 bool string ptr i32[]");
        let type_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(type_tokens.len(), 7);
        for t in &type_tokens {
            assert_eq!(
                t.token_type,
                TokenType::TypeToken,
                "failed for {:?}",
                t.value
            );
        }
    }

    #[test]
    fn test_identifier_not_keyword() {
        let tokens = tokenize("foobar fn_name return_value");
        let id_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(id_tokens.len(), 3);
        for t in &id_tokens {
            assert_eq!(t.token_type, TokenType::Identifier);
        }
    }

    #[test]
    fn test_underscore() {
        let tokens = tokenize("_");
        assert_eq!(tokens[0].token_type, TokenType::Underscore);
    }

    #[test]
    fn test_fstring() {
        let tokens = tokenize(r#"f"hello {name}""#);
        let non_ws: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(non_ws[0].token_type, TokenType::TemplateStringStart);
        assert_eq!(non_ws[1].token_type, TokenType::TemplateStringPart);
        assert_eq!(non_ws[1].value, "hello ");
        assert_eq!(non_ws[2].token_type, TokenType::TemplateInterpStart);
        assert_eq!(non_ws[3].token_type, TokenType::TemplateStringPart);
        assert_eq!(non_ws[3].value, "name");
        assert_eq!(non_ws[4].token_type, TokenType::TemplateInterpEnd);
        assert_eq!(non_ws[5].token_type, TokenType::TemplateStringEnd);
    }

    #[test]
    fn test_delimiters() {
        let tokens = tokenize("()[]{},:;.");
        let types: Vec<_> = tokens.iter().map(|t| t.token_type).collect();
        assert_eq!(
            types,
            vec![
                TokenType::LParen,
                TokenType::RParen,
                TokenType::LBracket,
                TokenType::RBracket,
                TokenType::LBrace,
                TokenType::RBrace,
                TokenType::Comma,
                TokenType::Colon,
                TokenType::Semicolon,
                TokenType::Dot,
            ]
        );
    }

    #[test]
    fn test_range() {
        let tokens = tokenize("0..10");
        let non_ws: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(non_ws[0].token_type, TokenType::Number);
        assert_eq!(non_ws[0].value, "0");
        assert_eq!(non_ws[1].token_type, TokenType::Range);
        assert_eq!(non_ws[2].token_type, TokenType::Number);
        assert_eq!(non_ws[2].value, "10");
    }

    #[test]
    fn test_position_tracking() {
        let tokens = tokenize("fn foo");
        assert_eq!(tokens[0].span.start.line, 1);
        assert_eq!(tokens[0].span.start.column, 1);
        // "fn" ends at column 3 (exclusive)
        assert_eq!(tokens[0].span.end.column, 3);
    }

    #[test]
    fn test_hash_comment() {
        let tokens = tokenize("# comment\n42");
        assert_eq!(tokens[0].token_type, TokenType::Comment);
        assert_eq!(tokens[0].value, "# comment");
    }

    #[test]
    fn test_bitwise_operators() {
        let tokens = tokenize("& | ^ ~ << >>");
        let op_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(op_tokens[0].token_type, TokenType::BitwiseAnd);
        assert_eq!(op_tokens[1].token_type, TokenType::BitwiseOr);
        assert_eq!(op_tokens[2].token_type, TokenType::BitwiseXor);
        assert_eq!(op_tokens[3].token_type, TokenType::BitwiseNot);
        assert_eq!(op_tokens[4].token_type, TokenType::LShift);
        assert_eq!(op_tokens[5].token_type, TokenType::RShift);
    }

    #[test]
    fn test_logical_operators() {
        let tokens = tokenize("&& ||");
        let op_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(op_tokens[0].token_type, TokenType::LogicalAnd);
        assert_eq!(op_tokens[1].token_type, TokenType::LogicalOr);
    }

    #[test]
    fn test_all_keywords() {
        let input = "fn return if else while for to in break continue cast as not struct soa requires optional LLM type match try catch ok err some none is tensor async await spawn package import pub with true false";
        let tokens = tokenize(input);
        let kw_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        let expected = vec![
            TokenType::Fn,
            TokenType::Return,
            TokenType::If,
            TokenType::Else,
            TokenType::While,
            TokenType::For,
            TokenType::To,
            TokenType::In,
            TokenType::Break,
            TokenType::Continue,
            TokenType::Cast,
            TokenType::As,
            TokenType::Not,
            TokenType::Struct,
            TokenType::Soa,
            TokenType::Requires,
            TokenType::Optional,
            TokenType::Llm,
            TokenType::TypeKw,
            TokenType::Match,
            TokenType::Try,
            TokenType::Catch,
            TokenType::Ok,
            TokenType::Err,
            TokenType::Some,
            TokenType::None,
            TokenType::Is,
            TokenType::Tensor,
            TokenType::Async,
            TokenType::Await,
            TokenType::Spawn,
            TokenType::Package,
            TokenType::Import,
            TokenType::Pub,
            TokenType::With,
            TokenType::True,
            TokenType::False,
        ];
        assert_eq!(kw_tokens.len(), expected.len());
        for (i, t) in kw_tokens.iter().enumerate() {
            assert_eq!(t.token_type, expected[i], "keyword {} mismatch", t.value);
        }
    }

    #[test]
    fn test_float_no_trailing_digits() {
        // "3." should parse as number "3."
        let tokens = tokenize("3.");
        let non_ws: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type != TokenType::Whitespace)
            .collect();
        assert_eq!(non_ws[0].token_type, TokenType::Number);
        assert_eq!(non_ws[0].value, "3.");
    }

    #[test]
    fn test_string_with_escapes() {
        let tokens = tokenize(r#""hello\nworld""#);
        assert_eq!(tokens[0].token_type, TokenType::StringLit);
        assert_eq!(tokens[0].value, r#""hello\nworld""#);
    }

    #[test]
    fn test_question_mark() {
        let tokens = tokenize("?");
        assert_eq!(tokens[0].token_type, TokenType::QuestionMark);
    }

    #[test]
    fn test_bf16_type() {
        let tokens = tokenize("bf16");
        assert_eq!(tokens[0].token_type, TokenType::TypeToken);
        assert_eq!(tokens[0].value, "bf16");
    }

    #[test]
    fn test_type_with_array_suffix() {
        let tokens = tokenize("i32[][]");
        assert_eq!(tokens[0].token_type, TokenType::TypeToken);
        assert_eq!(tokens[0].value, "i32[][]");
    }
}
