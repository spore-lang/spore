use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use sporec_typeck::platform::PlatformRegistry;

use super::toml_parse::{
    load_project_manifest, normalize_entry_path, normalize_module_path, path_stem,
    resolve_entry_source_root,
};
use super::{DependencySpec, ProjectManifest, ResolvedPlatformContract, ResolvedProjectTarget};

pub fn resolve_default_project_target(root: &Path) -> Result<ResolvedProjectTarget, String> {
    let manifest = load_project_manifest(root)?;

    let project = manifest.project.as_ref().ok_or_else(|| {
        format!(
            "`{}` has no `[project]` default target; pass an explicit entry file or declare `[project].default-entry` with a matching `[entries.<name>]` table",
            root.join("spore.toml").display()
        )
    })?;
    let default_entry = project.default_entry.as_deref().ok_or_else(|| {
        format!(
            "`{}` has `[project]` but no `default-entry`; pass an explicit entry file or declare one",
            root.join("spore.toml").display()
        )
    })?;
    resolve_declared_entry(root, &manifest, default_entry)
}

pub fn resolve_project_target_by_path(
    root: &Path,
    entry_path: &str,
) -> Result<ResolvedProjectTarget, String> {
    let manifest = load_project_manifest(root)?;
    let normalized = normalize_entry_path(entry_path)?;

    if manifest.project.is_some() {
        let Some(entry_name) = declared_entry_name_for_path(root, &manifest, &normalized)? else {
            return module_only_target(
                root,
                &normalized,
                dependency_source_roots(root, &manifest)?,
            );
        };
        return resolve_declared_entry(root, &manifest, &entry_name);
    }

    module_only_target(root, &normalized, dependency_source_roots(root, &manifest)?)
}

fn declared_entry_name_for_path(
    root: &Path,
    manifest: &ProjectManifest,
    normalized_entry_path: &str,
) -> Result<Option<String>, String> {
    for (entry_name, entry) in &manifest.entries {
        let entry_path = normalize_entry_path(&entry.path).map_err(|error| {
            format!(
                "invalid `[entries.{entry_name}].path` in `{}`: {error}",
                root.join("spore.toml").display()
            )
        })?;
        if entry_path == normalized_entry_path {
            return Ok(Some(entry_name.clone()));
        }
    }
    Ok(None)
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
    let source_roots = manifest.source_roots();
    let entry_source_root = resolve_entry_source_root(root, &source_roots, &entry_path)?;
    let dependency_source_roots = dependency_source_roots(root, manifest)?;

    let (platform_name, startup_function, platform_contract) = if project.platform.trim().is_empty()
    {
        (None, None, None)
    } else {
        let (startup_function, platform_contract) =
            resolve_platform_binding(root, manifest, &project.platform)?;
        (
            Some(project.platform.clone()),
            Some(startup_function),
            platform_contract,
        )
    };

    Ok(ResolvedProjectTarget {
        entry_name: entry_name.to_string(),
        entry_path,
        entry_source_root,
        source_roots,
        platform_name,
        startup_function,
        platform_contract,
        dependency_source_roots,
    })
}

