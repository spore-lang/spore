//! Internal type representation for Spore's type checker.

use std::fmt;

use crate::is_synthetic_hole_name;

pub use crate::effect_set::EffectSet;

/// The internal type representation used during type checking.
/// This is separate from the AST's `TypeExpr` — resolved and normalized.
#[derive(Debug, Clone)]
#[must_use]
pub enum Ty {
    /// Primitive integer types
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    // Future: F16
    F32,
    F64,
    Bool,
    Str,
    /// Unit type (empty tuple / void)
    Unit,
    /// Bottom type — subtype of all types
    Never,

    /// Named type (structs, type aliases, type params)
    Named(String),

    /// Generic type application: `List[I64]`
    App(String, Vec<Ty>),

    /// Tuple: `(I64, Str)`
    Tuple(Vec<Ty>),

    /// Function type: `(params) -> return [uses caps]`
    Fn(Vec<Ty>, Box<Ty>, EffectSet),

    /// First-class outcome type: `success ! failure`.
    Outcome(Box<Ty>, Box<Ty>),

    /// Type variable (for future inference / generics)
    Var(u32),

    /// The type of a hole — we know the expected type but it's unfilled
    Hole(String),

    /// Anonymous record type: `{ x: I64, y: I64 }`
    Record(Vec<(String, Ty)>),

    /// Refinement type: base type with decidable predicate.
    /// L0 only supports: comparisons, arithmetic on constants, len(), boolean connectives.
    Refined(Box<Ty>, String, Box<sporec_parser::ast::Expr>),

    /// Error sentinel — allows type checking to continue after errors
    Error,
}

