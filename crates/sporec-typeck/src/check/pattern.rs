use super::*;

impl Checker {
    // ── Pattern type checking ──────────────────────────────────────

    fn variant_pattern_field_types(&mut self, name: &str, scrutinee_ty: &Ty) -> Option<Vec<Ty>> {
        match scrutinee_ty {
            Ty::Named(type_name) => {
                let field_tys = self
                    .registry
                    .types
                    .get(type_name)
                    .and_then(|variants| variants.iter().find(|(vname, _)| vname == name))
                    .map(|(_, field_tys)| field_tys.clone());
                if let Some(field_tys) = field_tys {
                    let expected_ty = Ty::Named(type_name.clone());
                    self.unify(&expected_ty, scrutinee_ty, &format!("pattern `{name}`"));
                    return Some(field_tys);
                }
            }
            Ty::App(type_name, args) => {
                let field_tys = self
                    .registry
                    .types
                    .get(type_name)
                    .and_then(|variants| variants.iter().find(|(vname, _)| vname == name))
                    .map(|(_, field_tys)| field_tys.clone());
                if let Some(field_tys) = field_tys {
                    if let Some(type_params) = self.registry.type_type_params.get(type_name)
                        && type_params.len() == args.len()
                    {
                        let mapping: HashMap<String, Ty> = type_params
                            .iter()
                            .cloned()
                            .zip(args.iter().cloned())
                            .collect();
                        return Some(
                            field_tys
                                .iter()
                                .map(|ty| self.instantiate_ty(ty, &mapping))
                                .collect(),
                        );
                    }
                    return Some(field_tys);
                }
            }
            _ => {}
        }

        #[allow(clippy::type_complexity)]
        let types_snapshot: Vec<(String, Vec<(String, Vec<Ty>)>)> = self
            .registry
            .types
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (type_name, variants) in &types_snapshot {
            if let Some((_, field_tys)) = variants.iter().find(|(vname, _)| vname == name) {
                let expected_ty = match self.registry.type_type_params.get(type_name).cloned() {
                    Some(type_params) if !type_params.is_empty() => {
                        let field_tys = field_tys.clone();
                        let ret_ty = Ty::App(
                            type_name.clone(),
                            type_params
                                .iter()
                                .map(|param| Ty::Named(param.clone()))
                                .collect(),
                        );
                        let (inst_field_tys, inst_ret_ty, _) =
                            self.instantiate_sig(&type_params, &field_tys, &ret_ty);
                        self.unify(&inst_ret_ty, scrutinee_ty, &format!("pattern `{name}`"));
                        return Some(
                            inst_field_tys
                                .into_iter()
                                .map(|ty| self.apply_subst(&ty))
                                .collect(),
                        );
                    }
                    _ => Ty::Named(type_name.clone()),
                };
                self.unify(&expected_ty, scrutinee_ty, &format!("pattern `{name}`"));
                return Some(field_tys.clone());
            }
        }

        None
    }

    /// Check if `name` is a known zero-field enum variant.
    pub(super) fn find_unit_variant(&self, name: &str) -> Option<String> {
        for (type_name, variants) in &self.registry.types {
            if variants
                .iter()
                .any(|(vname, fields)| vname == name && fields.is_empty())
            {
                return Some(type_name.clone());
            }
        }
        None
    }

