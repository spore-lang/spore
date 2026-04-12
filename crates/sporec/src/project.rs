use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use spore_typeck::platform::PlatformRegistry;

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
    pub handles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPlatformContract {
    pub name: String,
    pub root: PathBuf,
    pub contract_module: String,
    pub startup_function: String,
    pub adapter_function: String,
    pub handles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProjectTarget {
    pub entry_name: String,
    pub entry_path: String,
    pub platform_name: Option<String>,
    pub startup_function: Option<String>,
    pub platform_contract: Option<ResolvedPlatformContract>,
}

pub fn load_project_manifest(root: &Path) -> Result<ProjectManifest, String> {
    let manifest_path = root.join("spore.toml");
    let source = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("cannot read `{}`: {e}", manifest_path.display()))?;
    let mut package_name = None;
    let mut package_type = None;
    let mut project_platform = None;
    let mut project_default_entry = None;
    let mut platform_contract_module = None;
    let mut platform_startup_contract = None;
    let mut platform_adapter_function = None;
    let mut platform_handles = Vec::new();
    let mut dependencies = BTreeMap::new();
    let mut entries = BTreeMap::new();
    let mut current_section = Section::Other;
    let mut saw_project_section = false;
    let mut saw_platform_section = false;

    for raw_line in source.lines() {
        let stripped = strip_toml_comment(raw_line);
        let line = stripped.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            current_section = if section == "package" {
                Section::Package
            } else if section == "project" {
                saw_project_section = true;
                Section::Project
            } else if section == "platform" {
                saw_platform_section = true;
                Section::Platform
            } else if section == "dependencies" {
                Section::Dependencies
            } else if let Some(name) = section.strip_prefix("entries.") {
                entries
                    .entry(name.to_string())
                    .or_insert_with(|| ProjectEntry {
                        path: String::new(),
                    });
                Section::Entry(name.to_string())
            } else {
                Section::Other
            };
            continue;
        }

        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let raw_value = raw_value.trim();

        match &current_section {
            Section::Package if key == "name" => {
                let value = parse_toml_string(raw_value);
                if !value.is_empty() {
                    package_name = Some(value);
                }
            }
            Section::Package if key == "type" => {
                let value = parse_toml_string(raw_value);
                if !value.is_empty() {
                    package_type = Some(value);
                }
            }
            Section::Project if key == "platform" => {
                let value = parse_toml_string(raw_value);
                if !value.is_empty() {
                    project_platform = Some(value);
                }
            }
            Section::Project if key == "default-entry" => {
                let value = parse_toml_string(raw_value);
                if !value.is_empty() {
                    project_default_entry = Some(value);
                }
            }
            Section::Platform if key == "contract-module" => {
                let value = parse_toml_string(raw_value);
                if !value.is_empty() {
                    platform_contract_module = Some(value);
                }
            }
            Section::Platform if key == "startup-contract" => {
                let value = parse_toml_string(raw_value);
                if !value.is_empty() {
                    platform_startup_contract = Some(value);
                }
            }
            Section::Platform if key == "adapter-function" => {
                let value = parse_toml_string(raw_value);
                if !value.is_empty() {
                    platform_adapter_function = Some(value);
                }
            }
            Section::Platform if key == "handles" => {
                platform_handles = parse_toml_string_array(raw_value);
            }
            Section::Dependencies => {
                dependencies.insert(key.to_string(), parse_dependency_spec(raw_value));
            }
            Section::Entry(name) if key == "path" => {
                let value = parse_toml_string(raw_value);
                if let Some(entry) = entries.get_mut(name) {
                    entry.path = value;
                }
            }
            Section::Package
            | Section::Project
            | Section::Platform
            | Section::Entry(_)
            | Section::Other => {}
        }
    }

    let project = if saw_project_section {
        Some(ProjectConfig {
            platform: project_platform.unwrap_or_default(),
            default_entry: project_default_entry,
        })
    } else {
        None
    };
    let platform = if saw_platform_section {
        Some(PlatformManifest {
            contract_module: platform_contract_module.unwrap_or_default(),
            startup_contract: platform_startup_contract.unwrap_or_default(),
            adapter_function: platform_adapter_function.unwrap_or_default(),
            handles: platform_handles,
        })
    } else {
        None
    };

    Ok(ProjectManifest {
        package_name,
        package_type,
        project,
        platform,
        dependencies,
        entries,
    })
}

