//! Hole report — structured output describing unfilled holes in a module.
//!
//! Implements the v0.3 hole info model from SEP-0005 §4.2.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use crate::types::Ty;

// ── Source location ─────────────────────────────────────────────────

/// Location in source (SEP-0005 v0.3).
#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

// ── Candidate scoring (Extension A) ─────────────────────────────────

/// Candidate scoring vector (SEP-0005 v0.3 Extension A).
#[derive(Debug, Clone)]
pub struct CandidateScore {
    pub name: String,
    /// Type match quality [0,1]
    pub type_match: f64,
    /// Cost fit quality [0,1]
    pub cost_fit: f64,
    /// Capability fit {0,1}
    pub capability_fit: f64,
    /// Error coverage [0,1]
    pub error_coverage: f64,
}

impl CandidateScore {
    pub fn overall(&self) -> f64 {
        0.40 * self.type_match
            + 0.20 * self.cost_fit
            + 0.25 * self.capability_fit
            + 0.15 * self.error_coverage
    }
}

// ── Confidence assessment (Extension C) ─────────────────────────────

/// Confidence assessment (SEP-0005 v0.3 Extension C).
#[derive(Debug, Clone)]
pub struct Confidence {
    pub type_inference: TypeInferenceConfidence,
    pub candidate_ranking: CandidateRanking,
    pub ambiguous_count: usize,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeInferenceConfidence {
    Certain,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CandidateRanking {
    UniqueBest,
    Ambiguous,
    NoCandidates,
}

// ── Error cluster (Extension D) ─────────────────────────────────────

/// Error cluster (SEP-0005 v0.3 Extension D).
#[derive(Debug, Clone)]
pub struct ErrorCluster {
    pub source: String,
    pub errors: Vec<String>,
    pub handling_suggestion: String,
}

// ── Cost budget ─────────────────────────────────────────────────────

/// Cost budget info for hole context.
#[derive(Debug, Clone)]
pub struct CostBudget {
    pub budget_total: Option<f64>,
    pub cost_before_hole: f64,
    pub budget_remaining: Option<f64>,
}

// ── HoleInfo v0.3 ───────────────────────────────────────────────────

/// Information collected about a single hole (SEP-0005 v0.3).
#[derive(Debug, Clone)]
pub struct HoleInfo {
    /// Hole name (from `?name`)
    pub name: String,
    /// Location in source
    pub location: Option<SourceLocation>,
    /// Inferred/expected type
    pub expected_type: Ty,
    /// What the type was inferred from (e.g. "return type of `foo`")
    pub type_inferred_from: Option<String>,
    /// The function this hole appears in
    pub function: String,
    /// Full enclosing function signature
    pub enclosing_signature: Option<String>,
    /// Local bindings available at the hole site (name → type)
    pub bindings: BTreeMap<String, Ty>,
    /// Dependency edges between bindings (Extension B)
    pub binding_dependencies: BTreeMap<String, Vec<String>>,
    /// Capabilities in scope
    pub capabilities: BTreeSet<String>,
    /// Error types that must be handled
    pub errors_to_handle: Vec<String>,
    /// Cost budget information
    pub cost_budget: Option<CostBudget>,
    /// Scored candidate list (replaces old `suggestions`)
    pub candidates: Vec<CandidateScore>,
    /// Other holes that depend on this hole
    pub dependent_holes: Vec<String>,
    /// Confidence assessment (Extension C)
    pub confidence: Option<Confidence>,
    /// Error clusters (Extension D)
    pub error_clusters: Vec<ErrorCluster>,
}

/// Collected report for all holes in a module.
#[derive(Debug, Clone, Default)]
pub struct HoleReport {
    pub holes: Vec<HoleInfo>,
    pub dependency_graph: HoleDependencyGraph,
}

impl HoleReport {
    pub fn new() -> Self {
        Self {
            holes: Vec::new(),
            dependency_graph: HoleDependencyGraph::new(),
        }
    }

    /// Serialize to JSON string (no serde dependency).
    pub fn to_json(&self) -> String {
        let mut out = String::from("{\n  \"holes\": [\n");
        for (i, h) in self.holes.iter().enumerate() {
            if i > 0 {
                out.push_str(",\n");
            }
            out.push_str("    {\n");
            out.push_str(&format!("      \"name\": {},\n", json_escape(&h.name)));

            // location (nullable)
            if let Some(ref loc) = h.location {
                out.push_str(&format!(
                    "      \"location\": {{\"file\": {}, \"line\": {}, \"column\": {}}},\n",
                    json_escape(&loc.file),
                    loc.line,
                    loc.column
                ));
            } else {
                out.push_str("      \"location\": null,\n");
            }

            out.push_str(&format!(
                "      \"expected_type\": {},\n",
                json_escape(&h.expected_type.to_string())
            ));

            // type_inferred_from (nullable)
            match h.type_inferred_from {
                Some(ref s) => out.push_str(&format!(
                    "      \"type_inferred_from\": {},\n",
                    json_escape(s)
                )),
                None => out.push_str("      \"type_inferred_from\": null,\n"),
            }

            out.push_str(&format!(
                "      \"function\": {},\n",
                json_escape(&h.function)
            ));

            // enclosing_signature (nullable)
            match h.enclosing_signature {
                Some(ref s) => out.push_str(&format!(
                    "      \"enclosing_signature\": {},\n",
                    json_escape(s)
                )),
                None => out.push_str("      \"enclosing_signature\": null,\n"),
            }

            // bindings
            out.push_str("      \"bindings\": {");
            for (j, (k, v)) in h.bindings.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!(
                    "{}: {}",
                    json_escape(k),
                    json_escape(&v.to_string())
                ));
            }
            out.push_str("},\n");

            // binding_dependencies (Extension B)
            out.push_str("      \"binding_dependencies\": {");
            for (j, (k, deps)) in h.binding_dependencies.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                let items: Vec<String> = deps.iter().map(|d| json_escape(d)).collect();
                out.push_str(&format!("{}: [{}]", json_escape(k), items.join(", ")));
            }
            out.push_str("},\n");

            // capabilities
            out.push_str("      \"capabilities\": [");
            for (j, c) in h.capabilities.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_escape(c));
            }
            out.push_str("],\n");

            // errors_to_handle
            out.push_str("      \"errors_to_handle\": [");
            for (j, e) in h.errors_to_handle.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_escape(e));
            }
            out.push_str("],\n");

            // cost_budget (nullable)
            if let Some(ref cb) = h.cost_budget {
                out.push_str("      \"cost_budget\": {");
                match cb.budget_total {
                    Some(v) => out.push_str(&format!("\"budget_total\": {v}")),
                    None => out.push_str("\"budget_total\": null"),
                }
                out.push_str(&format!(", \"cost_before_hole\": {}", cb.cost_before_hole));
                match cb.budget_remaining {
                    Some(v) => out.push_str(&format!(", \"budget_remaining\": {v}")),
                    None => out.push_str(", \"budget_remaining\": null"),
                }
                out.push_str("},\n");
            } else {
                out.push_str("      \"cost_budget\": null,\n");
            }

            // candidates (v0.3 scored candidates)
            out.push_str("      \"candidates\": [");
            for (j, cs) in h.candidates.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!(
                    "{{\"name\": {}, \"type_match\": {:.2}, \"cost_fit\": {:.2}, \"capability_fit\": {:.2}, \"error_coverage\": {:.2}, \"overall\": {:.2}}}",
                    json_escape(&cs.name),
                    cs.type_match,
                    cs.cost_fit,
                    cs.capability_fit,
                    cs.error_coverage,
                    cs.overall(),
                ));
            }
            out.push_str("],\n");

            // dependent_holes
            out.push_str("      \"dependent_holes\": [");
            for (j, dh) in h.dependent_holes.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_escape(dh));
            }
            out.push_str("],\n");

            // confidence (nullable, Extension C)
            if let Some(ref conf) = h.confidence {
                let ti = match conf.type_inference {
                    TypeInferenceConfidence::Certain => "certain",
                    TypeInferenceConfidence::Partial => "partial",
                    TypeInferenceConfidence::Unknown => "unknown",
                };
                let cr = match conf.candidate_ranking {
                    CandidateRanking::UniqueBest => "unique_best",
                    CandidateRanking::Ambiguous => "ambiguous",
                    CandidateRanking::NoCandidates => "no_candidates",
                };
                out.push_str(&format!(
                    "      \"confidence\": {{\"type_inference\": {}, \"candidate_ranking\": {}, \"ambiguous_count\": {}",
                    json_escape(ti), json_escape(cr), conf.ambiguous_count,
                ));
                if let Some(ref rec) = conf.recommendation {
                    out.push_str(&format!(", \"recommendation\": {}", json_escape(rec)));
                } else {
                    out.push_str(", \"recommendation\": null");
                }
                out.push_str("},\n");
            } else {
                out.push_str("      \"confidence\": null,\n");
            }

            // error_clusters (Extension D)
            out.push_str("      \"error_clusters\": [");
            for (j, ec) in h.error_clusters.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                let errs: Vec<String> = ec.errors.iter().map(|e| json_escape(e)).collect();
                out.push_str(&format!(
                    "{{\"source\": {}, \"errors\": [{}], \"handling_suggestion\": {}}}",
                    json_escape(&ec.source),
                    errs.join(", "),
                    json_escape(&ec.handling_suggestion),
                ));
            }
            out.push_str("]\n");

            out.push_str("    }");
        }
        out.push_str("\n  ],\n");
        out.push_str("  \"dependency_graph\": ");
        out.push_str(&self.dependency_graph.to_json_string());
        out.push('\n');
        out.push('}');
        out
    }
}

