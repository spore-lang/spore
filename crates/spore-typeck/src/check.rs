//! Core type-checking logic.
//!
//! Walks the AST and verifies type consistency, building up a type
//! environment as it goes. Reports all errors (does not bail on first).

use spore_parser::ast::*;

use crate::env::{Env, TypeRegistry};
use crate::error::TypeError;
use crate::hole::{HoleInfo, HoleReport};
use std::collections::HashMap;

use crate::types::{CapSet, Ty};

pub struct Checker {
    pub errors: Vec<TypeError>,
    pub registry: TypeRegistry,
    pub hole_report: HoleReport,
    env: Env,
    /// Capabilities of the function currently being checked.
    current_caps: CapSet,
    /// Name of the function currently being checked.
    current_function: String,
    /// Declared return type of the current function (for hole inference).
    expected_return_type: Option<Ty>,
    /// Next type variable ID for fresh type variables.
    next_var_id: u32,
    /// Substitution map: type variable ID → resolved type.
    substitution: HashMap<u32, Ty>,
}

impl Checker {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            registry: TypeRegistry::default(),
            hole_report: HoleReport::new(),
            env: Env::new(),
            current_caps: CapSet::new(),
            current_function: String::new(),
            expected_return_type: None,
            next_var_id: 0,
            substitution: HashMap::new(),
        }
    }

    /// Type-check an entire module.
    pub fn check_module(&mut self, module: &Module) {
        // First pass: register all top-level declarations
        for item in &module.items {
            self.register_item(item);
        }
        // Second pass: check function bodies
        for item in &module.items {
            self.check_item(item);
        }
    }

    // ── Registration (first pass) ───────────────────────────────────

    fn register_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => {
                let param_tys: Vec<Ty> = f.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(Ty::Unit);
                let caps: CapSet = f
                    .uses_clause
                    .as_ref()
                    .map(|uc| uc.resources.iter().cloned().collect())
                    .unwrap_or_default();
                self.registry
                    .functions
                    .insert(f.name.clone(), (param_tys, ret_ty, caps));
                if let Some(wc) = &f.where_clause {
                    let mut type_params: Vec<String> =
                        wc.constraints.iter().map(|c| c.type_var.clone()).collect();
                    type_params.sort();
                    type_params.dedup();
                    self.registry
                        .fn_type_params
                        .insert(f.name.clone(), type_params);
                }
            }
            Item::StructDef(s) => {
                let fields: Vec<(String, Ty)> = s
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), self.resolve_type(&f.ty)))
                    .collect();
                self.registry.structs.insert(s.name.clone(), fields);
            }
            Item::TypeDef(t) => {
                let variants: Vec<(String, Vec<Ty>)> = t
                    .variants
                    .iter()
                    .map(|v| {
                        let ftys: Vec<Ty> = v.fields.iter().map(|f| self.resolve_type(f)).collect();
                        (v.name.clone(), ftys)
                    })
                    .collect();
                self.registry.types.insert(t.name.clone(), variants);
            }
            Item::CapabilityDef(_) | Item::Import(_) => {}
        }
    }

    // ── Checking (second pass) ──────────────────────────────────────

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => self.check_fn(f),
            _ => {} // structs/types already registered; capabilities/imports deferred
        }
    }

    fn check_fn(&mut self, f: &FnDef) {
        let Some(body) = &f.body else { return };

        // Set current function's capability set
        let prev_caps = std::mem::replace(
            &mut self.current_caps,
            f.uses_clause
                .as_ref()
                .map(|uc| uc.resources.iter().cloned().collect())
                .unwrap_or_default(),
        );

        // Track current function name and return type for hole reporting
        let prev_function = std::mem::replace(&mut self.current_function, f.name.clone());
        let declared_ret = f
            .return_type
            .as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or(Ty::Unit);
        let prev_expected = std::mem::replace(&mut self.expected_return_type, Some(declared_ret.clone()));

        self.env.push_scope();

        // Bind parameters
        for param in &f.params {
            let ty = self.resolve_type(&param.ty);
            self.env.define(param.name.clone(), ty);
        }

        let body_ty = self.check_expr(body);
        let body_ty = self.apply_subst(&body_ty);
        let declared_ret = self.apply_subst(&declared_ret);

        self.unify(&declared_ret, &body_ty, &format!("function `{}`", f.name));

        self.env.pop_scope();
        self.current_caps = prev_caps;
        self.current_function = prev_function;
        self.expected_return_type = prev_expected;
    }

    // ── Expression type checking ────────────────────────────────────

    fn check_expr(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::IntLit(_) => Ty::Int,
            Expr::FloatLit(_) => Ty::Float,
            Expr::StrLit(_) => Ty::Str,
            Expr::BoolLit(_) => Ty::Bool,
            Expr::FString(_) => Ty::Str,

            Expr::Var(name) => {
                if let Some(ty) = self.env.lookup(name) {
                    ty.clone()
                } else if let Some((_, _ret, _)) = self.registry.functions.get(name) {
                    // bare function name as value — return its function type
                    let (params, ret, caps) = self.registry.functions[name].clone();
                    Ty::Fn(params, Box::new(ret), caps)
                } else {
                    self.err(format!("undefined variable `{name}`"));
                    Ty::Error
                }
            }

            Expr::BinOp(lhs, op, rhs) => self.check_binop(lhs, op, rhs),

            Expr::UnaryOp(op, expr) => {
                let ty = self.check_expr(expr);
                match op {
                    UnaryOp::Neg => {
                        if !ty.is_numeric() && !ty.is_error() {
                            self.err(format!("cannot negate type `{ty}`"));
                        }
                        ty
                    }
                    UnaryOp::Not => {
                        if ty != Ty::Bool && !ty.is_error() {
                            self.err(format!("cannot apply `!` to type `{ty}`"));
                        }
                        Ty::Bool
                    }
                    UnaryOp::BitNot => {
                        if ty != Ty::Int && !ty.is_error() {
                            self.err(format!("cannot apply `~` to type `{ty}`"));
                        }
                        Ty::Int
                    }
                }
            }

            Expr::Call(callee, args) => self.check_call(callee, args),

            Expr::Lambda(params, body) => {
                self.env.push_scope();
                let param_tys: Vec<Ty> = params
                    .iter()
                    .map(|p| {
                        let ty = self.resolve_type(&p.ty);
                        self.env.define(p.name.clone(), ty.clone());
                        ty
                    })
                    .collect();
                let ret_ty = self.check_expr(body);
                self.env.pop_scope();
                Ty::Fn(param_tys, Box::new(ret_ty), CapSet::new())
            }

            Expr::If(cond, then_branch, else_branch) => {
                let cond_ty = self.check_expr(cond);
                if cond_ty != Ty::Bool && !cond_ty.is_error() {
                    self.err(format!("if condition must be Bool, got `{cond_ty}`"));
                }
                let then_ty = self.check_expr(then_branch);
                if let Some(else_expr) = else_branch {
                    let else_ty = self.check_expr(else_expr);
                    self.unify(&then_ty, &else_ty, "if/else branches");
                    then_ty
                } else {
                    then_ty
                }
            }

            Expr::Match(scrutinee, arms) => {
                let _scrut_ty = self.check_expr(scrutinee);
                let mut result_ty: Option<Ty> = None;
                for arm in arms {
                    // TODO: check pattern against scrutinee type
                    let arm_ty = self.check_expr(&arm.body);
                    if let Some(ref expected) = result_ty {
                        self.unify(expected, &arm_ty, "match arms");
                    } else {
                        result_ty = Some(arm_ty);
                    }
                }
                result_ty.unwrap_or(Ty::Unit)
            }

            Expr::Block(stmts, tail) => {
                self.env.push_scope();
                for stmt in stmts {
                    self.check_stmt(stmt);
                }
                let ty = if let Some(tail_expr) = tail {
                    self.check_expr(tail_expr)
                } else {
                    Ty::Unit
                };
                self.env.pop_scope();
                ty
            }

            Expr::Pipe(lhs, rhs) => {
                let arg_ty = self.check_expr(lhs);
                let fn_ty = self.check_expr(rhs);
                match fn_ty {
                    Ty::Fn(params, ret, caps) => {
                        if params.len() != 1 {
                            self.err(format!(
                                "pipe target expects 1 argument, function takes {}",
                                params.len()
                            ));
                        } else {
                            self.unify(&params[0], &arg_ty, "pipe argument");
                        }
                        self.check_cap_propagation(&caps);
                        *ret
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.err(format!("pipe target must be a function, got `{fn_ty}`"));
                        Ty::Error
                    }
                }
            }

            Expr::FieldAccess(expr, field) => {
                let ty = self.check_expr(expr);
                match &ty {
                    Ty::Named(name) | Ty::App(name, _) => {
                        if let Some(fields) = self.registry.structs.get(name) {
                            if let Some((_, fty)) = fields.iter().find(|(n, _)| n == field) {
                                fty.clone()
                            } else {
                                self.err(format!("struct `{name}` has no field `{field}`"));
                                Ty::Error
                            }
                        } else {
                            self.err(format!("type `{name}` has no fields"));
                            Ty::Error
                        }
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.err(format!("cannot access field `{field}` on type `{ty}`"));
                        Ty::Error
                    }
                }
            }

            Expr::StructLit(name, fields) => {
                if let Some(def_fields) = self.registry.structs.get(name).cloned() {
                    for (fname, fexpr) in fields {
                        let fty = self.check_expr(fexpr);
                        if let Some((_, expected)) = def_fields.iter().find(|(n, _)| n == fname) {
                            self.unify(expected, &fty, &format!("struct field `{fname}`"));
                        } else {
                            self.err(format!("struct `{name}` has no field `{fname}`"));
                        }
                    }
                    Ty::Named(name.clone())
                } else {
                    self.err(format!("undefined struct `{name}`"));
                    Ty::Error
                }
            }

            Expr::Try(expr) => {
                // For now, just return the inner type
                // TODO: proper Result[T, E] unwrapping
                self.check_expr(expr)
            }

            Expr::Hole(name, ty_hint) => {
                let ty = if let Some(te) = ty_hint {
                    self.resolve_type(te)
                } else if let Some(ref ret) = self.expected_return_type {
                    ret.clone()
                } else {
                    Ty::Hole(name.clone())
                };

                // Collect hole info for the report
                let bindings = self.env.all_bindings();
                let expected = ty.clone();
                let suggestions = self.find_suggestions(&expected);
                self.hole_report.holes.push(HoleInfo {
                    name: name.clone(),
                    expected_type: expected,
                    function: self.current_function.clone(),
                    bindings,
                    suggestions,
                });

                ty
            }

            Expr::Spawn(expr) => {
                let _inner = self.check_expr(expr);
                // TODO: return Task[T] type
                Ty::Named("Task".into())
            }

            Expr::Await(expr) => {
                let ty = self.check_expr(expr);
                // TODO: unwrap Task[T] → T
                match ty {
                    Ty::Named(ref n) if n == "Task" => Ty::Unit, // simplified
                    _ => ty,
                }
            }
        }
    }

    // ── Binary operators ────────────────────────────────────────────

    fn check_binop(&mut self, lhs: &Expr, op: &BinOp, rhs: &Expr) -> Ty {
        let lt = self.check_expr(lhs);
        let rt = self.check_expr(rhs);

        if lt.is_error() || rt.is_error() {
            return Ty::Error;
        }

        match op {
            // Arithmetic: both operands must be same numeric type
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                // String concatenation with +
                if matches!(op, BinOp::Add) && lt == Ty::Str && rt == Ty::Str {
                    return Ty::Str;
                }
                if !lt.is_numeric() {
                    self.err(format!("cannot apply `{op:?}` to type `{lt}`"));
                    return Ty::Error;
                }
                self.unify(&lt, &rt, "arithmetic operands");
                lt
            }
            // Comparison: both operands same type, returns Bool
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                self.unify(&lt, &rt, "comparison operands");
                Ty::Bool
            }
            // Logical: both Bool
            BinOp::And | BinOp::Or => {
                if lt != Ty::Bool {
                    self.err(format!("logical `{op:?}` expects Bool, got `{lt}`"));
                }
                if rt != Ty::Bool {
                    self.err(format!("logical `{op:?}` expects Bool, got `{rt}`"));
                }
                Ty::Bool
            }
            // Bitwise: both Int
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                if lt != Ty::Int {
                    self.err(format!("bitwise `{op:?}` expects Int, got `{lt}`"));
                }
                if rt != Ty::Int {
                    self.err(format!("bitwise `{op:?}` expects Int, got `{rt}`"));
                }
                Ty::Int
            }
        }
    }

    // ── Function calls ──────────────────────────────────────────────

    fn check_call(&mut self, callee: &Expr, args: &[Expr]) -> Ty {
        // Direct call by name: `foo(args)`
        if let Expr::Var(name) = callee {
            if let Some((param_tys, ret_ty, callee_caps)) = self.registry.functions.get(name).cloned() {
                // Instantiate generic functions with fresh type variables
                let (param_tys, ret_ty) = match self.registry.fn_type_params.get(name).cloned() {
                    Some(ref tp) if !tp.is_empty() => self.instantiate_sig(tp, &param_tys, &ret_ty),
                    _ => (param_tys, ret_ty),
                };

                if param_tys.len() != args.len() {
                    self.err(format!(
                        "function `{name}` expects {} arguments, got {}",
                        param_tys.len(),
                        args.len()
                    ));
                    return self.apply_subst(&ret_ty);
                }
                for (i, (expected, arg_expr)) in param_tys.iter().zip(args).enumerate() {
                    let arg_ty = self.check_expr(arg_expr);
                    self.unify(
                        expected,
                        &arg_ty,
                        &format!("argument {} of `{name}`", i + 1),
                    );
                }
                self.check_cap_propagation(&callee_caps);
                return self.apply_subst(&ret_ty);
            }
            // Could be a variable holding a function
        }

        // Method call: `obj.method(args)` — callee is FieldAccess
        // General case: check callee type
        let fn_ty = self.check_expr(callee);
        match fn_ty {
            Ty::Fn(param_tys, ret_ty, caps) => {
                if param_tys.len() != args.len() {
                    self.err(format!(
                        "function expects {} arguments, got {}",
                        param_tys.len(),
                        args.len()
                    ));
                } else {
                    for (i, (expected, arg_expr)) in param_tys.iter().zip(args).enumerate() {
                        let arg_ty = self.check_expr(arg_expr);
                        self.unify(expected, &arg_ty, &format!("argument {}", i + 1));
                    }
                }
                self.check_cap_propagation(&caps);
                *ret_ty
            }
            Ty::Error => Ty::Error,
            _ => {
                self.err(format!("cannot call non-function type `{fn_ty}`"));
                Ty::Error
            }
        }
    }

    // ── Statements ──────────────────────────────────────────────────

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(name, ty_ann, init) => {
                let init_ty = self.check_expr(init);
                let ty = if let Some(te) = ty_ann {
                    let declared = self.resolve_type(te);
                    self.unify(&declared, &init_ty, &format!("let binding `{name}`"));
                    declared
                } else {
                    init_ty
                };
                self.env.define(name.clone(), ty);
            }
            Stmt::Expr(expr) => {
                self.check_expr(expr);
            }
        }
    }

    // ── Type resolution ─────────────────────────────────────────────

    fn resolve_type(&self, te: &TypeExpr) -> Ty {
        match te {
            TypeExpr::Named(name) => match name.as_str() {
                "Int" => Ty::Int,
                "Float" => Ty::Float,
                "Bool" => Ty::Bool,
                "String" => Ty::Str,
                "()" => Ty::Unit,
                _ => Ty::Named(name.clone()),
            },
            TypeExpr::Generic(name, args) => {
                let resolved: Vec<Ty> = args.iter().map(|a| self.resolve_type(a)).collect();
                Ty::App(name.clone(), resolved)
            }
            TypeExpr::Tuple(types) => {
                Ty::Tuple(types.iter().map(|t| self.resolve_type(t)).collect())
            }
            TypeExpr::Function(params, ret) => {
                let ptys: Vec<Ty> = params.iter().map(|p| self.resolve_type(p)).collect();
                Ty::Fn(ptys, Box::new(self.resolve_type(ret)), CapSet::new())
            }
            TypeExpr::Refinement(base, _, _) => {
                // For PoC, ignore refinement predicates — just use base type
                self.resolve_type(base)
            }
        }
    }

    // ── Type variable infrastructure ────────────────────────────────

    /// Create a fresh type variable.
    fn fresh_var(&mut self) -> Ty {
        let id = self.next_var_id;
        self.next_var_id += 1;
        Ty::Var(id)
    }

    /// Apply the current substitution to a type, resolving type variables.
    fn apply_subst(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Var(id) => {
                if let Some(t) = self.substitution.get(id) {
                    self.apply_subst(t)
                } else {
                    ty.clone()
                }
            }
            Ty::Fn(params, ret, caps) => Ty::Fn(
                params.iter().map(|p| self.apply_subst(p)).collect(),
                Box::new(self.apply_subst(ret)),
                caps.clone(),
            ),
            Ty::App(name, args) => Ty::App(
                name.clone(),
                args.iter().map(|a| self.apply_subst(a)).collect(),
            ),
            Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| self.apply_subst(t)).collect()),
            _ => ty.clone(),
        }
    }

    /// Substitute type parameter names with fresh type variables in a type.
    fn instantiate_ty(&self, ty: &Ty, mapping: &HashMap<String, Ty>) -> Ty {
        match ty {
            Ty::Named(name) => {
                if let Some(replacement) = mapping.get(name) {
                    replacement.clone()
                } else {
                    ty.clone()
                }
            }
            Ty::Fn(params, ret, caps) => Ty::Fn(
                params.iter().map(|p| self.instantiate_ty(p, mapping)).collect(),
                Box::new(self.instantiate_ty(ret, mapping)),
                caps.clone(),
            ),
            Ty::App(name, args) => Ty::App(
                name.clone(),
                args.iter().map(|a| self.instantiate_ty(a, mapping)).collect(),
            ),
            Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| self.instantiate_ty(t, mapping)).collect()),
            _ => ty.clone(),
        }
    }

    /// Create fresh type variables for each type parameter and substitute
    /// them into the function signature.
    fn instantiate_sig(&mut self, type_params: &[String], param_tys: &[Ty], ret_ty: &Ty) -> (Vec<Ty>, Ty) {
        let mapping: HashMap<String, Ty> = type_params
            .iter()
            .map(|name| (name.clone(), self.fresh_var()))
            .collect();
        let new_params: Vec<Ty> = param_tys.iter().map(|t| self.instantiate_ty(t, &mapping)).collect();
        let new_ret = self.instantiate_ty(ret_ty, &mapping);
        (new_params, new_ret)
    }

    // ── Capability propagation check ────────────────────────────────

    /// Verify that the current function's capability set is a superset of the callee's.
    fn check_cap_propagation(&mut self, callee_caps: &CapSet) {
        let missing: Vec<&str> = callee_caps
            .iter()
            .filter(|c| !self.current_caps.contains(*c))
            .map(|s| s.as_str())
            .collect();
        if !missing.is_empty() {
            self.err(format!(
                "missing capabilities [{}]: caller does not declare them",
                missing.join(", ")
            ));
        }
    }

    // ── Unification with type variable support ─────────────────────

    fn unify(&mut self, expected: &Ty, actual: &Ty, context: &str) {
        let e = self.apply_subst(expected);
        let a = self.apply_subst(actual);
        if e.is_error() || a.is_error() {
            return;
        }
        if e == a {
            return;
        }
        if matches!(e, Ty::Hole(_)) || matches!(a, Ty::Hole(_)) {
            return;
        }

        // Type variable binding
        if let Ty::Var(id) = e {
            self.substitution.insert(id, a);
            return;
        }
        if let Ty::Var(id) = a {
            self.substitution.insert(id, e);
            return;
        }

        // Structural unification
        match (&e, &a) {
            (Ty::Fn(p1, r1, _), Ty::Fn(p2, r2, _)) if p1.len() == p2.len() => {
                let pairs: Vec<(Ty, Ty)> = p1.iter().cloned().zip(p2.iter().cloned()).collect();
                let ret_pair = ((**r1).clone(), (**r2).clone());
                for (x, y) in &pairs {
                    self.unify(x, y, context);
                }
                self.unify(&ret_pair.0, &ret_pair.1, context);
            }
            (Ty::App(n1, a1), Ty::App(n2, a2)) if n1 == n2 && a1.len() == a2.len() => {
                let pairs: Vec<(Ty, Ty)> = a1.iter().cloned().zip(a2.iter().cloned()).collect();
                for (x, y) in &pairs {
                    self.unify(x, y, context);
                }
            }
            (Ty::Tuple(t1), Ty::Tuple(t2)) if t1.len() == t2.len() => {
                let pairs: Vec<(Ty, Ty)> = t1.iter().cloned().zip(t2.iter().cloned()).collect();
                for (x, y) in &pairs {
                    self.unify(x, y, context);
                }
            }
            _ => {
                self.err(format!(
                    "type mismatch in {context}: expected `{e}`, got `{a}`"
                ));
            }
        }
    }

    fn err(&mut self, message: String) {
        self.errors.push(TypeError::new(message));
    }

    /// Find registered functions whose return type matches the expected type.
    fn find_suggestions(&self, expected: &Ty) -> Vec<String> {
        if expected.is_error() || matches!(expected, Ty::Hole(_)) {
            return Vec::new();
        }
        let mut suggestions: Vec<String> = self
            .registry
            .functions
            .iter()
            .filter(|(name, (_, ret_ty, _))| {
                ret_ty == expected && *name != &self.current_function
            })
            .map(|(name, _)| name.clone())
            .collect();
        suggestions.sort();
        suggestions
    }
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}
