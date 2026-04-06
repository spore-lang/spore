/// sporec — Spore language compiler
///
/// Stateless pure function: source → compiled output.
/// All IO is handled by the `spore` codebase manager.
pub mod compiler;

pub use compiler::{
    CompileOutput, Diagnostic, DiagnosticSeverity, HoleSummary, check_project_verbose,
    check_verbose, compile, compile_diagnostics, compile_files, compile_project, format,
    hole_summary, holes, run, run_project, test_specs,
};
pub use spore_codegen::{SpecKind, SpecResult};
