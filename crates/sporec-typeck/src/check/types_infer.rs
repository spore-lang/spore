use super::*;

impl Checker {
    // ── Type resolution ─────────────────────────────────────────────

    pub(super) fn resolve_signature_type(
        &mut self,
        te: &TypeExpr,
        signature_holes: &mut HashMap<String, Ty>,
    ) -> Ty {
        match te {
            TypeExpr::Hole(Some(name)) => signature_holes
                .entry(name.clone())
                .or_insert_with(|| self.fresh_var())
                .clone(),
            TypeExpr::Hole(None) => self.fresh_var(),
            TypeExpr::Generic(name, args) => {
                let resolved = args
                    .iter()
                    .map(|a| self.resolve_signature_type(a, signature_holes))
                    .collect();
                self.instantiate_generic_alias(name, resolved)
            }
            TypeExpr::Tuple(types) => {
                if types.is_empty() {
                    Ty::Unit
                } else {
                    Ty::Tuple(
                        types
                            .iter()
                            .map(|t| self.resolve_signature_type(t, signature_holes))
                            .collect(),
                    )
                }
            }
            TypeExpr::Function(params, ret) => {
                let ptys = params
                    .iter()
                    .map(|p| self.resolve_signature_type(p, signature_holes))
                    .collect();
                Ty::Fn(
                    ptys,
                    Box::new(self.resolve_signature_type(ret, signature_holes)),
                    EffectSet::new(),
                )
            }
            TypeExpr::Outcome(success, failure) => Ty::Outcome(
                Box::new(self.resolve_signature_type(success, signature_holes)),
                Box::new(self.resolve_signature_type(failure, signature_holes)),
            ),
            TypeExpr::Refinement(base, var_name, pred_expr) => Ty::Refined(
                Box::new(self.resolve_signature_type(base, signature_holes)),
                var_name.clone(),
                pred_expr.clone(),
            ),
            TypeExpr::Record(fields) => Ty::Record(
                fields
                    .iter()
                    .map(|(name, te)| {
                        (
                            name.clone(),
                            self.resolve_signature_type(te, signature_holes),
                        )
                    })
                    .collect(),
            ),
            _ => self.resolve_type(te),
        }
    }

