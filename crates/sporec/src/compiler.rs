use spore_codegen::value::Value;
use spore_parser::parse;
use spore_typeck::module::ModuleRegistry;
use spore_typeck::{type_check, type_check_with_registry};

/// Compile Spore source code to output.
///
/// This is the core compiler pipeline:
/// 1. Parse (source text → AST)
/// 2. Type check (AST → Typed AST)
/// 3. Code gen (Typed AST → native code)
pub fn compile(source: &str) -> Result<(), String> {
    let ast = parse(source).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let _result = type_check(&ast).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok(())
}

/// Compile multiple Spore source files together with shared module resolution.
///
/// 1. Parses each source into an AST
/// 2. Builds a ModuleRegistry from all modules
/// 3. Type-checks each module with access to the shared registry
pub fn compile_files(paths: &[&str]) -> Result<(), String> {
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
    for (path, ast) in &modules {
        if let Err(errs) = type_check_with_registry(ast, registry.clone()) {
            for e in errs {
                all_errors.push(format!("{path}: {e}"));
            }
        }
    }

    if all_errors.is_empty() {
        Ok(())
    } else {
        Err(all_errors.join("\n"))
    }
}

/// Analyze holes in Spore source and return a JSON report.
pub fn holes(source: &str) -> Result<String, String> {
    let ast = parse(source).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let result = type_check(&ast).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok(result.hole_report.to_json())
}

/// Run a Spore program by executing its `main` function.
pub fn run(source: &str) -> Result<Value, String> {
    let ast = parse(source).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let _result = type_check(&ast).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    spore_codegen::run(&ast).map_err(|e| e.to_string())
}
