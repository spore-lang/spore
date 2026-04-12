use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::diagnostics::{diagnostics_for_parse_errors, diagnostics_for_type_errors, source_file};
use crate::project::{
    ResolvedPlatformContract, ResolvedProjectTarget, resolve_project_target_by_path,
};
use spore_codegen::RuntimePlatform;
use spore_codegen::value::Value;
use spore_typeck::CheckResult;
use spore_typeck::hole::{
    CandidateRanking, EdgeKind, HoleInfo as TypeckHoleInfo, HoleReport as TypeckHoleReport,
    TypeInferenceConfidence,
};
use spore_typeck::is_synthetic_hole_name;
use spore_typeck::module::{
    ModuleError, ModuleInterface, ModuleLoader, ModuleRegistry, PreludeOptions,
};
use spore_typeck::platform::{PlatformRegistry, PlatformStartupError, PlatformStartupErrorKind};
use spore_typeck::types::Ty;
use spore_typeck::{type_check, type_check_with_registry, type_check_with_registry_and_prelude};
use sporec_diagnostics::{
    Diagnostic as CanonicalDiagnostic, HoleCandidateJson, HoleCandidateRankingJson,
    HoleConfidenceJson, HoleCostBudgetJson, HoleDependencyEdgeJson, HoleDependencyGraphJson,
    HoleDependencyKind, HoleErrorClusterJson, HoleInfoJson, HoleLocationJson, HoleReportJson,
    HoleSummary, HoleTypeInferenceJson, Severity, SourceFile,
};
use sporec_parser::ast::{Expr, ImportDecl, Item, Module, Span, Stmt};
use sporec_parser::formatter::format_module;
use sporec_parser::parse;

