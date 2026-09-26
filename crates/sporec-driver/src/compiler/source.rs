use sporec_codegen::value::Value;
use sporec_parser::formatter::format_module;
use sporec_parser::parse;
use sporec_typeck::type_check;

use super::{CompileOutput, Diagnostic, DiagnosticSeverity, format_verbose_result, join_errors};

/// Compile and return structured diagnostics (for LSP and IDE integration).
pub fn compile_diagnostics(source: &str) -> Vec<Diagnostic> {
    let ast = match parse(source) {
        Ok(ast) => ast,
        Err(errs) => {
            return errs
                .into_iter()
                .map(|e| Diagnostic {
                    message: e.message,
                    span: Some(e.span),
                    severity: DiagnosticSeverity::Error,
                })
                .collect();
        }
    };
    match type_check(&ast) {
        Ok(result) => result
            .warnings
            .iter()
            .map(|w| Diagnostic {
                message: w.message.clone(),
                span: w.span,
                severity: DiagnosticSeverity::Warning,
            })
            .collect(),
        Err(errs) => errs
            .into_iter()
            .map(|e| Diagnostic {
                message: format!("[{}] {}", e.code, e.message),
                span: e.span,
                severity: DiagnosticSeverity::Error,
            })
            .collect(),
    }
}

/// Compile Spore source code to output.
///
/// This is the core compiler pipeline:
/// 1. Parse (source text → AST)
/// 2. Type check (AST → Typed AST)
/// 3. Code gen (Typed AST → runtime-ready output)
///
/// Returns structured warnings emitted by verification passes on success.
pub fn compile(source: &str) -> Result<CompileOutput, String> {
    let ast = parse(source).map_err(join_errors)?;
    let result = type_check(&ast).map_err(join_errors)?;
    let warnings = result.warnings.iter().map(|w| w.to_string()).collect();
    Ok(CompileOutput { warnings })
}

/// Run a Spore program by executing its current default startup function
/// (`main`).
pub fn run(source: &str) -> Result<Value, String> {
    let ast = parse(source).map_err(join_errors)?;
    let _result = type_check(&ast).map_err(join_errors)?;
    sporec_codegen::run(&ast).map_err(|e| e.to_string())
}

/// Run a pure scalar Spore program through the experimental native backend.
pub fn run_native(source: &str) -> Result<Value, String> {
    let ast = parse(source).map_err(join_errors)?;
    let _result = type_check(&ast).map_err(join_errors)?;
    sporec_codegen::run_native(&ast).map_err(|e| e.to_string())
}

/// Compile a pure scalar Spore program into a native object artifact.
pub fn build_native_object(source: &str) -> Result<Vec<u8>, String> {
    let ast = parse(source).map_err(join_errors)?;
    let _result = type_check(&ast).map_err(join_errors)?;
    sporec_codegen::emit_native_object(&ast).map_err(|e| e.to_string())
}

/// Call a named pure scalar function through the experimental native backend.
pub fn call_native(source: &str, name: &str, args: Vec<Value>) -> Result<Value, String> {
    let ast = parse(source).map_err(join_errors)?;
    let _result = type_check(&ast).map_err(join_errors)?;
    sporec_codegen::call_native(&ast, name, args).map_err(|e| e.to_string())
}

/// Run source properties and return validation results.
pub fn test_properties(source: &str) -> Result<Vec<sporec_codegen::PropertyResult>, String> {
    let ast = parse(source).map_err(join_errors)?;
    let _ = type_check(&ast);
    sporec_codegen::test_properties(&ast).map_err(|e| e.to_string())
}

/// Format Spore source code.
///
/// Parses the source into an AST and then pretty-prints it back using the
/// canonical formatter. Returns the formatted source text.
pub fn format(source: &str) -> Result<String, String> {
    let ast = parse(source).map_err(join_errors)?;
    Ok(format_module(&ast))
}

/// Type-check with verbose output: returns detailed analysis including type
/// inference context, effects, holes, and intent-signature budgets.
pub fn check_verbose(source: &str) -> Result<String, String> {
    let ast = parse(source).map_err(join_errors)?;
    let result = type_check(&ast).map_err(|errs| {
        errs.into_iter()
            .map(|e| format!("  {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok(format_verbose_result(&result))
}
