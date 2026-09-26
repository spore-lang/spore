//! Effect-set algebra.
//!
//! Provides formal algebraic operations on effect sets:
//! - Union (∪): combining effects of multiple calls
//! - Subset (⊆): checking propagation requirements
//! - Surface expansion: named surfaces expand to atomic effects

use std::collections::{BTreeMap, BTreeSet};

/// A set of effects with algebraic operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffectSet {
    /// The explicit effects in this set.
    effects: BTreeSet<String>,
}

impl EffectSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_names(iter: impl IntoIterator<Item = String>) -> Self {
        Self {
            effects: iter.into_iter().collect(),
        }
    }

    /// Create from a canonical ordered set.
    pub fn from_btreeset(set: BTreeSet<String>) -> Self {
        Self { effects: set }
    }

    /// Convert to the canonical ordered representation.
    pub fn to_btreeset(&self) -> BTreeSet<String> {
        self.effects.clone()
    }

    /// Check if this set is empty (pure function).
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Check if this set contains a specific effect.
    pub fn contains(&self, effect: &str) -> bool {
        self.effects.contains(effect)
    }

    /// Insert an effect.
    pub fn insert(&mut self, effect: String) {
        self.effects.insert(effect);
    }

    /// Union of two effect sets: self ∪ other
    /// The combined effect requirements of calling both.
    pub fn union(&self, other: &EffectSet) -> EffectSet {
        EffectSet {
            effects: self.effects.union(&other.effects).cloned().collect(),
        }
    }

    /// Intersection of two effect sets: self ∩ other
    pub fn intersection(&self, other: &EffectSet) -> EffectSet {
        EffectSet {
            effects: self.effects.intersection(&other.effects).cloned().collect(),
        }
    }

    /// Difference: self \ other (effects in self but not in other)
    pub fn difference(&self, other: &EffectSet) -> EffectSet {
        EffectSet {
            effects: self.effects.difference(&other.effects).cloned().collect(),
        }
    }

    /// Check if `other` is a subset of self (self ⊇ other).
    /// Used for propagation checking: caller must be superset of callee.
    pub fn is_superset_of(&self, other: &EffectSet) -> bool {
        other.effects.is_subset(&self.effects)
    }

    /// Check if self is a subset of `other` (self ⊆ other).
    pub fn is_subset_of(&self, other: &EffectSet) -> bool {
        self.effects.is_subset(&other.effects)
    }

    /// Get effects in `required` that are missing from `self`.
    pub fn missing_from(&self, required: &EffectSet) -> Vec<String> {
        required
            .effects
            .difference(&self.effects)
            .cloned()
            .collect()
    }

    /// Iterate over effects.
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.effects.iter()
    }

    /// Number of effects.
    pub fn len(&self) -> usize {
        self.effects.len()
    }
}

impl std::fmt::Display for EffectSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let effects: Vec<&str> = self.effects.iter().map(|s| s.as_str()).collect();
        write!(f, "[{}]", effects.join(", "))
    }
}

/// Named effect surfaces and their component relationships.
#[derive(Debug, Clone, Default)]
pub struct EffectHierarchy {
    /// Surface name → direct component names.
    children: BTreeMap<String, BTreeSet<String>>,
}

