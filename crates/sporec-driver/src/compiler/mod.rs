mod files;
mod hole_json;
mod project;
mod source;

pub use files::{check_files, compile_files};
pub use hole_json::{hole_summary, holes, holes_report, query_hole_report};
pub use project::{
    build_project_native_object, check_project, check_project_verbose, compile_project,
    run_project, run_project_with_outcome, test_specs_project,
};
pub use source::{
    build_native_object, call_native, check_verbose, compile, compile_diagnostics, format, run,
    run_native, test_specs,
};

use sporec_diagnostics::{Diagnostic as CanonicalDiagnostic, SourceFile};
use sporec_parser::ast::{Module, Span};
use sporec_typeck::{CheckResult, is_synthetic_hole_name};

/// Warnings collected during compilation (cost budget violations, etc.).
#[derive(Debug, Clone, Default)]
pub struct CompileOutput {
    pub warnings: Vec<String>,
}

/// A structured diagnostic with optional span information.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub span: Option<Span>,
    pub severity: DiagnosticSeverity,
}

/// Diagnostic severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub enum CheckReport {
    Success {
        sources: Vec<SourceFile>,
        warnings: Vec<CanonicalDiagnostic>,
    },
    Failure(CheckFailure),
}

#[derive(Debug, Clone)]
pub enum CheckFailure {
    Message(String),
    Diagnostics {
        sources: Vec<SourceFile>,
        diagnostics: Vec<CanonicalDiagnostic>,
    },
}

fn join_errors<E: std::fmt::Display>(errs: Vec<E>) -> String {
    errs.into_iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn with_module_name(ast: &Module, module_name: &str) -> Module {
    let mut ast = ast.clone();
    ast.name = module_name.to_string();
    ast
}

/// Summarise a successful CheckResult for --verbose output.
fn format_verbose_result(result: &CheckResult) -> String {
    let mut out = String::new();
    out.push_str("✓ no errors\n");

    out.push_str("\n── Type Inference ──\n");
    out.push_str(&format!(
        "  holes: {} total\n",
        result.hole_report.holes.len()
    ));
    for h in &result.hole_report.holes {
        let label = if is_synthetic_hole_name(&h.name) {
            "?".to_string()
        } else {
            format!("?{}", h.name)
        };
        out.push_str(&format!("    {label}: expected {}\n", h.expected_type));
    }

    if !result.cost_vectors.is_empty() {
        out.push_str("\n── Cost Analysis ──\n");
        for (fn_name, cv) in &result.cost_vectors {
            out.push_str(&format!("  {fn_name}: {cv}\n"));
        }
    }

    if !result.warnings.is_empty() {
        out.push_str("\n── Cost Warnings ──\n");
        for w in &result.warnings {
            out.push_str(&format!("  warning[{}]: {}\n", w.code, w.message));
        }
    }

    out
}
