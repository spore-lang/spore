//! Module system — resolution, exports, and import validation.

mod interface;
mod loader;
mod prelude;
mod registry;

use sporec_parser::{
    ast::{Span, Visibility},
    error::ParseError,
};

pub use interface::ModuleInterface;
pub use loader::ModuleLoader;
pub use registry::ModuleRegistry;

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

/// Options for how the synthetic prelude should be assembled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreludeOptions {
    pub include_console: bool,
}

impl Default for PreludeOptions {
    fn default() -> Self {
        Self {
            include_console: true,
        }
    }
}

/// The kind of an imported symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedSymbol {
    Function,
    Type,
    Struct,
    Handler,
    Interface,
}

/// Module resolution errors.
#[derive(Debug, Clone)]
pub enum ModuleError {
    ModuleNotFound(String),
    SymbolNotFound {
        module: String,
        symbol: String,
    },
    PrivateSymbol {
        module: String,
        symbol: String,
    },
    CircularDependency(Vec<String>),
    IoError {
        module: String,
        detail: String,
    },
    ParseErrors {
        module: String,
        errors: Vec<ParseError>,
    },
}

/// An import-resolution failure annotated with the import site that triggered it.
#[derive(Debug, Clone)]
pub struct ImportResolutionError {
    pub importing_module: String,
    pub imported_module: String,
    pub import_span: Option<Span>,
    pub error: ModuleError,
}

impl ImportResolutionError {
    pub fn new(
        importing_module: impl Into<String>,
        imported_module: impl Into<String>,
        import_span: Option<Span>,
        error: ModuleError,
    ) -> Self {
        Self {
            importing_module: importing_module.into(),
            imported_module: imported_module.into(),
            import_span,
            error,
        }
    }
}

impl std::fmt::Display for ImportResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
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
            ModuleError::ParseErrors { module, errors } => {
                write!(
                    f,
                    "parse error in module `{module}`: {}",
                    errors
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Ty;
    use sporec_parser::ast::ImportDecl;

    #[test]
    fn register_and_lookup_module() {
        let mut reg = ModuleRegistry::new();
        let mut m = ModuleInterface::new(vec!["Math".into()]);
        m.functions.insert("sqrt".into(), (vec![Ty::F64], Ty::F64));
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
    fn prelude_registration_can_skip_console_builtins() {
        let mut reg = ModuleRegistry::new();
        reg.register_prelude_with_options(PreludeOptions {
            include_console: false,
        });
        let prelude = reg
            .get(&["Std".into(), "Prelude".into()])
            .expect("prelude should be registered");
        assert!(!prelude.exports("print"));
        assert!(!prelude.exports("println"));
        assert!(!prelude.exports("read_line"));
        assert!(prelude.exports("char_to_int"));
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

        let iface2 = loader.load_module("utils.math").unwrap();
        assert!(iface2.exports("add"));
    }

    #[test]
    fn test_load_module_from_custom_source_root() {
        let dir = tempfile::tempdir().unwrap();
        let host = dir.path().join("host").join("utils");
        std::fs::create_dir_all(&host).unwrap();
        std::fs::write(
            host.join("math.sp"),
            "pub fn add(a: Int, b: Int) -> Int { a + b }",
        )
        .unwrap();

        let mut loader = ModuleLoader::with_source_roots(
            dir.path().to_path_buf(),
            vec![dir.path().join("host")],
            Vec::new(),
        );
        let iface = loader.load_module("utils.math").unwrap();
        assert!(iface.exports("add"));
        assert_eq!(iface.qualified_name(), "utils.math");
    }

    #[test]
    fn test_import_resolution_chain() {
        let dir = tempfile::tempdir().unwrap();
        let src_b = dir.path().join("src").join("b");
        let src_c = dir.path().join("src").join("c");
        std::fs::create_dir_all(&src_b).unwrap();
        std::fs::create_dir_all(&src_c).unwrap();

        std::fs::write(src_c.join("util.sp"), "pub fn helper() -> Int { 1 }").unwrap();
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

        assert!(registry.get_by_path("b.core").is_some());
        assert!(registry.get_by_path("c.util").is_some());
    }

    #[test]
    fn test_circular_import_detected() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();

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
                .any(|e| matches!(e.error, ModuleError::CircularDependency(_))),
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
