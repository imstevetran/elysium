use crate::error::SourceSpan;

/// A node in the AST with source location information.
#[derive(Debug, Clone)]
pub struct Node<T> {
    pub value: T,
    pub span: SourceSpan,
}

impl<T> Node<T> {
    pub fn new(value: T, span: SourceSpan) -> Self {
        Self { value, span }
    }
}

/// Top-level program.
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Node<Item>>,
}

/// A top-level item.
#[derive(Debug, Clone)]
pub enum Item {
    Function(Function),
    Class(Class),
    Enum(Enum),
    Component(Component),
    TypeAlias(TypeAlias),
    Import(String, Option<String>),
    Spec(Spec),
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
    pub is_async: bool,
    pub doc_comment: Option<String>,
    pub bc_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
    pub is_rest: bool,
}

#[derive(Debug, Clone)]
pub struct Class {
    pub name: String,
    pub fields: Vec<ClassField>,
    pub methods: Vec<Function>,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClassField {
    pub name: String,
    pub is_mutable: bool,
    pub type_ann: Option<TypeExpr>,
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<EnumField>,
}

#[derive(Debug, Clone)]
pub struct EnumField {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
}

#[derive(Debug, Clone)]
pub struct Component {
    pub name: String,
    pub params: Vec<Param>,
    pub state_vars: Vec<StateVar>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct StateVar {
    pub name: String,
    pub initial_value: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub name: String,
    pub type_expr: TypeExpr,
}

/// A block of statements.
#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Node<Stmt>>,
}

/// A statement.
#[derive(Debug, Clone)]
pub enum Stmt {
    Let(Box<Node<Let>>),
    Expr(Box<Node<Expr>>),
    Return(Option<Box<Node<Expr>>>),
    Assign(Box<Node<Assign>>),
    BcAssert(Box<Node<BcAssert>>),
    If(Box<Node<If>>),
    For(Box<Node<For>>),
    While(Box<Node<While>>),
    Match(Box<Node<Match>>),
    TryCatch(Box<Node<TryCatch>>),
    OnlyGuard(Box<Node<OnlyGuard>>),
    UnsafeBlock(Box<Node<UnsafeBlock>>),
    Expect(Box<Node<Expect>>),
    Todo(Box<Node<Todo>>),
    Question(Box<Node<Question>>),
    Bench(Box<Node<Bench>>),
}

#[derive(Debug, Clone)]
pub struct Let {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
    pub value: Option<Expr>,
    pub is_mutable: bool,
    pub is_only: bool,
    pub bc_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Assign {
    pub target: Box<Node<Expr>>,
    pub value: Box<Node<Expr>>,
}

#[derive(Debug, Clone)]
pub struct BcAssert {
    pub condition: Box<Node<Expr>>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct If {
    pub condition: Box<Node<Expr>>,
    pub then_block: Block,
    pub else_block: Option<Block>,
    pub is_expression: bool,
}

#[derive(Debug, Clone)]
pub struct For {
    pub variable: String,
    pub iterable: Box<Node<Expr>>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct While {
    pub condition: Box<Node<Expr>>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct Match {
    pub value: Box<Node<Expr>>,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,
    Binding(String),
    Literal(Literal),
    EnumVariant { name: String, bindings: Vec<String> },
    OnlyType(String),
}

#[derive(Debug, Clone)]
pub struct TryCatch {
    pub try_block: Block,
    pub catch_pattern: Option<Pattern>,
    pub catch_block: Block,
    pub finally_block: Option<Block>,
}

#[derive(Debug, Clone)]
pub struct OnlyGuard {
    pub condition: Box<Node<Expr>>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct UnsafeBlock {
    pub body: Block,
}

// ==================== Testing / Spec ====================

/// A test suite: `spec "name" { feat "..." { ... } }`
#[derive(Debug, Clone)]
pub struct Spec {
    pub name: String,
    pub feats: Vec<Feat>,
}

/// A single test case: `feat "description" { ... }` or `it "description" { ... }`
#[derive(Debug, Clone)]
pub struct Feat {
    pub name: String,
    pub body: Block,
}

/// An assertion statement: `expect <expr>`
#[derive(Debug, Clone)]
pub struct Expect {
    pub expr: Box<Node<Expr>>,
}

/// A benchmark block: `bench { ... }` or `bm { ... }`
/// Measures wall-clock time and prints it to the log.
#[derive(Debug, Clone)]
pub struct Bench {
    pub body: Block,
}

/// A todo marker: `todo "message"` or just `todo`
#[derive(Debug, Clone)]
pub struct Todo {
    pub message: Option<String>,
}

/// An open question/concern: `question "message"` or just `question`
#[derive(Debug, Clone)]
pub struct Question {
    pub message: Option<String>,
}

/// An expression.
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Node<Literal>),
    Identifier(String),
    BinaryOp {
        op: BinaryOpKind,
        left: Box<Node<Expr>>,
        right: Box<Node<Expr>>,
    },
    UnaryOp {
        op: UnaryOpKind,
        operand: Box<Node<Expr>>,
    },
    Call {
        callee: Box<Node<Expr>>,
        args: Vec<Node<Expr>>,
    },
    MethodCall {
        object: Box<Node<Expr>>,
        method: String,
        args: Vec<Node<Expr>>,
    },
    IfThenElse {
        condition: Box<Node<Expr>>,
        then_expr: Box<Node<Expr>>,
        else_expr: Option<Box<Node<Expr>>>,
    },
    Lambda {
        params: Vec<Param>,
        body: Box<Node<Expr>>,
    },
    Block(Block),
    Array(Vec<Node<Expr>>),
    Tuple(Vec<Node<Expr>>),
    Record(Vec<(String, Node<Expr>)>),
    Index {
        target: Box<Node<Expr>>,
        index: Box<Node<Expr>>,
    },
    MemberAccess {
        target: Box<Node<Expr>>,
        field: String,
    },
    Range {
        start: Box<Node<Expr>>,
        end: Box<Node<Expr>>,
        inclusive: bool,
    },
    Spread(Box<Node<Expr>>),
    BcAnnotation {
        expr: Box<Node<Expr>>,
        reason: String,
    },
    ErrorPropagate(Box<Node<Expr>>),
    MatchExpression {
        value: Box<Node<Expr>>,
        arms: Vec<MatchArm>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOpKind {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Char(char),
    Nil,
}

/// A type expression.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named(String),
    Generic(String, Vec<TypeExpr>),
    Array(Box<TypeExpr>),
    Option(Box<TypeExpr>),
    Result(Box<TypeExpr>, Box<TypeExpr>),
    Union(Vec<TypeExpr>),
    Function(Vec<TypeExpr>, Box<TypeExpr>),
    Tuple(Vec<TypeExpr>),
    Record(Vec<(String, TypeExpr)>),
    Infer,
}
