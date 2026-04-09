//! Internal type representation for Spore's type checker.

use std::collections::BTreeSet;
use std::fmt;

use crate::is_synthetic_hole_name;

/// A set of capabilities (effects) required by a function.
pub type CapSet = BTreeSet<String>;

/// A set of error types that a function may throw.
pub type ErrorSet = BTreeSet<String>;

/// The internal type representation used during type checking.
/// This is separate from the AST's `TypeExpr` — resolved and normalized.
#[derive(Debug, Clone)]
#[must_use]
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

    /// Recursively transform this type bottom-up.
    /// `f` is called on each sub-type after its children have been transformed.
    pub fn fold<F>(self, f: &mut F) -> Ty
    where
        F: FnMut(Ty) -> Ty,
    {
        let folded = match self {
            Ty::Fn(params, ret, caps, errors) => Ty::Fn(
                params.into_iter().map(|p| p.fold(f)).collect(),
                Box::new((*ret).fold(f)),
                caps,
                errors,
            ),
            Ty::App(name, args) => Ty::App(name, args.into_iter().map(|a| a.fold(f)).collect()),
            Ty::Tuple(ts) => Ty::Tuple(ts.into_iter().map(|t| t.fold(f)).collect()),
            Ty::Record(fields) => {
                Ty::Record(fields.into_iter().map(|(n, t)| (n, t.fold(f))).collect())
            }
            Ty::Refined(base, var, pred) => Ty::Refined(Box::new((*base).fold(f)), var, pred),
            other => other,
        };
        f(folded)
    }

    /// Walk this type, calling `f` on each sub-type (read-only visitor).
    pub fn visit<F>(&self, f: &mut F)
    where
        F: FnMut(&Ty),
    {
        f(self);
        match self {
            Ty::Fn(params, ret, _, _) => {
                for p in params {
                    p.visit(f);
                }
                ret.visit(f);
            }
            Ty::App(_, args) => {
                for a in args {
                    a.visit(f);
                }
            }
            Ty::Tuple(ts) => {
                for t in ts {
                    t.visit(f);
                }
            }
            Ty::Record(fields) => {
                for (_, t) in fields {
                    t.visit(f);
                }
            }
            Ty::Refined(base, _, _) => {
                base.visit(f);
            }
            _ => {}
        }
    }

    /// Recursively transform this type by reference, top-down.
    /// `f` is called on each sub-type; if it returns `Some(ty)`, that result
    /// is used directly (no further recursion). If it returns `None`, recursion
    /// continues into children and the node is reconstructed.
    pub fn fold_ref<F>(&self, f: &mut F) -> Ty
    where
        F: FnMut(&Ty) -> Option<Ty>,
    {
        if let Some(result) = f(self) {
            return result;
        }
        match self {
            Ty::Fn(params, ret, caps, errors) => Ty::Fn(
                params.iter().map(|p| p.fold_ref(f)).collect(),
                Box::new(ret.fold_ref(f)),
                caps.clone(),
                errors.clone(),
            ),
            Ty::App(name, args) => {
                Ty::App(name.clone(), args.iter().map(|a| a.fold_ref(f)).collect())
            }
            Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| t.fold_ref(f)).collect()),
            Ty::Record(fields) => Ty::Record(
                fields
                    .iter()
                    .map(|(n, t)| (n.clone(), t.fold_ref(f)))
                    .collect(),
            ),
            Ty::Refined(base, var, pred) => {
                Ty::Refined(Box::new(base.fold_ref(f)), var.clone(), pred.clone())
            }
            other => other.clone(),
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
            // Refined types: compare base, variable, and predicate structurally
            (Ty::Refined(b1, v1, p1), Ty::Refined(b2, v2, p2)) => b1 == b2 && v1 == v2 && p1 == p2,
            _ => false,
        }
    }
}

impl Eq for Ty {}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int => write!(f, "I32"),
            Ty::Float => write!(f, "F64"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Str => write!(f, "Str"),
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
            Ty::Hole(name) => {
                if is_synthetic_hole_name(name) {
                    write!(f, "?")
                } else {
                    write!(f, "?{name}")
                }
            }
            Ty::Refined(base, _var, _pred) => write!(f, "{base} when <predicate>"),
            Ty::Error => write!(f, "<error>"),
        }
    }
}
