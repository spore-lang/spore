//! Core type-checking logic.
//!
//! Walks the AST and verifies type consistency, building up a type
//! environment as it goes. Reports all errors (does not bail on first).

use spore_parser::ast::*;

use crate::env::{Env, TypeRegistry};
use crate::error::{ErrorCode, TypeError};
use crate::hole::{HoleDependencyGraph, HoleInfo, HoleReport};
use crate::module::{ImportedSymbol, ModuleError, ModuleRegistry};
use std::collections::{HashMap, HashSet};

use crate::capability::{CapabilityHierarchy, default_hierarchy};
use crate::types::{CapSet, ErrorSet, Ty};

pub struct Checker {
    pub errors: Vec<TypeError>,
    pub registry: TypeRegistry,
    pub hole_report: HoleReport,
    pub module_registry: ModuleRegistry,
    env: Env,
    /// Capabilities of the function currently being checked.
    current_caps: CapSet,
    /// Error set of the function currently being checked.
    current_errors: ErrorSet,
    /// Name of the function currently being checked.
    current_function: String,
    /// Declared return type of the current function (for hole inference).
    expected_return_type: Option<Ty>,
    /// Next type variable ID for fresh type variables.
    next_var_id: u32,
    /// Substitution map: type variable ID → resolved type.
    substitution: HashMap<u32, Ty>,
    /// Capability hierarchy for expanding parent caps (e.g. IO → 6 leaves).
    hierarchy: CapabilityHierarchy,
}

