//! Decidable refinement predicate evaluator (L0).
//!
//! Supports a restricted set of operations that can be evaluated at compile time
//! when the value is a known constant:
//! - Comparisons: `<, <=, ==, !=, >=, >`
//! - Arithmetic: `+, -, *`
//! - Boolean connectives: `&&, ||, !`
//! - String `.len()` method

use spore_parser::ast::{BinOp, Expr, UnaryOp};

/// A compile-time constant value for refinement checking.
#[derive(Debug, Clone)]
pub enum ConstValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
}

/// Evaluate a refinement predicate with the given variable bound to a constant.
///
/// Returns `Ok(true)` if the predicate is satisfied, `Ok(false)` if violated,
/// or `Err(reason)` if the predicate cannot be decided at compile time.
pub fn eval_refinement_predicate(
    pred: &Expr,
    var_name: &str,
    value: &ConstValue,
) -> Result<bool, String> {
    eval_expr(pred, var_name, value).and_then(|v| match v {
        ConstValue::Bool(b) => Ok(b),
        other => Err(format!(
            "predicate must evaluate to Bool, got {}",
            describe(&other)
        )),
    })
}

/// Try to extract a `ConstValue` from a literal expression.
pub fn expr_to_const(expr: &Expr) -> Option<ConstValue> {
    match expr {
        Expr::IntLit(n) => Some(ConstValue::Int(*n)),
        Expr::FloatLit(f) => Some(ConstValue::Float(*f)),
        Expr::BoolLit(b) => Some(ConstValue::Bool(*b)),
        Expr::StrLit(s) => Some(ConstValue::Str(s.clone())),
        Expr::UnaryOp(UnaryOp::Neg, inner) => match inner.as_ref() {
            Expr::IntLit(n) => n.checked_neg().map(ConstValue::Int),
            Expr::FloatLit(f) => Some(ConstValue::Float(-f)),
            _ => None,
        },
        _ => None,
    }
}

fn describe(v: &ConstValue) -> &'static str {
    match v {
        ConstValue::Int(_) => "I32",
        ConstValue::Float(_) => "F64",
        ConstValue::Bool(_) => "Bool",
        ConstValue::Str(_) => "Str",
    }
}

fn eval_expr(expr: &Expr, var_name: &str, value: &ConstValue) -> Result<ConstValue, String> {
    match expr {
        // The refinement variable itself
        Expr::Var(name) if name == var_name || name == "self" => Ok(value.clone()),

        // Literals
        Expr::IntLit(n) => Ok(ConstValue::Int(*n)),
        Expr::FloatLit(f) => Ok(ConstValue::Float(*f)),
        Expr::BoolLit(b) => Ok(ConstValue::Bool(*b)),
        Expr::StrLit(s) => Ok(ConstValue::Str(s.clone())),

        // Method call: self.len()
        Expr::Call(callee, args) if args.is_empty() => {
            if let Expr::FieldAccess(obj, method) = callee.as_ref() {
                let obj_val = eval_expr(obj, var_name, value)?;
                match (method.as_str(), &obj_val) {
                    ("len", ConstValue::Str(s)) => Ok(ConstValue::Int(s.len() as i64)),
                    _ => Err(format!(
                        "unsupported method `.{method}()` in refinement predicate"
                    )),
                }
            } else {
                Err("unsupported call expression in refinement predicate".into())
            }
        }

        // Unary operators
        Expr::UnaryOp(op, inner) => {
            let v = eval_expr(inner, var_name, value)?;
            match (op, &v) {
                (UnaryOp::Neg, ConstValue::Int(n)) => n
                    .checked_neg()
                    .map(ConstValue::Int)
                    .ok_or_else(|| "integer overflow in refinement predicate".into()),
                (UnaryOp::Neg, ConstValue::Float(f)) => Ok(ConstValue::Float(-f)),
                (UnaryOp::Not, ConstValue::Bool(b)) => Ok(ConstValue::Bool(!b)),
                _ => Err(format!("unsupported unary op `{op:?}` on {}", describe(&v))),
            }
        }

        // Binary operators
        Expr::BinOp(lhs, op, rhs) => {
            let l = eval_expr(lhs, var_name, value)?;
            let r = eval_expr(rhs, var_name, value)?;
            eval_binop(&l, op, &r)
        }

        _ => Err("expression not supported in L0 refinement predicate".into()),
    }
}

