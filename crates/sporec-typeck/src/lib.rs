pub mod budget;
pub mod check;
pub mod concurrency;
/// sporec-typeck — Spore type checker and analysis
///
/// Performs type checking, effect verification, and budget analysis.
pub mod effect_set;
pub mod env;
pub mod error;
pub mod hir;
pub mod hole;
pub mod incremental;
pub mod intent;
pub mod lower;
pub mod module;
pub mod platform;
pub mod refinement;
pub mod sig_hash;
pub mod types;

use budget::{check_module_budget_errors, enrich_hole_report_with_budgets};
use check::Checker;
use error::TypeError;
use hole::HoleReport;
use intent::enrich_hole_report_with_properties;
use module::{ModuleRegistry, PreludeOptions};
use sporec_parser::ast::Module;
use sporec_stdlib::prelude;

pub fn is_synthetic_hole_name(name: &str) -> bool {
    matches!(
        name.strip_prefix("_hole"),
        Some(suffix) if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
    )
}

fn parse_embedded_prelude() -> Module {
    sporec_parser::parse(prelude().source).expect("embedded stdlib prelude must parse")
}

/// Result of a successful type check, including hole reports and warnings.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub hole_report: HoleReport,
    /// Non-fatal checker diagnostics.
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

    enrich_hole_report_with_budgets(module, &mut checker.hole_report);
    enrich_hole_report_with_properties(module, &mut checker.hole_report);

    let warnings = Vec::new();
    checker.errors.extend(check_module_budget_errors(module));

    if checker.errors.is_empty() {
        Ok(CheckResult {
            hole_report: checker.hole_report,
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
    for item in &module.items {
        if matches!(item, Item::SurfaceDef(_)) {
            checker.register_item(item);
        }
    }
    let aliases: Vec<_> = module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Alias(alias_def) => Some(alias_def),
            _ => None,
        })
        .collect();
    for _ in 0..aliases.len() {
        for alias_def in &aliases {
            checker.register_item(&Item::Alias((*alias_def).clone()));
        }
    }
    iface.type_aliases = checker.registry.type_aliases.clone();
    iface.generic_type_aliases = checker.registry.generic_type_aliases.clone();
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
                iface.function_required_effects.insert(
                    f.name.clone(),
                    checker.declared_effects(f.uses_clause.as_ref()),
                );
                let mut type_params = f.type_params.clone();
                type_params.extend(f.type_param_bounds.iter().map(|c| c.type_var.clone()));
                let generic_bounds = f
                    .type_param_bounds
                    .iter()
                    .map(|c| (c.type_var.clone(), c.bound.clone()))
                    .collect::<Vec<_>>();
                if !generic_bounds.is_empty() {
                    iface
                        .function_generic_bounds
                        .insert(f.name.clone(), generic_bounds);
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
            Item::OpaqueType(t) => {
                iface
                    .opaque_types
                    .insert(t.name.clone(), t.type_params.clone());
                iface.set_visibility(&t.name, SymbolVisibility::from(&t.visibility));
            }
            Item::Alias(alias) => {
                iface.set_visibility(&alias.name, SymbolVisibility::from(&alias.visibility));
            }
            Item::TraitDef(trait_def) => {
                iface.interfaces.insert(trait_def.name.clone());
                iface.interface_members.insert(
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
                iface.interfaces.insert(effect.name.clone());
                iface.effects.insert(effect.name.clone());
                iface
                    .effect_type_params
                    .insert(effect.name.clone(), effect.type_params.clone());
                iface.interface_members.insert(
                    effect.name.clone(),
                    (
                        effect.type_params.clone(),
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
            Item::SurfaceDef(surface) => {
                iface.surfaces.insert(
                    surface.name.clone(),
                    checker.declared_effects(Some(&sporec_parser::ast::UsesClause {
                        surface: surface.surface.clone(),
                    })),
                );
                iface
                    .surface_type_params
                    .insert(surface.name.clone(), surface.type_params.clone());
                iface.set_visibility(&surface.name, SymbolVisibility::from(&surface.visibility));
            }
            Item::HandlerDef(handler) => {
                let mut methods = std::collections::HashMap::new();
                for handler_impl in &handler.impls {
                    let impl_methods = handler_impl
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
                    methods.insert(handler_impl.effect.clone(), impl_methods);
                }
                iface.handlers.insert(
                    handler.name.clone(),
                    env::HandlerInfo {
                        handled_effects: checker.declared_effects(Some(
                            &sporec_parser::ast::UsesClause {
                                surface: handler.surface.clone(),
                            },
                        )),
                        uses_effects: types::EffectSet::new(),
                        fields: Vec::new(),
                        methods,
                    },
                );
                iface.set_visibility(&handler.name, SymbolVisibility::from(&handler.visibility));
            }
            _ => {}
        }
    }

    iface
}
