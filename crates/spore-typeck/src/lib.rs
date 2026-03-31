/// spore-typeck — Spore type checker and analysis
///
/// Performs type checking, capability verification, and cost analysis.
pub mod check;
pub mod env;
pub mod error;
pub mod types;

use check::Checker;
use error::TypeError;
use spore_parser::ast::Module;

/// Type-check a parsed Spore module, returning all errors found.
pub fn type_check(module: &Module) -> Result<(), Vec<TypeError>> {
    let mut checker = Checker::new();
    checker.check_module(module);
    if checker.errors.is_empty() {
        Ok(())
    } else {
        Err(checker.errors)
    }
}
