use super::*;

impl Checker {
    // ── Unification with type variable support ─────────────────────

    pub(super) fn unify(&mut self, expected: &Ty, actual: &Ty, context: &str) {
        let e = self.apply_subst(expected);
        let a = self.apply_subst(actual);
        if e.is_error() || a.is_error() {
            return;
        }
        if e == a {
            return;
        }
        if matches!((&e, &a), (Ty::Unit, Ty::Tuple(v)) | (Ty::Tuple(v), Ty::Unit) if v.is_empty()) {
            return;
        }
        if matches!(e, Ty::Hole(_)) || matches!(a, Ty::Hole(_)) {
            return;
        }

        if matches!(a, Ty::Never) {
            return;
        }

        if let Ty::Var(id) = e {
            if self.occurs_in(id, &a) {
                self.err(
                    ErrorCode::E0003,
                    format!("infinite type: ?T{id} occurs in `{a}`"),
                );
                return;
            }
            self.substitution.insert(id, a);
            return;
        }
        if let Ty::Var(id) = a {
            if self.occurs_in(id, &e) {
                self.err(
                    ErrorCode::E0003,
                    format!("infinite type: ?T{id} occurs in `{e}`"),
                );
                return;
            }
            self.substitution.insert(id, e);
            return;
        }

        match (&e, &a) {
            (Ty::Fn(p1, r1, c1), Ty::Fn(p2, r2, c2)) if p1.len() == p2.len() => {
                let pairs: Vec<(Ty, Ty)> = p1.iter().cloned().zip(p2.iter().cloned()).collect();
                let ret_pair = ((**r1).clone(), (**r2).clone());
                for (x, y) in &pairs {
                    self.unify(x, y, context);
                }
                self.unify(&ret_pair.0, &ret_pair.1, context);
                let missing_effects = c1.missing_from(c2);
                if !missing_effects.is_empty() {
                    self.err(
                        ErrorCode::F0001,
                        format!(
                            "function effect mismatch in {context}: expected `{e}` but got `{a}` requiring effects [{}]",
                            missing_effects.join(", ")
                        ),
                    );
                }
            }
            (Ty::Outcome(success1, failure1), Ty::Outcome(success2, failure2)) => {
                self.unify(success1, success2, context);
                self.unify(failure1, failure2, context);
            }
            (Ty::App(n1, a1), Ty::App(n2, a2)) if n1 == n2 && a1.len() == a2.len() => {
                let pairs: Vec<(Ty, Ty)> = a1.iter().cloned().zip(a2.iter().cloned()).collect();
                for (x, y) in &pairs {
                    self.unify(x, y, context);
                }
            }
            (Ty::Tuple(t1), Ty::Tuple(t2)) if t1.len() == t2.len() => {
                let pairs: Vec<(Ty, Ty)> = t1.iter().cloned().zip(t2.iter().cloned()).collect();
                for (x, y) in &pairs {
                    self.unify(x, y, context);
                }
            }
            (Ty::Record(expected_fields), Ty::Record(actual_fields)) => {
                for (ename, ety) in expected_fields {
                    if let Some((_, aty)) = actual_fields.iter().find(|(n, _)| n == ename) {
                        self.unify(ety, aty, context);
                    } else {
                        self.err(
                            ErrorCode::E0001,
                            format!("type mismatch in {context}: record missing field `{ename}`"),
                        );
                    }
                }
            }
            (Ty::Refined(b1, _, _), Ty::Refined(b2, _, _)) => {
                let base1 = (**b1).clone();
                let base2 = (**b2).clone();
                self.unify(&base1, &base2, context);
            }
            (Ty::Refined(base, _, _), other) => {
                let base = (**base).clone();
                self.unify(&base, other, context);
            }
            (other, Ty::Refined(base, _, _)) => {
                let base = (**base).clone();
                self.unify(other, &base, context);
            }
            _ => {
                self.err(
                    ErrorCode::E0001,
                    format!("type mismatch in {context}: expected `{e}`, got `{a}`"),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> Checker {
        Checker::new()
    }

    #[test]
    fn unify_same_primitives() {
        let mut c = checker();
        c.unify(&Ty::I32, &Ty::I32, "test");
        assert!(c.errors.is_empty());
    }

    #[test]
    fn unify_mismatch_primitives() {
        let mut c = checker();
        c.unify(&Ty::I32, &Ty::Bool, "test");
        assert_eq!(c.errors.len(), 1);
        assert!(c.errors[0].message.contains("I32"));
        assert!(c.errors[0].message.contains("Bool"));
    }

    #[test]
    fn unify_unit_with_empty_tuple() {
        let mut c = checker();
        c.unify(&Ty::Unit, &Ty::Tuple(vec![]), "test");
        assert!(c.errors.is_empty());
    }

    #[test]
    fn unify_never_with_anything() {
        let mut c = checker();
        c.unify(&Ty::I32, &Ty::Never, "test");
        assert!(c.errors.is_empty());
    }

    #[test]
    fn unify_hole_skips() {
        let mut c = checker();
        c.unify(&Ty::Hole("x".into()), &Ty::I32, "test");
        assert!(c.errors.is_empty());
    }

    #[test]
    fn unify_error_sentinel_skips() {
        let mut c = checker();
        c.unify(&Ty::Error, &Ty::I32, "test");
        assert!(c.errors.is_empty());
    }

    #[test]
    fn unify_type_var_binds() {
        let mut c = checker();
        c.unify(&Ty::Var(0), &Ty::I32, "test");
        assert!(c.errors.is_empty());
        assert_eq!(c.substitution.get(&0), Some(&Ty::I32));
    }

    #[test]
    fn unify_type_var_occurs_check() {
        let mut c = checker();
        // Var(0) occurs in App("List", [Var(0)])
        let ty = Ty::App("List".into(), vec![Ty::Var(0)]);
        c.unify(&Ty::Var(0), &ty, "test");
        assert_eq!(c.errors.len(), 1);
        assert!(c.errors[0].message.contains("infinite type"));
    }

    #[test]
    fn unify_fn_same_arity() {
        let mut c = checker();
        let f1 = Ty::Fn(vec![Ty::I32], Box::new(Ty::Bool), EffectSet::new());
        let f2 = Ty::Fn(vec![Ty::I32], Box::new(Ty::Bool), EffectSet::new());
        c.unify(&f1, &f2, "test");
        assert!(c.errors.is_empty());
    }

    #[test]
    fn unify_fn_mismatch_arity() {
        let mut c = checker();
        let f1 = Ty::Fn(vec![Ty::I32], Box::new(Ty::Bool), EffectSet::new());
        let f2 = Ty::Fn(vec![Ty::I32, Ty::I32], Box::new(Ty::Bool), EffectSet::new());
        c.unify(&f1, &f2, "test");
        assert_eq!(c.errors.len(), 1);
    }

    #[test]
    fn unify_fn_effect_mismatch() {
        let mut c = checker();
        let mut callee_effects = EffectSet::new();
        callee_effects.insert("IO".into());
        let f1 = Ty::Fn(vec![], Box::new(Ty::Unit), EffectSet::new());
        let f2 = Ty::Fn(vec![], Box::new(Ty::Unit), callee_effects);
        c.unify(&f1, &f2, "test");
        assert_eq!(c.errors.len(), 1);
        assert!(c.errors[0].message.contains("IO"));
    }

    #[test]
    fn unify_tuple_same_length() {
        let mut c = checker();
        let t1 = Ty::Tuple(vec![Ty::I32, Ty::Bool]);
        let t2 = Ty::Tuple(vec![Ty::I32, Ty::Bool]);
        c.unify(&t1, &t2, "test");
        assert!(c.errors.is_empty());
    }

    #[test]
    fn unify_outcome_requires_an_outcome_on_both_sides() {
        let mut c = checker();
        let outcome = Ty::Outcome(Box::new(Ty::I32), Box::new(Ty::Str));
        c.unify(&outcome, &Ty::I32, "test");
        assert_eq!(c.errors.len(), 1);
    }

    #[test]
    fn unify_tuple_mismatch_length() {
        let mut c = checker();
        let t1 = Ty::Tuple(vec![Ty::I32]);
        let t2 = Ty::Tuple(vec![Ty::I32, Ty::Bool]);
        c.unify(&t1, &t2, "test");
        assert_eq!(c.errors.len(), 1);
    }

    #[test]
    fn unify_app_same_name_and_args() {
        let mut c = checker();
        let a = Ty::App("List".into(), vec![Ty::I32]);
        let b = Ty::App("List".into(), vec![Ty::I32]);
        c.unify(&a, &b, "test");
        assert!(c.errors.is_empty());
    }

    #[test]
    fn unify_app_different_name() {
        let mut c = checker();
        let a = Ty::App("List".into(), vec![Ty::I32]);
        let b = Ty::App("Set".into(), vec![Ty::I32]);
        c.unify(&a, &b, "test");
        assert_eq!(c.errors.len(), 1);
    }

    #[test]
    fn unify_record_missing_field() {
        let mut c = checker();
        let expected = Ty::Record(vec![("x".into(), Ty::I32), ("y".into(), Ty::I32)]);
        let actual = Ty::Record(vec![("x".into(), Ty::I32)]);
        c.unify(&expected, &actual, "test");
        assert_eq!(c.errors.len(), 1);
        assert!(c.errors[0].message.contains("y"));
    }

    #[test]
    fn unify_refined_strips_refinement() {
        let mut c = checker();
        let refined = Ty::Refined(Box::new(Ty::I32), "x".into(), Box::new(Expr::BoolLit(true)));
        c.unify(&refined, &Ty::I32, "test");
        assert!(c.errors.is_empty());
    }

    #[test]
    fn unify_applies_substitution() {
        let mut c = checker();
        c.substitution.insert(0, Ty::I32);
        // Var(0) should resolve to I32 via substitution
        c.unify(&Ty::Var(0), &Ty::I32, "test");
        assert!(c.errors.is_empty());
    }
}
