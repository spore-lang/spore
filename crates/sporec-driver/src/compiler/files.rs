use std::path::{Path, PathBuf};

use crate::diagnostics::{diagnostics_for_parse_errors, diagnostics_for_type_errors, source_file};
use sporec_diagnostics::{Diagnostic as CanonicalDiagnostic, Severity, SourceFile};
use sporec_parser::ast::Span;
use sporec_typeck::module::{ModuleError, ModuleLoader, ModuleRegistry};
use sporec_typeck::type_check_with_registry;

use super::{CheckFailure, CheckReport, CompileOutput, with_module_name};

pub(super) fn push_source_if_missing(sources: &mut Vec<SourceFile>, source: &SourceFile) {
    if !sources
        .iter()
        .any(|existing| existing.name() == source.name())
    {
        sources.push(source.clone());
    }
}

pub(super) fn file_source(path: &str, contents: String) -> SourceFile {
    source_file(path.replace('\\', "/"), contents)
}

pub(super) fn batch_error_source() -> SourceFile {
    source_file("<batch>", "")
}

pub(super) fn module_error_source(module_path: &str, loader: &ModuleLoader) -> Option<SourceFile> {
    loader
        .get_source(module_path)
        .map(|source| source_file(source_label_for_module(module_path), source.to_string()))
}

pub(super) fn anchor_diagnostics_to_source(
    source: &SourceFile,
    diagnostics: Vec<CanonicalDiagnostic>,
) -> Vec<CanonicalDiagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            if diagnostic.primary_span.is_some() {
                diagnostic
            } else {
                diagnostic.with_primary_span(source.span(0..0))
            }
        })
        .collect()
}

pub(super) fn module_error_to_diagnostics(
    loader: &ModuleLoader,
    error: ModuleError,
    anchor: Option<(&SourceFile, Span)>,
) -> (SourceFile, Vec<CanonicalDiagnostic>) {
    match error {
        ModuleError::ParseErrors { module, errors } => {
            if let Some(source) = module_error_source(&module, loader) {
                let diagnostics = diagnostics_for_parse_errors(&source, &errors);
                (source, diagnostics)
            } else {
                let source = batch_error_source();
                let diagnostics = errors
                    .into_iter()
                    .map(|error| {
                        CanonicalDiagnostic::new(
                            "parse-error",
                            Severity::Error,
                            format!("parse error in module `{module}`: {}", error.message),
                        )
                    })
                    .collect();
                (source, diagnostics)
            }
        }
        other => {
            let code = match other {
                ModuleError::ModuleNotFound(_) => "module-not-found",
                ModuleError::SymbolNotFound { .. } => "import-symbol-not-found",
                ModuleError::PrivateSymbol { .. } => "private-symbol",
                ModuleError::CircularDependency(_) => "circular-module-dependency",
                ModuleError::IoError { .. } => "module-io-error",
                ModuleError::ParseErrors { .. } => unreachable!(),
            };
            let source = anchor
                .map(|(source, _)| source.clone())
                .unwrap_or_else(batch_error_source);
            let diagnostic = if let Some((source, span)) = anchor {
                CanonicalDiagnostic::new(code, Severity::Error, other.to_string())
                    .with_primary_span(source.span(span.start..span.end))
            } else {
                CanonicalDiagnostic::new(code, Severity::Error, other.to_string())
            };
            (source, vec![diagnostic])
        }
    }
}

pub fn check_files(paths: &[&str]) -> CheckReport {
    if paths.is_empty() {
        return CheckReport::Failure(CheckFailure::Message(
            "check_files requires at least one input file".to_string(),
        ));
    }

    let mut modules = Vec::new();
    let mut sources = Vec::new();
    let mut diagnostics = Vec::new();

    for path in paths {
        let source_text =
            match std::fs::read_to_string(path).map_err(|e| format!("cannot read `{path}`: {e}")) {
                Ok(source) => source,
                Err(message) => return CheckReport::Failure(CheckFailure::Message(message)),
            };
        let canonical_path = match std::fs::canonicalize(path)
            .map_err(|e| format!("cannot canonicalize `{path}`: {e}"))
        {
            Ok(canonical_path) => canonical_path,
            Err(message) => return CheckReport::Failure(CheckFailure::Message(message)),
        };

        let source = file_source(path, source_text.clone());
        push_source_if_missing(&mut sources, &source);

        match sporec_parser::parse(&source_text) {
            Ok(ast) => modules.push(((*path).to_string(), canonical_path, source, ast)),
            Err(errors) => diagnostics.extend(diagnostics_for_parse_errors(&source, &errors)),
        }
    }

    if !diagnostics.is_empty() {
        return CheckReport::Failure(CheckFailure::Diagnostics {
            sources,
            diagnostics,
        });
    }

    let common_root = match common_parent_dir(
        &modules
            .iter()
            .map(|(_, canonical_path, _, _)| canonical_path.clone())
            .collect::<Vec<_>>(),
    ) {
        Ok(common_root) => common_root,
        Err(message) => return CheckReport::Failure(CheckFailure::Message(message)),
    };

    let mut registry = ModuleRegistry::new();
    let modules = match modules
        .into_iter()
        .map(|(path, canonical_path, source, ast)| {
            let module_name = module_name_for_path(&common_root, &canonical_path)?;
            let mut iface = sporec_typeck::build_module_interface(&ast);
            iface.path = module_name
                .split('.')
                .map(|segment| segment.to_string())
                .collect();
            registry.register(iface);
            Ok((path, source, module_name, ast))
        })
        .collect::<Result<Vec<_>, String>>()
    {
        Ok(modules) => modules,
        Err(message) => return CheckReport::Failure(CheckFailure::Message(message)),
    };

    let mut warnings = Vec::new();
    let mut diagnostics = Vec::new();
    for (_, source, module_name, ast) in &modules {
        let ast = with_module_name(ast, module_name);
        match type_check_with_registry(&ast, registry.clone()) {
            Ok(result) => warnings.extend(anchor_diagnostics_to_source(
                source,
                diagnostics_for_type_errors(source, &result.warnings),
            )),
            Err(errors) => diagnostics.extend(anchor_diagnostics_to_source(
                source,
                diagnostics_for_type_errors(source, &errors),
            )),
        }
    }

    if diagnostics.is_empty() {
        CheckReport::Success { sources, warnings }
    } else {
        CheckReport::Failure(CheckFailure::Diagnostics {
            sources,
            diagnostics,
        })
    }
}

