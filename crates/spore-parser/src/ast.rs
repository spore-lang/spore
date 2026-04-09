//! Spore Abstract Syntax Tree definitions.

pub use crate::lexer::Span;

/// A Spore module (one .spore file = one module).
#[derive(Debug, Clone)]
pub struct Module {
    /// Module name metadata (derived from file path by compiler/tooling).
    pub name: String,
    pub items: Vec<Item>,
}

/// Top-level items in a module.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Item {
    Function(FnDef),
    Const(ConstDef),
    StructDef(StructDef),
    TypeDef(TypeDef),
    /// Legacy `capability` definition kept for compatibility. Formatter and
    /// tooling present it using the canonical `trait` spelling.
    CapabilityDef(CapabilityDef),
    /// Legacy `capability IO = [FileRead, FileWrite]` alias. Formatter rewrites
    /// this to the canonical `effect IO = FileRead | FileWrite` spelling.
    CapabilityAlias {
        name: String,
        components: Vec<String>,
        span: Option<Span>,
    },
    ImplDef(ImplDef),
    Import(ImportDecl),
    Alias(AliasDef),
    TraitDef(TraitDef),
    EffectDef(EffectDef),
    EffectAlias(EffectAlias),
    HandlerDef(HandlerDef),
}

/// Type alias: `alias X = Y`
#[derive(Debug, Clone)]
pub struct AliasDef {
    pub name: String,
    pub visibility: Visibility,
    pub target: TypeExpr,
    pub span: Option<Span>,
}

/// Compile-time constant definition: `const MAX_SIZE: Int = 1024`
#[derive(Debug, Clone)]
pub struct ConstDef {
    pub name: String,
    pub visibility: Visibility,
    pub ty: TypeExpr,
    pub value: Expr,
    pub span: Option<Span>,
}

/// Function definition with full Spore signature.
///
/// Clauses are separate syntactic constructs:
/// - `where T: Bound`  — generic type constraints
/// - `uses [Memory]`    — resource dependencies
/// - `cost [1, 0, 0, 0]` — cost upper-bound vector
#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub visibility: Visibility,
    /// Generic type parameters: `fn foo[T, U](...)`
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub errors: Vec<TypeExpr>,
    /// Generic type constraints: `where T: Display, U: Clone`
    pub where_clause: Option<WhereClause>,
    /// Cost upper-bound: `cost [compute, alloc, io, parallel]`
    pub cost_clause: Option<CostClause>,
    /// Behavioral contract: `spec { example ... property ... }`
    pub spec_clause: Option<SpecClause>,
    /// Resource dependencies: `uses [Memory, FileSystem]`
    pub uses_clause: Option<UsesClause>,
    /// `@unbounded` annotation — skip cost analysis.
    pub is_unbounded: bool,
    /// `@allows[...]` annotation — default allow-list for holes in this function.
    pub hole_allows: Option<Vec<String>>,
    /// `foreign fn` — implemented by host platform, no body.
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

/// Generic type constraints introduced by `where`.
///
/// Example: `where T: Display, U: Clone`
///
/// This only covers type-parameter bounds. Effects, cost, and resources
/// are expressed with their own dedicated clauses (`with`, `cost`, `uses`).
#[derive(Debug, Clone)]
pub struct WhereClause {
    pub constraints: Vec<TypeConstraint>,
}

/// Cost upper-bound introduced by `cost`.
///
/// Example: `cost [1, O(n), 2, 3]`
#[derive(Debug, Clone)]
pub struct CostClause {
    pub compute: CostExpr,
    pub alloc: CostExpr,
    pub io: CostExpr,
    pub parallel: CostExpr,
}

/// Resource dependencies introduced by `uses`.
///
/// Example: `uses [Memory, FileSystem]`
#[derive(Debug, Clone)]
pub struct UsesClause {
    pub resources: Vec<String>,
}

/// Behavioral contract introduced by `spec`.
///
/// Contains example assertions and property-based invariants:
/// ```text
/// spec {
///     example "identity": add(0, x) == x
///     property "commutative": |a: Int, b: Int| add(a, b) == add(b, a)
/// }
/// ```
///
/// The original item order is preserved so formatters and diagnostics can
/// respect the source layout.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecClause {
    pub items: Vec<SpecItem>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpecItem {
    Example(ExampleItem),
    Property(PropertyItem),
}

/// A single example assertion inside a `spec` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ExampleItem {
    pub label: String,
    pub body: Box<Expr>,
    pub span: Option<Span>,
}

