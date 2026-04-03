//! Type environment — maps names to types during type checking.

use std::collections::HashMap;

use crate::types::{CapSet, ErrorSet, Ty};

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

/// Top-level type registry — struct definitions, type defs, function signatures.
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    /// Function signatures: name → (param types, return type, capabilities)
    pub functions: HashMap<String, (Vec<Ty>, Ty, CapSet)>,
    /// Error sets declared by functions: name → set of error type names
    pub fn_errors: HashMap<String, ErrorSet>,
    /// Struct definitions: name → field list (name, type)
    pub structs: HashMap<String, Vec<(String, Ty)>>,
    /// Type (enum) definitions: name → variant list (name, field types)
    pub types: HashMap<String, Vec<(String, Vec<Ty>)>>,
    /// Type parameter names for generic functions: name → [type param names]
    pub fn_type_params: HashMap<String, Vec<String>>,
    /// Capability (trait) definitions: name → (type_params, methods: [(method_name, param_types, return_type)])
    #[allow(clippy::type_complexity)]
    pub capabilities: HashMap<String, (Vec<String>, Vec<(String, Vec<Ty>, Ty)>)>,
    /// Trait implementations: (capability_name, type_name) → method impls: [(method_name, param_types, return_type)]
    #[allow(clippy::type_complexity)]
    pub impls: HashMap<(String, String), Vec<(String, Vec<Ty>, Ty)>>,
    /// Type aliases: name → resolved Ty (supports refinement aliases like `alias Port = Int when ...`)
    pub type_aliases: HashMap<String, Ty>,
}