/// Compile multiple Spore source files together with shared module resolution.
///
/// 1. Parses each source into an AST
/// 2. Builds a ModuleRegistry from all modules
/// 3. Type-checks each module with access to the shared registry
///
/// Returns warnings on success.
pub fn compile_files(paths: &[&str]) -> Result<CompileOutput, String> {
    let mut modules = Vec::new();

    for path in paths {
        let source =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read `{path}`: {e}"))?;
        let canonical_path = std::fs::canonicalize(path)
            .map_err(|e| format!("cannot canonicalize `{path}`: {e}"))?;
        let ast = sporec_parser::parse(&source).map_err(|errs| {
            let msgs: Vec<String> = errs.into_iter().map(|e| e.to_string()).collect();
            format!("{path}: {}", msgs.join("\n"))
        })?;
        modules.push(((*path).to_string(), canonical_path, ast));
    }

    let common_root = common_parent_dir(
        &modules
            .iter()
            .map(|(_, canonical_path, _)| canonical_path.clone())
            .collect::<Vec<_>>(),
    )?;

    let mut registry = ModuleRegistry::new();
    let modules = modules
        .into_iter()
        .map(|(path, canonical_path, ast)| {
            let module_name = module_name_for_path(&common_root, &canonical_path)?;
            let mut iface = sporec_typeck::build_module_interface(&ast);
            iface.path = module_name
                .split('.')
                .map(|segment| segment.to_string())
                .collect();
            registry.register(iface);
            Ok((path, module_name, ast))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut all_errors = Vec::new();
    let mut all_warnings = Vec::new();
    for (path, module_name, ast) in &modules {
        let ast = with_module_name(ast, module_name);
        match type_check_with_registry(&ast, registry.clone()) {
            Ok(result) => {
                for w in &result.warnings {
                    all_warnings.push(format!("{path}: {w}"));
                }
            }
            Err(errs) => {
                for e in errs {
                    all_errors.push(format!("{path}: {e}"));
                }
            }
        }
    }

    if all_errors.is_empty() {
        Ok(CompileOutput {
            warnings: all_warnings,
        })
    } else {
        Err(all_errors.join("\n"))
    }
}

fn common_parent_dir(paths: &[PathBuf]) -> Result<PathBuf, String> {
    let first = paths
        .first()
        .ok_or_else(|| "compile_files requires at least one input file".to_string())?;
    let mut common = first
        .parent()
        .ok_or_else(|| {
            format!(
                "cannot determine parent directory for `{}`",
                first.display()
            )
        })?
        .to_path_buf();

    for path in paths.iter().skip(1) {
        while !path.starts_with(&common) {
            if !common.pop() {
                return Err(format!(
                    "cannot determine a common module root for `{}` and `{}`",
                    first.display(),
                    path.display()
                ));
            }
        }
    }

    Ok(common)
}

fn module_name_for_path(common_root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(common_root).map_err(|_| {
        format!(
            "`{}` is not under common module root `{}`",
            path.display(),
            common_root.display()
        )
    })?;
    let mut components = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let Some(last) = components.last_mut() else {
        return Err(format!(
            "cannot derive module name from `{}`",
            path.display()
        ));
    };
    if let Some(stripped) = last
        .strip_suffix(".spore")
        .or_else(|| last.strip_suffix(".sp"))
    {
        *last = stripped.to_string();
    }
    Ok(components.join("."))
}

fn source_label_for_module(module_path: &str) -> String {
    format!("{}.sp", module_path.replace('.', "/"))
}
