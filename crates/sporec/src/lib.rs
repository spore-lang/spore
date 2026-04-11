/// sporec — Spore language compiler
///
/// Stateless pure function: source → compiled output.
/// All IO is handled by the `spore` codebase manager.
pub mod compiler;
pub mod project;

pub use compiler::{
    CompileOutput, Diagnostic, DiagnosticSeverity, HoleSummary, check_project_verbose,
    check_verbose, compile, compile_diagnostics, compile_files, compile_project, format,
    hole_summary, holes, run, run_project, test_specs,
};
pub use project::{
    ProjectConfig, ProjectEntry, ProjectManifest, ResolvedProjectTarget, load_project_manifest,
    resolve_default_project_target, resolve_project_target_by_path,
};
pub use spore_codegen::{SpecKind, SpecResult};
