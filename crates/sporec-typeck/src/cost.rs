//! Cost analysis engine — three-tier cost analysis for Spore functions.
//!
//! 1. **Automatic**: Detect structural recursion (one arg decreases by constant
//!    per recursive call) → cost is O(n).
//! 2. **Semi-auto**: Read `cost [compute, alloc, io, parallel]` clauses.
//! 3. **Escape**: `@unbounded` annotation skips cost checking.

use std::collections::HashMap;

use sporec_parser::ast::{self, BinOp, Expr, FnDef, HandleBinding, Item, Module, SelectArm, Stmt};

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
#[must_use]
pub enum CostResult {
    /// Non-recursive, bounded constant cost.
    Constant(u64),
    /// Structural recursion on parameter `name` — cost is O(n).
    Structural(String),
    /// User declared via `cost [compute, alloc, io, parallel]`; stores compute part.
    Declared(CostExpr),
    /// `@unbounded` annotation — cost checking skipped.
    Unbounded,
    /// Could not determine — warning message attached.
    Unknown(String),
}

/// Known I/O (capability-gated) function names.
const IO_FUNCTIONS: &[&str] = &[
    "print",
    "println",
    "eprintln",
    "eprint",
    "read_line",
    "read",
    "write",
    "open",
    "close",
    "send",
    "recv",
];

fn is_io_function(name: &str) -> bool {
    IO_FUNCTIONS.contains(&name)
}

/// Compute cost of a binary op in the *compute* dimension (SEP-0004 table).
fn binop_compute_cost(op: &BinOp, is_float: bool) -> u64 {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul => {
            if is_float {
                2
            } else {
                1
            }
        }
        BinOp::Div | BinOp::Mod => {
            if is_float {
                3
            } else {
                2
            }
        }
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 1,
        // Logical, bitwise, shift — 1 op each.
        BinOp::And
        | BinOp::Or
        | BinOp::BitAnd
        | BinOp::BitOr
        | BinOp::BitXor
        | BinOp::Shl
        | BinOp::Shr => 1,
    }
}

/// Best-effort check: is this expression obviously a float?
fn is_float_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::FloatLit(_))
}

