//! Module system — resolution, exports, and import validation.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use spore_parser::{
    ast::{ImportDecl, Item, Module as AstModule, TypeExpr, Visibility},
    parse,
};

use crate::types::{CapSet, ErrorSet, Ty};

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
    /// Exported types: name → variant names + field types
    pub types: HashMap<String, Vec<(String, Vec<Ty>)>>,
    /// Exported structs: name → field names + types
    pub structs: HashMap<String, Vec<(String, Ty)>>,
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

fn prelude_type_mapping(type_params: &[String]) -> HashMap<String, Ty> {
    type_params
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), Ty::Var(idx as u32)))
        .collect()
}

fn resolve_prelude_type(te: &TypeExpr, mapping: &HashMap<String, Ty>) -> Ty {
    match te {
        TypeExpr::Named(name) => match name.as_str() {
            "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64" | "Int" => Ty::Int,
            "F32" | "F64" | "Float" => Ty::Float,
            "Bool" => Ty::Bool,
            "Str" | "String" => Ty::Str,
            "Char" => Ty::Char,
            "Never" => Ty::Never,
            _ => mapping
                .get(name)
                .cloned()
                .unwrap_or_else(|| Ty::Named(name.clone())),
        },
        TypeExpr::Generic(name, args) => Ty::App(
            name.clone(),
            args.iter()
                .map(|arg| resolve_prelude_type(arg, mapping))
                .collect(),
        ),
        TypeExpr::Tuple(types) => {
            if types.is_empty() {
                Ty::Unit
            } else {
                Ty::Tuple(
                    types
                        .iter()
                        .map(|ty| resolve_prelude_type(ty, mapping))
                        .collect(),
                )
            }
        }
        TypeExpr::Function(params, ret, error_exprs) => {
            let errors: ErrorSet = error_exprs
                .iter()
                .filter_map(|te| match te {
                    TypeExpr::Named(name) => Some(name.clone()),
                    _ => None,
                })
                .collect();
            Ty::Fn(
                params
                    .iter()
                    .map(|param| resolve_prelude_type(param, mapping))
                    .collect(),
                Box::new(resolve_prelude_type(ret, mapping)),
                CapSet::new(),
                errors,
            )
        }
        TypeExpr::Refinement(base, var_name, pred_expr) => Ty::Refined(
            Box::new(resolve_prelude_type(base, mapping)),
            var_name.clone(),
            pred_expr.clone(),
        ),
        TypeExpr::Record(fields) => Ty::Record(
            fields
                .iter()
                .map(|(name, ty)| (name.clone(), resolve_prelude_type(ty, mapping)))
                .collect(),
        ),
    }
}