pub fn resolve_default_project_target(root: &Path) -> Result<ResolvedProjectTarget, String> {
    let manifest = load_project_manifest(root)?;

    if let Some(project) = &manifest.project {
        let default_entry = project.default_entry.as_deref().ok_or_else(|| {
            format!(
                "`{}` has `[project]` but no `default-entry`; pass an explicit entry file or declare one",
                root.join("spore.toml").display()
            )
        })?;
        return resolve_declared_entry(root, &manifest, default_entry);
    }

    legacy_default_target(root, &manifest)
}

pub fn resolve_project_target_by_path(
    root: &Path,
    entry_path: &str,
) -> Result<ResolvedProjectTarget, String> {
    let manifest = load_project_manifest(root)?;
    let normalized = normalize_entry_path(entry_path)?;

    if manifest.project.is_some() {
        let Some((entry_name, _)) = manifest.entries.iter().find(|(_, entry)| {
            normalize_entry_path(&entry.path)
                .map(|path| path == normalized)
                .unwrap_or(false)
        }) else {
            return module_only_target(root, &normalized);
        };
        return resolve_declared_entry(root, &manifest, entry_name);
    }

    legacy_target_for_path(root, &manifest, &normalized)
}

fn resolve_declared_entry(
    root: &Path,
    manifest: &ProjectManifest,
    entry_name: &str,
) -> Result<ResolvedProjectTarget, String> {
    let project = manifest.project.as_ref().ok_or_else(|| {
        format!(
            "`{}` is missing `[project]` configuration",
            root.join("spore.toml").display()
        )
    })?;
    let entry = manifest.entries.get(entry_name).ok_or_else(|| {
        format!(
            "`{}` declares default entry `{entry_name}` but no `[entries.{entry_name}]` table exists",
            root.join("spore.toml").display()
        )
    })?;
    let entry_path = normalize_entry_path(&entry.path)?;
    ensure_entry_exists(root, &entry_path)?;

    let (startup_function, platform_contract) =
        resolve_platform_binding(root, manifest, &project.platform)?;

    Ok(ResolvedProjectTarget {
        entry_name: entry_name.to_string(),
        entry_path,
        platform_name: Some(project.platform.clone()),
        startup_function: Some(startup_function),
        platform_contract,
    })
}

fn legacy_default_target(
    root: &Path,
    manifest: &ProjectManifest,
) -> Result<ResolvedProjectTarget, String> {
    match manifest.package_type.as_deref() {
        Some("application") => legacy_named_target(root, "app", "main.sp", true),
        Some("platform") => legacy_named_target(root, "host", "host.sp", true),
        Some("package") => legacy_named_target(root, "lib", "lib.sp", false),
        Some(other) => Err(format!(
            "unsupported legacy `[package].type = \"{other}\"` in `{}`",
            root.join("spore.toml").display()
        )),
        None => infer_single_default_target(root),
    }
}

fn legacy_target_for_path(
    root: &Path,
    manifest: &ProjectManifest,
    entry_path: &str,
) -> Result<ResolvedProjectTarget, String> {
    ensure_entry_exists(root, entry_path)?;

    match manifest.package_type.as_deref() {
        Some("application") if entry_path == "main.sp" => {
            legacy_named_target(root, "app", "main.sp", true)
        }
        Some("platform") if entry_path == "host.sp" => {
            legacy_named_target(root, "host", "host.sp", true)
        }
        None if entry_path == "main.sp" => legacy_named_target(root, "app", "main.sp", true),
        None if entry_path == "host.sp" => legacy_named_target(root, "host", "host.sp", true),
        Some("package") | Some("application") | Some("platform") | None => {
            Ok(ResolvedProjectTarget {
                entry_name: path_stem(entry_path),
                entry_path: entry_path.to_string(),
                platform_name: None,
                startup_function: None,
                platform_contract: None,
            })
        }
        Some(other) => Err(format!(
            "unsupported legacy `[package].type = \"{other}\"` in `{}`",
            root.join("spore.toml").display()
        )),
    }
}

