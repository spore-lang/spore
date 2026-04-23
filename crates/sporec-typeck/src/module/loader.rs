use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sporec_parser::ast::Module as AstModule;

use super::{ModuleError, ModuleInterface};

/// Resolves module paths to filesystem paths and loads module interfaces.
///
/// The loader caches both parsed ASTs and extracted interfaces so that
/// each module is read from disk at most once.
pub struct ModuleLoader {
    /// Project root directory.
    root: PathBuf,
    /// Additional dependency roots searched after the project root.
    dependency_roots: Vec<PathBuf>,
    /// Cache of already-loaded module interfaces.
    loaded: HashMap<String, ModuleInterface>,
    /// Cache of parsed ASTs (needed for transitive import extraction and interpreter).
    asts: HashMap<String, AstModule>,
    /// Cache of raw source text for loaded modules, including parse failures.
    sources: HashMap<String, String>,
}

impl ModuleLoader {
    pub fn new(root: PathBuf) -> Self {
        Self::with_dependency_roots(root, Vec::new())
    }

    pub fn with_dependency_roots(root: PathBuf, dependency_roots: Vec<PathBuf>) -> Self {
        Self {
            root,
            dependency_roots,
            loaded: HashMap::new(),
            asts: HashMap::new(),
            sources: HashMap::new(),
        }
    }

    /// Resolve a dot-separated module path to a filesystem path.
    ///
    /// `"billing.invoice"` → `{root}/src/billing/invoice.sp`
    pub fn resolve_path(&self, module_path: &str) -> Option<PathBuf> {
        let rel = module_path.replace('.', "/");
        for root in std::iter::once(&self.root).chain(self.dependency_roots.iter()) {
            let path = root.join("src").join(&rel).with_extension("sp");
            if path.exists() {
                return Some(path);
            }
        }
        None
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
        self.sources.insert(module_path.to_string(), source.clone());

        let ast = sporec_parser::parse(&source).map_err(|errs| ModuleError::ParseErrors {
            module: module_path.to_string(),
            errors: errs,
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

    /// Get cached source text for a previously loaded module.
    pub fn get_source(&self, module_path: &str) -> Option<&str> {
        self.sources.get(module_path).map(String::as_str)
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
