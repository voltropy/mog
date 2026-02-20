use std::fmt;

// ---------------------------------------------------------------------------
// Position & Span
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub line: u32,
    pub column: u32,
    pub index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

// ---------------------------------------------------------------------------
// TokenType – 85 variants
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    // Keywords (37)
    Fn,
    Return,
    If,
    Else,
    While,
    For,
    To,
    In,
    Break,
    Continue,
    Cast,
    As,
    Not,
    Struct,
    Soa,
    Requires,
    Optional,
    TypeKw,
    True,
    False,
    Match,
    Try,
    Catch,
    Ok,
    Err,
    Some,
    None,
    Is,
    Tensor,
    Async,
    Await,
    Spawn,
    Package,
    Import,
    Pub,
    With,
    Llm,

    // Operators (25)
    Plus,
    Minus,
    Times,
    Divide,
    Modulo,
    Less,
    Greater,
    Equal,
    NotEqual,
    LessEqual,
    GreaterEqual,
    Assign,
    LogicalAnd,
    LogicalOr,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitwiseNot,
    LShift,
    RShift,
    Arrow,
    FatArrow,
    Range,
    QuestionMark,
    Underscore,

    // Delimiters (10)
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    LParen,
    RParen,
    Comma,
    Colon,
    Semicolon,
    Dot,

    // Literals & Special (13)
    Identifier,
    Number,
    StringLit,
    TemplateStringStart,
    TemplateStringPart,
    TemplateStringEnd,
    TemplateInterpStart,
    TemplateInterpEnd,
    TypeToken,
    Unknown,
    Whitespace,
    Comment,
}

// ---------------------------------------------------------------------------
// TokenType – keyword lookup
// ---------------------------------------------------------------------------

impl TokenType {
    pub fn keyword_from_str(s: &str) -> Option<TokenType> {
        match s {
            "fn" => Some(TokenType::Fn),
            "return" => Some(TokenType::Return),
            "if" => Some(TokenType::If),
            "else" => Some(TokenType::Else),
            "while" => Some(TokenType::While),
            "for" => Some(TokenType::For),
            "to" => Some(TokenType::To),
            "in" => Some(TokenType::In),
            "break" => Some(TokenType::Break),
            "continue" => Some(TokenType::Continue),
            "cast" => Some(TokenType::Cast),
            "as" => Some(TokenType::As),
            "not" => Some(TokenType::Not),
            "struct" => Some(TokenType::Struct),
            "soa" => Some(TokenType::Soa),
            "requires" => Some(TokenType::Requires),
            "optional" => Some(TokenType::Optional),
            "type" => Some(TokenType::TypeKw),
            "true" => Some(TokenType::True),
            "false" => Some(TokenType::False),
            "match" => Some(TokenType::Match),
            "try" => Some(TokenType::Try),
            "catch" => Some(TokenType::Catch),
            "ok" => Some(TokenType::Ok),
            "err" => Some(TokenType::Err),
            "some" => Some(TokenType::Some),
            "none" => Some(TokenType::None),
            "is" => Some(TokenType::Is),
            "tensor" => Some(TokenType::Tensor),
            "async" => Some(TokenType::Async),
            "await" => Some(TokenType::Await),
            "spawn" => Some(TokenType::Spawn),
            "package" => Some(TokenType::Package),
            "import" => Some(TokenType::Import),
            "pub" => Some(TokenType::Pub),
            "with" => Some(TokenType::With),
            "llm" => Some(TokenType::Llm),
            _ => Option::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            // Keywords
            TokenType::Fn => "fn",
            TokenType::Return => "return",
            TokenType::If => "if",
            TokenType::Else => "else",
            TokenType::While => "while",
            TokenType::For => "for",
            TokenType::To => "to",
            TokenType::In => "in",
            TokenType::Break => "break",
            TokenType::Continue => "continue",
            TokenType::Cast => "cast",
            TokenType::As => "as",
            TokenType::Not => "not",
            TokenType::Struct => "struct",
            TokenType::Soa => "soa",
            TokenType::Requires => "requires",
            TokenType::Optional => "optional",
            TokenType::TypeKw => "type",
            TokenType::True => "true",
            TokenType::False => "false",
            TokenType::Match => "match",
            TokenType::Try => "try",
            TokenType::Catch => "catch",
            TokenType::Ok => "ok",
            TokenType::Err => "err",
            TokenType::Some => "some",
            TokenType::None => "none",
            TokenType::Is => "is",
            TokenType::Tensor => "tensor",
            TokenType::Async => "async",
            TokenType::Await => "await",
            TokenType::Spawn => "spawn",
            TokenType::Package => "package",
            TokenType::Import => "import",
            TokenType::Pub => "pub",
            TokenType::With => "with",
            TokenType::Llm => "llm",

            // Operators
            TokenType::Plus => "+",
            TokenType::Minus => "-",
            TokenType::Times => "*",
            TokenType::Divide => "/",
            TokenType::Modulo => "%",
            TokenType::Less => "<",
            TokenType::Greater => ">",
            TokenType::Equal => "==",
            TokenType::NotEqual => "!=",
            TokenType::LessEqual => "<=",
            TokenType::GreaterEqual => ">=",
            TokenType::Assign => "=",
            TokenType::LogicalAnd => "&&",
            TokenType::LogicalOr => "||",
            TokenType::BitwiseAnd => "&",
            TokenType::BitwiseOr => "|",
            TokenType::BitwiseXor => "^",
            TokenType::BitwiseNot => "~",
            TokenType::LShift => "<<",
            TokenType::RShift => ">>",
            TokenType::Arrow => "->",
            TokenType::FatArrow => "=>",
            TokenType::Range => "..",
            TokenType::QuestionMark => "?",
            TokenType::Underscore => "_",

            // Delimiters
            TokenType::LBracket => "[",
            TokenType::RBracket => "]",
            TokenType::LBrace => "{",
            TokenType::RBrace => "}",
            TokenType::LParen => "(",
            TokenType::RParen => ")",
            TokenType::Comma => ",",
            TokenType::Colon => ":",
            TokenType::Semicolon => ";",
            TokenType::Dot => ".",

            // Literals & Special
            TokenType::Identifier => "identifier",
            TokenType::Number => "number",
            TokenType::StringLit => "string",
            TokenType::TemplateStringStart => "template_string_start",
            TokenType::TemplateStringPart => "template_string_part",
            TokenType::TemplateStringEnd => "template_string_end",
            TokenType::TemplateInterpStart => "template_interp_start",
            TokenType::TemplateInterpEnd => "template_interp_end",
            TokenType::TypeToken => "type_token",
            TokenType::Unknown => "unknown",
            TokenType::Whitespace => "whitespace",
            TokenType::Comment => "comment",
        };
        write!(f, "{}", s)
    }
}

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub token_type: TokenType,
    pub value: String,
    pub span: Span,
}

