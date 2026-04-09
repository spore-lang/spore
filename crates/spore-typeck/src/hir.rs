//! High-level Intermediate Representation.
//!
//! A resolved, desugared version of the AST. Names are resolved,
//! syntactic sugar is expanded, and the structure is simplified
//! for type checking and code generation.

use std::collections::BTreeSet;

/// A unique identifier for resolved names.
pub type DefId = u32;

/// Sentinel value indicating an unresolved name.
pub const UNRESOLVED: DefId = u32::MAX;

/// A resolved module.
#[derive(Debug, Clone)]
pub struct HirModule {
    pub items: Vec<HirItem>,
}

#[derive(Debug, Clone)]
pub enum HirItem {
    Function(HirFnDef),
    StructDef(HirStructDef),
    TypeDef(HirTypeDef),
    CapabilityDef(HirCapabilityDef),
    ImplDef(HirImplDef),
}

#[derive(Debug, Clone)]
pub struct HirFnDef {
    pub name: String,
    pub def_id: DefId,
    pub params: Vec<HirParam>,
    pub return_type: Option<HirTypeRef>,
    pub body: Option<HirExpr>,
    pub uses_clause: BTreeSet<String>,
    pub throws: Vec<HirTypeRef>,
    pub where_clause: Vec<HirTypeConstraint>,
    /// Compute-dimension upper bound from `cost [compute, alloc, io, parallel]`.
    pub cost_bound: Option<Box<HirExpr>>,
}

#[derive(Debug, Clone)]
pub struct HirTypeConstraint {
    pub type_var: String,
    pub bound: String,
}

#[derive(Debug, Clone)]
pub struct HirParam {
    pub name: String,
    pub ty: HirTypeRef,
}

/// Type reference in HIR — resolved but not yet fully typed.
#[derive(Debug, Clone)]
pub enum HirTypeRef {
    Primitive(PrimitiveTy),
    Named(String, DefId),
    Generic(String, Vec<HirTypeRef>),
    Function(Vec<HirTypeRef>, Box<HirTypeRef>),
    /// Anonymous record type: `{ x: Int, y: Int }`
    Record(Vec<(String, Box<HirTypeRef>)>),
}

#[derive(Debug, Clone, Copy)]
pub enum PrimitiveTy {
    Int,
    Float,
    Bool,
    Str,
    Char,
    Unit,
    Never,
}

#[derive(Debug, Clone)]
pub struct HirStructDef {
    pub name: String,
    pub def_id: DefId,
    pub type_params: Vec<String>,
    pub fields: Vec<(String, HirTypeRef)>,
}

#[derive(Debug, Clone)]
pub struct HirTypeDef {
    pub name: String,
    pub def_id: DefId,
    pub type_params: Vec<String>,
    pub variants: Vec<HirVariant>,
}

#[derive(Debug, Clone)]
pub struct HirVariant {
    pub name: String,
    pub fields: Vec<HirTypeRef>,
}

#[derive(Debug, Clone)]
pub struct HirCapabilityDef {
    pub name: String,
    pub def_id: DefId,
    pub type_params: Vec<String>,
    pub methods: Vec<HirFnDef>,
}

#[derive(Debug, Clone)]
pub struct HirImplDef {
    pub capability: String,
    pub target_type: String,
    pub methods: Vec<HirFnDef>,
}

/// HIR expressions — desugared and simplified.
#[derive(Debug, Clone)]
pub enum HirExpr {
    // Literals
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    BoolLit(bool),

    // Variables (resolved)
    Var(String, DefId),

    // Operations
    BinOp(Box<HirExpr>, HirBinOp, Box<HirExpr>),
    UnaryOp(HirUnaryOp, Box<HirExpr>),

    // Function call (pipe desugared into this)
    Call(Box<HirExpr>, Vec<HirExpr>),

    // Control flow
    If(Box<HirExpr>, Box<HirExpr>, Option<Box<HirExpr>>),
    Match(Box<HirExpr>, Vec<HirMatchArm>),
    Block(Vec<HirStmt>, Option<Box<HirExpr>>),

    // Struct/field
    StructLit(String, Vec<(String, HirExpr)>),
    FieldAccess(Box<HirExpr>, String),

    // Lambda
    Lambda(Vec<HirParam>, Box<HirExpr>),

    // Effects
    Try(Box<HirExpr>),
    Spawn(Box<HirExpr>),
    Await(Box<HirExpr>),
    Return(Option<Box<HirExpr>>),
    Throw(Box<HirExpr>),
    List(Vec<HirExpr>),
    CharLit(char),

    // Holes (preserved for IDE support)
    Hole(String),
}

#[derive(Debug, Clone)]
pub enum HirBinOp {
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
pub enum HirUnaryOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone)]
pub struct HirMatchArm {
    pub pattern: HirPattern,
    pub guard: Option<HirExpr>,
    pub body: HirExpr,
}

#[derive(Debug, Clone)]
pub enum HirPattern {
    Wildcard,
    Var(String),
    IntLit(i64),
    StrLit(String),
    BoolLit(bool),
    Constructor(String, Vec<HirPattern>),
    Struct(String, Vec<(String, HirPattern)>),
    Or(Vec<HirPattern>),
}

#[derive(Debug, Clone)]
pub enum HirStmt {
    Let(String, Option<HirTypeRef>, HirExpr),
    Expr(HirExpr),
}
