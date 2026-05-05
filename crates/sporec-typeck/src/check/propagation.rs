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
