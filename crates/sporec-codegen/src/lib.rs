/// sporec-codegen — Spore code generation / execution
///
/// PoC: tree-walking interpreter for direct AST evaluation.
/// Future native backends can be added without exposing unused scaffolding today.
pub mod effect_handler;
pub mod interpret;
pub mod native;
pub mod value;

use effect_handler::{CliPlatformHandler, PackageHostHandler, RuntimeSignal};
use interpret::{Interpreter, RuntimeError};
use sporec_parser::ast::{Module, SpecItem, TypeExpr};
use value::Value;

pub use effect_handler::{RuntimePlatform, RuntimeSignal as ProjectRuntimeSignal};
pub use native::{
    NativeError, NativeProgram, call_native, compile_native, emit_native_object, run_native,
};

/// Result of evaluating a single spec clause.
#[derive(Debug, Clone)]
pub struct SpecResult {
    pub fn_name: String,
    pub label: String,
    pub kind: SpecKind,
    pub passed: bool,
    pub error: Option<String>,
}

/// What kind of spec clause was evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecKind {
    Example,
    Property,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectRunOutcome {
    Completed(Value),
    Exited(u8),
}

/// Execute a Spore module by calling its current default startup function
/// (`main`).
pub fn run(module: &Module) -> Result<Value, RuntimeError> {
    let mut interp = Interpreter::new();
    interp.register_effect_handler(Box::new(CliPlatformHandler));
    interp.load_prelude();
    interp.load_module(module);
    interp.call_function("main", vec![])
}

/// Execute a named function with arguments.
pub fn call(module: &Module, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let mut interp = Interpreter::new();
    interp.register_effect_handler(Box::new(CliPlatformHandler));
    interp.load_prelude();
    interp.load_module(module);
    interp.call_function(name, args)
}

fn register_project_runtime_handler(interp: &mut Interpreter, runtime_platform: RuntimePlatform) {
    match runtime_platform {
        RuntimePlatform::Cli => interp.register_effect_handler(Box::new(CliPlatformHandler)),
        RuntimePlatform::PackageHost => {
            // Registered by `project_interpreter`, which has access to the module graph.
        }
    }
}

fn project_interpreter(
    entry: &Module,
    imports: &[(String, Module)],
    runtime_platform: RuntimePlatform,
) -> Interpreter {
    let mut interp = Interpreter::new();
    register_project_runtime_handler(&mut interp, runtime_platform);
    if matches!(runtime_platform, RuntimePlatform::PackageHost) {
        interp.register_effect_handler(Box::new(PackageHostHandler::from_modules(entry, imports)));
    }
    interp.load_prelude();

    for (path, module) in imports {
        interp.load_module_functions(path, module);
    }

    interp.load_module(entry);
    interp
}

/// Execute a Spore project with cross-module imports.
///
/// Loads imported modules first (making their public symbols available),
/// then loads the entry module and calls the resolved startup function.
pub fn run_project(
    entry: &Module,
    imports: &[(String, Module)],
    startup_function: &str,
) -> Result<Value, RuntimeError> {
    run_project_on_platform(entry, imports, startup_function, RuntimePlatform::Cli)
}

/// Execute a Spore project against a selected runtime host profile.
pub fn run_project_on_platform(
    entry: &Module,
    imports: &[(String, Module)],
    startup_function: &str,
    runtime_platform: RuntimePlatform,
) -> Result<Value, RuntimeError> {
    match run_project_with_outcome_on_platform(entry, imports, startup_function, runtime_platform)?
    {
        ProjectRunOutcome::Completed(value) => Ok(value),
        ProjectRunOutcome::Exited(code) => Err(RuntimeError::signal(RuntimeSignal::Exit(code))),
    }
}

/// Execute a Spore project by routing startup through a platform adapter.
pub fn run_project_with_adapter(
    entry: &Module,
    imports: &[(String, Module)],
    startup_function: &str,
    adapter_function: &str,
) -> Result<Value, RuntimeError> {
    run_project_with_adapter_on_platform(
        entry,
        imports,
        startup_function,
        adapter_function,
        RuntimePlatform::Cli,
    )
}

/// Execute a Spore project through a platform adapter and runtime host profile.
pub fn run_project_with_adapter_on_platform(
    entry: &Module,
    imports: &[(String, Module)],
    startup_function: &str,
    adapter_function: &str,
    runtime_platform: RuntimePlatform,
) -> Result<Value, RuntimeError> {
    match run_project_with_adapter_outcome_on_platform(
        entry,
        imports,
        startup_function,
        adapter_function,
        runtime_platform,
    )? {
        ProjectRunOutcome::Completed(value) => Ok(value),
        ProjectRunOutcome::Exited(code) => Err(RuntimeError::signal(RuntimeSignal::Exit(code))),
    }
}