/// Analyze cost for functions in a module.
pub struct CostAnalyzer {
    results: HashMap<String, CostResult>,
    /// Per-function four-dimensional cost vectors computed by AST walk.
    cost_vectors: HashMap<String, CostVector>,
    /// Functions whose body cost exceeds their declared budget.
    /// Each entry: (fn_name, declared_bound, actual_cost).
    violations: Vec<(String, CostVector, CostVector)>,
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
            cost_vectors: HashMap::new(),
            violations: Vec::new(),
        }
    }

    pub fn results(&self) -> &HashMap<String, CostResult> {
        &self.results
    }

    /// Mutable access to results — useful for testing and manual insertion.
    pub fn results_mut(&mut self) -> &mut HashMap<String, CostResult> {
        &mut self.results
    }

    /// Budget violations found during analysis.
    pub fn violations(&self) -> &[(String, CostVector, CostVector)] {
        &self.violations
    }

    /// Four-dimensional cost vectors computed by AST walk.
    pub fn cost_vectors(&self) -> &HashMap<String, CostVector> {
        &self.cost_vectors
    }

    /// Analyze all functions in a module.
    pub fn analyze_module(&mut self, module: &Module) {
        // First pass: analyze each function independently (1D CostResult)
        for item in &module.items {
            if let Item::Function(fn_def) = item {
                self.analyze_function(fn_def);
            }
        }
        // 4D vector pass: walk each function body for per-expression costs
        for item in &module.items {
            if let Item::Function(fn_def) = item {
                self.analyze_function_vector(fn_def);
            }
        }
        // Second pass: propagate callee costs into callers
        self.propagate_callee_costs(module);
        // Third pass: check declared budgets against actual costs
        self.check_budgets(module);
    }

    /// Analyze cost for a single function definition.
    pub fn analyze_function(&mut self, fn_def: &FnDef) {
        let fn_name = &fn_def.name;

        // @unbounded → skip analysis entirely
        if fn_def.is_unbounded {
            self.results.insert(fn_name.clone(), CostResult::Unbounded);
            return;
        }

        // If there's a cost clause, record the declared cost for callers,
        // but also analyze the body (budget checking happens later).
        let declared = fn_def
            .cost_clause
            .as_ref()
            .map(|cc| ast_cost_to_cost_expr(&cc.compute));

        let body = match &fn_def.body {
            Some(b) => b,
            None => {
                // Hole body — treat as constant (no code to analyze)
                let result = if let Some(d) = declared {
                    CostResult::Declared(d)
                } else {
                    CostResult::Constant(1)
                };
                self.results.insert(fn_name.clone(), result);
                return;
            }
        };

        let params: Vec<String> = fn_def.params.iter().map(|p| p.name.clone()).collect();

        // Collect recursive calls
        let mut calls = Vec::new();
        collect_recursive_calls(fn_name, body, &mut calls);

        let body_cost = if calls.is_empty() {
            // Non-recursive → constant cost
            CostResult::Constant(1)
        } else if let Some(decreasing_param) = detect_structural_recursion(fn_name, &params, &calls)
        {
            CostResult::Structural(decreasing_param)
        } else {
            CostResult::Unknown(format!(
                "cannot determine cost for recursive function `{fn_name}`"
            ))
        };

        // Store the declared cost (if any) or the analyzed body cost
        if let Some(d) = declared {
            self.results
                .insert(fn_name.clone(), CostResult::Declared(d));
        } else {
            self.results.insert(fn_name.clone(), body_cost);
        }
    }

    /// Compute the four-dimensional [`CostVector`] for a single function.
    fn analyze_function_vector(&mut self, fn_def: &FnDef) {
        let fn_name = &fn_def.name;

        if fn_def.is_unbounded {
            self.cost_vectors.insert(
                fn_name.clone(),
                CostVector {
                    compute: CostExpr::Unbounded,
                    alloc: CostExpr::Unbounded,
                    io: CostExpr::Unbounded,
                    parallel: CostExpr::Unbounded,
                },
            );
            return;
        }

        let body = match &fn_def.body {
            Some(b) => b,
            None => {
                self.cost_vectors
                    .insert(fn_name.clone(), CostVector::zero());
                return;
            }
        };

        let base_cv = self.analyze_expr_cost(body);

        // If the 1D analysis found structural recursion, scale the body cost.
        let cv = match self.results.get(fn_name) {
            Some(CostResult::Structural(param)) => base_cv.scale(&CostExpr::Linear(param.clone())),
            _ => base_cv,
        };

        self.cost_vectors.insert(fn_name.clone(), cv);
    }

    /// Walk an expression and return its four-dimensional cost.
    fn analyze_expr_cost(&self, expr: &Expr) -> CostVector {
        match expr {
            // --- Leaves (zero cost) ---
            Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::CharLit(_) => {
                CostVector::zero()
            }
            Expr::Var(_) => CostVector::zero(),

            // String literal: alloc = ⌈len/8⌉
            Expr::StrLit(s) => {
                let cells = (s.len() as u64).div_ceil(8);
                CostVector::constant(0, cells, 0, 0)
            }

            // Binary operation
            Expr::BinOp(lhs, op, rhs) => {
                let lhs_cost = self.analyze_expr_cost(lhs);
                let rhs_cost = self.analyze_expr_cost(rhs);
                let is_float = is_float_expr(lhs) || is_float_expr(rhs);
                let op_cv = CostVector::constant(binop_compute_cost(op, is_float), 0, 0, 0);
                lhs_cost.seq(&rhs_cost).seq(&op_cv)
            }

            // Unary operation: 1 compute op
            Expr::UnaryOp(_, inner) => {
                let inner_cost = self.analyze_expr_cost(inner);
                inner_cost.seq(&CostVector::constant(1, 0, 0, 0))
            }

            // Function call
            Expr::Call(callee, args) => {
                let mut total = CostVector::zero();
                for arg in args {
                    total = total.seq(&self.analyze_expr_cost(arg));
                }
                // Call overhead: 3 compute ops
                total = total.seq(&CostVector::constant(3, 0, 0, 0));

                if let Expr::Var(name) = callee.as_ref() {
                    if is_io_function(name) {
                        total = total.seq(&CostVector::constant(0, 0, 1, 0));
                    }
                    if let Some(callee_cv) = self.cost_vectors.get(name) {
                        total = total.seq(callee_cv);
                    }
                }
                total
            }

            // If expression: cond + max(then, else)
            Expr::If(cond, then_br, else_br) => {
                let cond_cost = self.analyze_expr_cost(cond);
                let then_cost = self.analyze_expr_cost(then_br);
                let else_cost = else_br
                    .as_ref()
                    .map(|e| self.analyze_expr_cost(e))
                    .unwrap_or_else(CostVector::zero);
                let branch_max = then_cost.max(&else_cost);
                cond_cost.seq(&branch_max)
            }

            // Match: scrutinee + max(arm bodies) + arms×1 compute
            Expr::Match(scrutinee, arms) => {
                let scrutinee_cost = self.analyze_expr_cost(scrutinee);
                let arm_count = arms.len() as u64;

                let mut max_arm = CostVector::zero();
                for arm in arms {
                    let mut arm_cost = self.analyze_expr_cost(&arm.body);
                    if let Some(guard) = &arm.guard {
                        arm_cost = self.analyze_expr_cost(guard).seq(&arm_cost);
                    }
                    max_arm = max_arm.max(&arm_cost);
                }
                scrutinee_cost
                    .seq(&max_arm)
                    .seq(&CostVector::constant(arm_count, 0, 0, 0))
            }

            // Block: sequential composition of statements + optional tail
            Expr::Block(stmts, tail) => {
                let mut total = CostVector::zero();
                for stmt in stmts {
                    total = total.seq(&self.analyze_stmt_cost(stmt));
                }
                if let Some(tail_expr) = tail {
                    total = total.seq(&self.analyze_expr_cost(tail_expr));
                }
                total
            }

            // Struct literal: alloc = field_count cells
            Expr::StructLit(_, fields) => {
                let mut total = CostVector::zero();
                for (_, e) in fields {
                    total = total.seq(&self.analyze_expr_cost(e));
                }
                total.seq(&CostVector::constant(0, fields.len() as u64, 0, 0))
            }

            // List literal: alloc = element_count + 1 cells
            Expr::List(elems) => {
                let mut total = CostVector::zero();
                for e in elems {
                    total = total.seq(&self.analyze_expr_cost(e));
                }
                total.seq(&CostVector::constant(0, elems.len() as u64 + 1, 0, 0))
            }

            // Lambda: alloc = captured_var_count (0 without scope analysis)
            Expr::Lambda(params, _body) => {
                // Approximate capture count as parameter count.
                CostVector::constant(0, params.len() as u64, 0, 0)
            }

            // Spawn: +1 parallel lane
            Expr::Spawn(inner) => self
                .analyze_expr_cost(inner)
                .seq(&CostVector::constant(0, 0, 0, 1)),

            Expr::Await(inner) | Expr::Try(inner) | Expr::Throw(inner) => {
                self.analyze_expr_cost(inner)
            }
            Expr::ChannelNew { buffer, .. } => self.analyze_expr_cost(buffer),

            Expr::Return(inner) => inner
                .as_ref()
                .map(|e| self.analyze_expr_cost(e))
                .unwrap_or_else(CostVector::zero),

            // Pipe: sequential
            Expr::Pipe(lhs, rhs) => self
                .analyze_expr_cost(lhs)
                .seq(&self.analyze_expr_cost(rhs)),

            Expr::FieldAccess(inner, _) => self.analyze_expr_cost(inner),

            // FString / TString: sum of parts + 1 alloc cell
            Expr::FString(parts) => {
                let mut total = CostVector::zero();
                for part in parts {
                    if let ast::FStringPart::Expr(e) = part {
                        total = total.seq(&self.analyze_expr_cost(e));
                    }
                }
                total.seq(&CostVector::constant(0, 1, 0, 0))
            }
            Expr::TString(parts) => {
                let mut total = CostVector::zero();
                for part in parts {
                    if let ast::TStringPart::Expr(e) = part {
                        total = total.seq(&self.analyze_expr_cost(e));
                    }
                }
                total.seq(&CostVector::constant(0, 1, 0, 0))
            }

            // Parallel scope: +1 parallel lane
            Expr::ParallelScope { lanes, body } => {
                let mut total = CostVector::zero();
                if let Some(lanes_expr) = lanes {
                    total = total.seq(&self.analyze_expr_cost(lanes_expr));
                }
                total
                    .seq(&self.analyze_expr_cost(body))
                    .seq(&CostVector::constant(0, 0, 0, 1))
            }

            // Select: max of arm costs
            Expr::Select(arms) => {
                let mut max_arm = CostVector::zero();
                for arm in arms {
                    let arm_cost = match arm {
                        SelectArm::Recv { source, body, .. } => self
                            .analyze_expr_cost(source)
                            .seq(&self.analyze_expr_cost(body)),
                        SelectArm::Timeout { duration, body } => self
                            .analyze_expr_cost(duration)
                            .seq(&self.analyze_expr_cost(body)),
                    };
                    max_arm = max_arm.max(&arm_cost);
                }
                max_arm
            }

            Expr::Hole(_, _, _) => CostVector::constant(1, 0, 0, 0),

            // Placeholder is desugared to Lambda by the parser; should never reach here
            Expr::Placeholder => {
                unreachable!("Placeholder should be desugared before cost analysis")
            }

            // Perform: 1 IO cost (effect invocation)
            Expr::Perform { args, .. } => {
                let mut total = CostVector::zero();
                for arg in args {
                    total = total.seq(&self.analyze_expr_cost(arg));
                }
                total.seq(&CostVector::constant(1, 0, 1, 0))
            }

            // Handle: body cost + max(handler arm costs)
            Expr::Handle { body, handlers } => {
                let mut body_cost = self.analyze_expr_cost(body);
                let mut max_arm = CostVector::zero();
                for binding in handlers {
                    match binding {
                        HandleBinding::Use(handler_use) => {
                            for (_, value) in &handler_use.payload {
                                body_cost = body_cost.seq(&self.analyze_expr_cost(value));
                            }
                        }
                        HandleBinding::On(arm) => {
                            let arm_cost = self.analyze_expr_cost(&arm.body);
                            max_arm = max_arm.max(&arm_cost);
                        }
                    }
                }
                body_cost.seq(&max_arm)
            }
        }
    }

    /// Walk a statement and return its cost.
    fn analyze_stmt_cost(&self, stmt: &Stmt) -> CostVector {
        match stmt {
            Stmt::Let(_, _, expr) | Stmt::Expr(expr) => self.analyze_expr_cost(expr),
        }
    }

    /// Propagate callee costs into caller functions.
    ///
    /// For each non-recursive constant-cost function, walk its body to find
    /// all function call sites and sum callee costs.
    fn propagate_callee_costs(&mut self, module: &Module) {
        let mut updates: Vec<(String, CostResult)> = Vec::new();

        for item in &module.items {
            let Item::Function(fn_def) = item else {
                continue;
            };
            let fn_name = &fn_def.name;
            let Some(body) = &fn_def.body else { continue };

            let current = self.results.get(fn_name).cloned();
            let base_cost = match &current {
                Some(CostResult::Constant(k)) => *k,
                _ => continue, // only propagate into constant-cost functions
            };

            let callee_names = collect_callee_names(fn_name, body);
            if callee_names.is_empty() {
                continue;
            }

            let mut total = base_cost;
            let mut upgraded = false;

            for callee in &callee_names {
                match self.results.get(callee) {
                    Some(CostResult::Constant(k)) => total = total.saturating_add(*k),
                    Some(CostResult::Declared(expr)) => match expr {
                        CostExpr::Const(k) => total = total.saturating_add(*k),
                        _ => {
                            upgraded = true;
                            break;
                        }
                    },
                    Some(
                        CostResult::Structural(_) | CostResult::Unbounded | CostResult::Unknown(_),
                    ) => {
                        upgraded = true;
                        break;
                    }
                    None => {} // unknown function (built-in, etc.) — ignore
                }
            }

            if !upgraded && total != base_cost {
                updates.push((fn_name.clone(), CostResult::Constant(total)));
            }
        }

        for (name, result) in updates {
            self.results.insert(name, result);
        }
    }

    /// Check declared cost budgets against actual/propagated costs.
    fn check_budgets(&mut self, module: &Module) {
        for item in &module.items {
            let Item::Function(fn_def) = item else {
                continue;
            };
            if fn_def.is_unbounded {
                continue;
            }
            let Some(cost_clause) = &fn_def.cost_clause else {
                continue;
            };
            let fn_name = &fn_def.name;
            let declared = ast_cost_clause_to_cost_vector(cost_clause);
            let actual = self.compute_body_cost_vector(fn_def);
            if exceeds_budget_vector(&actual, &declared) {
                self.violations.push((fn_name.clone(), declared, actual));
            }
        }
    }

    /// Compute the actual 4D body cost for budget comparison.
    fn compute_body_cost_vector(&self, fn_def: &FnDef) -> CostVector {
        let Some(body) = &fn_def.body else {
            return CostVector::constant(1, 0, 0, 0);
        };
        self.analyze_expr_cost(body)
    }
}

