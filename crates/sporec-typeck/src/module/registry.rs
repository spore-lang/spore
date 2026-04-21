use std::collections::{HashMap, HashSet};

use sporec_parser::ast::{ImportDecl, Item};

use crate::types::{CapSet, ErrorSet, Ty};

use super::loader::ModuleLoader;
use super::prelude::build_prelude_interface;
use super::{ImportedSymbol, ModuleError, ModuleInterface, PreludeOptions, SymbolVisibility};

/// Module registry — stores all known modules and their interfaces.
#[derive(Debug, Clone, Default)]
pub struct ModuleRegistry {
    modules: HashMap<String, ModuleInterface>,
    /// Track module dependencies for cycle detection: module → [modules it imports from].
    dependencies: HashMap<String, Vec<String>>,
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

    /// Record that `importing_module` depends on `imported_module`.
    pub fn record_dependency(&mut self, importing_module: &str, imported_module: &str) {
        self.dependencies
            .entry(importing_module.to_string())
            .or_default()
            .push(imported_module.to_string());
    }

    /// Check for circular dependencies and return any cycles found.
    ///
    /// Uses DFS with temporary (in-stack) and permanent (visited) marks.
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut in_stack: HashSet<String> = HashSet::new();
        let mut cycles: Vec<Vec<String>> = Vec::new();
        let mut stack: Vec<String> = Vec::new();

