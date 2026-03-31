/// spore-typeck — Spore type checker and analysis
///
/// Performs type checking, capability verification, and cost analysis.
pub mod check;
pub mod cost;
pub mod env;
pub mod error;
pub mod hir;
pub mod hole;
pub mod incremental;
pub mod lower;
pub mod module;
pub mod sig_hash;
pub mod types;

use std::collections::HashMap;

use check::Checker;
use cost::{CostAnalyzer, CostChecker, CostResult, CostVector};
use error::TypeError;
use hole::HoleReport;
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
    let mut checker = Checker::new();
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
