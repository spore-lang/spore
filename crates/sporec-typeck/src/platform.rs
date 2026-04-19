//! Platform system — capability grants and startup contract validation.
//!
//! Platforms define the runtime environment for Spore programs.
//! They grant capabilities and define the startup contract for the selected
//! entry module.

use std::collections::HashMap;

use crate::capability::CapabilitySet;
use crate::module::ModuleInterface;

/// A platform definition.
#[derive(Debug, Clone)]
pub struct Platform {
    /// Platform name (e.g., "cli", "web", "embedded").
    pub name: String,
    /// Capabilities this platform provides.
    pub capabilities: CapabilitySet,
    /// Required startup function name inside the selected entry module.
    pub startup_function: String,
    /// Expected parameter types for the startup function.
    pub startup_params: Vec<String>,
    /// Expected return type of the startup function.
    pub startup_return: String,
    /// Platform-specific configuration.
    pub config: PlatformConfig,
}

/// Platform-specific configuration.
#[derive(Debug, Clone, Default)]
pub struct PlatformConfig {
    /// Maximum concurrency level (None = unlimited).
    pub max_concurrency: Option<u32>,
    /// Whether the platform supports async.
    pub async_support: bool,
    /// Runtime memory limit in bytes (None = unlimited).
    pub memory_limit: Option<u64>,
}

impl Platform {
    /// Create a CLI platform (most common).
    ///
    /// Grants the built-in intent-oriented effects for full CLI access.
    pub fn cli() -> Self {
        let capabilities = CapabilitySet::from_names([
            // ── Built-in effects ──
            "Console".into(),
            "FileRead".into(),
            "FileWrite".into(),
            "NetConnect".into(),
            "NetListen".into(),
            "Env".into(),
            "Spawn".into(),
            "Clock".into(),
            "Random".into(),
            "Exit".into(),
        ]);

        Self {
            name: "cli".into(),
            capabilities,
            startup_function: "main".into(),
            startup_params: vec![],
            startup_return: "()".into(),
            config: PlatformConfig {
                max_concurrency: None,
                async_support: true,
                memory_limit: None,
            },
        }
    }

    /// Create a web/WASI platform.
    ///
    /// Grants a subset of built-in effects appropriate for sandboxed web
    /// environments.
    pub fn web() -> Self {
        let capabilities = CapabilitySet::from_names([
            // ── Built-in effects ──
            "Console".into(),
            "NetConnect".into(),
            "Random".into(),
            "Clock".into(),
        ]);

        Self {
            name: "web".into(),
            capabilities,
            startup_function: "main".into(),
            startup_params: vec![],
            startup_return: "()".into(),
            config: PlatformConfig {
                max_concurrency: Some(1),
                async_support: true,
                memory_limit: None,
            },
        }
    }

    /// Create a minimal/embedded platform.
    pub fn embedded() -> Self {
        Self {
            name: "embedded".into(),
            capabilities: CapabilitySet::new(),
            startup_function: "main".into(),
            startup_params: vec![],
            startup_return: "()".into(),
            config: PlatformConfig {
                max_concurrency: Some(1),
                async_support: false,
                memory_limit: Some(64 * 1024),
            },
        }
    }

    /// Check if this platform grants a specific capability.
    pub fn grants(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }

    /// Validate that a function's required capabilities are granted by this platform.
    pub fn validate_capabilities(&self, required: &CapabilitySet) -> Result<(), Vec<String>> {
        let missing = self.capabilities.missing_from(required);
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }

    /// Validate the startup function against the platform startup contract.
    pub fn validate_startup_function(
        &self,
        startup_name: &str,
        param_count: usize,
        return_type: &str,
    ) -> Vec<PlatformWarning> {
        let mut warnings = Vec::new();

        if startup_name != self.startup_function {
            warnings.push(PlatformWarning {
                kind: PlatformWarningKind::MissingStartupFunction,
                message: format!(
                    "platform `{}` expects startup function `{}`, but it was not found",
                    self.name, self.startup_function
                ),
            });
        }

        if param_count != self.startup_params.len() {
            warnings.push(PlatformWarning {
                kind: PlatformWarningKind::WrongStartupSignature,
                message: format!(
                    "startup function `{}` should take {} parameters, takes {}",
                    self.startup_function,
                    self.startup_params.len(),
                    param_count
                ),
            });
        }

        if return_type != self.startup_return {
            warnings.push(PlatformWarning {
                kind: PlatformWarningKind::WrongStartupSignature,
                message: format!(
                    "startup function `{}` should return `{}`, returns `{}`",
                    self.startup_function, self.startup_return, return_type
                ),
            });
        }

        warnings
    }