fn join_errors<E: std::fmt::Display>(errs: Vec<E>) -> String {
    errs.into_iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn load_hole_report(source: &str) -> Result<TypeckHoleReport, String> {
    let ast = parse(source).map_err(join_errors)?;
    let result = type_check(&ast).map_err(join_errors)?;
    Ok(result.hole_report)
}

fn display_hole_name(name: &str) -> String {
    if is_synthetic_hole_name(name) {
        "?".to_string()
    } else {
        format!("?{name}")
    }
}

fn hole_type_inference_json(confidence: TypeInferenceConfidence) -> HoleTypeInferenceJson {
    match confidence {
        TypeInferenceConfidence::Certain => HoleTypeInferenceJson::Certain,
        TypeInferenceConfidence::Partial => HoleTypeInferenceJson::Partial,
        TypeInferenceConfidence::Unknown => HoleTypeInferenceJson::Unknown,
    }
}

fn hole_candidate_ranking_json(ranking: CandidateRanking) -> HoleCandidateRankingJson {
    match ranking {
        CandidateRanking::UniqueBest => HoleCandidateRankingJson::UniqueBest,
        CandidateRanking::Ambiguous => HoleCandidateRankingJson::Ambiguous,
        CandidateRanking::NoCandidates => HoleCandidateRankingJson::NoCandidates,
    }
}

fn hole_dependency_kind_json(kind: &EdgeKind) -> HoleDependencyKind {
    match kind {
        EdgeKind::Type => HoleDependencyKind::Type,
        EdgeKind::Value => HoleDependencyKind::Value,
        EdgeKind::Cost => HoleDependencyKind::Cost,
    }
}

fn hole_dependency_kind_rank(kind: &HoleDependencyKind) -> u8 {
    match kind {
        HoleDependencyKind::Type => 0,
        HoleDependencyKind::Value => 1,
        HoleDependencyKind::Cost => 2,
    }
}

fn hole_info_json(hole: &TypeckHoleInfo) -> HoleInfoJson {
    HoleInfoJson {
        name: hole.name.clone(),
        display_name: display_hole_name(&hole.name),
        location: hole.location.as_ref().map(|location| HoleLocationJson {
            file: location.file.clone(),
            line: location.line,
            column: location.column,
        }),
        expected_type: hole.expected_type.to_string(),
        type_inferred_from: hole.type_inferred_from.clone(),
        function: hole.function.clone(),
        enclosing_signature: hole.enclosing_signature.clone(),
        bindings: hole
            .bindings
            .iter()
            .map(|(name, ty)| (name.clone(), ty.to_string()))
            .collect(),
        binding_dependencies: hole.binding_dependencies.clone(),
        capabilities: hole.capabilities.iter().cloned().collect(),
        errors_to_handle: hole.errors_to_handle.clone(),
        cost_budget: hole.cost_budget.as_ref().map(|budget| HoleCostBudgetJson {
            budget_total: budget.budget_total,
            cost_before_hole: budget.cost_before_hole,
            budget_remaining: budget.budget_remaining,
        }),
        candidates: hole
            .candidates
            .iter()
            .map(|candidate| HoleCandidateJson {
                name: candidate.name.clone(),
                type_match: candidate.type_match,
                cost_fit: candidate.cost_fit,
                capability_fit: candidate.capability_fit,
                error_coverage: candidate.error_coverage,
                overall: candidate.overall(),
            })
            .collect(),
        dependent_holes: hole.dependent_holes.clone(),
        confidence: hole
            .confidence
            .as_ref()
            .map(|confidence| HoleConfidenceJson {
                type_inference: hole_type_inference_json(confidence.type_inference.clone()),
                candidate_ranking: hole_candidate_ranking_json(
                    confidence.candidate_ranking.clone(),
                ),
                ambiguous_count: confidence.ambiguous_count,
                recommendation: confidence.recommendation.clone(),
            }),
        error_clusters: hole
            .error_clusters
            .iter()
            .map(|cluster| HoleErrorClusterJson {
                source: cluster.source.clone(),
                errors: cluster.errors.clone(),
                handling_suggestion: cluster.handling_suggestion.clone(),
            })
            .collect(),
    }
}

fn hole_dependency_graph_json(
    graph: &spore_typeck::hole::HoleDependencyGraph,
) -> HoleDependencyGraphJson {
    let dependencies = graph
        .dependencies
        .iter()
        .map(|(hole, deps)| {
            let mut deps = deps.iter().cloned().collect::<Vec<_>>();
            deps.sort();
            (hole.clone(), deps)
        })
        .collect::<BTreeMap<_, _>>();

    let mut edges = graph
        .edges
        .iter()
        .map(|edge| HoleDependencyEdgeJson {
            from: edge.from.clone(),
            to: edge.to.clone(),
            kind: hole_dependency_kind_json(&edge.kind),
        })
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        (&left.from, &left.to, hole_dependency_kind_rank(&left.kind)).cmp(&(
            &right.from,
            &right.to,
            hole_dependency_kind_rank(&right.kind),
        ))
    });

    HoleDependencyGraphJson {
        dependencies,
        edges,
        roots: graph.roots(),
        suggested_order: graph.topological_order(),
    }
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
    ast: sporec_parser::ast::Module,
    entry_source: String,
    entry_interface: ModuleInterface,
    registry: ModuleRegistry,
    prelude_options: PreludeOptions,
    loader: ModuleLoader,
}

fn project_prelude_options(target: &ResolvedProjectTarget) -> PreludeOptions {
    PreludeOptions {
        include_console: target.platform_contract.is_none(),
    }
}

/// Parse the selected entry module file, build a module registry, and resolve imports.
fn prepare_project(root: &Path, target: &ResolvedProjectTarget) -> Result<PreparedProject, String> {
    let entry = &target.entry_path;
    let prelude_options = project_prelude_options(target);
    let mut loader =
        ModuleLoader::with_dependency_roots(root.to_path_buf(), target.dependency_roots.clone());

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
        prelude_options,
        loader,
    })
}

