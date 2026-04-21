use std::collections::HashMap;

use sporec_parser::{
    ast::{Item, TypeExpr},
    parse,
};

use crate::env::HandlerInfo;
use crate::types::{CapSet, ErrorSet, Ty};

use super::{ModuleInterface, SymbolVisibility};

fn prelude_type_mapping(type_params: &[String]) -> HashMap<String, Ty> {
    type_params
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), Ty::Var(idx as u32)))
        .collect()
}

fn resolve_prelude_type(te: &TypeExpr, mapping: &HashMap<String, Ty>) -> Ty {
    match te {
        TypeExpr::Named(name) => match name.as_str() {
            "I32" => Ty::I32,
            "I8" => Ty::I8,
            "I16" => Ty::I16,
            "I64" => Ty::I64,
            "U8" => Ty::U8,
            "U16" => Ty::U16,
            "U32" => Ty::U32,
            "U64" => Ty::U64,
            "F32" => Ty::F32,
            "F64" => Ty::F64,
            "Bool" => Ty::Bool,
            "Str" => Ty::Str,
            "Char" => Ty::Char,
            "Never" => Ty::Never,
            _ => mapping
                .get(name)
                .cloned()
                .unwrap_or_else(|| Ty::Named(name.clone())),
        },
        TypeExpr::Hole(name) => name
            .as_ref()
            .map_or_else(|| Ty::Named("_".into()), |n| Ty::Named(n.clone())),
        TypeExpr::Generic(name, args) => Ty::App(
            name.clone(),
            args.iter()
                .map(|arg| resolve_prelude_type(arg, mapping))
                .collect(),
        ),
        TypeExpr::Tuple(types) => {
            if types.is_empty() {
                Ty::Unit
            } else {
                Ty::Tuple(
                    types
                        .iter()
                        .map(|ty| resolve_prelude_type(ty, mapping))
                        .collect(),
                )
            }
        }
        TypeExpr::Function(params, ret, error_exprs) => {
            let errors: ErrorSet = error_exprs
                .iter()
                .filter_map(|te| match te {
                    TypeExpr::Named(name) => Some(name.clone()),
                    _ => None,
                })
                .collect();
            Ty::Fn(
                params
                    .iter()
                    .map(|param| resolve_prelude_type(param, mapping))
                    .collect(),
                Box::new(resolve_prelude_type(ret, mapping)),
                CapSet::new(),
                errors,
            )
        }
        TypeExpr::Refinement(base, var_name, pred_expr) => Ty::Refined(
            Box::new(resolve_prelude_type(base, mapping)),
            var_name.clone(),
            pred_expr.clone(),
        ),
        TypeExpr::Record(fields) => Ty::Record(
            fields
                .iter()
                .map(|(name, ty)| (name.clone(), resolve_prelude_type(ty, mapping)))
                .collect(),
        ),
    }
}

pub(super) fn build_prelude_interface() -> ModuleInterface {
    let module = parse(include_str!("../../../../stdlib/prelude.sp"))
        .expect("embedded stdlib/prelude.sp must parse");
    let mut iface = ModuleInterface::new(vec!["Std".into(), "Prelude".into()]);
    let checker = crate::check::Checker::new();

    for item in &module.items {
        match item {
            Item::Function(f) => {
                let mut type_params = f.type_params.clone();
                if let Some(wc) = &f.where_clause {
                    type_params.extend(wc.constraints.iter().map(|c| c.type_var.clone()));
                }
                type_params.sort();
                type_params.dedup();
                let mapping = prelude_type_mapping(&type_params);
                let param_tys = f
                    .params
                    .iter()
                    .map(|param| resolve_prelude_type(&param.ty, &mapping))
                    .collect();
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(|ty| resolve_prelude_type(ty, &mapping))
                    .unwrap_or(Ty::Unit);
                iface.functions.insert(f.name.clone(), (param_tys, ret_ty));
                iface.function_caps.insert(
                    f.name.clone(),
                    checker.declared_capabilities(f.uses_clause.as_ref()),
                );
                if !f.errors.is_empty() {
                    let error_set: ErrorSet = f
                        .errors
                        .iter()
                        .filter_map(|te| match te {
                            TypeExpr::Named(name) => Some(name.clone()),
                            _ => None,
                        })
                        .collect();
                    iface.function_errors.insert(f.name.clone(), error_set);
                }
                let mut fn_type_params = f.type_params.clone();
                if let Some(wc) = &f.where_clause {
                    fn_type_params.extend(wc.constraints.iter().map(|c| c.type_var.clone()));
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
                fn_type_params.sort();
                fn_type_params.dedup();
                if !fn_type_params.is_empty() {
                    iface
                        .function_type_params
                        .insert(f.name.clone(), fn_type_params);
                }
                iface.set_visibility(&f.name, SymbolVisibility::from(&f.visibility));
            }
            Item::StructDef(s) => {
                let mapping = HashMap::new();
                let fields = s
                    .fields
                    .iter()
                    .map(|field| {
                        (
                            field.name.clone(),
                            resolve_prelude_type(&field.ty, &mapping),
                        )
                    })
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
                let mapping = prelude_type_mapping(&t.type_params);
                let variants = t
                    .variants
                    .iter()
                    .map(|variant| {
                        (
                            variant.name.clone(),
                            variant
                                .fields
                                .iter()
                                .map(|field| resolve_prelude_type(field, &mapping))
                                .collect(),
                        )
                    })
                    .collect();
                iface.types.insert(t.name.clone(), variants);
                iface.set_visibility(&t.name, SymbolVisibility::from(&t.visibility));
            }
            Item::CapabilityDef(cap) => {
                iface.capabilities.insert(cap.name.clone());
                iface.set_visibility(&cap.name, SymbolVisibility::from(&cap.visibility));
            }
            Item::Const(_)
            | Item::ImplDef(_)
            | Item::Import(_)
            | Item::Alias(_)
            | Item::CapabilityAlias { .. }
            | Item::TraitDef(_)
            | Item::EffectDef(_)
            | Item::EffectAlias(_) => {}
            Item::HandlerDef(handler) => {
                let fields = handler
                    .fields
                    .iter()
                    .map(|field| {
                        (
                            field.name.clone(),
                            resolve_prelude_type(&field.ty, &HashMap::new()),
                        )
                    })
                    .collect();
                let methods = handler
                    .methods
                    .iter()
                    .map(|method| {
                        let param_tys = method
                            .params
                            .iter()
                            .map(|param| resolve_prelude_type(&param.ty, &HashMap::new()))
                            .collect();
                        let ret_ty = method
                            .return_type
                            .as_ref()
                            .map(|ty| resolve_prelude_type(ty, &HashMap::new()))
                            .unwrap_or(Ty::Unit);
                        (method.name.clone(), param_tys, ret_ty)
                    })
                    .collect();
                iface.handlers.insert(
                    handler.name.clone(),
                    HandlerInfo {
                        effect: handler.effect.clone(),
                        fields,
                        methods,
                    },
                );
            }
        }
    }

    iface
}
