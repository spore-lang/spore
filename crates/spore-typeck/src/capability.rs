//! Capability (effect) algebra.
//!
//! Provides formal algebraic operations on capability sets:
//! - Union (∪): combining capabilities of multiple calls
//! - Subset (⊆): checking propagation requirements
//! - Hierarchy: parent capabilities that imply children

use std::collections::{BTreeMap, BTreeSet};

/// A set of capabilities with algebraic operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilitySet {
    /// The explicit capabilities in this set.
    capabilities: BTreeSet<String>,
}

impl CapabilitySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_names(iter: impl IntoIterator<Item = String>) -> Self {
        Self {
            capabilities: iter.into_iter().collect(),
        }
    }

    /// Create from a BTreeSet (backward compatibility).
    pub fn from_btreeset(set: BTreeSet<String>) -> Self {
        Self { capabilities: set }
    }

    /// Convert to BTreeSet (backward compatibility).
    pub fn to_btreeset(&self) -> BTreeSet<String> {
        self.capabilities.clone()
    }

    /// Check if this set is empty (pure function).
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Check if this set contains a specific capability.
    pub fn contains(&self, cap: &str) -> bool {
        self.capabilities.contains(cap)
    }

    /// Insert a capability.
    pub fn insert(&mut self, cap: String) {
        self.capabilities.insert(cap);
    }

    /// Union of two capability sets: self ∪ other
    /// The combined effect requirements of calling both.
    pub fn union(&self, other: &CapabilitySet) -> CapabilitySet {
        CapabilitySet {
            capabilities: self
                .capabilities
                .union(&other.capabilities)
                .cloned()
                .collect(),
        }
    }

    /// Intersection of two capability sets: self ∩ other
    pub fn intersection(&self, other: &CapabilitySet) -> CapabilitySet {
        CapabilitySet {
            capabilities: self
                .capabilities
                .intersection(&other.capabilities)
                .cloned()
                .collect(),
        }
    }

    /// Difference: self \ other (capabilities in self but not in other)
    pub fn difference(&self, other: &CapabilitySet) -> CapabilitySet {
        CapabilitySet {
            capabilities: self
                .capabilities
                .difference(&other.capabilities)
                .cloned()
                .collect(),
        }
    }

    /// Check if `other` is a subset of self (self ⊇ other).
    /// Used for propagation checking: caller must be superset of callee.
    pub fn is_superset_of(&self, other: &CapabilitySet) -> bool {
        other.capabilities.is_subset(&self.capabilities)
    }

    /// Check if self is a subset of `other` (self ⊆ other).
    pub fn is_subset_of(&self, other: &CapabilitySet) -> bool {
        self.capabilities.is_subset(&other.capabilities)
    }

    /// Get capabilities in `required` that are missing from `self`.
    pub fn missing_from(&self, required: &CapabilitySet) -> Vec<String> {
        required
            .capabilities
            .difference(&self.capabilities)
            .cloned()
            .collect()
    }

    /// Iterate over capabilities.
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.capabilities.iter()
    }

    /// Number of capabilities.
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }
}

impl std::fmt::Display for CapabilitySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let caps: Vec<&str> = self.capabilities.iter().map(|s| s.as_str()).collect();
        write!(f, "[{}]", caps.join(", "))
    }
}

/// Capability hierarchy — defines parent-child relationships.
/// A parent capability implies all its children.
#[derive(Debug, Clone, Default)]
pub struct CapabilityHierarchy {
    /// parent → set of children
    children: BTreeMap<String, BTreeSet<String>>,
}

/// Build the default capability hierarchy with standard aliases:
///   - `FileIO` implies `[FileRead, FileWrite]`
///   - `NetIO`  implies `[NetRead, NetWrite]`
///   - `StateIO` implies `[StateRead, StateWrite]`
///   - `IO` implies all six leaf I/O capabilities
pub fn default_hierarchy() -> CapabilityHierarchy {
    let mut h = CapabilityHierarchy::new();
    h.add_implies("FileIO".into(), "FileRead".into());
    h.add_implies("FileIO".into(), "FileWrite".into());
    h.add_implies("NetIO".into(), "NetRead".into());
    h.add_implies("NetIO".into(), "NetWrite".into());
    h.add_implies("StateIO".into(), "StateRead".into());
    h.add_implies("StateIO".into(), "StateWrite".into());
    // IO is the top-level alias — implies all leaf I/O capabilities
    for leaf in [
        "FileRead",
        "FileWrite",
        "NetRead",
        "NetWrite",
        "StateRead",
        "StateWrite",
    ] {
        h.add_implies("IO".into(), leaf.into());
    }
    h
}

