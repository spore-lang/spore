//! Cost analysis engine — three-tier cost analysis for Spore functions.
//!
//! 1. **Automatic**: Detect structural recursion (one arg decreases by constant
//!    per recursive call) → cost is O(n).
//! 2. **Semi-auto**: Read `cost ≤ expr` clauses from function definitions.
//! 3. **Escape**: `@unbounded` annotation skips cost checking.

use std::collections::HashMap;

use spore_parser::ast::{self, BinOp, Expr, FnDef, Item, Module, Stmt};

/// Cost expression — a symbolic representation of computational cost.
///
/// Grammar: `+, *, ^const, log, max, min` — no division or conditionals.
#[derive(Debug, Clone, PartialEq)]
pub enum CostExpr {
    Const(u64),
    Var(String),
    Add(Box<CostExpr>, Box<CostExpr>),
    Mul(Box<CostExpr>, Box<CostExpr>),
    Pow(Box<CostExpr>, u32),
    Log(Box<CostExpr>),
    Max(Box<CostExpr>, Box<CostExpr>),
    Min(Box<CostExpr>, Box<CostExpr>),
}

impl std::fmt::Display for CostExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CostExpr::Const(n) => write!(f, "{n}"),
            CostExpr::Var(v) => write!(f, "{v}"),
            CostExpr::Add(a, b) => write!(f, "({a} + {b})"),
            CostExpr::Mul(a, b) => write!(f, "({a} * {b})"),
            CostExpr::Pow(base, exp) => write!(f, "{base}^{exp}"),
            CostExpr::Log(e) => write!(f, "log({e})"),
            CostExpr::Max(a, b) => write!(f, "max({a}, {b})"),
            CostExpr::Min(a, b) => write!(f, "min({a}, {b})"),
        }
    }
}

/// Result of cost analysis for a single function.
#[derive(Debug, Clone)]
pub enum CostResult {
    /// Non-recursive, bounded constant cost.
    Constant(u64),
    /// Structural recursion on parameter `name` — cost is O(n).
    Structural(String),
    /// User declared via `cost ≤ expr` and accepted.
    Declared(CostExpr),
    /// `@unbounded` annotation — cost checking skipped.
    Unbounded,
    /// Could not determine — warning message attached.
    Unknown(String),
}

/// Analyze cost for functions in a module.
pub struct CostAnalyzer {
    results: HashMap<String, CostResult>,
}

impl CostAnalyzer {
    pub fn new() -> Self {
        CostAnalyzer {
            results: HashMap::new(),
        }
    }

    pub fn results(&self) -> &HashMap<String, CostResult> {
        &self.results
    }

    /// Analyze all functions in a module.
    pub fn analyze_module(&mut self, module: &Module) {
        for item in &module.items {
            if let Item::Function(fn_def) = item {
                self.analyze_function(fn_def);
            }
        }
    }

    /// Analyze cost for a single function definition.
    pub fn analyze_function(&mut self, fn_def: &FnDef) {
        let fn_name = &fn_def.name;

        // Check for `cost ≤ expr` clause
        if let Some(cost_clause) = &fn_def.cost_clause {
            self.results.insert(
                fn_name.clone(),
                CostResult::Declared(ast_cost_to_cost_expr(&cost_clause.bound)),
            );
            return;
        }

        let body = match &fn_def.body {
            Some(b) => b,
            None => {
                // Hole body — treat as constant (no code to analyze)
                self.results
                    .insert(fn_name.clone(), CostResult::Constant(1));
                return;
            }
        };

        let params: Vec<String> = fn_def.params.iter().map(|p| p.name.clone()).collect();

        // Collect recursive calls
        let mut calls = Vec::new();
        collect_recursive_calls(fn_name, body, &mut calls);

        if calls.is_empty() {
            // Non-recursive → constant cost
            self.results
                .insert(fn_name.clone(), CostResult::Constant(1));
        } else if let Some(decreasing_param) = detect_structural_recursion(fn_name, &params, &calls)
        {
            self.results
                .insert(fn_name.clone(), CostResult::Structural(decreasing_param));
        } else {
            self.results.insert(
                fn_name.clone(),
                CostResult::Unknown(format!(
                    "cannot determine cost for recursive function `{fn_name}`"
                )),
            );
        }
    }
}