impl Token {
    /// Returns `true` if this token is a language keyword.
    pub fn is_keyword(&self) -> bool {
        matches!(
            self.token_type,
            TokenType::Fn
                | TokenType::Return
                | TokenType::If
                | TokenType::Else
                | TokenType::While
                | TokenType::For
                | TokenType::To
                | TokenType::In
                | TokenType::Break
                | TokenType::Continue
                | TokenType::Cast
                | TokenType::As
                | TokenType::Not
                | TokenType::Struct
                | TokenType::Soa
                | TokenType::Requires
                | TokenType::Optional
                | TokenType::TypeKw
                | TokenType::True
                | TokenType::False
                | TokenType::Match
                | TokenType::Try
                | TokenType::Catch
                | TokenType::Ok
                | TokenType::Err
                | TokenType::Some
                | TokenType::None
                | TokenType::Is
                | TokenType::Tensor
                | TokenType::Async
                | TokenType::Await
                | TokenType::Spawn
                | TokenType::Package
                | TokenType::Import
                | TokenType::Pub
                | TokenType::With
                | TokenType::Llm
        )
    }

    /// Returns `true` if this token is an operator.
    pub fn is_operator(&self) -> bool {
        matches!(
            self.token_type,
            TokenType::Plus
                | TokenType::Minus
                | TokenType::Times
                | TokenType::Divide
                | TokenType::Modulo
                | TokenType::Less
                | TokenType::Greater
                | TokenType::Equal
                | TokenType::NotEqual
                | TokenType::LessEqual
                | TokenType::GreaterEqual
                | TokenType::Assign
                | TokenType::LogicalAnd
                | TokenType::LogicalOr
                | TokenType::BitwiseAnd
                | TokenType::BitwiseOr
                | TokenType::BitwiseXor
                | TokenType::BitwiseNot
                | TokenType::LShift
                | TokenType::RShift
                | TokenType::Arrow
                | TokenType::FatArrow
                | TokenType::Range
                | TokenType::QuestionMark
                | TokenType::Underscore
        )
    }

    /// Returns `true` if this token is a literal (number, string, or boolean).
    pub fn is_literal(&self) -> bool {
        matches!(
            self.token_type,
            TokenType::Number
                | TokenType::StringLit
                | TokenType::True
                | TokenType::False
                | TokenType::TemplateStringStart
                | TokenType::TemplateStringPart
                | TokenType::TemplateStringEnd
        )
    }
}
