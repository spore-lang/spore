/// sporec-typeck — Spore type checker and analysis
///
/// Performs type checking, capability verification, and cost analysis.
pub mod capability;
pub mod check;
pub mod concurrency;
pub mod cost;
pub mod env;
pub mod error;
pub mod hir;
pub mod hole;
pub mod incremental;
pub mod lower;
pub mod module;
pub mod platform;
pub mod refinement;
pub mod sig_hash;
pub mod types;

use std::collections::HashMap;

use check::Checker;
use cost::{CostAnalyzer, CostChecker, CostResult, CostVector};
use error::{ErrorCode, TypeError};
use hole::HoleReport;
use module::{ModuleRegistry, PreludeOptions};
use sporec_parser::ast::Module;

pub fn is_synthetic_hole_name(name: &str) -> bool {
    matches!(
        name.strip_prefix("_hole"),
        Some(suffix) if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
    )
}

fn parse_embedded_prelude() -> Module {
    let source = include_str!("../../../stdlib/prelude.sp");
    sporec_parser::parse(source).expect("embedded stdlib/prelude.sp must parse")
}

/// Result of a successful type check, including hole report and cost analysis.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub hole_report: HoleReport,
    pub cost_results: HashMap<String, CostResult>,
    pub cost_vectors: HashMap<String, CostVector>,
    /// Cost budget warnings (SEP-0004: violations are warnings, not errors).
    pub warnings: Vec<TypeError>,
}

/// Lower an AST module to HIR.
pub fn lower(module: &Module) -> hir::HirModule {
    let mut lowering = lower::Lowering::new();
    lowering.lower_module(module)
}

/// Type-check a parsed Spore module, returning a CheckResult or all errors found.
pub fn type_check(module: &Module) -> Result<CheckResult, Vec<TypeError>> {
    type_check_with_registry(module, ModuleRegistry::new())
}

/// Type-check a parsed Spore module with a shared module registry.
pub fn type_check_with_registry(
    module: &Module,
    registry: ModuleRegistry,
) -> Result<CheckResult, Vec<TypeError>> {
    type_check_with_registry_and_prelude(module, registry, PreludeOptions::default())
}

/// Type-check a parsed Spore module with a shared module registry and custom prelude options.
pub fn type_check_with_registry_and_prelude(
    module: &Module,
    mut registry: ModuleRegistry,
    prelude_options: PreludeOptions,
) -> Result<CheckResult, Vec<TypeError>> {
    registry.register_prelude_with_options(prelude_options);
    let mut checker = Checker::with_module_registry(registry);
    checker.load_prelude(&parse_embedded_prelude());
    checker.check_module(module);

    // Run cost analysis (independent of type checking)
    let mut cost_analyzer = CostAnalyzer::new();
    cost_analyzer.analyze_module(module);

    // Build four-dimensional cost vectors
    let mut cost_checker = CostChecker::new();
    cost_checker.check_all(&cost_analyzer);

    // Convert cost budget violations into K0101 warnings (SEP-0004)
    let mut warnings = Vec::new();
    for (fn_name, declared, actual) in cost_analyzer.violations() {
        warnings.push(TypeError::new(
            ErrorCode::K0101,
            format!(
                "function `{fn_name}` exceeds its declared cost budget: \
                 actual {actual} exceeds declared {declared}"
            ),
        ));
    }

    if checker.errors.is_empty() {
        Ok(CheckResult {
            hole_report: checker.hole_report,
            cost_results: cost_analyzer.results().clone(),
            cost_vectors: cost_checker.costs,
            warnings,
        })
    } else {
        Err(checker.errors)
    }
}