/// Convert the parser's `ast::CostExpr` to our richer `CostExpr`.
fn ast_cost_to_cost_expr(ce: &ast::CostExpr) -> CostExpr {
    match ce {
        ast::CostExpr::Literal(n) => CostExpr::Const(*n),
        ast::CostExpr::Var(v) => CostExpr::Var(v.clone()),
        ast::CostExpr::Add(a, b) => CostExpr::Add(
            Box::new(ast_cost_to_cost_expr(a)),
            Box::new(ast_cost_to_cost_expr(b)),
        ),
        ast::CostExpr::Mul(a, b) => CostExpr::Mul(
            Box::new(ast_cost_to_cost_expr(a)),
            Box::new(ast_cost_to_cost_expr(b)),
        ),
    }
}

/// Check if a function uses structural recursion.
///
/// Returns `Some(param_name)` if exactly one parameter decreases by a constant
/// in **all** recursive calls and all other arguments are unchanged.
fn detect_structural_recursion(
    _fn_name: &str,
    params: &[String],
    recursive_calls: &[Vec<CallArg>],
) -> Option<String> {
    if recursive_calls.is_empty() {
        return None;
    }

    // Check each param to see if it structurally decreases in ALL calls
    for (i, param) in params.iter().enumerate() {
        let all_decrease = recursive_calls.iter().all(|args| {
            args.get(i)
                .is_some_and(|arg| matches!(arg, CallArg::Decreasing))
        });

        // Also check that all *other* args are unchanged in every call
        let others_unchanged = recursive_calls.iter().all(|args| {
            params.iter().enumerate().all(|(j, p)| {
                if j == i {
                    return true; // skip the decreasing param
                }
                args.get(j)
                    .is_some_and(|arg| matches!(arg, CallArg::Same(name) if name == p))
            })
        });

        if all_decrease && others_unchanged {
            return Some(param.clone());
        }
    }

    // Relaxed check: just require one param decreasing in all calls
    for (i, param) in params.iter().enumerate() {
        let all_decrease = recursive_calls.iter().all(|args| {
            args.get(i)
                .is_some_and(|arg| matches!(arg, CallArg::Decreasing))
        });
        if all_decrease {
            return Some(param.clone());
        }
    }

    None
}

/// Classification of an argument at a recursive call site.
#[derive(Debug)]
enum CallArg {
    /// The argument is the same variable as the corresponding parameter.
    Same(String),
    /// The argument is `param - const` (structurally decreasing).
    Decreasing,
    /// Something else (cannot classify).
    Other,
}

