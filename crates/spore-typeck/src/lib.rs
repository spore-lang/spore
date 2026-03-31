/// spore-typeck — Spore type checker and analysis
///
/// Performs type checking, capability verification, and cost analysis.
pub mod check;
pub mod env;
pub mod error;
pub mod hole;
pub mod types;

use check::Checker;
use error::TypeError;
use hole::HoleReport;
use spore_parser::ast::Module;

/// Result of a successful type check, including hole report.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub hole_report: HoleReport,
}

/// Type-check a parsed Spore module, returning a CheckResult or all errors found.
pub fn type_check(module: &Module) -> Result<CheckResult, Vec<TypeError>> {
    let mut checker = Checker::new();
    checker.check_module(module);
    if checker.errors.is_empty() {
        Ok(CheckResult {
            hole_report: checker.hole_report,
        })
    } else {
        Err(checker.errors)
    }
}
