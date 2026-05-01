use std::collections::{BTreeMap, BTreeSet};
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
use sporec_parser::ast::{
    Expr, FnDef, HandleBinding, ImportDecl, Item, MatchArm, Module, Pattern, SelectArm, Stmt,
    Visibility,
};
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

fn native_project_function_name(module_path: &str, function_name: &str) -> String {
    format!("{module_path}.{function_name}")
}

fn collect_native_project_modules(
    prep: &PreparedProject,
    target: &ResolvedProjectTarget,
) -> Result<BTreeMap<String, Module>, String> {
    let mut modules: BTreeMap<String, Module> = collect_runtime_import_modules(prep, target)?
        .into_iter()
        .map(|(path, ast)| (path.clone(), with_module_name(&ast, &path)))
        .collect();
    let entry_path = entry_module_name(&target.entry_path);
    modules.insert(entry_path.clone(), with_module_name(&prep.ast, &entry_path));
    Ok(modules)
}

fn collect_module_function_renames(module_path: &str, module: &Module) -> BTreeMap<String, String> {
    module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) => Some((
                function.name.clone(),
                native_project_function_name(module_path, &function.name),
            )),
            _ => None,
        })
        .collect()
}

fn collect_imported_function_bindings(
    module_path: &str,
    module: &Module,
    modules: &BTreeMap<String, Module>,
) -> Result<BTreeMap<String, String>, String> {
    let mut imported = BTreeMap::new();
    let mut origins = BTreeMap::<String, String>::new();
    for import in module_imports(module) {
        let import_path = match import {
            ImportDecl::Import { path, .. } | ImportDecl::Alias { path, .. } => path,
        };
        let imported_module = modules.get(&import_path).ok_or_else(|| {
            format!(
                "native project build could not resolve imported module `{import_path}` from `{module_path}`"
            )
        })?;
        for item in &imported_module.items {
            let Item::Function(function) = item else {
                continue;
            };
            if !matches!(function.visibility, Visibility::Pub | Visibility::PubPkg) {
                continue;
            }

            if let Some(existing) = origins.insert(function.name.clone(), import_path.clone())
                && existing != import_path
            {
                return Err(format!(
                    "ambiguous native project import in module `{module_path}`: function `{}` is exported by both `{existing}` and `{import_path}`",
                    function.name
                ));
            }

            imported.insert(
                function.name.clone(),
                native_project_function_name(&import_path, &function.name),
            );
        }
    }
    Ok(imported)
}

fn scoped_name(
    name: &str,
    scopes: &[BTreeSet<String>],
    local_functions: &BTreeMap<String, String>,
    imported_functions: &BTreeMap<String, String>,
) -> String {
    if scopes.iter().rev().any(|scope| scope.contains(name)) {
        return name.to_string();
    }
    if let Some(local) = local_functions.get(name) {
        return local.clone();
    }
    if let Some(imported) = imported_functions.get(name) {
        return imported.clone();
    }
    name.to_string()
}

fn pattern_bindings(pattern: &Pattern, bindings: &mut BTreeSet<String>) {
    match pattern {
        Pattern::Wildcard | Pattern::IntLit(_) | Pattern::StrLit(_) | Pattern::BoolLit(_) => {}
        Pattern::Var(name) => {
            bindings.insert(name.clone());
        }
        Pattern::Constructor(_, patterns) | Pattern::Or(patterns) | Pattern::List(patterns, _) => {
            for pattern in patterns {
                pattern_bindings(pattern, bindings);
            }
            if let Pattern::List(_, Some(rest)) = pattern {
                bindings.insert(rest.clone());
            }
        }
        Pattern::Struct(_, fields) => {
            for (_, pattern) in fields {
                pattern_bindings(pattern, bindings);
            }
        }
    }
}

fn rewrite_native_project_stmt(
    stmt: &Stmt,
    scopes: &mut Vec<BTreeSet<String>>,
    local_functions: &BTreeMap<String, String>,
    imported_functions: &BTreeMap<String, String>,
) -> Stmt {
    match stmt {
        Stmt::Let(name, annotation, value) => {
            let value =
                rewrite_native_project_expr(value, scopes, local_functions, imported_functions);
            scopes.last_mut().unwrap().insert(name.clone());
            Stmt::Let(name.clone(), annotation.clone(), value)
        }
        Stmt::Expr(expr) => Stmt::Expr(rewrite_native_project_expr(
            expr,
            scopes,
            local_functions,
            imported_functions,
        )),
    }
}

fn rewrite_match_arm(
    arm: &MatchArm,
    scopes: &mut Vec<BTreeSet<String>>,
    local_functions: &BTreeMap<String, String>,
    imported_functions: &BTreeMap<String, String>,
) -> MatchArm {
    let mut bindings = BTreeSet::new();
    pattern_bindings(&arm.pattern, &mut bindings);
    scopes.push(bindings);
    let guard = arm.guard.as_ref().map(|guard| {
        rewrite_native_project_expr(guard, scopes, local_functions, imported_functions)
    });
    let body = rewrite_native_project_expr(&arm.body, scopes, local_functions, imported_functions);
    scopes.pop();
    MatchArm {
        pattern: arm.pattern.clone(),
        guard,
        body,
    }
}