impl Ty {
    /// Check if this type is an integer type.
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Ty::I8 | Ty::I16 | Ty::I32 | Ty::I64 | Ty::U8 | Ty::U16 | Ty::U32 | Ty::U64
        )
    }

    /// Check if this type is numeric (any integer, F32, or F64).
    pub fn is_numeric(&self) -> bool {
        self.is_integer() || matches!(self, Ty::F32 | Ty::F64)
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
            Ty::Fn(params, ret, caps) => Ty::Fn(
                params.into_iter().map(|p| p.fold(f)).collect(),
                Box::new((*ret).fold(f)),
                caps,
            ),
            Ty::Outcome(success, failure) => {
                Ty::Outcome(Box::new((*success).fold(f)), Box::new((*failure).fold(f)))
            }
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
            Ty::Fn(params, ret, _) => {
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
            Ty::Outcome(success, failure) => {
                success.visit(f);
                failure.visit(f);
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
            Ty::Fn(params, ret, caps) => Ty::Fn(
                params.iter().map(|p| p.fold_ref(f)).collect(),
                Box::new(ret.fold_ref(f)),
                caps.clone(),
            ),
            Ty::Outcome(success, failure) => {
                Ty::Outcome(Box::new(success.fold_ref(f)), Box::new(failure.fold_ref(f)))
            }
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
            (Ty::I8, Ty::I8)
            | (Ty::I16, Ty::I16)
            | (Ty::I32, Ty::I32)
            | (Ty::I64, Ty::I64)
            | (Ty::U8, Ty::U8)
            | (Ty::U16, Ty::U16)
            | (Ty::U32, Ty::U32)
            | (Ty::U64, Ty::U64)
            | (Ty::F32, Ty::F32)
            | (Ty::F64, Ty::F64)
            | (Ty::Bool, Ty::Bool)
            | (Ty::Str, Ty::Str)
            | (Ty::Unit, Ty::Unit)
            | (Ty::Never, Ty::Never)
            | (Ty::Error, Ty::Error) => true,
            (Ty::Named(a), Ty::Named(b)) => a == b,
            (Ty::App(n1, a1), Ty::App(n2, a2)) => n1 == n2 && a1 == a2,
            (Ty::Tuple(a), Ty::Tuple(b)) => a == b,
            (Ty::Fn(p1, r1, c1), Ty::Fn(p2, r2, c2)) => p1 == p2 && r1 == r2 && c1 == c2,
            (Ty::Outcome(s1, f1), Ty::Outcome(s2, f2)) => s1 == s2 && f1 == f2,
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
            Ty::I8 => write!(f, "I8"),
            Ty::I16 => write!(f, "I16"),
            Ty::I32 => write!(f, "I32"),
            Ty::I64 => write!(f, "I64"),
            Ty::U8 => write!(f, "U8"),
            Ty::U16 => write!(f, "U16"),
            Ty::U32 => write!(f, "U32"),
            Ty::U64 => write!(f, "U64"),
            Ty::F32 => write!(f, "F32"),
            Ty::F64 => write!(f, "F64"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Str => write!(f, "Str"),
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
            Ty::Outcome(success, failure) => write!(f, "{success} ! {failure}"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use sporec_parser::ast::Expr;

    fn dummy_expr() -> Box<Expr> {
        Box::new(Expr::BoolLit(true))
    }

    // ── is_integer / is_numeric / is_error ──────────────────────────

    #[test]
    fn integer_types() {
        for ty in [
            Ty::I8,
            Ty::I16,
            Ty::I32,
            Ty::I64,
            Ty::U8,
            Ty::U16,
            Ty::U32,
            Ty::U64,
        ] {
            assert!(ty.is_integer(), "{ty} should be integer");
            assert!(ty.is_numeric(), "{ty} should be numeric");
        }
    }

    #[test]
    fn float_types_are_numeric_not_integer() {
        for ty in [Ty::F32, Ty::F64] {
            assert!(!ty.is_integer(), "{ty} should not be integer");
            assert!(ty.is_numeric(), "{ty} should be numeric");
        }
    }

    #[test]
    fn non_numeric_types() {
        assert!(!Ty::Bool.is_numeric());
        assert!(!Ty::Str.is_numeric());
        assert!(!Ty::Unit.is_numeric());
        assert!(!Ty::Never.is_numeric());
    }

    #[test]
    fn error_sentinel() {
        assert!(Ty::Error.is_error());
        assert!(!Ty::I32.is_error());
    }

    // ── base_type ───────────────────────────────────────────────────

    #[test]
    fn base_type_strips_single_refinement() {
        let refined = Ty::Refined(Box::new(Ty::I32), "x".into(), dummy_expr());
        assert_eq!(*refined.base_type(), Ty::I32);
    }

    #[test]
    fn base_type_strips_nested_refinement() {
        let inner = Ty::Refined(Box::new(Ty::I32), "y".into(), dummy_expr());
        let outer = Ty::Refined(Box::new(inner), "x".into(), dummy_expr());
        assert_eq!(*outer.base_type(), Ty::I32);
    }

    #[test]
    fn base_type_identity_for_non_refined() {
        assert_eq!(*Ty::Bool.base_type(), Ty::Bool);
        assert_eq!(*Ty::Str.base_type(), Ty::Str);
    }

    // ── PartialEq ───────────────────────────────────────────────────

    #[test]
    fn equality_same_primitives() {
        assert_eq!(Ty::I32, Ty::I32);
        assert_ne!(Ty::I32, Ty::I64);
    }

    #[test]
    fn equality_named() {
        assert_eq!(Ty::Named("Foo".into()), Ty::Named("Foo".into()));
        assert_ne!(Ty::Named("Foo".into()), Ty::Named("Bar".into()));
    }

    #[test]
    fn equality_app() {
        let a = Ty::App("List".into(), vec![Ty::I32]);
        let b = Ty::App("List".into(), vec![Ty::I32]);
        let c = Ty::App("List".into(), vec![Ty::Bool]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn equality_refined() {
        let a = Ty::Refined(Box::new(Ty::I32), "x".into(), dummy_expr());
        let b = Ty::Refined(Box::new(Ty::I32), "x".into(), dummy_expr());
        assert_eq!(a, b);
    }

    #[test]
    fn equality_refined_vs_base() {
        let refined = Ty::Refined(Box::new(Ty::I32), "x".into(), dummy_expr());
        assert_ne!(refined, Ty::I32, "refined should not equal base type");
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn display_primitives() {
        assert_eq!(Ty::I32.to_string(), "I32");
        assert_eq!(Ty::Bool.to_string(), "Bool");
        assert_eq!(Ty::Unit.to_string(), "()");
        assert_eq!(Ty::Never.to_string(), "Never");
    }

    #[test]
    fn display_app() {
        let ty = Ty::App("List".into(), vec![Ty::I32]);
        assert_eq!(ty.to_string(), "List[I32]");
    }

    #[test]
    fn display_tuple() {
        let ty = Ty::Tuple(vec![Ty::I32, Ty::Bool]);
        assert_eq!(ty.to_string(), "(I32, Bool)");
    }

    #[test]
    fn display_fn() {
        let ty = Ty::Fn(vec![Ty::I32], Box::new(Ty::Bool), EffectSet::new());
        assert_eq!(ty.to_string(), "(I32) -> Bool");
    }

    #[test]
    fn display_outcome() {
        let ty = Ty::Outcome(Box::new(Ty::I32), Box::new(Ty::Named("IoError".into())));
        assert_eq!(ty.to_string(), "I32 ! IoError");
    }

    #[test]
    fn display_record() {
        let ty = Ty::Record(vec![("x".into(), Ty::I32), ("y".into(), Ty::I32)]);
        assert_eq!(ty.to_string(), "{ x: I32, y: I32 }");
    }

    #[test]
    fn display_hole() {
        assert_eq!(Ty::Hole("foo".into()).to_string(), "?foo");
    }

    #[test]
    fn display_refinement() {
        let ty = Ty::Refined(Box::new(Ty::I32), "x".into(), dummy_expr());
        assert_eq!(ty.to_string(), "I32 when <predicate>");
    }

    // ── fold / visit ────────────────────────────────────────────────

    #[test]
    fn fold_transforms_bottom_up() {
        // Replace all I32 with I64
        let ty = Ty::Tuple(vec![Ty::I32, Ty::Bool, Ty::I32]);
        let result = ty.fold(&mut |t| if t == Ty::I32 { Ty::I64 } else { t });
        assert_eq!(result, Ty::Tuple(vec![Ty::I64, Ty::Bool, Ty::I64]));
    }

    #[test]
    fn fold_ref_transforms_top_down() {
        let ty = Ty::App("List".into(), vec![Ty::I32]);
        let result = ty.fold_ref(&mut |t| match t {
            Ty::App(name, _) if name == "List" => Some(Ty::Named("Vec".into())),
            _ => None,
        });
        assert_eq!(result, Ty::Named("Vec".into()));
    }

    #[test]
    fn visit_collects_types() {
        let ty = Ty::Tuple(vec![Ty::I32, Ty::App("List".into(), vec![Ty::Bool])]);
        let mut seen = Vec::new();
        ty.visit(&mut |t| seen.push(format!("{t}")));
        assert!(seen.contains(&"I32".to_string()));
        assert!(seen.contains(&"Bool".to_string()));
        assert!(seen.contains(&"List[Bool]".to_string()));
    }

    #[test]
    fn fold_transforms_outcome_members() {
        let ty = Ty::Outcome(Box::new(Ty::I32), Box::new(Ty::Named("Failure".into())));
        let result = ty.fold(&mut |ty| if ty == Ty::I32 { Ty::I64 } else { ty });
        assert_eq!(
            result,
            Ty::Outcome(Box::new(Ty::I64), Box::new(Ty::Named("Failure".into())))
        );
    }
}