/// Build a `ModuleInterface` from a parsed module (for multi-file compilation).
pub fn build_module_interface(module: &Module) -> module::ModuleInterface {
    use module::{ModuleInterface, SymbolVisibility};
    use sporec_parser::ast::Item;

    let path: Vec<String> = if module.name.is_empty() {
        Vec::new()
    } else {
        module.name.split('.').map(|s| s.to_string()).collect()
    };
    let mut iface = ModuleInterface::new(path);

    let mut checker = Checker::new();
    let aliases: Vec<_> = module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Alias(alias_def) => Some(alias_def),
            _ => None,
        })
        .collect();
    for _ in 0..aliases.len() {
        let mut changed = false;
        for alias_def in &aliases {
            let resolved = checker.resolve_type(&alias_def.target);
            let previous = checker
                .registry
                .type_aliases
                .insert(alias_def.name.clone(), resolved.clone());
            if previous.as_ref() != Some(&resolved) {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for item in &module.items {
        match item {
            Item::Function(f) => {
                let param_tys: Vec<types::Ty> = f
                    .params
                    .iter()
                    .map(|p| checker.resolve_type(&p.ty))
                    .collect();
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(|t| checker.resolve_type(t))
                    .unwrap_or(types::Ty::Unit);
                iface.functions.insert(f.name.clone(), (param_tys, ret_ty));
                iface.function_caps.insert(
                    f.name.clone(),
                    checker.declared_capabilities(f.uses_clause.as_ref()),
                );
                if !f.errors.is_empty() {
                    let error_set: types::ErrorSet = f
                        .errors
                        .iter()
                        .filter_map(|te| match te {
                            sporec_parser::ast::TypeExpr::Named(name) => Some(name.clone()),
                            _ => None,
                        })
                        .collect();
                    iface.function_errors.insert(f.name.clone(), error_set);
                }
                let mut type_params = f.type_params.clone();
                if let Some(wc) = &f.where_clause {
                    type_params.extend(wc.constraints.iter().map(|c| c.type_var.clone()));
                    if !wc.constraints.is_empty() {
                        iface.function_where_bounds.insert(
                            f.name.clone(),
                            wc.constraints
                                .iter()
                                .map(|c| (c.type_var.clone(), c.bound.clone()))
                                .collect(),
                        );
                    }
                }
                type_params.sort();
                type_params.dedup();
                if !type_params.is_empty() {
                    iface
                        .function_type_params
                        .insert(f.name.clone(), type_params);
                }
                iface.set_visibility(&f.name, SymbolVisibility::from(&f.visibility));
            }
            Item::StructDef(s) => {
                let fields: Vec<(String, types::Ty)> = s
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), checker.resolve_type(&f.ty)))
                    .collect();
                iface.structs.insert(s.name.clone(), fields);
                if !s.type_params.is_empty() {
                    iface
                        .struct_type_params
                        .insert(s.name.clone(), s.type_params.clone());
                }
                iface.set_visibility(&s.name, SymbolVisibility::from(&s.visibility));
            }
            Item::TypeDef(t) => {
                let variants: Vec<(String, Vec<types::Ty>)> = t
                    .variants
                    .iter()
                    .map(|v| {
                        let ftys: Vec<types::Ty> =
                            v.fields.iter().map(|f| checker.resolve_type(f)).collect();
                        (v.name.clone(), ftys)
                    })
                    .collect();
                iface.types.insert(t.name.clone(), variants);
                iface.set_visibility(&t.name, SymbolVisibility::from(&t.visibility));
            }
            Item::CapabilityDef(cap) => {
                iface.capabilities.insert(cap.name.clone());
                iface.capability_methods.insert(
                    cap.name.clone(),
                    (
                        cap.type_params.clone(),
                        cap.methods
                            .iter()
                            .map(|method| {
                                let param_tys = method
                                    .params
                                    .iter()
                                    .map(|param| checker.resolve_type(&param.ty))
                                    .collect();
                                let ret_ty = method
                                    .return_type
                                    .as_ref()
                                    .map(|ty| checker.resolve_type(ty))
                                    .unwrap_or(types::Ty::Unit);
                                (method.name.clone(), param_tys, ret_ty)
                            })
                            .collect(),
                    ),
                );
                iface.set_visibility(&cap.name, SymbolVisibility::from(&cap.visibility));
            }
            Item::TraitDef(trait_def) => {
                iface.capabilities.insert(trait_def.name.clone());
                iface.capability_methods.insert(
                    trait_def.name.clone(),
                    (
                        trait_def.type_params.clone(),
                        trait_def
                            .methods
                            .iter()
                            .map(|method| {
                                let param_tys = method
                                    .params
                                    .iter()
                                    .map(|param| checker.resolve_type(&param.ty))
                                    .collect();
                                let ret_ty = method
                                    .return_type
                                    .as_ref()
                                    .map(|ty| checker.resolve_type(ty))
                                    .unwrap_or(types::Ty::Unit);
                                (method.name.clone(), param_tys, ret_ty)
                            })
                            .collect(),
                    ),
                );
                iface.set_visibility(
                    &trait_def.name,
                    SymbolVisibility::from(&trait_def.visibility),
                );
            }
            Item::EffectDef(effect) => {
                iface.capabilities.insert(effect.name.clone());
                iface.capability_methods.insert(
                    effect.name.clone(),
                    (
                        vec![],
                        effect
                            .operations
                            .iter()
                            .map(|operation| {
                                let param_tys = operation
                                    .params
                                    .iter()
                                    .map(|param| checker.resolve_type(&param.ty))
                                    .collect();
                                let ret_ty = operation
                                    .return_type
                                    .as_ref()
                                    .map(|ty| checker.resolve_type(ty))
                                    .unwrap_or(types::Ty::Unit);
                                (operation.name.clone(), param_tys, ret_ty)
                            })
                            .collect(),
                    ),
                );
                iface.set_visibility(&effect.name, SymbolVisibility::from(&effect.visibility));
            }
            Item::HandlerDef(handler) => {
                let fields = handler
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), checker.resolve_type(&field.ty)))
                    .collect();
                let methods = handler
                    .methods
                    .iter()
                    .map(|method| {
                        let param_tys = method
                            .params
                            .iter()
                            .map(|param| checker.resolve_type(&param.ty))
                            .collect();
                        let ret_ty = method
                            .return_type
                            .as_ref()
                            .map(|ty| checker.resolve_type(ty))
                            .unwrap_or(types::Ty::Unit);
                        (method.name.clone(), param_tys, ret_ty)
                    })
                    .collect();
                iface.handlers.insert(
                    handler.name.clone(),
                    env::HandlerInfo {
                        effect: handler.effect.clone(),
                        fields,
                        methods,
                    },
                );
            }
            _ => {}
        }
    }

    iface
}