    /// Type-check a pattern against a scrutinee type.
    /// Returns bindings introduced by the pattern (name -> type).
    pub(super) fn check_pattern(
        &mut self,
        pattern: &Pattern,
        scrutinee_ty: &Ty,
    ) -> Vec<(String, Ty)> {
        match pattern {
            Pattern::Wildcard => vec![],
            Pattern::Var(name) => {
                // Zero-field enum variants (e.g. Red, None) are parsed as Var.
                if let Some(field_tys) = self.variant_pattern_field_types(name, scrutinee_ty)
                    && field_tys.is_empty()
                {
                    vec![]
                } else {
                    vec![(name.clone(), scrutinee_ty.clone())]
                }
            }
            Pattern::IntLit(_) => {
                if !scrutinee_ty.is_integer() && !scrutinee_ty.is_error() {
                    self.err(
                        ErrorCode::E0011,
                        format!("integer pattern cannot match type `{scrutinee_ty}`"),
                    );
                }
                vec![]
            }
            Pattern::StrLit(_) => {
                if *scrutinee_ty != Ty::Str && !scrutinee_ty.is_error() {
                    self.err(
                        ErrorCode::E0011,
                        format!("string pattern cannot match type `{scrutinee_ty}`"),
                    );
                }
                vec![]
            }
            Pattern::BoolLit(_) => {
                if *scrutinee_ty != Ty::Bool && !scrutinee_ty.is_error() {
                    self.err(
                        ErrorCode::E0011,
                        format!("boolean pattern cannot match type `{scrutinee_ty}`"),
                    );
                }
                vec![]
            }
            Pattern::Constructor(name, sub_pats) => {
                if let Some(field_tys) = self.variant_pattern_field_types(name, scrutinee_ty) {
                    if sub_pats.len() != field_tys.len() {
                        self.err(
                            ErrorCode::E0007,
                            format!(
                                "variant `{name}` expects {} fields, got {}",
                                field_tys.len(),
                                sub_pats.len()
                            ),
                        );
                    }

                    let mut bindings = vec![];
                    for (sub_pat, field_ty) in sub_pats.iter().zip(field_tys.iter()) {
                        bindings.extend(self.check_pattern(sub_pat, field_ty));
                    }
                    bindings
                } else if !scrutinee_ty.is_error() {
                    self.err(ErrorCode::E0006, format!("unknown variant `{name}`"));
                    vec![]
                } else {
                    vec![]
                }
            }
            Pattern::Struct(name, field_pats) => {
                let def_fields = self.registry.structs.get(name).cloned();
                if let Some(def_fields) = def_fields {
                    let (def_fields, expected_ty) =
                        self.struct_fields_for_type(name, &def_fields, scrutinee_ty);
                    self.unify(
                        &expected_ty,
                        scrutinee_ty,
                        &format!("struct pattern `{name}`"),
                    );

                    let mut bindings = vec![];
                    for (fname, fpat) in field_pats {
                        if let Some((_, fty)) = def_fields.iter().find(|(n, _)| n == fname) {
                            bindings.extend(self.check_pattern(fpat, fty));
                        } else {
                            self.err(
                                ErrorCode::E0015,
                                format!("struct `{name}` has no field `{fname}`"),
                            );
                        }
                    }
                    bindings
                } else {
                    if !scrutinee_ty.is_error() {
                        self.err(
                            ErrorCode::E0005,
                            format!("unknown struct `{name}` in pattern"),
                        );
                    }
                    vec![]
                }
            }
            Pattern::Or(pats) => {
                if pats.is_empty() {
                    return vec![];
                }
                let first_bindings = self.check_pattern(&pats[0], scrutinee_ty);
                let first_names: std::collections::BTreeSet<&str> =
                    first_bindings.iter().map(|(n, _)| n.as_str()).collect();

                for pat in &pats[1..] {
                    let alt_bindings = self.check_pattern(pat, scrutinee_ty);
                    let alt_names: std::collections::BTreeSet<&str> =
                        alt_bindings.iter().map(|(n, _)| n.as_str()).collect();

                    if first_names != alt_names {
                        self.err(
                            ErrorCode::E0504,
                            format!(
                                "or-pattern alternatives must bind the same names: expected {first_names:?}, found {alt_names:?}",
                            ),
                        );
                    } else {
                        for ((name, ty1), (_, ty2)) in
                            first_bindings.iter().zip(alt_bindings.iter())
                        {
                            self.unify(
                                ty1,
                                ty2,
                                &format!(
                                    "or-pattern binding `{name}` type mismatch across alternatives"
                                ),
                            );
                        }
                    }
                }
                first_bindings
            }
            Pattern::List(elements, _rest) => {
                // For list patterns, the scrutinee should be a list type
                let elem_ty = self.fresh_var();
                let list_ty = Ty::App("List".into(), vec![elem_ty.clone()]);
                self.unify(&list_ty, scrutinee_ty, "list pattern");
                let mut bindings = vec![];
                for pat in elements {
                    bindings.extend(self.check_pattern(pat, &elem_ty));
                }
                if let Some(rest_name) = _rest {
                    bindings.push((rest_name.clone(), scrutinee_ty.clone()));
                }
                bindings
            }
        }
    }

