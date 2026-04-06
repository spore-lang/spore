//! Internal type representation for Spore's type checker.

use std::collections::BTreeSet;
use std::fmt;

/// A set of capabilities (effects) required by a function.
pub type CapSet = BTreeSet<String>;

/// A set of error types that a function may throw.
pub type ErrorSet = BTreeSet<String>;

/// The internal type representation used during type checking.
/// This is separate from the AST's `TypeExpr` — resolved and normalized.
#[derive(Debug, Clone)]
pub enum Ty {
    /// Primitive types
    Int,
    Float,
    Bool,
    Str,
    Char,
    /// Unit type (empty tuple / void)
    Unit,
    /// Bottom type — subtype of all types
    Never,

    /// Named type (structs, type aliases, type params)
    Named(String),

    /// Generic type application: `List[Int]`, `Result[T, E]`
    App(String, Vec<Ty>),

    /// Tuple: `(Int, String)`
    Tuple(Vec<Ty>),

    /// Function type: `(params) -> return [uses caps] [! errors]`
    Fn(Vec<Ty>, Box<Ty>, CapSet, ErrorSet),

    /// Type variable (for future inference / generics)
    Var(u32),

    /// The type of a hole — we know the expected type but it's unfilled
    Hole(String),

    /// Anonymous record type: `{ x: Int, y: Int }`
    Record(Vec<(String, Ty)>),

    /// Refinement type: base type with decidable predicate.
    /// L0 only supports: comparisons, arithmetic on constants, len(), boolean connectives.
    Refined(Box<Ty>, String, Box<spore_parser::ast::Expr>),

    /// Error sentinel — allows type checking to continue after errors
    Error,
}

impl Ty {
    /// Check if this type is numeric (Int or Float).
    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Float)
    }

    /// Check if this type is the error sentinel.
    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }

    /// Extract the base type, stripping refinement if present.
    pub fn base_type(&self) -> &Ty {
        match self {
            Ty::Refined(base, _, _) => base.base_type(),
            other => other,
        }
    }
}

impl PartialEq for Ty {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Ty::Int, Ty::Int)
            | (Ty::Float, Ty::Float)
            | (Ty::Bool, Ty::Bool)
            | (Ty::Str, Ty::Str)
            | (Ty::Char, Ty::Char)
            | (Ty::Unit, Ty::Unit)
            | (Ty::Never, Ty::Never)
            | (Ty::Error, Ty::Error) => true,
            (Ty::Named(a), Ty::Named(b)) => a == b,
            (Ty::App(n1, a1), Ty::App(n2, a2)) => n1 == n2 && a1 == a2,
            (Ty::Tuple(a), Ty::Tuple(b)) => a == b,
            (Ty::Fn(p1, r1, c1, e1), Ty::Fn(p2, r2, c2, e2)) => {
                p1 == p2 && r1 == r2 && c1 == c2 && e1 == e2
            }
            (Ty::Var(a), Ty::Var(b)) => a == b,
            (Ty::Hole(a), Ty::Hole(b)) => a == b,
            (Ty::Record(a), Ty::Record(b)) => a == b,
            // Refined types: compare base and var name only (predicates checked separately)
            (Ty::Refined(b1, v1, _), Ty::Refined(b2, v2, _)) => b1 == b2 && v1 == v2,
            _ => false,
        }
    }
}

impl Eq for Ty {}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int => write!(f, "Int"),
            Ty::Float => write!(f, "Float"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Str => write!(f, "String"),
            Ty::Char => write!(f, "Char"),
            Ty::Unit => write!(f, "()"),
            Ty::Never => write!(f, "Never"),
            Ty::Named(n) => write!(f, "{n}"),
            Ty::App(name, args) => {
                write!(f, "{name}[")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, "]")
            }
            Ty::Tuple(ts) => {
                write!(f, "(")?;
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, ")")
            }
            Ty::Fn(params, ret, caps, errors) => {
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {ret}")?;
                if !caps.is_empty() {
                    let cap_list: Vec<&str> = caps.iter().map(|s| s.as_str()).collect();
                    write!(f, " uses [{}]", cap_list.join(", "))?;
                }
                if !errors.is_empty() {
                    let err_list: Vec<&str> = errors.iter().map(|s| s.as_str()).collect();
                    write!(f, " ! {}", err_list.join(" | "))?;
                }
                Ok(())
            }
            Ty::Record(fields) => {
                write!(f, "{{ ")?;
                for (i, (name, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{name}: {ty}")?;
                }
                write!(f, " }}")
            }
            Ty::Var(id) => write!(f, "?T{id}"),
            Ty::Hole(name) => write!(f, "?{name}"),
            Ty::Refined(base, _var, _pred) => write!(f, "{base} when <predicate>"),
            Ty::Error => write!(f, "<error>"),
        }
    }
}