impl Checker {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            registry: TypeRegistry::default(),
            hole_report: HoleReport::new(),
            module_registry: ModuleRegistry::new(),
            env: Env::new(),
            current_caps: CapSet::new(),
            current_errors: ErrorSet::new(),
            current_function: String::new(),
            expected_return_type: None,
            next_var_id: 0,
            substitution: HashMap::new(),
            hierarchy: default_hierarchy(),
        }
    }

    /// Create a new Checker with an existing module registry.
    pub fn with_module_registry(module_registry: ModuleRegistry) -> Self {
        Self {
            errors: Vec::new(),
            registry: TypeRegistry::default(),
            hole_report: HoleReport::new(),
            module_registry,
            env: Env::new(),
            current_caps: CapSet::new(),
            current_errors: ErrorSet::new(),
            current_function: String::new(),
            expected_return_type: None,
            next_var_id: 0,
            substitution: HashMap::new(),
            hierarchy: default_hierarchy(),
        }
    }

    /// Type-check an entire module.
    pub fn check_module(&mut self, module: &Module) {
        // First pass: register all top-level declarations
        for item in &module.items {
            self.register_item(item);
        }
        // Process imports after registration (so local symbols exist)
        for item in &module.items {
            if let Item::Import(import) = item {
                self.resolve_import(import);
            }
        }
        // Second pass: check function bodies
        for item in &module.items {
            self.check_item(item);
        }
        // Build the hole dependency graph based on shared type variables
        self.hole_report.dependency_graph = self.build_hole_dependency_graph();
    }

    // ── Registration (first pass) ───────────────────────────────────

    /// Resolve an import declaration, importing symbols into the current registry.
    fn resolve_import(&mut self, import: &ImportDecl) {
        match import {
            ImportDecl::Import { path, alias } => {
                let path_segments: Vec<String> = path.split('.').map(|s| s.to_string()).collect();
                // The alias is the module alias; import all exported symbols
                let module = match self.module_registry.get(&path_segments) {
                    Some(m) => m.clone(),
                    None => {
                        self.err(ErrorCode::M0001, format!("module `{path}` not found"));
                        return;
                    }
                };
                let all_names = module.all_exports();
                match self
                    .module_registry
                    .resolve_import(&path_segments, &all_names)
                {
                    Ok(resolved) => {
                        self.import_resolved_symbols(&module, &resolved, alias);
                    }
                    Err(ModuleError::PrivateSymbol { symbol, module: m }) => {
                        self.err(
                            ErrorCode::M0003,
                            format!(
                                "symbol `{symbol}` in module `{m}` is private and not accessible"
                            ),
                        );
                    }
                    Err(ModuleError::SymbolNotFound { symbol, module: m }) => {
                        self.err(
                            ErrorCode::M0002,
                            format!("symbol `{symbol}` not found in module `{m}`"),
                        );
                    }
                    Err(ModuleError::ModuleNotFound(m)) => {
                        self.err(ErrorCode::M0001, format!("module `{m}` not found"));
                    }
                }
                let _ = alias; // alias available for qualified access
            }
            ImportDecl::Alias { name, path } => {
                let path_segments: Vec<String> = path.split('.').map(|s| s.to_string()).collect();
                let module = match self.module_registry.get(&path_segments) {
                    Some(m) => m.clone(),
                    None => {
                        self.err(ErrorCode::M0001, format!("module `{path}` not found"));
                        return;
                    }
                };
                let all_names = module.all_exports();
                match self
                    .module_registry
                    .resolve_import(&path_segments, &all_names)
                {
                    Ok(resolved) => {
                        self.import_resolved_symbols(&module, &resolved, name);
                    }
                    Err(ModuleError::PrivateSymbol { symbol, module: m }) => {
                        self.err(
                            ErrorCode::M0003,
                            format!(
                                "symbol `{symbol}` in module `{m}` is private and not accessible"
                            ),
                        );
                    }
                    Err(ModuleError::SymbolNotFound { symbol, module: m }) => {
                        self.err(
                            ErrorCode::M0002,
                            format!("symbol `{symbol}` not found in module `{m}`"),
                        );
                    }
                    Err(ModuleError::ModuleNotFound(m)) => {
                        self.err(ErrorCode::M0001, format!("module `{m}` not found"));
                    }
                }
            }
        }
    }

    /// Import resolved symbols from a module into the current type registry.
    fn import_resolved_symbols(
        &mut self,
        module: &crate::module::ModuleInterface,
        resolved: &[(String, ImportedSymbol)],
        _alias: &str,
    ) {
        for (name, kind) in resolved {
            match kind {
                ImportedSymbol::Function => {
                    if let Some((params, ret)) = module.functions.get(name) {
                        // Import with empty capability set (cross-module calls
                        // inherit the callee's declared caps on invocation)
                        self.registry
                            .functions
                            .insert(name.clone(), (params.clone(), ret.clone(), CapSet::new()));
                    }
                }
                ImportedSymbol::Type => {
                    if let Some(variants) = module.types.get(name) {
                        let variant_tys: Vec<(String, Vec<Ty>)> =
                            variants.iter().map(|v| (v.clone(), Vec::new())).collect();
                        self.registry.types.insert(name.clone(), variant_tys);
                    }
                }
                ImportedSymbol::Struct => {
                    if let Some(fields) = module.structs.get(name) {
                        let field_tys: Vec<(String, Ty)> = fields
                            .iter()
                            .map(|f| (f.clone(), Ty::Named("Unknown".into())))
                            .collect();
                        self.registry.structs.insert(name.clone(), field_tys);
                    }
                }
                ImportedSymbol::Capability => {
                    if module.capabilities.contains(name) {
                        self.registry
                            .capabilities
                            .insert(name.clone(), (Vec::new(), Vec::new()));
                    }
                }
            }
        }
    }

    fn register_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => {
                let param_tys: Vec<Ty> =
                    f.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(Ty::Unit);
                let caps: CapSet = f
                    .uses_clause
                    .as_ref()
                    .map(|uc| {
                        let raw = crate::capability::CapabilitySet::from_names(
                            uc.resources.iter().cloned(),
                        );
                        self.hierarchy.expand(&raw).to_btreeset()
                    })
                    .unwrap_or_default();
                self.registry
                    .functions
                    .insert(f.name.clone(), (param_tys, ret_ty, caps));
                // Register error set (! [E1, E2])
                if !f.errors.is_empty() {
                    let error_set: ErrorSet = f
                        .errors
                        .iter()
                        .filter_map(|te| {
                            if let TypeExpr::Named(n) = te {
                                Some(n.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    self.registry.fn_errors.insert(f.name.clone(), error_set);
                }
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
                self.registry.types.insert(t.name.clone(), variants.clone());

                // Register each variant as a constructor in the value namespace.
                let ret_ty = if t.type_params.is_empty() {
                    Ty::Named(t.name.clone())
                } else {
                    Ty::App(
                        t.name.clone(),
                        t.type_params.iter().map(|p| Ty::Named(p.clone())).collect(),
                    )
                };

                for (vname, field_tys) in &variants {
                    if field_tys.is_empty() {
                        // Zero-field variant: register as a value of the enum type.
                        self.env.define(vname.clone(), ret_ty.clone());
                    } else {
                        // Variant with fields: register as a constructor function.
                        self.registry.functions.insert(
                            vname.clone(),
                            (field_tys.clone(), ret_ty.clone(), CapSet::new()),
                        );
                        if !t.type_params.is_empty() {
                            self.registry
                                .fn_type_params
                                .insert(vname.clone(), t.type_params.clone());
                        }
                    }
                }
            }
            Item::CapabilityDef(cap) => {
                let methods: Vec<(String, Vec<Ty>, Ty)> = cap
                    .methods
                    .iter()
                    .map(|m| {
                        let param_tys: Vec<Ty> =
                            m.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                        let ret_ty = m
                            .return_type
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .unwrap_or(Ty::Unit);
                        (m.name.clone(), param_tys, ret_ty)
                    })
                    .collect();
                self.registry
                    .capabilities
                    .insert(cap.name.clone(), (cap.type_params.clone(), methods));
            }
            Item::ImplDef(impl_def) => {
                if !self
                    .registry
                    .capabilities
                    .contains_key(&impl_def.capability)
                {
                    self.err(
                        ErrorCode::C0002,
                        format!("unknown capability `{}`", impl_def.capability),
                    );
                    return;
                }
                let methods: Vec<(String, Vec<Ty>, Ty)> = impl_def
                    .methods
                    .iter()
                    .map(|m| {
                        let param_tys: Vec<Ty> =
                            m.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                        let ret_ty = m
                            .return_type
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .unwrap_or(Ty::Unit);
                        (m.name.clone(), param_tys, ret_ty)
                    })
                    .collect();
                self.registry.impls.insert(
                    (impl_def.capability.clone(), impl_def.target_type.clone()),
                    methods,
                );
            }
            Item::Import(_) | Item::Const(_) | Item::CapabilityAlias { .. } => {}
            Item::Alias(alias_def) => {
                let resolved = self.resolve_type(&alias_def.target);
                self.registry
                    .type_aliases
                    .insert(alias_def.name.clone(), resolved);
            }
        }
    }

    // ── Checking (second pass) ──────────────────────────────────────

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => self.check_fn(f),
            Item::ImplDef(impl_def) => self.check_impl(impl_def),
            _ => {} // structs/types already registered; capabilities/imports deferred
        }
    }

    fn check_impl(&mut self, impl_def: &ImplDef) {
        let Some((_cap_type_params, cap_methods)) = self
            .registry
            .capabilities
            .get(&impl_def.capability)
            .cloned()
        else {
            return; // Already errored in registration
        };

        // Check that all required methods are implemented
        for (method_name, _expected_params, _expected_ret) in &cap_methods {
            if !impl_def.methods.iter().any(|m| &m.name == method_name) {
                self.err(
                    ErrorCode::E0013,
                    format!(
                        "impl `{}` for `{}` is missing method `{}`",
                        impl_def.capability, impl_def.target_type, method_name
                    ),
                );
            }
        }

        // Check that no extra methods are defined
        for method in &impl_def.methods {
            if !cap_methods.iter().any(|(name, _, _)| name == &method.name) {
                self.err(
                    ErrorCode::E0014,
                    format!(
                        "method `{}` is not defined in capability `{}`",
                        method.name, impl_def.capability
                    ),
                );
            }
        }

        // Validate that each implemented method's signature matches the capability.
        // Substitute capability type params (e.g., T in Display[T]) with the target type.
        let cap_type_params = _cap_type_params;
        let type_mapping: HashMap<String, Ty> = if cap_type_params.is_empty() {
            HashMap::new()
        } else if !impl_def.type_args.is_empty() {
            cap_type_params
                .iter()
                .zip(impl_def.type_args.iter())
                .map(|(param, arg)| (param.clone(), self.resolve_type(arg)))
                .collect()
        } else if cap_type_params.len() == 1 {
            // Default: map the single type param to the target type
            let mut m = HashMap::new();
            m.insert(
                cap_type_params[0].clone(),
                self.resolve_type(&TypeExpr::Named(impl_def.target_type.clone())),
            );
            m
        } else {
            HashMap::new()
        };

        for method in &impl_def.methods {
            if let Some((_expected_name, expected_params, expected_ret)) =
                cap_methods.iter().find(|(name, _, _)| name == &method.name)
            {
                // Apply type param substitution to expected types
                let expected_params: Vec<Ty> = expected_params
                    .iter()
                    .map(|t| self.instantiate_ty(t, &type_mapping))
                    .collect();
                let expected_ret = self.instantiate_ty(expected_ret, &type_mapping);

                let impl_params: Vec<Ty> = method
                    .params
                    .iter()
                    .map(|p| self.resolve_type(&p.ty))
                    .collect();
                let impl_ret = method
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(Ty::Unit);

                if impl_params.len() != expected_params.len() {
                    self.err(
                        ErrorCode::E0001,
                        format!(
                            "method `{}` in impl `{}` for `{}` expects {} parameters, got {}",
                            method.name,
                            impl_def.capability,
                            impl_def.target_type,
                            expected_params.len(),
                            impl_params.len()
                        ),
                    );
                } else {
                    for (i, (exp, act)) in
                        expected_params.iter().zip(impl_params.iter()).enumerate()
                    {
                        self.unify(
                            exp,
                            act,
                            &format!(
                                "parameter {} of method `{}` in impl `{}` for `{}`",
                                i + 1,
                                method.name,
                                impl_def.capability,
                                impl_def.target_type
                            ),
                        );
                    }
                }
                self.unify(
                    &expected_ret,
                    &impl_ret,
                    &format!(
                        "return type of method `{}` in impl `{}` for `{}`",
                        method.name, impl_def.capability, impl_def.target_type
                    ),
                );
            }
        }

        // Type-check each method body
        for method in &impl_def.methods {
            self.check_fn(method);
        }
    }

    fn check_fn(&mut self, f: &FnDef) {
        let Some(body) = &f.body else { return };

        // Set current function's capability set (with hierarchy expansion)
        let prev_caps = std::mem::replace(
            &mut self.current_caps,
            f.uses_clause
                .as_ref()
                .map(|uc| {
                    let raw =
                        crate::capability::CapabilitySet::from_names(uc.resources.iter().cloned());
                    self.hierarchy.expand(&raw).to_btreeset()
                })
                .unwrap_or_default(),
        );

        // Set current function's error set (! [E1, E2])
        let prev_errors = std::mem::replace(
            &mut self.current_errors,
            f.errors
                .iter()
                .filter_map(|te| {
                    if let TypeExpr::Named(n) = te {
                        Some(n.clone())
                    } else {
                        None
                    }
                })
                .collect(),
        );

        // Track current function name and return type for hole reporting
        let prev_function = std::mem::replace(&mut self.current_function, f.name.clone());
        let declared_ret = f
            .return_type
            .as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or(Ty::Unit);
        let prev_expected = self.expected_return_type.take();
        self.expected_return_type = Some(declared_ret.clone());

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
        self.current_errors = prev_errors;
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
            Expr::TString(_) => Ty::Str,

            Expr::Var(name) => {
                if let Some(ty) = self.env.lookup(name) {
                    ty.clone()
                } else if let Some((_, _ret, _)) = self.registry.functions.get(name) {
                    // bare function name as value — return its function type
                    let (params, ret, caps) = self.registry.functions[name].clone();
                    Ty::Fn(params, Box::new(ret), caps)
                } else if let Some((params, ret)) = self.lookup_module_function(name) {
                    Ty::Fn(params, Box::new(ret), CapSet::new())
                } else {
                    self.err(ErrorCode::E0004, format!("undefined variable `{name}`"));
                    Ty::Error
                }
            }

            Expr::BinOp(lhs, op, rhs) => self.check_binop(lhs, op, rhs),

            Expr::UnaryOp(op, expr) => {
                let ty = self.check_expr(expr);
                match op {
                    UnaryOp::Neg => {
                        if !ty.is_numeric() && !ty.is_error() {
                            self.err(ErrorCode::E0002, format!("cannot negate type `{ty}`"));
                        }
                        ty
                    }
                    UnaryOp::Not => {
                        if ty != Ty::Bool && !ty.is_error() {
                            self.err(ErrorCode::E0002, format!("cannot apply `!` to type `{ty}`"));
                        }
                        Ty::Bool
                    }
                    UnaryOp::BitNot => {
                        if ty != Ty::Int && !ty.is_error() {
                            self.err(ErrorCode::E0002, format!("cannot apply `~` to type `{ty}`"));
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
                    self.err(
                        ErrorCode::E0001,
                        format!("if condition must be Bool, got `{cond_ty}`"),
                    );
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
                let scrut_ty = self.check_expr(scrutinee);
                let scrut_ty = self.apply_subst(&scrut_ty);

                // Check exhaustiveness
                self.check_exhaustiveness(&scrut_ty, arms);

                let mut result_ty: Option<Ty> = None;
                for arm in arms {
                    // Check pattern against scrutinee type and get bindings
                    let bindings = self.check_pattern(&arm.pattern, &scrut_ty);

                    // Create a new scope with pattern bindings
                    self.env.push_scope();
                    for (name, ty) in bindings {
                        self.env.define(name, ty);
                    }

                    // Check guard if present
                    if let Some(guard) = &arm.guard {
                        let guard_ty = self.check_expr(guard);
                        if guard_ty != Ty::Bool && !guard_ty.is_error() {
                            self.err(
                                ErrorCode::E0017,
                                format!("match guard must be Bool, got `{guard_ty}`"),
                            );
                        }
                    }

                    let arm_ty = self.check_expr(&arm.body);
                    self.env.pop_scope();

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
                            self.err(
                                ErrorCode::E0009,
                                format!(
                                    "pipe target expects 1 argument, function takes {}",
                                    params.len()
                                ),
                            );
                        } else {
                            self.unify(&params[0], &arg_ty, "pipe argument");
                        }
                        self.check_cap_propagation(&caps);
                        *ret
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.err(
                            ErrorCode::E0009,
                            format!("pipe target must be a function, got `{fn_ty}`"),
                        );
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
                                self.err(
                                    ErrorCode::E0015,
                                    format!("struct `{name}` has no field `{field}`"),
                                );
                                Ty::Error
                            }
                        } else {
                            self.err(ErrorCode::E0016, format!("type `{name}` has no fields"));
                            Ty::Error
                        }
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.err(
                            ErrorCode::E0016,
                            format!("cannot access field `{field}` on type `{ty}`"),
                        );
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
                            self.err(
                                ErrorCode::E0015,
                                format!("struct `{name}` has no field `{fname}`"),
                            );
                        }
                    }
                    Ty::Named(name.clone())
                } else {
                    self.err(ErrorCode::E0005, format!("undefined struct `{name}`"));
                    Ty::Error
                }
            }

            Expr::Try(expr) => {
                // Extract the callee name for error-set lookup when inner is a call
                let callee_name = match expr.as_ref() {
                    Expr::Call(callee, _) => {
                        if let Expr::Var(name) = callee.as_ref() {
                            Some(name.clone())
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                let inner_ty = self.check_expr(expr);

                // Check error propagation: callee's error set ⊆ caller's error set
                if let Some(name) = callee_name
                    && let Some(callee_errors) = self.registry.fn_errors.get(&name).cloned()
                {
                    self.check_error_propagation(&callee_errors);
                }

                inner_ty
            }

            Expr::Hole(name, ty_hint, _allows) => {
                let ty = if let Some(te) = ty_hint {
                    self.resolve_type(te)
                } else if let Some(ref ret) = self.expected_return_type {
                    ret.clone()
                } else {
                    Ty::Hole(name.clone())
                };

                // Collect hole info for the report (v0.3)
                let bindings = self.env.all_bindings();
                let expected = ty.clone();
                let suggestions = self.find_suggestions(&expected);

                // Build scored candidates from simple suggestions
                let candidates: Vec<crate::hole::CandidateScore> = suggestions
                    .into_iter()
                    .map(|s| crate::hole::CandidateScore {
                        name: s,
                        type_match: 1.0,
                        cost_fit: 0.5,
                        capability_fit: 1.0,
                        error_coverage: 0.5,
                    })
                    .collect();

                // Collect capabilities and errors in scope
                let capabilities = self.current_caps.clone();
                let errors_to_handle: Vec<String> = self.current_errors.iter().cloned().collect();

                self.hole_report.holes.push(HoleInfo {
                    name: name.clone(),
                    location: None,
                    expected_type: expected,
                    type_inferred_from: None,
                    function: self.current_function.clone(),
                    enclosing_signature: None,
                    bindings,
                    binding_dependencies: std::collections::BTreeMap::new(),
                    capabilities,
                    errors_to_handle,
                    cost_budget: None,
                    candidates,
                    dependent_holes: Vec::new(),
                    confidence: None,
                    error_clusters: Vec::new(),
                });

                ty
            }

            Expr::Spawn(expr) => {
                let inner = self.check_expr(expr);
                Ty::App("Task".into(), vec![inner])
            }

            Expr::Await(expr) => {
                let ty = self.check_expr(expr);
                let ty = self.apply_subst(&ty);
                match ty {
                    Ty::App(ref name, ref args) if name == "Task" && args.len() == 1 => {
                        args[0].clone()
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.err(
                            ErrorCode::E0001,
                            format!("await expects Task[T], got `{ty}`"),
                        );
                        Ty::Error
                    }
                }
            }

            Expr::Return(expr) => {
                if let Some(inner) = expr {
                    self.check_expr(inner)
                } else {
                    Ty::Unit
                }
            }

            Expr::Throw(expr) => {
                self.check_expr(expr);
                Ty::Never
            }

            Expr::List(elems) => {
                if elems.is_empty() {
                    Ty::App("List".into(), vec![self.fresh_var()])
                } else {
                    let first_ty = self.check_expr(&elems[0]);
                    for elem in &elems[1..] {
                        let elem_ty = self.check_expr(elem);
                        self.unify(&first_ty, &elem_ty, "list elements");
                    }
                    Ty::App("List".into(), vec![first_ty])
                }
            }

            Expr::CharLit(_) => Ty::Char,

            Expr::ParallelScope { lanes, body } => {
                if let Some(lanes_expr) = lanes {
                    let lanes_ty = self.check_expr(lanes_expr);
                    if lanes_ty != Ty::Int && !lanes_ty.is_error() {
                        self.err(
                            ErrorCode::E0002,
                            format!("parallel_scope lanes must be Int, got `{lanes_ty}`"),
                        );
                    }
                }
                self.check_expr(body)
            }

            Expr::Select(arms) => {
                let mut result_ty: Option<Ty> = None;
                for arm in arms {
                    let _source_ty = self.check_expr(&arm.source);
                    self.env.push_scope();
                    // Bind the received value — type is unknown for now
                    let binding_ty = self.fresh_var();
                    self.env.define(arm.binding.clone(), binding_ty);
                    let arm_ty = self.check_expr(&arm.body);
                    self.env.pop_scope();
                    if let Some(ref expected) = result_ty {
                        self.unify(expected, &arm_ty, "select arms");
                    } else {
                        result_ty = Some(arm_ty);
                    }
                }
                result_ty.unwrap_or(Ty::Unit)
            }

            Expr::Placeholder => {
                unreachable!(
                    "`_` placeholder should have been desugared into a lambda by the parser"
                )
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
                    self.err(
                        ErrorCode::E0002,
                        format!("cannot apply `{op:?}` to type `{lt}`"),
                    );
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
                    self.err(
                        ErrorCode::E0002,
                        format!("logical `{op:?}` expects Bool, got `{lt}`"),
                    );
                }
                if rt != Ty::Bool {
                    self.err(
                        ErrorCode::E0002,
                        format!("logical `{op:?}` expects Bool, got `{rt}`"),
                    );
                }
                Ty::Bool
            }
            // Bitwise: both Int
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                if lt != Ty::Int {
                    self.err(
                        ErrorCode::E0002,
                        format!("bitwise `{op:?}` expects Int, got `{lt}`"),
                    );
                }
                if rt != Ty::Int {
                    self.err(
                        ErrorCode::E0002,
                        format!("bitwise `{op:?}` expects Int, got `{rt}`"),
                    );
                }
                Ty::Int
            }
        }
    }

    // ── Function calls ──────────────────────────────────────────────

    fn check_call(&mut self, callee: &Expr, args: &[Expr]) -> Ty {
        // Direct call by name: `foo(args)`
        if let Expr::Var(name) = callee
            && let Some((param_tys, ret_ty, callee_caps)) =
                self.registry.functions.get(name).cloned()
        {
            // Instantiate generic functions with fresh type variables
            let (param_tys, ret_ty) = match self.registry.fn_type_params.get(name).cloned() {
                Some(ref tp) if !tp.is_empty() => self.instantiate_sig(tp, &param_tys, &ret_ty),
                _ => (param_tys, ret_ty),
            };

            if param_tys.len() != args.len() {
                self.err(
                    ErrorCode::E0007,
                    format!(
                        "function `{name}` expects {} arguments, got {}",
                        param_tys.len(),
                        args.len()
                    ),
                );
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

        // Direct call by name: check module registry (prelude builtins)
        if let Expr::Var(name) = callee
            && let Some((param_tys, ret_ty)) = self.lookup_module_function(name)
        {
            if param_tys.len() != args.len() {
                self.err(
                    ErrorCode::E0007,
                    format!(
                        "function `{name}` expects {} arguments, got {}",
                        param_tys.len(),
                        args.len()
                    ),
                );
                return ret_ty;
            }
            for (i, (expected, arg_expr)) in param_tys.iter().zip(args).enumerate() {
                let arg_ty = self.check_expr(arg_expr);
                self.unify(
                    expected,
                    &arg_ty,
                    &format!("argument {} of `{name}`", i + 1),
                );
            }
            return ret_ty;
        }
        // Could be a variable holding a function

        // Method call: `obj.method(args)` — callee is FieldAccess
        // General case: check callee type
        let fn_ty = self.check_expr(callee);
        match fn_ty {
            Ty::Fn(param_tys, ret_ty, caps) => {
                if param_tys.len() != args.len() {
                    self.err(
                        ErrorCode::E0007,
                        format!(
                            "function expects {} arguments, got {}",
                            param_tys.len(),
                            args.len()
                        ),
                    );
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
                self.err(
                    ErrorCode::E0008,
                    format!("cannot call non-function type `{fn_ty}`"),
                );
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
                    // Check refinement predicate on constant initializers
                    if let Ty::Refined(_, ref var_name, ref pred) = declared {
                        self.check_refinement_on_expr(init, var_name, pred, name);
                    }
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

    // ── Module registry lookup ──────────────────────────────────────

    /// Look up a function in the module registry (e.g. prelude builtins).
    fn lookup_module_function(&self, name: &str) -> Option<(Vec<Ty>, Ty)> {
        for module in self.module_registry.all_interfaces() {
            if let Some((params, ret)) = module.functions.get(name) {
                return Some((params.clone(), ret.clone()));
            }
        }
        None
    }

    // ── Type resolution ─────────────────────────────────────────────

    pub fn resolve_type(&self, te: &TypeExpr) -> Ty {
        match te {
            TypeExpr::Named(name) => match name.as_str() {
                "Int" => Ty::Int,
                "Float" => Ty::Float,
                "Bool" => Ty::Bool,
                "String" => Ty::Str,
                "Char" => Ty::Char,
                "()" => Ty::Unit,
                "Never" => Ty::Never,
                _ => {
                    // Check type aliases (supports refined aliases like `alias Port = Int when ...`)
                    if let Some(ty) = self.registry.type_aliases.get(name) {
                        ty.clone()
                    } else {
                        Ty::Named(name.clone())
                    }
                }
            },
            TypeExpr::Generic(name, args) => {
                let resolved: Vec<Ty> = args.iter().map(|a| self.resolve_type(a)).collect();
                Ty::App(name.clone(), resolved)
            }
            TypeExpr::Tuple(types) => {
                Ty::Tuple(types.iter().map(|t| self.resolve_type(t)).collect())
            }
            TypeExpr::Function(params, ret, _errors) => {
                let ptys: Vec<Ty> = params.iter().map(|p| self.resolve_type(p)).collect();
                Ty::Fn(ptys, Box::new(self.resolve_type(ret)), CapSet::new())
            }
            TypeExpr::Refinement(base, var_name, pred_expr) => {
                let base_ty = self.resolve_type(base);
                Ty::Refined(Box::new(base_ty), var_name.clone(), pred_expr.clone())
            }
            TypeExpr::Record(fields) => {
                let resolved = fields
                    .iter()
                    .map(|(name, te)| (name.clone(), self.resolve_type(te)))
                    .collect();
                Ty::Record(resolved)
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
            Ty::Record(fields) => Ty::Record(
                fields
                    .iter()
                    .map(|(n, t)| (n.clone(), self.apply_subst(t)))
                    .collect(),
            ),
            Ty::Refined(base, var, pred) => {
                Ty::Refined(Box::new(self.apply_subst(base)), var.clone(), pred.clone())
            }
            _ => ty.clone(),
        }
    }

    /// Check if type variable `id` occurs anywhere in `ty`.
    fn occurs_in(&self, id: u32, ty: &Ty) -> bool {
        let ty = self.apply_subst(ty);
        match &ty {
            Ty::Var(vid) => *vid == id,
            Ty::Fn(params, ret, _) => {
                params.iter().any(|p| self.occurs_in(id, p)) || self.occurs_in(id, ret)
            }
            Ty::App(_, args) => args.iter().any(|a| self.occurs_in(id, a)),
            Ty::Tuple(ts) => ts.iter().any(|t| self.occurs_in(id, t)),
            Ty::Record(fields) => fields.iter().any(|(_, t)| self.occurs_in(id, t)),
            Ty::Refined(base, _, _) => self.occurs_in(id, base),
            _ => false,
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
                params
                    .iter()
                    .map(|p| self.instantiate_ty(p, mapping))
                    .collect(),
                Box::new(self.instantiate_ty(ret, mapping)),
                caps.clone(),
            ),
            Ty::App(name, args) => Ty::App(
                name.clone(),
                args.iter()
                    .map(|a| self.instantiate_ty(a, mapping))
                    .collect(),
            ),
            Ty::Tuple(ts) => {
                Ty::Tuple(ts.iter().map(|t| self.instantiate_ty(t, mapping)).collect())
            }
            Ty::Record(fields) => Ty::Record(
                fields
                    .iter()
                    .map(|(n, t)| (n.clone(), self.instantiate_ty(t, mapping)))
                    .collect(),
            ),
            Ty::Refined(base, var, pred) => Ty::Refined(
                Box::new(self.instantiate_ty(base, mapping)),
                var.clone(),
                pred.clone(),
            ),
            _ => ty.clone(),
        }
    }

    /// Create fresh type variables for each type parameter and substitute
    /// them into the function signature.
    fn instantiate_sig(
        &mut self,
        type_params: &[String],
        param_tys: &[Ty],
        ret_ty: &Ty,
    ) -> (Vec<Ty>, Ty) {
        let mapping: HashMap<String, Ty> = type_params
            .iter()
            .map(|name| (name.clone(), self.fresh_var()))
            .collect();
        let new_params: Vec<Ty> = param_tys
            .iter()
            .map(|t| self.instantiate_ty(t, &mapping))
            .collect();
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
            self.err(
                ErrorCode::C0001,
                format!(
                    "missing capabilities [{}]: caller does not declare them",
                    missing.join(", ")
                ),
            );
        }
    }

    // ── Error set propagation check ─────────────────────────────────

    /// Verify that the current function's error set is a superset of the callee's.
    fn check_error_propagation(&mut self, callee_errors: &ErrorSet) {
        let missing: Vec<&str> = callee_errors
            .iter()
            .filter(|e| !self.current_errors.contains(*e))
            .map(|s| s.as_str())
            .collect();
        if !missing.is_empty() {
            self.err(
                ErrorCode::E0012,
                format!(
                    "missing errors [{}] in `?`: caller does not declare them in its error set",
                    missing.join(", ")
                ),
            );
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

        // Never is subtype of all types
        if matches!(e, Ty::Never) || matches!(a, Ty::Never) {
            return;
        }

        // Type variable binding
        if let Ty::Var(id) = e {
            if self.occurs_in(id, &a) {
                self.err(
                    ErrorCode::E0003,
                    format!("infinite type: ?T{id} occurs in `{a}`"),
                );
                return;
            }
            self.substitution.insert(id, a);
            return;
        }
        if let Ty::Var(id) = a {
            if self.occurs_in(id, &e) {
                self.err(
                    ErrorCode::E0003,
                    format!("infinite type: ?T{id} occurs in `{e}`"),
                );
                return;
            }
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
            // Width subtyping: actual record may have extra fields
            (Ty::Record(expected_fields), Ty::Record(actual_fields)) => {
                for (ename, ety) in expected_fields {
                    if let Some((_, aty)) = actual_fields.iter().find(|(n, _)| n == ename) {
                        self.unify(ety, aty, context);
                    } else {
                        self.err(
                            ErrorCode::E0001,
                            format!("type mismatch in {context}: record missing field `{ename}`"),
                        );
                    }
                }
            }
            // Refined ↔ Refined: unify bases (predicate compatibility checked separately)
            (Ty::Refined(b1, _, _), Ty::Refined(b2, _, _)) => {
                let base1 = (**b1).clone();
                let base2 = (**b2).clone();
                self.unify(&base1, &base2, context);
            }
            // Refined is subtype of its base type (strip refinement)
            (Ty::Refined(base, _, _), other) => {
                let base = (**base).clone();
                self.unify(&base, other, context);
            }
            (other, Ty::Refined(base, _, _)) => {
                let base = (**base).clone();
                self.unify(other, &base, context);
            }
            _ => {
                self.err(
                    ErrorCode::E0001,
                    format!("type mismatch in {context}: expected `{e}`, got `{a}`"),
                );
            }
        }
    }

    // ── Pattern type checking ──────────────────────────────────────

    /// Check if `name` is a known zero-field enum variant.
    fn find_unit_variant(&self, name: &str) -> Option<String> {
        for (type_name, variants) in &self.registry.types {
            if variants
                .iter()
                .any(|(vname, fields)| vname == name && fields.is_empty())
            {
                return Some(type_name.clone());
            }
        }
        None
    }

    /// Type-check a pattern against a scrutinee type.
    /// Returns bindings introduced by the pattern (name -> type).
    fn check_pattern(&mut self, pattern: &Pattern, scrutinee_ty: &Ty) -> Vec<(String, Ty)> {
        match pattern {
            Pattern::Wildcard => vec![],
            Pattern::Var(name) => {
                // Zero-field enum variants (e.g. Red, None) are parsed as Var.
                if let Some(type_name) = self.find_unit_variant(name) {
                    let expected_ty = Ty::Named(type_name);
                    self.unify(&expected_ty, scrutinee_ty, &format!("pattern `{name}`"));
                    vec![]
                } else {
                    vec![(name.clone(), scrutinee_ty.clone())]
                }
            }
            Pattern::IntLit(_) => {
                if *scrutinee_ty != Ty::Int && !scrutinee_ty.is_error() {
                    self.err(
                        ErrorCode::E0011,
                        format!("integer pattern cannot match type `{scrutinee_ty}`"),
                    );
                }
                vec![]
            }
            Pattern::StrLit(_) => {
                if *scrutinee_ty != Ty::Str && !scrutinee_ty.is_error() {
                    self.err(
                        ErrorCode::E0011,
                        format!("string pattern cannot match type `{scrutinee_ty}`"),
                    );
                }
                vec![]
            }
            Pattern::BoolLit(_) => {
                if *scrutinee_ty != Ty::Bool && !scrutinee_ty.is_error() {
                    self.err(
                        ErrorCode::E0011,
                        format!("boolean pattern cannot match type `{scrutinee_ty}`"),
                    );
                }
                vec![]
            }
            Pattern::Constructor(name, sub_pats) => {
                #[allow(clippy::type_complexity)]
                let types_snapshot: Vec<(String, Vec<(String, Vec<Ty>)>)> = self
                    .registry
                    .types
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                for (type_name, variants) in &types_snapshot {
                    if let Some((_, field_tys)) = variants.iter().find(|(vname, _)| vname == name) {
                        let expected_ty = Ty::Named(type_name.clone());
                        self.unify(&expected_ty, scrutinee_ty, &format!("pattern `{name}`"));

                        if sub_pats.len() != field_tys.len() {
                            self.err(
                                ErrorCode::E0007,
                                format!(
                                    "variant `{name}` expects {} fields, got {}",
                                    field_tys.len(),
                                    sub_pats.len()
                                ),
                            );
                        }

                        let mut bindings = vec![];
                        for (sub_pat, field_ty) in sub_pats.iter().zip(field_tys.iter()) {
                            bindings.extend(self.check_pattern(sub_pat, field_ty));
                        }
                        return bindings;
                    }
                }
                if !scrutinee_ty.is_error() {
                    self.err(ErrorCode::E0006, format!("unknown variant `{name}`"));
                }
                vec![]
            }
            Pattern::Struct(name, field_pats) => {
                let def_fields = self.registry.structs.get(name).cloned();
                if let Some(def_fields) = def_fields {
                    let expected_ty = Ty::Named(name.clone());
                    self.unify(
                        &expected_ty,
                        scrutinee_ty,
                        &format!("struct pattern `{name}`"),
                    );

                    let mut bindings = vec![];
                    for (fname, fpat) in field_pats {
                        if let Some((_, fty)) = def_fields.iter().find(|(n, _)| n == fname) {
                            bindings.extend(self.check_pattern(fpat, fty));
                        } else {
                            self.err(
                                ErrorCode::E0015,
                                format!("struct `{name}` has no field `{fname}`"),
                            );
                        }
                    }
                    bindings
                } else {
                    if !scrutinee_ty.is_error() {
                        self.err(
                            ErrorCode::E0005,
                            format!("unknown struct `{name}` in pattern"),
                        );
                    }
                    vec![]
                }
            }
            Pattern::Or(pats) => {
                let mut all_bindings = vec![];
                for pat in pats {
                    all_bindings = self.check_pattern(pat, scrutinee_ty);
                }
                all_bindings
            }
            Pattern::List(elements, _rest) => {
                // For list patterns, the scrutinee should be a list type
                let elem_ty = self.fresh_var();
                let list_ty = Ty::App("List".into(), vec![elem_ty.clone()]);
                self.unify(&list_ty, scrutinee_ty, "list pattern");
                let mut bindings = vec![];
                for pat in elements {
                    bindings.extend(self.check_pattern(pat, &elem_ty));
                }
                if let Some(rest_name) = _rest {
                    bindings.push((rest_name.clone(), scrutinee_ty.clone()));
                }
                bindings
            }
        }
    }

    // ── Exhaustiveness checking ─────────────────────────────────────

    /// Check if match arms exhaustively cover the scrutinee type.
    fn check_exhaustiveness(&mut self, scrutinee_ty: &Ty, arms: &[MatchArm]) {
        if scrutinee_ty.is_error() {
            return;
        }

        let has_catch_all = arms.iter().any(|arm| {
            let is_catch_all = match &arm.pattern {
                Pattern::Wildcard => true,
                Pattern::Var(name) => self.find_unit_variant(name).is_none(),
                _ => false,
            };
            is_catch_all && arm.guard.is_none()
        });
        if has_catch_all {
            return;
        }

        match scrutinee_ty {
            Ty::Bool => {
                let has_true = arms
                    .iter()
                    .any(|arm| pattern_contains_bool(&arm.pattern, true));
                let has_false = arms
                    .iter()
                    .any(|arm| pattern_contains_bool(&arm.pattern, false));
                if !has_true || !has_false {
                    let mut missing = vec![];
                    if !has_true {
                        missing.push("true");
                    }
                    if !has_false {
                        missing.push("false");
                    }
                    self.err(
                        ErrorCode::E0010,
                        format!(
                            "non-exhaustive match: missing pattern(s) {}",
                            missing.join(", ")
                        ),
                    );
                }
            }
            Ty::Named(name) => {
                if let Some(variants) = self.registry.types.get(name).cloned() {
                    let variant_names: Vec<String> =
                        variants.iter().map(|(n, _)| n.clone()).collect();
                    let mut covered: Vec<bool> = vec![false; variant_names.len()];

                    for arm in arms {
                        mark_covered_variants(&arm.pattern, &variant_names, &mut covered);
                    }

                    let missing: Vec<&str> = variant_names
                        .iter()
                        .zip(covered.iter())
                        .filter(|(_, c)| !**c)
                        .map(|(n, _)| n.as_str())
                        .collect();

                    if !missing.is_empty() {
                        self.err(
                            ErrorCode::E0010,
                            format!(
                                "non-exhaustive match on `{name}`: missing variant(s) {}",
                                missing.join(", ")
                            ),
                        );
                    }
                }
            }
            Ty::Int | Ty::Float | Ty::Str => {
                self.err(
                    ErrorCode::E0010,
                    format!(
                        "non-exhaustive match: `{}` requires a wildcard `_` or variable pattern",
                        scrutinee_ty
                    ),
                );
            }
            _ => {}
        }
    }

    fn err(&mut self, code: ErrorCode, message: String) {
        self.errors.push(TypeError::new(code, message));
    }

    /// Check a refinement predicate against a constant expression.
    /// If the init expression is a constant, evaluate the predicate.
    /// If not constant, skip (runtime check needed).
    fn check_refinement_on_expr(
        &mut self,
        init: &Expr,
        var_name: &str,
        pred: &Expr,
        binding_name: &str,
    ) {
        use crate::refinement::{eval_refinement_predicate, expr_to_const};
        if let Some(cv) = expr_to_const(init) {
            match eval_refinement_predicate(pred, var_name, &cv) {
                Ok(true) => {} // predicate satisfied
                Ok(false) => {
                    self.err(
                        ErrorCode::R0001,
                        format!(
                            "refinement predicate violated for `{binding_name}`: \
                             value does not satisfy the type constraint"
                        ),
                    );
                }
                Err(_reason) => {
                    // Predicate not decidable at compile time — skip
                }
            }
        }
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
            .filter(|(name, (_, ret_ty, _))| ret_ty == expected && *name != &self.current_function)
            .map(|(name, _)| name.clone())
            .collect();
        suggestions.sort();
        suggestions
    }

    /// Build a dependency graph between holes based on shared type variables.
    fn build_hole_dependency_graph(&self) -> HoleDependencyGraph {
        let mut graph = HoleDependencyGraph::new();

        for hole in &self.hole_report.holes {
            graph.add_hole(hole.name.clone());
        }

        let hole_vars: Vec<(&str, HashSet<u32>)> = self
            .hole_report
            .holes
            .iter()
            .map(|h| {
                let vars = self.collect_type_vars(&h.expected_type);
                (h.name.as_str(), vars)
            })
            .collect();

        // Two holes that share a type variable are dependent
        for (i, (name1, vars1)) in hole_vars.iter().enumerate() {
            for (name2, vars2) in hole_vars.iter().skip(i + 1) {
                if vars1.iter().any(|v| vars2.contains(v)) {
                    graph.add_dependency(name2.to_string(), name1.to_string());
                }
            }
        }

        graph
    }

    /// Collect all type variable IDs in a type (following substitutions).
    fn collect_type_vars(&self, ty: &Ty) -> HashSet<u32> {
        let mut vars = HashSet::new();
        self.collect_type_vars_inner(ty, &mut vars);
        vars
    }

    fn collect_type_vars_inner(&self, ty: &Ty, vars: &mut HashSet<u32>) {
        match ty {
            Ty::Var(id) => {
                vars.insert(*id);
                if let Some(resolved) = self.substitution.get(id) {
                    self.collect_type_vars_inner(resolved, vars);
                }
            }
            Ty::Fn(params, ret, _) => {
                for p in params {
                    self.collect_type_vars_inner(p, vars);
                }
                self.collect_type_vars_inner(ret, vars);
            }
            Ty::App(_, args) => {
                for a in args {
                    self.collect_type_vars_inner(a, vars);
                }
            }
            Ty::Tuple(ts) => {
                for t in ts {
                    self.collect_type_vars_inner(t, vars);
                }
            }
            Ty::Record(fields) => {
                for (_, t) in fields {
                    self.collect_type_vars_inner(t, vars);
                }
            }
            _ => {}
        }
    }
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

// ── Free helper functions for pattern analysis ──────────────────────

fn pattern_contains_bool(pattern: &Pattern, val: bool) -> bool {
    match pattern {
        Pattern::BoolLit(b) => *b == val,
        Pattern::Or(pats) => pats.iter().any(|p| pattern_contains_bool(p, val)),
        _ => false,
    }
}

fn mark_covered_variants(pattern: &Pattern, variant_names: &[String], covered: &mut Vec<bool>) {
    match pattern {
        Pattern::Constructor(name, _) => {
            if let Some(idx) = variant_names.iter().position(|v| v == name) {
                covered[idx] = true;
            }
        }
        Pattern::Var(name) => {
            if let Some(idx) = variant_names.iter().position(|v| v == name) {
                covered[idx] = true;
            }
        }
        Pattern::Wildcard => {
            for c in covered.iter_mut() {
                *c = true;
            }
        }
        Pattern::Or(pats) => {
            for p in pats {
                mark_covered_variants(p, variant_names, covered);
            }
        }
        _ => {}
    }
}
