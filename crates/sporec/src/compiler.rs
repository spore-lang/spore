use std::path::Path;

use spore_codegen::value::Value;
use spore_parser::ast::{ImportDecl, Item, Span};
use spore_parser::formatter::format_module;
use spore_parser::parse;
use spore_typeck::CheckResult;
use spore_typeck::module::{ModuleLoader, ModuleRegistry};
use spore_typeck::{type_check, type_check_with_registry};

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
        let ast = parse(&source).map_err(|errs| {
            let msgs: Vec<String> = errs.into_iter().map(|e| e.to_string()).collect();
            format!("{path}: {}", msgs.join("\n"))
        })?;
        modules.push((*path, ast));
    }

    // Phase 2: Build ModuleRegistry from all modules
    let mut registry = ModuleRegistry::new();
    for (_path, ast) in &modules {
        let iface = spore_typeck::build_module_interface(ast);
        registry.register(iface);
    }

    // Phase 3: Type-check each module with the shared registry
    let mut all_errors = Vec::new();
    let mut all_warnings = Vec::new();
    for (path, ast) in &modules {
        match type_check_with_registry(ast, registry.clone()) {
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

/// Intermediate state after parsing and resolving a project entry point.
///
/// Shared setup for [`compile_project`] and [`run_project`].
struct PreparedProject {
    ast: spore_parser::ast::Module,
    registry: ModuleRegistry,
    loader: ModuleLoader,
}

/// Parse the entry file, build a module registry, and resolve imports.
fn prepare_project(root: &Path, entry: &str) -> Result<PreparedProject, String> {
    let mut loader = ModuleLoader::new(root.to_path_buf());

    // Parse entry file
    let entry_path = root.join("src").join(entry);
    let source = std::fs::read_to_string(&entry_path)
        .map_err(|e| format!("cannot read `{}`: {e}", entry_path.display()))?;
    let ast = parse(&source).map_err(join_errors)?;

    // Derive a module name from the entry path
    let module_name = if ast.name.is_empty() {
        entry.trim_end_matches(".sp").replace(['/', '\\'], ".")
    } else {
        ast.name.clone()
    };

    // Build registry and register the entry module
    let mut registry = ModuleRegistry::new();
    let mut entry_iface = spore_typeck::build_module_interface(&ast);
    entry_iface.path = module_name.split('.').map(|s| s.to_string()).collect();
    registry.register(entry_iface);

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
        registry,
        loader,
    })
}

fn entry_module_name(ast: &spore_parser::ast::Module, entry: &str) -> String {
    if ast.name.is_empty() {
        entry.trim_end_matches(".sp").replace(['/', '\\'], ".")
    } else {
        ast.name.clone()
    }
}

fn source_label_for_module(module_path: &str) -> String {
    format!("{}.sp", module_path.replace('.', "/"))
}

fn with_module_name(
    ast: &spore_parser::ast::Module,
    module_name: &str,
) -> spore_parser::ast::Module {
    let mut ast = ast.clone();
    if ast.name.is_empty() {
        ast.name = module_name.to_string();
    }
    ast
}

fn type_check_prepared_project(
    prep: &PreparedProject,
    entry: &str,
) -> Result<CompileOutput, String> {
    let mut all_errors = Vec::new();
    let mut all_warnings = Vec::new();

    let mut loaded_modules = prep.loader.loaded_modules();
    loaded_modules.sort();

    for module_path in loaded_modules {
        let Some(ast) = prep.loader.get_ast(&module_path) else {
            continue;
        };
        let ast = with_module_name(ast, &module_path);
        let label = source_label_for_module(&module_path);
        match type_check_with_registry(&ast, prep.registry.clone()) {
            Ok(result) => {
                for warning in &result.warnings {
                    all_warnings.push(format!("{label}: {warning}"));
                }
            }
            Err(errs) => {
                for err in errs {
                    all_errors.push(format!("{label}: {err}"));
                }
            }
        }
    }

    let entry_label = entry.replace('\\', "/");
    let entry_name = entry_module_name(&prep.ast, entry);
    let entry_ast = with_module_name(&prep.ast, &entry_name);
    match type_check_with_registry(&entry_ast, prep.registry.clone()) {
        Ok(result) => {
            for warning in &result.warnings {
                all_warnings.push(format!("{entry_label}: {warning}"));
            }
        }
        Err(errs) => {
            for err in errs {
                all_errors.push(format!("{entry_label}: {err}"));
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

/// Compile a Spore project rooted at `root`, starting from `entry`.
///
/// 1. Creates a [`ModuleLoader`] from the project root
/// 2. Parses the entry file at `{root}/src/{entry}`
/// 3. Recursively resolves all imports from disk
/// 4. Type-checks with a shared [`ModuleRegistry`]
///
/// Single-file projects (no imports) work without a `ModuleLoader`.
pub fn compile_project(root: &Path, entry: &str) -> Result<CompileOutput, String> {
    let prep = prepare_project(root, entry)?;
    type_check_prepared_project(&prep, entry)
}

/// Run a Spore project by compiling and executing its entry file's `main`.
///
/// Like [`compile_project`], but also invokes the interpreter with
/// cross-module function resolution.
pub fn run_project(root: &Path, entry: &str) -> Result<Value, String> {
    let prep = prepare_project(root, entry)?;

    // Type-check
    let _result = type_check_prepared_project(&prep, entry)?;

    // Collect imported module ASTs for the interpreter
    let mut imported_paths = prep.loader.loaded_modules();
    imported_paths.sort();
    let imported: Vec<(String, spore_parser::ast::Module)> = imported_paths
        .into_iter()
        .filter_map(|path| prep.loader.get_ast(&path).map(|ast| (path, ast.clone())))
        .collect();

    spore_codegen::run_project(&prep.ast, &imported).map_err(|e| e.to_string())
}

/// Analyze holes in Spore source and return a JSON report.
pub fn holes(source: &str) -> Result<String, String> {
    let ast = parse(source).map_err(join_errors)?;
    let result = type_check(&ast).map_err(join_errors)?;
    Ok(result.hole_report.to_json())
}

/// Run a Spore program by executing its `main` function.
pub fn run(source: &str) -> Result<Value, String> {
    let ast = parse(source).map_err(join_errors)?;
    let _result = type_check(&ast).map_err(join_errors)?;
    spore_codegen::run(&ast).map_err(|e| e.to_string())
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
        out.push_str(&format!("    ?{}: expected {}\n", h.name, h.expected_type));
    }

    // Cost analysis
    if !result.cost_vectors.is_empty() {
        out.push_str("\n── Cost Analysis ──\n");
        for (fn_name, cv) in &result.cost_vectors {
            out.push_str(&format!(
                "  {fn_name}: compute={}, alloc={}, io={}, parallel={}\n",
                cv.compute, cv.alloc, cv.io, cv.parallel
            ));
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
