//! Hole report — structured output describing unfilled holes in a module.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::types::Ty;

/// Information collected about a single hole.
#[derive(Debug, Clone)]
pub struct HoleInfo {
    /// Hole name (from `?name`)
    pub name: String,
    /// Inferred/expected type
    pub expected_type: Ty,
    /// The function this hole appears in
    pub function: String,
    /// Local bindings available at the hole site (name → type)
    pub bindings: BTreeMap<String, Ty>,
    /// Available functions that return the expected type
    pub suggestions: Vec<String>,
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
            out.push_str(&format!(
                "      \"expected_type\": {},\n",
                json_escape(&h.expected_type.to_string())
            ));
            out.push_str(&format!(
                "      \"function\": {},\n",
                json_escape(&h.function)
            ));
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
            out.push_str("      \"suggestions\": [");
            for (j, s) in h.suggestions.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_escape(s));
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

/// A dependency graph between typed holes.
///
/// If hole A's type depends on hole B (e.g., A's expected type contains
/// a type variable that B's type also contains), then filling B first
/// may help resolve A's type.
#[derive(Debug, Clone, Default)]
pub struct HoleDependencyGraph {
    /// hole_name → set of holes it depends on
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
    pub fn add_dependency(&mut self, hole: String, dependency: String) {
        self.add_hole(hole.clone());
        self.add_hole(dependency.clone());
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