fn eval_binop(l: &ConstValue, op: &BinOp, r: &ConstValue) -> Result<ConstValue, String> {
    match (l, r) {
        // Int × Int
        (ConstValue::Int(a), ConstValue::Int(b)) => match op {
            BinOp::Add => a
                .checked_add(*b)
                .map(ConstValue::Int)
                .ok_or_else(|| "integer overflow in refinement predicate".into()),
            BinOp::Sub => a
                .checked_sub(*b)
                .map(ConstValue::Int)
                .ok_or_else(|| "integer overflow in refinement predicate".into()),
            BinOp::Mul => a
                .checked_mul(*b)
                .map(ConstValue::Int)
                .ok_or_else(|| "integer overflow in refinement predicate".into()),
            BinOp::Lt => Ok(ConstValue::Bool(*a < *b)),
            BinOp::Le => Ok(ConstValue::Bool(*a <= *b)),
            BinOp::Gt => Ok(ConstValue::Bool(*a > *b)),
            BinOp::Ge => Ok(ConstValue::Bool(*a >= *b)),
            BinOp::Eq => Ok(ConstValue::Bool(*a == *b)),
            BinOp::Ne => Ok(ConstValue::Bool(*a != *b)),
            _ => Err(format!("unsupported op `{op:?}` on Int")),
        },
        // Float × Float
        (ConstValue::Float(a), ConstValue::Float(b)) => match op {
            BinOp::Add => Ok(ConstValue::Float(a + b)),
            BinOp::Sub => Ok(ConstValue::Float(a - b)),
            BinOp::Mul => Ok(ConstValue::Float(a * b)),
            BinOp::Lt => Ok(ConstValue::Bool(*a < *b)),
            BinOp::Le => Ok(ConstValue::Bool(*a <= *b)),
            BinOp::Gt => Ok(ConstValue::Bool(*a > *b)),
            BinOp::Ge => Ok(ConstValue::Bool(*a >= *b)),
            BinOp::Eq => Ok(ConstValue::Bool(*a == *b)),
            BinOp::Ne => Ok(ConstValue::Bool(*a != *b)),
            _ => Err(format!("unsupported op `{op:?}` on Float")),
        },
        // Bool × Bool (logical connectives)
        (ConstValue::Bool(a), ConstValue::Bool(b)) => match op {
            BinOp::And => Ok(ConstValue::Bool(*a && *b)),
            BinOp::Or => Ok(ConstValue::Bool(*a || *b)),
            BinOp::Eq => Ok(ConstValue::Bool(*a == *b)),
            BinOp::Ne => Ok(ConstValue::Bool(*a != *b)),
            _ => Err(format!("unsupported op `{op:?}` on Bool")),
        },
        // String comparisons
        (ConstValue::Str(a), ConstValue::Str(b)) => match op {
            BinOp::Eq => Ok(ConstValue::Bool(*a == *b)),
            BinOp::Ne => Ok(ConstValue::Bool(*a != *b)),
            _ => Err(format!("unsupported op `{op:?}` on String")),
        },
        _ => Err(format!(
            "type mismatch in refinement: cannot apply `{op:?}` to {} and {}",
            describe(l),
            describe(r)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int_expr(n: i64) -> Expr {
        Expr::IntLit(n)
    }

    fn var_expr(name: &str) -> Expr {
        Expr::Var(name.into())
    }

    fn binop(l: Expr, op: BinOp, r: Expr) -> Expr {
        Expr::BinOp(Box::new(l), op, Box::new(r))
    }

    #[test]
    fn test_simple_gt() {
        // x > 0
        let pred = binop(var_expr("x"), BinOp::Gt, int_expr(0));
        assert!(eval_refinement_predicate(&pred, "x", &ConstValue::Int(5)).unwrap());
        assert!(!eval_refinement_predicate(&pred, "x", &ConstValue::Int(-1)).unwrap());
    }

    #[test]
    fn test_range_check() {
        // self >= 1 && self <= 65535
        let lower = binop(var_expr("self"), BinOp::Ge, int_expr(1));
        let upper = binop(var_expr("self"), BinOp::Le, int_expr(65535));
        let pred = binop(lower, BinOp::And, upper);

        assert!(eval_refinement_predicate(&pred, "self", &ConstValue::Int(80)).unwrap());
        assert!(eval_refinement_predicate(&pred, "self", &ConstValue::Int(1)).unwrap());
        assert!(eval_refinement_predicate(&pred, "self", &ConstValue::Int(65535)).unwrap());
        assert!(!eval_refinement_predicate(&pred, "self", &ConstValue::Int(0)).unwrap());
        assert!(!eval_refinement_predicate(&pred, "self", &ConstValue::Int(70000)).unwrap());
    }

    #[test]
    fn test_string_len() {
        // self.len() > 0
        let len_call = Expr::Call(
            Box::new(Expr::FieldAccess(Box::new(var_expr("self")), "len".into())),
            vec![],
        );
        let pred = binop(len_call, BinOp::Gt, int_expr(0));

        assert!(
            eval_refinement_predicate(&pred, "self", &ConstValue::Str("hello".into())).unwrap()
        );
        assert!(!eval_refinement_predicate(&pred, "self", &ConstValue::Str("".into())).unwrap());
    }

    #[test]
    fn test_arithmetic() {
        // x + 1 > 5
        let pred = binop(
            binop(var_expr("x"), BinOp::Add, int_expr(1)),
            BinOp::Gt,
            int_expr(5),
        );
        assert!(eval_refinement_predicate(&pred, "x", &ConstValue::Int(5)).unwrap());
        assert!(!eval_refinement_predicate(&pred, "x", &ConstValue::Int(4)).unwrap());
    }

    #[test]
    fn test_negation() {
        // !(x == 0)
        let pred = Expr::UnaryOp(
            UnaryOp::Not,
            Box::new(binop(var_expr("x"), BinOp::Eq, int_expr(0))),
        );
        assert!(eval_refinement_predicate(&pred, "x", &ConstValue::Int(1)).unwrap());
        assert!(!eval_refinement_predicate(&pred, "x", &ConstValue::Int(0)).unwrap());
    }

    #[test]
    fn test_integer_add_overflow_is_undecidable() {
        let pred = binop(
            binop(var_expr("x"), BinOp::Add, int_expr(1)),
            BinOp::Gt,
            int_expr(0),
        );
        let err = eval_refinement_predicate(&pred, "x", &ConstValue::Int(i64::MAX)).unwrap_err();
        assert!(err.contains("overflow"));
    }

    #[test]
    fn test_integer_neg_overflow_is_undecidable() {
        let pred = Expr::UnaryOp(UnaryOp::Neg, Box::new(var_expr("x")));
        let err = eval_expr(&pred, "x", &ConstValue::Int(i64::MIN)).unwrap_err();
        assert!(err.contains("overflow"));
    }
}