// ── Typed dependency edges (SEP-0005 §5) ────────────────────────────

/// Kind of dependency between two holes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// h2's expected type depends on h1's output
    Type,
    /// h2's bindings trace back to h1's output
    Value,
    /// h2's cost budget depends on h1's actual cost
    Cost,
}

/// A typed edge between two holes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

/// A dependency graph between typed holes.
///
/// If hole A's type depends on hole B (e.g., A's expected type contains
/// a type variable that B's type also contains), then filling B first
/// may help resolve A's type.
#[derive(Debug, Clone, Default)]
pub struct HoleDependencyGraph {
    /// Typed edges between holes
    pub edges: Vec<DependencyEdge>,
    /// hole_name → set of holes it depends on (fast lookup)
    pub dependencies: HashMap<String, HashSet<String>>,
    /// hole_name → set of holes that depend on it
    pub dependents: HashMap<String, HashSet<String>>,
    /// All hole names in the graph
    pub nodes: HashSet<String>,
}

impl HoleDependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a hole to the graph.
    pub fn add_hole(&mut self, name: String) {
        self.nodes.insert(name.clone());
        self.dependencies.entry(name.clone()).or_default();
        self.dependents.entry(name).or_default();
    }

    /// Record that `hole` depends on `dependency` (filling dependency may help resolve hole).
    /// Defaults to `EdgeKind::Type`.
    pub fn add_dependency(&mut self, hole: String, dependency: String) {
        self.add_dependency_typed(hole, dependency, EdgeKind::Type);
    }

    /// Record a typed dependency: `hole` depends on `dependency` via `kind`.
    pub fn add_dependency_typed(&mut self, hole: String, dependency: String, kind: EdgeKind) {
        self.add_hole(hole.clone());
        self.add_hole(dependency.clone());
        self.edges.push(DependencyEdge {
            from: dependency.clone(),
            to: hole.clone(),
            kind,
        });
        self.dependencies
            .entry(hole.clone())
            .or_default()
            .insert(dependency.clone());
        self.dependents.entry(dependency).or_default().insert(hole);
    }

    /// Get holes that have no dependencies (can be filled first).
    pub fn roots(&self) -> Vec<String> {
        let mut roots: Vec<String> = self
            .nodes
            .iter()
            .filter(|n| self.dependencies.get(*n).is_none_or(|d| d.is_empty()))
            .cloned()
            .collect();
        roots.sort();
        roots
    }

    /// Get a topological ordering of holes (suggested fill order).
    pub fn topological_order(&self) -> Vec<String> {
        let mut in_degree: HashMap<&String, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.insert(node, self.dependencies.get(node).map_or(0, |d| d.len()));
        }

        let mut sorted_zero: Vec<&String> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&node, _)| node)
            .collect();
        sorted_zero.sort();
        let mut queue: VecDeque<&String> = sorted_zero.into_iter().collect();

        let mut order = Vec::new();
        while let Some(node) = queue.pop_front() {
            order.push(node.clone());
            if let Some(deps) = self.dependents.get(node) {
                let mut sorted_deps: Vec<&String> = deps.iter().collect();
                sorted_deps.sort();
                for dep in sorted_deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }

        // Add any remaining nodes (cycles) at the end
        let mut remaining: Vec<String> = self
            .nodes
            .iter()
            .filter(|n| !order.contains(n))
            .cloned()
            .collect();
        remaining.sort();
        order.extend(remaining);

        order
    }

    /// Find connected components (independent clusters of holes).
    pub fn clusters(&self) -> Vec<Vec<String>> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut clusters = Vec::new();

        let mut sorted_nodes: Vec<&String> = self.nodes.iter().collect();
        sorted_nodes.sort();

        for node in sorted_nodes {
            if visited.contains(node) {
                continue;
            }
            let mut cluster = Vec::new();
            let mut stack = vec![node.clone()];
            while let Some(n) = stack.pop() {
                if visited.insert(n.clone()) {
                    cluster.push(n.clone());
                    if let Some(deps) = self.dependencies.get(&n) {
                        for d in deps {
                            if !visited.contains(d) {
                                stack.push(d.clone());
                            }
                        }
                    }
                    if let Some(deps) = self.dependents.get(&n) {
                        for d in deps {
                            if !visited.contains(d) {
                                stack.push(d.clone());
                            }
                        }
                    }
                }
            }
            cluster.sort();
            clusters.push(cluster);
        }

        clusters.sort_by(|a, b| a.first().cmp(&b.first()));
        clusters
    }

    /// Get the dependencies of a specific hole.
    pub fn dependencies_of(&self, hole: &str) -> Vec<String> {
        let mut deps: Vec<String> = self
            .dependencies
            .get(hole)
            .map(|d| d.iter().cloned().collect())
            .unwrap_or_default();
        deps.sort();
        deps
    }

    /// Get holes that depend on a specific hole.
    pub fn dependents_of(&self, hole: &str) -> Vec<String> {
        let mut deps: Vec<String> = self
            .dependents
            .get(hole)
            .map(|d| d.iter().cloned().collect())
            .unwrap_or_default();
        deps.sort();
        deps
    }

    /// Number of holes in the graph.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Check if the graph has cycles (SEP-0005 §5).
    pub fn has_cycle(&self) -> bool {
        let mut in_degree: HashMap<&String, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.insert(node, self.dependencies.get(node).map_or(0, |d| d.len()));
        }
        let mut queue: VecDeque<&String> = in_degree
            .iter()
            .filter(|&(_, &d)| d == 0)
            .map(|(&n, _)| n)
            .collect();
        let mut count = 0;
        while let Some(node) = queue.pop_front() {
            count += 1;
            if let Some(deps) = self.dependents.get(node) {
                for dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }
        count < self.nodes.len()
    }

    /// Compute parallel-ready fill layers per SEP-0005 §5.6.
    ///
    /// Returns `Ok(layers)` where `layers[0]` can be filled first (in parallel),
    /// `layers[1]` after `layers[0]` is done, etc.
    /// Returns `Err` with cycle node names if the graph has cycles.
    pub fn layered_topological_order(&self) -> Result<Vec<Vec<String>>, Vec<String>> {
        if self.has_cycle() {
            // Find cycle nodes: those not reachable by Kahn's algorithm
            let mut in_degree: HashMap<&String, usize> = HashMap::new();
            for node in &self.nodes {
                in_degree.insert(node, self.dependencies.get(node).map_or(0, |d| d.len()));
            }
            let mut queue: VecDeque<&String> = in_degree
                .iter()
                .filter(|&(_, &d)| d == 0)
                .map(|(&n, _)| n)
                .collect();
            let mut processed: HashSet<&String> = HashSet::new();
            while let Some(node) = queue.pop_front() {
                processed.insert(node);
                if let Some(deps) = self.dependents.get(node) {
                    for dep in deps {
                        if let Some(deg) = in_degree.get_mut(dep) {
                            *deg -= 1;
                            if *deg == 0 {
                                queue.push_back(dep);
                            }
                        }
                    }
                }
            }
            let mut cycle_nodes: Vec<String> = self
                .nodes
                .iter()
                .filter(|n| !processed.contains(n))
                .cloned()
                .collect();
            cycle_nodes.sort();
            return Err(cycle_nodes);
        }

        // Build layers using Kahn's algorithm variant
        let mut in_degree: HashMap<&String, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.insert(node, self.dependencies.get(node).map_or(0, |d| d.len()));
        }

        let mut remaining: HashSet<&String> = self.nodes.iter().collect();
        let mut layers = Vec::new();

        while !remaining.is_empty() {
            let mut ready: Vec<String> = remaining
                .iter()
                .filter(|&&n| in_degree.get(n).copied().unwrap_or(0) == 0)
                .map(|&n| n.clone())
                .collect();

            if ready.is_empty() {
                break; // shouldn't happen since we already checked for cycles
            }

            ready.sort();

            for name in &ready {
                remaining.remove(name);
                if let Some(deps) = self.dependents.get(name) {
                    for dep in deps {
                        if let Some(deg) = in_degree.get_mut(dep) {
                            *deg = deg.saturating_sub(1);
                        }
                    }
                }
            }

            layers.push(ready);
        }

        Ok(layers)
    }

    /// Serialize to a JSON string (no serde dependency).
    pub fn to_json_string(&self) -> String {
        let mut out = String::from("{\n");

        // Dependencies as adjacency list
        out.push_str("    \"dependencies\": {");
        let mut sorted_deps: Vec<(&String, &HashSet<String>)> = self.dependencies.iter().collect();
        sorted_deps.sort_by_key(|(k, _)| *k);
        for (i, (hole, deps)) in sorted_deps.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let mut sorted: Vec<&String> = deps.iter().collect();
            sorted.sort();
            let items: Vec<String> = sorted.iter().map(|d| json_escape(d)).collect();
            out.push_str(&format!("{}: [{}]", json_escape(hole), items.join(", ")));
        }
        out.push_str("},\n");

        // Typed edges
        out.push_str("    \"edges\": [");
        let mut sorted_edges: Vec<&DependencyEdge> = self.edges.iter().collect();
        sorted_edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
        for (i, edge) in sorted_edges.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let kind_str = match edge.kind {
                EdgeKind::Type => "type",
                EdgeKind::Value => "value",
                EdgeKind::Cost => "cost",
            };
            out.push_str(&format!(
                "{{\"from\": {}, \"to\": {}, \"kind\": {}}}",
                json_escape(&edge.from),
                json_escape(&edge.to),
                json_escape(kind_str),
            ));
        }
        out.push_str("],\n");

        // Roots
        out.push_str("    \"roots\": [");
        let roots = self.roots();
        for (i, r) in roots.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&json_escape(r));
        }
        out.push_str("],\n");

        // Suggested order
        out.push_str("    \"suggested_order\": [");
        let order = self.topological_order();
        for (i, n) in order.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&json_escape(n));
        }
        out.push_str("]\n");

        out.push_str("  }");
        out
    }
}

