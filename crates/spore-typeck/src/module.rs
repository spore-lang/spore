//! Module system — resolution, exports, and import validation.

use std::collections::{HashMap, HashSet};

use spore_parser::ast::Visibility;

use crate::types::{CapSet, Ty};

/// Visibility of an exported symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolVisibility {
    Private,
    PubPkg,
    Pub,
}

impl From<&Visibility> for SymbolVisibility {
    fn from(v: &Visibility) -> Self {
        match v {
            Visibility::Private => SymbolVisibility::Private,
            Visibility::PubPkg => SymbolVisibility::PubPkg,
            Visibility::Pub => SymbolVisibility::Pub,
        }
    }
}

/// Represents a compiled module's public interface.
#[derive(Debug, Clone, Default)]
pub struct ModuleInterface {
    /// Module path (e.g., ["Std", "Collections", "List"])
    pub path: Vec<String>,
    /// Exported functions: name → (param types, return type)
    pub functions: HashMap<String, (Vec<Ty>, Ty)>,
    /// Exported types: name → variant list
    pub types: HashMap<String, Vec<String>>,
    /// Exported structs: name → field names
    pub structs: HashMap<String, Vec<String>>,
    /// Exported capabilities
    pub capabilities: HashSet<String>,
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
    }

    /// Get all exported names (sorted, deduplicated).
    pub fn all_exports(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .functions
            .keys()
            .chain(self.types.keys())
            .chain(self.structs.keys())
            .chain(self.capabilities.iter())
            .cloned()
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

/// Module registry — stores all known modules and their interfaces.
#[derive(Debug, Clone, Default)]
pub struct ModuleRegistry {
    modules: HashMap<String, ModuleInterface>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a module interface.
    pub fn register(&mut self, module: ModuleInterface) {
        let key = module.qualified_name();
        self.modules.insert(key, module);
    }

    /// Look up a module by its path segments.
    pub fn get(&self, path: &[String]) -> Option<&ModuleInterface> {
        let key = path.join(".");
        self.modules.get(&key)
    }

    /// Look up a module by its dot-separated path string.
    pub fn get_by_path(&self, path: &str) -> Option<&ModuleInterface> {
        self.modules.get(path)
    }

    /// Resolve an import: check that the module exists and the requested names
    /// are exported, enforcing visibility.
    pub fn resolve_import(
        &self,
        module_path: &[String],
        requested_names: &[String],
    ) -> Result<Vec<(String, ImportedSymbol)>, ModuleError> {
        let module = self
            .get(module_path)
            .ok_or_else(|| ModuleError::ModuleNotFound(module_path.join(".")))?;

        let mut resolved = Vec::new();
        for name in requested_names {
            if !module.exports(name) {
                return Err(ModuleError::SymbolNotFound {
                    module: module_path.join("."),
                    symbol: name.clone(),
                });
            }

            // Enforce visibility
            let vis = module.visibility(name);
            if *vis == SymbolVisibility::Private {
                return Err(ModuleError::PrivateSymbol {
                    module: module_path.join("."),
                    symbol: name.clone(),
                });
            }

            let kind = if module.functions.contains_key(name) {
                ImportedSymbol::Function
            } else if module.types.contains_key(name) {
                ImportedSymbol::Type
            } else if module.structs.contains_key(name) {
                ImportedSymbol::Struct
            } else {
                ImportedSymbol::Capability
            };

            resolved.push((name.clone(), kind));
        }

        Ok(resolved)
    }

    /// Register the standard library prelude.
    pub fn register_prelude(&mut self) {
        let mut prelude = ModuleInterface::new(vec!["Std".into(), "Prelude".into()]);

        prelude
            .types
            .insert("Option".into(), vec!["Some".into(), "None".into()]);
        prelude
            .types
            .insert("Result".into(), vec!["Ok".into(), "Err".into()]);
        prelude.types.insert("List".into(), vec![]);

        // ── IO builtins ──────────────────────────────────────────
        prelude
            .functions
            .insert("print".into(), (vec![Ty::Str], Ty::Unit));
        prelude
            .functions
            .insert("println".into(), (vec![Ty::Str], Ty::Unit));
        prelude
            .functions
            .insert("read_line".into(), (vec![], Ty::Str));

        // ── String builtins ──────────────────────────────────────
        prelude
            .functions
            .insert("string_length".into(), (vec![Ty::Str], Ty::Int));
        prelude.functions.insert(
            "split".into(),
            (vec![Ty::Str, Ty::Str], Ty::Named("List".into())),
        );
        prelude
            .functions
            .insert("trim".into(), (vec![Ty::Str], Ty::Str));
        prelude
            .functions
            .insert("to_upper".into(), (vec![Ty::Str], Ty::Str));
        prelude
            .functions
            .insert("to_lower".into(), (vec![Ty::Str], Ty::Str));
        prelude
            .functions
            .insert("starts_with".into(), (vec![Ty::Str, Ty::Str], Ty::Bool));
        prelude
            .functions
            .insert("ends_with".into(), (vec![Ty::Str, Ty::Str], Ty::Bool));
        prelude
            .functions
            .insert("char_at".into(), (vec![Ty::Str, Ty::Int], Ty::Str));
        prelude.functions.insert(
            "substring".into(),
            (vec![Ty::Str, Ty::Int, Ty::Int], Ty::Str),
        );
        prelude
            .functions
            .insert("replace".into(), (vec![Ty::Str, Ty::Str, Ty::Str], Ty::Str));
        prelude
            .functions
            .insert("to_string".into(), (vec![Ty::Int], Ty::Str));
        prelude
            .functions
            .insert("toString".into(), (vec![Ty::Int], Ty::Str));
        prelude
            .functions
            .insert("string_index_of".into(), (vec![Ty::Str, Ty::Str], Ty::Int));

        // ── Math builtins ────────────────────────────────────────
        prelude
            .functions
            .insert("abs".into(), (vec![Ty::Int], Ty::Int));
        prelude
            .functions
            .insert("min".into(), (vec![Ty::Int, Ty::Int], Ty::Int));
        prelude
            .functions
            .insert("max".into(), (vec![Ty::Int, Ty::Int], Ty::Int));

        // ── List builtins ────────────────────────────────────────
        // Use Ty::App("List", [Ty::Var(N)]) for generic list types.
        // Var IDs are instantiated to fresh variables on each lookup.
        let list_t = Ty::App("List".into(), vec![Ty::Var(0)]);
        let list_u = Ty::App("List".into(), vec![Ty::Var(1)]);

        prelude
            .functions
            .insert("len".into(), (vec![list_t.clone()], Ty::Int));
        prelude.functions.insert(
            "range".into(),
            (
                vec![Ty::Int, Ty::Int],
                Ty::App("List".into(), vec![Ty::Int]),
            ),
        );
        prelude
            .functions
            .insert("reverse".into(), (vec![list_t.clone()], list_t.clone()));
        prelude.functions.insert(
            "map".into(),
            (
                vec![
                    list_t.clone(),
                    Ty::Fn(vec![Ty::Var(0)], Box::new(Ty::Var(1)), CapSet::new()),
                ],
                list_u.clone(),
            ),
        );
        prelude.functions.insert(
            "filter".into(),
            (
                vec![
                    list_t.clone(),
                    Ty::Fn(vec![Ty::Var(0)], Box::new(Ty::Bool), CapSet::new()),
                ],
                list_t.clone(),
            ),
        );
        prelude.functions.insert(
            "fold".into(),
            (
                vec![
                    list_t.clone(),
                    Ty::Var(1),
                    Ty::Fn(
                        vec![Ty::Var(1), Ty::Var(0)],
                        Box::new(Ty::Var(1)),
                        CapSet::new(),
                    ),
                ],
                Ty::Var(1),
            ),
        );
        prelude.functions.insert(
            "each".into(),
            (
                vec![
                    list_t.clone(),
                    Ty::Fn(vec![Ty::Var(0)], Box::new(Ty::Unit), CapSet::new()),
                ],
                Ty::Unit,
            ),
        );
        prelude.functions.insert(
            "append".into(),
            (vec![list_t.clone(), Ty::Var(0)], list_t.clone()),
        );
        prelude.functions.insert(
            "prepend".into(),
            (vec![list_t.clone(), Ty::Var(0)], list_t.clone()),
        );
        prelude
            .functions
            .insert("head".into(), (vec![list_t.clone()], Ty::Var(0)));
        prelude
            .functions
            .insert("tail".into(), (vec![list_t.clone()], list_t.clone()));
        prelude.functions.insert(
            "contains".into(),
            (vec![list_t.clone(), Ty::Var(0)], Ty::Bool),
        );
        prelude.functions.insert(
            "concat".into(),
            (vec![list_t.clone(), list_t.clone()], list_t.clone()),
        );

        self.register(prelude);
    }

    /// Get all registered module paths.
    pub fn all_modules(&self) -> Vec<String> {
        let mut paths: Vec<String> = self.modules.keys().cloned().collect();
        paths.sort();
        paths
    }

    /// Get all registered module interfaces.
    pub fn all_interfaces(&self) -> impl Iterator<Item = &ModuleInterface> {
        self.modules.values()
    }
}

/// The kind of an imported symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedSymbol {
    Function,
    Type,
    Struct,
    Capability,
}

