//! Type environment — maps names to types during type checking.

use std::collections::HashMap;

use crate::types::{EffectSet, ErrorSet, Ty};

/// A scoped type environment (symbol table).
///
/// Uses a stack of scopes for lexical scoping: `let` introduces
/// a new binding in the current scope, blocks push/pop scopes.
#[derive(Debug, Clone)]
pub struct Env {
    scopes: Vec<HashMap<String, Ty>>,
}

impl Env {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    /// Push a new scope (entering a block / function body).
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the current scope (leaving a block / function body).
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Define a name in the current (innermost) scope.
    pub fn define(&mut self, name: String, ty: Ty) {
        self.scopes.last_mut().unwrap().insert(name, ty);
    }

    /// Look up a name, searching from innermost scope outward.
    pub fn lookup(&self, name: &str) -> Option<&Ty> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    /// Return all bindings visible in the current scope chain (innermost wins).
    pub fn all_bindings(&self) -> std::collections::BTreeMap<String, Ty> {
        let mut result = std::collections::BTreeMap::new();
        // Iterate from outermost to innermost so inner scopes shadow outer
        for scope in &self.scopes {
            for (k, v) in scope {
                result.insert(k.clone(), v.clone());
            }
        }
        result
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

/// Registered signature information for a named handler.
#[derive(Debug, Clone, PartialEq)]
pub struct HandlerInfo {
    pub handled_effects: EffectSet,
    pub uses_effects: EffectSet,
    pub fields: Vec<(String, Ty)>,
    pub methods: HashMap<String, Vec<(String, Vec<Ty>, Ty)>>,
}

/// Top-level type registry — struct definitions, type defs, function signatures.
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    /// Function signatures: name → (param types, return type, required effects)
    pub functions: HashMap<String, (Vec<Ty>, Ty, EffectSet)>,
    /// Error sets declared by functions: name → set of error type names
    pub fn_errors: HashMap<String, ErrorSet>,
    /// Struct definitions: name → field list (name, type)
    pub structs: HashMap<String, Vec<(String, Ty)>>,
    /// Generic struct type parameters: name → ordered type parameter names
    pub struct_type_params: HashMap<String, Vec<String>>,
    /// Type (enum) definitions: name → variant list (name, field types)
    pub types: HashMap<String, Vec<(String, Vec<Ty>)>>,
    /// Generic type (enum) type parameters: name → ordered type parameter names
    pub type_type_params: HashMap<String, Vec<String>>,
    /// Type parameter names for generic functions: name → [type param names]
    pub fn_type_params: HashMap<String, Vec<String>>,
    /// `where` trait bounds for functions: name → [(type_var, trait_name)]
    pub fn_where_bounds: HashMap<String, Vec<(String, String)>>,
    /// Interface (trait) definitions: name → (type_params, methods: [(method_name, param_types, return_type)])
    #[allow(clippy::type_complexity)]
    pub interfaces: HashMap<String, (Vec<String>, Vec<(String, Vec<Ty>, Ty)>)>,
    /// Trait implementations: (trait_name, type_name) → method impls: [(method_name, param_types, return_type)]
    #[allow(clippy::type_complexity)]
    pub impls: HashMap<(String, String), Vec<(String, Vec<Ty>, Ty)>>,
    /// Type aliases: name → resolved Ty (supports refinement aliases like `alias Port = I64 when ...`)
    pub type_aliases: HashMap<String, Ty>,
    /// Named handlers: handler name → handler metadata.
    pub handlers: HashMap<String, HandlerInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Env scope management ────────────────────────────────────────

    #[test]
    fn define_and_lookup() {
        let mut env = Env::new();
        env.define("x".into(), Ty::I32);
        assert_eq!(env.lookup("x"), Some(&Ty::I32));
    }

    #[test]
    fn lookup_missing_returns_none() {
        let env = Env::new();
        assert_eq!(env.lookup("missing"), None);
    }

    #[test]
    fn inner_scope_shadows_outer() {
        let mut env = Env::new();
        env.define("x".into(), Ty::I32);
        env.push_scope();
        env.define("x".into(), Ty::Bool);
        assert_eq!(env.lookup("x"), Some(&Ty::Bool));
        env.pop_scope();
        assert_eq!(env.lookup("x"), Some(&Ty::I32));
    }

    #[test]
    fn inner_scope_sees_outer() {
        let mut env = Env::new();
        env.define("x".into(), Ty::I32);
        env.push_scope();
        assert_eq!(env.lookup("x"), Some(&Ty::I32));
        env.pop_scope();
    }

    #[test]
    fn pop_scope_removes_bindings() {
        let mut env = Env::new();
        env.push_scope();
        env.define("local".into(), Ty::Str);
        assert_eq!(env.lookup("local"), Some(&Ty::Str));
        env.pop_scope();
        assert_eq!(env.lookup("local"), None);
    }

    #[test]
    fn all_bindings_merges_scopes() {
        let mut env = Env::new();
        env.define("a".into(), Ty::I32);
        env.push_scope();
        env.define("b".into(), Ty::Bool);
        let bindings = env.all_bindings();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings.get("a"), Some(&Ty::I32));
        assert_eq!(bindings.get("b"), Some(&Ty::Bool));
    }

    #[test]
    fn all_bindings_inner_shadows_outer() {
        let mut env = Env::new();
        env.define("x".into(), Ty::I32);
        env.push_scope();
        env.define("x".into(), Ty::Bool);
        let bindings = env.all_bindings();
        assert_eq!(bindings.get("x"), Some(&Ty::Bool));
    }

    #[test]
    fn nested_scopes_depth() {
        let mut env = Env::new();
        env.define("a".into(), Ty::I32);
        env.push_scope();
        env.define("b".into(), Ty::Bool);
        env.push_scope();
        env.define("c".into(), Ty::Str);
        assert_eq!(env.lookup("a"), Some(&Ty::I32));
        assert_eq!(env.lookup("b"), Some(&Ty::Bool));
        assert_eq!(env.lookup("c"), Some(&Ty::Str));
        env.pop_scope();
        assert_eq!(env.lookup("c"), None);
        assert_eq!(env.lookup("b"), Some(&Ty::Bool));
        env.pop_scope();
        assert_eq!(env.lookup("b"), None);
    }
}