fn infer_single_default_target(root: &Path) -> Result<ResolvedProjectTarget, String> {
    let mut candidates = Vec::new();
    for (entry_name, path, runnable) in [
        ("app", "main.sp", true),
        ("lib", "lib.sp", false),
        ("host", "host.sp", true),
    ] {
        if root.join("src").join(path).is_file() {
            candidates.push((entry_name, path, runnable));
        }
    }

    match candidates.as_slice() {
        [(entry_name, path, runnable)] => legacy_named_target(root, entry_name, path, *runnable),
        [] => Err(format!(
            "could not infer a project default entry path from `{}`; add `[project]` and `[entries]`, set legacy `[package].type`, or pass FILE explicitly",
            root.join("spore.toml").display()
        )),
        _ => Err(format!(
            "could not infer a project default entry path for `{}`; found multiple defaults in src/ ({}) — pass FILE explicitly or declare `[project].default-entry`",
            root.display(),
            candidates
                .iter()
                .map(|(_, path, _)| *path)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn legacy_named_target(
    root: &Path,
    entry_name: &str,
    entry_path: &str,
    runnable: bool,
) -> Result<ResolvedProjectTarget, String> {
    ensure_entry_exists(root, entry_path)?;
    Ok(ResolvedProjectTarget {
        entry_name: entry_name.to_string(),
        entry_path: entry_path.to_string(),
        platform_name: runnable.then(|| "cli".to_string()),
        startup_function: runnable.then(|| "main".to_string()),
        platform_contract: None,
    })
}

fn module_only_target(root: &Path, entry_path: &str) -> Result<ResolvedProjectTarget, String> {
    ensure_entry_exists(root, entry_path)?;
    Ok(ResolvedProjectTarget {
        entry_name: path_stem(entry_path),
        entry_path: entry_path.to_string(),
        platform_name: None,
        startup_function: None,
        platform_contract: None,
    })
}

fn resolve_platform_binding(
    root: &Path,
    manifest: &ProjectManifest,
    platform_name: &str,
) -> Result<(String, Option<ResolvedPlatformContract>), String> {
    if let Some(dep) = manifest.dependencies.get(platform_name) {
        let contract = resolve_platform_dependency(root, platform_name, dep)?;
        return Ok((contract.startup_function.clone(), Some(contract)));
    }

    let registry = PlatformRegistry::with_builtins();
    let platform = registry.get(platform_name).ok_or_else(|| {
        format!(
            "unknown platform `{platform_name}` in `{}`; declare a matching `[dependencies]` path dependency or use one of the built-ins: {}",
            root.join("spore.toml").display(),
            registry.all_names().join(", ")
        )
    })?;
    Ok((platform.startup_function.clone(), None))
}

fn resolve_platform_dependency(
    root: &Path,
    platform_name: &str,
    dep: &DependencySpec,
) -> Result<ResolvedPlatformContract, String> {
    let manifest_path = root.join("spore.toml");
    let dep_path = dep.path.as_deref().ok_or_else(|| {
        format!(
            "platform `{platform_name}` in `{}` must be backed by a dependency with `path = ...`",
            manifest_path.display()
        )
    })?;
    let dep_root = resolve_dependency_root(root, dep_path);
    if !dep_root.is_dir() {
        return Err(format!(
            "platform dependency `{platform_name}` resolves to `{}` which is not a directory",
            dep_root.display()
        ));
    }

    let dep_manifest = load_project_manifest(&dep_root)?;
    if dep_manifest.package_type.as_deref() != Some("platform") {
        let actual = dep_manifest
            .package_type
            .as_deref()
            .unwrap_or("missing `[package].type`");
        return Err(format!(
            "platform dependency `{platform_name}` at `{}` must declare `[package].type = \"platform\"`, found `{actual}`",
            dep_root.join("spore.toml").display()
        ));
    }
    let platform = dep_manifest.platform.as_ref().ok_or_else(|| {
        format!(
            "platform dependency `{platform_name}` at `{}` is missing `[platform]` metadata",
            dep_root.join("spore.toml").display()
        )
    })?;

    let contract_module = normalize_module_path(&platform.contract_module).map_err(|error| {
        format!(
            "invalid `[platform].contract-module` for dependency `{platform_name}` in `{}`: {error}",
            dep_root.join("spore.toml").display()
        )
    })?;
    if platform.startup_contract.trim().is_empty() {
        return Err(format!(
            "platform dependency `{platform_name}` at `{}` is missing `[platform].startup-contract`",
            dep_root.join("spore.toml").display()
        ));
    }
    if platform.adapter_function.trim().is_empty() {
        return Err(format!(
            "platform dependency `{platform_name}` at `{}` is missing `[platform].adapter-function`",
            dep_root.join("spore.toml").display()
        ));
    }

    let contract_path = dep_root
        .join("src")
        .join(contract_module.replace('.', "/"))
        .with_extension("sp");
    if !contract_path.is_file() {
        return Err(format!(
            "platform dependency `{platform_name}` expects contract module `{contract_module}` at `{}`",
            contract_path.display()
        ));
    }

    Ok(ResolvedPlatformContract {
        name: platform_name.to_string(),
        root: dep_root,
        contract_module,
        startup_function: platform.startup_contract.clone(),
        adapter_function: platform.adapter_function.clone(),
        handles: platform.handles.clone(),
    })
}

fn resolve_dependency_root(root: &Path, dep_path: &str) -> PathBuf {
    let dep_path = Path::new(dep_path);
    if dep_path.is_absolute() {
        dep_path.to_path_buf()
    } else {
        root.join(dep_path)
    }
}

fn normalize_entry_path(path: &str) -> Result<String, String> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err("entry path cannot be empty".to_string());
    }
    if normalized.starts_with('/') {
        return Err(format!("entry path `{path}` must be relative to `src/`"));
    }
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => continue,
            ".." => return Err(format!("entry path `{path}` must stay within `src/`")),
            _ => parts.push(part),
        }
    }

    if parts.is_empty() {
        return Err(format!("entry path `{path}` must name a file under `src/`"));
    }

    Ok(parts.join("/"))
}

