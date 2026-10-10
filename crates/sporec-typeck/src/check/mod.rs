//! Core type-checking logic.
//!
//! Walks the AST and verifies type consistency, building up a type
//! environment as it goes. Reports all errors (does not bail on first).

mod diag;
mod expr;
mod hole;
mod import;
mod items;
mod pattern;
mod propagation;
mod types_infer;
mod unify;

use sporec_parser::ast::*;

use crate::concurrency::ConcurrencyAnalyzer;
use crate::effect_set::EffectHierarchy;
use crate::env::{Env, HandlerInfo, InstantiatedMethod, MethodInfo, TypeRegistry};
use crate::error::{ErrorCode, TypeError};
use crate::hole::{HoleDependencyGraph, HoleInfo, HoleReport};
use crate::module::{ImportedSymbol, ModuleError, ModuleRegistry};
use crate::types::{EffectSet, Ty};

use std::collections::{HashMap, HashSet};

/// Return items present in `callee_set` but absent from `current_set`.
fn find_missing_set_items<'a>(callee_set: &'a EffectSet, current_set: &EffectSet) -> Vec<&'a str> {
    callee_set
        .iter()
        .filter(|item| !current_set.contains(item))
        .map(|s| s.as_str())
        .collect()
}

fn handler_self_type_name(name: &str) -> String {
    format!("__handler::{name}")
}

fn registry_with_builtin_effects() -> TypeRegistry {
    let mut registry = TypeRegistry::default();
    registry.effects.extend(
        crate::platform::BUILTIN_EFFECTS
            .iter()
            .map(|effect| (*effect).to_string()),
    );
    registry
}

#[derive(Clone)]
pub(super) struct EnclosingHandlerEffectContext {
    pub surviving_effects: EffectSet,
    pub discharged_effects: EffectSet,
}

pub struct Checker {
    pub errors: Vec<TypeError>,
    pub registry: TypeRegistry,
    pub hole_report: HoleReport,
    pub module_registry: ModuleRegistry,
    env: Env,
    /// Required effects of the function currently being checked.
    current_effects: EffectSet,
    /// Failure type of the enclosing outcome boundary, when present.
    current_outcome_failure: Option<Ty>,
    /// Name of the function currently being checked.
    current_function: String,
    /// Name of the module currently being checked.
    current_module_name: String,
    /// Declared return type of the current function (for hole inference).
    expected_return_type: Option<Ty>,
    /// Next type variable ID for fresh type variables.
    next_var_id: u32,
    /// Next synthetic name ID for unnamed holes (`?`).
    next_unnamed_hole_id: u32,
    /// Substitution map: type variable ID → resolved type.
    substitution: HashMap<u32, Ty>,
    /// Explicit named surfaces used to expand `uses` clauses to atomic effects.
    hierarchy: EffectHierarchy,
    /// Structured concurrency analyzer (parallel scopes + spawn sites).
    concurrency: ConcurrencyAnalyzer,
    /// Nested effect observations used for handle discharge and leak diagnostics.
    effect_observation_stack: Vec<EffectSet>,
    /// Enclosing handler discharge context used to enrich hole reports.
    hole_effect_context_stack: Vec<EnclosingHandlerEffectContext>,
}

impl Checker {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            registry: registry_with_builtin_effects(),
            hole_report: HoleReport::new(),
            module_registry: ModuleRegistry::new(),
            env: Env::new(),
            current_effects: EffectSet::new(),
            current_outcome_failure: None,
            current_function: String::new(),
            current_module_name: String::new(),
            expected_return_type: None,
            next_var_id: 0,
            next_unnamed_hole_id: 0,
            substitution: HashMap::new(),
            hierarchy: EffectHierarchy::new(),
            concurrency: ConcurrencyAnalyzer::new(),
            effect_observation_stack: Vec::new(),
            hole_effect_context_stack: Vec::new(),
        }
    }

    /// Create a new Checker with an existing module registry.
    pub fn with_module_registry(module_registry: ModuleRegistry) -> Self {
        Self {
            errors: Vec::new(),
            registry: registry_with_builtin_effects(),
            hole_report: HoleReport::new(),
            module_registry,
            env: Env::new(),
            current_effects: EffectSet::new(),
            current_outcome_failure: None,
            current_function: String::new(),
            current_module_name: String::new(),
            expected_return_type: None,
            next_var_id: 0,
            next_unnamed_hole_id: 0,
            substitution: HashMap::new(),
            hierarchy: EffectHierarchy::new(),
            concurrency: ConcurrencyAnalyzer::new(),
            effect_observation_stack: Vec::new(),
            hole_effect_context_stack: Vec::new(),
        }
    }

    pub(super) fn push_effect_observer(&mut self) {
        self.effect_observation_stack.push(EffectSet::new());
    }

    pub(super) fn pop_effect_observer(&mut self) -> EffectSet {
        self.effect_observation_stack.pop().unwrap_or_default()
    }

    pub(super) fn observe_effects(&mut self, effects: &EffectSet) {
        if let Some(current) = self.effect_observation_stack.pop() {
            self.effect_observation_stack.push(current.union(effects));
        }
    }

    pub(super) fn observe_effect(&mut self, effect: impl Into<String>) {
        let mut set = EffectSet::new();
        set.insert(effect.into());
        self.observe_effects(&set);
    }

    /// Type-check an entire module.
    pub fn check_module(&mut self, module: &Module) {
        self.current_module_name = module.name.clone();
        // Surface declarations are order-independent and must be available
        // before function signatures are normalized.
        for item in &module.items {
            if matches!(item, Item::SurfaceDef(_)) {
                self.register_item(item);
            }
        }
        self.register_aliases(module);
        // Register declarations that establish the local symbol environment.
        // Function and handler signatures are delayed until imports are
        // available because their `uses` clauses may name imported surfaces.
        for item in &module.items {
            if !matches!(
                item,
                Item::SurfaceDef(_)
                    | Item::Import(_)
                    | Item::Alias(_)
                    | Item::Function(_)
                    | Item::ImplDef(_)
                    | Item::HandlerDef(_)
            ) {
                self.register_item(item);
            }
        }
        // Resolve imports before normalizing callable effect surfaces.
        for item in &module.items {
            if let Item::Import(import) = item {
                self.resolve_import(import);
            }
        }
        for item in &module.items {
            if matches!(
                item,
                Item::Function(_) | Item::ImplDef(_) | Item::HandlerDef(_)
            ) {
                self.register_item(item);
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
            if matches!(item, Item::SurfaceDef(_)) {
                self.register_item(item);
            }
        }
        self.register_aliases(module);
        for item in &module.items {
            if !matches!(item, Item::SurfaceDef(_) | Item::Alias(_)) {
                self.register_item(item);
            }
        }
    }

    fn register_aliases(&mut self, module: &Module) {
        let aliases = module
            .items
            .iter()
            .filter(|item| matches!(item, Item::Alias(_)))
            .collect::<Vec<_>>();
        for _ in 0..aliases.len() {
            for item in &aliases {
                self.register_item(item);
            }
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
