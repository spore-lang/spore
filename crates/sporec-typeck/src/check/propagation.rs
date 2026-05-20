use super::*;

impl Checker {
    // ── Set propagation checks ─────────────────────────────────────

    /// Verify that the current function's effect set is a superset of the callee's.
    pub(super) fn check_effect_propagation(&mut self, callee_effects: &EffectSet) {
        self.observe_effects(callee_effects);
        let missing = find_missing_set_items(callee_effects, &self.current_effects);
        if !missing.is_empty() {
            self.err(
                ErrorCode::C0001,
                format!(
                    "missing effects [{}]: caller does not declare them",
                    missing.join(", ")
                ),
            );
        }
    }

    /// Verify that the current function's error set is a superset of the callee's.
    pub(super) fn check_error_propagation(&mut self, callee_errors: &ErrorSet) {
        let missing = crate::types::missing_errors(callee_errors, &self.current_errors);
        if !missing.is_empty() {
            self.err(
                ErrorCode::E0012,
                format!(
                    "missing errors [{}] in `?`: caller does not declare them in its error set",
                    missing.join(", ")
                ),
            );
        }
    }

    pub(super) fn check_throw_coverage(&mut self, thrown_expr: &Expr) {
        if self.current_errors.is_empty() {
            self.err(
                ErrorCode::E0012,
                format!(
                    "`throw` in `{}` requires declaring an error set with `! E`",
                    self.current_function
                ),
            );
            return;
        }

        let Some(thrown_name) = self.infer_thrown_error_name(thrown_expr) else {
            return;
        };
        if !self.current_errors.contains(&thrown_name) {
            self.err(
                ErrorCode::E0012,
                format!(
                    "thrown error `{thrown_name}` is not declared in `{}` error set",
                    self.current_function
                ),
            );
        }
    }

    pub(super) fn infer_thrown_error_name(&self, expr: &Expr) -> Option<String> {
        pub(super) fn looks_like_error_name(name: &str) -> bool {
            name.chars().next().is_some_and(char::is_uppercase)
        }

        match expr {
            Expr::Var(name) if looks_like_error_name(name) => Some(name.clone()),
            Expr::Call(callee, _) => match callee.as_ref() {
                Expr::Var(name) if looks_like_error_name(name) => Some(name.clone()),
                _ => None,
            },
            Expr::StructLit(name, _) if looks_like_error_name(name) => Some(name.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn checker_with_effects(effects: Vec<&str>) -> Checker {
        let mut c = Checker::new();
        c.current_function = "test_fn".into();
        for e in effects {
            c.current_effects.insert(e.into());
        }
        c
    }

    fn checker_with_errors(errors: Vec<&str>) -> Checker {
        let mut c = Checker::new();
        c.current_function = "test_fn".into();
        for e in errors {
            c.current_errors.insert(e.into());
        }
        c
    }

    // ── check_effect_propagation ────────────────────────────────────

    #[test]
    fn effect_propagation_ok_when_covered() {
        let mut c = checker_with_effects(vec!["IO", "NetConnect"]);
        c.check_effect_propagation(&{
            let mut s = EffectSet::new();
            s.insert("IO".into());
            s
        });
        assert!(c.errors.is_empty());
    }

    #[test]
    fn effect_propagation_fails_when_missing() {
        let mut c = checker_with_effects(vec!["NetConnect"]);
        c.check_effect_propagation(&{
            let mut s = EffectSet::new();
            s.insert("IO".into());
            s
        });
        assert_eq!(c.errors.len(), 1);
        assert!(c.errors[0].message.contains("IO"));
    }

    #[test]
    fn effect_propagation_empty_callee_ok() {
        let mut c = checker_with_effects(vec![]);
        c.check_effect_propagation(&EffectSet::new());
        assert!(c.errors.is_empty());
    }

    #[test]
    fn effect_propagation_multiple_missing() {
        let mut c = checker_with_effects(vec![]);
        let mut required = EffectSet::new();
        required.insert("IO".into());
        required.insert("NetConnect".into());
        c.check_effect_propagation(&required);
        assert_eq!(c.errors.len(), 1);
        assert!(c.errors[0].message.contains("IO"));
        assert!(c.errors[0].message.contains("NetConnect"));
    }

    // ── check_error_propagation ─────────────────────────────────────

    #[test]
    fn error_propagation_ok_when_covered() {
        let mut c = checker_with_errors(vec!["NotFound", "Timeout"]);
        c.check_error_propagation(&{
            let mut s = BTreeSet::new();
            s.insert("NotFound".into());
            s
        });
        assert!(c.errors.is_empty());
    }

    #[test]
    fn error_propagation_fails_when_missing() {
        let mut c = checker_with_errors(vec!["Timeout"]);
        c.check_error_propagation(&{
            let mut s = BTreeSet::new();
            s.insert("NotFound".into());
            s
        });
        assert_eq!(c.errors.len(), 1);
        assert!(c.errors[0].message.contains("NotFound"));
    }

    // ── check_throw_coverage ────────────────────────────────────────

    #[test]
    fn throw_requires_error_set() {
        let mut c = checker_with_errors(vec![]);
        c.check_throw_coverage(&Expr::Var("NotFound".into()));
        assert_eq!(c.errors.len(), 1);
        assert!(c.errors[0].message.contains("error set"));
    }

    #[test]
    fn throw_covered_by_declared_error() {
        let mut c = checker_with_errors(vec!["NotFound"]);
        c.check_throw_coverage(&Expr::Var("NotFound".into()));
        assert!(c.errors.is_empty());
    }

    #[test]
    fn throw_not_in_declared_errors() {
        let mut c = checker_with_errors(vec!["Timeout"]);
        c.check_throw_coverage(&Expr::Var("NotFound".into()));
        assert_eq!(c.errors.len(), 1);
        assert!(c.errors[0].message.contains("NotFound"));
    }

    // ── infer_thrown_error_name ─────────────────────────────────────

    #[test]
    fn infer_error_from_var() {
        let c = Checker::new();
        assert_eq!(
            c.infer_thrown_error_name(&Expr::Var("NotFound".into())),
            Some("NotFound".into())
        );
    }

    #[test]
    fn infer_error_from_call() {
        let c = Checker::new();
        let expr = Expr::Call(
            Box::new(Expr::Var("NotFound".into())),
            vec![Expr::StrLit("msg".into())],
        );
        assert_eq!(c.infer_thrown_error_name(&expr), Some("NotFound".into()));
    }

    #[test]
    fn infer_error_from_struct_lit() {
        let c = Checker::new();
        let expr = Expr::StructLit("NotFound".into(), vec![]);
        assert_eq!(c.infer_thrown_error_name(&expr), Some("NotFound".into()));
    }

    #[test]
    fn infer_error_ignores_lowercase() {
        let c = Checker::new();
        assert_eq!(
            c.infer_thrown_error_name(&Expr::Var("notFound".into())),
            None
        );
    }
}
