/// spore-typeck — Spore type checker and analysis
///
/// Performs type checking, capability verification, and cost analysis.
pub mod check;
pub mod cost;
pub mod env;
pub mod error;
pub mod hole;
pub mod types;

use std::collections::HashMap;

use check::Checker;
use cost::{CostAnalyzer, CostResult};
use error::TypeError;
use hole::HoleReport;
use spore_parser::ast::Module;

/// Result of a successful type check, including hole report and cost analysis.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub hole_report: HoleReport,
    pub cost_results: HashMap<String, CostResult>,
}

/// Type-check a parsed Spore module, returning a CheckResult or all errors found.
pub fn type_check(module: &Module) -> Result<CheckResult, Vec<TypeError>> {
    let mut checker = Checker::new();
    checker.check_module(module);

    // Run cost analysis (independent of type checking)
    let mut cost_analyzer = CostAnalyzer::new();
    cost_analyzer.analyze_module(module);

    if checker.errors.is_empty() {
        Ok(CheckResult {
            hole_report: checker.hole_report,
            cost_results: cost_analyzer.results().clone(),
        })
    } else {
        Err(checker.errors)
    }
}