    /// Validate the selected entry module against the platform startup contract.
    pub fn validate_entry_startup(
        &self,
        entry_iface: &ModuleInterface,
    ) -> Result<(), PlatformStartupError> {
        let module_name = entry_iface.qualified_name();
        let Some((params, ret_ty)) = entry_iface.functions.get(&self.startup_function) else {
            return Err(PlatformStartupError {
                kind: PlatformStartupErrorKind::MissingStartupFunction,
                message: format!(
                    "entry module `{module_name}` does not define required startup function `{}` for platform `{}`",
                    self.startup_function, self.name
                ),
            });
        };

        let actual_params: Vec<String> = params.iter().map(ToString::to_string).collect();
        if actual_params != self.startup_params {
            return Err(PlatformStartupError {
                kind: PlatformStartupErrorKind::WrongStartupSignature,
                message: format!(
                    "startup function `{}` in entry module `{module_name}` should take ({}) for platform `{}`, found ({})",
                    self.startup_function,
                    self.startup_params.join(", "),
                    self.name,
                    actual_params.join(", ")
                ),
            });
        }

        let actual_return = ret_ty.to_string();
        if actual_return != self.startup_return {
            return Err(PlatformStartupError {
                kind: PlatformStartupErrorKind::WrongStartupSignature,
                message: format!(
                    "startup function `{}` in entry module `{module_name}` should return `{}` for platform `{}`, found `{}`",
                    self.startup_function, self.startup_return, self.name, actual_return
                ),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformStartupError {
    pub kind: PlatformStartupErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformStartupErrorKind {
    MissingStartupFunction,
    WrongStartupSignature,
    UnsupportedCapability,
    InvalidPlatformContract,
}

/// Platform-related warnings.
#[derive(Debug, Clone)]
pub struct PlatformWarning {
    pub kind: PlatformWarningKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformWarningKind {
    MissingStartupFunction,
    WrongStartupSignature,
    UnsupportedCapability,
}

/// Registry of available platforms.
#[derive(Debug, Clone, Default)]
pub struct PlatformRegistry {
    platforms: HashMap<String, Platform>,
}

impl PlatformRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry with built-in platforms.
    pub fn with_builtins() -> Self {
        let mut reg = Self::new();
        reg.register(Platform::cli());
        reg.register(Platform::web());
        reg.register(Platform::embedded());
        reg
    }

    pub fn register(&mut self, platform: Platform) {
        self.platforms.insert(platform.name.clone(), platform);
    }

    pub fn get(&self, name: &str) -> Option<&Platform> {
        self.platforms.get(name)
    }

    pub fn all_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.platforms.keys().cloned().collect();
        names.sort();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_platform_has_capabilities() {
        let p = Platform::cli();
        // Built-in effects
        assert!(p.grants("Console"));
        assert!(p.grants("FileRead"));
        assert!(p.grants("FileWrite"));
        assert!(p.grants("NetConnect"));
        assert!(p.grants("NetListen"));
        assert!(p.grants("Env"));
        assert!(p.grants("Spawn"));
        assert!(p.grants("Clock"));
        assert!(p.grants("Random"));
        assert!(p.grants("Exit"));
        // Not granted
        assert!(!p.grants("Gpu"));
    }

    #[test]
    fn embedded_platform_has_no_capabilities() {
        let p = Platform::embedded();
        assert!(!p.grants("Console"));
        assert!(!p.grants("FileRead"));
    }

    #[test]
    fn validate_capabilities_ok() {
        let p = Platform::cli();
        let required = CapabilitySet::from_names(["Console".into(), "FileRead".into()]);
        assert!(p.validate_capabilities(&required).is_ok());
    }

    #[test]
    fn validate_capabilities_missing() {
        let p = Platform::embedded();
        let required = CapabilitySet::from_names(["NetConnect".into()]);
        let result = p.validate_capabilities(&required);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(&"NetConnect".to_string()));
    }

    #[test]
    fn startup_function_validation() {
        let p = Platform::cli();
        let warnings = p.validate_startup_function("main", 0, "()");
        assert!(warnings.is_empty());

        let warnings = p.validate_startup_function("main", 0, "Str");
        assert!(
            warnings
                .iter()
                .any(|w| w.kind == PlatformWarningKind::WrongStartupSignature)
        );
    }

    #[test]
    fn entry_startup_validation_rejects_missing_function() {
        let platform = Platform::cli();
        let entry = ModuleInterface::new(vec!["app".into()]);

        let err = platform
            .validate_entry_startup(&entry)
            .expect_err("missing startup should fail");
        assert_eq!(err.kind, PlatformStartupErrorKind::MissingStartupFunction);
        assert!(err.message.contains("required startup function `main`"));
    }

    #[test]
    fn entry_startup_validation_rejects_wrong_return_type() {
        let platform = Platform::cli();
        let mut entry = ModuleInterface::new(vec!["app".into()]);
        entry
            .functions
            .insert("main".into(), (vec![], crate::types::Ty::I32));

        let err = platform
            .validate_entry_startup(&entry)
            .expect_err("wrong return type should fail");
        assert_eq!(err.kind, PlatformStartupErrorKind::WrongStartupSignature);
        assert!(err.message.contains("should return `()`"));
    }

    #[test]
    fn entry_startup_validation_accepts_matching_signature() {
        let platform = Platform::cli();
        let mut entry = ModuleInterface::new(vec!["app".into()]);
        entry
            .functions
            .insert("main".into(), (vec![], crate::types::Ty::Unit));

        platform
            .validate_entry_startup(&entry)
            .expect("matching startup signature should pass");
    }

    #[test]
    fn platform_registry() {
        let reg = PlatformRegistry::with_builtins();
        assert!(reg.get("cli").is_some());
        assert!(reg.get("web").is_some());
        assert!(reg.get("embedded").is_some());
        assert!(reg.get("nonexistent").is_none());
        assert_eq!(reg.all_names().len(), 3);
    }

    #[test]
    fn web_platform_limited_concurrency() {
        let p = Platform::web();
        assert_eq!(p.config.max_concurrency, Some(1));
        assert!(p.config.async_support);
    }
}
