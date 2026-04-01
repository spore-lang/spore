/// spore-typeck — Spore type checker and analysis
///
/// Performs type checking, capability verification, and cost analysis.
pub mod capability;
pub mod check;
pub mod concurrency;
pub mod cost;
pub mod env;
pub mod error;
pub mod hir;
pub mod hole;
pub mod incremental;
pub mod lower;
pub mod module;
pub mod platform;
pub mod sig_hash;
pub mod types;

use std::collections::HashMap;

use check::Checker;
use cost::{CostAnalyzer, CostChecker, CostResult, CostVector};
use error::TypeError;
use hole::HoleReport;
use module::ModuleRegistry;
use spore_parser::ast::Module;

/// Result of a successful type check, including hole report and cost analysis.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub hole_report: HoleReport,
    pub cost_results: HashMap<String, CostResult>,
    pub cost_vectors: HashMap<String, CostVector>,
}

/// Lower an AST module to HIR.
pub fn lower(module: &Module) -> hir::HirModule {
    let mut lowering = lower::Lowering::new();
    lowering.lower_module(module)
}

/// Type-check a parsed Spore module, returning a CheckResult or all errors found.
pub fn type_check(module: &Module) -> Result<CheckResult, Vec<TypeError>> {
    type_check_with_registry(module, ModuleRegistry::new())
}

/// Type-check a parsed Spore module with a shared module registry.
pub fn type_check_with_registry(
    module: &Module,
    registry: ModuleRegistry,
) -> Result<CheckResult, Vec<TypeError>> {
    let mut checker = Checker::with_module_registry(registry);
    checker.check_module(module);

    // Run cost analysis (independent of type checking)
    let mut cost_analyzer = CostAnalyzer::new();
    cost_analyzer.analyze_module(module);

    // Build four-dimensional cost vectors
    let mut cost_checker = CostChecker::new();
    cost_checker.check_all(&cost_analyzer);

    if checker.errors.is_empty() {
        Ok(CheckResult {
            hole_report: checker.hole_report,
            cost_results: cost_analyzer.results().clone(),
            cost_vectors: cost_checker.costs,
        })
    } else {
        Err(checker.errors)
    }
}

/// Build a `ModuleInterface` from a parsed module (for multi-file compilation).
pub fn build_module_interface(module: &Module) -> module::ModuleInterface {
    use module::{ModuleInterface, SymbolVisibility};
    use spore_parser::ast::Item;

    let path: Vec<String> = module.name.split('.').map(|s| s.to_string()).collect();
    let mut iface = ModuleInterface::new(path);

    for item in &module.items {
        match item {
            Item::Function(f) => {
                let checker = Checker::new();
                let param_tys: Vec<types::Ty> = f
                    .params
                    .iter()
                    .map(|p| checker.resolve_type(&p.ty))
                    .collect();
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(|t| checker.resolve_type(t))
                    .unwrap_or(types::Ty::Unit);
                iface.functions.insert(f.name.clone(), (param_tys, ret_ty));
                iface.set_visibility(&f.name, SymbolVisibility::from(&f.visibility));
            }
            Item::StructDef(s) => {
                let fields: Vec<String> = s.fields.iter().map(|f| f.name.clone()).collect();
                iface.structs.insert(s.name.clone(), fields);
                iface.set_visibility(&s.name, SymbolVisibility::from(&s.visibility));
            }
            Item::TypeDef(t) => {
                let variants: Vec<String> = t.variants.iter().map(|v| v.name.clone()).collect();
                iface.types.insert(t.name.clone(), variants);
                iface.set_visibility(&t.name, SymbolVisibility::from(&t.visibility));
            }
            Item::CapabilityDef(cap) => {
                iface.capabilities.insert(cap.name.clone());
                iface.set_visibility(&cap.name, SymbolVisibility::from(&cap.visibility));
            }
            _ => {}
        }
    }

    iface
}
