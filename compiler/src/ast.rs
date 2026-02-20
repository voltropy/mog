use crate::types::Type;

// ---------------------------------------------------------------------------
// Span / Position
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub line: u32,
    pub column: u32,
    pub index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

// ---------------------------------------------------------------------------
// Top-level wrapper structs (kind + span)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Block — shared between statements and expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub scope_id: String,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    pub name: String,
    pub param_type: Type,
    pub default_value: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub field_type: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    String(String),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapEntry {
    pub key: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructFieldInit {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoAColumn {
    pub name: String,
    pub values: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedArg {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchPattern {
    LiteralPattern(Expr),
    WildcardPattern,
    VariantPattern {
        name: String,
        binding: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum IfElseBranch {
    Block(Block),
    IfExpression(Box<Expr>),
}

// ---------------------------------------------------------------------------
// StatementKind — 25 variants
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Program {
        statements: Vec<Statement>,
        scope_id: String,
    },
    PackageDeclaration {
        name: String,
    },
    ImportDeclaration {
        paths: Vec<String>,
    },
    StructDefinition {
        name: String,
        fields: Vec<FieldDef>,
        is_public: bool,
    },
    VariableDeclaration {
        name: String,
        var_type: Option<Type>,
        value: Option<Box<Expr>>,
    },
    Assignment {
        name: String,
        value: Box<Expr>,
    },
    ExpressionStatement {
        expression: Box<Expr>,
    },
    Block {
        statements: Vec<Statement>,
        scope_id: String,
    },
    Return {
        value: Option<Box<Expr>>,
    },
    Conditional {
        condition: Box<Expr>,
        true_branch: Block,
        false_branch: Option<Block>,
    },
    WhileLoop {
        test: Box<Expr>,
        body: Block,
    },
    ForLoop {
        variable: String,
        start: Box<Expr>,
        end: Box<Expr>,
        body: Block,
    },
    ForEachLoop {
        variable: String,
        var_type: Option<Type>,
        array: Box<Expr>,
        body: Block,
    },
    ForInRange {
        variable: String,
        start: Box<Expr>,
        end: Box<Expr>,
        body: Block,
    },
    ForInIndex {
        index_variable: String,
        value_variable: String,
        iterable: Box<Expr>,
        body: Block,
    },
    ForInMap {
        key_variable: String,
        value_variable: String,
        map: Box<Expr>,
        body: Block,
    },
    Break,
    Continue,
    FunctionDeclaration {
        name: String,
        params: Vec<FunctionParam>,
        return_type: Type,
        body: Block,
        is_public: bool,
    },
    AsyncFunctionDeclaration {
        name: String,
        params: Vec<FunctionParam>,
        return_type: Type,
        body: Block,
        is_public: bool,
    },
    RequiresDeclaration {
        capabilities: Vec<String>,
    },
    OptionalDeclaration {
        capabilities: Vec<String>,
    },
    TryCatch {
        try_body: Block,
        error_var: String,
        catch_body: Block,
    },
    WithBlock {
        context: Box<Expr>,
        body: Block,
    },
    TypeAliasDeclaration {
        name: String,
        aliased_type: Type,
    },
}

// ---------------------------------------------------------------------------
// ExprKind — 41 variants
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Identifier {
        name: String,
    },
    NumberLiteral {
        value: String,
        literal_type: Option<Type>,
    },
    BooleanLiteral {
        value: bool,
    },
    StringLiteral {
        value: String,
    },
    TemplateLiteral {
        parts: Vec<TemplatePart>,
    },
    ArrayLiteral {
        elements: Vec<Expr>,
    },
    ArrayFill {
        value: Box<Expr>,
        count: Box<Expr>,
    },
    MapLiteral {
        entries: Vec<MapEntry>,
    },
    StructLiteral {
        struct_name: Option<String>,
        fields: Vec<StructFieldInit>,
    },
    SoALiteral {
        columns: Vec<SoAColumn>,
    },
    SoAConstructor {
        struct_name: String,
        capacity: Option<usize>,
    },
    TensorConstruction {
        dtype: Type,
        args: Vec<Expr>,
    },
    BinaryExpression {
        operator: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryExpression {
        operator: String,
        operand: Box<Expr>,
    },
    AssignmentExpression {
        name: Option<String>,
        target: Option<Box<Expr>>,
        value: Box<Expr>,
    },
    CallExpression {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
        named_args: Option<Vec<NamedArg>>,
    },
    MemberExpression {
        object: Box<Expr>,
        property: String,
    },
    IndexExpression {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    SliceExpression {
        object: Box<Expr>,
        start: Box<Expr>,
        end: Box<Expr>,
        step: Option<Box<Expr>>,
    },
    Lambda {
        params: Vec<FunctionParam>,
        return_type: Type,
        body: Block,
    },
    BlockExpression {
        block: Block,
    },
    CastExpression {
        target_type: Type,
        value: Box<Expr>,
        source_type: Option<Type>,
    },
    IfExpression {
        condition: Box<Expr>,
        true_branch: Block,
        false_branch: IfElseBranch,
    },
    MatchExpression {
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    OkExpression {
        value: Box<Expr>,
    },
    ErrExpression {
        value: Box<Expr>,
    },
    SomeExpression {
        value: Box<Expr>,
    },
    NoneExpression,
    PropagateExpression {
        value: Box<Expr>,
    },
    IsSomeExpression {
        value: Box<Expr>,
        binding: String,
    },
    IsNoneExpression {
        value: Box<Expr>,
    },
    IsOkExpression {
        value: Box<Expr>,
        binding: String,
    },
    IsErrExpression {
        value: Box<Expr>,
        binding: String,
    },
    AwaitExpression {
        argument: Box<Expr>,
    },
    SpawnExpression {
        argument: Box<Expr>,
    },
    LLMExpression {
        prompt: Box<Expr>,
        model_size: Box<Expr>,
        reasoning_effort: Box<Expr>,
        context: Box<Expr>,
        return_type: Type,
    },
}
