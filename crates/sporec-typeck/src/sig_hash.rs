//! Function signature hashing for incremental compilation.
//!
//! Uses BLAKE3 for 256-bit content-addressed hashing per SEP-0006/SEP-0008.
//! Two hash types:
//! - **SigHash**: covers function signature (name, params, return, caps, errors, type_params)
//! - **ImplHash**: covers function body (implementation AST)

use crate::types::Ty;

/// A 256-bit BLAKE3 signature hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SigHash(pub [u8; 32]);

/// A 256-bit BLAKE3 implementation hash (function body).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImplHash(pub [u8; 32]);

impl SigHash {
    /// Compute the signature hash for a function.
    pub fn compute(
        name: &str,
        params: &[Ty],
        ret: &Ty,
        effects: &crate::effect_set::EffectSet,
        errors: &crate::types::ErrorSet,
        type_params: &[String],
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"fn:");
        hasher.update(name.as_bytes());
        for tp in type_params {
            hasher.update(b"|tp:");
            hasher.update(tp.as_bytes());
        }
        for p in params {
            hasher.update(b"|p:");
            hasher.update(format!("{p}").as_bytes());
        }
        hasher.update(b"|ret:");
        hasher.update(format!("{ret}").as_bytes());
        for effect in effects.iter() {
            hasher.update(b"|effect:");
            hasher.update(effect.as_bytes());
        }
        for e in errors {
            hasher.update(b"|err:");
            hasher.update(e.as_bytes());
        }
        SigHash(*hasher.finalize().as_bytes())
    }

    /// Compute signature hash for a struct definition.
    pub fn compute_struct(name: &str, fields: &[(String, Ty)]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"struct:");
        hasher.update(name.as_bytes());
        for (fname, fty) in fields {
            hasher.update(b"|f:");
            hasher.update(fname.as_bytes());
            hasher.update(b":");
            hasher.update(format!("{fty}").as_bytes());
        }
        SigHash(*hasher.finalize().as_bytes())
    }

    /// Compute signature hash for a type (enum) definition.
    pub fn compute_type(
        name: &str,
        type_params: &[String],
        variants: &[(String, Vec<Ty>)],
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"type:");
        hasher.update(name.as_bytes());
        for tp in type_params {
            hasher.update(b"|tp:");
            hasher.update(tp.as_bytes());
        }
        for (vname, fields) in variants {
            hasher.update(b"|v:");
            hasher.update(vname.as_bytes());
            for f in fields {
                hasher.update(b":");
                hasher.update(format!("{f}").as_bytes());
            }
        }
        SigHash(*hasher.finalize().as_bytes())
    }

    /// Compute signature hash for a trait definition.
    pub fn compute_trait(
        name: &str,
        type_params: &[String],
        methods: &[(String, Vec<Ty>, Ty)],
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"trait:");
        hasher.update(name.as_bytes());
        for tp in type_params {
            hasher.update(b"|tp:");
            hasher.update(tp.as_bytes());
        }
        for (mname, params, ret) in methods {
            hasher.update(b"|m:");
            hasher.update(mname.as_bytes());
            for p in params {
                hasher.update(b":");
                hasher.update(format!("{p}").as_bytes());
            }
            hasher.update(b"->");
            hasher.update(format!("{ret}").as_bytes());
        }
        SigHash(*hasher.finalize().as_bytes())
    }
}

impl std::fmt::Display for SigHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl ImplHash {
    /// Compute implementation hash from a function body source representation.
    pub fn compute(body: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"impl:");
        hasher.update(body.as_bytes());
        ImplHash(*hasher.finalize().as_bytes())
    }
}

