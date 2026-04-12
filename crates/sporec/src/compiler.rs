use std::path::{Path, PathBuf};

use crate::diagnostics::{diagnostics_for_parse_errors, diagnostics_for_type_errors, source_file};
use crate::project::{ResolvedProjectTarget, resolve_project_target_by_path};
use spore_codegen::value::Value;
use spore_parser::ast::{ImportDecl, Item, Span};
use spore_parser::formatter::format_module;
use spore_parser::parse;
use spore_typeck::CheckResult;
use spore_typeck::is_synthetic_hole_name;
use spore_typeck::module::{ModuleError, ModuleInterface, ModuleLoader, ModuleRegistry};
use spore_typeck::platform::{PlatformRegistry, PlatformStartupError, PlatformStartupErrorKind};
use spore_typeck::{type_check, type_check_with_registry};
use sporec_diagnostics::{Diagnostic as CanonicalDiagnostic, Severity, SourceFile};

fn join_errors<E: std::fmt::Display>(errs: Vec<E>) -> String {
    errs.into_iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

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
/// 3. Code gen (Typed AST → native code)
///
/// Returns warnings (e.g. cost budget violations) on success.
pub fn compile(source: &str) -> Result<CompileOutput, String> {
    let ast = parse(source).map_err(join_errors)?;
    let result = type_check(&ast).map_err(join_errors)?;
    let warnings = result.warnings.iter().map(|w| w.to_string()).collect();
    Ok(CompileOutput { warnings })
}

fn push_source_if_missing(sources: &mut Vec<SourceFile>, source: &SourceFile) {
    if !sources
        .iter()
        .any(|existing| existing.name() == source.name())
    {
        sources.push(source.clone());
    }
}

fn file_source(path: &str, contents: String) -> SourceFile {
    source_file(path.replace('\\', "/"), contents)
}

fn batch_error_source() -> SourceFile {
    source_file("<batch>", "")
}

fn module_error_source(module_path: &str, loader: &ModuleLoader) -> Option<SourceFile> {
    loader
        .get_source(module_path)
        .map(|source| source_file(source_label_for_module(module_path), source.to_string()))
}

fn anchor_diagnostics_to_source(
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

fn module_error_to_diagnostics(
    loader: &ModuleLoader,
    error: ModuleError,
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
            let source = batch_error_source();
            let diagnostic = CanonicalDiagnostic::new(code, Severity::Error, other.to_string());
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

        match parse(&source_text) {
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
            let mut iface = spore_typeck::build_module_interface(&ast);
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

    // Phase 1: Parse all files
    for path in paths {
        let source =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read `{path}`: {e}"))?;
        let canonical_path = std::fs::canonicalize(path)
            .map_err(|e| format!("cannot canonicalize `{path}`: {e}"))?;
        let ast = parse(&source).map_err(|errs| {
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

    // Phase 2: Build ModuleRegistry from all modules
    let mut registry = ModuleRegistry::new();
    let modules = modules
        .into_iter()
        .map(|(path, canonical_path, ast)| {
            let module_name = module_name_for_path(&common_root, &canonical_path)?;
            let mut iface = spore_typeck::build_module_interface(&ast);
            iface.path = module_name
                .split('.')
                .map(|segment| segment.to_string())
                .collect();
            registry.register(iface);
            Ok((path, module_name, ast))
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Phase 3: Type-check each module with the shared registry
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

/// Intermediate state after parsing and resolving a project entry module.
///
/// Shared setup for [`compile_project`] and [`run_project`].
struct PreparedProject {
    ast: spore_parser::ast::Module,
    entry_source: String,
    entry_interface: ModuleInterface,
    registry: ModuleRegistry,
    loader: ModuleLoader,
}

/// Parse the selected entry module file, build a module registry, and resolve imports.
fn prepare_project(root: &Path, entry: &str) -> Result<PreparedProject, String> {
    let mut loader = ModuleLoader::new(root.to_path_buf());

    // Parse the selected entry module file.
    let entry_path = root.join("src").join(entry);
    let source = std::fs::read_to_string(&entry_path)
        .map_err(|e| format!("cannot read `{}`: {e}", entry_path.display()))?;
    let ast = parse(&source).map_err(join_errors)?;

    // Module names are derived from file paths.
    let module_name = entry.trim_end_matches(".sp").replace(['/', '\\'], ".");

    // Build registry and register the entry module
    let mut registry = ModuleRegistry::new();
    let mut entry_iface = spore_typeck::build_module_interface(&ast);
    entry_iface.path = module_name.split('.').map(|s| s.to_string()).collect();
    registry.register(entry_iface.clone());

    // Extract and resolve imports
    let imports: Vec<ImportDecl> = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Import(d) => Some(d.clone()),
            _ => None,
        })
        .collect();

    if !imports.is_empty() {
        registry
            .resolve_imports(&mut loader, &module_name, &imports)
            .map_err(join_errors)?;
    }

    Ok(PreparedProject {
        ast,
        entry_source: source,
        entry_interface: entry_iface,
        registry,
        loader,
    })
}

fn prepare_project_for_report(root: &Path, entry: &str) -> Result<PreparedProject, CheckFailure> {
    let mut loader = ModuleLoader::new(root.to_path_buf());

    let entry_path = root.join("src").join(entry);
    let source = match std::fs::read_to_string(&entry_path)
        .map_err(|e| format!("cannot read `{}`: {e}", entry_path.display()))
    {
        Ok(source) => source,
        Err(message) => return Err(CheckFailure::Message(message)),
    };
    let entry_source = file_source(entry, source.clone());
    let ast = match parse(&source) {
        Ok(ast) => ast,
        Err(errors) => {
            return Err(CheckFailure::Diagnostics {
                sources: vec![entry_source.clone()],
                diagnostics: diagnostics_for_parse_errors(&entry_source, &errors),
            });
        }
    };

    let module_name = entry.trim_end_matches(".sp").replace(['/', '\\'], ".");

    let mut registry = ModuleRegistry::new();
    let mut entry_iface = spore_typeck::build_module_interface(&ast);
    entry_iface.path = module_name.split('.').map(|s| s.to_string()).collect();
    registry.register(entry_iface.clone());

    let imports: Vec<ImportDecl> = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Import(d) => Some(d.clone()),
            _ => None,
        })
        .collect();

    if !imports.is_empty()
        && let Err(errors) = registry.resolve_imports(&mut loader, &module_name, &imports)
    {
        let mut sources = vec![entry_source.clone()];
        let mut diagnostics = Vec::new();
        for error in errors {
            let (source, module_diagnostics) = module_error_to_diagnostics(&loader, error);
            push_source_if_missing(&mut sources, &source);
            diagnostics.extend(module_diagnostics);
        }
        return Err(CheckFailure::Diagnostics {
            sources,
            diagnostics,
        });
    }

    Ok(PreparedProject {
        ast,
        entry_source: source,
        entry_interface: entry_iface,
        registry,
        loader,
    })
}

fn entry_module_name(entry: &str) -> String {
    entry.trim_end_matches(".sp").replace(['/', '\\'], ".")
}

fn source_label_for_module(module_path: &str) -> String {
    format!("{}.sp", module_path.replace('.', "/"))
}

fn with_module_name(
    ast: &spore_parser::ast::Module,
    module_name: &str,
) -> spore_parser::ast::Module {
    let mut ast = ast.clone();
    ast.name = module_name.to_string();
    ast
}

fn collect_prepared_project_results(
    prep: &PreparedProject,
    entry: &str,
) -> Result<Vec<(String, CheckResult)>, String> {
    let mut all_errors = Vec::new();
    let mut results = Vec::new();

    let mut loaded_modules = prep.loader.loaded_modules();
    loaded_modules.sort();

    for module_path in loaded_modules {
        let Some(ast) = prep.loader.get_ast(&module_path) else {
            continue;
        };
        let ast = with_module_name(ast, &module_path);
        let label = source_label_for_module(&module_path);
        match type_check_with_registry(&ast, prep.registry.clone()) {
            Ok(result) => results.push((label, result)),
            Err(errs) => {
                for err in errs {
                    all_errors.push(format!("{label}: {err}"));
                }
            }
        }
    }

    let entry_label = entry.replace('\\', "/");
    let entry_name = entry_module_name(entry);
    let entry_ast = with_module_name(&prep.ast, &entry_name);
    match type_check_with_registry(&entry_ast, prep.registry.clone()) {
        Ok(result) => results.push((entry_label, result)),
        Err(errs) => {
            for err in errs {
                all_errors.push(format!("{entry_label}: {err}"));
            }
        }
    }

    if all_errors.is_empty() {
        Ok(results)
    } else {
        Err(all_errors.join("\n"))
    }
}

fn collect_prepared_project_diagnostics(
    prep: &PreparedProject,
    entry: &str,
) -> (
    Vec<SourceFile>,
    Vec<CanonicalDiagnostic>,
    Vec<CanonicalDiagnostic>,
) {
    let mut sources = Vec::new();
    let mut warnings = Vec::new();
    let mut diagnostics = Vec::new();

    let mut loaded_modules = prep.loader.loaded_modules();
    loaded_modules.sort();

    for module_path in loaded_modules {
        let Some(ast) = prep.loader.get_ast(&module_path) else {
            continue;
        };
        let source =
            module_error_source(&module_path, &prep.loader).unwrap_or_else(batch_error_source);
        push_source_if_missing(&mut sources, &source);
        let ast = with_module_name(ast, &module_path);
        match type_check_with_registry(&ast, prep.registry.clone()) {
            Ok(result) => warnings.extend(anchor_diagnostics_to_source(
                &source,
                diagnostics_for_type_errors(&source, &result.warnings),
            )),
            Err(errors) => diagnostics.extend(anchor_diagnostics_to_source(
                &source,
                diagnostics_for_type_errors(&source, &errors),
            )),
        }
    }

    let entry_source = file_source(entry, prep.entry_source.clone());
    push_source_if_missing(&mut sources, &entry_source);
    let entry_name = entry_module_name(entry);
    let entry_ast = with_module_name(&prep.ast, &entry_name);
    match type_check_with_registry(&entry_ast, prep.registry.clone()) {
        Ok(result) => warnings.extend(anchor_diagnostics_to_source(
            &entry_source,
            diagnostics_for_type_errors(&entry_source, &result.warnings),
        )),
        Err(errors) => diagnostics.extend(anchor_diagnostics_to_source(
            &entry_source,
            diagnostics_for_type_errors(&entry_source, &errors),
        )),
    }

    (sources, warnings, diagnostics)
}

fn startup_error_to_diagnostic(
    source: &SourceFile,
    error: PlatformStartupError,
) -> CanonicalDiagnostic {
    let code = match error.kind {
        PlatformStartupErrorKind::MissingStartupFunction => "missing-startup-function",
        PlatformStartupErrorKind::WrongStartupSignature => "wrong-startup-signature",
    };
    CanonicalDiagnostic::new(code, Severity::Error, error.message)
        .with_primary_span(source.span(0..0))
}

fn validate_project_startup_error(
    prep: &PreparedProject,
    target: &ResolvedProjectTarget,
) -> Result<(), PlatformStartupError> {
    let Some(platform_name) = target.platform_name.as_deref() else {
        return Ok(());
    };

    let registry = PlatformRegistry::with_builtins();
    let platform = registry
        .get(platform_name)
        .ok_or_else(|| PlatformStartupError {
            kind: PlatformStartupErrorKind::MissingStartupFunction,
            message: format!(
                "unknown platform `{platform_name}` while validating entry path `{}`",
                target.entry_path
            ),
        })?;

    platform.validate_entry_startup(&prep.entry_interface)
}

fn validate_project_startup(
    prep: &PreparedProject,
    target: &ResolvedProjectTarget,
) -> Result<(), String> {
    validate_project_startup_error(prep, target).map_err(|err| err.message)
}

pub fn check_project(root: &Path, entry: &str) -> CheckReport {
    let target = match resolve_project_target_by_path(root, entry) {
        Ok(target) => target,
        Err(message) => return CheckReport::Failure(CheckFailure::Message(message)),
    };
    let prep = match prepare_project_for_report(root, &target.entry_path) {
        Ok(prep) => prep,
        Err(failure) => return CheckReport::Failure(failure),
    };
    let (mut sources, warnings, diagnostics) =
        collect_prepared_project_diagnostics(&prep, &target.entry_path);
    if !diagnostics.is_empty() {
        return CheckReport::Failure(CheckFailure::Diagnostics {
            sources,
            diagnostics,
        });
    }
    if let Err(error) = validate_project_startup_error(&prep, &target) {
        let entry_source = file_source(&target.entry_path, prep.entry_source.clone());
        push_source_if_missing(&mut sources, &entry_source);
        return CheckReport::Failure(CheckFailure::Diagnostics {
            sources,
            diagnostics: vec![startup_error_to_diagnostic(&entry_source, error)],
        });
    }

    CheckReport::Success { sources, warnings }
}

/// Compile a Spore project rooted at `root`, starting from `entry`.
///
/// 1. Creates a [`ModuleLoader`] from the project root
/// 2. Parses the entry module file at `{root}/src/{entry}`
/// 3. Recursively resolves all imports from disk
/// 4. Type-checks with a shared [`ModuleRegistry`]
///
/// Single-file projects (no imports) work without a `ModuleLoader`.
pub fn compile_project(root: &Path, entry: &str) -> Result<CompileOutput, String> {
    let target = resolve_project_target_by_path(root, entry)?;
    let prep = prepare_project(root, &target.entry_path)?;
    let results = collect_prepared_project_results(&prep, &target.entry_path)?;
    validate_project_startup(&prep, &target)?;
    let warnings = results
        .into_iter()
        .flat_map(|(label, result)| {
            result
                .warnings
                .into_iter()
                .map(move |warning| format!("{label}: {warning}"))
        })
        .collect();
    Ok(CompileOutput { warnings })
}

/// Run a Spore project by compiling and executing its resolved startup function.
///
/// Like [`compile_project`], but also invokes the interpreter with
/// cross-module function resolution.
pub fn run_project(root: &Path, entry: &str) -> Result<Value, String> {
    let target = resolve_project_target_by_path(root, entry)?;
    let startup_function = target.startup_function.as_deref().ok_or_else(|| {
        format!(
            "entry path `{}` is not runnable: no platform startup contract is bound",
            target.entry_path
        )
    })?;
    let prep = prepare_project(root, &target.entry_path)?;

    // Type-check
    let _results = collect_prepared_project_results(&prep, &target.entry_path)?;
    validate_project_startup(&prep, &target)?;

    // Collect imported module ASTs for the interpreter
    let mut imported_paths = prep.loader.loaded_modules();
    imported_paths.sort();
    let imported: Vec<(String, spore_parser::ast::Module)> = imported_paths
        .into_iter()
        .filter_map(|path| prep.loader.get_ast(&path).map(|ast| (path, ast.clone())))
        .collect();

    spore_codegen::run_project(&prep.ast, &imported, startup_function).map_err(|e| e.to_string())
}

/// Analyze holes in Spore source and return a JSON report.
pub fn holes(source: &str) -> Result<String, String> {
    let ast = parse(source).map_err(join_errors)?;
    let result = type_check(&ast).map_err(join_errors)?;
    Ok(result.hole_report.to_json())
}

/// Run a Spore program by executing its current default startup function
/// (`main`).
pub fn run(source: &str) -> Result<Value, String> {
    let ast = parse(source).map_err(join_errors)?;
    let _result = type_check(&ast).map_err(join_errors)?;
    spore_codegen::run(&ast).map_err(|e| e.to_string())
}

/// Run spec clauses in source code and return test results.
pub fn test_specs(source: &str) -> Result<Vec<spore_codegen::SpecResult>, String> {
    let ast = parse(source).map_err(join_errors)?;
    // Type-check errors are non-fatal for spec evaluation — the type checker
    // currently has known limitations with generics (Option[T], Pair[K,V])
    // that would block spec testing of otherwise valid code.
    let _ = type_check(&ast);
    spore_codegen::test_specs(&ast).map_err(|e| e.to_string())
}

/// Format Spore source code.
///
/// Parses the source into an AST and then pretty-prints it back using the
/// canonical formatter.  Returns the formatted source text.
pub fn format(source: &str) -> Result<String, String> {
    let ast = parse(source).map_err(join_errors)?;
    Ok(format_module(&ast))
}

/// Type-check with verbose output: returns detailed analysis including type
/// inference context, capability annotations, and cost summaries.
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

/// Type-check a Spore project with verbose per-module output.
pub fn check_project_verbose(root: &Path, entry: &str) -> Result<String, String> {
    let target = resolve_project_target_by_path(root, entry)?;
    let prep = prepare_project(root, &target.entry_path)?;
    let results = collect_prepared_project_results(&prep, &target.entry_path)?;
    validate_project_startup(&prep, &target)?;
    Ok(format_project_verbose_results(&results))
}

/// Summarise a successful CheckResult for --verbose output.
fn format_verbose_result(result: &CheckResult) -> String {
    let mut out = String::new();
    out.push_str("✓ no errors\n");

    // Type inference summary
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

    // Cost analysis
    if !result.cost_vectors.is_empty() {
        out.push_str("\n── Cost Analysis ──\n");
        for (fn_name, cv) in &result.cost_vectors {
            out.push_str(&format!("  {fn_name}: {cv}\n"));
        }
    }

    // Cost warnings
    if !result.warnings.is_empty() {
        out.push_str("\n── Cost Warnings ──\n");
        for w in &result.warnings {
            out.push_str(&format!("  warning[{}]: {}\n", w.code, w.message));
        }
    }

    out
}

fn format_project_verbose_results(results: &[(String, CheckResult)]) -> String {
    if results.len() == 1 {
        return format_verbose_result(&results[0].1);
    }

    let mut out = String::from("✓ no errors\n");
    for (label, result) in results {
        out.push_str(&format!("\n── {label} ──"));
        let detail = format_verbose_result(result);
        if let Some(detail) = detail.strip_prefix("✓ no errors") {
            out.push_str(detail);
        } else {
            out.push('\n');
            out.push_str(&detail);
        }
    }
    out
}

/// Return a hole graph summary suitable for NDJSON watch events.
pub fn hole_summary(source: &str) -> Option<HoleSummary> {
    let ast = parse(source).ok()?;
    let result = type_check(&ast).ok()?;
    let report = &result.hole_report;
    let graph = &report.dependency_graph;

    let holes_total = report.holes.len();
    if holes_total == 0 {
        return None;
    }

    let ready_to_fill = graph.roots().len();
    let blocked = holes_total.saturating_sub(ready_to_fill);

    Some(HoleSummary {
        holes_total,
        filled_this_cycle: 0,
        ready_to_fill,
        blocked,
    })
}

/// Summary of hole status for a single check cycle.
#[derive(Debug, Clone)]
pub struct HoleSummary {
    pub holes_total: usize,
    pub filled_this_cycle: usize,
    pub ready_to_fill: usize,
    pub blocked: usize,
}

impl HoleSummary {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"event\":\"hole_graph_update\",\"holes_total\":{},\"filled_this_cycle\":{},\"ready_to_fill\":{},\"blocked\":{}}}",
            self.holes_total, self.filled_this_cycle, self.ready_to_fill, self.blocked
        )
    }
}