fn rewrite_native_project_expr(
    expr: &Expr,
    scopes: &mut Vec<BTreeSet<String>>,
    local_functions: &BTreeMap<String, String>,
    imported_functions: &BTreeMap<String, String>,
) -> Expr {
    match expr {
        Expr::IntLit(_)
        | Expr::FloatLit(_)
        | Expr::StrLit(_)
        | Expr::BoolLit(_)
        | Expr::Hole(_, _, _, _)
        | Expr::Placeholder => expr.clone(),
        Expr::FString(parts) => Expr::FString(
            parts
                .iter()
                .map(|part| match part {
                    sporec_parser::ast::FStringPart::Literal(text) => {
                        sporec_parser::ast::FStringPart::Literal(text.clone())
                    }
                    sporec_parser::ast::FStringPart::Expr(expr) => {
                        sporec_parser::ast::FStringPart::Expr(rewrite_native_project_expr(
                            expr,
                            scopes,
                            local_functions,
                            imported_functions,
                        ))
                    }
                })
                .collect(),
        ),
        Expr::TString(parts) => Expr::TString(
            parts
                .iter()
                .map(|part| match part {
                    sporec_parser::ast::TStringPart::Literal(text) => {
                        sporec_parser::ast::TStringPart::Literal(text.clone())
                    }
                    sporec_parser::ast::TStringPart::Expr(expr) => {
                        sporec_parser::ast::TStringPart::Expr(rewrite_native_project_expr(
                            expr,
                            scopes,
                            local_functions,
                            imported_functions,
                        ))
                    }
                })
                .collect(),
        ),
        Expr::Var(name) => Expr::Var(scoped_name(
            name,
            scopes,
            local_functions,
            imported_functions,
        )),
        Expr::Call(callee, args) => Expr::Call(
            Box::new(rewrite_native_project_expr(
                callee,
                scopes,
                local_functions,
                imported_functions,
            )),
            args.iter()
                .map(|arg| {
                    rewrite_native_project_expr(arg, scopes, local_functions, imported_functions)
                })
                .collect(),
        ),
        Expr::Lambda(params, body) => {
            scopes.push(params.iter().map(|param| param.name.clone()).collect());
            let body =
                rewrite_native_project_expr(body, scopes, local_functions, imported_functions);
            scopes.pop();
            Expr::Lambda(params.clone(), Box::new(body))
        }
        Expr::BinOp(lhs, op, rhs) => Expr::BinOp(
            Box::new(rewrite_native_project_expr(
                lhs,
                scopes,
                local_functions,
                imported_functions,
            )),
            op.clone(),
            Box::new(rewrite_native_project_expr(
                rhs,
                scopes,
                local_functions,
                imported_functions,
            )),
        ),
        Expr::UnaryOp(op, value) => Expr::UnaryOp(
            op.clone(),
            Box::new(rewrite_native_project_expr(
                value,
                scopes,
                local_functions,
                imported_functions,
            )),
        ),
        Expr::FieldAccess(target, field) => Expr::FieldAccess(
            Box::new(rewrite_native_project_expr(
                target,
                scopes,
                local_functions,
                imported_functions,
            )),
            field.clone(),
        ),
        Expr::Pipe(lhs, rhs) => Expr::Pipe(
            Box::new(rewrite_native_project_expr(
                lhs,
                scopes,
                local_functions,
                imported_functions,
            )),
            Box::new(rewrite_native_project_expr(
                rhs,
                scopes,
                local_functions,
                imported_functions,
            )),
        ),
        Expr::If(condition, then_branch, else_branch) => Expr::If(
            Box::new(rewrite_native_project_expr(
                condition,
                scopes,
                local_functions,
                imported_functions,
            )),
            Box::new(rewrite_native_project_expr(
                then_branch,
                scopes,
                local_functions,
                imported_functions,
            )),
            else_branch.as_ref().map(|else_branch| {
                Box::new(rewrite_native_project_expr(
                    else_branch,
                    scopes,
                    local_functions,
                    imported_functions,
                ))
            }),
        ),
        Expr::Match(value, arms) => Expr::Match(
            Box::new(rewrite_native_project_expr(
                value,
                scopes,
                local_functions,
                imported_functions,
            )),
            arms.iter()
                .map(|arm| rewrite_match_arm(arm, scopes, local_functions, imported_functions))
                .collect(),
        ),
        Expr::Block(stmts, tail) => {
            scopes.push(BTreeSet::new());
            let stmts = stmts
                .iter()
                .map(|stmt| {
                    rewrite_native_project_stmt(stmt, scopes, local_functions, imported_functions)
                })
                .collect();
            let tail = tail.as_ref().map(|tail| {
                Box::new(rewrite_native_project_expr(
                    tail,
                    scopes,
                    local_functions,
                    imported_functions,
                ))
            });
            scopes.pop();
            Expr::Block(stmts, tail)
        }
        Expr::Try(expr) => Expr::Try(Box::new(rewrite_native_project_expr(
            expr,
            scopes,
            local_functions,
            imported_functions,
        ))),
        Expr::StructLit(name, fields) => Expr::StructLit(
            name.clone(),
            fields
                .iter()
                .map(|(field, value)| {
                    (
                        field.clone(),
                        rewrite_native_project_expr(
                            value,
                            scopes,
                            local_functions,
                            imported_functions,
                        ),
                    )
                })
                .collect(),
        ),
        Expr::Spawn(expr) => Expr::Spawn(Box::new(rewrite_native_project_expr(
            expr,
            scopes,
            local_functions,
            imported_functions,
        ))),
        Expr::Await(expr) => Expr::Await(Box::new(rewrite_native_project_expr(
            expr,
            scopes,
            local_functions,
            imported_functions,
        ))),
        Expr::ChannelNew { elem_type, buffer } => Expr::ChannelNew {
            elem_type: elem_type.clone(),
            buffer: Box::new(rewrite_native_project_expr(
                buffer,
                scopes,
                local_functions,
                imported_functions,
            )),
        },
        Expr::Return(value) => Expr::Return(value.as_ref().map(|value| {
            Box::new(rewrite_native_project_expr(
                value,
                scopes,
                local_functions,
                imported_functions,
            ))
        })),
        Expr::Throw(value) => Expr::Throw(Box::new(rewrite_native_project_expr(
            value,
            scopes,
            local_functions,
            imported_functions,
        ))),
        Expr::List(items) => Expr::List(
            items
                .iter()
                .map(|item| {
                    rewrite_native_project_expr(item, scopes, local_functions, imported_functions)
                })
                .collect(),
        ),
        Expr::ParallelScope { lanes, body } => Expr::ParallelScope {
            lanes: lanes.as_ref().map(|lanes| {
                Box::new(rewrite_native_project_expr(
                    lanes,
                    scopes,
                    local_functions,
                    imported_functions,
                ))
            }),
            body: Box::new(rewrite_native_project_expr(
                body,
                scopes,
                local_functions,
                imported_functions,
            )),
        },
        Expr::Select(arms) => Expr::Select(
            arms.iter()
                .map(|arm| match arm {
                    SelectArm::Recv {
                        binding,
                        source,
                        body,
                    } => {
                        let source = rewrite_native_project_expr(
                            source,
                            scopes,
                            local_functions,
                            imported_functions,
                        );
                        scopes.push([binding.clone()].into_iter().collect());
                        let body = rewrite_native_project_expr(
                            body,
                            scopes,
                            local_functions,
                            imported_functions,
                        );
                        scopes.pop();
                        SelectArm::Recv {
                            binding: binding.clone(),
                            source,
                            body,
                        }
                    }
                    SelectArm::Timeout { duration, body } => SelectArm::Timeout {
                        duration: rewrite_native_project_expr(
                            duration,
                            scopes,
                            local_functions,
                            imported_functions,
                        ),
                        body: rewrite_native_project_expr(
                            body,
                            scopes,
                            local_functions,
                            imported_functions,
                        ),
                    },
                })
                .collect(),
        ),
        Expr::Perform {
            effect,
            operation,
            args,
        } => Expr::Perform {
            effect: effect.clone(),
            operation: operation.clone(),
            args: args
                .iter()
                .map(|arg| {
                    Box::new(rewrite_native_project_expr(
                        arg,
                        scopes,
                        local_functions,
                        imported_functions,
                    ))
                })
                .collect(),
        },
        Expr::Handle { body, handlers } => Expr::Handle {
            body: Box::new(rewrite_native_project_expr(
                body,
                scopes,
                local_functions,
                imported_functions,
            )),
            handlers: handlers
                .iter()
                .map(|handler| match handler {
                    HandleBinding::Use(use_handler) => {
                        HandleBinding::Use(sporec_parser::ast::HandlerUse {
                            handler: use_handler.handler.clone(),
                            payload: use_handler
                                .payload
                                .iter()
                                .map(|(name, value)| {
                                    (
                                        name.clone(),
                                        rewrite_native_project_expr(
                                            value,
                                            scopes,
                                            local_functions,
                                            imported_functions,
                                        ),
                                    )
                                })
                                .collect(),
                        })
                    }
                    HandleBinding::On(arm) => {
                        scopes.push(arm.params.iter().cloned().collect());
                        let body = rewrite_native_project_expr(
                            &arm.body,
                            scopes,
                            local_functions,
                            imported_functions,
                        );
                        scopes.pop();
                        HandleBinding::On(sporec_parser::ast::EffectArm {
                            effect: arm.effect.clone(),
                            operation: arm.operation.clone(),
                            params: arm.params.clone(),
                            body: Box::new(body),
                        })
                    }
                })
                .collect(),
        },
    }
}

