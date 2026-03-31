//! Internal type representation for Spore's type checker.

use std::collections::BTreeSet;

/// A set of capabilities (effects) required by a function.
pub type CapSet = BTreeSet<String>;

/// The internal type representation used during type checking.
/// This is separate from the AST's `TypeExpr` — resolved and normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    /// Primitive types
    Int,
    Float,
    Bool,
    Str,
    /// Unit type (empty tuple / void)
    Unit,

    /// Named type (structs, type aliases, type params)
    Named(String),

    /// Generic type application: `List[Int]`, `Result[T, E]`
    App(String, Vec<Ty>),

    /// Tuple: `(Int, String)`
    Tuple(Vec<Ty>),

    /// Function type: `(params) -> return [uses caps]`
    Fn(Vec<Ty>, Box<Ty>, CapSet),

    /// Type variable (for future inference / generics)
    Var(u32),

    /// The type of a hole — we know the expected type but it's unfilled
    Hole(String),

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
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Int => write!(f, "Int"),
            Ty::Float => write!(f, "Float"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Str => write!(f, "String"),
            Ty::Unit => write!(f, "()"),
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
            Ty::Fn(params, ret, caps) => {
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
                Ok(())
            }
            Ty::Var(id) => write!(f, "?T{id}"),
            Ty::Hole(name) => write!(f, "?{name}"),
            Ty::Error => write!(f, "<error>"),
        }
    }
}
