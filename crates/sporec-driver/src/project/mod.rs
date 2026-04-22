use std::collections::BTreeMap;
use std::path::PathBuf;

mod resolve;
mod toml_parse;

pub use resolve::{resolve_default_project_target, resolve_project_target_by_path};
pub use toml_parse::load_project_manifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectManifest {
    pub package_name: Option<String>,
    pub package_type: Option<String>,
    pub project: Option<ProjectConfig>,
    pub platform: Option<PlatformManifest>,
    pub dependencies: BTreeMap<String, DependencySpec>,
    pub entries: BTreeMap<String, ProjectEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub platform: String,
    pub default_entry: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEntry {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySpec {
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformManifest {
    pub contract_module: String,
    pub startup_contract: String,
    pub adapter_function: String,
    pub handled_effects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPlatformContract {
    pub name: String,
    pub root: PathBuf,
    pub contract_module: String,
    pub startup_function: String,
    pub adapter_function: String,
    pub handled_effects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProjectTarget {
    pub entry_name: String,
    pub entry_path: String,
    pub platform_name: Option<String>,
    pub startup_function: Option<String>,
    pub platform_contract: Option<ResolvedPlatformContract>,
    pub dependency_roots: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempProject {
        root: PathBuf,
    }

    impl TempProject {
        fn new(name: &str, manifest: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "spore-project-{name}-{unique}-{}",
                std::process::id()
            ));
            fs::create_dir_all(root.join("src")).expect("temp project src dir");
            fs::write(root.join("spore.toml"), manifest).expect("temp project manifest");
            Self { root }
        }

        fn root(&self) -> &Path {
            &self.root
        }

        fn write(&self, rel: &str, content: &str) {
            let path = self.root.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent dirs");
            }
            fs::write(path, content).expect("write project file");
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn resolve_default_target_from_project_section() {
        let project = TempProject::new(
            "project-section",
            r#"
            [package]
            name = "demo"
            version = "0.1.0"
            type = "application"

            [project]
            platform = "cli"
            default-entry = "app"

            [entries.app]
            path = "main.sp"
            "#,
        );
        project.write("src/main.sp", "fn main() -> () { return }\n");

        let target = resolve_default_project_target(project.root()).expect("resolved target");
        assert_eq!(target.entry_name, "app");
        assert_eq!(target.entry_path, "main.sp");
        assert_eq!(target.platform_name.as_deref(), Some("cli"));
        assert_eq!(target.startup_function.as_deref(), Some("main"));
        assert!(target.platform_contract.is_none());
        assert!(target.dependency_roots.is_empty());
    }

    #[test]
    fn resolve_default_target_from_path_dependency_platform_contract() {
        let project = TempProject::new(
            "path-platform",
            r#"
            [package]
            name = "demo"
            type = "application"

            [project]
            platform = "basic-cli"
            default-entry = "app"

            [dependencies]
            basic-cli = { path = "vendor/basic-cli" }

            [entries.app]
            path = "app.sp"
            "#,
        );
        project.write("src/app.sp", "fn main() -> () { return }\n");
        project.write(
            "vendor/basic-cli/spore.toml",
            r#"
            [package]
            name = "basic-cli"
            type = "platform"

            [platform]
            contract-module = "platform_contract"
            startup-contract = "main"
            adapter-function = "main_for_host"
            handled-effects = ["Console", "Env"]
            "#,
        );
        project.write(
            "vendor/basic-cli/src/platform_contract.sp",
            r#"
            pub fn main() -> () {
                ?platform_startup_contract
            }

            pub fn main_for_host(app_main: () -> ()) -> () {
                app_main();
                return
            }
            "#,
        );

        let target = resolve_default_project_target(project.root()).expect("resolved target");
        assert_eq!(target.entry_name, "app");
        assert_eq!(target.entry_path, "app.sp");
        assert_eq!(target.platform_name.as_deref(), Some("basic-cli"));
        assert_eq!(target.startup_function.as_deref(), Some("main"));
        let contract = target
            .platform_contract
            .expect("expected resolved platform package contract");
        assert_eq!(contract.contract_module, "platform_contract");
        assert_eq!(contract.adapter_function, "main_for_host");
        assert_eq!(
            contract.handled_effects,
            vec!["Console".to_string(), "Env".to_string()]
        );
        assert_eq!(
            target.dependency_roots,
            vec![
                project
                    .root()
                    .join("vendor/basic-cli")
                    .canonicalize()
                    .expect("canonical dependency root")
            ]
        );
    }

    #[test]
    fn resolve_project_target_by_path_allows_non_entry_modules() {
        let project = TempProject::new(
            "undeclared-entry",
            r#"
            [package]
            name = "demo"

            [project]
            platform = "cli"
            default-entry = "app"

            [entries.app]
            path = "main.sp"
            "#,
        );
        project.write("src/main.sp", "fn main() -> () { return }\n");
        project.write("src/tools/repl.sp", "fn main() -> () { return }\n");

        let target = resolve_project_target_by_path(project.root(), "tools/repl.sp")
            .expect("non-entry module should still resolve for build/check flows");
        assert_eq!(target.entry_name, "repl");
        assert_eq!(target.entry_path, "tools/repl.sp");
        assert!(target.platform_name.is_none());
        assert!(target.startup_function.is_none());
        assert!(target.platform_contract.is_none());
        assert!(target.dependency_roots.is_empty());
    }

    #[test]
    fn resolve_project_target_by_path_normalizes_declared_entry_paths() {
        let project = TempProject::new(
            "normalized-entry-path",
            r#"
            [package]
            name = "demo"

            [project]
            platform = "cli"
            default-entry = "tool"

            [entries.tool]
            path = ".\tools\tool.sp"
            "#,
        );
        project.write("src/tools/tool.sp", "fn main() -> () { return }\n");

        let target = resolve_project_target_by_path(project.root(), "tools/tool.sp")
            .expect("normalized declared entry should resolve");
        assert_eq!(target.entry_name, "tool");
        assert_eq!(target.entry_path, "tools/tool.sp");
        assert_eq!(target.platform_name.as_deref(), Some("cli"));
        assert_eq!(target.startup_function.as_deref(), Some("main"));
        assert!(target.platform_contract.is_none());
        assert!(target.dependency_roots.is_empty());
    }

    #[test]
    fn resolve_project_target_by_path_preserves_hash_in_declared_entry_path() {
        let project = TempProject::new(
            "hash-entry-path",
            r#"
            [package]
            name = "demo"

            [project]
            platform = "cli"
            default-entry = "hash"

            [entries.hash]
            path = "tools/#cli.sp" # keep the # inside the quoted path
            "#,
        );
        project.write("src/tools/#cli.sp", "fn main() -> () { return }\n");

        let target = resolve_project_target_by_path(project.root(), "tools/#cli.sp")
            .expect("quoted # in entry path should parse correctly");
        assert_eq!(target.entry_name, "hash");
        assert_eq!(target.entry_path, "tools/#cli.sp");
        assert_eq!(target.platform_name.as_deref(), Some("cli"));
        assert_eq!(target.startup_function.as_deref(), Some("main"));
        assert!(target.platform_contract.is_none());
        assert!(target.dependency_roots.is_empty());
    }

    #[test]
    fn resolve_default_target_legacy_package_type_application() {
        let project = TempProject::new(
            "legacy-app",
            r#"
            [package]
            name = "demo"
            type = "application"
            "#,
        );
        project.write("src/main.sp", "fn main() -> () { return }\n");

        let target = resolve_default_project_target(project.root()).expect("legacy app target");
        assert_eq!(target.entry_path, "main.sp");
        assert_eq!(target.platform_name.as_deref(), Some("cli"));
        assert_eq!(target.startup_function.as_deref(), Some("main"));
        assert!(target.platform_contract.is_none());
        assert!(target.dependency_roots.is_empty());
    }

    #[test]
    fn resolve_default_target_legacy_application_collects_transitive_dependency_roots() {
        let project = TempProject::new(
            "legacy-app-transitive-deps",
            r#"
            [package]
            name = "demo"
            type = "application"

            [dependencies]
            dep-a = { path = "vendor/dep-a" }
            "#,
        );
        project.write("src/main.sp", "fn main() -> () { return }\n");
        project.write(
            "vendor/dep-a/spore.toml",
            r#"
            [package]
            name = "dep-a"
            type = "package"

            [dependencies]
            dep-b = { path = "../dep-b" }
            "#,
        );
        project.write(
            "vendor/dep-b/spore.toml",
            r#"
            [package]
            name = "dep-b"
            type = "package"
            "#,
        );

        let target = resolve_default_project_target(project.root()).expect("legacy app target");
        assert_eq!(target.entry_path, "main.sp");
        assert_eq!(target.platform_name.as_deref(), Some("cli"));
        assert_eq!(target.startup_function.as_deref(), Some("main"));
        assert!(target.platform_contract.is_none());
        assert_eq!(
            target.dependency_roots,
            vec![
                project
                    .root()
                    .join("vendor/dep-a")
                    .canonicalize()
                    .expect("canonical dep-a root"),
                project
                    .root()
                    .join("vendor/dep-b")
                    .canonicalize()
                    .expect("canonical dep-b root")
            ]
        );
    }

    #[test]
    fn resolve_default_target_legacy_package_type_package_is_non_runnable() {
        let project = TempProject::new(
            "legacy-package",
            r#"
            [package]
            name = "demo"
            type = "package"
            "#,
        );
        project.write(
            "src/lib.sp",
            "pub fn add(a: I32, b: I32) -> I32 { a + b }\n",
        );

        let target = resolve_default_project_target(project.root()).expect("legacy package target");
        assert_eq!(target.entry_path, "lib.sp");
        assert!(target.platform_name.is_none());
        assert!(target.startup_function.is_none());
        assert!(target.platform_contract.is_none());
        assert!(target.dependency_roots.is_empty());
    }
}
