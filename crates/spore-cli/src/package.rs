//! Package management — manifest, dependencies, and registry.
//!
//! Spore packages are declared in `spore.toml` with content-addressed
//! dependencies using BLAKE3 hashes for reproducible builds.

use std::collections::HashMap;
use std::path::PathBuf;

/// A Spore package manifest (spore.toml).
#[derive(Debug, Clone)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub license: Option<String>,
    /// Minimum Spore compiler version.
    pub spore_version: Option<String>,
    /// Dependencies: name → dependency spec
    pub dependencies: HashMap<String, DependencySpec>,
    /// Platform configurations
    pub platforms: Vec<PlatformConfig>,
    /// Package-level capability grants
    pub capabilities: Vec<String>,
}

impl PackageManifest {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            description: None,
            authors: Vec::new(),
            license: None,
            spore_version: None,
            dependencies: HashMap::new(),
            platforms: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    /// Parse a TOML string into a manifest (simplified).
    pub fn from_toml(toml_str: &str) -> Result<Self, PackageError> {
        let mut manifest = PackageManifest::new("unknown", "0.0.0");

        for line in toml_str.lines() {
            let line = line.trim();
            if line.starts_with("name") {
                if let Some(val) = extract_string_value(line) {
                    manifest.name = val;
                }
            } else if line.starts_with("version") {
                if let Some(val) = extract_string_value(line) {
                    manifest.version = val;
                }
            } else if line.starts_with("description") {
                manifest.description = extract_string_value(line);
            } else if line.starts_with("license") {
                manifest.license = extract_string_value(line);
            }
        }

        if manifest.name == "unknown" {
            return Err(PackageError::InvalidManifest(
                "missing `name` field".into(),
            ));
        }

        Ok(manifest)
    }
}

/// Extract a string value from a TOML-like line: `key = "value"`.
fn extract_string_value(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.splitn(2, '=').collect();
    if parts.len() == 2 {
        let val = parts[1].trim().trim_matches('"');
        Some(val.to_string())
    } else {
        None
    }
}

/// A dependency specification.
#[derive(Debug, Clone)]
pub enum DependencySpec {
    /// Registry dependency with version constraint.
    Registry { version: String },
    /// Git dependency.
    Git { url: String, rev: Option<String> },
    /// Local path dependency.
    Path { path: PathBuf },
}

impl DependencySpec {
    pub fn registry(version: &str) -> Self {
        DependencySpec::Registry {
            version: version.to_string(),
        }
    }

    pub fn path(path: impl Into<PathBuf>) -> Self {
        DependencySpec::Path { path: path.into() }
    }

    pub fn git(url: &str) -> Self {
        DependencySpec::Git {
            url: url.to_string(),
            rev: None,
        }
    }
}

/// Platform configuration.
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub name: String,
    /// Capabilities the platform provides.
    pub provides: Vec<String>,
    /// Entry point function.
    pub entry: Option<String>,
}

/// Resolved dependency graph.
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    /// Package name → resolved version + dependencies
    pub packages: HashMap<String, ResolvedPackage>,
}

#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub name: String,
    pub version: String,
    /// Content hash for reproducibility.
    pub hash: Option<String>,
    /// Direct dependencies.
    pub dependencies: Vec<String>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_package(&mut self, pkg: ResolvedPackage) {
        self.packages.insert(pkg.name.clone(), pkg);
    }

    /// Get a topological ordering of packages (dependencies first).
    pub fn build_order(&self) -> Vec<String> {
        let mut visited = std::collections::HashSet::new();
        let mut order = Vec::new();

        // Sort keys for deterministic output
        let mut keys: Vec<&String> = self.packages.keys().collect();
        keys.sort();

        for name in keys {
            self.visit(name, &mut visited, &mut order);
        }

        order
    }

    fn visit(
        &self,
        name: &str,
        visited: &mut std::collections::HashSet<String>,
        order: &mut Vec<String>,
    ) {
        if visited.contains(name) {
            return;
        }
        visited.insert(name.to_string());

        if let Some(pkg) = self.packages.get(name) {
            for dep in &pkg.dependencies {
                self.visit(dep, visited, order);
            }
        }

        order.push(name.to_string());
    }

    /// Total number of packages.
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

/// Package errors.
#[derive(Debug, Clone)]
pub enum PackageError {
    InvalidManifest(String),
    DependencyNotFound(String),
    VersionConflict {
        package: String,
        required: String,
        available: String,
    },
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageError::InvalidManifest(msg) => write!(f, "invalid manifest: {msg}"),
            PackageError::DependencyNotFound(name) => {
                write!(f, "dependency `{name}` not found")
            }
            PackageError::VersionConflict {
                package,
                required,
                available,
            } => {
                write!(
                    f,
                    "version conflict for `{package}`: need {required}, have {available}"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_creation() {
        let m = PackageManifest::new("my-app", "1.0.0");
        assert_eq!(m.name, "my-app");
        assert_eq!(m.version, "1.0.0");
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn manifest_from_toml() {
        let toml = r#"
            name = "hello"
            version = "0.1.0"
            description = "A hello world app"
        "#;
        let m = PackageManifest::from_toml(toml).unwrap();
        assert_eq!(m.name, "hello");
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.description, Some("A hello world app".into()));
    }

    #[test]
    fn manifest_missing_name() {
        let toml = "version = \"1.0.0\"";
        assert!(PackageManifest::from_toml(toml).is_err());
    }

    #[test]
    fn dependency_graph_build_order() {
        let mut graph = DependencyGraph::new();
        graph.add_package(ResolvedPackage {
            name: "app".into(),
            version: "1.0.0".into(),
            hash: None,
            dependencies: vec!["lib-a".into(), "lib-b".into()],
        });
        graph.add_package(ResolvedPackage {
            name: "lib-a".into(),
            version: "0.5.0".into(),
            hash: None,
            dependencies: vec!["lib-b".into()],
        });
        graph.add_package(ResolvedPackage {
            name: "lib-b".into(),
            version: "0.3.0".into(),
            hash: None,
            dependencies: vec![],
        });

        let order = graph.build_order();
        let b_pos = order.iter().position(|x| x == "lib-b").unwrap();
        let a_pos = order.iter().position(|x| x == "lib-a").unwrap();
        let app_pos = order.iter().position(|x| x == "app").unwrap();
        assert!(b_pos < a_pos);
        assert!(a_pos < app_pos);
    }

    #[test]
    fn dependency_spec_variants() {
        let reg = DependencySpec::registry("^1.0.0");
        assert!(matches!(reg, DependencySpec::Registry { .. }));

        let path = DependencySpec::path("../local-lib");
        assert!(matches!(path, DependencySpec::Path { .. }));

        let git = DependencySpec::git("https://github.com/example/lib");
        assert!(matches!(git, DependencySpec::Git { .. }));
    }
}