/// Module resolution errors.
#[derive(Debug, Clone)]
pub enum ModuleError {
    ModuleNotFound(String),
    SymbolNotFound { module: String, symbol: String },
    PrivateSymbol { module: String, symbol: String },
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleError::ModuleNotFound(m) => write!(f, "module `{m}` not found"),
            ModuleError::SymbolNotFound { module, symbol } => {
                write!(f, "symbol `{symbol}` not found in module `{module}`")
            }
            ModuleError::PrivateSymbol { module, symbol } => {
                write!(
                    f,
                    "symbol `{symbol}` in module `{module}` is private and not accessible"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_lookup_module() {
        let mut reg = ModuleRegistry::new();
        let mut m = ModuleInterface::new(vec!["Math".into()]);
        m.functions
            .insert("sqrt".into(), (vec![Ty::Float], Ty::Float));
        reg.register(m);

        let found = reg.get(&["Math".into()]);
        assert!(found.is_some());
        assert!(found.unwrap().exports("sqrt"));
    }

    #[test]
    fn resolve_import_success() {
        let mut reg = ModuleRegistry::new();
        let mut m = ModuleInterface::new(vec!["Collections".into()]);
        m.types.insert("List".into(), vec![]);
        m.functions.insert("sort".into(), (vec![], Ty::Unit));
        reg.register(m);

        let result = reg.resolve_import(&["Collections".into()], &["List".into(), "sort".into()]);
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn resolve_import_module_not_found() {
        let reg = ModuleRegistry::new();
        let result = reg.resolve_import(&["NonExistent".into()], &["foo".into()]);
        assert!(matches!(result, Err(ModuleError::ModuleNotFound(_))));
    }

    #[test]
    fn resolve_import_symbol_not_found() {
        let mut reg = ModuleRegistry::new();
        reg.register(ModuleInterface::new(vec!["Empty".into()]));
        let result = reg.resolve_import(&["Empty".into()], &["missing".into()]);
        assert!(matches!(result, Err(ModuleError::SymbolNotFound { .. })));
    }

    #[test]
    fn prelude_registration() {
        let mut reg = ModuleRegistry::new();
        reg.register_prelude();
        let prelude = reg.get(&["Std".into(), "Prelude".into()]);
        assert!(prelude.is_some());
        assert!(prelude.unwrap().exports("Option"));
        assert!(prelude.unwrap().exports("print"));
    }

    #[test]
    fn all_exports_sorted() {
        let mut m = ModuleInterface::new(vec!["Test".into()]);
        m.functions.insert("beta".into(), (vec![], Ty::Unit));
        m.functions.insert("alpha".into(), (vec![], Ty::Unit));
        m.types.insert("Gamma".into(), vec![]);
        let exports = m.all_exports();
        assert_eq!(exports, vec!["Gamma", "alpha", "beta"]);
    }

    #[test]
    fn get_by_path_string() {
        let mut reg = ModuleRegistry::new();
        let m = ModuleInterface::new(vec!["Std".into(), "IO".into()]);
        reg.register(m);
        assert!(reg.get_by_path("Std.IO").is_some());
        assert!(reg.get_by_path("Std.Math").is_none());
    }

    #[test]
    fn all_modules_sorted() {
        let mut reg = ModuleRegistry::new();
        reg.register(ModuleInterface::new(vec!["Zebra".into()]));
        reg.register(ModuleInterface::new(vec!["Alpha".into()]));
        assert_eq!(reg.all_modules(), vec!["Alpha", "Zebra"]);
    }

    #[test]
    fn resolve_import_private_symbol() {
        let mut reg = ModuleRegistry::new();
        let mut m = ModuleInterface::new(vec!["Lib".into()]);
        m.functions.insert("secret".into(), (vec![], Ty::Unit));
        m.set_visibility("secret", SymbolVisibility::Private);
        reg.register(m);

        let result = reg.resolve_import(&["Lib".into()], &["secret".into()]);
        assert!(matches!(result, Err(ModuleError::PrivateSymbol { .. })));
    }

    #[test]
    fn resolve_import_pub_pkg_symbol() {
        let mut reg = ModuleRegistry::new();
        let mut m = ModuleInterface::new(vec!["Lib".into()]);
        m.functions.insert("internal".into(), (vec![], Ty::Unit));
        m.set_visibility("internal", SymbolVisibility::PubPkg);
        reg.register(m);

        // pub(pkg) is accessible (same package assumed for now)
        let result = reg.resolve_import(&["Lib".into()], &["internal".into()]);
        assert!(result.is_ok());
    }
}
