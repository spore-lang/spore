use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sporec_typeck::platform::PlatformRegistry;

use super::toml_parse::{
    ensure_entry_exists, load_project_manifest, normalize_entry_path, normalize_module_path,
    path_stem,
};
use super::{DependencySpec, ProjectManifest, ResolvedPlatformContract, ResolvedProjectTarget};

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
            return module_only_target(root, &normalized, dependency_roots(root, &manifest));
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
    let dependency_roots = dependency_roots(root, manifest);

    let (startup_function, platform_contract) =
        resolve_platform_binding(root, manifest, &project.platform)?;

    Ok(ResolvedProjectTarget {
        entry_name: entry_name.to_string(),
        entry_path,
        platform_name: Some(project.platform.clone()),
        startup_function: Some(startup_function),
        platform_contract,
        dependency_roots,
    })
}

fn legacy_default_target(
    root: &Path,
    manifest: &ProjectManifest,
) -> Result<ResolvedProjectTarget, String> {
    let dependency_roots = dependency_roots(root, manifest);
    match manifest.package_type.as_deref() {
        Some("application") => legacy_named_target(root, "app", "main.sp", true, dependency_roots),
        Some("platform") => legacy_named_target(root, "host", "host.sp", true, dependency_roots),
        Some("package") => legacy_named_target(root, "lib", "lib.sp", false, dependency_roots),
        Some(other) => Err(format!(
            "unsupported legacy `[package].type = \"{other}\"` in `{}`",
            root.join("spore.toml").display()
        )),
        None => infer_single_default_target(root, dependency_roots),
    }
}

fn legacy_target_for_path(
    root: &Path,
    manifest: &ProjectManifest,
    entry_path: &str,
) -> Result<ResolvedProjectTarget, String> {
    ensure_entry_exists(root, entry_path)?;
    let dependency_roots = dependency_roots(root, manifest);

    match manifest.package_type.as_deref() {
        Some("application") if entry_path == "main.sp" => {
            legacy_named_target(root, "app", "main.sp", true, dependency_roots)
        }
        Some("platform") if entry_path == "host.sp" => {
            legacy_named_target(root, "host", "host.sp", true, dependency_roots)
        }
        None if entry_path == "main.sp" => {
            legacy_named_target(root, "app", "main.sp", true, dependency_roots)
        }
        None if entry_path == "host.sp" => {
            legacy_named_target(root, "host", "host.sp", true, dependency_roots)
        }
        Some("package") | Some("application") | Some("platform") | None => {
            Ok(ResolvedProjectTarget {
                entry_name: path_stem(entry_path),
                entry_path: entry_path.to_string(),
                platform_name: None,
                startup_function: None,
                platform_contract: None,
                dependency_roots,
            })
        }
        Some(other) => Err(format!(
            "unsupported legacy `[package].type = \"{other}\"` in `{}`",
            root.join("spore.toml").display()
        )),
    }
}

fn infer_single_default_target(
    root: &Path,
    dependency_roots: Vec<PathBuf>,
) -> Result<ResolvedProjectTarget, String> {
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
        [(entry_name, path, runnable)] => {
            legacy_named_target(root, entry_name, path, *runnable, dependency_roots)
        }
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
    dependency_roots: Vec<PathBuf>,
) -> Result<ResolvedProjectTarget, String> {
    ensure_entry_exists(root, entry_path)?;
    Ok(ResolvedProjectTarget {
        entry_name: entry_name.to_string(),
        entry_path: entry_path.to_string(),
        platform_name: runnable.then(|| "cli".to_string()),
        startup_function: runnable.then(|| "main".to_string()),
        platform_contract: None,
        dependency_roots,
    })
}

fn module_only_target(
    root: &Path,
    entry_path: &str,
    dependency_roots: Vec<PathBuf>,
) -> Result<ResolvedProjectTarget, String> {
    ensure_entry_exists(root, entry_path)?;
    Ok(ResolvedProjectTarget {
        entry_name: path_stem(entry_path),
        entry_path: entry_path.to_string(),
        platform_name: None,
        startup_function: None,
        platform_contract: None,
        dependency_roots,
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

fn dependency_roots(root: &Path, manifest: &ProjectManifest) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    collect_dependency_roots(root, manifest, &mut roots, &mut seen);
    roots
}

fn collect_dependency_roots(
    root: &Path,
    manifest: &ProjectManifest,
    roots: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) {
    for dep_path in manifest
        .dependencies
        .values()
        .filter_map(|dep| dep.path.as_deref())
    {
        let dep_root = resolve_dependency_root(root, dep_path);
        if !dep_root.is_dir() {
            continue;
        }
        let normalized_root = std::fs::canonicalize(&dep_root).unwrap_or_else(|_| dep_root.clone());
        if !seen.insert(normalized_root.clone()) {
            continue;
        }
        roots.push(normalized_root.clone());
        if let Ok(dep_manifest) = load_project_manifest(&normalized_root) {
            collect_dependency_roots(&normalized_root, &dep_manifest, roots, seen);
        }
    }
}