impl std::fmt::Display for ImplHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
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

        for (name, hash) in &self.hashes {
            match previous.hashes.get(name) {
                Some(prev_hash) if prev_hash == hash => {}
                _ => changed.push(name.clone()),
            }
        }

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
    use crate::effect_set::EffectSet;
    use crate::types::{Ty, canonical_error_set};

    #[test]
    fn same_signature_same_hash() {
        let h1 = SigHash::compute(
            "foo",
            &[Ty::I32],
            &Ty::Bool,
            &EffectSet::new(),
            &crate::types::ErrorSet::new(),
            &[],
        );
        let h2 = SigHash::compute(
            "foo",
            &[Ty::I32],
            &Ty::Bool,
            &EffectSet::new(),
            &crate::types::ErrorSet::new(),
            &[],
        );
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_param_different_hash() {
        let h1 = SigHash::compute(
            "foo",
            &[Ty::I32],
            &Ty::Bool,
            &EffectSet::new(),
            &crate::types::ErrorSet::new(),
            &[],
        );
        let h2 = SigHash::compute(
            "foo",
            &[Ty::Str],
            &Ty::Bool,
            &EffectSet::new(),
            &crate::types::ErrorSet::new(),
            &[],
        );
        assert_ne!(h1, h2);
    }

    #[test]
    fn caps_affect_hash() {
        let mut caps = EffectSet::new();
        let h1 = SigHash::compute(
            "foo",
            &[],
            &Ty::Unit,
            &caps,
            &crate::types::ErrorSet::new(),
            &[],
        );
        caps.insert("NetConnect".into());
        let h2 = SigHash::compute(
            "foo",
            &[],
            &Ty::Unit,
            &caps,
            &crate::types::ErrorSet::new(),
            &[],
        );
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_map_diff() {
        let hash_a = SigHash::compute(
            "a",
            &[],
            &Ty::Unit,
            &EffectSet::new(),
            &crate::types::ErrorSet::new(),
            &[],
        );
        let hash_b = SigHash::compute(
            "b",
            &[],
            &Ty::Unit,
            &EffectSet::new(),
            &crate::types::ErrorSet::new(),
            &[],
        );
        let hash_c = SigHash::compute(
            "c",
            &[],
            &Ty::Unit,
            &EffectSet::new(),
            &crate::types::ErrorSet::new(),
            &[],
        );
        let hash_b2 = SigHash::compute(
            "b2",
            &[],
            &Ty::I32,
            &EffectSet::new(),
            &crate::types::ErrorSet::new(),
            &[],
        );

        let mut old = SigHashMap::new();
        old.insert("foo".into(), hash_a);
        old.insert("bar".into(), hash_b);

        let mut new = SigHashMap::new();
        new.insert("foo".into(), hash_a); // unchanged
        new.insert("bar".into(), hash_b2); // changed
        new.insert("baz".into(), hash_c); // added

        let changed = new.diff(&old);
        assert!(changed.contains(&"bar".to_string()));
        assert!(changed.contains(&"baz".to_string()));
        assert!(!changed.contains(&"foo".to_string()));
    }

    #[test]
    fn display_hex_64_chars() {
        let h = SigHash::compute(
            "test",
            &[],
            &Ty::Unit,
            &EffectSet::new(),
            &crate::types::ErrorSet::new(),
            &[],
        );
        let s = h.to_string();
        assert_eq!(s.len(), 64); // 32 bytes × 2 hex chars
    }

    #[test]
    fn canonical_error_sets_hash_the_same() {
        let errors_a = canonical_error_set(["ParseError", "IoError", "ParseError"]);
        let errors_b = canonical_error_set(["IoError", "ParseError"]);
        let h1 = SigHash::compute("foo", &[], &Ty::Unit, &EffectSet::new(), &errors_a, &[]);
        let h2 = SigHash::compute("foo", &[], &Ty::Unit, &EffectSet::new(), &errors_b, &[]);
        assert_eq!(h1, h2);
    }

    #[test]
    fn impl_hash_same_body_same_hash() {
        let h1 = ImplHash::compute("x + 1");
        let h2 = ImplHash::compute("x + 1");
        assert_eq!(h1, h2);
    }

    #[test]
    fn impl_hash_different_body_different_hash() {
        let h1 = ImplHash::compute("x + 1");
        let h2 = ImplHash::compute("x + 2");
        assert_ne!(h1, h2);
    }
}
