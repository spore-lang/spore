//! Spore Abstract Syntax Tree definitions.

pub use crate::lexer::Span;
pub use crate::lexer::{Comment, CommentKind};

/// A Spore module (one .spore file = one module).
#[derive(Debug, Clone)]
pub struct Module {
    /// Module name metadata (derived from file path by compiler/tooling).
    pub name: String,
    pub items: Vec<Item>,
    /// Source-level comments preserved for the formatter.
    pub comments: Vec<Comment>,
}

/// Top-level items in a module.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Item {
    Function(FnDef),
    Const(ConstDef),
    StructDef(StructDef),
    TypeDef(TypeDef),
    ImplDef(ImplDef),
    Import(ImportDecl),
    Alias(AliasDef),
    OpaqueType(OpaqueTypeDef),
    TraitDef(TraitDef),
    EffectDef(EffectDef),
    SurfaceDef(SurfaceDef),
    HandlerDef(HandlerDef),
}

impl Item {
    /// Return the source span of this item, if available.
    pub fn span(&self) -> Option<Span> {
        match self {
            Item::Function(f) => f.span,
            Item::Const(c) => c.span,
            Item::StructDef(s) => s.span,
            Item::TypeDef(t) => t.span,
            Item::ImplDef(i) => i.span,
            Item::Import(i) => match i {
                ImportDecl::Import { span, .. } | ImportDecl::Alias { span, .. } => *span,
            },
            Item::Alias(a) => a.span,
            Item::OpaqueType(t) => t.span,
            Item::TraitDef(t) => t.span,
            Item::EffectDef(e) => e.span,
            Item::SurfaceDef(s) => s.span,
            Item::HandlerDef(h) => h.span,
        }
    }
}

/// Source-level item metadata introduced by `@name(...)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub args: Vec<AttrArg>,
    pub span: Option<Span>,
}

/// Positional or named attribute argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrArg {
    Positional(AttrValue),
    Named { name: String, value: AttrValue },
}

/// Literal forms accepted by the attribute grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
    Ident(String),
    Str(String),
    Int(i64),
}

/// Transparent type alias: `type X = Y`
#[derive(Debug, Clone)]
pub struct AliasDef {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub target: TypeExpr,
    pub span: Option<Span>,
}

/// Externally-provided opaque type declaration: `@foreign type Name[T];`
#[derive(Debug, Clone)]
pub struct OpaqueTypeDef {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub span: Option<Span>,
}

/// Compile-time constant definition: `const MAX_SIZE: I64 = 1024`
#[derive(Debug, Clone)]
pub struct ConstDef {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub visibility: Visibility,
    pub ty: TypeExpr,
    pub value: Expr,
    pub span: Option<Span>,
}

/// Function definition with full Spore signature.
///
/// A function has a Base Signature plus optional Intent Signature clauses.
#[derive(Debug, Clone)]
pub struct FnDef {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub visibility: Visibility,
    /// Generic type parameters: `fn foo[T, U](...)`
    pub type_params: Vec<String>,
    /// Inline generic bounds: `fn foo[T: Display](...)`
    pub type_param_bounds: Vec<TypeConstraint>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    /// Realization-shape budget: `budget { branches: 4 }`
    pub budget_clause: Option<BudgetClause>,
    /// Source properties: `properties { name(x: T): predicate }`
    pub properties_clause: Option<PropertiesClause>,
    /// Required effects: `uses [Console, FileRead]`
    pub uses_clause: Option<UsesClause>,
    /// External function declaration. The surface spelling is owned by attributes.
    pub is_foreign: bool,
    /// None means this is a hole (?name)
    pub body: Option<Expr>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq)]
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

/// Named realization-shape budget introduced by `budget`.
///
/// Example:
/// ```text
/// budget {
///     branches: 4
///     holes: 0
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetClause {
    pub items: Vec<BudgetItem>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetItem {
    pub field: String,
    pub limit: u64,
    pub span: Option<Span>,
}

/// A source-level effect-surface expression.
///
/// Examples: `IO`, `State[Session]`, `[Console, FileRead]`
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceRef {
    pub name: String,
    pub type_args: Vec<TypeExpr>,
}

impl SurfaceRef {
    pub fn bare(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_args: Vec::new(),
        }
    }
}