/// Escape a string for JSON output.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        let g = HoleDependencyGraph::new();
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
        assert_eq!(g.roots(), Vec::<String>::new());
        assert_eq!(g.topological_order(), Vec::<String>::new());
        assert!(g.clusters().is_empty());
    }

    #[test]
    fn single_hole_is_root() {
        let mut g = HoleDependencyGraph::new();
        g.add_hole("?impl".into());
        assert_eq!(g.len(), 1);
        assert!(!g.is_empty());
        assert_eq!(g.roots(), vec!["?impl"]);
        assert_eq!(g.topological_order(), vec!["?impl"]);
        assert_eq!(g.clusters(), vec![vec!["?impl".to_string()]]);
    }

    #[test]
    fn dependency_ordering() {
        let mut g = HoleDependencyGraph::new();
        g.add_dependency("?b".into(), "?a".into());
        assert_eq!(g.roots(), vec!["?a"]);
        let order = g.topological_order();
        let a_pos = order.iter().position(|x| x == "?a").unwrap();
        let b_pos = order.iter().position(|x| x == "?b").unwrap();
        assert!(a_pos < b_pos);
    }

    #[test]
    fn dependency_queries() {
        let mut g = HoleDependencyGraph::new();
        g.add_dependency("?b".into(), "?a".into());
        g.add_dependency("?c".into(), "?a".into());
        assert_eq!(g.dependencies_of("?b"), vec!["?a"]);
        assert_eq!(g.dependencies_of("?a"), Vec::<String>::new());
        assert_eq!(g.dependents_of("?a"), vec!["?b", "?c"]);
        assert_eq!(g.dependents_of("?b"), Vec::<String>::new());
    }

    #[test]
    fn independent_clusters() {
        let mut g = HoleDependencyGraph::new();
        g.add_dependency("?b".into(), "?a".into());
        g.add_hole("?c".into());
        let clusters = g.clusters();
        assert_eq!(clusters.len(), 2);
        assert!(clusters[0].contains(&"?a".to_string()));
        assert!(clusters[0].contains(&"?b".to_string()));
        assert_eq!(clusters[1], vec!["?c".to_string()]);
    }

    #[test]
    fn chain_dependency_order() {
        let mut g = HoleDependencyGraph::new();
        g.add_dependency("?c".into(), "?b".into());
        g.add_dependency("?b".into(), "?a".into());
        assert_eq!(g.roots(), vec!["?a"]);
        let order = g.topological_order();
        assert_eq!(order, vec!["?a", "?b", "?c"]);
        assert_eq!(g.clusters().len(), 1);
    }

    #[test]
    fn cycle_handling() {
        let mut g = HoleDependencyGraph::new();
        g.add_dependency("?a".into(), "?b".into());
        g.add_dependency("?b".into(), "?a".into());
        let order = g.topological_order();
        assert_eq!(order.len(), 2);
        assert!(order.contains(&"?a".to_string()));
        assert!(order.contains(&"?b".to_string()));
    }

    #[test]
    fn diamond_dependency() {
        let mut g = HoleDependencyGraph::new();
        g.add_dependency("?b".into(), "?a".into());
        g.add_dependency("?c".into(), "?a".into());
        g.add_dependency("?d".into(), "?b".into());
        g.add_dependency("?d".into(), "?c".into());
        assert_eq!(g.roots(), vec!["?a"]);
        let order = g.topological_order();
        let a_pos = order.iter().position(|x| x == "?a").unwrap();
        let b_pos = order.iter().position(|x| x == "?b").unwrap();
        let c_pos = order.iter().position(|x| x == "?c").unwrap();
        let d_pos = order.iter().position(|x| x == "?d").unwrap();
        assert!(a_pos < b_pos);
        assert!(a_pos < c_pos);
        assert!(b_pos < d_pos);
        assert!(c_pos < d_pos);
    }

    #[test]
    fn json_serialization() {
        let mut g = HoleDependencyGraph::new();
        g.add_dependency("?b".into(), "?a".into());
        let json = g.to_json_string();
        assert!(json.contains("\"dependencies\""));
        assert!(json.contains("\"roots\""));
        assert!(json.contains("\"suggested_order\""));
        assert!(json.contains("\"?a\""));
        assert!(json.contains("\"?b\""));
    }

    #[test]
    fn hole_report_includes_graph() {
        let report = HoleReport::new();
        assert!(report.dependency_graph.is_empty());
        let json = report.to_json();
        assert!(json.contains("\"dependency_graph\""));
    }
}
