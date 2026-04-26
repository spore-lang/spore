use std::collections::BTreeSet;
use std::path::Path;

use crate::compiler::files::{
    anchor_diagnostics_to_source, batch_error_source, file_source, module_error_source,
    module_error_to_diagnostics, push_source_if_missing,
};
use crate::diagnostics::{diagnostics_for_parse_errors, diagnostics_for_type_errors};
use crate::project::{
    ResolvedPlatformContract, ResolvedProjectTarget, resolve_project_target_by_path,
};
use sporec_codegen::value::Value;
use sporec_codegen::{ProjectRunOutcome, RuntimePlatform};
use sporec_diagnostics::{Diagnostic as CanonicalDiagnostic, Severity, SourceFile};
use sporec_parser::ast::{Expr, FnDef, ImportDecl, Item, Module, Stmt};
use sporec_typeck::CheckResult;
use sporec_typeck::module::{ModuleInterface, ModuleLoader, ModuleRegistry, PreludeOptions};
use sporec_typeck::platform::{PlatformRegistry, PlatformStartupError, PlatformStartupErrorKind};
use sporec_typeck::type_check_with_registry_and_prelude;
use sporec_typeck::types::Ty;

use super::{
    CheckFailure, CheckReport, CompileOutput, format_verbose_result, join_errors, with_module_name,
};

/// Intermediate state after parsing and resolving a project entry module.
///
/// Shared setup for [`compile_project`] and [`run_project`].
struct PreparedProject {
    ast: Module,
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
    let mut loader = ModuleLoader::with_source_roots(
        root.to_path_buf(),
        target
            .source_roots
            .iter()
            .map(|source_root| root.join(source_root))
            .collect(),
        target.dependency_source_roots.clone(),
    );

    let entry_path = root.join(&target.entry_source_root).join(entry);
    let source = std::fs::read_to_string(&entry_path)
        .map_err(|e| format!("cannot read `{}`: {e}", entry_path.display()))?;
    let ast = sporec_parser::parse(&source).map_err(join_errors)?;

    let module_name = entry_module_name(entry);

    let mut registry = ModuleRegistry::new();
    let mut entry_iface = sporec_typeck::build_module_interface(&ast);
    entry_iface.path = module_name.split('.').map(|s| s.to_string()).collect();
    registry.register(entry_iface.clone());

    let imports = module_imports(&ast);
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
    let mut loader = ModuleLoader::with_source_roots(
        root.to_path_buf(),
        target
            .source_roots
            .iter()
            .map(|source_root| root.join(source_root))
            .collect(),
        target.dependency_source_roots.clone(),
    );

    let entry_path = root.join(&target.entry_source_root).join(entry);
    let source = match std::fs::read_to_string(&entry_path)
        .map_err(|e| format!("cannot read `{}`: {e}", entry_path.display()))
    {
        Ok(source) => source,
        Err(message) => return Err(CheckFailure::Message(message)),
    };
    let entry_source = file_source(entry, source.clone());
    let ast = match sporec_parser::parse(&source) {
        Ok(ast) => ast,
        Err(errors) => {
            return Err(CheckFailure::Diagnostics {
                sources: vec![entry_source.clone()],
                diagnostics: diagnostics_for_parse_errors(&entry_source, &errors),
            });
        }
    };

    let module_name = entry_module_name(entry);

    let mut registry = ModuleRegistry::new();
    let mut entry_iface = sporec_typeck::build_module_interface(&ast);
    entry_iface.path = module_name.split('.').map(|s| s.to_string()).collect();
    registry.register(entry_iface.clone());