pub fn run_project_with_outcome(
    entry: &Module,
    imports: &[(String, Module)],
    startup_function: &str,
) -> Result<ProjectRunOutcome, RuntimeError> {
    run_project_with_outcome_on_platform(entry, imports, startup_function, RuntimePlatform::Cli)
}

pub fn run_project_with_outcome_on_platform(
    entry: &Module,
    imports: &[(String, Module)],
    startup_function: &str,
    runtime_platform: RuntimePlatform,
) -> Result<ProjectRunOutcome, RuntimeError> {
    let mut interp = project_interpreter(entry, imports, runtime_platform);
    project_run_outcome(interp.call_function(startup_function, vec![]))
}

pub fn run_project_with_adapter_outcome(
    entry: &Module,
    imports: &[(String, Module)],
    startup_function: &str,
    adapter_function: &str,
) -> Result<ProjectRunOutcome, RuntimeError> {
    run_project_with_adapter_outcome_on_platform(
        entry,
        imports,
        startup_function,
        adapter_function,
        RuntimePlatform::Cli,
    )
}

pub fn run_project_with_adapter_outcome_on_platform(
    entry: &Module,
    imports: &[(String, Module)],
    startup_function: &str,
    adapter_function: &str,
    runtime_platform: RuntimePlatform,
) -> Result<ProjectRunOutcome, RuntimeError> {
    let mut interp = project_interpreter(entry, imports, runtime_platform);
    let app_main = interp.named_function_value(startup_function)?;
    project_run_outcome(interp.call_function(adapter_function, vec![app_main]))
}

fn project_run_outcome(
    result: Result<Value, RuntimeError>,
) -> Result<ProjectRunOutcome, RuntimeError> {
    match result {
        Ok(value) => Ok(ProjectRunOutcome::Completed(value)),
        Err(error) => match error.runtime_signal() {
            Some(RuntimeSignal::Exit(code)) => Ok(ProjectRunOutcome::Exited(code)),
            None => Err(error),
        },
    }
}

/// Generate test input values for a given type.
///
/// Refinement types reuse the base type's sample values and keep only those
/// that satisfy the `when` predicate.
fn test_values_for_type(interp: &mut Interpreter, ty: &TypeExpr) -> Vec<Value> {
    match ty {
        TypeExpr::Named(name) => match name.as_str() {
            "I8" | "I16" | "I32" | "I64" => vec![
                Value::Int(0),
                Value::Int(1),
                Value::Int(-1),
                Value::Int(42),
                Value::Int(100),
            ],
            "U8" | "U16" | "U32" | "U64" => vec![
                Value::Int(0),
                Value::Int(1),
                Value::Int(42),
                Value::Int(100),
            ],
            "Bool" => vec![Value::Bool(true), Value::Bool(false)],
            "Str" => vec![Value::Str(String::new()), Value::Str("hello".into())],
            "F32" | "F64" => vec![Value::Float(0.0), Value::Float(1.0), Value::Float(-1.0)],
            _ => vec![],
        },
        TypeExpr::Refinement(base, binding, predicate) => test_values_for_type(interp, base)
            .into_iter()
            .filter(|candidate| {
                matches!(
                    interp.eval_expr_with_bindings(
                        predicate,
                        &[(binding.clone(), candidate.clone())],
                    ),
                    Ok(Value::Bool(true))
                )
            })
            .collect(),
        _ => vec![],
    }
}

/// Build the cartesian product of test value lists for each parameter.
fn cartesian_product(param_values: &[Vec<Value>]) -> Vec<Vec<Value>> {
    if param_values.is_empty() {
        return vec![vec![]];
    }
    let mut result = vec![vec![]];
    for values in param_values {
        let mut next = Vec::new();
        for combo in &result {
            for val in values {
                let mut extended = combo.clone();
                extended.push(val.clone());
                next.push(extended);
            }
        }
        result = next;
    }
    result
}

/// Run all spec clauses in a module.
///
/// For each function with a `spec` block:
/// - Examples: evaluate the body expression; pass if result is `Bool(true)`
/// - Properties: evaluate the lambda to get a closure, call both the property
///   and the function under test with generated inputs, and require the
///   returned values to match for every sampled input
pub fn test_specs(module: &Module) -> Result<Vec<SpecResult>, RuntimeError> {
    test_specs_with_imports(module, &[])
}