fn rewrite_native_project_function(
    module_path: &str,
    function: &FnDef,
    local_functions: &BTreeMap<String, String>,
    imported_functions: &BTreeMap<String, String>,
) -> FnDef {
    let mut rewritten = function.clone();
    rewritten.name = native_project_function_name(module_path, &function.name);
    rewritten.body = function.body.as_ref().map(|body| {
        let mut scopes = vec![
            function
                .params
                .iter()
                .map(|param| param.name.clone())
                .collect(),
        ];
        rewrite_native_project_expr(body, &mut scopes, local_functions, imported_functions)
    });
    rewritten
}

fn expr_is_startup_alias(expr: &Expr, startup_function: &str) -> bool {
    match expr {
        Expr::Var(name) => name == startup_function,
        Expr::Block(stmts, Some(tail)) if stmts.is_empty() => {
            expr_is_startup_alias(tail, startup_function)
        }
        _ => false,
    }
}

fn lookup_startup_alias(name: &str, scopes: &[BTreeMap<String, bool>]) -> Option<bool> {
    scopes
        .iter()
        .rev()
        .find_map(|scope| scope.get(name).copied())
}

fn startup_specialization_cache_key(
    function_name: &str,
    bound_positions: &BTreeSet<usize>,
) -> String {
    format!(
        "{function_name}|{}",
        bound_positions
            .iter()
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join("_")
    )
}