    let imports = module_imports(&ast);
    if !imports.is_empty()
        && let Err(errors) = registry.resolve_imports(&mut loader, &module_name, &imports)
    {
        let mut sources = vec![entry_source.clone()];
        let mut diagnostics = Vec::new();
        for error in errors {
            let anchor_source = if error.importing_module == module_name {
                Some(entry_source.clone())
            } else {
                module_error_source(&error.importing_module, &loader)
            };
            let anchor = error
                .import_span
                .and_then(|span| anchor_source.as_ref().map(|source| (source, span)));
            let (source, module_diagnostics) =
                module_error_to_diagnostics(&loader, error.error, anchor);
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
        PlatformStartupErrorKind::UnsupportedEffect => "unsupported-platform-effect",
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

fn platform_contract_loader(contract: &ResolvedPlatformContract) -> ModuleLoader {
    ModuleLoader::with_source_roots(
        contract.root.clone(),
        contract
            .source_roots
            .iter()
            .map(|source_root| contract.root.join(source_root))
            .collect(),
        Vec::new(),
    )
}

fn load_platform_contract(
    contract: &ResolvedPlatformContract,
) -> Result<LoadedPlatformContract, PlatformStartupError> {
    let mut loader = platform_contract_loader(contract);
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
    let mut loader = platform_contract_loader(contract);
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
    let mut imports = std::collections::BTreeMap::new();

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
        Expr::Hole(_, _, _, _) => true,
        Expr::Block(stmts, Some(expr)) if stmts.is_empty() => is_hole_backed_contract_expr(expr),
        Expr::Block(stmts, None) => match stmts.as_slice() {
            [Stmt::Expr(expr)] => is_hole_backed_contract_expr(expr),
            _ => false,
        },
        _ => false,
    }
}

fn contract_function_def<'a>(module: &'a Module, name: &str) -> Option<&'a FnDef> {
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

    let handled_effects: BTreeSet<String> = contract.handled_effects.iter().cloned().collect();
    let required_effects = entry_iface
        .function_required_effects
        .get(&contract.startup_function)
        .cloned()
        .unwrap_or_default();
    let missing_effects: Vec<String> = required_effects
        .iter()
        .filter(|effect| !handled_effects.contains(*effect))
        .cloned()
        .collect();
    if !missing_effects.is_empty() {
        return Err(PlatformStartupError {
            kind: PlatformStartupErrorKind::UnsupportedEffect,
            message: format!(
                "startup function `{}` in entry module `{module_name}` requires effects [{}] not listed in `[platform].handled-effects` for platform `{}`",
                contract.startup_function,
                missing_effects.join(", "),
                contract.name
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
/// 2. Parses the entry module file under the configured source root for `{entry}`
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
    match run_project_with_outcome(root, entry)? {
        ProjectRunOutcome::Completed(value) => Ok(value),
        ProjectRunOutcome::Exited(code) => Err(format!("project requested exit with code {code}")),
    }
}

pub fn run_project_with_outcome(root: &Path, entry: &str) -> Result<ProjectRunOutcome, String> {
    let target = resolve_project_target_by_path(root, entry)?;
    let startup_function = target.startup_function.as_deref().ok_or_else(|| {
        format!(
            "entry path `{}` is not runnable: no platform startup contract is bound",
            target.entry_path
        )
    })?;
    let prep = prepare_project(root, &target)?;

    let _results = collect_prepared_project_results(&prep, &target.entry_path)?;
    validate_project_startup(&prep, &target)?;

    let imported = collect_runtime_import_modules(&prep, &target)?;
    let runtime_platform = runtime_platform_for_target(&target)?;
    if let Some(contract) = target.platform_contract.as_ref() {
        let adapter_function =
            format!("{}.{}", contract.contract_module, contract.adapter_function);
        return sporec_codegen::run_project_with_adapter_outcome_on_platform(
            &prep.ast,
            &imported,
            startup_function,
            &adapter_function,
            runtime_platform,
        )
        .map_err(|error| error.to_string());
    }

    sporec_codegen::run_project_with_outcome_on_platform(
        &prep.ast,
        &imported,
        startup_function,
        runtime_platform,
    )
    .map_err(|error| error.to_string())
}

/// Type-check a Spore project with verbose per-module output.
pub fn check_project_verbose(root: &Path, entry: &str) -> Result<String, String> {
    let target = resolve_project_target_by_path(root, entry)?;
    let prep = prepare_project(root, &target)?;
    let results = collect_prepared_project_results(&prep, &target.entry_path)?;
    validate_project_startup(&prep, &target)?;
    Ok(format_project_verbose_results(&results))
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
