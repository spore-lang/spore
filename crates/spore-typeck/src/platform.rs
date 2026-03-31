//! Platform system — capability grants and entry point validation.
//!
//! Platforms define the runtime environment for Spore programs.
//! They grant capabilities and define the entry point signature.

use std::collections::HashMap;

use crate::capability::CapabilitySet;

/// A platform definition.
#[derive(Debug, Clone)]
pub struct Platform {
    /// Platform name (e.g., "cli", "web", "embedded").
    pub name: String,
    /// Capabilities this platform provides.
    pub capabilities: CapabilitySet,
    /// Required entry point function name.
    pub entry_point: String,
    /// Expected signature of the entry point.
    pub entry_params: Vec<String>,
    /// Expected return type of the entry point.
    pub entry_return: String,
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
    pub fn cli() -> Self {
        let capabilities = CapabilitySet::from_names([
            "Console".into(),
            "FileRead".into(),
            "FileWrite".into(),
            "NetRead".into(),
            "NetWrite".into(),
            "Env".into(),
            "Process".into(),
        ]);

        Self {
            name: "cli".into(),
            capabilities,
            entry_point: "main".into(),
            entry_params: vec![],
            entry_return: "Int".into(),
            config: PlatformConfig {
                max_concurrency: None,
                async_support: true,
                memory_limit: None,
            },
        }
    }

    /// Create a web/WASI platform.
    pub fn web() -> Self {
        let capabilities =
            CapabilitySet::from_names(["Console".into(), "NetRead".into(), "NetWrite".into()]);

        Self {
            name: "web".into(),
            capabilities,
            entry_point: "main".into(),
            entry_params: vec![],
            entry_return: "Unit".into(),
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
            entry_point: "main".into(),
            entry_params: vec![],
            entry_return: "Unit".into(),
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

    /// Validate entry point signature.
    pub fn validate_entry_point(
        &self,
        fn_name: &str,
        param_count: usize,
        return_type: &str,
    ) -> Vec<PlatformWarning> {
        let mut warnings = Vec::new();

        if fn_name != self.entry_point {
            warnings.push(PlatformWarning {
                kind: PlatformWarningKind::MissingEntryPoint,
                message: format!(
                    "platform `{}` expects entry point `{}`, but it was not found",
                    self.name, self.entry_point
                ),
            });
        }

        if param_count != self.entry_params.len() {
            warnings.push(PlatformWarning {
                kind: PlatformWarningKind::WrongEntrySignature,
                message: format!(
                    "entry point `{}` should take {} parameters, takes {}",
                    self.entry_point,
                    self.entry_params.len(),
                    param_count
                ),
            });
        }

        if return_type != self.entry_return {
            warnings.push(PlatformWarning {
                kind: PlatformWarningKind::WrongEntrySignature,
                message: format!(
                    "entry point `{}` should return `{}`, returns `{}`",
                    self.entry_point, self.entry_return, return_type
                ),
            });
        }

        warnings
    }
}

/// Platform-related warnings.
#[derive(Debug, Clone)]
pub struct PlatformWarning {
    pub kind: PlatformWarningKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformWarningKind {
    MissingEntryPoint,
    WrongEntrySignature,
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
        assert!(p.grants("Console"));
        assert!(p.grants("FileRead"));
        assert!(p.grants("NetRead"));
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
        let required = CapabilitySet::from_names(["NetRead".into()]);
        let result = p.validate_capabilities(&required);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(&"NetRead".to_string()));
    }

    #[test]
    fn entry_point_validation() {
        let p = Platform::cli();
        let warnings = p.validate_entry_point("main", 0, "Int");
        assert!(warnings.is_empty());

        let warnings = p.validate_entry_point("main", 0, "String");
        assert!(
            warnings
                .iter()
                .any(|w| w.kind == PlatformWarningKind::WrongEntrySignature)
        );
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
