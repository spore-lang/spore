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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostExpr {
    Const(u64),
    Var(String),
    Add(Box<CostExpr>, Box<CostExpr>),
    Mul(Box<CostExpr>, Box<CostExpr>),
    Pow(Box<CostExpr>, u32),
    Log(Box<CostExpr>),
    Max(Box<CostExpr>, Box<CostExpr>),
    Min(Box<CostExpr>, Box<CostExpr>),
    /// Linear in a named variable — represents O(n) cost.
    Linear(String),
    /// Unbounded / unknown cost — analysis could not determine a bound.
    Unbounded,
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
            CostExpr::Linear(v) => write!(f, "O({v})"),
            CostExpr::Unbounded => write!(f, "∞"),
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

impl Default for CostAnalyzer {
    fn default() -> Self {
        Self::new()
    }
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

    /// Mutable access to results — useful for testing and manual insertion.
    pub fn results_mut(&mut self) -> &mut HashMap<String, CostResult> {
        &mut self.results
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
            if let Expr::Var(name) = callee.as_ref()
                && name == fn_name
            {
                let classified: Vec<CallArg> = args.iter().map(classify_arg).collect();
                out.push(classified);
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
        Expr::Try(inner) | Expr::Spawn(inner) | Expr::Await(inner) | Expr::Throw(inner) => {
            collect_recursive_calls(fn_name, inner, out);
        }
        Expr::Return(inner) => {
            if let Some(e) = inner {
                collect_recursive_calls(fn_name, e, out);
            }
        }
        Expr::List(elems) => {
            for e in elems {
                collect_recursive_calls(fn_name, e, out);
            }
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
        Expr::TString(parts) => {
            for part in parts {
                if let ast::TStringPart::Expr(e) = part {
                    collect_recursive_calls(fn_name, e, out);
                }
            }
        }
        // Leaves — no recursion
        Expr::IntLit(_)
        | Expr::FloatLit(_)
        | Expr::StrLit(_)
        | Expr::BoolLit(_)
        | Expr::CharLit(_)
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

// ---------------------------------------------------------------------------
// Four-dimensional cost model (SEP-0004)
// ---------------------------------------------------------------------------

/// Four-dimensional cost vector per the spec (SEP-0004).
///
/// Each function maps to one of these, describing:
/// - **compute** — operation count
/// - **alloc** — memory cell allocation
/// - **io** — I/O call count
/// - **parallel** — lane/spawn count
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostVector {
    pub compute: CostExpr,
    pub alloc: CostExpr,
    pub io: CostExpr,
    pub parallel: CostExpr,
}

impl CostVector {
    pub fn constant(compute: u64, alloc: u64, io: u64, parallel: u64) -> Self {
        Self {
            compute: CostExpr::Const(compute),
            alloc: CostExpr::Const(alloc),
            io: CostExpr::Const(io),
            parallel: CostExpr::Const(parallel),
        }
    }

    pub fn zero() -> Self {
        Self::constant(0, 0, 0, 0)
    }

    /// Check if all dimensions are bounded (no `Unbounded`).
    pub fn is_bounded(&self) -> bool {
        !matches!(self.compute, CostExpr::Unbounded)
            && !matches!(self.alloc, CostExpr::Unbounded)
            && !matches!(self.io, CostExpr::Unbounded)
            && !matches!(self.parallel, CostExpr::Unbounded)
    }

    /// Sequentially compose two cost vectors (both execute).
    ///
    /// compute = sum, alloc/io/parallel = max.
    pub fn seq(&self, other: &CostVector) -> CostVector {
        CostVector {
            compute: add_cost(&self.compute, &other.compute),
            alloc: max_cost(&self.alloc, &other.alloc),
            io: max_cost(&self.io, &other.io),
            parallel: max_cost(&self.parallel, &other.parallel),
        }
    }

    /// Parallel compose (spawn): compute = max, alloc = sum, parallel += 1.
    pub fn par(&self, other: &CostVector) -> CostVector {
        CostVector {
            compute: max_cost(&self.compute, &other.compute),
            alloc: add_cost(&self.alloc, &other.alloc),
            io: max_cost(&self.io, &other.io),
            parallel: add_cost(&self.parallel, &CostExpr::Const(1)),
        }
    }

    /// Scale by a loop/recursion factor.
    ///
    /// compute and alloc are multiplied, io increments by 1.
    pub fn scale(&self, factor: &CostExpr) -> CostVector {
        CostVector {
            compute: mul_cost(&self.compute, factor),
            alloc: mul_cost(&self.alloc, factor),
            io: add_cost(&self.io, &CostExpr::Const(1)),
            parallel: self.parallel.clone(),
        }
    }
}

impl std::fmt::Display for CostVector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CostVector {{ compute: {}, alloc: {}, io: {}, parallel: {} }}",
            self.compute, self.alloc, self.io, self.parallel
        )
    }
}

/// Helper: add two cost expressions.
fn add_cost(a: &CostExpr, b: &CostExpr) -> CostExpr {
    match (a, b) {
        (CostExpr::Const(0), other) | (other, CostExpr::Const(0)) => other.clone(),
        (CostExpr::Const(x), CostExpr::Const(y)) => CostExpr::Const(x + y),
        (CostExpr::Unbounded, _) | (_, CostExpr::Unbounded) => CostExpr::Unbounded,
        _ => CostExpr::Unbounded, // Conservative: can't simplify symbolic
    }
}

/// Helper: max of two cost expressions.
fn max_cost(a: &CostExpr, b: &CostExpr) -> CostExpr {
    match (a, b) {
        (CostExpr::Const(x), CostExpr::Const(y)) => CostExpr::Const(*x.max(y)),
        (CostExpr::Unbounded, _) | (_, CostExpr::Unbounded) => CostExpr::Unbounded,
        (CostExpr::Const(0), other) | (other, CostExpr::Const(0)) => other.clone(),
        _ => CostExpr::Unbounded,
    }
}

/// Helper: multiply two cost expressions.
fn mul_cost(a: &CostExpr, b: &CostExpr) -> CostExpr {
    match (a, b) {
        (CostExpr::Const(0), _) | (_, CostExpr::Const(0)) => CostExpr::Const(0),
        (CostExpr::Const(1), other) | (other, CostExpr::Const(1)) => other.clone(),
        (CostExpr::Const(x), CostExpr::Const(y)) => CostExpr::Const(x * y),
        (CostExpr::Unbounded, _) | (_, CostExpr::Unbounded) => CostExpr::Unbounded,
        _ => CostExpr::Unbounded,
    }
}

/// Convert a [`CostResult`] into its time-dimension [`CostExpr`].
fn cost_result_to_expr(result: &CostResult) -> CostExpr {
    match result {
        CostResult::Constant(n) => CostExpr::Const(*n),
        CostResult::Structural(param) => CostExpr::Linear(param.clone()),
        CostResult::Declared(expr) => expr.clone(),
        CostResult::Unbounded => CostExpr::Unbounded,
        CostResult::Unknown(_) => CostExpr::Unbounded,
    }
}

/// Analyzes functions and produces four-dimensional cost vectors.
///
/// Works on top of [`CostAnalyzer`] — it reuses the existing single-dimension
/// analysis as the *time* component and derives the other three dimensions.
pub struct CostChecker {
    /// Function name → cost vector.
    pub costs: HashMap<String, CostVector>,
}

impl CostChecker {
    pub fn new() -> Self {
        Self {
            costs: HashMap::new(),
        }
    }

    /// Derive [`CostVector`]s for every function already analyzed by `analyzer`.
    pub fn check_all(&mut self, analyzer: &CostAnalyzer) {
        for (name, result) in analyzer.results() {
            let cv = Self::cost_vector_from_result(result);
            self.costs.insert(name.clone(), cv);
        }
    }

    /// Build a [`CostVector`] from a single [`CostResult`].
    fn cost_vector_from_result(result: &CostResult) -> CostVector {
        let compute = cost_result_to_expr(result);

        // I/O call depth: structural recursion ⇒ O(n), otherwise O(1).
        let io = match result {
            CostResult::Structural(_) => CostExpr::Linear("n".into()),
            _ => CostExpr::Const(1),
        };

        // Alloc: conservatively mirrors compute for now.
        let alloc = compute.clone();

        // Parallel: 0 by default (spawns are not yet tracked by CostAnalyzer).
        let parallel = CostExpr::Const(0);

        CostVector {
            compute,
            alloc,
            io,
            parallel,
        }
    }
}

impl Default for CostChecker {
    fn default() -> Self {
        Self::new()
    }
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

    // -----------------------------------------------------------------------
    // CostVector tests
    // -----------------------------------------------------------------------

    #[test]
    fn zero_cost() {
        let c = CostVector::zero();
        assert!(c.is_bounded());
        assert_eq!(c.compute, CostExpr::Const(0));
        assert_eq!(c.alloc, CostExpr::Const(0));
        assert_eq!(c.io, CostExpr::Const(0));
        assert_eq!(c.parallel, CostExpr::Const(0));
    }

    #[test]
    fn sequential_composition() {
        let a = CostVector::constant(5, 2, 1, 0);
        let b = CostVector::constant(3, 4, 1, 0);
        let c = a.seq(&b);
        assert_eq!(c.compute, CostExpr::Const(8)); // sum
        assert_eq!(c.alloc, CostExpr::Const(4)); // max
        assert_eq!(c.io, CostExpr::Const(1)); // max
        assert_eq!(c.parallel, CostExpr::Const(0)); // max
    }

    #[test]
    fn parallel_composition() {
        let a = CostVector::constant(5, 2, 1, 0);
        let b = CostVector::constant(3, 4, 1, 0);
        let c = a.par(&b);
        assert_eq!(c.compute, CostExpr::Const(5)); // max
        assert_eq!(c.alloc, CostExpr::Const(6)); // sum
        assert_eq!(c.parallel, CostExpr::Const(1)); // +1 for spawn
    }

    #[test]
    fn unbounded_propagation() {
        let a = CostVector {
            compute: CostExpr::Unbounded,
            alloc: CostExpr::Const(1),
            io: CostExpr::Const(1),
            parallel: CostExpr::Const(0),
        };
        assert!(!a.is_bounded());
        let b = CostVector::constant(1, 1, 1, 0);
        let c = a.seq(&b);
        assert_eq!(c.compute, CostExpr::Unbounded);
    }

    #[test]
    fn scale_by_factor() {
        let a = CostVector::constant(3, 2, 1, 0);
        let scaled = a.scale(&CostExpr::Const(10));
        assert_eq!(scaled.compute, CostExpr::Const(30));
        assert_eq!(scaled.alloc, CostExpr::Const(20));
        assert_eq!(scaled.io, CostExpr::Const(2)); // +1 for recursion depth
    }

    #[test]
    fn cost_vector_display() {
        let c = CostVector::constant(5, 3, 1, 0);
        let s = c.to_string();
        assert!(s.contains("compute"));
        assert!(s.contains("alloc"));
        assert!(s.contains("io"));
        assert!(s.contains("parallel"));
    }

    #[test]
    fn cost_vector_display_unbounded() {
        let c = CostVector {
            compute: CostExpr::Unbounded,
            alloc: CostExpr::Const(0),
            io: CostExpr::Const(1),
            parallel: CostExpr::Const(0),
        };
        let s = c.to_string();
        assert!(s.contains('∞'));
    }

    #[test]
    fn add_cost_identity() {
        assert_eq!(
            add_cost(&CostExpr::Const(0), &CostExpr::Const(7)),
            CostExpr::Const(7)
        );
        assert_eq!(
            add_cost(&CostExpr::Const(3), &CostExpr::Const(0)),
            CostExpr::Const(3)
        );
    }

    #[test]
    fn mul_cost_identity_and_zero() {
        assert_eq!(
            mul_cost(&CostExpr::Const(1), &CostExpr::Const(42)),
            CostExpr::Const(42)
        );
        assert_eq!(
            mul_cost(&CostExpr::Const(0), &CostExpr::Const(42)),
            CostExpr::Const(0)
        );
    }

    #[test]
    fn cost_result_to_expr_conversions() {
        assert_eq!(
            cost_result_to_expr(&CostResult::Constant(5)),
            CostExpr::Const(5)
        );
        assert_eq!(
            cost_result_to_expr(&CostResult::Structural("n".into())),
            CostExpr::Linear("n".into())
        );
        assert_eq!(
            cost_result_to_expr(&CostResult::Unbounded),
            CostExpr::Unbounded
        );
        assert_eq!(
            cost_result_to_expr(&CostResult::Unknown("msg".into())),
            CostExpr::Unbounded
        );
    }

    #[test]
    fn cost_checker_constant_function() {
        let mut analyzer = CostAnalyzer::new();
        // Manually insert a constant result to test the checker.
        analyzer
            .results_mut()
            .insert("foo".into(), CostResult::Constant(1));

        let mut checker = CostChecker::new();
        checker.check_all(&analyzer);

        let cv = checker.costs.get("foo").expect("foo should be analyzed");
        assert_eq!(cv.compute, CostExpr::Const(1));
        assert_eq!(cv.io, CostExpr::Const(1));
        assert!(cv.is_bounded());
    }

    #[test]
    fn cost_checker_structural_function() {
        let mut analyzer = CostAnalyzer::new();
        analyzer
            .results_mut()
            .insert("bar".into(), CostResult::Structural("n".into()));

        let mut checker = CostChecker::new();
        checker.check_all(&analyzer);

        let cv = checker.costs.get("bar").expect("bar should be analyzed");
        assert_eq!(cv.compute, CostExpr::Linear("n".into()));
        assert_eq!(cv.io, CostExpr::Linear("n".into()));
        assert!(cv.is_bounded());
    }
}