    pub fn resolve_type(&mut self, te: &TypeExpr) -> Ty {
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
                "Never" => Ty::Never,
                _ => {
                    // Check type aliases (supports refined aliases like `type Port = I64 when ...`)
                    if let Some(ty) = self.registry.type_aliases.get(name) {
                        ty.clone()
                    } else {
                        Ty::Named(name.clone())
                    }
                }
            },
            TypeExpr::Hole(_) => self.fresh_var(),
            TypeExpr::Generic(name, args) => {
                let resolved: Vec<Ty> = args.iter().map(|a| self.resolve_type(a)).collect();
                self.instantiate_generic_alias(name, resolved)
            }
            TypeExpr::Tuple(types) => {
                if types.is_empty() {
                    Ty::Unit
                } else {
                    Ty::Tuple(types.iter().map(|t| self.resolve_type(t)).collect())
                }
            }
            TypeExpr::Function(params, ret) => {
                let ptys: Vec<Ty> = params.iter().map(|p| self.resolve_type(p)).collect();
                Ty::Fn(ptys, Box::new(self.resolve_type(ret)), EffectSet::new())
            }
            TypeExpr::Outcome(success, failure) => Ty::Outcome(
                Box::new(self.resolve_type(success)),
                Box::new(self.resolve_type(failure)),
            ),
            TypeExpr::Refinement(base, var_name, pred_expr) => {
                let base_ty = self.resolve_type(base);
                Ty::Refined(Box::new(base_ty), var_name.clone(), pred_expr.clone())
            }
            TypeExpr::Record(fields) => {
                let resolved = fields
                    .iter()
                    .map(|(name, te)| (name.clone(), self.resolve_type(te)))
                    .collect();
                Ty::Record(resolved)
            }
        }
    }

    fn instantiate_generic_alias(&mut self, name: &str, args: Vec<Ty>) -> Ty {
        let Some((type_params, target)) = self.registry.generic_type_aliases.get(name).cloned()
        else {
            return Ty::App(name.to_string(), args);
        };
        if type_params.len() != args.len() {
            self.err(
                ErrorCode::E0002,
                format!(
                    "type alias `{name}` expects {} type arguments, got {}",
                    type_params.len(),
                    args.len()
                ),
            );
            return Ty::Error;
        }

        let substitutions = type_params.into_iter().zip(args).collect::<HashMap<_, _>>();
        target.fold_ref(&mut |ty| match ty {
            Ty::Named(name) => substitutions.get(name).cloned(),
            _ => None,
        })
    }

    // ── Type variable infrastructure ────────────────────────────────

    /// Create a fresh type variable.
    pub(super) fn fresh_var(&mut self) -> Ty {
        let id = self.next_var_id;
        self.next_var_id += 1;
        Ty::Var(id)
    }

    /// Apply the current substitution to a type, resolving type variables.
    pub(super) fn apply_subst(&self, ty: &Ty) -> Ty {
        ty.fold_ref(&mut |t| match t {
            Ty::Var(id) => {
                if let Some(resolved) = self.substitution.get(id) {
                    Some(self.apply_subst(resolved))
                } else {
                    Some(t.clone())
                }
            }
            _ => None,
        })
    }

    /// Check if type variable `id` occurs anywhere in `ty`.
    pub(super) fn occurs_in(&self, id: u32, ty: &Ty) -> bool {
        let ty = self.apply_subst(ty);
        let mut found = false;
        ty.visit(&mut |t| {
            if let Ty::Var(vid) = t
                && *vid == id
            {
                found = true;
            }
        });
        found
    }

    /// Substitute type parameter names with fresh type variables in a type.
    pub(super) fn instantiate_ty(&self, ty: &Ty, mapping: &HashMap<String, Ty>) -> Ty {
        ty.fold_ref(&mut |t| match t {
            Ty::Named(name) => mapping.get(name).cloned(),
            _ => None,
        })
    }

    pub(super) fn instantiate_struct_fields(
        &mut self,
        name: &str,
        field_defs: &[(String, Ty)],
    ) -> (Vec<(String, Ty)>, Ty) {
        match self.registry.struct_type_params.get(name).cloned() {
            Some(type_params) if !type_params.is_empty() => {
                let field_tys: Vec<Ty> = field_defs.iter().map(|(_, ty)| ty.clone()).collect();
                let ret_ty = Ty::App(
                    name.to_string(),
                    type_params
                        .iter()
                        .map(|param| Ty::Named(param.clone()))
                        .collect(),
                );
                let (inst_field_tys, inst_ret_ty, _) =
                    self.instantiate_sig(&type_params, &field_tys, &ret_ty);
                let inst_fields = field_defs
                    .iter()
                    .map(|(field_name, _)| field_name.clone())
                    .zip(inst_field_tys)
                    .collect();
                (inst_fields, inst_ret_ty)
            }
            _ => (field_defs.to_vec(), Ty::Named(name.to_string())),
        }
    }

    pub(super) fn apply_struct_args(
        &self,
        name: &str,
        field_defs: &[(String, Ty)],
        args: &[Ty],
    ) -> Option<Vec<(String, Ty)>> {
        let type_params = self.registry.struct_type_params.get(name)?;
        if type_params.len() != args.len() {
            return None;
        }
        let mapping: HashMap<String, Ty> = type_params
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        Some(
            field_defs
                .iter()
                .map(|(field_name, ty)| (field_name.clone(), self.instantiate_ty(ty, &mapping)))
                .collect(),
        )
    }

    pub(super) fn struct_fields_for_type(
        &mut self,
        name: &str,
        field_defs: &[(String, Ty)],
        ty: &Ty,
    ) -> (Vec<(String, Ty)>, Ty) {
        if let Ty::App(actual_name, args) = ty
            && actual_name == name
            && let Some(fields) = self.apply_struct_args(name, field_defs, args)
        {
            return (fields, ty.clone());
        }
        self.instantiate_struct_fields(name, field_defs)
    }

    /// Create fresh type variables for each type parameter and substitute
    /// them into the function signature.
    pub(super) fn instantiate_sig(
        &mut self,
        type_params: &[String],
        param_tys: &[Ty],
        ret_ty: &Ty,
    ) -> (Vec<Ty>, Ty, HashMap<String, Ty>) {
        let mapping: HashMap<String, Ty> = type_params
            .iter()
            .map(|name| (name.clone(), self.fresh_var()))
            .collect();
        let new_params: Vec<Ty> = param_tys
            .iter()
            .map(|t| self.instantiate_ty(t, &mapping))
            .collect();
        let new_ret = self.instantiate_ty(ret_ty, &mapping);
        (new_params, new_ret, mapping)
    }

    fn method_owner_matches(
        &self,
        template: &Ty,
        actual: &Ty,
        type_params: &HashSet<String>,
        mapping: &mut HashMap<String, Ty>,
    ) -> bool {
        let actual = self.apply_subst(actual);
        match template {
            Ty::Named(name) if type_params.contains(name) => {
                if let Some(previous) = mapping.get(name) {
                    self.apply_subst(previous) == actual
                } else {
                    mapping.insert(name.clone(), actual);
                    true
                }
            }
            Ty::Refined(base, _, _) => {
                self.method_owner_matches(base, &actual, type_params, mapping)
            }
            Ty::App(name, args) => {
                let Ty::App(actual_name, actual_args) = actual else {
                    return false;
                };
                name == &actual_name
                    && args.len() == actual_args.len()
                    && args.iter().zip(&actual_args).all(|(template, actual)| {
                        self.method_owner_matches(template, actual, type_params, mapping)
                    })
            }
            _ => template == &actual,
        }
    }

    fn instantiate_method(
        &mut self,
        method: &MethodInfo,
        mut mapping: HashMap<String, Ty>,
    ) -> InstantiatedMethod {
        for type_param in &method.type_params {
            mapping
                .entry(type_param.clone())
                .or_insert_with(|| self.fresh_var());
        }
        InstantiatedMethod {
            params: method
                .params
                .iter()
                .map(|ty| self.instantiate_ty(ty, &mapping))
                .collect(),
            return_type: self.instantiate_ty(&method.return_type, &mapping),
            required_effects: method.required_effects.clone(),
            generic_bounds: method.generic_bounds.clone(),
            type_mapping: mapping,
        }
    }

    fn select_method_candidate(
        &mut self,
        method_name: &str,
        candidates: Vec<(MethodInfo, HashMap<String, Ty>)>,
    ) -> Option<InstantiatedMethod> {
        let prefer_inherent = candidates
            .iter()
            .any(|(candidate, _)| candidate.trait_name.is_none());
        let mut candidates = candidates
            .into_iter()
            .filter(|(candidate, _)| !prefer_inherent || candidate.trait_name.is_none());
        let first = candidates.next()?;
        if candidates.next().is_some() {
            self.err(
                ErrorCode::E0014,
                format!("method call `{method_name}` is ambiguous for the receiver type"),
            );
        }
        Some(self.instantiate_method(&first.0, first.1))
    }

    pub(super) fn lookup_receiver_method(
        &mut self,
        receiver_ty: &Ty,
        method_name: &str,
    ) -> Option<InstantiatedMethod> {
        let candidates = self.registry.methods.get(method_name).cloned()?;
        let matches = candidates
            .into_iter()
            .filter(|candidate| candidate.has_receiver)
            .filter_map(|candidate| {
                let type_params = candidate.type_params.iter().cloned().collect();
                let mut mapping = HashMap::new();
                self.method_owner_matches(&candidate.owner, receiver_ty, &type_params, &mut mapping)
                    .then_some((candidate, mapping))
            })
            .collect();
        self.select_method_candidate(method_name, matches)
    }

    pub(super) fn lookup_static_method(
        &mut self,
        owner_name: &str,
        method_name: &str,
    ) -> Option<InstantiatedMethod> {
        let candidates = self.registry.methods.get(method_name).cloned()?;
        let matches = candidates
            .into_iter()
            .filter(|candidate| !candidate.has_receiver)
            .filter_map(|candidate| {
                self.bound_target_names(&candidate.owner)
                    .iter()
                    .any(|name| name == owner_name)
                    .then_some((candidate, HashMap::new()))
            })
            .collect();
        self.select_method_candidate(method_name, matches)
    }

    pub(super) fn lookup_generic_bound_method(
        &mut self,
        receiver_ty: &Ty,
        method_name: &str,
    ) -> Option<InstantiatedMethod> {
        let Ty::Named(type_var) = self.apply_subst(receiver_ty) else {
            return None;
        };
        let bounds = self
            .registry
            .fn_generic_bounds
            .get(&self.current_function)
            .cloned()
            .unwrap_or_default();
        let candidates = bounds
            .into_iter()
            .filter(|(bounded_type, _)| bounded_type == &type_var)
            .filter_map(|(_, trait_name)| {
                let (type_params, methods) = self.registry.interfaces.get(&trait_name)?.clone();
                if !type_params.is_empty() {
                    return None;
                }
                let (_, params, return_type) = methods
                    .into_iter()
                    .find(|(name, _, _)| name == method_name)?;
                let self_mapping = HashMap::from([("Self".to_string(), receiver_ty.clone())]);
                Some(MethodInfo {
                    owner: receiver_ty.clone(),
                    trait_name: Some(trait_name),
                    type_params: Vec::new(),
                    generic_bounds: Vec::new(),
                    params: params
                        .iter()
                        .map(|ty| self.instantiate_ty(ty, &self_mapping))
                        .collect(),
                    return_type: self.instantiate_ty(&return_type, &self_mapping),
                    required_effects: EffectSet::new(),
                    has_receiver: true,
                })
            })
            .map(|candidate| (candidate, HashMap::new()))
            .collect();
        self.select_method_candidate(method_name, candidates)
    }

    pub(super) fn check_generic_bounds(
        &mut self,
        fn_name: &str,
        type_mapping: &HashMap<String, Ty>,
    ) {
        let Some(constraints) = self.registry.fn_generic_bounds.get(fn_name).cloned() else {
            return;
        };
        self.check_instantiated_bounds(fn_name, &constraints, type_mapping);
    }

    pub(super) fn check_instantiated_bounds(
        &mut self,
        context: &str,
        constraints: &[(String, String)],
        type_mapping: &HashMap<String, Ty>,
    ) {
        for (type_var, bound) in constraints {
            if !self.registry.interfaces.contains_key(bound) {
                self.err(
                    ErrorCode::E0403,
                    format!("unknown trait bound `{bound}` in generic bounds of `{context}`"),
                );
                continue;
            }
            let Some(instantiated) = type_mapping.get(type_var) else {
                continue;
            };
            let resolved = self.apply_subst(instantiated);
            if self.has_unresolved_type_var(&resolved) {
                self.err(
                    ErrorCode::E0404,
                    format!(
                        "cannot infer type parameter `{type_var}` for generic bound `{type_var}: {bound}` in `{context}`"
                    ),
                );
                continue;
            }
            if !self.satisfies_trait_bound(bound, &resolved) {
                self.err(
                    ErrorCode::E0403,
                    format!(
                        "type `{resolved}` does not satisfy generic bound `{type_var}: {bound}` in `{context}`"
                    ),
                );
            }
        }
    }

    pub(super) fn has_unresolved_type_var(&self, ty: &Ty) -> bool {
        let mut found = false;
        ty.visit(&mut |t| {
            if matches!(t, Ty::Var(_)) {
                found = true;
            }
        });
        found
    }

    pub(super) fn satisfies_trait_bound(&self, bound: &str, ty: &Ty) -> bool {
        self.bound_target_names(ty).into_iter().any(|target| {
            self.registry
                .impls
                .contains_key(&(bound.to_string(), target))
        })
    }

    pub(super) fn bound_target_names(&self, ty: &Ty) -> Vec<String> {
        match ty {
            Ty::Refined(base, _, _) => self.bound_target_names(base),
            Ty::Named(name) | Ty::App(name, _) => vec![name.clone()],
            Ty::I8 => vec!["I8".into()],
            Ty::I16 => vec!["I16".into()],
            Ty::I32 => vec!["I32".into()],
            Ty::I64 => vec!["I64".into()],
            Ty::U8 => vec!["U8".into()],
            Ty::U16 => vec!["U16".into()],
            Ty::U32 => vec!["U32".into()],
            Ty::U64 => vec!["U64".into()],
            Ty::F32 => vec!["F32".into()],
            Ty::F64 => vec!["F64".into()],
            Ty::Bool => vec!["Bool".into()],
            Ty::Str => vec!["Str".into()],
            Ty::Unit => vec!["Unit".into()],
            Ty::Never => vec!["Never".into()],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> Checker {
        Checker::new()
    }

    // ── resolve_type ────────────────────────────────────────────────

    #[test]
    fn resolve_primitives() {
        let mut c = checker();
        assert_eq!(c.resolve_type(&TypeExpr::Named("I32".into())), Ty::I32);
        assert_eq!(c.resolve_type(&TypeExpr::Named("Bool".into())), Ty::Bool);
        assert_eq!(c.resolve_type(&TypeExpr::Named("Str".into())), Ty::Str);
        assert_eq!(c.resolve_type(&TypeExpr::Named("Never".into())), Ty::Never);
        assert_eq!(c.resolve_type(&TypeExpr::Named("F64".into())), Ty::F64);
    }

    #[test]
    fn resolve_unknown_named_type() {
        let mut c = checker();
        assert_eq!(
            c.resolve_type(&TypeExpr::Named("Foo".into())),
            Ty::Named("Foo".into())
        );
    }

    #[test]
    fn resolve_type_alias() {
        let mut c = checker();
        c.registry.type_aliases.insert("Port".into(), Ty::I32);
        assert_eq!(c.resolve_type(&TypeExpr::Named("Port".into())), Ty::I32);
    }

    #[test]
    fn resolve_generic_type() {
        let mut c = checker();
        let te = TypeExpr::Generic("List".into(), vec![TypeExpr::Named("I32".into())]);
        assert_eq!(c.resolve_type(&te), Ty::App("List".into(), vec![Ty::I32]));
    }

    #[test]
    fn resolve_empty_tuple_is_unit() {
        let mut c = checker();
        assert_eq!(c.resolve_type(&TypeExpr::Tuple(vec![])), Ty::Unit);
    }

    #[test]
    fn resolve_non_empty_tuple() {
        let mut c = checker();
        let te = TypeExpr::Tuple(vec![
            TypeExpr::Named("I32".into()),
            TypeExpr::Named("Bool".into()),
        ]);
        assert_eq!(c.resolve_type(&te), Ty::Tuple(vec![Ty::I32, Ty::Bool]));
    }

    #[test]
    fn resolve_function_type() {
        let mut c = checker();
        let te = TypeExpr::Function(
            vec![TypeExpr::Named("I32".into())],
            Box::new(TypeExpr::Named("Bool".into())),
        );
        let resolved = c.resolve_type(&te);
        match resolved {
            Ty::Fn(params, ret, effects) => {
                assert_eq!(params, vec![Ty::I32]);
                assert_eq!(*ret, Ty::Bool);
                assert!(effects.is_empty());
            }
            other => panic!("expected Fn, got {other}"),
        }
    }

    #[test]
    fn resolve_hole_produces_fresh_var() {
        let mut c = checker();
        let v1 = c.resolve_type(&TypeExpr::Hole(None));
        let v2 = c.resolve_type(&TypeExpr::Hole(None));
        assert_ne!(v1, v2, "each hole should produce a distinct type variable");
    }

    // ── fresh_var ───────────────────────────────────────────────────

    #[test]
    fn fresh_var_increments() {
        let mut c = checker();
        let v0 = c.fresh_var();
        let v1 = c.fresh_var();
        assert_eq!(v0, Ty::Var(0));
        assert_eq!(v1, Ty::Var(1));
    }

    // ── apply_subst ─────────────────────────────────────────────────

    #[test]
    fn apply_subst_resolves_chain() {
        let mut c = checker();
        c.substitution.insert(0, Ty::Var(1));
        c.substitution.insert(1, Ty::I32);
        assert_eq!(c.apply_subst(&Ty::Var(0)), Ty::I32);
    }

    #[test]
    fn apply_subst_leaves_unbound_var() {
        let c = checker();
        assert_eq!(c.apply_subst(&Ty::Var(99)), Ty::Var(99));
    }

    #[test]
    fn apply_subst_through_app() {
        let mut c = checker();
        c.substitution.insert(0, Ty::I32);
        let ty = Ty::App("List".into(), vec![Ty::Var(0)]);
        assert_eq!(c.apply_subst(&ty), Ty::App("List".into(), vec![Ty::I32]));
    }

    // ── occurs_in ───────────────────────────────────────────────────

    #[test]
    fn occurs_in_direct() {
        let c = checker();
        assert!(c.occurs_in(0, &Ty::Var(0)));
    }

    #[test]
    fn occurs_in_nested() {
        let c = checker();
        let ty = Ty::App("List".into(), vec![Ty::Var(0)]);
        assert!(c.occurs_in(0, &ty));
    }

    #[test]
    fn occurs_in_absent() {
        let c = checker();
        let ty = Ty::App("List".into(), vec![Ty::Var(1)]);
        assert!(!c.occurs_in(0, &ty));
    }

    #[test]
    fn occurs_in_resolves_substitution() {
        let mut c = checker();
        c.substitution
            .insert(0, Ty::App("List".into(), vec![Ty::Var(1)]));
        assert!(c.occurs_in(1, &Ty::Var(0)));
    }

    // ── instantiate_sig ─────────────────────────────────────────────

    #[test]
    fn instantiate_sig_creates_fresh_vars() {
        let mut c = checker();
        let params = vec![Ty::Named("T".into())];
        let ret = Ty::Named("T".into());
        let (new_params, new_ret, mapping) = c.instantiate_sig(&["T".into()], &params, &ret);
        // Both should be the same fresh var
        assert_eq!(new_params[0], new_ret);
        match &new_params[0] {
            Ty::Var(_) => {} // ok
            other => panic!("expected Var, got {other}"),
        }
        assert!(mapping.contains_key("T"));
    }

    // ── bound_target_names ──────────────────────────────────────────

    #[test]
    fn bound_target_names_primitives() {
        let c = checker();
        assert_eq!(c.bound_target_names(&Ty::I32), vec!["I32"]);
        assert_eq!(c.bound_target_names(&Ty::Bool), vec!["Bool"]);
        assert_eq!(c.bound_target_names(&Ty::Str), vec!["Str"]);
    }

    #[test]
    fn bound_target_names_refined() {
        let c = checker();
        let refined = Ty::Refined(Box::new(Ty::I32), "x".into(), Box::new(Expr::BoolLit(true)));
        assert_eq!(c.bound_target_names(&refined), vec!["I32"]);
    }

    #[test]
    fn bound_target_names_named() {
        let c = checker();
        assert_eq!(c.bound_target_names(&Ty::Named("Foo".into())), vec!["Foo"]);
    }

    #[test]
    fn bound_target_names_app() {
        let c = checker();
        assert_eq!(
            c.bound_target_names(&Ty::App("List".into(), vec![Ty::I32])),
            vec!["List"]
        );
    }
}
