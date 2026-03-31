//! Incremental compilation infrastructure.
//!
//! Provides a query-based compilation model where results are memoized
//! and invalidated when inputs change. This is the foundation for
//! future salsa integration.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A fingerprint of an input (source text, AST, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub u64);

impl Fingerprint {
    pub fn of(data: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        Fingerprint(hasher.finish())
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// Revision number — incremented each time the database changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

impl Revision {
    pub fn initial() -> Self {
        Revision(0)
    }

    pub fn next(self) -> Self {
        Revision(self.0 + 1)
    }
}

/// A cached query result with its revision.
#[derive(Debug, Clone)]
struct CachedResult<T: Clone> {
    value: T,
    /// The revision at which this result was computed.
    #[allow(dead_code)]
    computed_at: Revision,
    /// Fingerprint of the input that produced this result.
    input_fingerprint: Fingerprint,
}

/// A simple incremental database for memoizing compilation results.
///
/// Each "query" is identified by a string key. Results are cached
/// and only recomputed when the input fingerprint changes.
#[derive(Debug)]
pub struct IncrementalDb {
    /// Current revision number.
    revision: Revision,
    /// Cached type check results: function name → (errors as strings, fingerprint)
    type_check_cache: HashMap<String, CachedResult<Vec<String>>>,
    /// Cached cost analysis results: function name → (cost string, fingerprint)
    cost_cache: HashMap<String, CachedResult<String>>,
    /// Input fingerprints: source file path → fingerprint
    source_fingerprints: HashMap<String, Fingerprint>,
    /// Statistics
    pub stats: CacheStats,
}

/// Cache hit/miss statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl IncrementalDb {
    pub fn new() -> Self {
        Self {
            revision: Revision::initial(),
            type_check_cache: HashMap::new(),
            cost_cache: HashMap::new(),
            source_fingerprints: HashMap::new(),
            stats: CacheStats::default(),
        }
    }

    /// Get the current revision.
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Update a source file's fingerprint. Returns true if it changed.
    pub fn update_source(&mut self, path: &str, source: &str) -> bool {
        let new_fp = Fingerprint::of(source);
        let changed = self.source_fingerprints.get(path) != Some(&new_fp);
        if changed {
            self.source_fingerprints.insert(path.to_string(), new_fp);
            self.revision = self.revision.next();
            self.stats.invalidations += 1;
        }
        changed
    }

    /// Get a source file's fingerprint.
    pub fn source_fingerprint(&self, path: &str) -> Option<Fingerprint> {
        self.source_fingerprints.get(path).copied()
    }

    /// Query type check results for a function. Returns None if cache miss.
    pub fn query_type_check(
        &mut self,
        fn_name: &str,
        input_fp: Fingerprint,
    ) -> Option<Vec<String>> {
        if let Some(cached) = self.type_check_cache.get(fn_name)
            && cached.input_fingerprint == input_fp
        {
            self.stats.hits += 1;
            return Some(cached.value.clone());
        }
        self.stats.misses += 1;
        None
    }

    /// Store type check results.
    pub fn store_type_check(&mut self, fn_name: &str, input_fp: Fingerprint, errors: Vec<String>) {
        self.type_check_cache.insert(
            fn_name.to_string(),
            CachedResult {
                value: errors,
                computed_at: self.revision,
                input_fingerprint: input_fp,
            },
        );
    }

    /// Query cost analysis for a function.
    pub fn query_cost(&mut self, fn_name: &str, input_fp: Fingerprint) -> Option<String> {
        if let Some(cached) = self.cost_cache.get(fn_name)
            && cached.input_fingerprint == input_fp
        {
            self.stats.hits += 1;
            return Some(cached.value.clone());
        }
        self.stats.misses += 1;
        None
    }

    /// Store cost analysis result.
    pub fn store_cost(&mut self, fn_name: &str, input_fp: Fingerprint, cost: String) {
        self.cost_cache.insert(
            fn_name.to_string(),
            CachedResult {
                value: cost,
                computed_at: self.revision,
                input_fingerprint: input_fp,
            },
        );
    }

    /// Invalidate all cached results for items that depend on changed sources.
    pub fn invalidate_dependents(&mut self, changed_functions: &[String]) {
        for name in changed_functions {
            self.type_check_cache.remove(name);
            self.cost_cache.remove(name);
        }
    }

    /// Clear all caches.
    pub fn clear(&mut self) {
        self.type_check_cache.clear();
        self.cost_cache.clear();
        self.revision = self.revision.next();
    }

    /// Get the number of cached entries.
    pub fn cache_size(&self) -> usize {
        self.type_check_cache.len() + self.cost_cache.len()
    }
}

impl Default for IncrementalDb {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_deterministic() {
        let f1 = Fingerprint::of("fn foo() -> Int { 42 }");
        let f2 = Fingerprint::of("fn foo() -> Int { 42 }");
        assert_eq!(f1, f2);
    }

    #[test]
    fn fingerprint_changes_with_content() {
        let f1 = Fingerprint::of("fn foo() -> Int { 42 }");
        let f2 = Fingerprint::of("fn foo() -> Int { 43 }");
        assert_ne!(f1, f2);
    }

    #[test]
    fn revision_increments() {
        let r = Revision::initial();
        assert_eq!(r.0, 0);
        assert_eq!(r.next().0, 1);
    }

    #[test]
    fn cache_hit_and_miss() {
        let mut db = IncrementalDb::new();
        let fp = Fingerprint::of("fn foo() -> Int { 42 }");

        // Miss
        assert!(db.query_type_check("foo", fp).is_none());
        assert_eq!(db.stats.misses, 1);

        // Store
        db.store_type_check("foo", fp, vec![]);

        // Hit
        let result = db.query_type_check("foo", fp);
        assert!(result.is_some());
        assert_eq!(db.stats.hits, 1);
    }

    #[test]
    fn cache_invalidation_on_change() {
        let mut db = IncrementalDb::new();
        let fp1 = Fingerprint::of("version 1");
        db.store_type_check("foo", fp1, vec![]);

        // Different fingerprint → miss
        let fp2 = Fingerprint::of("version 2");
        assert!(db.query_type_check("foo", fp2).is_none());
    }

    #[test]
    fn source_update_tracking() {
        let mut db = IncrementalDb::new();

        // First update
        assert!(db.update_source("main.sp", "fn main() {}"));
        assert_eq!(db.revision().0, 1);

        // Same content → no change
        assert!(!db.update_source("main.sp", "fn main() {}"));
        assert_eq!(db.revision().0, 1);

        // Different content → change
        assert!(db.update_source("main.sp", "fn main() { 42 }"));
        assert_eq!(db.revision().0, 2);
    }

    #[test]
    fn hit_rate_calculation() {
        let mut stats = CacheStats::default();
        assert_eq!(stats.hit_rate(), 0.0);
        stats.hits = 3;
        stats.misses = 1;
        assert!((stats.hit_rate() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn clear_resets_cache() {
        let mut db = IncrementalDb::new();
        let fp = Fingerprint::of("test");
        db.store_type_check("foo", fp, vec![]);
        assert_eq!(db.cache_size(), 1);
        db.clear();
        assert_eq!(db.cache_size(), 0);
    }
}