fn module_only_target(
    root: &Path,
    entry_path: &str,
    dependency_source_roots: Vec<PathBuf>,
) -> Result<ResolvedProjectTarget, String> {
    let source_roots = load_project_manifest(root)?.source_roots();
    let entry_source_root = resolve_entry_source_root(root, &source_roots, entry_path)?;
    Ok(ResolvedProjectTarget {
        entry_name: path_stem(entry_path),
        entry_path: entry_path.to_string(),
        entry_source_root,
        source_roots,
        platform_name: None,
        startup_function: None,
        platform_contract: None,
        dependency_source_roots,
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
    let dep_root =
        resolve_dependency_root_for(root, root, platform_name, dep).map_err(|error| {
            format!(
                "cannot resolve platform dependency `{platform_name}` in `{}`: {error}",
                manifest_path.display()
            )
        })?;
    if !dep_root.is_dir() {
        return Err(format!(
            "platform dependency `{platform_name}` resolves to `{}` which is not a directory",
            dep_root.display()
        ));
    }
    let dep_root = dep_root.canonicalize().map_err(|e| {
        format!(
            "cannot resolve platform dependency `{platform_name}` at `{}`: {e}",
            dep_root.display()
        )
    })?;

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

    let contract_rel_path = PathBuf::from(contract_module.replace('.', "/")).with_extension("sp");
    let mut contract_paths = project_source_roots(&dep_manifest)
        .into_iter()
        .map(|source_root| dep_root.join(source_root).join(&contract_rel_path))
        .filter(|path| path.is_file());
    let Some(_contract_path) = contract_paths.next() else {
        return Err(format!(
            "platform dependency `{platform_name}` expects contract module `{contract_module}` under one of the configured source roots in `{}`",
            dep_root.join("spore.toml").display()
        ));
    };
    if contract_paths.next().is_some() {
        return Err(format!(
            "platform dependency `{platform_name}` resolves contract module `{contract_module}` ambiguously across multiple configured source roots in `{}`",
            dep_root.join("spore.toml").display()
        ));
    }

    Ok(ResolvedPlatformContract {
        name: platform_name.to_string(),
        root: dep_root,
        source_roots: project_source_roots(&dep_manifest),
        contract_module,
        startup_function: platform.startup_contract.clone(),
        adapter_function: platform.adapter_function.clone(),
        handled_effects: platform.handled_effects.clone(),
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

fn resolve_dependency_root_for(
    project_root: &Path,
    package_root: &Path,
    dep_name: &str,
    dep: &DependencySpec,
) -> Result<PathBuf, String> {
    if let Some(dep_path) = dep.path.as_deref()
        && !is_locked_store_package(project_root, package_root)
    {
        return Ok(resolve_dependency_root(package_root, dep_path));
    }

    let lock = load_lockfile(project_root)?;
    let Some(entry) = lock.get(dep_name) else {
        return Err(format!(
            "dependency `{dep_name}` has no `path = ...` and no matching package entry in `{}`",
            project_root.join(".spore-lock").display()
        ));
    };
    if entry.store_path.trim().is_empty() {
        return Err(format!(
            "locked dependency `{dep_name}` in `{}` has no `store-path`",
            project_root.join(".spore-lock").display()
        ));
    }
    Ok(resolve_dependency_root(project_root, &entry.store_path))
}

fn is_locked_store_package(project_root: &Path, package_root: &Path) -> bool {
    package_root.starts_with(project_root.join(".spore-store/packages"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LockPackageEntry {
    store_path: String,
}

fn load_lockfile(root: &Path) -> Result<BTreeMap<String, LockPackageEntry>, String> {
    let path = root.join(".spore-lock");
    let source = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
    let mut packages = BTreeMap::new();
    let mut current_name: Option<String> = None;
    let mut current_store_path: Option<String> = None;

    let flush = |packages: &mut BTreeMap<String, LockPackageEntry>,
                 current_name: &mut Option<String>,
                 current_store_path: &mut Option<String>| {
        if let Some(name) = current_name.take() {
            packages.insert(
                name,
                LockPackageEntry {
                    store_path: current_store_path.take().unwrap_or_default(),
                },
            );
        }
        *current_store_path = None;
    };

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[package]]" {
            flush(&mut packages, &mut current_name, &mut current_store_path);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "name" => current_name = Some(parse_lock_string(value)),
            "store-path" => current_store_path = Some(parse_lock_string(value)),
            _ => {}
        }
    }
    flush(&mut packages, &mut current_name, &mut current_store_path);
    Ok(packages)
}

fn parse_lock_string(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

fn project_source_roots(manifest: &ProjectManifest) -> Vec<String> {
    manifest.source_roots()
}

fn dependency_source_roots(
    root: &Path,
    manifest: &ProjectManifest,
) -> Result<Vec<PathBuf>, String> {
    let project_root = root
        .canonicalize()
        .map_err(|e| format!("cannot resolve project root `{}`: {e}", root.display()))?;
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    collect_dependency_source_roots(
        &project_root,
        &project_root,
        manifest,
        &mut roots,
        &mut seen,
    )?;
    Ok(roots)
}

fn collect_dependency_source_roots(
    project_root: &Path,
    package_root: &Path,
    manifest: &ProjectManifest,
    roots: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    for (dep_name, dep) in &manifest.dependencies {
        let dep_root = resolve_dependency_root_for(project_root, package_root, dep_name, dep)?;
        if !dep_root.is_dir() {
            return Err(format!(
                "dependency `{dep_name}` resolves to `{}` which is not a directory",
                dep_root.display()
            ));
        }
        let normalized_root = std::fs::canonicalize(&dep_root).map_err(|e| {
            format!(
                "cannot resolve dependency root `{}`: {e}",
                dep_root.display()
            )
        })?;
        if !seen.insert(normalized_root.clone()) {
            continue;
        }
        let dep_manifest = load_project_manifest(&normalized_root).map_err(|error| {
            format!(
                "cannot load dependency `{dep_name}` manifest at `{}`: {error}",
                normalized_root.join("spore.toml").display()
            )
        })?;
        for source_root in dep_manifest.source_roots() {
            roots.push(normalized_root.join(source_root));
        }
        collect_dependency_source_roots(
            project_root,
            &normalized_root,
            &dep_manifest,
            roots,
            seen,
        )?;
    }
    Ok(())
}
