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
use crate::effect_set::{EffectHierarchy, default_effect_hierarchy};
use crate::env::{Env, HandlerInfo, TypeRegistry};
use crate::error::{ErrorCode, TypeError};
use crate::hole::{HoleDependencyGraph, HoleInfo, HoleReport};
use crate::module::{ImportedSymbol, ModuleError, ModuleRegistry};
use crate::types::{EffectSet, ErrorSet, Ty};

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

pub struct Checker {
    pub errors: Vec<TypeError>,
    pub registry: TypeRegistry,
    pub hole_report: HoleReport,
    pub module_registry: ModuleRegistry,
    env: Env,
    /// Required effects of the function currently being checked.
    current_effects: EffectSet,
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
    /// Effect hierarchy for expanding parent effects (e.g. IO → 4 leaves).
    hierarchy: EffectHierarchy,
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
            current_effects: EffectSet::new(),
            current_errors: ErrorSet::new(),
            current_function: String::new(),
            current_module_name: String::new(),
            expected_return_type: None,
            current_hole_allows: None,
            next_var_id: 0,
            next_unnamed_hole_id: 0,
            substitution: HashMap::new(),
            hierarchy: default_effect_hierarchy(),
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
            current_effects: EffectSet::new(),
            current_errors: ErrorSet::new(),
            current_function: String::new(),
            current_module_name: String::new(),
            expected_return_type: None,
            current_hole_allows: None,
            next_var_id: 0,
            next_unnamed_hole_id: 0,
            substitution: HashMap::new(),
            hierarchy: default_effect_hierarchy(),
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