impl From<&str> for SurfaceRef {
    fn from(name: &str) -> Self {
        Self::bare(name)
    }
}

impl From<String> for SurfaceRef {
    fn from(name: String) -> Self {
        Self::bare(name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceExpr {
    Named(SurfaceRef),
    Set(Vec<SurfaceRef>),
}

impl SurfaceExpr {
    pub fn names(&self) -> Vec<&str> {
        match self {
            Self::Named(reference) => vec![reference.name.as_str()],
            Self::Set(references) => references.iter().map(|item| item.name.as_str()).collect(),
        }
    }

    pub fn references(&self) -> Vec<&SurfaceRef> {
        match self {
            Self::Named(reference) => vec![reference],
            Self::Set(references) => references.iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Set(references) if references.is_empty())
    }
}

/// Required effect surface introduced by `uses`.
///
/// Examples: `uses IO`, `uses [Console, FileRead]`
#[derive(Debug, Clone, PartialEq)]
pub struct UsesClause {
    pub surface: SurfaceExpr,
}

/// Source properties introduced by `properties`.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertiesClause {
    pub items: Vec<PropertyDecl>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub predicate: Box<Expr>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct TypeConstraint {
    pub type_var: String,
    pub bound: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named(String),
    /// Type hole in signatures, e.g. `-> ?` or `x: ?`.
    Hole(Option<String>),
    Generic(String, Vec<TypeExpr>),
    Tuple(Vec<TypeExpr>),
    /// Function type: `(I32) -> I32 ! ParseError`
    Function(Vec<TypeExpr>, Box<TypeExpr>),
    /// First-class outcome type: `A ! E`.
    Outcome(Box<TypeExpr>, Box<TypeExpr>),
    /// Refinement type using `when`: `{ x: I64 when x > 0 }`
    ///
    /// Fields: base type, binding name, predicate expression.
    Refinement(Box<TypeExpr>, String, Box<Expr>),
    /// Anonymous record type: `{ x: I64, y: I64 }`
    Record(Vec<(String, TypeExpr)>),
}

/// Expression — everything in Spore is an expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    SuffixedIntLit(i64, String),
    FloatLit(f64),
    StrLit(String),
    FString(Vec<FStringPart>),
    BoolLit(bool),
    Unit,
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
    Hole(Option<String>, Option<Box<TypeExpr>>, Option<Span>),
    StructLit(String, Vec<(String, Expr)>),
    Spawn(Box<Expr>),
    Await(Box<Expr>),
    /// `Channel.new[T](buffer: N)`
    ChannelNew {
        elem_type: TypeExpr,
        buffer: Box<Expr>,
    },
    Return(Option<Box<Expr>>),
    /// Construct a failed outcome: `fail error`.
    Fail(Box<Expr>),
    List(Vec<Expr>),
    TString(Vec<TStringPart>),
    /// `parallel_scope { body }` or `parallel_scope(lanes: N) { body }`
    ParallelScope {
        lanes: Option<Box<Expr>>,
        body: Box<Expr>,
    },
    /// `select { val from rx => body, ... }`
    Select(Vec<SelectArm>),
    /// `perform StdIO.println("hello")` — invoke an effect operation.
    Perform {
        effect: String,
        operation: String,
        args: Vec<Box<Expr>>,
    },
    /// `handle { body } with { use HostConsole {}, on Console.println(msg) => { ... } }`
    /// — install named and/or inline handlers.
    Handle {
        body: Box<Expr>,
        handlers: Vec<HandleBinding>,
    },
    /// Placeholder for partial application — desugared to lambda parameter.
    /// `f(_, 2)` desugars to `|_p0| f(_p0, 2)`.
    /// Should never reach codegen; the parser rewrites calls containing
    /// placeholders into `Lambda(params, Call(...))`.
    Placeholder,
}

/// A single item inside a `handle ... with { ... }` block.
#[derive(Debug, Clone, PartialEq)]
pub enum HandleBinding {
    Use(HandlerUse),
    On(EffectArm),
}

/// Install a named handler instance with explicit payload initialization.
#[derive(Debug, Clone, PartialEq)]
pub struct HandlerUse {
    pub handler: String,
    pub payload: Vec<(String, Expr)>,
}

/// A single inline effect handler arm in a `handle` expression.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectArm {
    pub effect: String,
    pub operation: String,
    pub params: Vec<String>,
    pub body: Box<Expr>,
}

/// A single arm of a `select` expression.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectArm {
    /// `<binding> from <source> => <body>`
    Recv {
        binding: String,
        source: Expr,
        body: Expr,
    },
    /// `timeout(<duration>) => <body>`
    Timeout { duration: Expr, body: Expr },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FStringPart {
    Literal(String),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TStringPart {
    Literal(String),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let(String, Option<TypeExpr>, Expr),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard,
    Var(String),
    IntLit(i64),
    StrLit(String),
    BoolLit(bool),
    /// Match the successful value of an outcome: `ok value`.
    OutcomeOk(Box<Pattern>),
    /// Match the failure value of an outcome: `fail error`.
    OutcomeFail(Box<Pattern>),
    Constructor(String, Vec<Pattern>),
    Struct(String, Vec<(String, Pattern)>),
    Or(Vec<Pattern>),
    /// List pattern: `[head, ..tail]` — elements + optional rest binding.
    List(Vec<Pattern>, Option<String>),
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub fields: Vec<FieldDef>,
    pub implements: Vec<ImplBlock>,
    pub deriving: Vec<String>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub variants: Vec<Variant>,
    pub implements: Vec<ImplBlock>,
    pub deriving: Vec<String>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<TypeExpr>,
}

/// Associated type declaration inside a trait/effect-style definition.
#[derive(Debug, Clone)]
pub struct AssocType {
    pub name: String,
    pub bounds: Vec<TypeExpr>,
}

/// Trait definition for type interfaces.
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub type_param_bounds: Vec<TypeConstraint>,
    pub methods: Vec<FnDef>,
    pub assoc_types: Vec<AssocType>,
    pub span: Option<Span>,
}

/// Atomic effect definition: `effect Console { fn println(msg: Str) -> Unit }`
#[derive(Debug, Clone)]
pub struct EffectDef {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub type_param_bounds: Vec<TypeConstraint>,
    pub operations: Vec<FnDef>,
    pub span: Option<Span>,
}

/// Reusable effect surface: `surface IO = [Console, FileRead, FileWrite]`
#[derive(Debug, Clone)]
pub struct SurfaceDef {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub type_param_bounds: Vec<TypeConstraint>,
    pub surface: SurfaceExpr,
    pub span: Option<Span>,
}

/// Operations implemented by a handler for one atomic effect.
#[derive(Debug, Clone)]
pub struct HandlerImpl {
    pub effect: String,
    pub methods: Vec<FnDef>,
    pub span: Option<Span>,
}

/// Named handler for an effect surface.
#[derive(Debug, Clone)]
pub struct HandlerDef {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub visibility: Visibility,
    pub surface: SurfaceExpr,
    pub impls: Vec<HandlerImpl>,
    pub span: Option<Span>,
}

/// Top-level impl block: `impl[T] Trait[T] for Type[T] { ... }` or `impl[T] Type[T] { ... }`
#[derive(Debug, Clone)]
pub struct ImplDef {
    pub attributes: Vec<Attribute>,
    pub type_params: Vec<String>,
    pub type_param_bounds: Vec<TypeConstraint>,
    pub interface_type: TypeExpr,
    pub target_type: Option<TypeExpr>,
    pub methods: Vec<FnDef>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub trait_name: String,
    pub methods: Vec<(String, Expr)>,
}

#[derive(Debug, Clone)]
pub enum ImportDecl {
    Import {
        path: String,
        alias: String,
        span: Option<Span>,
    },
    Alias {
        name: String,
        path: String,
        span: Option<Span>,
    },
}