fn build_prelude_interface() -> ModuleInterface {
    let module = parse(include_str!("../../../stdlib/prelude.sp"))
        .expect("embedded stdlib/prelude.sp must parse");
    let mut iface = ModuleInterface::new(vec!["Std".into(), "Prelude".into()]);

    for item in &module.items {
        match item {
            Item::Function(f) => {
                let mut type_params = f.type_params.clone();
                if let Some(wc) = &f.where_clause {
                    type_params.extend(wc.constraints.iter().map(|c| c.type_var.clone()));
                }
                type_params.sort();
                type_params.dedup();
                let mapping = prelude_type_mapping(&type_params);
                let param_tys = f
                    .params
                    .iter()
                    .map(|param| resolve_prelude_type(&param.ty, &mapping))
                    .collect();
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(|ty| resolve_prelude_type(ty, &mapping))
                    .unwrap_or(Ty::Unit);
                iface.functions.insert(f.name.clone(), (param_tys, ret_ty));
                iface.set_visibility(&f.name, SymbolVisibility::from(&f.visibility));
            }
            Item::StructDef(s) => {
                let mapping = prelude_type_mapping(&s.type_params);
                let fields = s
                    .fields
                    .iter()
                    .map(|field| {
                        (
                            field.name.clone(),
                            resolve_prelude_type(&field.ty, &mapping),
                        )
                    })
                    .collect();
                iface.structs.insert(s.name.clone(), fields);
                iface.set_visibility(&s.name, SymbolVisibility::from(&s.visibility));
            }
            Item::TypeDef(t) => {
                let mapping = prelude_type_mapping(&t.type_params);
                let variants = t
                    .variants
                    .iter()
                    .map(|variant| {
                        (
                            variant.name.clone(),
                            variant
                                .fields
                                .iter()
                                .map(|field| resolve_prelude_type(field, &mapping))
                                .collect(),
                        )
                    })
                    .collect();
                iface.types.insert(t.name.clone(), variants);
                iface.set_visibility(&t.name, SymbolVisibility::from(&t.visibility));
            }
            Item::CapabilityDef(cap) => {
                iface.capabilities.insert(cap.name.clone());
                iface.set_visibility(&cap.name, SymbolVisibility::from(&cap.visibility));
            }
            Item::Const(_)
            | Item::ImplDef(_)
            | Item::Import(_)
            | Item::Alias(_)
            | Item::CapabilityAlias { .. }
            | Item::TraitDef(_)
            | Item::EffectDef(_)
            | Item::EffectAlias(_)
            | Item::HandlerDef(_) => {}
        }
    }

    iface
}

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
        all_modules.sort(); // deterministic order
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
                } else if in_stack.contains(dep.as_str()) {
                    // Found a cycle — extract the cycle path from the stack
                    if let Some(pos) = stack.iter().position(|n| n == dep) {
                        let mut cycle: Vec<String> = stack[pos..].to_vec();
                        cycle.push(dep.clone()); // close the cycle
                        cycles.push(cycle);
                    }
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

            // Enforce visibility
            let vis = module.visibility(name);
            if *vis == SymbolVisibility::Private {
                return Err(ModuleError::PrivateSymbol {
                    module: module_path.join("."),
                    symbol: name.clone(),
                });
            }
            // TODO: enforce PubPkg — when package identity is tracked on
            // ModuleInterface, reject PubPkg symbols imported from a
            // different package.  For now PubPkg is treated as Pub.

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
        let mut prelude = build_prelude_interface();

        prelude.types.entry("List".into()).or_default();

        prelude
            .functions
            .insert("print".into(), (vec![Ty::Str], Ty::Unit));
        prelude
            .functions
            .insert("println".into(), (vec![Ty::Str], Ty::Unit));
        prelude
            .functions
            .insert("read_line".into(), (vec![], Ty::Str));

        prelude
            .functions
            .insert("string_length".into(), (vec![Ty::Str], Ty::Int));
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
                vec![Ty::Str, Ty::Int],
                Ty::App("Option".into(), vec![Ty::Str]),
            ),
        );
        prelude.functions.insert(
            "substring".into(),
            (vec![Ty::Str, Ty::Int, Ty::Int], Ty::Str),
        );
        prelude
            .functions
            .insert("replace".into(), (vec![Ty::Str, Ty::Str, Ty::Str], Ty::Str));
        prelude
            .functions
            .insert("to_string".into(), (vec![Ty::Var(0)], Ty::Str));
        prelude
            .functions
            .insert("string_index_of".into(), (vec![Ty::Str, Ty::Str], Ty::Int));

        prelude
            .functions
            .insert("abs".into(), (vec![Ty::Int], Ty::Int));
        prelude
            .functions
            .insert("min".into(), (vec![Ty::Int, Ty::Int], Ty::Int));
        prelude
            .functions
            .insert("max".into(), (vec![Ty::Int, Ty::Int], Ty::Int));

        let list_t = Ty::App("List".into(), vec![Ty::Var(0)]);
        let list_u = Ty::App("List".into(), vec![Ty::Var(1)]);
        prelude
            .functions
            .insert("len".into(), (vec![Ty::Var(0)], Ty::Int));
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

        // Check for circular dependencies after all resolution
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

            // Skip if already registered
            if self.get_by_path(&path).is_some() {
                continue;
            }

            // Load the module from disk
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

            // Recursively resolve transitive imports from the loaded module
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

/// Resolves module paths to filesystem paths and loads module interfaces.
///
/// The loader caches both parsed ASTs and extracted interfaces so that
/// each module is read from disk at most once.
pub struct ModuleLoader {
    /// Project root directory.
    root: PathBuf,
    /// Cache of already-loaded module interfaces.
    loaded: HashMap<String, ModuleInterface>,
    /// Cache of parsed ASTs (needed for transitive import extraction and interpreter).
    asts: HashMap<String, AstModule>,
}