        let mut all_modules: Vec<&String> = self.dependencies.keys().collect();
        all_modules.sort();
        for module in all_modules {
            if !visited.contains(module) {
                self.dfs_detect(module, &mut visited, &mut in_stack, &mut stack, &mut cycles);
            }
        }
        cycles
    }

    fn dfs_detect(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
        stack: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        in_stack.insert(node.to_string());
        stack.push(node.to_string());

        if let Some(deps) = self.dependencies.get(node) {
            for dep in deps {
                if !visited.contains(dep.as_str()) {
                    self.dfs_detect(dep, visited, in_stack, stack, cycles);
                } else if in_stack.contains(dep.as_str())
                    && let Some(pos) = stack.iter().position(|n| n == dep)
                {
                    let mut cycle: Vec<String> = stack[pos..].to_vec();
                    cycle.push(dep.clone());
                    cycles.push(cycle);
                }
            }
        }

        stack.pop();
        in_stack.remove(node);
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
            } else if module.handlers.contains_key(name) {
                ImportedSymbol::Handler
            } else {
                ImportedSymbol::Capability
            };

            resolved.push((name.clone(), kind));
        }

        Ok(resolved)
    }

    /// Register the standard library prelude.
    pub fn register_prelude(&mut self) {
        self.register_prelude_with_options(PreludeOptions::default());
    }

    /// Register the standard library prelude with custom builtin options.
    pub fn register_prelude_with_options(&mut self, options: PreludeOptions) {
        let mut prelude = build_prelude_interface();

        prelude.types.entry("List".into()).or_default();

        if options.include_console {
            prelude
                .functions
                .insert("print".into(), (vec![Ty::Str], Ty::Unit));
            prelude
                .functions
                .insert("println".into(), (vec![Ty::Str], Ty::Unit));
            prelude
                .functions
                .insert("read_line".into(), (vec![], Ty::Str));
        }

        prelude
            .functions
            .insert("string_length".into(), (vec![Ty::Str], Ty::I32));
        prelude.functions.insert(
            "split".into(),
            (
                vec![Ty::Str, Ty::Str],
                Ty::App("List".into(), vec![Ty::Str]),
            ),
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
        prelude.functions.insert(
            "char_at".into(),
            (
                vec![Ty::Str, Ty::I32],
                Ty::App("Option".into(), vec![Ty::Str]),
            ),
        );
        prelude
            .functions
            .insert("char_to_int".into(), (vec![Ty::Str], Ty::I32));
        prelude
            .functions
            .insert("int_to_char".into(), (vec![Ty::I32], Ty::Str));
        prelude.functions.insert(
            "substring".into(),
            (vec![Ty::Str, Ty::I32, Ty::I32], Ty::Str),
        );
        prelude
            .functions
            .insert("replace".into(), (vec![Ty::Str, Ty::Str, Ty::Str], Ty::Str));
        prelude
            .functions
            .insert("to_string".into(), (vec![Ty::Var(0)], Ty::Str));
        prelude
            .functions
            .insert("string_index_of".into(), (vec![Ty::Str, Ty::Str], Ty::I32));

        prelude
            .functions
            .insert("abs".into(), (vec![Ty::I32], Ty::I32));
        prelude
            .functions
            .insert("min".into(), (vec![Ty::I32, Ty::I32], Ty::I32));
        prelude
            .functions
            .insert("max".into(), (vec![Ty::I32, Ty::I32], Ty::I32));

        let list_t = Ty::App("List".into(), vec![Ty::Var(0)]);
        let list_u = Ty::App("List".into(), vec![Ty::Var(1)]);
        prelude
            .functions
            .insert("len".into(), (vec![Ty::Var(0)], Ty::I32));
        prelude.functions.insert(
            "range".into(),
            (
                vec![Ty::I32, Ty::I32],
                Ty::App("List".into(), vec![Ty::I32]),
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
                    Ty::Fn(
                        vec![Ty::Var(0)],
                        Box::new(Ty::Var(1)),
                        CapSet::new(),
                        ErrorSet::new(),
                    ),
                ],
                list_u.clone(),
            ),
        );
        prelude.functions.insert(
            "filter".into(),
            (
                vec![
                    list_t.clone(),
                    Ty::Fn(
                        vec![Ty::Var(0)],
                        Box::new(Ty::Bool),
                        CapSet::new(),
                        ErrorSet::new(),
                    ),
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
                        ErrorSet::new(),
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
                    Ty::Fn(
                        vec![Ty::Var(0)],
                        Box::new(Ty::Unit),
                        CapSet::new(),
                        ErrorSet::new(),
                    ),
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
            (vec![Ty::Var(0), list_t.clone()], list_t.clone()),
        );
        prelude.functions.insert(
            "head".into(),
            (
                vec![list_t.clone()],
                Ty::App("Option".into(), vec![Ty::Var(0)]),
            ),
        );
        prelude.functions.insert(
            "tail".into(),
            (
                vec![list_t.clone()],
                Ty::App("Option".into(), vec![list_t.clone()]),
            ),
        );
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

    /// Resolve all imports in a module, loading dependencies from disk as needed.
    ///
    /// Recursively processes transitive imports and records dependency edges.
    /// After all imports are loaded, checks for circular dependencies.
    pub fn resolve_imports(
        &mut self,
        loader: &mut ModuleLoader,
        importing_module: &str,
        imports: &[ImportDecl],
    ) -> Result<(), Vec<ModuleError>> {
        let mut errors = Vec::new();
        self.resolve_imports_inner(loader, importing_module, imports, &mut errors);

        let cycles = self.detect_cycles();
        for cycle in cycles {
            errors.push(ModuleError::CircularDependency(cycle));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn resolve_imports_inner(
        &mut self,
        loader: &mut ModuleLoader,
        importing_module: &str,
        imports: &[ImportDecl],
        errors: &mut Vec<ModuleError>,
    ) {
        for decl in imports {
            let path = match decl {
                ImportDecl::Import { path, .. } | ImportDecl::Alias { path, .. } => path.clone(),
            };

            self.record_dependency(importing_module, &path);

            if self.get_by_path(&path).is_some() {
                continue;
            }

            match loader.load_module(&path) {
                Ok(iface) => {
                    let iface = iface.clone();
                    self.register(iface);
                }
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            }

            let sub_imports: Vec<ImportDecl> = loader
                .get_ast(&path)
                .map(|ast| {
                    ast.items
                        .iter()
                        .filter_map(|item| match item {
                            Item::Import(d) => Some(d.clone()),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();

            if !sub_imports.is_empty() {
                self.resolve_imports_inner(loader, &path, &sub_imports, errors);
            }
        }
    }
}
