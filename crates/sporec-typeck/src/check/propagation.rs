use super::*;

impl Checker {
    // ── Set propagation checks ─────────────────────────────────────

    /// Verify that the current function's effect set is a superset of the callee's.
    pub(super) fn check_effect_propagation(&mut self, callee_effects: &EffectSet) {
        self.observe_effects(callee_effects);
        let missing = find_missing_set_items(callee_effects, &self.current_effects);
        if !missing.is_empty() {
            self.err(
                ErrorCode::F0001,
                format!(
                    "missing effects [{}]: caller does not declare them",
                    missing.join(", ")
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checker_with_effects(effects: Vec<&str>) -> Checker {
        let mut c = Checker::new();
        c.current_function = "test_fn".into();
        for e in effects {
            c.current_effects.insert(e.into());
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
}