    // ── Exhaustiveness checking ─────────────────────────────────────

    /// Check if match arms exhaustively cover the scrutinee type.
    pub(super) fn check_exhaustiveness(&mut self, scrutinee_ty: &Ty, arms: &[MatchArm]) {
        if scrutinee_ty.is_error() {
            return;
        }

        let has_catch_all = arms.iter().any(|arm| {
            let is_catch_all = match &arm.pattern {
                Pattern::Wildcard => true,
                Pattern::Var(name) => self.find_unit_variant(name).is_none(),
                _ => false,
            };
            is_catch_all && arm.guard.is_none()
        });
        if has_catch_all {
            return;
        }

        match scrutinee_ty {
            Ty::Bool => {
                let has_true = arms
                    .iter()
                    .any(|arm| pattern_contains_bool(&arm.pattern, true));
                let has_false = arms
                    .iter()
                    .any(|arm| pattern_contains_bool(&arm.pattern, false));
                if !has_true || !has_false {
                    let mut missing = vec![];
                    if !has_true {
                        missing.push("true");
                    }
                    if !has_false {
                        missing.push("false");
                    }
                    self.err(
                        ErrorCode::E0010,
                        format!(
                            "non-exhaustive match: missing pattern(s) {}",
                            missing.join(", ")
                        ),
                    );
                }
            }
            Ty::Named(name) => {
                if let Some(variants) = self.registry.types.get(name).cloned() {
                    let variant_names: Vec<String> =
                        variants.iter().map(|(n, _)| n.clone()).collect();
                    let mut covered: Vec<bool> = vec![false; variant_names.len()];

                    for arm in arms {
                        mark_covered_variants(&arm.pattern, &variant_names, &mut covered);
                    }

                    let missing: Vec<&str> = variant_names
                        .iter()
                        .zip(covered.iter())
                        .filter(|(_, c)| !**c)
                        .map(|(n, _)| n.as_str())
                        .collect();

                    if !missing.is_empty() {
                        self.err(
                            ErrorCode::E0010,
                            format!(
                                "non-exhaustive match on `{name}`: missing variant(s) {}",
                                missing.join(", ")
                            ),
                        );
                    }
                }
            }
            Ty::I8
            | Ty::I16
            | Ty::I32
            | Ty::I64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::F32
            | Ty::F64
            | Ty::Str => {
                self.err(
                    ErrorCode::E0010,
                    format!(
                        "non-exhaustive match: `{}` requires a wildcard `_` or variable pattern",
                        scrutinee_ty
                    ),
                );
            }
            Ty::App(name, _args) => {
                // For parameterized types like Option[Int], look up the base type's variants
                if let Some(variants) = self.registry.types.get(name).cloned() {
                    let variant_names: Vec<String> =
                        variants.iter().map(|(n, _)| n.clone()).collect();
                    let mut covered: Vec<bool> = vec![false; variant_names.len()];
                    for arm in arms {
                        mark_covered_variants(&arm.pattern, &variant_names, &mut covered);
                    }
                    let missing: Vec<&str> = variant_names
                        .iter()
                        .zip(covered.iter())
                        .filter(|(_, c)| !**c)
                        .map(|(n, _)| n.as_str())
                        .collect();
                    if !missing.is_empty() {
                        self.err(
                            ErrorCode::E0010,
                            format!(
                                "non-exhaustive match on `{}`: missing variant(s) {}",
                                scrutinee_ty,
                                missing.join(", ")
                            ),
                        );
                    }
                }
            }
            _ => {}
        }
    }
}
