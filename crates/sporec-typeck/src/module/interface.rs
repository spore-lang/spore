use std::collections::{HashMap, HashSet};

use crate::env::HandlerInfo;
use crate::types::{CapSet, ErrorSet, Ty};

use super::SymbolVisibility;

/// Represents a compiled module's public interface.
#[derive(Debug, Clone, Default)]
pub struct ModuleInterface {
    /// Module path (e.g., ["Std", "Collections", "List"])
    pub path: Vec<String>,
    /// Exported functions: name → (param types, return type)
    pub functions: HashMap<String, (Vec<Ty>, Ty)>,
    /// Exported function capabilities: name → declared/normalized `uses [...]`
    pub function_caps: HashMap<String, CapSet>,
    /// Exported function error sets: name → declared `! E1 | E2`
    pub function_errors: HashMap<String, ErrorSet>,
    /// Exported generic function type parameters.
    pub function_type_params: HashMap<String, Vec<String>>,
    /// Exported generic `where` bounds for functions.
    pub function_where_bounds: HashMap<String, Vec<(String, String)>>,
    /// Exported types: name → variant names + field types
    pub types: HashMap<String, Vec<(String, Vec<Ty>)>>,
    /// Exported structs: name → field names + types
    pub structs: HashMap<String, Vec<(String, Ty)>>,
    /// Exported generic struct parameters: name → ordered type parameter names
    pub struct_type_params: HashMap<String, Vec<String>>,
    /// Exported capabilities
    pub capabilities: HashSet<String>,
    /// Exported capability/effect method signatures.
    #[allow(clippy::type_complexity)]
    pub capability_methods: HashMap<String, (Vec<String>, Vec<(String, Vec<Ty>, Ty)>)>,
    /// Exported named handlers
    pub handlers: HashMap<String, HandlerInfo>,
    /// Visibility of each symbol
    pub visibilities: HashMap<String, SymbolVisibility>,
}

impl ModuleInterface {
    pub fn new(path: Vec<String>) -> Self {
        Self {
            path,
            ..Default::default()
        }
    }

    /// Set visibility for a symbol.
    pub fn set_visibility(&mut self, name: &str, vis: SymbolVisibility) {
        self.visibilities.insert(name.to_string(), vis);
    }

    /// Get visibility of a symbol (defaults to Pub for unset entries, e.g. prelude).
    pub fn visibility(&self, name: &str) -> &SymbolVisibility {
        self.visibilities
            .get(name)
            .unwrap_or(&SymbolVisibility::Pub)
    }

    /// Get the fully-qualified module name.
    pub fn qualified_name(&self) -> String {
        self.path.join(".")
    }

    /// Check if a name is exported by this module.
    pub fn exports(&self, name: &str) -> bool {
        self.functions.contains_key(name)
            || self.types.contains_key(name)
            || self.structs.contains_key(name)
            || self.capabilities.contains(name)
            || self.handlers.contains_key(name)
    }

    /// Get all exported names (sorted, deduplicated).
    pub fn all_exports(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .functions
            .keys()
            .chain(self.types.keys())
            .chain(self.structs.keys())
            .chain(self.capabilities.iter())
            .chain(self.handlers.keys())
            .cloned()
            .collect();
        names.sort();
        names.dedup();
        names
    }
}
