//! Function signature hashing for incremental compilation.
//!
//! Each function signature is hashed to detect changes. When a function's
//! sig-hash changes, its callers must be rechecked.

use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::types::Ty;

/// A 64-bit signature hash (simplified from spec's 256-bit BLAKE3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SigHash(pub u64);

impl SigHash {
    /// Compute the signature hash for a function.
    pub fn compute(
        name: &str,
        params: &[Ty],
        ret: &Ty,
        caps: &BTreeSet<String>,
        errors: &BTreeSet<String>,
        type_params: &[String],
    ) -> Self {
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        for tp in type_params {
            tp.hash(&mut hasher);
        }
        for p in params {
            format!("{p}").hash(&mut hasher);
        }
        format!("{ret}").hash(&mut hasher);
        for c in caps {
            c.hash(&mut hasher);
        }
        for e in errors {
            e.hash(&mut hasher);
        }
        SigHash(hasher.finish())
    }

    /// Compute signature hash for a struct definition.
    pub fn compute_struct(name: &str, fields: &[(String, Ty)]) -> Self {
        let mut hasher = DefaultHasher::new();
        "struct".hash(&mut hasher);
        name.hash(&mut hasher);
        for (fname, fty) in fields {
            fname.hash(&mut hasher);
            format!("{fty}").hash(&mut hasher);
        }
        SigHash(hasher.finish())
    }

    /// Compute signature hash for a type (enum) definition.
    pub fn compute_type(
        name: &str,
        type_params: &[String],
        variants: &[(String, Vec<Ty>)],
    ) -> Self {
        let mut hasher = DefaultHasher::new();
        "type".hash(&mut hasher);
        name.hash(&mut hasher);
        for tp in type_params {
            tp.hash(&mut hasher);
        }
        for (vname, fields) in variants {
            vname.hash(&mut hasher);
            for f in fields {
                format!("{f}").hash(&mut hasher);
            }
        }
        SigHash(hasher.finish())
    }

    /// Compute signature hash for a capability definition.
    pub fn compute_capability(
        name: &str,
        type_params: &[String],
        methods: &[(String, Vec<Ty>, Ty)],
    ) -> Self {
        let mut hasher = DefaultHasher::new();
        "capability".hash(&mut hasher);
        name.hash(&mut hasher);
        for tp in type_params {
            tp.hash(&mut hasher);
        }
        for (mname, params, ret) in methods {
            mname.hash(&mut hasher);
            for p in params {
                format!("{p}").hash(&mut hasher);
            }
            format!("{ret}").hash(&mut hasher);
        }
        SigHash(hasher.finish())
    }
}

impl std::fmt::Display for SigHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// A collection of signature hashes for all items in a module.
#[derive(Debug, Clone, Default)]
pub struct SigHashMap {
    hashes: std::collections::HashMap<String, SigHash>,
}

impl SigHashMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: String, hash: SigHash) {
        self.hashes.insert(name, hash);
    }

    pub fn get(&self, name: &str) -> Option<&SigHash> {
        self.hashes.get(name)
    }

    /// Compare with a previous hash map, returning names of changed items.
    pub fn diff(&self, previous: &SigHashMap) -> Vec<String> {
        let mut changed = Vec::new();

        // Items that changed or were added
        for (name, hash) in &self.hashes {
            match previous.hashes.get(name) {
                Some(prev_hash) if prev_hash == hash => {} // unchanged
                _ => changed.push(name.clone()),
            }
        }

        // Items that were removed
        for name in previous.hashes.keys() {
            if !self.hashes.contains_key(name) {
                changed.push(name.clone());
            }
        }

        changed.sort();
        changed
    }

    pub fn len(&self) -> usize {
        self.hashes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::types::Ty;

    #[test]
    fn same_signature_same_hash() {
        let h1 = SigHash::compute(
            "foo",
            &[Ty::Int],
            &Ty::Bool,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &[],
        );
        let h2 = SigHash::compute(
            "foo",
            &[Ty::Int],
            &Ty::Bool,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &[],
        );
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_param_different_hash() {
        let h1 = SigHash::compute(
            "foo",
            &[Ty::Int],
            &Ty::Bool,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &[],
        );
        let h2 = SigHash::compute(
            "foo",
            &[Ty::Str],
            &Ty::Bool,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &[],
        );
        assert_ne!(h1, h2);
    }

    #[test]
    fn caps_affect_hash() {
        let mut caps = BTreeSet::new();
        let h1 = SigHash::compute("foo", &[], &Ty::Unit, &caps, &BTreeSet::new(), &[]);
        caps.insert("NetRead".into());
        let h2 = SigHash::compute("foo", &[], &Ty::Unit, &caps, &BTreeSet::new(), &[]);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_map_diff() {
        let mut old = SigHashMap::new();
        old.insert("foo".into(), SigHash(100));
        old.insert("bar".into(), SigHash(200));

        let mut new = SigHashMap::new();
        new.insert("foo".into(), SigHash(100)); // unchanged
        new.insert("bar".into(), SigHash(999)); // changed
        new.insert("baz".into(), SigHash(300)); // added

        let changed = new.diff(&old);
        assert!(changed.contains(&"bar".to_string()));
        assert!(changed.contains(&"baz".to_string()));
        assert!(!changed.contains(&"foo".to_string()));
    }

    #[test]
    fn display_hex() {
        let h = SigHash(0xDEADBEEF);
        assert_eq!(h.to_string(), "00000000deadbeef");
    }
}