fn prepare_project_for_report(
    root: &Path,
    target: &ResolvedProjectTarget,
) -> Result<PreparedProject, CheckFailure> {
    let entry = &target.entry_path;
    let prelude_options = project_prelude_options(target);
    let mut loader =
        ModuleLoader::with_dependency_roots(root.to_path_buf(), target.dependency_roots.clone());

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
        prelude_options,
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
    ast: &sporec_parser::ast::Module,
    module_name: &str,
) -> sporec_parser::ast::Module {
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
        match type_check_with_registry_and_prelude(
            &ast,
            prep.registry.clone(),
            prep.prelude_options,
        ) {
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
    match type_check_with_registry_and_prelude(
        &entry_ast,
        prep.registry.clone(),
        prep.prelude_options,
    ) {
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
        match type_check_with_registry_and_prelude(
            &ast,
            prep.registry.clone(),
            prep.prelude_options,
        ) {
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
    match type_check_with_registry_and_prelude(
        &entry_ast,
        prep.registry.clone(),
        prep.prelude_options,
    ) {
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
        PlatformStartupErrorKind::InvalidPlatformContract => "invalid-platform-contract",
    };
    CanonicalDiagnostic::new(code, Severity::Error, error.message)
        .with_primary_span(source.span(0..0))
}

#[derive(Debug, Clone)]
struct LoadedPlatformContract {
    startup_params: Vec<Ty>,
    startup_return: Ty,
}

fn invalid_platform_contract_error(message: impl Into<String>) -> PlatformStartupError {
    PlatformStartupError {
        kind: PlatformStartupErrorKind::InvalidPlatformContract,
        message: message.into(),
    }
}

fn load_platform_contract(
    contract: &ResolvedPlatformContract,
) -> Result<LoadedPlatformContract, PlatformStartupError> {
    let mut loader = ModuleLoader::new(contract.root.clone());
    let contract_iface = loader
        .load_module(&contract.contract_module)
        .map_err(|error| {
            invalid_platform_contract_error(format!(
                "platform `{}` contract module `{}` could not be loaded from `{}`: {error}",
                contract.name,
                contract.contract_module,
                contract.root.display()
            ))
        })?
        .clone();
    let contract_ast = loader.get_ast(&contract.contract_module).ok_or_else(|| {
        invalid_platform_contract_error(format!(
            "platform `{}` contract module `{}` did not produce a parsed AST",
            contract.name, contract.contract_module
        ))
    })?;

    let startup_def =
        contract_function_def(contract_ast, &contract.startup_function).ok_or_else(|| {
            invalid_platform_contract_error(format!(
                "platform `{}` contract module `{}` does not define startup contract `{}`",
                contract.name, contract.contract_module, contract.startup_function
            ))
        })?;
    if !startup_def
        .body
        .as_ref()
        .is_some_and(is_hole_backed_contract_expr)
    {
        return Err(invalid_platform_contract_error(format!(
            "platform `{}` startup contract `{}` in module `{}` must be hole-backed",
            contract.name, contract.startup_function, contract.contract_module
        )));
    }

    let (startup_params, startup_return) = contract_iface
        .functions
        .get(&contract.startup_function)
        .cloned()
        .ok_or_else(|| {
            invalid_platform_contract_error(format!(
                "platform `{}` contract module `{}` could not extract a signature for startup contract `{}`",
                contract.name, contract.contract_module, contract.startup_function
            ))
        })?;
    let (adapter_params, adapter_return) = contract_iface
        .functions
        .get(&contract.adapter_function)
        .cloned()
        .ok_or_else(|| {
            invalid_platform_contract_error(format!(
                "platform `{}` contract module `{}` does not define adapter function `{}`",
                contract.name, contract.contract_module, contract.adapter_function
            ))
        })?;
    let expected_adapter_params = vec![Ty::Fn(
        startup_params.clone(),
        Box::new(startup_return.clone()),
        Default::default(),
        Default::default(),
    )];
    if adapter_params != expected_adapter_params || adapter_return != startup_return {
        return Err(invalid_platform_contract_error(format!(
            "platform `{}` adapter `{}` in module `{}` should match `{}`, found `{}`",
            contract.name,
            contract.adapter_function,
            contract.contract_module,
            format_signature(
                &contract.adapter_function,
                &expected_adapter_params,
                &startup_return
            ),
            format_signature(&contract.adapter_function, &adapter_params, &adapter_return)
        )));
    }

    Ok(LoadedPlatformContract {
        startup_params,
        startup_return,
    })
}

fn module_imports(module: &Module) -> Vec<ImportDecl> {
    module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Import(import) => Some(import.clone()),
            _ => None,
        })
        .collect()
}

fn load_platform_runtime_modules(
    contract: &ResolvedPlatformContract,
) -> Result<Vec<(String, Module)>, String> {
    let mut loader = ModuleLoader::new(contract.root.clone());
    let contract_iface = loader
        .load_module(&contract.contract_module)
        .map_err(|error| {
            format!(
                "platform `{}` contract module `{}` could not be loaded from `{}`: {error}",
                contract.name,
                contract.contract_module,
                contract.root.display()
            )
        })?
        .clone();
    let contract_ast = loader
        .get_ast(&contract.contract_module)
        .cloned()
        .ok_or_else(|| {
            format!(
                "platform `{}` contract module `{}` did not produce a parsed AST",
                contract.name, contract.contract_module
            )
        })?;

    let mut registry = ModuleRegistry::new();
    registry.register(contract_iface);
    registry
        .resolve_imports(
            &mut loader,
            &contract.contract_module,
            &module_imports(&contract_ast),
        )
        .map_err(|errors| {
            format!(
                "platform `{}` contract module `{}` could not resolve runtime imports: {}",
                contract.name,
                contract.contract_module,
                join_errors(errors)
            )
        })?;

    let mut loaded_paths = loader.loaded_modules();
    loaded_paths.sort();
    Ok(loaded_paths
        .into_iter()
        .filter_map(|path| loader.get_ast(&path).map(|ast| (path, ast.clone())))
        .collect())
}

fn collect_runtime_import_modules(
    prep: &PreparedProject,
    target: &ResolvedProjectTarget,
) -> Result<Vec<(String, Module)>, String> {
    let mut imports = BTreeMap::new();

    let mut loaded_paths = prep.loader.loaded_modules();
    loaded_paths.sort();
    for path in loaded_paths {
        if let Some(ast) = prep.loader.get_ast(&path) {
            imports.insert(path, ast.clone());
        }
    }

    if let Some(contract) = target.platform_contract.as_ref() {
        for (path, ast) in load_platform_runtime_modules(contract)? {
            imports.insert(path, ast);
        }
    }

    Ok(imports.into_iter().collect())
}

fn runtime_platform_for_target(target: &ResolvedProjectTarget) -> Result<RuntimePlatform, String> {
    if let Some(contract) = target.platform_contract.as_ref() {
        return match contract.name.as_str() {
            "basic-cli" => Ok(RuntimePlatform::BasicCli),
            other => Err(format!(
                "runtime host binding for package platform `{other}` is not implemented yet; currently supported package platforms: basic-cli"
            )),
        };
    }

    Ok(RuntimePlatform::Cli)
}

fn is_hole_backed_contract_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Hole(_, _, _) => true,
        Expr::Block(stmts, Some(expr)) if stmts.is_empty() => is_hole_backed_contract_expr(expr),
        Expr::Block(stmts, None) => match stmts.as_slice() {
            [Stmt::Expr(expr)] => is_hole_backed_contract_expr(expr),
            _ => false,
        },
        _ => false,
    }
}