impl CapabilityHierarchy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register that `parent` implies `child`.
    pub fn add_implies(&mut self, parent: String, child: String) {
        self.children.entry(parent).or_default().insert(child);
    }

    /// Expand a capability set by adding all implied children.
    pub fn expand(&self, set: &CapabilitySet) -> CapabilitySet {
        let mut expanded = set.capabilities.clone();
        let mut worklist: Vec<String> = set.capabilities.iter().cloned().collect();

        while let Some(cap) = worklist.pop() {
            if let Some(children) = self.children.get(&cap) {
                for child in children {
                    if expanded.insert(child.clone()) {
                        worklist.push(child.clone());
                    }
                }
            }
        }

        CapabilitySet {
            capabilities: expanded,
        }
    }

    /// Check if `declared` capabilities (after expansion) are a superset of `required`.
    pub fn check_propagation(
        &self,
        declared: &CapabilitySet,
        required: &CapabilitySet,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set_is_pure() {
        let set = CapabilitySet::new();
        assert!(set.is_empty());
        assert_eq!(set.to_string(), "[]");
    }

    #[test]
    fn union_combines_capabilities() {
        let a = CapabilitySet::from_names(["NetRead".into(), "FileRead".into()]);
        let b = CapabilitySet::from_names(["FileWrite".into(), "NetRead".into()]);
        let c = a.union(&b);
        assert_eq!(c.len(), 3);
        assert!(c.contains("NetRead"));
        assert!(c.contains("FileRead"));
        assert!(c.contains("FileWrite"));
    }

    #[test]
    fn subset_checking() {
        let caller =
            CapabilitySet::from_names(["NetRead".into(), "FileRead".into(), "FileWrite".into()]);
        let callee = CapabilitySet::from_names(["NetRead".into(), "FileRead".into()]);
        assert!(caller.is_superset_of(&callee));
        assert!(!callee.is_superset_of(&caller));
    }

    #[test]
    fn missing_capabilities() {
        let caller = CapabilitySet::from_names(["NetRead".into()]);
        let callee = CapabilitySet::from_names(["NetRead".into(), "FileWrite".into()]);
        let missing = caller.missing_from(&callee);
        assert_eq!(missing, vec!["FileWrite"]);
    }

    #[test]
    fn hierarchy_expansion() {
        let mut h = CapabilityHierarchy::new();
        h.add_implies("FileSystem".into(), "FileRead".into());
        h.add_implies("FileSystem".into(), "FileWrite".into());

        let declared = CapabilitySet::from_names(["FileSystem".into()]);
        let expanded = h.expand(&declared);
        assert!(expanded.contains("FileSystem"));
        assert!(expanded.contains("FileRead"));
        assert!(expanded.contains("FileWrite"));
    }

    #[test]
    fn hierarchy_propagation_check() {
        let mut h = CapabilityHierarchy::new();
        h.add_implies("FileSystem".into(), "FileRead".into());
        h.add_implies("FileSystem".into(), "FileWrite".into());

        let declared = CapabilitySet::from_names(["FileSystem".into()]);
        let required = CapabilitySet::from_names(["FileRead".into()]);
        assert!(h.check_propagation(&declared, &required).is_ok());
    }

    #[test]
    fn difference_operation() {
        let a = CapabilitySet::from_names(["A".into(), "B".into(), "C".into()]);
        let b = CapabilitySet::from_names(["B".into()]);
        let diff = a.difference(&b);
        assert_eq!(diff.len(), 2);
        assert!(diff.contains("A"));
        assert!(diff.contains("C"));
    }

    #[test]
    fn display_format() {
        let set = CapabilitySet::from_names(["FileRead".into(), "NetRead".into()]);
        assert_eq!(set.to_string(), "[FileRead, NetRead]");
    }

    // ── default_hierarchy tests ─────────────────────────────────────

    #[test]
    fn default_hierarchy_io_expands_to_six() {
        let h = default_hierarchy();
        let declared = CapabilitySet::from_names(["IO".into()]);
        let expanded = h.expand(&declared);
        assert!(expanded.contains("IO"));
        assert!(expanded.contains("FileRead"));
        assert!(expanded.contains("FileWrite"));
        assert!(expanded.contains("NetRead"));
        assert!(expanded.contains("NetWrite"));
        assert!(expanded.contains("StateRead"));
        assert!(expanded.contains("StateWrite"));
        assert_eq!(expanded.len(), 7); // IO + 6 leaves
    }

    #[test]
    fn default_hierarchy_file_io() {
        let h = default_hierarchy();
        let declared = CapabilitySet::from_names(["FileIO".into()]);
        let expanded = h.expand(&declared);
        assert!(expanded.contains("FileIO"));
        assert!(expanded.contains("FileRead"));
        assert!(expanded.contains("FileWrite"));
        assert_eq!(expanded.len(), 3);
    }

    #[test]
    fn default_hierarchy_net_io() {
        let h = default_hierarchy();
        let declared = CapabilitySet::from_names(["NetIO".into()]);
        let expanded = h.expand(&declared);
        assert!(expanded.contains("NetIO"));
        assert!(expanded.contains("NetRead"));
        assert!(expanded.contains("NetWrite"));
        assert_eq!(expanded.len(), 3);
    }

    #[test]
    fn default_hierarchy_state_io() {
        let h = default_hierarchy();
        let declared = CapabilitySet::from_names(["StateIO".into()]);
        let expanded = h.expand(&declared);
        assert!(expanded.contains("StateIO"));
        assert!(expanded.contains("StateRead"));
        assert!(expanded.contains("StateWrite"));
        assert_eq!(expanded.len(), 3);
    }

    #[test]
    fn default_hierarchy_propagation_io_covers_file_read() {
        let h = default_hierarchy();
        let declared = CapabilitySet::from_names(["IO".into()]);
        let required = CapabilitySet::from_names(["FileRead".into(), "NetWrite".into()]);
        assert!(h.check_propagation(&declared, &required).is_ok());
    }

    #[test]
    fn default_hierarchy_propagation_missing() {
        let h = default_hierarchy();
        let declared = CapabilitySet::from_names(["FileIO".into()]);
        let required = CapabilitySet::from_names(["NetRead".into()]);
        let err = h.check_propagation(&declared, &required).unwrap_err();
        assert_eq!(err, vec!["NetRead"]);
    }
}
