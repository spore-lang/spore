//! Spore Abstract Syntax Tree definitions.

/// A Spore module (one .spore file = one module).
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub items: Vec<Item>,
}

/// Top-level items in a module.
#[derive(Debug, Clone)]
pub enum Item {
    Function(FnDef),
    StructDef(StructDef),
    TypeDef(TypeDef),
    CapabilityDef(CapabilityDef),
    Import(ImportDecl),
}

/// Function definition with full Spore signature.
#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub visibility: Visibility,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub errors: Vec<TypeExpr>,
    pub where_clause: Option<WhereClause>,
    /// None means this is a hole (?name)
    pub body: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone, Default)]
pub enum Visibility {
    #[default]
    Private,
    PubPkg,
    Pub,
}

/// Where clause: effects, cost, capabilities.
#[derive(Debug, Clone)]
pub struct WhereClause {
    pub type_constraints: Vec<TypeConstraint>,
    pub effects: Vec<String>,
    pub cost: Option<CostExpr>,
    pub uses: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TypeConstraint {
    pub type_var: String,
    pub bound: String,
}

#[derive(Debug, Clone)]
pub enum CostExpr {
    Literal(u64),
    Var(String),
    Mul(Box<CostExpr>, Box<CostExpr>),
    Add(Box<CostExpr>, Box<CostExpr>),
}

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Named(String),
    Generic(String, Vec<TypeExpr>),
    Tuple(Vec<TypeExpr>),
    Function(Vec<TypeExpr>, Box<TypeExpr>),
}

/// Expression — everything in Spore is an expression.
#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    FString(Vec<FStringPart>),
    BoolLit(bool),
    Var(String),
    Call(Box<Expr>, Vec<Expr>),
    Lambda(Vec<Param>, Box<Expr>),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    UnaryOp(UnaryOp, Box<Expr>),
    FieldAccess(Box<Expr>, String),
    Pipe(Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Match(Box<Expr>, Vec<MatchArm>),
    Block(Vec<Stmt>, Option<Box<Expr>>),
    Try(Box<Expr>),
    Hole(String, Option<Box<TypeExpr>>),
    StructLit(String, Vec<(String, Expr)>),
    Spawn(Box<Expr>),
    Await(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum FStringPart {
    Literal(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum BinOp {
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
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(String, Option<TypeExpr>, Expr),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,
    Var(String),
    IntLit(i64),
    StrLit(String),
    BoolLit(bool),
    Constructor(String, Vec<Pattern>),
    Struct(String, Vec<(String, Pattern)>),
    Or(Vec<Pattern>),
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub fields: Vec<FieldDef>,
    pub implements: Vec<ImplBlock>,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub variants: Vec<Variant>,
    pub implements: Vec<ImplBlock>,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<TypeExpr>,
}

#[derive(Debug, Clone)]
pub struct CapabilityDef {
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub methods: Vec<FnDef>,
}

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub capability: String,
    pub methods: Vec<(String, Expr)>,
}

#[derive(Debug, Clone)]
pub enum ImportDecl {
    Import { path: String, alias: String },
    Alias { name: String, path: String },
}
