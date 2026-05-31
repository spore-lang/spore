use super::*;

fn valid_foreign_args(attribute: &Attribute) -> bool {
    let mut provider_seen = false;
    let mut name_seen = false;
    attribute.args.iter().all(|arg| match arg {
        AttrArg::Positional(AttrValue::Str(_)) if !provider_seen => {
            provider_seen = true;
            true
        }
        AttrArg::Named {
            name,
            value: AttrValue::Str(_),
        } if name == "name" && !name_seen => {
            name_seen = true;
            true
        }
        _ => false,
    })
}

fn valid_export_c_abi(attribute: &Attribute) -> bool {
    matches!(
        attribute.args.as_slice(),
        [AttrArg::Positional(AttrValue::Str(abi))] if abi == "C"
    )
}

fn type_expr_name_and_args(ty: &TypeExpr) -> Option<(&str, &[TypeExpr])> {
    match ty {
        TypeExpr::Named(name) => Some((name, &[])),
        TypeExpr::Generic(name, args) => Some((name, args)),
        _ => None,
    }
}

impl Checker {
    fn register_impl_methods(
        &mut self,
        impl_def: &ImplDef,
        owner_type: &TypeExpr,
        trait_name: Option<&str>,
    ) {
        let owner = self.resolve_type(owner_type);
        let self_mapping = HashMap::from([("Self".to_string(), owner.clone())]);

        for method in &impl_def.methods {
            let params = method
                .params
                .iter()
                .map(|param| {
                    let resolved = self.resolve_type(&param.ty);
                    self.instantiate_ty(&resolved, &self_mapping)
                })
                .collect();
            let return_type = method
                .return_type
                .as_ref()
                .map(|ty| self.resolve_type(ty))
                .map(|ty| self.instantiate_ty(&ty, &self_mapping))
                .unwrap_or(Ty::Unit);
            let required_effects = self.declared_effects(method.uses_clause.as_ref());
            let mut type_params = impl_def.type_params.clone();
            type_params.extend(method.type_params.iter().cloned());
            type_params.sort();
            type_params.dedup();
            let mut generic_bounds = impl_def
                .type_param_bounds
                .iter()
                .chain(&method.type_param_bounds)
                .map(|constraint| (constraint.type_var.clone(), constraint.bound.clone()))
                .collect::<Vec<_>>();
            generic_bounds.sort();
            generic_bounds.dedup();

            self.registry
                .methods
                .entry(method.name.clone())
                .or_default()
                .push(MethodInfo {
                    owner: owner.clone(),
                    trait_name: trait_name.map(str::to_string),
                    type_params,
                    generic_bounds,
                    params,
                    return_type,
                    required_effects,
                    has_receiver: method
                        .params
                        .first()
                        .is_some_and(|param| param.name == "self"),
                });
        }
    }

