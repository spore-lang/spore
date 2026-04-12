/// sporec — Spore language compiler
///
/// Stateless pure function: source → compiled output.
/// All IO is handled by the `spore` codebase manager.
pub mod compiler;
pub mod diagnostics;
pub mod project;

pub use compiler::{
    CheckFailure, CheckReport, CompileOutput, Diagnostic as CompilerDiagnostic,
    DiagnosticSeverity as CompilerDiagnosticSeverity, check_files, check_project,
    check_project_verbose, check_verbose, compile,
    compile_diagnostics as compile_legacy_diagnostics, compile_files, compile_project, format,
    hole_summary, holes, holes_report, query_hole_report, run, run_project, test_specs,
};
pub use diagnostics::{
    SourceCheckFailure, SourceCheckReport, check_source_file, diagnostics_for_type_errors,
    source_file, type_error_to_diagnostic,
};
pub use project::{
    DependencySpec, PlatformManifest, ProjectConfig, ProjectEntry, ProjectManifest,
    ResolvedPlatformContract, ResolvedProjectTarget, load_project_manifest,
    resolve_default_project_target, resolve_project_target_by_path,
};
pub use spore_codegen::{SpecKind, SpecResult};
pub use sporec_diagnostics::{
    Diagnostic, DiagnosticRange, HoleCandidateJson, HoleCandidateRankingJson, HoleConfidenceJson,
    HoleCostBudgetJson, HoleDependencyEdgeJson, HoleDependencyGraphJson, HoleDependencyKind,
    HoleErrorClusterJson, HoleInfoJson, HoleLocationJson, HoleReportJson, HoleSummary,
    HoleTypeInferenceJson, JsonReport as DiagnosticJsonReport, LspDiagnostic,
    LspDiagnosticRelatedInformation, LspLocation, LspPosition, LspRange,
    Position as DiagnosticPosition, RelatedDiagnostic as DiagnosticRelated,
    RenderError as DiagnosticRenderError, ReportStatus as DiagnosticReportStatus,
    SecondaryLabel as DiagnosticSecondaryLabel, Severity as CanonicalSeverity, SourceFile,
    SourceSpan, lsp_diagnostic_for_source, lsp_diagnostics_for_source, render_diagnostic,
    render_diagnostic_to_string,
};