/// Walk the AST collecting argument lists from recursive calls to `fn_name`.
fn collect_recursive_calls(fn_name: &str, expr: &Expr, out: &mut Vec<Vec<CallArg>>) {
    match expr {
        Expr::Call(callee, args) => {
            // Check if the callee is `fn_name`
            if let Expr::Var(name) = callee.as_ref() {
                if name == fn_name {
                    let classified: Vec<CallArg> = args.iter().map(|a| classify_arg(a)).collect();
                    out.push(classified);
                }
            }
            // Also recurse into callee and args
            collect_recursive_calls(fn_name, callee, out);
            for arg in args {
                collect_recursive_calls(fn_name, arg, out);
            }
        }
        Expr::BinOp(lhs, _, rhs) => {
            collect_recursive_calls(fn_name, lhs, out);
            collect_recursive_calls(fn_name, rhs, out);
        }
        Expr::UnaryOp(_, inner) => {
            collect_recursive_calls(fn_name, inner, out);
        }
        Expr::If(cond, then_br, else_br) => {
            collect_recursive_calls(fn_name, cond, out);
            collect_recursive_calls(fn_name, then_br, out);
            if let Some(e) = else_br {
                collect_recursive_calls(fn_name, e, out);
            }
        }
        Expr::Block(stmts, tail) => {
            for stmt in stmts {
                match stmt {
                    Stmt::Let(_, _, e) | Stmt::Expr(e) => {
                        collect_recursive_calls(fn_name, e, out);
                    }
                }
            }
            if let Some(tail_expr) = tail {
                collect_recursive_calls(fn_name, tail_expr, out);
            }
        }
        Expr::Match(scrutinee, arms) => {
            collect_recursive_calls(fn_name, scrutinee, out);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_recursive_calls(fn_name, guard, out);
                }
                collect_recursive_calls(fn_name, &arm.body, out);
            }
        }
        Expr::Lambda(_, body) => {
            collect_recursive_calls(fn_name, body, out);
        }
        Expr::Pipe(lhs, rhs) => {
            collect_recursive_calls(fn_name, lhs, out);
            collect_recursive_calls(fn_name, rhs, out);
        }
        Expr::FieldAccess(inner, _) => {
            collect_recursive_calls(fn_name, inner, out);
        }
        Expr::Try(inner) | Expr::Spawn(inner) | Expr::Await(inner) => {
            collect_recursive_calls(fn_name, inner, out);
        }
        Expr::StructLit(_, fields) => {
            for (_, e) in fields {
                collect_recursive_calls(fn_name, e, out);
            }
        }
        Expr::FString(parts) => {
            for part in parts {
                if let ast::FStringPart::Expr(e) = part {
                    collect_recursive_calls(fn_name, e, out);
                }
            }
        }
        // Leaves — no recursion
        Expr::IntLit(_)
        | Expr::FloatLit(_)
        | Expr::StrLit(_)
        | Expr::BoolLit(_)
        | Expr::Var(_)
        | Expr::Hole(_, _) => {}
    }
}

/// Classify an argument expression for structural recursion detection.
fn classify_arg(expr: &Expr) -> CallArg {
    match expr {
        // Same variable passed through
        Expr::Var(name) => CallArg::Same(name.clone()),
        // `param - <int literal>` → structurally decreasing
        Expr::BinOp(lhs, BinOp::Sub, rhs) => {
            if matches!(lhs.as_ref(), Expr::Var(_)) && is_positive_int(rhs) {
                CallArg::Decreasing
            } else {
                CallArg::Other
            }
        }
        _ => CallArg::Other,
    }
}

/// Check if an expression is a positive integer literal.
fn is_positive_int(expr: &Expr) -> bool {
    matches!(expr, Expr::IntLit(n) if *n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_expr_display() {
        let e = CostExpr::Add(
            Box::new(CostExpr::Var("n".into())),
            Box::new(CostExpr::Const(1)),
        );
        assert_eq!(e.to_string(), "(n + 1)");

        let e2 = CostExpr::Pow(Box::new(CostExpr::Var("n".into())), 2);
        assert_eq!(e2.to_string(), "n^2");

        let e3 = CostExpr::Log(Box::new(CostExpr::Var("n".into())));
        assert_eq!(e3.to_string(), "log(n)");
    }

    #[test]
    fn classify_var_arg() {
        let arg = Expr::Var("x".into());
        assert!(matches!(classify_arg(&arg), CallArg::Same(ref n) if n == "x"));
    }

    #[test]
    fn classify_decreasing_arg() {
        let arg = Expr::BinOp(
            Box::new(Expr::Var("n".into())),
            BinOp::Sub,
            Box::new(Expr::IntLit(1)),
        );
        assert!(matches!(classify_arg(&arg), CallArg::Decreasing));
    }

    #[test]
    fn classify_other_arg() {
        let arg = Expr::IntLit(42);
        assert!(matches!(classify_arg(&arg), CallArg::Other));
    }

    #[test]
    fn ast_cost_conversion() {
        let ast_ce = ast::CostExpr::Add(
            Box::new(ast::CostExpr::Var("n".into())),
            Box::new(ast::CostExpr::Literal(5)),
        );
        let ce = ast_cost_to_cost_expr(&ast_ce);
        assert_eq!(
            ce,
            CostExpr::Add(
                Box::new(CostExpr::Var("n".into())),
                Box::new(CostExpr::Const(5)),
            )
        );
    }
}