/// A single property invariant inside a `spec` block.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyItem {
    pub label: String,
    pub predicate: Box<Expr>,
    pub span: Option<Span>,
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
    /// Big-O linear notation: `O(n)`.
    Linear(String),
    Mul(Box<CostExpr>, Box<CostExpr>),
    Add(Box<CostExpr>, Box<CostExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named(String),
    /// Type hole in signatures, e.g. `-> ?` or `x: ?`.
    Hole(Option<String>),
    Generic(String, Vec<TypeExpr>),
    Tuple(Vec<TypeExpr>),
    /// Function type with optional error set: `(I32) -> I32 ! ParseError | IoError`
    Function(Vec<TypeExpr>, Box<TypeExpr>, Vec<TypeExpr>),
    /// Refinement type using `when`: `{ x: Int when x > 0 }`
    ///
    /// Fields: base type, binding name, predicate expression.
    Refinement(Box<TypeExpr>, String, Box<Expr>),
    /// Anonymous record type: `{ x: Int, y: Int }`
    Record(Vec<(String, TypeExpr)>),
}

/// Expression — everything in Spore is an expression.
#[derive(Debug, Clone, PartialEq)]
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
    Hole(Option<String>, Option<Box<TypeExpr>>, Option<Vec<String>>),
    StructLit(String, Vec<(String, Expr)>),
    Spawn(Box<Expr>),
    Await(Box<Expr>),
    Return(Option<Box<Expr>>),
    Throw(Box<Expr>),
    List(Vec<Expr>),
    CharLit(char),
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
    /// `handle { body } with { StdIO.println(msg) => { ... } }` — install handlers.
    Handle {
        body: Box<Expr>,
        handlers: Vec<EffectArm>,
    },
    /// Placeholder for partial application — desugared to lambda parameter.
    /// `f(_, 2)` desugars to `|_p0| f(_p0, 2)`.
    /// Should never reach codegen; the parser rewrites calls containing
    /// placeholders into `Lambda(params, Call(...))`.
    Placeholder,
}

/// A single effect handler arm in a `handle` expression.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectArm {
    pub effect: String,
    pub operation: String,
    pub params: Vec<String>,
    pub body: Box<Expr>,
}

/// A single arm of a `select` expression.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectArm {
    pub binding: String,
    pub source: Expr,
    pub body: Expr,
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
    Constructor(String, Vec<Pattern>),
    Struct(String, Vec<(String, Pattern)>),
    Or(Vec<Pattern>),
    /// List pattern: `[head, ..tail]` — elements + optional rest binding.
    List(Vec<Pattern>, Option<String>),
}

#[derive(Debug, Clone)]
pub struct StructDef {
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

#[derive(Debug, Clone)]
pub struct CapabilityDef {
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub methods: Vec<FnDef>,
    pub assoc_types: Vec<AssocType>,
    pub span: Option<Span>,
}

impl CapabilityDef {
    pub fn canonical_keyword(&self) -> &'static str {
        "trait"
    }

    pub fn completion_detail(&self) -> &'static str {
        "trait"
    }
}

/// Trait definition (preferred alias for `capability` when defining type interfaces).
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub name: String,
    pub visibility: Visibility,
    pub type_params: Vec<String>,
    pub methods: Vec<FnDef>,
    pub assoc_types: Vec<AssocType>,
    pub span: Option<Span>,
}

/// Atomic effect definition: `effect Console { fn println(msg: Str) -> Unit }`
#[derive(Debug, Clone)]
pub struct EffectDef {
    pub name: String,
    pub visibility: Visibility,
    pub operations: Vec<FnDef>,
    pub span: Option<Span>,
}

/// Effect alias (union of effects): `effect IO = Console | FileRead | FileWrite`
#[derive(Debug, Clone)]
pub struct EffectAlias {
    pub name: String,
    pub visibility: Visibility,
    pub effects: Vec<String>,
    pub span: Option<Span>,
}

/// Handler implementation: `handler MockConsole for Console { ... }`
#[derive(Debug, Clone)]
pub struct HandlerDef {
    pub name: String,
    pub effect: String,
    pub fields: Vec<FieldDef>,
    pub methods: Vec<FnDef>,
    pub span: Option<Span>,
}

/// Top-level impl block: `impl Capability for Type { ... }`
#[derive(Debug, Clone)]
pub struct ImplDef {
    pub capability: String,
    pub target_type: String,
    pub type_args: Vec<TypeExpr>,
    pub methods: Vec<FnDef>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub capability: String,
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
