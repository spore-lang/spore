//! Core type-checking logic.
//!
//! Walks the AST and verifies type consistency, building up a type
//! environment as it goes. Reports all errors (does not bail on first).

use sporec_parser::ast::*;

use crate::env::{Env, HandlerInfo, TypeRegistry};
use crate::error::{ErrorCode, TypeError};
use crate::hole::{HoleDependencyGraph, HoleInfo, HoleReport};
use crate::module::{ImportedSymbol, ModuleError, ModuleRegistry};
use std::collections::{HashMap, HashSet};

use crate::capability::{CapabilityHierarchy, default_hierarchy};
use crate::concurrency::ConcurrencyAnalyzer;
use crate::types::{CapSet, ErrorSet, Ty};

use std::collections::BTreeSet;

/// Return items present in `callee_set` but absent from `current_set`.
fn find_missing_set_items<'a>(
    callee_set: &'a BTreeSet<String>,
    current_set: &BTreeSet<String>,
) -> Vec<&'a str> {
    callee_set
        .iter()
        .filter(|item| !current_set.contains(*item))
        .map(|s| s.as_str())
        .collect()
}

fn handler_self_type_name(name: &str) -> String {
    format!("__handler::{name}")
}

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
    /// Name of the module currently being checked.
    current_module_name: String,
    /// Declared return type of the current function (for hole inference).
    expected_return_type: Option<Ty>,
    /// `@allows[...]` default allow-list in scope for hole suggestions.
    current_hole_allows: Option<Vec<String>>,
    /// Next type variable ID for fresh type variables.
    next_var_id: u32,
    /// Next synthetic name ID for unnamed holes (`?`).
    next_unnamed_hole_id: u32,
    /// Substitution map: type variable ID → resolved type.
    substitution: HashMap<u32, Ty>,
    /// Capability hierarchy for expanding parent caps (e.g. IO → 4 leaves).
    hierarchy: CapabilityHierarchy,
    /// Structured concurrency analyzer (parallel scopes + spawn sites).
    concurrency: ConcurrencyAnalyzer,
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
            current_module_name: String::new(),
            expected_return_type: None,
            current_hole_allows: None,
            next_var_id: 0,
            next_unnamed_hole_id: 0,
            substitution: HashMap::new(),
            hierarchy: default_hierarchy(),
            concurrency: ConcurrencyAnalyzer::new(),
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
            current_module_name: String::new(),
            expected_return_type: None,
            current_hole_allows: None,
            next_var_id: 0,
            next_unnamed_hole_id: 0,
            substitution: HashMap::new(),
            hierarchy: default_hierarchy(),
            concurrency: ConcurrencyAnalyzer::new(),
        }
    }

    /// Type-check an entire module.
    pub fn check_module(&mut self, module: &Module) {
        self.current_module_name = module.name.clone();
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
        // Check for circular module dependencies
        for cycle in self.module_registry.detect_cycles() {
            self.err(
                ErrorCode::M0101,
                format!("circular module dependency: {}", cycle.join(" -> ")),
            );
        }
        // Second pass: check function bodies
        for item in &module.items {
            self.check_item(item);
        }
        // Build the hole dependency graph based on shared type variables
        self.hole_report.dependency_graph = self.build_hole_dependency_graph();
    }

    /// Register prelude declarations into the local checker registry.
    pub(crate) fn load_prelude(&mut self, module: &Module) {
        for item in &module.items {
            self.register_item(item);
        }
    }

    // ── Registration (first pass) ───────────────────────────────────

    /// Resolve an import declaration, importing symbols into the current registry.
    fn resolve_import(&mut self, import: &ImportDecl) {
        let (path, alias) = match import {
            ImportDecl::Import { path, alias, .. } => (path.as_str(), alias.as_str()),
            ImportDecl::Alias { name, path, .. } => (path.as_str(), name.as_str()),
        };
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
                if !self.current_module_name.is_empty() {
                    self.module_registry
                        .record_dependency(&self.current_module_name, path);
                }
                self.import_resolved_symbols(&module, &resolved, alias);
            }
            Err(ModuleError::PrivateSymbol { symbol, module: m }) => {
                self.err(
                    ErrorCode::M0003,
                    format!("symbol `{symbol}` in module `{m}` is private and not accessible"),
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
            Err(ModuleError::CircularDependency(cycle)) => {
                self.err(
                    ErrorCode::M0101,
                    format!("circular module dependency: {}", cycle.join(" -> ")),
                );
            }
            Err(ModuleError::IoError { module: m, detail }) => {
                self.err(
                    ErrorCode::M0001,
                    format!("cannot read module `{m}`: {detail}"),
                );
            }
            Err(ModuleError::ParseErrors { module: m, errors }) => {
                self.err(
                    ErrorCode::M0001,
                    format!(
                        "parse error in module `{m}`: {}",
                        errors
                            .into_iter()
                            .map(|error| error.to_string())
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                );
            }
        }
    }

    /// Import resolved symbols from a module into the current type registry.
    ///
    /// When `alias` differs from the module's own leaf name, symbols are
    /// registered under `alias.symbol` so that user code can write
    /// `Alias.func(…)` instead of `Original.func(…)`.
    fn import_resolved_symbols(
        &mut self,
        module: &crate::module::ModuleInterface,
        resolved: &[(String, ImportedSymbol)],
        alias: &str,
    ) {
        // Determine the prefix that will qualify imported symbols.
        // If the caller supplied a non-empty alias that differs from the
        // module's leaf name, register under the alias as well.
        let _alias_prefix: Option<&str> = if alias.is_empty() { None } else { Some(alias) };
        // NOTE: Currently symbols are imported *unqualified* (bare names)
        // into the local registry.  When we add qualified-name resolution
        // (`Alias.func`), `_alias_prefix` should be used to register a
        // second, qualified entry.  The alias is now accepted (no longer
        // prefixed with `_`) to silence the "unused alias" warnings once
        // callers start threading it through.

        for (name, kind) in resolved {
            match kind {
                ImportedSymbol::Function => {
                    if let Some((params, ret)) = module.functions.get(name) {
                        let caps = module.function_caps.get(name).cloned().unwrap_or_default();
                        let errors = module
                            .function_errors
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        let type_params = module
                            .function_type_params
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        let where_bounds = module
                            .function_where_bounds
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        // Detect ambiguous imports: if the name already
                        // exists from a *different* module, emit an error.
                        if let Some(existing) = self.registry.functions.get(name) {
                            // Same signature from the prelude is OK (re-import);
                            // different signature means different source module.
                            let existing_errors = self
                                .registry
                                .fn_errors
                                .get(name)
                                .cloned()
                                .unwrap_or_default();
                            let existing_type_params = self
                                .registry
                                .fn_type_params
                                .get(name)
                                .cloned()
                                .unwrap_or_default();
                            let existing_where_bounds = self
                                .registry
                                .fn_where_bounds
                                .get(name)
                                .cloned()
                                .unwrap_or_default();
                            if existing.0 != *params
                                || existing.1 != *ret
                                || existing.2 != caps
                                || existing_errors != errors
                                || existing_type_params != type_params
                                || existing_where_bounds != where_bounds
                            {
                                self.err(
                                    ErrorCode::M0303,
                                    format!(
                                        "ambiguous import: `{name}` is exported by multiple imported modules"
                                    ),
                                );
                                continue;
                            }
                        }
                        // Preserve exported `uses [...]` metadata so imported
                        // calls propagate capabilities across module boundaries.
                        self.registry
                            .functions
                            .insert(name.clone(), (params.clone(), ret.clone(), caps));
                        if !errors.is_empty() {
                            self.registry.fn_errors.insert(name.clone(), errors);
                        }
                        if !type_params.is_empty() {
                            self.registry
                                .fn_type_params
                                .insert(name.clone(), type_params);
                        }
                        if !where_bounds.is_empty() {
                            self.registry
                                .fn_where_bounds
                                .insert(name.clone(), where_bounds);
                        }
                    }
                }
                ImportedSymbol::Type => {
                    if let Some(variants) = module.types.get(name) {
                        self.registry.types.insert(name.clone(), variants.clone());
                    }
                }
                ImportedSymbol::Struct => {
                    if let Some(fields) = module.structs.get(name) {
                        self.registry.structs.insert(name.clone(), fields.clone());
                    }
                    if let Some(type_params) = module.struct_type_params.get(name) {
                        self.registry
                            .struct_type_params
                            .insert(name.clone(), type_params.clone());
                    }
                }
                ImportedSymbol::Handler => {
                    if let Some(handler) = module.handlers.get(name) {
                        if let Some(existing) = self.registry.handlers.get(name)
                            && existing != handler
                        {
                            self.err(
                                ErrorCode::M0303,
                                format!(
                                    "ambiguous import: `{name}` is exported by multiple imported modules"
                                ),
                            );
                            continue;
                        }
                        self.registry.handlers.insert(name.clone(), handler.clone());
                    }
                }
                ImportedSymbol::Capability => {
                    if module.capabilities.contains(name) {
                        let methods = module
                            .capability_methods
                            .get(name)
                            .cloned()
                            .unwrap_or((Vec::new(), Vec::new()));
                        if let Some(existing) = self.registry.capabilities.get(name)
                            && existing != &methods
                        {
                            self.err(
                                ErrorCode::M0303,
                                format!(
                                    "ambiguous import: `{name}` is exported by multiple imported modules"
                                ),
                            );
                            continue;
                        }
                        self.registry.capabilities.insert(name.clone(), methods);
                    }
                }
            }
        }
    }

    fn register_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => {
                let mut signature_holes = HashMap::new();
                let param_tys: Vec<Ty> = f
                    .params
                    .iter()
                    .map(|p| self.resolve_signature_type(&p.ty, &mut signature_holes))
                    .collect();
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_signature_type(t, &mut signature_holes))
                    .unwrap_or(Ty::Unit);
                let caps: CapSet = f
                    .uses_clause
                    .as_ref()
                    .map(|uc| self.declared_capabilities(Some(uc)))
                    .unwrap_or_default();
                self.registry
                    .functions
                    .insert(f.name.clone(), (param_tys, ret_ty, caps));
                // Register error set (! E1 | E2)
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
                let mut type_params = f.type_params.clone();
                if let Some(wc) = &f.where_clause {
                    type_params.extend(wc.constraints.iter().map(|c| c.type_var.clone()));
                }
                type_params.sort();
                type_params.dedup();
                if !type_params.is_empty() {
                    self.registry
                        .fn_type_params
                        .insert(f.name.clone(), type_params);
                }
                if let Some(wc) = &f.where_clause
                    && !wc.constraints.is_empty()
                {
                    self.registry.fn_where_bounds.insert(
                        f.name.clone(),
                        wc.constraints
                            .iter()
                            .map(|c| (c.type_var.clone(), c.bound.clone()))
                            .collect(),
                    );
                }
            }
            Item::StructDef(s) => {
                let fields: Vec<(String, Ty)> = s
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), self.resolve_type(&f.ty)))
                    .collect();
                self.registry.structs.insert(s.name.clone(), fields);
                if !s.type_params.is_empty() {
                    self.registry
                        .struct_type_params
                        .insert(s.name.clone(), s.type_params.clone());
                }
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
                        if t.type_params.is_empty() {
                            // Non-generic zero-field variant: register as a value.
                            self.env.define(vname.clone(), ret_ty.clone());
                        } else {
                            // Generic zero-field variant: treat it like a zero-arg constructor so
                            // each use can freshen the type parameters independently.
                            self.registry
                                .functions
                                .insert(vname.clone(), (Vec::new(), ret_ty.clone(), CapSet::new()));
                            self.registry
                                .fn_type_params
                                .insert(vname.clone(), t.type_params.clone());
                        }
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
            Item::EffectAlias(ea) => {
                // Register the alias into the capability hierarchy so that
                // `uses [AliasName]` expands to its constituent effects.
                for component in &ea.effects {
                    self.hierarchy
                        .add_implies(ea.name.clone(), component.clone());
                }
            }
            Item::TraitDef(td) => {
                let methods: Vec<(String, Vec<Ty>, Ty)> = td
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
                    .insert(td.name.clone(), (td.type_params.clone(), methods));
            }
            Item::EffectDef(ed) => {
                let methods: Vec<(String, Vec<Ty>, Ty)> = ed
                    .operations
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
                    .insert(ed.name.clone(), (vec![], methods));
            }
            Item::HandlerDef(hd) => {
                if !self.registry.capabilities.contains_key(&hd.effect) {
                    self.err(ErrorCode::C0002, format!("unknown effect `{}`", hd.effect));
                    return;
                }
                let fields: Vec<(String, Ty)> = hd
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), self.resolve_type(&field.ty)))
                    .collect();
                let methods: Vec<(String, Vec<Ty>, Ty)> = hd
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
                self.registry.handlers.insert(
                    hd.name.clone(),
                    HandlerInfo {
                        effect: hd.effect.clone(),
                        fields: fields.clone(),
                        methods: methods.clone(),
                    },
                );
                self.registry
                    .structs
                    .insert(handler_self_type_name(&hd.name), fields);
                self.registry
                    .impls
                    .insert((hd.effect.clone(), hd.name.clone()), methods);
            }
            Item::Alias(alias_def) => {
                let resolved = self.resolve_type(&alias_def.target);
                self.registry
                    .type_aliases
                    .insert(alias_def.name.clone(), resolved);
            }
        }
    }

    pub(crate) fn declared_capabilities(
        &self,
        uses_clause: Option<&UsesClause>,
    ) -> BTreeSet<String> {
        uses_clause
            .map(|uc| {
                let raw =
                    crate::capability::CapabilitySet::from_names(uc.resources.iter().cloned());
                self.hierarchy.expand(&raw).to_btreeset()
            })
            .unwrap_or_default()
    }

    // ── Checking (second pass) ──────────────────────────────────────

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => self.check_fn(f),
            Item::ImplDef(impl_def) => self.check_impl(impl_def),
            Item::HandlerDef(handler_def) => self.check_handler(handler_def),
            Item::EffectAlias(ea) => {
                // Validate that every component of the alias names a known
                // effect/capability defined in this module (or imported).
                // TODO: cross-module alias export — aliases are currently only
                //       visible within the same module's Checker instance.
                for component in &ea.effects {
                    if !self.registry.capabilities.contains_key(component) {
                        self.err(
                            ErrorCode::C0002,
                            format!(
                                "effect alias `{}` references unknown effect `{}`",
                                ea.name, component
                            ),
                        );
                    }
                }
            }
            _ => {} // structs/types already registered; capabilities/imports deferred
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_contract_impl(
        &mut self,
        contract_name: &str,
        contract_methods: &[(String, Vec<Ty>, Ty)],
        methods: &[FnDef],
        impl_label: &str,
        member_noun: &str,
        contract_noun: &str,
        span: Option<Span>,
        type_mapping: &HashMap<String, Ty>,
        extra_bindings: &[(String, Ty)],
    ) {
        for (method_name, _expected_params, _expected_ret) in contract_methods {
            if !methods.iter().any(|m| &m.name == method_name) {
                let msg = format!("{impl_label} is missing {member_noun} `{method_name}`");
                if let Some(span) = span {
                    self.err_at(ErrorCode::E0013, msg, span);
                } else {
                    self.err(ErrorCode::E0013, msg);
                }
            }
        }

        for method in methods {
            if !contract_methods
                .iter()
                .any(|(name, _, _)| name == &method.name)
            {
                self.err(
                    ErrorCode::E0014,
                    format!(
                        "{member_noun} `{}` is not defined in {contract_noun} `{contract_name}`",
                        method.name
                    ),
                );
            }
        }

        for method in methods {
            if let Some((_expected_name, expected_params, expected_ret)) = contract_methods
                .iter()
                .find(|(name, _, _)| name == &method.name)
            {
                let expected_params: Vec<Ty> = expected_params
                    .iter()
                    .map(|t| self.instantiate_ty(t, type_mapping))
                    .collect();
                let expected_ret = self.instantiate_ty(expected_ret, type_mapping);

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
                            "{member_noun} `{}` in {impl_label} expects {} parameters, got {}",
                            method.name,
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
                                "parameter {} of {member_noun} `{}` in {impl_label}",
                                i + 1,
                                method.name
                            ),
                        );
                    }
                }
                self.unify(
                    &expected_ret,
                    &impl_ret,
                    &format!(
                        "return type of {member_noun} `{}` in {impl_label}",
                        method.name
                    ),
                );
            }
        }

        for method in methods {
            self.check_fn_with_extra_bindings(method, extra_bindings);
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

        let impl_label = format!(
            "impl `{}` for `{}`",
            impl_def.capability, impl_def.target_type
        );
        self.check_contract_impl(
            &impl_def.capability,
            &cap_methods,
            &impl_def.methods,
            &impl_label,
            "method",
            "capability",
            impl_def.span,
            &type_mapping,
            &[],
        );
    }

    fn check_handler(&mut self, handler_def: &HandlerDef) {
        let Some((_effect_type_params, effect_methods)) =
            self.registry.capabilities.get(&handler_def.effect).cloned()
        else {
            return; // Already errored in registration
        };

        let impl_label = format!(
            "handler `{}` for effect `{}`",
            handler_def.name, handler_def.effect
        );
        let self_ty = Ty::Named(handler_self_type_name(&handler_def.name));
        let extra_bindings = vec![("self".to_string(), self_ty)];
        self.check_contract_impl(
            &handler_def.effect,
            &effect_methods,
            &handler_def.methods,
            &impl_label,
            "operation",
            "effect",
            handler_def.span,
            &HashMap::new(),
            &extra_bindings,
        );
    }

    fn check_fn(&mut self, f: &FnDef) {
        self.check_fn_with_extra_bindings(f, &[]);
    }

    fn check_fn_with_extra_bindings(&mut self, f: &FnDef, extra_bindings: &[(String, Ty)]) {
        self.concurrency.enter_function(&f.name);
        // Set current function's capability set (with hierarchy expansion)
        let declared_caps = self.declared_capabilities(f.uses_clause.as_ref());
        let prev_caps = std::mem::replace(&mut self.current_caps, declared_caps);

        // Set current function's error set (! E1 | E2)
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
        let (declared_param_tys, declared_ret) = self
            .registry
            .functions
            .get(&f.name)
            .map(|(params, ret, _)| (params.clone(), ret.clone()))
            .unwrap_or_else(|| {
                (
                    f.params.iter().map(|p| self.resolve_type(&p.ty)).collect(),
                    f.return_type
                        .as_ref()
                        .map(|t| self.resolve_type(t))
                        .unwrap_or(Ty::Unit),
                )
            });
        let prev_expected = self.expected_return_type.take();
        self.expected_return_type = Some(declared_ret.clone());
        let prev_hole_allows = self.current_hole_allows.take();
        self.current_hole_allows = f.hole_allows.clone();

        self.env.push_scope();

        // Bind parameters
        for (param, ty) in f.params.iter().zip(declared_param_tys) {
            self.env.define(param.name.clone(), ty);
        }
        for (name, ty) in extra_bindings {
            self.env.define(name.clone(), ty.clone());
        }

        // Check body if present
        if let Some(body) = &f.body {
            let body_ty = self.check_expr(body);
            let body_ty = self.apply_subst(&body_ty);
            let declared_ret = self.apply_subst(&declared_ret);

            self.unify(&declared_ret, &body_ty, &format!("function `{}`", f.name));
        }

        // Check spec clause (in scope where params are bound + fn is registered)
        if let Some(spec) = &f.spec_clause {
            self.check_spec_clause(spec, &f.name);
        }

        self.env.pop_scope();
        self.current_caps = prev_caps;
        self.current_errors = prev_errors;
        self.current_function = prev_function;
        self.expected_return_type = prev_expected;
        self.current_hole_allows = prev_hole_allows;
        self.concurrency.leave_function(&f.name);
    }

    /// Type-check a `spec { ... }` clause attached to a function.
    fn check_spec_clause(&mut self, spec: &SpecClause, fn_name: &str) {
        use crate::types::Ty;

        for item in &spec.items {
            match item {
                SpecItem::Example(ex) => {
                    let ty = self.check_expr(&ex.body);
                    let ty = self.apply_subst(&ty);
                    self.unify(
                        &Ty::Bool,
                        &ty,
                        &format!("spec example \"{}\" in `{fn_name}`", ex.label),
                    );
                }
                SpecItem::Property(prop) => {
                    let ty = self.check_expr(&prop.predicate);
                    let ty = self.apply_subst(&ty);
                    match &ty {
                        Ty::Fn(_, ret, _, _) => {
                            self.unify(
                                &Ty::Bool,
                                ret,
                                &format!("spec property \"{}\" in `{fn_name}`", prop.label),
                            );
                        }
                        _ => {
                            self.err(
                                ErrorCode::E0301,
                                format!(
                                    "spec property \"{}\" in `{fn_name}` must be a lambda, found {ty:?}",
                                    prop.label
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    // ── Expression type checking ────────────────────────────────────

    fn check_expr(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::IntLit(_) => Ty::I32,
            Expr::FloatLit(_) => Ty::F64,
            Expr::StrLit(_) => Ty::Str,
            Expr::BoolLit(_) => Ty::Bool,
            Expr::FString(_) => Ty::Str,
            Expr::TString(_) => Ty::Str,

            Expr::Var(name) => {
                if let Some(ty) = self.env.lookup(name) {
                    ty.clone()
                } else if let Some((params, ret, caps)) = self.registry.functions.get(name).cloned()
                {
                    if params.is_empty() && self.find_unit_variant(name).is_some() {
                        if let Some(type_params) = self.registry.fn_type_params.get(name).cloned() {
                            let (_, ret, _) = self.instantiate_sig(&type_params, &[], &ret);
                            ret
                        } else {
                            ret
                        }
                    } else {
                        // bare function name as value — return its function type
                        let errors = self
                            .registry
                            .fn_errors
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        Ty::Fn(params, Box::new(ret), caps, errors)
                    }
                } else if let Some((params, ret, caps)) = self.lookup_module_function(name) {
                    Ty::Fn(params, Box::new(ret), caps, ErrorSet::new())
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
                        if !ty.is_integer() && !ty.is_error() {
                            self.err(ErrorCode::E0002, format!("cannot apply `~` to type `{ty}`"));
                        }
                        ty
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
                Ty::Fn(param_tys, Box::new(ret_ty), CapSet::new(), ErrorSet::new())
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
                    // If one branch diverges (Never), the overall type is the other branch.
                    if matches!(then_ty, Ty::Never) {
                        else_ty
                    } else if matches!(else_ty, Ty::Never) {
                        then_ty
                    } else {
                        self.unify(&then_ty, &else_ty, "if/else branches");
                        then_ty
                    }
                } else {
                    // No else branch: the expression types as Unit.
                    // Unify then_ty with Unit so non-Unit then-branches are flagged.
                    self.unify(&Ty::Unit, &then_ty, "if without else must be Unit");
                    Ty::Unit
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
                        // If the accumulated result type is Never (all prior arms diverged),
                        // adopt this arm's type. If this arm diverges, keep the existing type.
                        if matches!(expected, Ty::Never) {
                            result_ty = Some(arm_ty);
                        } else if !matches!(arm_ty, Ty::Never) {
                            self.unify(expected, &arm_ty, "match arms");
                        }
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
                    Ty::Fn(params, ret, caps, errors) => {
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
                        self.check_error_propagation(&errors);
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
                        if let Some(fields) = self.registry.structs.get(name).cloned() {
                            let (fields, _) = self.struct_fields_for_type(name, &fields, &ty);
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
                    let (def_fields, struct_ty) = self.instantiate_struct_fields(name, &def_fields);
                    // Check for duplicate fields in the literal
                    let mut seen = HashSet::new();
                    for (fname, _) in fields.iter() {
                        if !seen.insert(fname.as_str()) {
                            self.err(
                                ErrorCode::E0015,
                                format!("duplicate field `{fname}` in struct `{name}`"),
                            );
                        }
                    }

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

                    // Check for missing required fields
                    let provided_names: HashSet<&str> =
                        fields.iter().map(|(n, _)| n.as_str()).collect();
                    for (def_name, _) in &def_fields {
                        if !provided_names.contains(def_name.as_str()) {
                            self.err(
                                ErrorCode::E0015,
                                format!("missing field `{def_name}` in struct `{name}`"),
                            );
                        }
                    }

                    struct_ty
                } else {
                    self.err(ErrorCode::E0005, format!("undefined struct `{name}`"));
                    Ty::Error
                }
            }

            Expr::Try(expr) => self.check_expr(expr),

            Expr::Hole(name, ty_hint, allows) => {
                let hole_name = name
                    .clone()
                    .unwrap_or_else(|| self.fresh_unnamed_hole_name());
                let effective_allows = allows.clone().or_else(|| self.current_hole_allows.clone());
                let inferred_from_allows = effective_allows
                    .as_deref()
                    .and_then(|allowed| self.infer_hole_type_from_allows(allowed));
                let return_expected = self
                    .expected_return_type
                    .as_ref()
                    .map(|ret| self.apply_subst(ret));
                let (ty, type_inferred_from) = if let Some(te) = ty_hint {
                    (
                        self.resolve_type(te),
                        Some("hole type annotation".to_string()),
                    )
                } else if let Some(ret) = return_expected {
                    if matches!(ret, Ty::Var(_) | Ty::Hole(_)) {
                        if let Some(inferred) = inferred_from_allows {
                            (inferred, Some("`@allows[...]` candidates".to_string()))
                        } else {
                            (
                                ret,
                                Some(format!("return type of `{}`", self.current_function)),
                            )
                        }
                    } else {
                        (
                            ret,
                            Some(format!("return type of `{}`", self.current_function)),
                        )
                    }
                } else if let Some(inferred) = inferred_from_allows {
                    (inferred, Some("`@allows[...]` candidates".to_string()))
                } else {
                    (Ty::Hole(hole_name.clone()), None)
                };

                // Collect hole info for the report (v0.3)
                let bindings = self.env.all_bindings();
                let expected = self.apply_subst(&ty);
                let suggestions = self.find_suggestions(&expected, effective_allows.as_deref());

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
                    name: hole_name,
                    location: None,
                    expected_type: expected,
                    type_inferred_from,
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
                if !self.current_caps.contains("Spawn") {
                    self.err(
                        ErrorCode::C0001,
                        "spawn requires capability `Spawn`; add `uses [Spawn]`".to_string(),
                    );
                }
                if !self.concurrency.in_parallel_scope(&self.current_function) {
                    self.err(
                        ErrorCode::C0103,
                        "spawn is only allowed inside `parallel_scope { ... }`".to_string(),
                    );
                }
                let inner = self.check_expr(expr);
                self.concurrency.record_spawn(
                    &self.current_function,
                    &inner.to_string(),
                    self.current_caps.iter().cloned().collect(),
                );
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

            Expr::ChannelNew { elem_type, buffer } => {
                let buffer_ty = self.check_expr(buffer);
                self.unify(&Ty::I32, &buffer_ty, "Channel.new buffer");
                let elem_ty = self.resolve_type(elem_type);
                Ty::Tuple(vec![
                    Ty::App("Sender".into(), vec![elem_ty.clone()]),
                    Ty::App("Receiver".into(), vec![elem_ty]),
                ])
            }

            Expr::Return(expr) => {
                if let Some(inner) = expr {
                    let ret_val_ty = self.check_expr(inner);
                    if let Some(expected) = self.expected_return_type.clone() {
                        self.unify(&expected, &ret_val_ty, "return");
                    }
                }
                Ty::Never
            }

            Expr::Throw(expr) => {
                let _ = self.check_expr(expr);
                self.check_throw_coverage(expr);
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
                    if lanes_ty != Ty::I32 && !lanes_ty.is_error() {
                        self.err(
                            ErrorCode::E0002,
                            format!("parallel_scope lanes must be Int, got `{lanes_ty}`"),
                        );
                    }
                    if let Expr::IntLit(n) = lanes_expr.as_ref()
                        && *n <= 0
                    {
                        self.err(
                            ErrorCode::E0002,
                            format!("parallel_scope lanes must be positive, got `{n}`"),
                        );
                    }
                    if let Expr::IntLit(n) = lanes_expr.as_ref() {
                        let spawn_sites = Self::count_spawns(body);
                        if spawn_sites > *n as usize {
                            self.err(
                                ErrorCode::C0103,
                                format!(
                                    "parallel_scope(lanes: {n}) has {spawn_sites} spawn site(s) in body"
                                ),
                            );
                        }
                    }
                }
                self.concurrency
                    .enter_parallel_scope(&self.current_function);
                let body_ty = self.check_expr(body);
                self.concurrency
                    .leave_parallel_scope(&self.current_function);
                body_ty
            }

            Expr::Select(arms) => {
                let mut result_ty: Option<Ty> = None;
                for arm in arms {
                    let arm_ty = match arm {
                        SelectArm::Recv {
                            binding,
                            source,
                            body,
                        } => {
                            let source_raw_ty = self.check_expr(source);
                            let source_ty = self.apply_subst(&source_raw_ty);
                            self.env.push_scope();
                            let binding_ty = match source_ty {
                                Ty::App(ref name, ref args)
                                    if name == "Receiver" && args.len() == 1 =>
                                {
                                    args[0].clone()
                                }
                                Ty::Error => Ty::Error,
                                other => {
                                    self.err(
                                        ErrorCode::E0001,
                                        format!("select source must be Receiver[T], got `{other}`"),
                                    );
                                    Ty::Error
                                }
                            };
                            self.env.define(binding.clone(), binding_ty);
                            let arm_ty = self.check_expr(body);
                            self.env.pop_scope();
                            arm_ty
                        }
                        SelectArm::Timeout { duration, body } => {
                            let duration_ty = self.check_expr(duration);
                            self.unify(&Ty::I32, &duration_ty, "select timeout");
                            self.check_expr(body)
                        }
                    };
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

            Expr::Perform {
                effect,
                operation,
                args,
            } => {
                // Verify the effect capability is in the current function's uses set
                if !self.current_caps.contains(effect) {
                    self.err(
                        ErrorCode::C0001,
                        format!(
                            "perform requires capability `{effect}` but current function does not declare it"
                        ),
                    );
                }
                if !self.registry.capabilities.contains_key(effect) {
                    for arg in args {
                        let _ = self.check_expr(arg);
                    }
                    self.err(ErrorCode::C0002, format!("unknown effect `{effect}`"));
                    return Ty::Error;
                }
                if let Some((param_tys, ret_ty)) =
                    self.lookup_registered_effect_operation(effect, operation)
                {
                    if param_tys.len() != args.len() {
                        self.err(
                            ErrorCode::E0007,
                            format!(
                                "effect operation `{effect}.{operation}` expects {} arguments, got {}",
                                param_tys.len(),
                                args.len()
                            ),
                        );
                        for arg in args {
                            let _ = self.check_expr(arg);
                        }
                        return self.apply_subst(&ret_ty);
                    }
                    for (i, (expected, arg_expr)) in param_tys.iter().zip(args).enumerate() {
                        let arg_ty = self.check_expr(arg_expr);
                        self.unify(
                            expected,
                            &arg_ty,
                            &format!("argument {} of `{effect}.{operation}`", i + 1),
                        );
                    }
                    return self.apply_subst(&ret_ty);
                }
                for arg in args {
                    let _ = self.check_expr(arg);
                }
                Ty::Error
            }

            Expr::Handle { body, handlers } => {
                let mut provided_caps: CapSet = std::collections::BTreeSet::new();
                let mut seen_operations: HashSet<(String, String)> = HashSet::new();

                for binding in handlers {
                    match binding {
                        HandleBinding::On(arm) => {
                            if !self.registry.capabilities.contains_key(&arm.effect) {
                                self.err(
                                    ErrorCode::C0002,
                                    format!("unknown effect `{}`", arm.effect),
                                );
                                continue;
                            }
                            provided_caps.insert(arm.effect.clone());
                            let key = (arm.effect.clone(), arm.operation.clone());
                            if !seen_operations.insert(key.clone()) {
                                self.err(
                                    ErrorCode::E0014,
                                    format!(
                                        "duplicate handler binding for `{}.{}` in one `with` block",
                                        key.0, key.1
                                    ),
                                );
                            }
                        }
                        HandleBinding::Use(handler_use) => {
                            let Some(info) =
                                self.registry.handlers.get(&handler_use.handler).cloned()
                            else {
                                self.err(
                                    ErrorCode::C0002,
                                    format!("unknown handler `{}`", handler_use.handler),
                                );
                                continue;
                            };

                            provided_caps.insert(info.effect.clone());
                            for (operation, _, _) in &info.methods {
                                let key = (info.effect.clone(), operation.clone());
                                if !seen_operations.insert(key.clone()) {
                                    self.err(
                                        ErrorCode::E0014,
                                        format!(
                                            "duplicate handler binding for `{}.{}` in one `with` block",
                                            key.0, key.1
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }

                for binding in handlers {
                    if let HandleBinding::Use(handler_use) = binding {
                        let Some(info) = self.registry.handlers.get(&handler_use.handler).cloned()
                        else {
                            continue;
                        };

                        let mut seen_fields = HashSet::new();
                        for (field_name, value_expr) in &handler_use.payload {
                            if !seen_fields.insert(field_name.clone()) {
                                self.err(
                                    ErrorCode::E0015,
                                    format!(
                                        "duplicate payload field `{field_name}` in handler `{}`",
                                        handler_use.handler
                                    ),
                                );
                            }

                            let value_ty = self.check_expr(value_expr);
                            if let Some((_, expected_ty)) =
                                info.fields.iter().find(|(name, _)| name == field_name)
                            {
                                self.unify(
                                    expected_ty,
                                    &value_ty,
                                    &format!(
                                        "payload field `{field_name}` for handler `{}`",
                                        handler_use.handler
                                    ),
                                );
                            } else {
                                self.err(
                                    ErrorCode::E0015,
                                    format!(
                                        "handler `{}` has no payload field `{field_name}`",
                                        handler_use.handler
                                    ),
                                );
                            }
                        }

                        for (field_name, _) in &info.fields {
                            if !handler_use
                                .payload
                                .iter()
                                .any(|(name, _)| name == field_name)
                            {
                                self.err(
                                    ErrorCode::E0101,
                                    format!(
                                        "handler `{}` is missing payload field `{field_name}`",
                                        handler_use.handler
                                    ),
                                );
                            }
                        }
                    }
                }

                let prev_caps = self.current_caps.clone();
                self.current_caps.extend(provided_caps);

                let body_ty = self.check_expr(body);

                for binding in handlers {
                    let HandleBinding::On(arm) = binding else {
                        continue;
                    };

                    self.env.push_scope();
                    if self.registry.capabilities.contains_key(&arm.effect) {
                        if let Some((param_tys, ret_ty)) =
                            self.lookup_registered_effect_operation(&arm.effect, &arm.operation)
                        {
                            if param_tys.len() != arm.params.len() {
                                self.err(
                                    ErrorCode::E0007,
                                    format!(
                                        "handler arm `{}.{}` expects {} parameters, got {}",
                                        arm.effect,
                                        arm.operation,
                                        param_tys.len(),
                                        arm.params.len()
                                    ),
                                );
                            }

                            for (param, expected_ty) in arm.params.iter().zip(param_tys.iter()) {
                                self.env.define(param.clone(), expected_ty.clone());
                            }
                            for param in arm.params.iter().skip(param_tys.len()) {
                                let var = self.fresh_var();
                                self.env.define(param.clone(), var);
                            }

                            let arm_ty = self.check_expr(&arm.body);
                            self.unify(
                                &ret_ty,
                                &arm_ty,
                                &format!("handler arm `{}.{}`", arm.effect, arm.operation),
                            );
                        } else {
                            for param in &arm.params {
                                let var = self.fresh_var();
                                self.env.define(param.clone(), var);
                            }
                            let _ = self.check_expr(&arm.body);
                        }
                    } else {
                        for param in &arm.params {
                            let var = self.fresh_var();
                            self.env.define(param.clone(), var);
                        }
                        let _ = self.check_expr(&arm.body);
                    }
                    self.env.pop_scope();
                }

                self.current_caps = prev_caps;
                body_ty
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
            // Bitwise: both integer
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                if !lt.is_integer() && !lt.is_error() {
                    self.err(
                        ErrorCode::E0002,
                        format!("bitwise `{op:?}` expects integer type, got `{lt}`"),
                    );
                }
                if !rt.is_integer() && !rt.is_error() {
                    self.err(
                        ErrorCode::E0002,
                        format!("bitwise `{op:?}` expects integer type, got `{rt}`"),
                    );
                }
                lt
            }
        }
    }

    fn count_spawns(expr: &Expr) -> usize {
        match expr {
            Expr::Spawn(inner) => 1 + Self::count_spawns(inner),
            Expr::Call(callee, args) => {
                Self::count_spawns(callee) + args.iter().map(Self::count_spawns).sum::<usize>()
            }
            Expr::Lambda(_, body)
            | Expr::Await(body)
            | Expr::Try(body)
            | Expr::Return(Some(body))
            | Expr::Throw(body)
            | Expr::UnaryOp(_, body) => Self::count_spawns(body),
            Expr::BinOp(lhs, _, rhs) | Expr::Pipe(lhs, rhs) => {
                Self::count_spawns(lhs) + Self::count_spawns(rhs)
            }
            Expr::FieldAccess(base, _) => Self::count_spawns(base),
            Expr::If(cond, then_branch, else_branch) => {
                Self::count_spawns(cond)
                    + Self::count_spawns(then_branch)
                    + else_branch
                        .as_ref()
                        .map_or(0, |else_expr| Self::count_spawns(else_expr))
            }
            Expr::Match(scrutinee, arms) => {
                Self::count_spawns(scrutinee)
                    + arms
                        .iter()
                        .map(|arm| {
                            arm.guard.as_ref().map_or(0, Self::count_spawns)
                                + Self::count_spawns(&arm.body)
                        })
                        .sum::<usize>()
            }
            Expr::Block(stmts, tail) => {
                let stmt_spawns: usize = stmts
                    .iter()
                    .map(|stmt| match stmt {
                        Stmt::Let(_, _, value) => Self::count_spawns(value),
                        Stmt::Expr(e) => Self::count_spawns(e),
                    })
                    .sum();
                stmt_spawns + tail.as_ref().map_or(0, |e| Self::count_spawns(e))
            }
            Expr::Hole(_, _, _)
            | Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StrLit(_)
            | Expr::BoolLit(_)
            | Expr::Var(_)
            | Expr::CharLit(_)
            | Expr::Placeholder => 0,
            Expr::StructLit(_, fields) => fields.iter().map(|(_, e)| Self::count_spawns(e)).sum(),
            Expr::List(elems) => elems.iter().map(Self::count_spawns).sum(),
            Expr::TString(parts) => parts
                .iter()
                .map(|part| match part {
                    TStringPart::Literal(_) => 0,
                    TStringPart::Expr(e) => Self::count_spawns(e),
                })
                .sum(),
            Expr::FString(parts) => parts
                .iter()
                .map(|part| match part {
                    FStringPart::Literal(_) => 0,
                    FStringPart::Expr(e) => Self::count_spawns(e),
                })
                .sum(),
            Expr::ParallelScope { body, .. } => Self::count_spawns(body),
            Expr::Select(arms) => arms
                .iter()
                .map(|arm| match arm {
                    SelectArm::Recv { source, body, .. } => {
                        Self::count_spawns(source) + Self::count_spawns(body)
                    }
                    SelectArm::Timeout { duration, body } => {
                        Self::count_spawns(duration) + Self::count_spawns(body)
                    }
                })
                .sum(),
            Expr::Handle { body, handlers } => {
                Self::count_spawns(body)
                    + handlers
                        .iter()
                        .map(|binding| match binding {
                            HandleBinding::Use(handler_use) => handler_use
                                .payload
                                .iter()
                                .map(|(_, expr)| Self::count_spawns(expr))
                                .sum(),
                            HandleBinding::On(arm) => Self::count_spawns(&arm.body),
                        })
                        .sum::<usize>()
            }
            Expr::Perform { args, .. } => args.iter().map(|arg| Self::count_spawns(arg)).sum(),
            Expr::ChannelNew { buffer, .. } => Self::count_spawns(buffer),
            Expr::Return(None) => 0,
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
            let mut type_mapping = HashMap::new();
            let (param_tys, ret_ty) = match self.registry.fn_type_params.get(name).cloned() {
                Some(ref tp) if !tp.is_empty() => {
                    let (inst_params, inst_ret, mapping) =
                        self.instantiate_sig(tp, &param_tys, &ret_ty);
                    type_mapping = mapping;
                    (inst_params, inst_ret)
                }
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
            self.check_where_bounds(name, &type_mapping);
            self.check_cap_propagation(&callee_caps);
            if let Some(callee_errors) = self.registry.fn_errors.get(name).cloned() {
                self.check_error_propagation(&callee_errors);
            }
            return self.apply_subst(&ret_ty);
        }

        // Direct call by name: check module registry (prelude builtins)
        if let Expr::Var(name) = callee
            && let Some((param_tys, ret_ty, caps)) = self.lookup_module_function(name)
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
            self.check_cap_propagation(&caps);
            return ret_ty;
        }
        // Could be a variable holding a function

        // Method call: `obj.method(args)` — callee is FieldAccess
        // General case: check callee type
        let fn_ty = self.check_expr(callee);
        match fn_ty {
            Ty::Fn(param_tys, ret_ty, caps, errors) => {
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
                self.check_error_propagation(&errors);
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
                let _ = self.check_expr(expr);
            }
        }
    }

    // ── Module registry lookup ──────────────────────────────────────

    /// Look up an operation on an explicitly declared `effect`.
    ///
    /// This does not try to infer signatures for platform-provided runtime
    /// handlers whose effect interfaces have not been loaded as source yet.
    fn lookup_registered_effect_operation(
        &mut self,
        effect: &str,
        operation: &str,
    ) -> Option<(Vec<Ty>, Ty)> {
        let (type_params, methods) = self.registry.capabilities.get(effect).cloned()?;
        let Some((_name, param_tys, ret_ty)) =
            methods.into_iter().find(|(name, _, _)| name == operation)
        else {
            self.err(
                ErrorCode::C0002,
                format!("effect `{effect}` has no operation `{operation}`"),
            );
            return None;
        };

        if type_params.is_empty() {
            Some((param_tys, ret_ty))
        } else {
            let (params, ret, _) = self.instantiate_sig(&type_params, &param_tys, &ret_ty);
            Some((params, ret))
        }
    }

    /// Look up a function in the module registry (e.g. prelude builtins).
    /// Instantiates fresh type variables for each `Ty::Var` in the signature
    /// to avoid collisions with the checker's own variable counter.
    fn lookup_module_function(&mut self, name: &str) -> Option<(Vec<Ty>, Ty, CapSet)> {
        // First pass: find the signature (immutable borrow of module_registry)
        let found = self.module_registry.all_interfaces().find_map(|module| {
            module.functions.get(name).map(|(params, ret)| {
                (
                    params.clone(),
                    ret.clone(),
                    module.function_caps.get(name).cloned().unwrap_or_default(),
                )
            })
        });

        let (params, ret, caps) = found?;

        // Collect all Var IDs used in the signature
        let mut var_ids = std::collections::BTreeSet::new();
        for p in &params {
            Self::collect_vars(p, &mut var_ids);
        }
        Self::collect_vars(&ret, &mut var_ids);
        if var_ids.is_empty() {
            return Some((params, ret, caps));
        }
        // Map old IDs → fresh variables (mutable borrow of self)
        let mapping: std::collections::BTreeMap<u32, Ty> = var_ids
            .into_iter()
            .map(|id| (id, self.fresh_var()))
            .collect();
        let params = params
            .iter()
            .map(|t| Self::replace_vars(t, &mapping))
            .collect();
        let ret = Self::replace_vars(&ret, &mapping);
        Some((params, ret, caps))
    }

    /// Collect all `Ty::Var` IDs from a type.
    fn collect_vars(ty: &Ty, ids: &mut std::collections::BTreeSet<u32>) {
        ty.visit(&mut |t| {
            if let Ty::Var(id) = t {
                ids.insert(*id);
            }
        });
    }

    /// Replace `Ty::Var` IDs according to a mapping.
    fn replace_vars(ty: &Ty, mapping: &std::collections::BTreeMap<u32, Ty>) -> Ty {
        ty.fold_ref(&mut |t| match t {
            Ty::Var(id) => Some(mapping.get(id).cloned().unwrap_or_else(|| t.clone())),
            _ => None,
        })
    }

    // ── Type resolution ─────────────────────────────────────────────

    fn resolve_signature_type(
        &mut self,
        te: &TypeExpr,
        signature_holes: &mut HashMap<String, Ty>,
    ) -> Ty {
        match te {
            TypeExpr::Hole(Some(name)) => signature_holes
                .entry(name.clone())
                .or_insert_with(|| self.fresh_var())
                .clone(),
            TypeExpr::Hole(None) => self.fresh_var(),
            TypeExpr::Generic(name, args) => {
                let resolved = args
                    .iter()
                    .map(|a| self.resolve_signature_type(a, signature_holes))
                    .collect();
                Ty::App(name.clone(), resolved)
            }
            TypeExpr::Tuple(types) => {
                if types.is_empty() {
                    Ty::Unit
                } else {
                    Ty::Tuple(
                        types
                            .iter()
                            .map(|t| self.resolve_signature_type(t, signature_holes))
                            .collect(),
                    )
                }
            }
            TypeExpr::Function(params, ret, error_exprs) => {
                let ptys = params
                    .iter()
                    .map(|p| self.resolve_signature_type(p, signature_holes))
                    .collect();
                let errors: ErrorSet = error_exprs
                    .iter()
                    .filter_map(|te| {
                        if let TypeExpr::Named(n) = te {
                            Some(n.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                Ty::Fn(
                    ptys,
                    Box::new(self.resolve_signature_type(ret, signature_holes)),
                    CapSet::new(),
                    errors,
                )
            }
            TypeExpr::Refinement(base, var_name, pred_expr) => Ty::Refined(
                Box::new(self.resolve_signature_type(base, signature_holes)),
                var_name.clone(),
                pred_expr.clone(),
            ),
            TypeExpr::Record(fields) => Ty::Record(
                fields
                    .iter()
                    .map(|(name, te)| {
                        (
                            name.clone(),
                            self.resolve_signature_type(te, signature_holes),
                        )
                    })
                    .collect(),
            ),
            _ => self.resolve_type(te),
        }
    }

    pub fn resolve_type(&mut self, te: &TypeExpr) -> Ty {
        match te {
            TypeExpr::Named(name) => match name.as_str() {
                "Int" | "I32" => Ty::I32,
                "I8" => Ty::I8,
                "I16" => Ty::I16,
                "I64" => Ty::I64,
                "U8" => Ty::U8,
                "U16" => Ty::U16,
                "U32" => Ty::U32,
                "U64" => Ty::U64,
                "F32" => Ty::F32,
                "F64" => Ty::F64,
                "Bool" => Ty::Bool,
                "Str" => Ty::Str,
                "Char" => Ty::Char,
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
            TypeExpr::Hole(_) => self.fresh_var(),
            TypeExpr::Generic(name, args) => {
                let resolved: Vec<Ty> = args.iter().map(|a| self.resolve_type(a)).collect();
                Ty::App(name.clone(), resolved)
            }
            TypeExpr::Tuple(types) => {
                if types.is_empty() {
                    Ty::Unit
                } else {
                    Ty::Tuple(types.iter().map(|t| self.resolve_type(t)).collect())
                }
            }
            TypeExpr::Function(params, ret, error_exprs) => {
                let ptys: Vec<Ty> = params.iter().map(|p| self.resolve_type(p)).collect();
                let errors: ErrorSet = error_exprs
                    .iter()
                    .filter_map(|te| {
                        if let TypeExpr::Named(n) = te {
                            Some(n.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                Ty::Fn(
                    ptys,
                    Box::new(self.resolve_type(ret)),
                    CapSet::new(),
                    errors,
                )
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

    fn infer_hole_type_from_allows(&self, allow_list: &[String]) -> Option<Ty> {
        let mut inferred: Option<Ty> = None;

        for allowed_name in allow_list {
            let candidate = if let Some(ty) = self.env.lookup(allowed_name) {
                if let Ty::Fn(_, ret, _, _) = ty {
                    Some(ret.as_ref().clone())
                } else {
                    None
                }
            } else {
                self.registry
                    .functions
                    .get(allowed_name)
                    .map(|(_, ret_ty, _)| ret_ty.clone())
            };

            let Some(candidate_ty) = candidate.map(|t| self.apply_subst(&t)) else {
                continue;
            };
            if candidate_ty.is_error() || matches!(candidate_ty, Ty::Hole(_)) {
                continue;
            }

            match &inferred {
                Some(existing) if existing != &candidate_ty => return None,
                Some(_) => {}
                None => inferred = Some(candidate_ty),
            }
        }

        inferred
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
        ty.fold_ref(&mut |t| match t {
            Ty::Var(id) => {
                if let Some(resolved) = self.substitution.get(id) {
                    Some(self.apply_subst(resolved))
                } else {
                    Some(t.clone())
                }
            }
            _ => None,
        })
    }

    /// Check if type variable `id` occurs anywhere in `ty`.
    fn occurs_in(&self, id: u32, ty: &Ty) -> bool {
        let ty = self.apply_subst(ty);
        let mut found = false;
        ty.visit(&mut |t| {
            if let Ty::Var(vid) = t
                && *vid == id
            {
                found = true;
            }
        });
        found
    }

    /// Substitute type parameter names with fresh type variables in a type.
    fn instantiate_ty(&self, ty: &Ty, mapping: &HashMap<String, Ty>) -> Ty {
        ty.fold_ref(&mut |t| match t {
            Ty::Named(name) => mapping.get(name).cloned(),
            _ => None,
        })
    }

    fn instantiate_struct_fields(
        &mut self,
        name: &str,
        field_defs: &[(String, Ty)],
    ) -> (Vec<(String, Ty)>, Ty) {
        match self.registry.struct_type_params.get(name).cloned() {
            Some(type_params) if !type_params.is_empty() => {
                let field_tys: Vec<Ty> = field_defs.iter().map(|(_, ty)| ty.clone()).collect();
                let ret_ty = Ty::App(
                    name.to_string(),
                    type_params
                        .iter()
                        .map(|param| Ty::Named(param.clone()))
                        .collect(),
                );
                let (inst_field_tys, inst_ret_ty, _) =
                    self.instantiate_sig(&type_params, &field_tys, &ret_ty);
                let inst_fields = field_defs
                    .iter()
                    .map(|(field_name, _)| field_name.clone())
                    .zip(inst_field_tys)
                    .collect();
                (inst_fields, inst_ret_ty)
            }
            _ => (field_defs.to_vec(), Ty::Named(name.to_string())),
        }
    }

    fn apply_struct_args(
        &self,
        name: &str,
        field_defs: &[(String, Ty)],
        args: &[Ty],
    ) -> Option<Vec<(String, Ty)>> {
        let type_params = self.registry.struct_type_params.get(name)?;
        if type_params.len() != args.len() {
            return None;
        }
        let mapping: HashMap<String, Ty> = type_params
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        Some(
            field_defs
                .iter()
                .map(|(field_name, ty)| (field_name.clone(), self.instantiate_ty(ty, &mapping)))
                .collect(),
        )
    }

    fn struct_fields_for_type(
        &mut self,
        name: &str,
        field_defs: &[(String, Ty)],
        ty: &Ty,
    ) -> (Vec<(String, Ty)>, Ty) {
        if let Ty::App(actual_name, args) = ty
            && actual_name == name
            && let Some(fields) = self.apply_struct_args(name, field_defs, args)
        {
            return (fields, ty.clone());
        }
        self.instantiate_struct_fields(name, field_defs)
    }

    /// Create fresh type variables for each type parameter and substitute
    /// them into the function signature.
    fn instantiate_sig(
        &mut self,
        type_params: &[String],
        param_tys: &[Ty],
        ret_ty: &Ty,
    ) -> (Vec<Ty>, Ty, HashMap<String, Ty>) {
        let mapping: HashMap<String, Ty> = type_params
            .iter()
            .map(|name| (name.clone(), self.fresh_var()))
            .collect();
        let new_params: Vec<Ty> = param_tys
            .iter()
            .map(|t| self.instantiate_ty(t, &mapping))
            .collect();
        let new_ret = self.instantiate_ty(ret_ty, &mapping);
        (new_params, new_ret, mapping)
    }

    fn check_where_bounds(&mut self, fn_name: &str, type_mapping: &HashMap<String, Ty>) {
        let Some(constraints) = self.registry.fn_where_bounds.get(fn_name).cloned() else {
            return;
        };
        for (type_var, bound) in constraints {
            if !self.registry.capabilities.contains_key(&bound) {
                self.err(
                    ErrorCode::E0403,
                    format!("unknown trait bound `{bound}` in where clause of `{fn_name}`"),
                );
                continue;
            }
            let Some(instantiated) = type_mapping.get(&type_var) else {
                continue;
            };
            let resolved = self.apply_subst(instantiated);
            if self.has_unresolved_type_var(&resolved) {
                self.err(
                    ErrorCode::E0404,
                    format!(
                        "cannot infer type parameter `{type_var}` for where bound `{type_var}: {bound}` in `{fn_name}`"
                    ),
                );
                continue;
            }
            if !self.satisfies_trait_bound(&bound, &resolved) {
                self.err(
                    ErrorCode::E0403,
                    format!(
                        "type `{resolved}` does not satisfy where bound `{type_var}: {bound}` in `{fn_name}`"
                    ),
                );
            }
        }
    }

    fn has_unresolved_type_var(&self, ty: &Ty) -> bool {
        let mut found = false;
        ty.visit(&mut |t| {
            if matches!(t, Ty::Var(_)) {
                found = true;
            }
        });
        found
    }

    fn satisfies_trait_bound(&self, bound: &str, ty: &Ty) -> bool {
        self.bound_target_names(ty).into_iter().any(|target| {
            self.registry
                .impls
                .contains_key(&(bound.to_string(), target))
        })
    }

    fn bound_target_names(&self, ty: &Ty) -> Vec<String> {
        match ty {
            Ty::Refined(base, _, _) => self.bound_target_names(base),
            Ty::Named(name) | Ty::App(name, _) => vec![name.clone()],
            Ty::I8 => vec!["I8".into()],
            Ty::I16 => vec!["I16".into()],
            Ty::I32 => vec!["I32".into()],
            Ty::I64 => vec!["I64".into()],
            Ty::U8 => vec!["U8".into()],
            Ty::U16 => vec!["U16".into()],
            Ty::U32 => vec!["U32".into()],
            Ty::U64 => vec!["U64".into()],
            Ty::F32 => vec!["F32".into()],
            Ty::F64 => vec!["F64".into()],
            Ty::Bool => vec!["Bool".into()],
            Ty::Str => vec!["Str".into()],
            Ty::Char => vec!["Char".into()],
            Ty::Unit => vec!["Unit".into()],
            Ty::Never => vec!["Never".into()],
            _ => vec![],
        }
    }

    // ── Set propagation checks ─────────────────────────────────────

    /// Verify that the current function's capability set is a superset of the callee's.
    fn check_cap_propagation(&mut self, callee_caps: &CapSet) {
        let missing = find_missing_set_items(callee_caps, &self.current_caps);
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

    /// Verify that the current function's error set is a superset of the callee's.
    fn check_error_propagation(&mut self, callee_errors: &ErrorSet) {
        let missing = find_missing_set_items(callee_errors, &self.current_errors);
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

    fn check_throw_coverage(&mut self, thrown_expr: &Expr) {
        if self.current_errors.is_empty() {
            self.err(
                ErrorCode::E0012,
                format!(
                    "`throw` in `{}` requires declaring an error set with `! E`",
                    self.current_function
                ),
            );
            return;
        }

        let Some(thrown_name) = self.infer_thrown_error_name(thrown_expr) else {
            return;
        };
        if !self.current_errors.contains(&thrown_name) {
            self.err(
                ErrorCode::E0012,
                format!(
                    "thrown error `{thrown_name}` is not declared in `{}` error set",
                    self.current_function
                ),
            );
        }
    }

    fn infer_thrown_error_name(&self, expr: &Expr) -> Option<String> {
        fn looks_like_error_name(name: &str) -> bool {
            name.chars().next().is_some_and(char::is_uppercase)
        }

        match expr {
            Expr::Var(name) if looks_like_error_name(name) => Some(name.clone()),
            Expr::Call(callee, _) => match callee.as_ref() {
                Expr::Var(name) if looks_like_error_name(name) => Some(name.clone()),
                _ => None,
            },
            Expr::StructLit(name, _) if looks_like_error_name(name) => Some(name.clone()),
            _ => None,
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
        // Treat empty tuple as Unit (the parser produces Tuple([]) for `()`)
        if matches!((&e, &a), (Ty::Unit, Ty::Tuple(v)) | (Ty::Tuple(v), Ty::Unit) if v.is_empty()) {
            return;
        }
        if matches!(e, Ty::Hole(_)) || matches!(a, Ty::Hole(_)) {
            return;
        }

        // Never is a subtype of all types (actual can be Never for any expected)
        if matches!(a, Ty::Never) {
            return;
        }
        // But expected=Never means the context requires divergence; don't allow
        // arbitrary types to satisfy it.

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
            (Ty::Fn(p1, r1, _, _), Ty::Fn(p2, r2, _, _)) if p1.len() == p2.len() => {
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
                if !scrutinee_ty.is_integer() && !scrutinee_ty.is_error() {
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
                    let (def_fields, expected_ty) =
                        self.struct_fields_for_type(name, &def_fields, scrutinee_ty);
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
                if pats.is_empty() {
                    return vec![];
                }
                let first_bindings = self.check_pattern(&pats[0], scrutinee_ty);
                let first_names: std::collections::BTreeSet<&str> =
                    first_bindings.iter().map(|(n, _)| n.as_str()).collect();

                for pat in &pats[1..] {
                    let alt_bindings = self.check_pattern(pat, scrutinee_ty);
                    let alt_names: std::collections::BTreeSet<&str> =
                        alt_bindings.iter().map(|(n, _)| n.as_str()).collect();

                    if first_names != alt_names {
                        self.err(
                            ErrorCode::E0504,
                            format!(
                                "or-pattern alternatives must bind the same names: expected {first_names:?}, found {alt_names:?}",
                            ),
                        );
                    } else {
                        for ((name, ty1), (_, ty2)) in
                            first_bindings.iter().zip(alt_bindings.iter())
                        {
                            self.unify(
                                ty1,
                                ty2,
                                &format!(
                                    "or-pattern binding `{name}` type mismatch across alternatives"
                                ),
                            );
                        }
                    }
                }
                first_bindings
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
            Ty::I8
            | Ty::I16
            | Ty::I32
            | Ty::I64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::F32
            | Ty::F64
            | Ty::Str => {
                self.err(
                    ErrorCode::E0010,
                    format!(
                        "non-exhaustive match: `{}` requires a wildcard `_` or variable pattern",
                        scrutinee_ty
                    ),
                );
            }
            Ty::App(name, _args) => {
                // For parameterized types like Option[Int], look up the base type's variants
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
                                "non-exhaustive match on `{}`: missing variant(s) {}",
                                scrutinee_ty,
                                missing.join(", ")
                            ),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn err(&mut self, code: ErrorCode, message: String) {
        self.errors.push(TypeError::new(code, message));
    }

    fn err_at(&mut self, code: ErrorCode, message: String, span: Span) {
        self.errors.push(TypeError::with_span(code, message, span));
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
    fn find_suggestions(&self, expected: &Ty, allow_list: Option<&[String]>) -> Vec<String> {
        if expected.is_error() || matches!(expected, Ty::Hole(_)) {
            return Vec::new();
        }
        let mut suggestions: Vec<String> = self
            .registry
            .functions
            .iter()
            .filter(|(name, (_, ret_ty, _))| {
                ret_ty == expected
                    && *name != &self.current_function
                    && allow_list.is_none_or(|allowed| allowed.iter().any(|a| a == *name))
            })
            .map(|(name, _)| name.clone())
            .collect();
        suggestions.sort();
        suggestions
    }

    fn fresh_unnamed_hole_name(&mut self) -> String {
        let id = self.next_unnamed_hole_id;
        self.next_unnamed_hole_id += 1;
        format!("_hole{id}")
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
        ty.visit(&mut |t| {
            if let Ty::Var(id) = t {
                vars.insert(*id);
                // Follow substitution chains that visit() cannot see
                if let Some(resolved) = self.substitution.get(id) {
                    vars.extend(self.collect_type_vars(resolved));
                }
            }
        });
        vars
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