/// Run all spec clauses in a project module's entry file, with imported modules
/// pre-loaded so that spec bodies can call helper functions defined in those
/// modules.
///
/// This is the project-aware counterpart of [`test_specs`]: callers provide the
/// parsed entry module together with all of its imported modules (as returned by
/// `collect_runtime_import_modules`), and the interpreter loads them in the same
/// order that `run_project_with_outcome_on_platform` uses.
pub fn test_specs_with_imports(
    module: &Module,
    imports: &[(String, Module)],
) -> Result<Vec<SpecResult>, RuntimeError> {
    let mut interp = Interpreter::new();
    interp.register_effect_handler(Box::new(CliPlatformHandler));
    interp.load_prelude();
    // Load imported helper modules first so their public symbols are visible to
    // spec bodies in the entry module.
    for (path, import_module) in imports {
        interp.load_module_functions(path, import_module);
    }
    interp.load_module(module);

    run_specs_on_interpreter(&mut interp)
}

/// Execute all spec clauses on a fully-set-up interpreter, collecting results.
fn run_specs_on_interpreter(interp: &mut Interpreter) -> Result<Vec<SpecResult>, RuntimeError> {
    let specs = interp.functions_with_specs();
    let mut results = Vec::new();

    for (fn_name, fndef) in &specs {
        let spec = fndef.spec_clause.as_ref().unwrap();
        for item in &spec.items {
            match item {
                SpecItem::Example(ex) => {
                    let result = interp.eval_expr(&ex.body);
                    let (passed, error) = match result {
                        Ok(Value::Bool(true)) => (true, None),
                        Ok(Value::Bool(false)) => (false, Some("returned false".into())),
                        Ok(other) => (
                            false,
                            Some(format!("expected Bool, got {}: {other}", other.type_name())),
                        ),
                        Err(e) => (false, Some(e.message.clone())),
                    };
                    results.push(SpecResult {
                        fn_name: fn_name.clone(),
                        label: ex.label.clone(),
                        kind: SpecKind::Example,
                        passed,
                        error,
                    });
                }
                SpecItem::Property(prop) => {
                    let closure_result = interp.eval_expr(&prop.predicate);
                    match closure_result {
                        Ok(Value::Closure(closure)) => {
                            let param_types: Vec<&TypeExpr> =
                                if let sporec_parser::ast::Expr::Lambda(params, _) =
                                    prop.predicate.as_ref()
                                {
                                    params.iter().map(|p| &p.ty).collect()
                                } else {
                                    vec![]
                                };

                            let mut param_value_lists = Vec::with_capacity(param_types.len());
                            for ty in &param_types {
                                param_value_lists.push(test_values_for_type(interp, ty));
                            }

                            let combos = cartesian_product(&param_value_lists);

                            if combos.is_empty() || combos.iter().all(|c| c.is_empty()) {
                                results.push(SpecResult {
                                    fn_name: fn_name.clone(),
                                    label: prop.label.clone(),
                                    kind: SpecKind::Property,
                                    passed: true,
                                    error: Some("no test inputs generated (skipped)".into()),
                                });
                                continue;
                            }

                            let mut all_passed = true;
                            let mut first_error = None;

                            for combo in &combos {
                                let expected_result = interp.call_value_pub(
                                    &Value::Closure(closure.clone()),
                                    combo.clone(),
                                );

                                let expected = match expected_result {
                                    Ok(value) => value,
                                    Err(e) => {
                                        all_passed = false;
                                        first_error = Some(e.message.clone());
                                        break;
                                    }
                                };

                                let actual_result = interp.call_function(fn_name, combo.clone());
                                match actual_result {
                                    Ok(actual) if actual == expected => {}
                                    Ok(actual) => {
                                        all_passed = false;
                                        let args_str: Vec<String> =
                                            combo.iter().map(|v| format!("{v}")).collect();
                                        first_error = Some(format!(
                                            "failed for ({}) : expected {}, got {}",
                                            args_str.join(", "),
                                            expected,
                                            actual
                                        ));
                                        break;
                                    }
                                    Err(e) => {
                                        all_passed = false;
                                        first_error = Some(e.message.clone());
                                        break;
                                    }
                                }
                            }

                            results.push(SpecResult {
                                fn_name: fn_name.clone(),
                                label: prop.label.clone(),
                                kind: SpecKind::Property,
                                passed: all_passed,
                                error: first_error,
                            });
                        }
                        Ok(other) => {
                            results.push(SpecResult {
                                fn_name: fn_name.clone(),
                                label: prop.label.clone(),
                                kind: SpecKind::Property,
                                passed: false,
                                error: Some(format!(
                                    "predicate did not evaluate to a closure, got {}",
                                    other.type_name()
                                )),
                            });
                        }
                        Err(e) => {
                            results.push(SpecResult {
                                fn_name: fn_name.clone(),
                                label: prop.label.clone(),
                                kind: SpecKind::Property,
                                passed: false,
                                error: Some(e.message.clone()),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(results)
}