fn ensure_entry_exists(root: &Path, entry_path: &str) -> Result<(), String> {
    let full_path = root.join("src").join(entry_path);
    if full_path.is_file() {
        Ok(())
    } else {
        Err(format!(
            "expected entry path `{}` to exist at `{}`",
            entry_path,
            full_path.display()
        ))
    }
}

fn path_stem(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".sp")
        .to_string()
}

enum Section {
    Package,
    Project,
    Platform,
    Dependencies,
    Entry(String),
    Other,
}

fn strip_toml_comment(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_double => {
                out.push(ch);
                escaped = true;
            }
            '\'' if !in_double => {
                in_single = !in_single;
                out.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                out.push(ch);
            }
            '#' if !in_single && !in_double => break,
            _ => out.push(ch),
        }
    }

    out
}

fn parse_toml_string(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

fn parse_toml_string_array(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    let Some(inner) = trimmed
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
    else {
        return Vec::new();
    };

    split_toml_items(inner)
        .into_iter()
        .map(|item| parse_toml_string(&item))
        .filter(|item| !item.is_empty())
        .collect()
}

fn parse_dependency_spec(value: &str) -> DependencySpec {
    let trimmed = value.trim();
    let path = if trimmed.starts_with('{') {
        parse_inline_table_string_field(trimmed, "path")
    } else {
        let value = parse_toml_string(trimmed);
        (!value.is_empty()).then_some(value)
    };
    DependencySpec { path }
}

fn parse_inline_table_string_field(value: &str, field: &str) -> Option<String> {
    let trimmed = value.trim();
    let inner = trimmed
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))?;
    for item in split_toml_items(inner) {
        let Some((key, raw_value)) = item.split_once('=') else {
            continue;
        };
        if key.trim() != field {
            continue;
        }
        let value = parse_toml_string(raw_value);
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

fn split_toml_items(input: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_double => {
                current.push(ch);
                escaped = true;
            }
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            '[' if !in_single && !in_double => {
                bracket_depth += 1;
                current.push(ch);
            }
            ']' if !in_single && !in_double => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(ch);
            }
            '{' if !in_single && !in_double => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' if !in_single && !in_double => {
                brace_depth = brace_depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if !in_single && !in_double && bracket_depth == 0 && brace_depth == 0 => {
                let item = current.trim();
                if !item.is_empty() {
                    items.push(item.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let item = current.trim();
    if !item.is_empty() {
        items.push(item.to_string());
    }

    items
}

fn normalize_module_path(module: &str) -> Result<String, String> {
    let normalized = module.trim().trim_end_matches(".sp").replace('\\', "/");
    if normalized.is_empty() {
        return Err("module path cannot be empty".to_string());
    }
    if normalized.starts_with('/') {
        return Err(format!("module path `{module}` must be relative to `src/`"));
    }

    let mut parts = Vec::new();
    for segment in normalized.split('/') {
        for part in segment.split('.') {
            match part {
                "" | "." => continue,
                ".." => return Err(format!("module path `{module}` must stay within `src/`")),
                _ => parts.push(part.to_string()),
            }
        }
    }

    if parts.is_empty() {
        return Err(format!(
            "module path `{module}` must name a module under `src/`"
        ));
    }

    Ok(parts.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
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
            basic-cli = { path = "../basic-cli" }

            [entries.app]
            path = "app.sp"
            "#,
        );
        project.write("src/app.sp", "fn main() -> () { return }\n");
        project.write(
            "../basic-cli/spore.toml",
            r#"
            [package]
            name = "basic-cli"
            type = "platform"

            [platform]
            contract-module = "platform_contract"
            startup-contract = "main"
            adapter-function = "main_for_host"
            handles = ["Console", "Env"]
            "#,
        );
        project.write(
            "../basic-cli/src/platform_contract.sp",
            r#"
            pub fn main() -> () {
                ?platform_startup_contract
            }

            pub fn main_for_host(app_main: () -> ()) -> () {
                app_main()
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
            contract.handles,
            vec!["Console".to_string(), "Env".to_string()]
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
    }
}