impl EffectHierarchy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a named surface and its direct components.
    pub fn add_surface(&mut self, name: String, components: impl IntoIterator<Item = String>) {
        self.children
            .insert(name, components.into_iter().collect::<BTreeSet<_>>());
    }

    /// Expand named surfaces into a canonical set of atomic effects.
    pub fn expand(&self, set: &EffectSet) -> EffectSet {
        let mut expanded = BTreeSet::new();
        let mut worklist: Vec<String> = set.effects.iter().cloned().collect();
        let mut visited = BTreeSet::new();

        while let Some(effect) = worklist.pop() {
            if !visited.insert(effect.clone()) {
                continue;
            }
            if let Some(children) = self.children.get(&effect) {
                for child in children {
                    worklist.push(child.clone());
                }
            } else {
                expanded.insert(effect);
            }
        }

        EffectSet { effects: expanded }
    }

    /// Return whether a named surface expands recursively to itself.
    pub fn has_cycle(&self, root: &str) -> bool {
        fn visit(
            hierarchy: &EffectHierarchy,
            current: &str,
            active: &mut BTreeSet<String>,
            finished: &mut BTreeSet<String>,
        ) -> bool {
            if active.contains(current) {
                return true;
            }
            if finished.contains(current) {
                return false;
            }

            active.insert(current.to_string());
            if let Some(children) = hierarchy.children.get(current) {
                for child in children {
                    if hierarchy.children.contains_key(child)
                        && visit(hierarchy, child, active, finished)
                    {
                        return true;
                    }
                }
            }
            active.remove(current);
            finished.insert(current.to_string());
            false
        }

        visit(self, root, &mut BTreeSet::new(), &mut BTreeSet::new())
    }

    /// Check if `declared` effects (after expansion) are a superset of `required`.
    pub fn check_propagation(
        &self,
        declared: &EffectSet,
        required: &EffectSet,
    ) -> Result<(), Vec<String>> {
        let expanded = self.expand(declared);
        let missing = expanded.missing_from(required);
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

impl FromIterator<String> for EffectSet {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        Self::from_names(iter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set_is_pure() {
        let set = EffectSet::new();
        assert!(set.is_empty());
        assert_eq!(set.to_string(), "[]");
    }

    #[test]
    fn union_combines_effects() {
        let a = EffectSet::from_names(["NetConnect".into(), "FileRead".into()]);
        let b = EffectSet::from_names(["FileWrite".into(), "NetConnect".into()]);
        let c = a.union(&b);
        assert_eq!(c.len(), 3);
        assert!(c.contains("NetConnect"));
        assert!(c.contains("FileRead"));
        assert!(c.contains("FileWrite"));
    }

    #[test]
    fn subset_checking() {
        let caller =
            EffectSet::from_names(["NetConnect".into(), "FileRead".into(), "FileWrite".into()]);
        let callee = EffectSet::from_names(["NetConnect".into(), "FileRead".into()]);
        assert!(caller.is_superset_of(&callee));
        assert!(!callee.is_superset_of(&caller));
    }

    #[test]
    fn missing_effects() {
        let caller = EffectSet::from_names(["NetConnect".into()]);
        let callee = EffectSet::from_names(["NetConnect".into(), "FileWrite".into()]);
        let missing = caller.missing_from(&callee);
        assert_eq!(missing, vec!["FileWrite"]);
    }

    #[test]
    fn hierarchy_expansion() {
        let mut h = EffectHierarchy::new();
        h.add_surface("FileSystem".into(), ["FileRead".into(), "FileWrite".into()]);

        let declared = EffectSet::from_names(["FileSystem".into()]);
        let expanded = h.expand(&declared);
        assert!(expanded.contains("FileRead"));
        assert!(expanded.contains("FileWrite"));
    }

    #[test]
    fn hierarchy_propagation_check() {
        let mut h = EffectHierarchy::new();
        h.add_surface("FileSystem".into(), ["FileRead".into(), "FileWrite".into()]);

        let declared = EffectSet::from_names(["FileSystem".into()]);
        let required = EffectSet::from_names(["FileRead".into()]);
        assert!(h.check_propagation(&declared, &required).is_ok());
    }

    #[test]
    fn difference_operation() {
        let a = EffectSet::from_names(["A".into(), "B".into(), "C".into()]);
        let b = EffectSet::from_names(["B".into()]);
        let diff = a.difference(&b);
        assert_eq!(diff.len(), 2);
        assert!(diff.contains("A"));
        assert!(diff.contains("C"));
    }

    #[test]
    fn display_format() {
        let set = EffectSet::from_names(["FileRead".into(), "NetConnect".into()]);
        assert_eq!(set.to_string(), "[FileRead, NetConnect]");
    }

    #[test]
    fn nested_surface_expansion_returns_only_atomic_effects() {
        let mut h = EffectHierarchy::new();
        h.add_surface("FileIO".into(), ["FileRead".into(), "FileWrite".into()]);
        h.add_surface("IO".into(), ["FileIO".into(), "Clock".into()]);

        let expanded = h.expand(&EffectSet::from_names(["IO".into()]));
        assert_eq!(
            expanded,
            EffectSet::from_names(["Clock".into(), "FileRead".into(), "FileWrite".into()])
        );
    }

    #[test]
    fn detects_surface_cycle() {
        let mut h = EffectHierarchy::new();
        h.add_surface("A".into(), ["B".into()]);
        h.add_surface("B".into(), ["A".into()]);
        assert!(h.has_cycle("A"));
    }
}