impl ModuleLoader {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            loaded: HashMap::new(),
            asts: HashMap::new(),
        }
    }

    /// Resolve a dot-separated module path to a filesystem path.
    ///
    /// `"billing.invoice"` → `{root}/src/billing/invoice.sp`
    pub fn resolve_path(&self, module_path: &str) -> Option<PathBuf> {
        let rel = module_path.replace('.', "/");
        let path = self.root.join("src").join(&rel).with_extension("sp");
        if path.exists() { Some(path) } else { None }
    }

    /// Load a module from disk, parse it, and extract its interface.
    ///
    /// Returns a cached interface if the module has already been loaded.
    pub fn load_module(&mut self, module_path: &str) -> Result<&ModuleInterface, ModuleError> {
        if self.loaded.contains_key(module_path) {
            return Ok(&self.loaded[module_path]);
        }

        let file_path = self
            .resolve_path(module_path)
            .ok_or_else(|| ModuleError::ModuleNotFound(module_path.to_string()))?;

        let source = std::fs::read_to_string(&file_path).map_err(|e| ModuleError::IoError {
            module: module_path.to_string(),
            detail: e.to_string(),
        })?;

        let ast = spore_parser::parse(&source).map_err(|errs| ModuleError::ParseError {
            module: module_path.to_string(),
            detail: errs
                .into_iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        })?;

        let mut iface = crate::build_module_interface(&ast);
        iface.path = module_path.split('.').map(|s| s.to_string()).collect();

        self.asts.insert(module_path.to_string(), ast);
        self.loaded.insert(module_path.to_string(), iface);
        Ok(&self.loaded[module_path])
    }

    /// Get the cached AST for a previously loaded module.
    pub fn get_ast(&self, module_path: &str) -> Option<&AstModule> {
        self.asts.get(module_path)
    }

    /// Get a cached module interface.
    pub fn get_cached(&self, module_path: &str) -> Option<&ModuleInterface> {
        self.loaded.get(module_path)
    }

    /// Return all loaded module paths.
    pub fn loaded_modules(&self) -> Vec<String> {
        self.asts.keys().cloned().collect()
    }

    /// Get the project root path.
    pub fn root(&self) -> &Path {
        &self.root
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
    CircularDependency(Vec<String>),
    IoError { module: String, detail: String },
    ParseError { module: String, detail: String },
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
            ModuleError::CircularDependency(cycle) => {
                write!(f, "circular module dependency: {}", cycle.join(" -> "))
            }
            ModuleError::IoError { module, detail } => {
                write!(f, "cannot read module `{module}`: {detail}")
            }
            ModuleError::ParseError { module, detail } => {
                write!(f, "parse error in module `{module}`: {detail}")
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
        assert!(prelude.unwrap().exports("identity"));
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

    #[test]
    fn detect_cycle_a_imports_b_imports_a() {
        let mut reg = ModuleRegistry::new();
        reg.record_dependency("A", "B");
        reg.record_dependency("B", "A");
        let cycles = reg.detect_cycles();
        assert!(!cycles.is_empty(), "expected a cycle between A and B");
        let cycle = &cycles[0];
        assert!(
            cycle.first() == cycle.last(),
            "cycle should close on itself"
        );
    }

    #[test]
    fn detect_cycle_three_modules() {
        let mut reg = ModuleRegistry::new();
        reg.record_dependency("A", "B");
        reg.record_dependency("B", "C");
        reg.record_dependency("C", "A");
        let cycles = reg.detect_cycles();
        assert!(!cycles.is_empty(), "expected a cycle among A, B, C");
        let cycle = &cycles[0];
        assert!(
            cycle.first() == cycle.last(),
            "cycle should close on itself"
        );
        assert!(cycle.len() == 4, "cycle path should be [A, B, C, A]");
    }

    #[test]
    fn no_cycle_linear_chain() {
        let mut reg = ModuleRegistry::new();
        reg.record_dependency("A", "B");
        reg.record_dependency("B", "C");
        let cycles = reg.detect_cycles();
        assert!(cycles.is_empty(), "expected no cycles in a linear chain");
    }

    #[test]
    fn detect_self_import_cycle() {
        let mut reg = ModuleRegistry::new();
        reg.record_dependency("A", "A");
        let cycles = reg.detect_cycles();
        assert!(!cycles.is_empty(), "expected a self-import cycle");
        let cycle = &cycles[0];
        assert_eq!(cycle, &vec!["A".to_string(), "A".to_string()]);
    }

    // ── ModuleLoader tests ──────────────────────────────────────────

    #[test]
    fn test_resolve_path() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src").join("billing");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("invoice.sp"), "pub fn total() -> Int { 0 }").unwrap();

        let loader = ModuleLoader::new(dir.path().to_path_buf());
        let resolved = loader.resolve_path("billing.invoice");
        assert!(resolved.is_some());
        assert!(resolved.unwrap().ends_with("src/billing/invoice.sp"));

        // Non-existent module returns None
        assert!(loader.resolve_path("billing.nonexistent").is_none());
    }

    #[test]
    fn test_load_module_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src").join("utils");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("math.sp"),
            "pub fn add(a: Int, b: Int) -> Int { a + b }",
        )
        .unwrap();

        let mut loader = ModuleLoader::new(dir.path().to_path_buf());
        let iface = loader.load_module("utils.math").unwrap();
        assert!(iface.exports("add"));
        assert_eq!(*iface.visibility("add"), SymbolVisibility::Pub);
        assert_eq!(iface.qualified_name(), "utils.math");

        // Second load returns cached result
        let iface2 = loader.load_module("utils.math").unwrap();
        assert!(iface2.exports("add"));
    }

    #[test]
    fn test_import_resolution_chain() {
        let dir = tempfile::tempdir().unwrap();
        let src_b = dir.path().join("src").join("b");
        let src_c = dir.path().join("src").join("c");
        std::fs::create_dir_all(&src_b).unwrap();
        std::fs::create_dir_all(&src_c).unwrap();

        // C has no imports
        std::fs::write(src_c.join("util.sp"), "pub fn helper() -> Int { 1 }").unwrap();

        // B imports C
        std::fs::write(
            src_b.join("core.sp"),
            "import c.util\npub fn work() -> Int { helper() }",
        )
        .unwrap();

        let mut loader = ModuleLoader::new(dir.path().to_path_buf());
        let mut registry = ModuleRegistry::new();

        let imports = vec![ImportDecl::Import {
            path: "b.core".into(),
            alias: "core".into(),
            span: None,
        }];

        registry
            .resolve_imports(&mut loader, "a.main", &imports)
            .unwrap();

        // Both b.core and c.util should be registered
        assert!(registry.get_by_path("b.core").is_some());
        assert!(registry.get_by_path("c.util").is_some());
    }

    #[test]
    fn test_circular_import_detected() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();

        // A imports B, B imports A → cycle detected via dependency graph
        std::fs::write(src.join("a.sp"), "import b\npub fn fa() -> Int { 1 }").unwrap();
        std::fs::write(src.join("b.sp"), "import a\npub fn fb() -> Int { 2 }").unwrap();

        let mut loader = ModuleLoader::new(dir.path().to_path_buf());
        let mut registry = ModuleRegistry::new();

        let imports = vec![ImportDecl::Import {
            path: "a".into(),
            alias: "a".into(),
            span: None,
        }];

        let result = registry.resolve_imports(&mut loader, "entry", &imports);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(
            errs.iter()
                .any(|e| matches!(e, ModuleError::CircularDependency(_))),
            "expected circular dependency error, got: {errs:?}"
        );
    }

    #[test]
    fn test_nonexistent_module_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();

        let mut loader = ModuleLoader::new(dir.path().to_path_buf());
        let result = loader.load_module("does.not.exist");
        assert!(matches!(result, Err(ModuleError::ModuleNotFound(_))));
    }

    #[test]
    fn test_private_symbol_not_exported_via_loader() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("lib.sp"),
            "pub fn public_fn() -> Int { 1 }\nfn private_fn() -> Int { 2 }",
        )
        .unwrap();

        let mut loader = ModuleLoader::new(dir.path().to_path_buf());
        let iface = loader.load_module("lib").unwrap();

        assert!(iface.exports("public_fn"));
        assert_eq!(*iface.visibility("public_fn"), SymbolVisibility::Pub);
        assert!(iface.exports("private_fn"));
        assert_eq!(*iface.visibility("private_fn"), SymbolVisibility::Private);
    }
}