/// Convert the parser's `ast::CostExpr` to our richer `CostExpr`.
fn ast_cost_to_cost_expr(ce: &ast::CostExpr) -> CostExpr {
    match ce {
        ast::CostExpr::Literal(n) => CostExpr::Const(*n),
        ast::CostExpr::Var(v) => CostExpr::Var(v.clone()),
        ast::CostExpr::Linear(v) => CostExpr::Linear(v.clone()),
    }
}

fn ast_cost_clause_to_cost_vector(clause: &ast::CostClause) -> CostVector {
    CostVector {
        compute: ast_cost_to_cost_expr(&clause.compute),
        alloc: ast_cost_to_cost_expr(&clause.alloc),
        io: ast_cost_to_cost_expr(&clause.io),
        parallel: ast_cost_to_cost_expr(&clause.parallel),
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

/// Walk the AST, invoking `on_call(callee, args)` at every `Expr::Call` node,
/// then recursing into all sub-expressions.
fn walk_expr<F>(expr: &Expr, on_call: &mut F)
where
    F: FnMut(&Expr, &[Expr]),
{
    match expr {
        Expr::Call(callee, args) => {
            on_call(callee, args);
            walk_expr(callee, on_call);
            for arg in args {
                walk_expr(arg, on_call);
            }
        }
        Expr::BinOp(lhs, _, rhs) | Expr::Pipe(lhs, rhs) => {
            walk_expr(lhs, on_call);
            walk_expr(rhs, on_call);
        }
        Expr::UnaryOp(_, inner)
        | Expr::FieldAccess(inner, _)
        | Expr::Try(inner)
        | Expr::Spawn(inner)
        | Expr::Await(inner)
        | Expr::Throw(inner) => {
            walk_expr(inner, on_call);
        }
        Expr::ChannelNew { buffer, .. } => {
            walk_expr(buffer, on_call);
        }
        Expr::If(cond, then_br, else_br) => {
            walk_expr(cond, on_call);
            walk_expr(then_br, on_call);
            if let Some(e) = else_br {
                walk_expr(e, on_call);
            }
        }
        Expr::Block(stmts, tail) => {
            for stmt in stmts {
                match stmt {
                    Stmt::Let(_, _, e) | Stmt::Expr(e) => {
                        walk_expr(e, on_call);
                    }
                }
            }
            if let Some(tail_expr) = tail {
                walk_expr(tail_expr, on_call);
            }
        }
        Expr::Match(scrutinee, arms) => {
            walk_expr(scrutinee, on_call);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    walk_expr(guard, on_call);
                }
                walk_expr(&arm.body, on_call);
            }
        }
        Expr::Lambda(_, body) => {
            walk_expr(body, on_call);
        }
        Expr::ParallelScope { lanes, body } => {
            if let Some(lanes_expr) = lanes {
                walk_expr(lanes_expr, on_call);
            }
            walk_expr(body, on_call);
        }
        Expr::Select(arms) => {
            for arm in arms {
                match arm {
                    SelectArm::Recv { source, body, .. } => {
                        walk_expr(source, on_call);
                        walk_expr(body, on_call);
                    }
                    SelectArm::Timeout { duration, body } => {
                        walk_expr(duration, on_call);
                        walk_expr(body, on_call);
                    }
                }
            }
        }
        Expr::Return(inner) => {
            if let Some(e) = inner {
                walk_expr(e, on_call);
            }
        }
        Expr::List(elems) => {
            for e in elems {
                walk_expr(e, on_call);
            }
        }
        Expr::StructLit(_, fields) => {
            for (_, e) in fields {
                walk_expr(e, on_call);
            }
        }
        Expr::FString(parts) => {
            for part in parts {
                if let ast::FStringPart::Expr(e) = part {
                    walk_expr(e, on_call);
                }
            }
        }
        Expr::TString(parts) => {
            for part in parts {
                if let ast::TStringPart::Expr(e) = part {
                    walk_expr(e, on_call);
                }
            }
        }
        Expr::Perform { args, .. } => {
            for arg in args {
                walk_expr(arg, on_call);
            }
        }
        Expr::Handle { body, handlers } => {
            walk_expr(body, on_call);
            for binding in handlers {
                match binding {
                    HandleBinding::Use(handler_use) => {
                        for (_, expr) in &handler_use.payload {
                            walk_expr(expr, on_call);
                        }
                    }
                    HandleBinding::On(arm) => walk_expr(&arm.body, on_call),
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
        | Expr::Hole(_, _, _)
        | Expr::Placeholder => {}
    }
}

/// Walk the AST collecting argument lists from recursive calls to `fn_name`.
fn collect_recursive_calls(fn_name: &str, expr: &Expr, out: &mut Vec<Vec<CallArg>>) {
    walk_expr(expr, &mut |callee, args| {
        if let Expr::Var(name) = callee
            && name == fn_name
        {
            let classified: Vec<CallArg> = args.iter().map(classify_arg).collect();
            out.push(classified);
        }
    });
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

/// Walk the AST collecting names of non-recursive function calls.
///
/// Returns one entry per call site (duplicates allowed).
fn collect_callee_names(self_name: &str, expr: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    collect_callee_names_inner(self_name, expr, &mut out);
    out
}

fn collect_callee_names_inner(self_name: &str, expr: &Expr, out: &mut Vec<String>) {
    walk_expr(expr, &mut |callee, _args| {
        if let Expr::Var(name) = callee
            && name != self_name
        {
            out.push(name.clone());
        }
    });
}

/// Check if an actual cost exceeds a declared budget.
fn exceeds_budget(actual: &CostExpr, budget: &CostExpr) -> bool {
    match (actual, budget) {
        // Constant actual vs constant budget — straightforward comparison
        (CostExpr::Const(actual_n), CostExpr::Const(budget_n)) => actual_n > budget_n,
        // Unbounded actual always exceeds a finite budget
        (CostExpr::Unbounded, CostExpr::Const(_)) => true,
        // Structurally equal expressions never exceed each other
        (a, b) if a == b => false,
        // Symbolic actual against a constant budget — can't prove it fits
        (_, CostExpr::Const(_)) => true,
        // If budget is also symbolic, we can't decide — don't flag
        _ => false,
    }
}

fn exceeds_budget_vector(actual: &CostVector, budget: &CostVector) -> bool {
    exceeds_budget(&actual.compute, &budget.compute)
        || exceeds_budget(&actual.alloc, &budget.alloc)
        || exceeds_budget(&actual.io, &budget.io)
        || exceeds_budget(&actual.parallel, &budget.parallel)
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

    /// Component-wise maximum of two cost vectors.
    pub fn max(&self, other: &CostVector) -> CostVector {
        CostVector {
            compute: max_cost(&self.compute, &other.compute),
            alloc: max_cost(&self.alloc, &other.alloc),
            io: max_cost(&self.io, &other.io),
            parallel: max_cost(&self.parallel, &other.parallel),
        }
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
            "cost [{}, {}, {}, {}]",
            self.compute, self.alloc, self.io, self.parallel
        )
    }
}

/// Helper: add two cost expressions.
fn add_cost(a: &CostExpr, b: &CostExpr) -> CostExpr {
    match (a, b) {
        // Identity: x + 0 = x
        (x, CostExpr::Const(0)) | (CostExpr::Const(0), x) => x.clone(),
        // Constant folding: c1 + c2 = c1+c2
        (CostExpr::Const(x), CostExpr::Const(y)) => CostExpr::Const(x.saturating_add(*y)),
        // Unbounded propagation
        (CostExpr::Unbounded, _) | (_, CostExpr::Unbounded) => CostExpr::Unbounded,
        // Same-variable linear merge: O(n) + O(n) = 2 * O(n)
        (CostExpr::Linear(a_var), CostExpr::Linear(b_var)) if a_var == b_var => CostExpr::Mul(
            Box::new(CostExpr::Const(2)),
            Box::new(CostExpr::Linear(a_var.clone())),
        ),
        // Preserve symbolic addition
        _ => CostExpr::Add(Box::new(a.clone()), Box::new(b.clone())),
    }
}

/// Helper: max of two cost expressions.
fn max_cost(a: &CostExpr, b: &CostExpr) -> CostExpr {
    match (a, b) {
        (CostExpr::Const(x), CostExpr::Const(y)) => CostExpr::Const(*x.max(y)),
        (CostExpr::Unbounded, _) | (_, CostExpr::Unbounded) => CostExpr::Unbounded,
        (CostExpr::Const(0), other) | (other, CostExpr::Const(0)) => other.clone(),
        // Preserve symbolic max
        _ => CostExpr::Max(Box::new(a.clone()), Box::new(b.clone())),
    }
}

/// Helper: multiply two cost expressions.
fn mul_cost(a: &CostExpr, b: &CostExpr) -> CostExpr {
    match (a, b) {
        // Zero: x * 0 = 0
        (_, CostExpr::Const(0)) | (CostExpr::Const(0), _) => CostExpr::Const(0),
        // Identity: x * 1 = x
        (x, CostExpr::Const(1)) | (CostExpr::Const(1), x) => x.clone(),
        // Constant folding
        (CostExpr::Const(x), CostExpr::Const(y)) => CostExpr::Const(x.saturating_mul(*y)),
        // Unbounded propagation
        (CostExpr::Unbounded, _) | (_, CostExpr::Unbounded) => CostExpr::Unbounded,
        // Preserve symbolic multiplication
        _ => CostExpr::Mul(Box::new(a.clone()), Box::new(b.clone())),
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
            let cv = if let Some(vec) = analyzer.cost_vectors().get(name) {
                vec.clone()
            } else {
                Self::cost_vector_from_result(result)
            };
            self.costs.insert(name.clone(), cv);
        }
    }

    /// Fallback: build a [`CostVector`] from a [`CostResult`] when no AST-derived
    /// vector is available.
    fn cost_vector_from_result(result: &CostResult) -> CostVector {
        let compute = cost_result_to_expr(result);

        // Without an AST walk we cannot know alloc/io/parallel, so default
        // them conservatively to zero (pure, no allocation, no parallelism).
        let alloc = CostExpr::Const(0);
        let io = CostExpr::Const(0);
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
        let ast_ce = ast::CostExpr::Linear("n".into());
        let ce = ast_cost_to_cost_expr(&ast_ce);
        assert_eq!(ce, CostExpr::Linear("n".into()));
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
        assert_eq!(s, "cost [5, 3, 1, 0]");
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
        assert_eq!(s, "cost [∞, 0, 1, 0]");
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
        // Manually insert a constant result to test the fallback path.
        analyzer
            .results_mut()
            .insert("foo".into(), CostResult::Constant(1));

        let mut checker = CostChecker::new();
        checker.check_all(&analyzer);

        let cv = checker.costs.get("foo").expect("foo should be analyzed");
        assert_eq!(cv.compute, CostExpr::Const(1));
        // Fallback: no AST → io/alloc/parallel default to 0.
        assert_eq!(cv.io, CostExpr::Const(0));
        assert_eq!(cv.alloc, CostExpr::Const(0));
        assert_eq!(cv.parallel, CostExpr::Const(0));
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
        // Fallback: no AST walk → io defaults to 0.
        assert_eq!(cv.io, CostExpr::Const(0));
        assert!(cv.is_bounded());
    }

    // -----------------------------------------------------------------------
    // 4D per-expression cost tests (SEP-0004)
    // -----------------------------------------------------------------------

    /// Helper: create a fresh analyzer to use `analyze_expr_cost`.
    fn expr_cost(expr: &Expr) -> CostVector {
        let analyzer = CostAnalyzer::new();
        analyzer.analyze_expr_cost(expr)
    }

    #[test]
    fn pure_arithmetic_4d() {
        // `a + b * c`  (all int) → compute = 1+1 = 2, alloc = 0, io = 0, parallel = 0
        let expr = Expr::BinOp(
            Box::new(Expr::Var("a".into())),
            BinOp::Add,
            Box::new(Expr::BinOp(
                Box::new(Expr::Var("b".into())),
                BinOp::Mul,
                Box::new(Expr::Var("c".into())),
            )),
        );
        let cv = expr_cost(&expr);
        assert_eq!(cv.compute, CostExpr::Const(2));
        assert_eq!(cv.alloc, CostExpr::Const(0));
        assert_eq!(cv.io, CostExpr::Const(0));
        assert_eq!(cv.parallel, CostExpr::Const(0));
    }

    #[test]
    fn float_div_cost() {
        // `1.0 / 2.0` → compute = 3 (float div)
        let expr = Expr::BinOp(
            Box::new(Expr::FloatLit(1.0)),
            BinOp::Div,
            Box::new(Expr::FloatLit(2.0)),
        );
        let cv = expr_cost(&expr);
        assert_eq!(cv.compute, CostExpr::Const(3));
        assert_eq!(cv.alloc, CostExpr::Const(0));
    }

    #[test]
    fn int_div_cost() {
        // `10 / 3` → compute = 2 (int div)
        let expr = Expr::BinOp(
            Box::new(Expr::IntLit(10)),
            BinOp::Div,
            Box::new(Expr::IntLit(3)),
        );
        let cv = expr_cost(&expr);
        assert_eq!(cv.compute, CostExpr::Const(2));
    }

    #[test]
    fn struct_creation_alloc() {
        // `Point { x: 1, y: 2, z: 3 }` → alloc = 3 (field count)
        let expr = Expr::StructLit(
            "Point".into(),
            vec![
                ("x".into(), Expr::IntLit(1)),
                ("y".into(), Expr::IntLit(2)),
                ("z".into(), Expr::IntLit(3)),
            ],
        );
        let cv = expr_cost(&expr);
        assert_eq!(cv.alloc, CostExpr::Const(3));
        assert_eq!(cv.compute, CostExpr::Const(0));
        assert_eq!(cv.io, CostExpr::Const(0));
    }

    #[test]
    fn list_creation_alloc() {
        // `[1, 2, 3]` → alloc = 3 + 1 = 4
        let expr = Expr::List(vec![Expr::IntLit(1), Expr::IntLit(2), Expr::IntLit(3)]);
        let cv = expr_cost(&expr);
        assert_eq!(cv.alloc, CostExpr::Const(4));
    }

    #[test]
    fn string_literal_alloc() {
        // 16-char string → alloc = ceil(16/8) = 2
        let expr = Expr::StrLit("hello, world!!!!".into());
        let cv = expr_cost(&expr);
        assert_eq!(cv.alloc, CostExpr::Const(2));
        assert_eq!(cv.compute, CostExpr::Const(0));
    }

    #[test]
    fn println_call_io() {
        // `println(x)` → io = 1, compute = 3 (call overhead)
        let expr = Expr::Call(
            Box::new(Expr::Var("println".into())),
            vec![Expr::Var("x".into())],
        );
        let cv = expr_cost(&expr);
        assert_eq!(cv.io, CostExpr::Const(1));
        assert_eq!(cv.compute, CostExpr::Const(3));
        assert_eq!(cv.parallel, CostExpr::Const(0));
    }

    #[test]
    fn pure_call_no_io() {
        // `foo(1)` → io = 0 (not in IO_FUNCTIONS)
        let expr = Expr::Call(Box::new(Expr::Var("foo".into())), vec![Expr::IntLit(1)]);
        let cv = expr_cost(&expr);
        assert_eq!(cv.io, CostExpr::Const(0));
        assert_eq!(cv.compute, CostExpr::Const(3)); // call overhead only
    }

    #[test]
    fn spawn_parallel() {
        // `spawn(expr)` → parallel = 1
        let expr = Expr::Spawn(Box::new(Expr::IntLit(42)));
        let cv = expr_cost(&expr);
        assert_eq!(cv.parallel, CostExpr::Const(1));
    }

    #[test]
    fn sequential_block_4d() {
        // Block: { let x = 1 + 2; println(x) }
        // Stmt1: compute=1, alloc=0, io=0
        // Stmt2: compute=3, alloc=0, io=1
        // Total: compute=4, alloc=0, io=1, parallel=0
        let block = Expr::Block(
            vec![
                Stmt::Let(
                    "x".into(),
                    None,
                    Expr::BinOp(
                        Box::new(Expr::IntLit(1)),
                        BinOp::Add,
                        Box::new(Expr::IntLit(2)),
                    ),
                ),
                Stmt::Expr(Expr::Call(
                    Box::new(Expr::Var("println".into())),
                    vec![Expr::Var("x".into())],
                )),
            ],
            None,
        );
        let cv = expr_cost(&block);
        assert_eq!(cv.compute, CostExpr::Const(4));
        assert_eq!(cv.alloc, CostExpr::Const(0));
        assert_eq!(cv.io, CostExpr::Const(1));
        assert_eq!(cv.parallel, CostExpr::Const(0));
    }

    #[test]
    fn if_branch_max() {
        // if true { 1 + 2 } else { 3 * 4 }
        // cond: 0, then: compute=1, else: compute=1 → max=1
        // total compute = 0 + 1 = 1
        let expr = Expr::If(
            Box::new(Expr::BoolLit(true)),
            Box::new(Expr::BinOp(
                Box::new(Expr::IntLit(1)),
                BinOp::Add,
                Box::new(Expr::IntLit(2)),
            )),
            Some(Box::new(Expr::BinOp(
                Box::new(Expr::IntLit(3)),
                BinOp::Mul,
                Box::new(Expr::IntLit(4)),
            ))),
        );
        let cv = expr_cost(&expr);
        assert_eq!(cv.compute, CostExpr::Const(1));
        assert_eq!(cv.alloc, CostExpr::Const(0));
    }

    #[test]
    fn io_function_detection() {
        assert!(is_io_function("print"));
        assert!(is_io_function("println"));
        assert!(is_io_function("read_line"));
        assert!(is_io_function("open"));
        assert!(!is_io_function("foo"));
        assert!(!is_io_function("map"));
    }

    #[test]
    fn binop_compute_costs() {
        assert_eq!(binop_compute_cost(&BinOp::Add, false), 1);
        assert_eq!(binop_compute_cost(&BinOp::Add, true), 2);
        assert_eq!(binop_compute_cost(&BinOp::Div, false), 2);
        assert_eq!(binop_compute_cost(&BinOp::Div, true), 3);
        assert_eq!(binop_compute_cost(&BinOp::Eq, false), 1);
    }

    #[test]
    fn var_read_zero_cost() {
        let cv = expr_cost(&Expr::Var("x".into()));
        assert_eq!(cv, CostVector::zero());
    }

    // -----------------------------------------------------------------------
    // Symbolic cost algebra preservation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_linear_linear_same_var() {
        let result = add_cost(&CostExpr::Linear("n".into()), &CostExpr::Linear("n".into()));
        assert_eq!(
            result,
            CostExpr::Mul(
                Box::new(CostExpr::Const(2)),
                Box::new(CostExpr::Linear("n".into()))
            )
        );
    }

    #[test]
    fn test_add_linear_const() {
        let result = add_cost(&CostExpr::Linear("n".into()), &CostExpr::Const(5));
        assert_eq!(
            result,
            CostExpr::Add(
                Box::new(CostExpr::Linear("n".into())),
                Box::new(CostExpr::Const(5))
            )
        );
    }

    #[test]
    fn test_mul_const_linear() {
        let result = mul_cost(&CostExpr::Const(3), &CostExpr::Linear("n".into()));
        assert_eq!(
            result,
            CostExpr::Mul(
                Box::new(CostExpr::Const(3)),
                Box::new(CostExpr::Linear("n".into()))
            )
        );
    }

    #[test]
    fn test_add_identity() {
        let lin = CostExpr::Linear("x".into());
        assert_eq!(add_cost(&lin, &CostExpr::Const(0)), lin);
        assert_eq!(add_cost(&CostExpr::Const(0), &lin), lin);
    }

    #[test]
    fn test_mul_identity() {
        let lin = CostExpr::Linear("x".into());
        assert_eq!(mul_cost(&lin, &CostExpr::Const(1)), lin);
        assert_eq!(mul_cost(&CostExpr::Const(1), &lin), lin);
    }

    #[test]
    fn test_unbounded_propagation() {
        let lin = CostExpr::Linear("x".into());
        assert_eq!(add_cost(&CostExpr::Unbounded, &lin), CostExpr::Unbounded);
        assert_eq!(add_cost(&lin, &CostExpr::Unbounded), CostExpr::Unbounded);
        assert_eq!(mul_cost(&CostExpr::Unbounded, &lin), CostExpr::Unbounded);
        assert_eq!(
            max_cost(&CostExpr::Unbounded, &CostExpr::Const(5)),
            CostExpr::Unbounded
        );
    }

    #[test]
    fn test_add_linear_linear_diff_var() {
        let result = add_cost(&CostExpr::Linear("n".into()), &CostExpr::Linear("m".into()));
        assert_eq!(
            result,
            CostExpr::Add(
                Box::new(CostExpr::Linear("n".into())),
                Box::new(CostExpr::Linear("m".into()))
            )
        );
    }

    #[test]
    fn test_max_preserves_symbolic() {
        let result = max_cost(&CostExpr::Linear("n".into()), &CostExpr::Const(5));
        assert_eq!(
            result,
            CostExpr::Max(
                Box::new(CostExpr::Linear("n".into())),
                Box::new(CostExpr::Const(5))
            )
        );
    }

    #[test]
    fn test_exceeds_budget_structural_equality() {
        let expr = CostExpr::Linear("n".into());
        assert!(!exceeds_budget(&expr, &expr));
    }
}
