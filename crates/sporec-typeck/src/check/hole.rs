use super::*;

impl Checker {
    /// Check a refinement predicate against a constant expression.
    /// If the init expression is a constant, evaluate the predicate.
    /// If not constant, skip (runtime check needed).
    pub(super) fn check_refinement_on_expr(
        &mut self,
        init: &Expr,
        var_name: &str,
        pred: &Expr,
        binding_name: &str,
    ) {
        use crate::refinement::{eval_refinement_predicate, expr_to_const};
        if let Some(cv) = expr_to_const(init) {
            match eval_refinement_predicate(pred, var_name, &cv) {
                Ok(true) => {} // predicate satisfied
                Ok(false) => {
                    self.err(
                        ErrorCode::R0001,
                        format!(
                            "refinement predicate violated for `{binding_name}`: \
                             value does not satisfy the type constraint"
                        ),
                    );
                }
                Err(_reason) => {
                    // Predicate not decidable at compile time — skip
                }
            }
        }
    }

    /// Find registered functions whose return type matches the expected type.
    pub(super) fn find_suggestions(
        &self,
        expected: &Ty,
        allow_list: Option<&[String]>,
    ) -> Vec<String> {
        if expected.is_error() || matches!(expected, Ty::Hole(_)) {
            return Vec::new();
        }
        let mut suggestions: Vec<String> = self
            .registry
            .functions
            .iter()
            .filter(|(name, (_, ret_ty, _))| {
                ret_ty == expected
                    && *name != &self.current_function
                    && allow_list.is_none_or(|allowed| allowed.iter().any(|a| a == *name))
            })
            .map(|(name, _)| name.clone())
            .collect();
        suggestions.sort();
        suggestions
    }

    pub(super) fn fresh_unnamed_hole_name(&mut self) -> String {
        let id = self.next_unnamed_hole_id;
        self.next_unnamed_hole_id += 1;
        format!("_hole{id}")
    }

    /// Build a dependency graph between holes based on shared type variables.
    pub(super) fn build_hole_dependency_graph(&self) -> HoleDependencyGraph {
        let mut graph = HoleDependencyGraph::new();

        for hole in &self.hole_report.holes {
            graph.add_hole(hole.name.clone());
        }

        let hole_vars: Vec<(&str, HashSet<u32>)> = self
            .hole_report
            .holes
            .iter()
            .map(|h| {
                let vars = self.collect_type_vars(&h.expected_type);
                (h.name.as_str(), vars)
            })
            .collect();

        // Two holes that share a type variable are dependent
        for (i, (name1, vars1)) in hole_vars.iter().enumerate() {
            for (name2, vars2) in hole_vars.iter().skip(i + 1) {
                if vars1.iter().any(|v| vars2.contains(v)) {
                    graph.add_dependency(name2.to_string(), name1.to_string());
                }
            }
        }

        graph
    }

    /// Collect all type variable IDs in a type (following substitutions).
    pub(super) fn collect_type_vars(&self, ty: &Ty) -> HashSet<u32> {
        let mut vars = HashSet::new();
        ty.visit(&mut |t| {
            if let Ty::Var(id) = t {
                vars.insert(*id);
                // Follow substitution chains that visit() cannot see
                if let Some(resolved) = self.substitution.get(id) {
                    vars.extend(self.collect_type_vars(resolved));
                }
            }
        });
        vars
    }
}
