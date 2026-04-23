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
            (Ty::Fn(p1, r1, _, _), Ty::Fn(p2, r2, _, _)) if p1.len() == p2.len() => {
                let pairs: Vec<(Ty, Ty)> = p1.iter().cloned().zip(p2.iter().cloned()).collect();
                let ret_pair = ((**r1).clone(), (**r2).clone());
                for (x, y) in &pairs {
                    self.unify(x, y, context);
                }
                self.unify(&ret_pair.0, &ret_pair.1, context);
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
