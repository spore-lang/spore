use super::*;

impl Checker {
    pub(super) fn register_item(&mut self, item: &Item) {
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
                if !f.errors.is_empty() {
                    let error_set = crate::types::declared_error_set(&f.errors);
                    self.registry.fn_errors.insert(f.name.clone(), error_set);
                }
                let mut type_params = f.type_params.clone();
                if let Some(wc) = &f.where_clause {
                    type_params.extend(wc.constraints.iter().map(|c| c.type_var.clone()));
                }
                type_params.sort();
                type_params.dedup();
                if !type_params.is_empty() {
                    self.registry
                        .fn_type_params
                        .insert(f.name.clone(), type_params);
                }
                if let Some(wc) = &f.where_clause
                    && !wc.constraints.is_empty()
                {
                    self.registry.fn_where_bounds.insert(
                        f.name.clone(),
                        wc.constraints
                            .iter()
                            .map(|c| (c.type_var.clone(), c.bound.clone()))
                            .collect(),
                    );
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
                if !self.registry.interfaces.contains_key(&impl_def.trait_name) {
                    self.err(
                        ErrorCode::C0002,
                        format!("unknown trait `{}`", impl_def.trait_name),
                    );
                    return;
                }
                let methods: Vec<(String, Vec<Ty>, Ty)> = impl_def
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
                    (impl_def.trait_name.clone(), impl_def.target_type.clone()),
                    methods,
                );
            }
            Item::Import(_) | Item::Const(_) => {}
            Item::EffectAlias(ea) => {
                for component in &ea.effects {
                    self.hierarchy
                        .add_implies(ea.name.clone(), component.clone());
                }
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
                            hd.handles_clause.effects.iter().cloned(),
                        ));
                let uses_effects = self.declared_effects(hd.uses_clause.as_ref());
                let fields: Vec<(String, Ty)> = hd
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), self.resolve_type(&field.ty)))
                    .collect();
                let mut methods = HashMap::new();
                for handler_impl in &hd.impls {
                    if !self.registry.interfaces.contains_key(&handler_impl.effect) {
                        self.err(
                            ErrorCode::C0002,
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
                        uses_effects,
                        fields: fields.clone(),
                        methods,
                    },
                );
                self.registry
                    .structs
                    .insert(handler_self_type_name(&hd.name), fields);
            }
            Item::Alias(alias_def) => {
                let resolved = self.resolve_type(&alias_def.target);
                self.registry
                    .type_aliases
                    .insert(alias_def.name.clone(), resolved);
            }
        }
    }

    pub(crate) fn declared_effects(&self, uses_clause: Option<&UsesClause>) -> EffectSet {
        uses_clause
            .map(|uc| {
                let raw = crate::effect_set::EffectSet::from_names(uc.resources.iter().cloned());
                self.hierarchy.expand(&raw)
            })
            .unwrap_or_default()
    }

    // ── Checking (second pass) ──────────────────────────────────────

    pub(super) fn check_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => self.check_fn(f),
            Item::ImplDef(impl_def) => self.check_impl(impl_def),
            Item::HandlerDef(handler_def) => self.check_handler(handler_def),
            Item::EffectAlias(ea) => {
                for component in &ea.effects {
                    if !self.registry.interfaces.contains_key(component) {
                        self.err(
                            ErrorCode::C0002,
                            format!(
                                "effect alias `{}` references unknown effect `{}`",
                                ea.name, component
                            ),
                        );
                    }
                }
            }
            _ => {}
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
                    .map(|p| self.resolve_type(&p.ty))
                    .collect();
                let impl_ret = method
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
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
        let Some((_trait_type_params, trait_methods)) =
            self.registry.interfaces.get(&impl_def.trait_name).cloned()
        else {
            return;
        };

        let trait_type_params = _trait_type_params;
        let type_mapping: HashMap<String, Ty> = if trait_type_params.is_empty() {
            HashMap::new()
        } else if !impl_def.type_args.is_empty() {
            trait_type_params
                .iter()
                .zip(impl_def.type_args.iter())
                .map(|(param, arg)| (param.clone(), self.resolve_type(arg)))
                .collect()
        } else if trait_type_params.len() == 1 {
            let mut m = HashMap::new();
            m.insert(
                trait_type_params[0].clone(),
                self.resolve_type(&TypeExpr::Named(impl_def.target_type.clone())),
            );
            m
        } else {
            HashMap::new()
        };

        let impl_label = format!(
            "impl `{}` for `{}`",
            impl_def.trait_name, impl_def.target_type
        );
        self.check_contract_impl(
            &impl_def.trait_name,
            &trait_methods,
            &impl_def.methods,
            &impl_label,
            "method",
            "trait",
            impl_def.span,
            &type_mapping,
            &[],
            &EffectSet::new(),
        );
    }

    pub(super) fn check_handler(&mut self, handler_def: &HandlerDef) {
        let declared_handles = crate::effect_set::EffectSet::from_names(
            handler_def.handles_clause.effects.iter().cloned(),
        );
        let handler_uses = self.declared_effects(handler_def.uses_clause.as_ref());
        let mut seen_declared = HashSet::new();
        for effect in &handler_def.handles_clause.effects {
            if !seen_declared.insert(effect.clone()) {
                self.err(
                    ErrorCode::E0014,
                    format!(
                        "handler `{}` declares duplicate handled effect `{effect}`",
                        handler_def.name
                    ),
                );
            }
            if !self.registry.interfaces.contains_key(effect) {
                self.err(ErrorCode::C0002, format!("unknown effect `{effect}`"));
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
                        "handler `{}` implements effect `{}` but does not declare it in `handles [...]`",
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
                &handler_uses,
            );
            let leaked_effects = observed_effects.difference(&handler_uses);
            if !leaked_effects.is_empty() {
                self.err(
                    ErrorCode::C0001,
                    format!(
                        "handler `{}` impl for effect `{}` leaks undeclared effects {}; add them to the handler `uses [...]` clause",
                        handler_def.name, handler_impl.effect, leaked_effects
                    ),
                );
            }
        }

        for effect in &handler_def.handles_clause.effects {
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
        self.concurrency.enter_function(&f.name);
        let declared_effects =
            inherited_effects.union(&self.declared_effects(f.uses_clause.as_ref()));
        let prev_effects = std::mem::replace(&mut self.current_effects, declared_effects);

        let prev_errors = std::mem::replace(
            &mut self.current_errors,
            crate::types::declared_error_set(&f.errors),
        );

        let prev_function = std::mem::replace(&mut self.current_function, f.name.clone());
        let (declared_param_tys, declared_ret) = self
            .registry
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
            });
        let prev_expected = self.expected_return_type.take();
        self.expected_return_type = Some(declared_ret.clone());
        let prev_hole_allows = self.current_hole_allows.take();
        self.current_hole_allows = f.hole_allows.clone();

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

        if let Some(spec) = &f.spec_clause {
            self.check_spec_clause(spec, &f.name, &declared_param_tys, &declared_ret);
        }

        self.env.pop_scope();
        self.current_effects = prev_effects;
        self.current_errors = prev_errors;
        self.current_function = prev_function;
        self.expected_return_type = prev_expected;
        self.current_hole_allows = prev_hole_allows;
        self.concurrency.leave_function(&f.name);
        observed_effects
    }

    /// Type-check a `spec { ... }` clause attached to a function.
    fn spec_property_param_compatible(&self, property_param: &Ty, function_param: &Ty) -> bool {
        let property_param = self.apply_subst(property_param);
        let function_param = self.apply_subst(function_param);

        if property_param == function_param {
            return true;
        }

        match (&property_param, &function_param) {
            // A property may narrow an unrefined function parameter into a
            // refinement-based input subset that shares the same base type.
            (Ty::Refined(property_base, _, _), function_param)
                if !matches!(function_param, Ty::Refined(_, _, _)) =>
            {
                self.apply_subst(property_base.as_ref()) == function_param.clone()
            }
            _ => false,
        }
    }

    pub(super) fn check_spec_clause(
        &mut self,
        spec: &SpecClause,
        fn_name: &str,
        fn_params: &[Ty],
        fn_ret: &Ty,
    ) {
        use crate::types::Ty;

        for item in &spec.items {
            match item {
                SpecItem::Example(ex) => {
                    let ty = self.check_expr(&ex.body);
                    let ty = self.apply_subst(&ty);
                    self.unify(
                        &Ty::Bool,
                        &ty,
                        &format!("spec example \"{}\" in `{fn_name}`", ex.label),
                    );
                }
                SpecItem::Property(prop) => {
                    let ty = self.check_expr(&prop.predicate);
                    let ty = self.apply_subst(&ty);
                    match &ty {
                        Ty::Fn(params, ret, _, _) => {
                            if params.len() != fn_params.len() {
                                self.err(
                                    ErrorCode::E0001,
                                    format!(
                                        "spec property \"{}\" in `{fn_name}` must take {} parameter(s), got {}",
                                        prop.label,
                                        fn_params.len(),
                                        params.len()
                                    ),
                                );
                                continue;
                            }

                            for (idx, (prop_param, fn_param)) in
                                params.iter().zip(fn_params.iter()).enumerate()
                            {
                                if !self.spec_property_param_compatible(prop_param, fn_param) {
                                    self.err(
                                        ErrorCode::E0001,
                                        format!(
                                            "spec property \"{}\" in `{fn_name}` parameter {} must match the function input type or a refinement subset of it; expected `{}`, got `{}`",
                                            prop.label,
                                            idx + 1,
                                            fn_param,
                                            prop_param
                                        ),
                                    );
                                }
                            }

                            self.unify(
                                fn_ret,
                                ret,
                                &format!("spec property \"{}\" in `{fn_name}`", prop.label),
                            );
                        }
                        _ => {
                            self.err(
                                ErrorCode::E0301,
                                format!(
                                    "spec property \"{}\" in `{fn_name}` must be a lambda, found {ty:?}",
                                    prop.label
                                ),
                            );
                        }
                    }
                }
            }
        }
    }
}
