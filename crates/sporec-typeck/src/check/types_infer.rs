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
                Ty::App(name.clone(), resolved)
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
            TypeExpr::Function(params, ret, error_exprs) => {
                let ptys = params
                    .iter()
                    .map(|p| self.resolve_signature_type(p, signature_holes))
                    .collect();
                let errors = crate::types::declared_error_set(error_exprs);
                Ty::Fn(
                    ptys,
                    Box::new(self.resolve_signature_type(ret, signature_holes)),
                    EffectSet::new(),
                    errors,
                )
            }
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
                    // Check type aliases (supports refined aliases like `alias Port = I64 when ...`)
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
                Ty::App(name.clone(), resolved)
            }
            TypeExpr::Tuple(types) => {
                if types.is_empty() {
                    Ty::Unit
                } else {
                    Ty::Tuple(types.iter().map(|t| self.resolve_type(t)).collect())
                }
            }
            TypeExpr::Function(params, ret, error_exprs) => {
                let ptys: Vec<Ty> = params.iter().map(|p| self.resolve_type(p)).collect();
                let errors = crate::types::declared_error_set(error_exprs);
                Ty::Fn(
                    ptys,
                    Box::new(self.resolve_type(ret)),
                    EffectSet::new(),
                    errors,
                )
            }
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

    pub(super) fn infer_hole_type_from_allows(&self, allow_list: &[String]) -> Option<Ty> {
        let mut inferred: Option<Ty> = None;

        for allowed_name in allow_list {
            let candidate = if let Some(ty) = self.env.lookup(allowed_name) {
                if let Ty::Fn(_, ret, _, _) = ty {
                    Some(ret.as_ref().clone())
                } else {
                    None
                }
            } else {
                self.registry
                    .functions
                    .get(allowed_name)
                    .map(|(_, ret_ty, _)| ret_ty.clone())
            };

            let Some(candidate_ty) = candidate.map(|t| self.apply_subst(&t)) else {
                continue;
            };
            if candidate_ty.is_error() || matches!(candidate_ty, Ty::Hole(_)) {
                continue;
            }

            match &inferred {
                Some(existing) if existing != &candidate_ty => return None,
                Some(_) => {}
                None => inferred = Some(candidate_ty),
            }
        }

        inferred
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

    pub(super) fn check_where_bounds(&mut self, fn_name: &str, type_mapping: &HashMap<String, Ty>) {
        let Some(constraints) = self.registry.fn_where_bounds.get(fn_name).cloned() else {
            return;
        };
        for (type_var, bound) in constraints {
            if !self.registry.interfaces.contains_key(&bound) {
                self.err(
                    ErrorCode::E0403,
                    format!("unknown trait bound `{bound}` in where clause of `{fn_name}`"),
                );
                continue;
            }
            let Some(instantiated) = type_mapping.get(&type_var) else {
                continue;
            };
            let resolved = self.apply_subst(instantiated);
            if self.has_unresolved_type_var(&resolved) {
                self.err(
                    ErrorCode::E0404,
                    format!(
                        "cannot infer type parameter `{type_var}` for where bound `{type_var}: {bound}` in `{fn_name}`"
                    ),
                );
                continue;
            }
            if !self.satisfies_trait_bound(&bound, &resolved) {
                self.err(
                    ErrorCode::E0403,
                    format!(
                        "type `{resolved}` does not satisfy where bound `{type_var}: {bound}` in `{fn_name}`"
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
            vec![],
        );
        let resolved = c.resolve_type(&te);
        match resolved {
            Ty::Fn(params, ret, effects, errors) => {
                assert_eq!(params, vec![Ty::I32]);
                assert_eq!(*ret, Ty::Bool);
                assert!(effects.is_empty());
                assert!(errors.is_empty());
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