fn startup_specialization_name(function_name: &str, bound_positions: &BTreeSet<usize>) -> String {
    format!(
        "{function_name}.__spore_startup_{}",
        bound_positions
            .iter()
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join("_")
    )
}

fn specialize_startup_expr(
    expr: &Expr,
    bound_params: &BTreeSet<String>,
    startup_function: &str,
    functions: &mut BTreeMap<String, FnDef>,
    specializations: &mut BTreeMap<String, String>,
    scopes: &mut Vec<BTreeMap<String, bool>>,
) -> Result<Expr, String> {
    Ok(match expr {
        Expr::IntLit(_)
        | Expr::FloatLit(_)
        | Expr::StrLit(_)
        | Expr::BoolLit(_)
        | Expr::Hole(_, _, _, _)
        | Expr::Placeholder => expr.clone(),
        Expr::FString(parts) => Expr::FString(
            parts
                .iter()
                .map(|part| match part {
                    sporec_parser::ast::FStringPart::Literal(text) => {
                        Ok(sporec_parser::ast::FStringPart::Literal(text.clone()))
                    }
                    sporec_parser::ast::FStringPart::Expr(expr) => Ok(
                        sporec_parser::ast::FStringPart::Expr(specialize_startup_expr(
                            expr,
                            bound_params,
                            startup_function,
                            functions,
                            specializations,
                            scopes,
                        )?),
                    ),
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        Expr::TString(parts) => Expr::TString(
            parts
                .iter()
                .map(|part| match part {
                    sporec_parser::ast::TStringPart::Literal(text) => {
                        Ok(sporec_parser::ast::TStringPart::Literal(text.clone()))
                    }
                    sporec_parser::ast::TStringPart::Expr(expr) => Ok(
                        sporec_parser::ast::TStringPart::Expr(specialize_startup_expr(
                            expr,
                            bound_params,
                            startup_function,
                            functions,
                            specializations,
                            scopes,
                        )?),
                    ),
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        Expr::Var(name) => match lookup_startup_alias(name, scopes) {
            Some(true) => Expr::Var(startup_function.to_string()),
            Some(false) => Expr::Var(name.clone()),
            None if bound_params.contains(name) => Expr::Var(startup_function.to_string()),
            None => Expr::Var(name.clone()),
        },
        Expr::Call(callee, args) => {
            let callee = Box::new(specialize_startup_expr(
                callee,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?);
            let args = args
                .iter()
                .map(|arg| {
                    specialize_startup_expr(
                        arg,
                        bound_params,
                        startup_function,
                        functions,
                        specializations,
                        scopes,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;

            if let Expr::Var(name) = callee.as_ref()
                && functions.contains_key(name)
            {
                let bound_positions = args
                    .iter()
                    .enumerate()
                    .filter_map(|(index, arg)| {
                        expr_is_startup_alias(arg, startup_function).then_some(index)
                    })
                    .collect::<BTreeSet<_>>();
                if !bound_positions.is_empty() {
                    let specialized_name = ensure_startup_specialization(
                        functions,
                        specializations,
                        name,
                        &bound_positions,
                        startup_function,
                    )?;
                    let args = args
                        .into_iter()
                        .enumerate()
                        .filter_map(|(index, arg)| {
                            (!bound_positions.contains(&index)).then_some(arg)
                        })
                        .collect();
                    Expr::Call(Box::new(Expr::Var(specialized_name)), args)
                } else {
                    Expr::Call(callee, args)
                }
            } else {
                Expr::Call(callee, args)
            }
        }
        Expr::Lambda(params, body) => {
            scopes.push(
                params
                    .iter()
                    .map(|param| (param.name.clone(), false))
                    .collect(),
            );
            let body = specialize_startup_expr(
                body,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?;
            scopes.pop();
            Expr::Lambda(params.clone(), Box::new(body))
        }
        Expr::BinOp(lhs, op, rhs) => Expr::BinOp(
            Box::new(specialize_startup_expr(
                lhs,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
            op.clone(),
            Box::new(specialize_startup_expr(
                rhs,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
        ),
        Expr::UnaryOp(op, value) => Expr::UnaryOp(
            op.clone(),
            Box::new(specialize_startup_expr(
                value,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
        ),
        Expr::FieldAccess(target, field) => Expr::FieldAccess(
            Box::new(specialize_startup_expr(
                target,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
            field.clone(),
        ),
        Expr::Pipe(lhs, rhs) => Expr::Pipe(
            Box::new(specialize_startup_expr(
                lhs,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
            Box::new(specialize_startup_expr(
                rhs,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
        ),
        Expr::If(condition, then_branch, else_branch) => Expr::If(
            Box::new(specialize_startup_expr(
                condition,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
            Box::new(specialize_startup_expr(
                then_branch,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
            else_branch
                .as_ref()
                .map(|else_branch| {
                    specialize_startup_expr(
                        else_branch,
                        bound_params,
                        startup_function,
                        functions,
                        specializations,
                        scopes,
                    )
                    .map(Box::new)
                })
                .transpose()?,
        ),
        Expr::Match(value, arms) => Expr::Match(
            Box::new(specialize_startup_expr(
                value,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
            arms.iter()
                .map(|arm| {
                    let mut bindings = BTreeSet::new();
                    pattern_bindings(&arm.pattern, &mut bindings);
                    scopes.push(bindings.into_iter().map(|name| (name, false)).collect());
                    let guard = arm
                        .guard
                        .as_ref()
                        .map(|guard| {
                            specialize_startup_expr(
                                guard,
                                bound_params,
                                startup_function,
                                functions,
                                specializations,
                                scopes,
                            )
                        })
                        .transpose()?;
                    let body = specialize_startup_expr(
                        &arm.body,
                        bound_params,
                        startup_function,
                        functions,
                        specializations,
                        scopes,
                    )?;
                    scopes.pop();
                    Ok(MatchArm {
                        pattern: arm.pattern.clone(),
                        guard,
                        body,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        Expr::Block(stmts, tail) => {
            scopes.push(BTreeMap::new());
            let mut rewritten_stmts = Vec::with_capacity(stmts.len());
            for stmt in stmts {
                match stmt {
                    Stmt::Let(name, annotation, value) => {
                        let value = specialize_startup_expr(
                            value,
                            bound_params,
                            startup_function,
                            functions,
                            specializations,
                            scopes,
                        )?;
                        let aliases_startup = expr_is_startup_alias(&value, startup_function);
                        scopes
                            .last_mut()
                            .unwrap()
                            .insert(name.clone(), aliases_startup);
                        // Alias-only bindings are tracked in scope and elided so the
                        // native backend never sees a first-class function value.
                        if !aliases_startup {
                            rewritten_stmts.push(Stmt::Let(
                                name.clone(),
                                annotation.clone(),
                                value,
                            ));
                        }
                    }
                    Stmt::Expr(expr) => rewritten_stmts.push(Stmt::Expr(specialize_startup_expr(
                        expr,
                        bound_params,
                        startup_function,
                        functions,
                        specializations,
                        scopes,
                    )?)),
                }
            }
            let tail = tail
                .as_ref()
                .map(|tail| {
                    specialize_startup_expr(
                        tail,
                        bound_params,
                        startup_function,
                        functions,
                        specializations,
                        scopes,
                    )
                    .map(Box::new)
                })
                .transpose()?;
            scopes.pop();
            Expr::Block(rewritten_stmts, tail)
        }
        Expr::Try(expr) => Expr::Try(Box::new(specialize_startup_expr(
            expr,
            bound_params,
            startup_function,
            functions,
            specializations,
            scopes,
        )?)),
        Expr::StructLit(name, fields) => Expr::StructLit(
            name.clone(),
            fields
                .iter()
                .map(|(field, value)| {
                    Ok((
                        field.clone(),
                        specialize_startup_expr(
                            value,
                            bound_params,
                            startup_function,
                            functions,
                            specializations,
                            scopes,
                        )?,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        Expr::Spawn(expr) => Expr::Spawn(Box::new(specialize_startup_expr(
            expr,
            bound_params,
            startup_function,
            functions,
            specializations,
            scopes,
        )?)),
        Expr::Await(expr) => Expr::Await(Box::new(specialize_startup_expr(
            expr,
            bound_params,
            startup_function,
            functions,
            specializations,
            scopes,
        )?)),
        Expr::ChannelNew { elem_type, buffer } => Expr::ChannelNew {
            elem_type: elem_type.clone(),
            buffer: Box::new(specialize_startup_expr(
                buffer,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
        },
        Expr::Return(value) => Expr::Return(
            value
                .as_ref()
                .map(|value| {
                    specialize_startup_expr(
                        value,
                        bound_params,
                        startup_function,
                        functions,
                        specializations,
                        scopes,
                    )
                    .map(Box::new)
                })
                .transpose()?,
        ),
        Expr::Throw(value) => Expr::Throw(Box::new(specialize_startup_expr(
            value,
            bound_params,
            startup_function,
            functions,
            specializations,
            scopes,
        )?)),
        Expr::List(items) => Expr::List(
            items
                .iter()
                .map(|item| {
                    specialize_startup_expr(
                        item,
                        bound_params,
                        startup_function,
                        functions,
                        specializations,
                        scopes,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Expr::ParallelScope { lanes, body } => Expr::ParallelScope {
            lanes: lanes
                .as_ref()
                .map(|lanes| {
                    specialize_startup_expr(
                        lanes,
                        bound_params,
                        startup_function,
                        functions,
                        specializations,
                        scopes,
                    )
                    .map(Box::new)
                })
                .transpose()?,
            body: Box::new(specialize_startup_expr(
                body,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
        },
        Expr::Select(arms) => Expr::Select(
            arms.iter()
                .map(|arm| match arm {
                    SelectArm::Recv {
                        binding,
                        source,
                        body,
                    } => {
                        let source = specialize_startup_expr(
                            source,
                            bound_params,
                            startup_function,
                            functions,
                            specializations,
                            scopes,
                        )?;
                        scopes.push([(binding.clone(), false)].into_iter().collect());
                        let body = specialize_startup_expr(
                            body,
                            bound_params,
                            startup_function,
                            functions,
                            specializations,
                            scopes,
                        )?;
                        scopes.pop();
                        Ok(SelectArm::Recv {
                            binding: binding.clone(),
                            source,
                            body,
                        })
                    }
                    SelectArm::Timeout { duration, body } => Ok(SelectArm::Timeout {
                        duration: specialize_startup_expr(
                            duration,
                            bound_params,
                            startup_function,
                            functions,
                            specializations,
                            scopes,
                        )?,
                        body: specialize_startup_expr(
                            body,
                            bound_params,
                            startup_function,
                            functions,
                            specializations,
                            scopes,
                        )?,
                    }),
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        Expr::Perform {
            effect,
            operation,
            args,
        } => Expr::Perform {
            effect: effect.clone(),
            operation: operation.clone(),
            args: args
                .iter()
                .map(|arg| {
                    specialize_startup_expr(
                        arg,
                        bound_params,
                        startup_function,
                        functions,
                        specializations,
                        scopes,
                    )
                    .map(Box::new)
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
        Expr::Handle { body, handlers } => Expr::Handle {
            body: Box::new(specialize_startup_expr(
                body,
                bound_params,
                startup_function,
                functions,
                specializations,
                scopes,
            )?),
            handlers: handlers
                .iter()
                .map(|handler| match handler {
                    HandleBinding::Use(use_handler) => {
                        Ok(HandleBinding::Use(sporec_parser::ast::HandlerUse {
                            handler: use_handler.handler.clone(),
                            payload: use_handler
                                .payload
                                .iter()
                                .map(|(name, value)| {
                                    Ok((
                                        name.clone(),
                                        specialize_startup_expr(
                                            value,
                                            bound_params,
                                            startup_function,
                                            functions,
                                            specializations,
                                            scopes,
                                        )?,
                                    ))
                                })
                                .collect::<Result<Vec<_>, String>>()?,
                        }))
                    }
                    HandleBinding::On(arm) => {
                        scopes.push(
                            arm.params
                                .iter()
                                .cloned()
                                .map(|name| (name, false))
                                .collect(),
                        );
                        let body = specialize_startup_expr(
                            &arm.body,
                            bound_params,
                            startup_function,
                            functions,
                            specializations,
                            scopes,
                        )?;
                        scopes.pop();
                        Ok(HandleBinding::On(sporec_parser::ast::EffectArm {
                            effect: arm.effect.clone(),
                            operation: arm.operation.clone(),
                            params: arm.params.clone(),
                            body: Box::new(body),
                        }))
                    }
                })
                .collect::<Result<Vec<_>, String>>()?,
        },
    })
}

fn ensure_startup_specialization(
    functions: &mut BTreeMap<String, FnDef>,
    specializations: &mut BTreeMap<String, String>,
    function_name: &str,
    bound_positions: &BTreeSet<usize>,
    startup_function: &str,
) -> Result<String, String> {
    let cache_key = startup_specialization_cache_key(function_name, bound_positions);
    if let Some(name) = specializations.get(&cache_key) {
        return Ok(name.clone());
    }

    let original = functions.get(function_name).cloned().ok_or_else(|| {
        format!("native project build could not specialize missing function `{function_name}`")
    })?;
    let body = original.body.as_ref().ok_or_else(|| {
        format!("native project build cannot specialize body-less function `{function_name}`")
    })?;

    let specialized_name = startup_specialization_name(function_name, bound_positions);
    specializations.insert(cache_key, specialized_name.clone());

    let bound_param_names = original
        .params
        .iter()
        .enumerate()
        .filter_map(|(index, param)| {
            bound_positions
                .contains(&index)
                .then_some(param.name.clone())
        })
        .collect::<BTreeSet<_>>();
    let retained_params = original
        .params
        .iter()
        .enumerate()
        .filter_map(|(index, param)| (!bound_positions.contains(&index)).then_some(param.clone()))
        .collect::<Vec<_>>();

    functions.insert(
        specialized_name.clone(),
        FnDef {
            name: specialized_name.clone(),
            params: retained_params.clone(),
            body: None,
            ..original.clone()
        },
    );

    let mut scopes = vec![
        retained_params
            .iter()
            .map(|param| (param.name.clone(), false))
            .collect(),
    ];
    let specialized_body = specialize_startup_expr(
        body,
        &bound_param_names,
        startup_function,
        functions,
        specializations,
        &mut scopes,
    )?;

    functions.insert(
        specialized_name.clone(),
        FnDef {
            name: specialized_name.clone(),
            params: retained_params,
            body: Some(specialized_body),
            ..original
        },
    );

    Ok(specialized_name)
}

fn collect_direct_calls(expr: &Expr, calls: &mut BTreeSet<String>) {
    match expr {
        Expr::Call(callee, args) => {
            if let Expr::Var(name) = callee.as_ref() {
                calls.insert(name.clone());
            } else {
                collect_direct_calls(callee, calls);
            }
            for arg in args {
                collect_direct_calls(arg, calls);
            }
        }
        Expr::Lambda(_, body)
        | Expr::Try(body)
        | Expr::Spawn(body)
        | Expr::Await(body)
        | Expr::Throw(body) => collect_direct_calls(body, calls),
        Expr::UnaryOp(_, value) | Expr::FieldAccess(value, _) | Expr::Return(Some(value)) => {
            collect_direct_calls(value, calls)
        }
        Expr::BinOp(lhs, _, rhs) | Expr::Pipe(lhs, rhs) => {
            collect_direct_calls(lhs, calls);
            collect_direct_calls(rhs, calls);
        }
        Expr::If(condition, then_branch, else_branch) => {
            collect_direct_calls(condition, calls);
            collect_direct_calls(then_branch, calls);
            if let Some(else_branch) = else_branch {
                collect_direct_calls(else_branch, calls);
            }
        }
        Expr::Match(value, arms) => {
            collect_direct_calls(value, calls);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_direct_calls(guard, calls);
                }
                collect_direct_calls(&arm.body, calls);
            }
        }
        Expr::Block(stmts, tail) => {
            for stmt in stmts {
                match stmt {
                    Stmt::Let(_, _, value) | Stmt::Expr(value) => {
                        collect_direct_calls(value, calls)
                    }
                }
            }
            if let Some(tail) = tail {
                collect_direct_calls(tail, calls);
            }
        }
        Expr::StructLit(_, fields) => {
            for (_, value) in fields {
                collect_direct_calls(value, calls);
            }
        }
        Expr::ChannelNew { buffer, .. } => collect_direct_calls(buffer, calls),
        Expr::List(items) => {
            for item in items {
                collect_direct_calls(item, calls);
            }
        }
        Expr::ParallelScope { lanes, body } => {
            if let Some(lanes) = lanes {
                collect_direct_calls(lanes, calls);
            }
            collect_direct_calls(body, calls);
        }
        Expr::Select(arms) => {
            for arm in arms {
                match arm {
                    SelectArm::Recv { source, body, .. } => {
                        collect_direct_calls(source, calls);
                        collect_direct_calls(body, calls);
                    }
                    SelectArm::Timeout { duration, body } => {
                        collect_direct_calls(duration, calls);
                        collect_direct_calls(body, calls);
                    }
                }
            }
        }
        Expr::Perform { args, .. } => {
            for arg in args {
                collect_direct_calls(arg, calls);
            }
        }
        Expr::Handle { body, handlers } => {
            collect_direct_calls(body, calls);
            for handler in handlers {
                match handler {
                    HandleBinding::Use(use_handler) => {
                        for (_, value) in &use_handler.payload {
                            collect_direct_calls(value, calls);
                        }
                    }
                    HandleBinding::On(arm) => collect_direct_calls(&arm.body, calls),
                }
            }
        }
        Expr::FString(parts) => {
            for part in parts {
                if let sporec_parser::ast::FStringPart::Expr(expr) = part {
                    collect_direct_calls(expr, calls);
                }
            }
        }
        Expr::TString(parts) => {
            for part in parts {
                if let sporec_parser::ast::TStringPart::Expr(expr) = part {
                    collect_direct_calls(expr, calls);
                }
            }
        }
        Expr::IntLit(_)
        | Expr::FloatLit(_)
        | Expr::StrLit(_)
        | Expr::BoolLit(_)
        | Expr::Var(_)
        | Expr::Hole(_, _, _, _)
        | Expr::Return(None)
        | Expr::Placeholder => {}
    }
}

fn reachable_native_functions(
    functions: &BTreeMap<String, FnDef>,
    entrypoint: &str,
) -> BTreeSet<String> {
    let mut reachable = BTreeSet::new();
    let mut stack = vec![entrypoint.to_string()];
    while let Some(name) = stack.pop() {
        if !reachable.insert(name.clone()) {
            continue;
        }
        let Some(function) = functions.get(&name) else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let mut calls = BTreeSet::new();
        collect_direct_calls(body, &mut calls);
        for callee in calls {
            if functions.contains_key(&callee) {
                stack.push(callee);
            }
        }
    }
    reachable
}

fn build_native_project_module(
    prep: &PreparedProject,
    target: &ResolvedProjectTarget,
) -> Result<Module, String> {
    let startup_name = target.startup_function.as_deref().ok_or_else(|| {
        format!(
            "entry path `{}` is not runnable: no platform startup contract is bound",
            target.entry_path
        )
    })?;
    let modules = collect_native_project_modules(prep, target)?;
    let entry_module = entry_module_name(&target.entry_path);
    let qualified_startup = native_project_function_name(&entry_module, startup_name);

    let mut transformed_functions = BTreeMap::new();
    for (module_path, module) in &modules {
        let local_functions = collect_module_function_renames(module_path, module);
        let imported_functions = collect_imported_function_bindings(module_path, module, &modules)?;
        for item in &module.items {
            let Item::Function(function) = item else {
                continue;
            };
            let rewritten = rewrite_native_project_function(
                module_path,
                function,
                &local_functions,
                &imported_functions,
            );
            transformed_functions.insert(rewritten.name.clone(), rewritten);
        }
    }

    let startup_def = transformed_functions
        .get(&qualified_startup)
        .ok_or_else(|| {
            format!("native project build could not find startup function `{qualified_startup}`")
        })?;
    if !startup_def.params.is_empty() {
        return Err(format!(
            "native project build requires a zero-argument startup function, found `{qualified_startup}` with {} parameter(s)",
            startup_def.params.len()
        ));
    }

    let mut startup_specializations = BTreeMap::new();
    let wrapper = if let Some(contract) = target.platform_contract.as_ref() {
        let qualified_adapter =
            native_project_function_name(&contract.contract_module, &contract.adapter_function);
        let adapter_def = transformed_functions
            .get(&qualified_adapter)
            .ok_or_else(|| {
                format!("native project build could not find startup adapter `{qualified_adapter}`")
            })?;
        if adapter_def.params.len() != 1 {
            return Err(format!(
                "native project build requires startup adapter `{qualified_adapter}` to take exactly one app function parameter"
            ));
        }
        let adapter_return_type = adapter_def.return_type.clone();
        let adapter_startup_positions = [0usize].into_iter().collect::<BTreeSet<_>>();
        let specialized_adapter = ensure_startup_specialization(
            &mut transformed_functions,
            &mut startup_specializations,
            &qualified_adapter,
            &adapter_startup_positions,
            &qualified_startup,
        )?;
        FnDef {
            name: "main".to_string(),
            visibility: Visibility::Pub,
            type_params: Vec::new(),
            params: Vec::new(),
            return_type: adapter_return_type,
            errors: Vec::new(),
            where_clause: None,
            cost_clause: None,
            spec_clause: None,
            uses_clause: None,
            is_unbounded: false,
            hole_allows: None,
            is_foreign: false,
            body: Some(Expr::Call(
                Box::new(Expr::Var(specialized_adapter)),
                Vec::new(),
            )),
            span: None,
        }
    } else {
        FnDef {
            name: "main".to_string(),
            visibility: Visibility::Pub,
            type_params: Vec::new(),
            params: Vec::new(),
            return_type: startup_def.return_type.clone(),
            errors: Vec::new(),
            where_clause: None,
            cost_clause: None,
            spec_clause: None,
            uses_clause: None,
            is_unbounded: false,
            hole_allows: None,
            is_foreign: false,
            body: Some(Expr::Call(
                Box::new(Expr::Var(qualified_startup)),
                Vec::new(),
            )),
            span: None,
        }
    };

    transformed_functions.insert("main".to_string(), wrapper);
    let reachable = reachable_native_functions(&transformed_functions, "main");
    let mut items = Vec::with_capacity(reachable.len());
    items.push(Item::Function(
        transformed_functions
            .remove("main")
            .expect("wrapper main should exist"),
    ));
    for name in reachable.into_iter().filter(|name| name != "main") {
        if let Some(function) = transformed_functions.remove(&name) {
            items.push(Item::Function(function));
        }
    }

    Ok(Module {
        name: "spore.native.project".to_string(),
        items,
        comments: Vec::new(),
    })
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

/// Build a runnable native object for a project entry by flattening its resolved
/// import closure into a synthetic import-free module and binding its startup to
/// an exported `main` symbol.
pub fn build_project_native_object(root: &Path, entry: &str) -> Result<Vec<u8>, String> {
    let target = resolve_project_target_by_path(root, entry)?;
    let prep = prepare_project(root, &target)?;
    let _results = collect_prepared_project_results(&prep, &target.entry_path)?;
    validate_project_startup(&prep, &target)?;
    let module = build_native_project_module(&prep, &target)?;
    sporec_codegen::emit_native_object(&module).map_err(|error| error.to_string())
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

/// Run spec clauses in a project module's entry file, with all imported modules
/// available to spec bodies.
///
/// This is the project-aware counterpart of `sporec_driver::test_specs`: it
/// type-checks the project (resolving imports), then executes spec clauses with
/// the imported modules pre-loaded in the interpreter so helpers defined in
/// those modules are callable from spec bodies.
///
/// Returns `Ok(specs)` on success or a human-readable error string on failure.
pub fn test_specs_project(
    root: &Path,
    entry: &str,
) -> Result<Vec<sporec_codegen::SpecResult>, String> {
    let target = resolve_project_target_by_path(root, entry)?;
    let prep = prepare_project(root, &target)?;
    let _results = collect_prepared_project_results(&prep, &target.entry_path)?;

    let imported = collect_runtime_import_modules(&prep, &target)?;
    sporec_codegen::test_specs_with_imports(&prep.ast, &imported).map_err(|e| e.to_string())
}
