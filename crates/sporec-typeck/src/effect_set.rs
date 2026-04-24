//! Effect-set algebra.
//!
//! Provides formal algebraic operations on effect sets:
//! - Union (∪): combining effects of multiple calls
//! - Subset (⊆): checking propagation requirements
//! - Hierarchy: parent effects that imply children

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

    /// Create from a BTreeSet (backward compatibility).
    pub fn from_btreeset(set: BTreeSet<String>) -> Self {
        Self { effects: set }
    }

    /// Convert to BTreeSet (backward compatibility).
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

/// Effect hierarchy — defines parent-child relationships.
/// A parent effect implies all its children.
#[derive(Debug, Clone, Default)]
pub struct EffectHierarchy {
    /// parent → set of children
    children: BTreeMap<String, BTreeSet<String>>,
}

/// Build the default effect hierarchy with standard aliases:
///   - `FileIO` implies `[FileRead, FileWrite]`
///   - `NetIO`  implies `[NetConnect, NetListen]`
///   - `IO` implies all four leaf I/O effects
pub fn default_effect_hierarchy() -> EffectHierarchy {
    let mut h = EffectHierarchy::new();
    h.add_implies("FileIO".into(), "FileRead".into());
    h.add_implies("FileIO".into(), "FileWrite".into());
    h.add_implies("NetIO".into(), "NetConnect".into());
    h.add_implies("NetIO".into(), "NetListen".into());
    // IO is the top-level alias — implies all leaf I/O effects
    for leaf in ["FileRead", "FileWrite", "NetConnect", "NetListen"] {
        h.add_implies("IO".into(), leaf.into());
    }
    h
}

impl EffectHierarchy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register that `parent` implies `child`.
    pub fn add_implies(&mut self, parent: String, child: String) {
        self.children.entry(parent).or_default().insert(child);
    }

    /// Expand an effect set by adding all implied children.
    pub fn expand(&self, set: &EffectSet) -> EffectSet {
        let mut expanded = set.effects.clone();
        let mut worklist: Vec<String> = set.effects.iter().cloned().collect();

        while let Some(effect) = worklist.pop() {
            if let Some(children) = self.children.get(&effect) {
                for child in children {
                    if expanded.insert(child.clone()) {
                        worklist.push(child.clone());
                    }
                }
            }
        }

        EffectSet { effects: expanded }
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
        h.add_implies("FileSystem".into(), "FileRead".into());
        h.add_implies("FileSystem".into(), "FileWrite".into());

        let declared = EffectSet::from_names(["FileSystem".into()]);
        let expanded = h.expand(&declared);
        assert!(expanded.contains("FileSystem"));
        assert!(expanded.contains("FileRead"));
        assert!(expanded.contains("FileWrite"));
    }

    #[test]
    fn hierarchy_propagation_check() {
        let mut h = EffectHierarchy::new();
        h.add_implies("FileSystem".into(), "FileRead".into());
        h.add_implies("FileSystem".into(), "FileWrite".into());

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

    // ── default_effect_hierarchy tests ─────────────────────────────────────

    #[test]
    fn default_effect_hierarchy_io_expands_to_four() {
        let h = default_effect_hierarchy();
        let declared = EffectSet::from_names(["IO".into()]);
        let expanded = h.expand(&declared);
        assert!(expanded.contains("IO"));
        assert!(expanded.contains("FileRead"));
        assert!(expanded.contains("FileWrite"));
        assert!(expanded.contains("NetConnect"));
        assert!(expanded.contains("NetListen"));
        assert_eq!(expanded.len(), 5); // IO + 4 leaves
    }

    #[test]
    fn default_effect_hierarchy_file_io() {
        let h = default_effect_hierarchy();
        let declared = EffectSet::from_names(["FileIO".into()]);
        let expanded = h.expand(&declared);
        assert!(expanded.contains("FileIO"));
        assert!(expanded.contains("FileRead"));
        assert!(expanded.contains("FileWrite"));
        assert_eq!(expanded.len(), 3);
    }

    #[test]
    fn default_effect_hierarchy_net_io() {
        let h = default_effect_hierarchy();
        let declared = EffectSet::from_names(["NetIO".into()]);
        let expanded = h.expand(&declared);
        assert!(expanded.contains("NetIO"));
        assert!(expanded.contains("NetConnect"));
        assert!(expanded.contains("NetListen"));
        assert_eq!(expanded.len(), 3);
    }

    #[test]
    fn default_effect_hierarchy_propagation_io_covers_file_read() {
        let h = default_effect_hierarchy();
        let declared = EffectSet::from_names(["IO".into()]);
        let required = EffectSet::from_names(["FileRead".into(), "NetListen".into()]);
        assert!(h.check_propagation(&declared, &required).is_ok());
    }

    #[test]
    fn default_effect_hierarchy_propagation_missing() {
        let h = default_effect_hierarchy();
        let declared = EffectSet::from_names(["FileIO".into()]);
        let required = EffectSet::from_names(["NetConnect".into()]);
        let err = h.check_propagation(&declared, &required).unwrap_err();
        assert_eq!(err, vec!["NetConnect"]);
    }
}