    pub(crate) fn register_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => {
                let mut signature_holes = HashMap::new();
                let param_tys: Vec<Ty> = f
                    .params
                    .iter()
                    .map(|p| self.resolve_signature_type(&p.ty, &mut signature_holes))
                    .collect();
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_signature_type(t, &mut signature_holes))
                    .unwrap_or(Ty::Unit);
                let effects: EffectSet = f
                    .uses_clause
                    .as_ref()
                    .map(|uc| self.declared_effects(Some(uc)))
                    .unwrap_or_default();
                self.registry
                    .functions
                    .insert(f.name.clone(), (param_tys, ret_ty, effects));
                let mut type_params = f.type_params.clone();
                type_params.extend(f.type_param_bounds.iter().map(|c| c.type_var.clone()));
                type_params.sort();
                type_params.dedup();
                if !type_params.is_empty() {
                    self.registry
                        .fn_type_params
                        .insert(f.name.clone(), type_params);
                }
                let generic_bounds = f
                    .type_param_bounds
                    .iter()
                    .map(|c| (c.type_var.clone(), c.bound.clone()))
                    .collect::<Vec<_>>();
                if !generic_bounds.is_empty() {
                    self.registry
                        .fn_generic_bounds
                        .insert(f.name.clone(), generic_bounds);
                }
            }
            Item::StructDef(s) => {
                let fields: Vec<(String, Ty)> = s
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), self.resolve_type(&f.ty)))
                    .collect();
                self.registry.structs.insert(s.name.clone(), fields);
                if !s.type_params.is_empty() {
                    self.registry
                        .struct_type_params
                        .insert(s.name.clone(), s.type_params.clone());
                }
            }
            Item::TypeDef(t) => {
                let variants: Vec<(String, Vec<Ty>)> = t
                    .variants
                    .iter()
                    .map(|v| {
                        let ftys: Vec<Ty> = v.fields.iter().map(|f| self.resolve_type(f)).collect();
                        (v.name.clone(), ftys)
                    })
                    .collect();
                self.registry.types.insert(t.name.clone(), variants.clone());
                if !t.type_params.is_empty() {
                    self.registry
                        .type_type_params
                        .insert(t.name.clone(), t.type_params.clone());
                }

                let ret_ty = if t.type_params.is_empty() {
                    Ty::Named(t.name.clone())
                } else {
                    Ty::App(
                        t.name.clone(),
                        t.type_params.iter().map(|p| Ty::Named(p.clone())).collect(),
                    )
                };

                for (vname, field_tys) in &variants {
                    if field_tys.is_empty() {
                        if t.type_params.is_empty() {
                            self.env.define(vname.clone(), ret_ty.clone());
                        } else {
                            self.registry.functions.insert(
                                vname.clone(),
                                (Vec::new(), ret_ty.clone(), EffectSet::new()),
                            );
                            self.registry
                                .fn_type_params
                                .insert(vname.clone(), t.type_params.clone());
                        }
                    } else {
                        self.registry.functions.insert(
                            vname.clone(),
                            (field_tys.clone(), ret_ty.clone(), EffectSet::new()),
                        );
                        if !t.type_params.is_empty() {
                            self.registry
                                .fn_type_params
                                .insert(vname.clone(), t.type_params.clone());
                        }
                    }
                }
            }
            Item::ImplDef(impl_def) => {
                let Some(target_type) = &impl_def.target_type else {
                    let owner_type = &impl_def.interface_type;
                    self.register_impl_methods(impl_def, owner_type, None);
                    return;
                };
                let Some((trait_name, _)) = type_expr_name_and_args(&impl_def.interface_type)
                else {
                    self.err(
                        ErrorCode::E0001,
                        "trait position in an impl declaration must name a trait".into(),
                    );
                    return;
                };
                if !self.registry.interfaces.contains_key(trait_name) {
                    self.err(ErrorCode::F0002, format!("unknown trait `{trait_name}`"));
                    return;
                }
                let Some((target_name, _)) = type_expr_name_and_args(target_type) else {
                    return;
                };
                self.register_impl_methods(impl_def, target_type, Some(trait_name));
                let self_ty = self.resolve_type(target_type);
                let self_mapping = HashMap::from([("Self".to_string(), self_ty)]);
                let methods: Vec<(String, Vec<Ty>, Ty)> = impl_def
                    .methods
                    .iter()
                    .map(|m| {
                        let param_tys: Vec<Ty> = m
                            .params
                            .iter()
                            .map(|p| {
                                let resolved = self.resolve_type(&p.ty);
                                self.instantiate_ty(&resolved, &self_mapping)
                            })
                            .collect();
                        let ret_ty = m
                            .return_type
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .map(|ty| self.instantiate_ty(&ty, &self_mapping))
                            .unwrap_or(Ty::Unit);
                        (m.name.clone(), param_tys, ret_ty)
                    })
                    .collect();
                self.registry
                    .impls
                    .insert((trait_name.to_string(), target_name.to_string()), methods);
            }
            Item::Import(_) | Item::Const(_) => {}
            Item::SurfaceDef(surface) => {
                self.registry.surfaces.insert(surface.name.clone());
                self.registry
                    .surface_type_params
                    .insert(surface.name.clone(), surface.type_params.clone());
                self.hierarchy.add_surface(
                    surface.name.clone(),
                    surface.surface.names().into_iter().map(str::to_string),
                );
            }
            Item::TraitDef(td) => {
                let methods: Vec<(String, Vec<Ty>, Ty)> = td
                    .methods
                    .iter()
                    .map(|m| {
                        let param_tys: Vec<Ty> =
                            m.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                        let ret_ty = m
                            .return_type
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .unwrap_or(Ty::Unit);
                        (m.name.clone(), param_tys, ret_ty)
                    })
                    .collect();
                self.registry
                    .interfaces
                    .insert(td.name.clone(), (td.type_params.clone(), methods));
            }
            Item::EffectDef(ed) => {
                self.registry.effects.insert(ed.name.clone());
                self.registry
                    .effect_type_params
                    .insert(ed.name.clone(), ed.type_params.clone());
                let methods: Vec<(String, Vec<Ty>, Ty)> = ed
                    .operations
                    .iter()
                    .map(|m| {
                        let param_tys: Vec<Ty> =
                            m.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                        let ret_ty = m
                            .return_type
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .unwrap_or(Ty::Unit);
                        (m.name.clone(), param_tys, ret_ty)
                    })
                    .collect();
                self.registry
                    .interfaces
                    .insert(ed.name.clone(), (vec![], methods));
            }
            Item::HandlerDef(hd) => {
                let handled_effects =
                    self.hierarchy
                        .expand(&crate::effect_set::EffectSet::from_names(
                            hd.surface.names().into_iter().map(str::to_string),
                        ));
                let mut methods = HashMap::new();
                for handler_impl in &hd.impls {
                    if !self.registry.effects.contains(&handler_impl.effect) {
                        self.err(
                            ErrorCode::F0002,
                            format!("unknown effect `{}`", handler_impl.effect),
                        );
                        continue;
                    }
                    let impl_methods: Vec<(String, Vec<Ty>, Ty)> = handler_impl
                        .methods
                        .iter()
                        .map(|m| {
                            let param_tys: Vec<Ty> =
                                m.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                            let ret_ty = m
                                .return_type
                                .as_ref()
                                .map(|t| self.resolve_type(t))
                                .unwrap_or(Ty::Unit);
                            (m.name.clone(), param_tys, ret_ty)
                        })
                        .collect();
                    self.registry.impls.insert(
                        (handler_impl.effect.clone(), hd.name.clone()),
                        impl_methods.clone(),
                    );
                    methods.insert(handler_impl.effect.clone(), impl_methods);
                }
                self.registry.handlers.insert(
                    hd.name.clone(),
                    HandlerInfo {
                        handled_effects,
                        uses_effects: EffectSet::new(),
                        fields: Vec::new(),
                        methods,
                    },
                );
                self.registry
                    .structs
                    .insert(handler_self_type_name(&hd.name), Vec::new());
            }
            Item::Alias(alias_def) => {
                let resolved = self.resolve_type(&alias_def.target);
                if alias_def.type_params.is_empty() {
                    self.registry
                        .type_aliases
                        .insert(alias_def.name.clone(), resolved);
                } else {
                    self.registry.generic_type_aliases.insert(
                        alias_def.name.clone(),
                        (alias_def.type_params.clone(), resolved),
                    );
                }
            }
            Item::OpaqueType(type_def) => {
                self.registry
                    .opaque_types
                    .insert(type_def.name.clone(), type_def.type_params.clone());
            }
        }
    }

    pub(crate) fn declared_effects(&self, uses_clause: Option<&UsesClause>) -> EffectSet {
        uses_clause
            .map(|uc| {
                let raw = crate::effect_set::EffectSet::from_names(
                    uc.surface.names().into_iter().map(str::to_string),
                );
                self.hierarchy.expand(&raw)
            })
            .unwrap_or_default()
    }

    fn check_surface_expr(&mut self, surface: &SurfaceExpr, owner: &str) {
        for reference in surface.references() {
            self.check_surface_ref(reference, owner);
        }
    }

    fn check_surface_ref(&mut self, reference: &SurfaceRef, owner: &str) {
        let type_params = if self.registry.effects.contains(&reference.name) {
            self.registry
                .effect_type_params
                .get(&reference.name)
                .cloned()
                .unwrap_or_default()
        } else if self.registry.surfaces.contains(&reference.name) {
            self.registry
                .surface_type_params
                .get(&reference.name)
                .cloned()
                .unwrap_or_default()
        } else {
            self.err(
                ErrorCode::F0002,
                format!(
                    "{owner} references unknown effect or surface `{}`",
                    reference.name
                ),
            );
            return;
        };
        if type_params.len() != reference.type_args.len() {
            self.err(
                ErrorCode::E0401,
                format!(
                    "effect surface reference `{}` expects {} type arguments, got {}",
                    reference.name,
                    type_params.len(),
                    reference.type_args.len()
                ),
            );
        }
        for type_arg in &reference.type_args {
            let _ = self.resolve_type(type_arg);
        }
    }

    // ── Checking (second pass) ──────────────────────────────────────

    pub(super) fn check_item(&mut self, item: &Item) {
        self.check_non_function_attributes(item);
        match item {
            Item::Function(f) => self.check_fn(f),
            Item::ImplDef(impl_def) => self.check_impl(impl_def),
            Item::HandlerDef(handler_def) => self.check_handler(handler_def),
            Item::SurfaceDef(surface) => {
                self.check_surface_expr(&surface.surface, &format!("surface `{}`", surface.name));
                if self.hierarchy.has_cycle(&surface.name) {
                    self.err(
                        ErrorCode::F0002,
                        format!("surface `{}` contains a recursive expansion", surface.name),
                    );
                }
            }
            _ => {}
        }
    }

    fn check_non_function_attributes(&mut self, item: &Item) {
        if let Item::OpaqueType(type_def) = item {
            self.check_opaque_type_attributes(type_def);
            return;
        }
        let (kind, attributes) = match item {
            Item::Function(_) => return,
            Item::Const(item) => ("const declaration", &item.attributes),
            Item::StructDef(item) => ("struct declaration", &item.attributes),
            Item::TypeDef(item) => ("enum declaration", &item.attributes),
            Item::ImplDef(item) => ("impl declaration", &item.attributes),
            Item::Import(_) => return,
            Item::Alias(item) => ("type alias", &item.attributes),
            Item::TraitDef(item) => ("trait declaration", &item.attributes),
            Item::EffectDef(item) => ("effect declaration", &item.attributes),
            Item::SurfaceDef(item) => ("surface declaration", &item.attributes),
            Item::HandlerDef(item) => ("handler declaration", &item.attributes),
            Item::OpaqueType(_) => unreachable!(),
        };
        for attribute in attributes {
            if attribute.name == "foreign" || attribute.name == "export" {
                self.attribute_error(
                    ErrorCode::M0601,
                    attribute,
                    format!("`@{}` is not valid on a {kind}", attribute.name),
                );
            } else {
                self.attribute_error(
                    ErrorCode::M0601,
                    attribute,
                    format!("unsupported attribute `@{}`", attribute.name),
                );
            }
        }
    }

    fn check_opaque_type_attributes(&mut self, type_def: &OpaqueTypeDef) {
        let foreign_attributes = type_def
            .attributes
            .iter()
            .filter(|attribute| attribute.name == "foreign")
            .collect::<Vec<_>>();

        for attribute in &type_def.attributes {
            if attribute.name != "foreign" {
                self.attribute_error(
                    ErrorCode::M0601,
                    attribute,
                    format!(
                        "unsupported attribute `@{}` on opaque type `{}`",
                        attribute.name, type_def.name
                    ),
                );
            }
        }

        if foreign_attributes.is_empty() {
            self.err(
                ErrorCode::M0601,
                format!(
                    "bodyless type `{}` must be marked `@foreign` or replaced with a type alias",
                    type_def.name
                ),
            );
            return;
        }

        if foreign_attributes.len() > 1 {
            self.attribute_error(
                ErrorCode::M0601,
                foreign_attributes[1],
                format!(
                    "opaque type `{}` declares `@foreign` more than once",
                    type_def.name
                ),
            );
        }

        for attribute in foreign_attributes {
            if !attribute.args.is_empty() {
                self.attribute_error(
                    ErrorCode::M0601,
                    attribute,
                    "`@foreign` opaque type declarations do not accept linkage arguments".into(),
                );
            }
        }
    }

    fn check_fn_attributes(&mut self, function: &FnDef) {
        for attribute in &function.attributes {
            if attribute.name != "foreign" && attribute.name != "export" {
                self.attribute_error(
                    ErrorCode::M0601,
                    attribute,
                    format!("unsupported attribute `@{}`", attribute.name),
                );
            }
        }
        let foreign_attributes = function
            .attributes
            .iter()
            .filter(|attribute| attribute.name == "foreign")
            .collect::<Vec<_>>();
        let export_attributes = function
            .attributes
            .iter()
            .filter(|attribute| attribute.name == "export")
            .collect::<Vec<_>>();

        if foreign_attributes.len() > 1 {
            self.attribute_error(
                ErrorCode::M0601,
                foreign_attributes[1],
                format!(
                    "function `{}` declares `@foreign` more than once",
                    function.name
                ),
            );
        }
        for attribute in &foreign_attributes {
            if !valid_foreign_args(attribute) {
                self.attribute_error(
                    ErrorCode::M0601,
                    attribute,
                    "`@foreign` accepts an optional provider string and optional `name = \"...\"`"
                        .into(),
                );
            }
            if function.body.is_some() {
                self.attribute_error(
                    ErrorCode::M0602,
                    attribute,
                    format!(
                        "`@foreign` function `{}` must be a bodyless declaration ending with `;`",
                        function.name
                    ),
                );
            }
        }

        if export_attributes.len() > 1 {
            self.attribute_error(
                ErrorCode::M0601,
                export_attributes[1],
                format!(
                    "function `{}` declares `@export` more than once",
                    function.name
                ),
            );
        }
        for attribute in &export_attributes {
            if !valid_export_c_abi(attribute) {
                self.attribute_error(
                    ErrorCode::M0603,
                    attribute,
                    "`@export` currently supports only `@export(\"C\")`".into(),
                );
            }
            if !matches!(&function.visibility, Visibility::Pub) || function.body.is_none() {
                self.attribute_error(
                    ErrorCode::M0601,
                    attribute,
                    format!(
                        "`@export` function `{}` must be public and have a body",
                        function.name
                    ),
                );
            }
        }

        if !foreign_attributes.is_empty() && !export_attributes.is_empty() {
            self.attribute_error(
                ErrorCode::M0601,
                export_attributes[0],
                format!(
                    "function `{}` cannot declare both `@foreign` and `@export`",
                    function.name
                ),
            );
        }
    }

    fn attribute_error(&mut self, code: ErrorCode, attribute: &Attribute, message: String) {
        if let Some(span) = attribute.span {
            self.err_at(code, message, span);
        } else {
            self.err(code, message);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn check_contract_impl(
        &mut self,
        contract_name: &str,
        contract_methods: &[(String, Vec<Ty>, Ty)],
        methods: &[FnDef],
        impl_label: &str,
        member_noun: &str,
        contract_noun: &str,
        span: Option<Span>,
        type_mapping: &HashMap<String, Ty>,
        extra_bindings: &[(String, Ty)],
        inherited_effects: &EffectSet,
    ) -> EffectSet {
        for (method_name, _expected_params, _expected_ret) in contract_methods {
            if !methods.iter().any(|m| &m.name == method_name) {
                let msg = format!("{impl_label} is missing {member_noun} `{method_name}`");
                if let Some(span) = span {
                    self.err_at(ErrorCode::E0013, msg, span);
                } else {
                    self.err(ErrorCode::E0013, msg);
                }
            }
        }

        for method in methods {
            if !contract_methods
                .iter()
                .any(|(name, _, _)| name == &method.name)
            {
                self.err(
                    ErrorCode::E0014,
                    format!(
                        "{member_noun} `{}` is not defined in {contract_noun} `{contract_name}`",
                        method.name
                    ),
                );
            }
        }

        for method in methods {
            if let Some((_expected_name, expected_params, expected_ret)) = contract_methods
                .iter()
                .find(|(name, _, _)| name == &method.name)
            {
                let expected_params: Vec<Ty> = expected_params
                    .iter()
                    .map(|t| self.instantiate_ty(t, type_mapping))
                    .collect();
                let expected_ret = self.instantiate_ty(expected_ret, type_mapping);
                let impl_params: Vec<Ty> = method
                    .params
                    .iter()
                    .map(|p| {
                        let resolved = self.resolve_type(&p.ty);
                        self.instantiate_ty(&resolved, type_mapping)
                    })
                    .collect();
                let impl_ret = method
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .map(|ty| self.instantiate_ty(&ty, type_mapping))
                    .unwrap_or(Ty::Unit);

                if expected_params.len() != impl_params.len() {
                    self.err(
                        ErrorCode::E0007,
                        format!(
                            "{member_noun} `{}` in {impl_label} expects {} parameters, got {}",
                            method.name,
                            expected_params.len(),
                            impl_params.len()
                        ),
                    );
                    continue;
                }

                for (i, (expected_param, impl_param)) in
                    expected_params.iter().zip(impl_params.iter()).enumerate()
                {
                    self.unify(
                        expected_param,
                        impl_param,
                        &format!(
                            "parameter {} of {member_noun} `{}` in {impl_label}",
                            i + 1,
                            method.name
                        ),
                    );
                }
                self.unify(
                    &expected_ret,
                    &impl_ret,
                    &format!(
                        "return type of {member_noun} `{}` in {impl_label}",
                        method.name
                    ),
                );
            }
        }

        let mut observed_effects = EffectSet::new();
        for method in methods {
            observed_effects = observed_effects.union(&self.check_fn_with_extra_bindings(
                method,
                extra_bindings,
                inherited_effects,
            ));
        }
        observed_effects
    }

    pub(super) fn check_impl(&mut self, impl_def: &ImplDef) {
        for constraint in &impl_def.type_param_bounds {
            if !self.registry.interfaces.contains_key(&constraint.bound) {
                self.err(
                    ErrorCode::E0403,
                    format!(
                        "unknown trait bound `{}` in impl type parameter `{}`",
                        constraint.bound, constraint.type_var
                    ),
                );
            }
        }

        let Some(target_type) = &impl_def.target_type else {
            let self_ty = self.resolve_type(&impl_def.interface_type);
            let extra_bindings = vec![("self".to_string(), self_ty)];
            for method in &impl_def.methods {
                let _ =
                    self.check_fn_with_extra_bindings(method, &extra_bindings, &EffectSet::new());
            }
            return;
        };
        let Some((trait_name, trait_args)) = type_expr_name_and_args(&impl_def.interface_type)
        else {
            return;
        };
        let Some((trait_type_params, trait_methods)) =
            self.registry.interfaces.get(trait_name).cloned()
        else {
            return;
        };

        if trait_type_params.len() != trait_args.len() {
            self.err(
                ErrorCode::E0401,
                format!(
                    "trait `{trait_name}` expects {} type arguments, got {}",
                    trait_type_params.len(),
                    trait_args.len()
                ),
            );
            return;
        }

        let mut type_mapping: HashMap<String, Ty> = trait_type_params
            .iter()
            .zip(trait_args.iter())
            .map(|(param, arg)| (param.clone(), self.resolve_type(arg)))
            .collect();
        let self_ty = self.resolve_type(target_type);
        type_mapping.insert("Self".into(), self_ty.clone());
        let extra_bindings = vec![("self".to_string(), self_ty)];

        let impl_label = format!(
            "impl `{trait_name}` for `{}`",
            self.resolve_type(target_type)
        );
        self.check_contract_impl(
            trait_name,
            &trait_methods,
            &impl_def.methods,
            &impl_label,
            "method",
            "trait",
            impl_def.span,
            &type_mapping,
            &extra_bindings,
            &EffectSet::new(),
        );
    }

    pub(super) fn check_handler(&mut self, handler_def: &HandlerDef) {
        self.check_surface_expr(
            &handler_def.surface,
            &format!("handler `{}` target surface", handler_def.name),
        );
        let declared_handles = self
            .hierarchy
            .expand(&crate::effect_set::EffectSet::from_names(
                handler_def.surface.names().into_iter().map(str::to_string),
            ));
        for effect in declared_handles.iter() {
            if !self.registry.effects.contains(effect) {
                self.err(ErrorCode::F0002, format!("unknown effect `{effect}`"));
            }
        }

        let self_ty = Ty::Named(handler_self_type_name(&handler_def.name));
        let extra_bindings = vec![("self".to_string(), self_ty)];
        let mut seen_impls = HashSet::new();
        for handler_impl in &handler_def.impls {
            if !seen_impls.insert(handler_impl.effect.clone()) {
                self.err(
                    ErrorCode::E0014,
                    format!(
                        "handler `{}` has duplicate impl block for effect `{}`",
                        handler_def.name, handler_impl.effect
                    ),
                );
                continue;
            }
            if !declared_handles.contains(&handler_impl.effect) {
                self.err(
                    ErrorCode::E0014,
                    format!(
                        "handler `{}` implements effect `{}` outside its target surface",
                        handler_def.name, handler_impl.effect
                    ),
                );
            }
            let Some((_effect_type_params, effect_methods)) =
                self.registry.interfaces.get(&handler_impl.effect).cloned()
            else {
                continue;
            };

            let impl_label = format!(
                "handler `{}` impl for effect `{}`",
                handler_def.name, handler_impl.effect
            );
            let observed_effects = self.check_contract_impl(
                &handler_impl.effect,
                &effect_methods,
                &handler_impl.methods,
                &impl_label,
                "operation",
                "effect",
                handler_impl.span.or(handler_def.span),
                &HashMap::new(),
                &extra_bindings,
                &EffectSet::new(),
            );
            let _ = observed_effects;
        }

        for effect in declared_handles.iter() {
            if !handler_def
                .impls
                .iter()
                .any(|handler_impl| &handler_impl.effect == effect)
            {
                self.err(
                    ErrorCode::E0013,
                    format!(
                        "handler `{}` is missing impl block for effect `{effect}`",
                        handler_def.name
                    ),
                );
            }
        }

        let inferred_fields = self
            .registry
            .structs
            .get(&handler_self_type_name(&handler_def.name))
            .cloned()
            .unwrap_or_default();
        if let Some(info) = self.registry.handlers.get_mut(&handler_def.name) {
            info.fields = inferred_fields;
        }
    }

    pub(super) fn check_fn(&mut self, f: &FnDef) {
        let _ = self.check_fn_with_extra_bindings(f, &[], &EffectSet::new());
    }

    pub(super) fn check_fn_with_extra_bindings(
        &mut self,
        f: &FnDef,
        extra_bindings: &[(String, Ty)],
        inherited_effects: &EffectSet,
    ) -> EffectSet {
        self.check_fn_attributes(f);
        self.concurrency.enter_function(&f.name);
        if let Some(uses_clause) = &f.uses_clause {
            self.check_surface_expr(
                &uses_clause.surface,
                &format!("function `{}` `uses` clause", f.name),
            );
        }
        let declared_effects =
            inherited_effects.union(&self.declared_effects(f.uses_clause.as_ref()));
        let prev_effects = std::mem::replace(&mut self.current_effects, declared_effects);

        let prev_function = std::mem::replace(&mut self.current_function, f.name.clone());
        let (declared_param_tys, declared_ret) = if extra_bindings.is_empty() {
            self.registry
                .functions
                .get(&f.name)
                .map(|(params, ret, _)| (params.clone(), ret.clone()))
                .unwrap_or_else(|| {
                    (
                        f.params.iter().map(|p| self.resolve_type(&p.ty)).collect(),
                        f.return_type
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .unwrap_or(Ty::Unit),
                    )
                })
        } else {
            (
                f.params.iter().map(|p| self.resolve_type(&p.ty)).collect(),
                f.return_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(Ty::Unit),
            )
        };
        let prev_expected = self.expected_return_type.take();
        self.expected_return_type = Some(declared_ret.clone());
        let prev_outcome_failure = self.current_outcome_failure.take();
        self.current_outcome_failure = match &declared_ret {
            Ty::Outcome(_, failure) => Some((**failure).clone()),
            _ => None,
        };
        self.env.push_scope();

        for (param, ty) in f.params.iter().zip(declared_param_tys.iter().cloned()) {
            self.env.define(param.name.clone(), ty);
        }
        for (name, ty) in extra_bindings {
            self.env.define(name.clone(), ty.clone());
        }

        let mut observed_effects = EffectSet::new();
        if let Some(body) = &f.body {
            self.push_effect_observer();
            let _ = self.check_expr_against(&declared_ret, body, &format!("function `{}`", f.name));
            observed_effects = self.pop_effect_observer();
        }

        if let Some(properties) = &f.properties_clause {
            self.check_properties_clause(properties, &f.name);
        }

        self.env.pop_scope();
        self.current_effects = prev_effects;
        self.current_outcome_failure = prev_outcome_failure;
        self.current_function = prev_function;
        self.expected_return_type = prev_expected;
        self.concurrency.leave_function(&f.name);
        observed_effects
    }

    pub(super) fn check_properties_clause(&mut self, properties: &PropertiesClause, fn_name: &str) {
        for property in &properties.items {
            self.env.push_scope();
            for param in &property.params {
                let ty = self.resolve_type(&param.ty);
                self.env.define(param.name.clone(), ty);
            }
            let ty = self.check_expr(&property.predicate);
            let ty = self.apply_subst(&ty);
            self.unify(
                &Ty::Bool,
                &ty,
                &format!("property `{}` in `{fn_name}`", property.name),
            );
            self.env.pop_scope();
        }
    }
}