fn contract_function_def<'a>(
    module: &'a sporec_parser::ast::Module,
    name: &str,
) -> Option<&'a sporec_parser::ast::FnDef> {
    module.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == name => Some(function),
        _ => None,
    })
}

fn format_signature(name: &str, params: &[Ty], ret: &Ty) -> String {
    format!(
        "{name}({}) -> {ret}",
        params
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn validate_platform_contract_entry_startup(
    entry_iface: &ModuleInterface,
    contract: &ResolvedPlatformContract,
) -> Result<(), PlatformStartupError> {
    let loaded = load_platform_contract(contract)?;
    let module_name = entry_iface.qualified_name();
    let Some((actual_params, actual_return)) =
        entry_iface.functions.get(&contract.startup_function)
    else {
        return Err(PlatformStartupError {
            kind: PlatformStartupErrorKind::MissingStartupFunction,
            message: format!(
                "entry module `{module_name}` does not define required startup function `{}` from platform `{}` contract module `{}`",
                contract.startup_function, contract.name, contract.contract_module
            ),
        });
    };

    if actual_params != &loaded.startup_params || actual_return != &loaded.startup_return {
        return Err(PlatformStartupError {
            kind: PlatformStartupErrorKind::WrongStartupSignature,
            message: format!(
                "startup function `{}` in entry module `{module_name}` should match platform contract `{}` from `{}` ({})",
                contract.startup_function,
                contract.contract_module,
                contract.name,
                format_signature(
                    &contract.startup_function,
                    &loaded.startup_params,
                    &loaded.startup_return
                )
            ),
        });
    }

    Ok(())
}

fn validate_project_startup_error(
    prep: &PreparedProject,
    target: &ResolvedProjectTarget,
) -> Result<(), PlatformStartupError> {
    let Some(platform_name) = target.platform_name.as_deref() else {
        return Ok(());
    };
    if let Some(contract) = target.platform_contract.as_ref() {
        return validate_platform_contract_entry_startup(&prep.entry_interface, contract);
    }

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
    let prep = match prepare_project_for_report(root, &target) {
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
    let prep = prepare_project(root, &target)?;
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
    let prep = prepare_project(root, &target)?;

    // Type-check
    let _results = collect_prepared_project_results(&prep, &target.entry_path)?;
    validate_project_startup(&prep, &target)?;

    let imported = collect_runtime_import_modules(&prep, &target)?;
    let runtime_platform = runtime_platform_for_target(&target)?;
    if let Some(contract) = target.platform_contract.as_ref() {
        let adapter_function =
            format!("{}.{}", contract.contract_module, contract.adapter_function);
        return spore_codegen::run_project_with_adapter_on_platform(
            &prep.ast,
            &imported,
            startup_function,
            &adapter_function,
            runtime_platform,
        )
        .map_err(|error| error.to_string());
    }

    spore_codegen::run_project_on_platform(&prep.ast, &imported, startup_function, runtime_platform)
        .map_err(|error| error.to_string())
}

/// Analyze holes in Spore source and return the shared JSON report payload.
pub fn holes_report(source: &str) -> Result<HoleReportJson, String> {
    let report = load_hole_report(source)?;
    Ok(HoleReportJson {
        holes: report.holes.iter().map(hole_info_json).collect(),
        dependency_graph: hole_dependency_graph_json(&report.dependency_graph),
    })
}

/// Analyze holes in Spore source and return a JSON report.
pub fn holes(source: &str) -> Result<String, String> {
    let report = holes_report(source)?;
    serde_json::to_string(&report).map_err(|error| error.to_string())
}

/// Inspect a named hole and return the shared JSON payload used by `query-hole`.
pub fn query_hole_report(file: &str, source: &str, hole: &str) -> Result<HoleInfoJson, String> {
    let report = load_hole_report(source)?;
    let needle = hole.strip_prefix('?').unwrap_or(hole);
    let matches: Vec<&TypeckHoleInfo> = report
        .holes
        .iter()
        .filter(|candidate| candidate.name == needle)
        .collect();

    match matches.as_slice() {
        [hole] => Ok(hole_info_json(hole)),
        [] => Err(format!("hole `?{needle}` not found in `{file}`")),
        _ => {
            let locations = matches
                .iter()
                .map(|candidate| candidate.function.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "hole `?{needle}` is ambiguous in `{file}`; matching functions: {locations}"
            ))
        }
    }
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
    let prep = prepare_project(root, &target)?;
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
    let report = load_hole_report(source).ok()?;
    let graph = &report.dependency_graph;

    let holes_total = report.holes.len();
    if holes_total == 0 {
        return None;
    }

    let ready_to_fill = graph.roots().len();
    let blocked = holes_total.saturating_sub(ready_to_fill);

    Some(HoleSummary::new(holes_total, 0, ready_to_fill, blocked))
}
