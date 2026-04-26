use std::path::{Path, PathBuf};

fn project_source_roots(root: &Path) -> Result<Option<Vec<PathBuf>>, String> {
    let manifest_path = root.join("spore.toml");
    if !manifest_path.is_file() {
        return Ok(None);
    }

    let manifest = sporec_driver::load_project_manifest(root)?;
    let configured_source_roots = manifest.source_roots();
    let source_roots: Vec<PathBuf> = configured_source_roots
        .iter()
        .map(|source_root| root.join(source_root))
        .filter(|source_root| source_root.is_dir())
        .collect();
    if source_roots.is_empty() {
        return Err(format!(
            "`{}` does not have any existing configured source roots ({})",
            manifest_path.display(),
            configured_source_roots.join(", ")
        ));
    }
    Ok(Some(source_roots))
}

fn is_project_root(root: &Path) -> Result<bool, String> {
    Ok(project_source_roots(root)?.is_some())
}

pub(crate) fn find_project_target(file: &str) -> Result<Option<(PathBuf, String)>, String> {
    let file_path = match std::fs::canonicalize(file) {
        Ok(file_path) => file_path,
        Err(_) => return Ok(None),
    };
    let mut dir = match file_path.parent() {
        Some(dir) => dir,
        None => return Ok(None),
    };

    loop {
        // A parent directory may also be a Spore project (e.g. a repo root with configured
        // sources for the platform, while a nested `examples/.../src` app is a separate
        // package). Keep walking up instead of failing the whole search.
        if let Some(source_roots) = project_source_roots(dir)?
            && let Some(rel) = source_roots
                .into_iter()
                .find_map(|source_root| file_path.strip_prefix(&source_root).ok())
        {
            return Ok(Some((
                dir.to_path_buf(),
                rel.to_string_lossy().replace('\\', "/"),
            )));
        }
        dir = match dir.parent() {
            Some(parent) => parent,
            None => return Ok(None),
        };
    }
}

pub(crate) fn find_project_root(path: &Path) -> Result<Option<PathBuf>, String> {
    let canonical = match std::fs::canonicalize(path) {
        Ok(canonical) => canonical,
        Err(_) => return Ok(None),
    };
    let mut dir = if canonical.is_dir() {
        canonical
    } else {
        match canonical.parent() {
            Some(parent) => parent.to_path_buf(),
            None => return Ok(None),
        }
    };

    loop {
        if is_project_root(&dir)? {
            return Ok(Some(dir));
        }
        dir = match dir.parent() {
            Some(parent) => parent.to_path_buf(),
            None => return Ok(None),
        };
    }
}

pub(crate) fn infer_project_entry(root: &Path) -> Result<String, String> {
    sporec_driver::resolve_default_project_target(root).map(|target| target.entry_path)
}

pub(crate) enum BuildTarget {
    Project { root: PathBuf, entry: String },
    File(String),
}

pub(crate) fn resolve_cli_path(path: &str, cwd: &Path) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

/// Recursively collect all `.sp` files under a directory.
fn collect_sp_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory `{}`: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("directory read error: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_sp_recursive(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "sp") {
            out.push(path);
        }
    }
    Ok(())
}

fn collect_project_recursive(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let Some(source_roots) = project_source_roots(root)? else {
        return Ok(());
    };
    for source_root in &source_roots {
        collect_sp_recursive(source_root, out)?;
    }
    collect_nested_projects(root, &source_roots, out)
}

fn collect_nested_projects(
    dir: &Path,
    source_roots: &[PathBuf],
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory `{}`: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("directory read error: {e}"))?;
        let path = entry.path();
        if !path.is_dir()
            || source_roots
                .iter()
                .any(|source_root| path.starts_with(source_root))
        {
            continue;
        }
        if is_project_root(&path)? {
            collect_project_recursive(&path, out)?;
        } else {
            collect_nested_projects(&path, source_roots, out)?;
        }
    }
    Ok(())
}

/// Resolve a list of CLI path arguments to `.sp` file paths.
///
/// - Empty `paths` defaults to `cwd` (ruff-style: operate on the current directory).
/// - Directories are recursed to find all `.sp` files.
/// - File paths are used as-is.
pub(crate) fn resolve_sp_targets(paths: &[String], cwd: &Path) -> Result<Vec<PathBuf>, String> {
    let roots: Vec<PathBuf> = if paths.is_empty() {
        let root = find_project_root(cwd)?.unwrap_or_else(|| cwd.to_path_buf());
        vec![root]
    } else {
        paths.iter().map(|p| resolve_cli_path(p, cwd)).collect()
    };

    let mut result = Vec::new();
    for root in roots {
        if root.is_dir() {
            if is_project_root(&root)? {
                collect_project_recursive(&root, &mut result)?;
            } else {
                collect_sp_recursive(&root, &mut result)?;
            }
        } else {
            result.push(root);
        }
    }
    result.sort();
    result.dedup();
    Ok(result)
}

pub(crate) fn resolve_build_target(file: Option<&str>, cwd: &Path) -> Result<BuildTarget, String> {
    match file {
        Some(path) => {
            let resolved_path = resolve_cli_path(path, cwd);
            if Path::new(path) == Path::new(".") || resolved_path.is_dir() {
                let root = find_project_root(&resolved_path)?.ok_or_else(|| {
                    format!(
                        "`{}` is not a Spore project directory (expected `spore.toml` and at least one configured source root)",
                        Path::new(path).display()
                    )
                })?;
                let entry = infer_project_entry(&root)?;
                Ok(BuildTarget::Project { root, entry })
            } else if let Some((root, entry)) =
                find_project_target(resolved_path.to_string_lossy().as_ref())?
            {
                Ok(BuildTarget::Project { root, entry })
            } else {
                Ok(BuildTarget::File(path.to_string()))
            }
        }
        None => {
            let root = find_project_root(cwd)?.ok_or_else(|| {
                "no FILE provided and current directory is not inside a Spore project; pass a .sp file or run `spore build` from a project root".to_string()
            })?;
            let entry = infer_project_entry(&root)?;
            Ok(BuildTarget::Project { root, entry })
        }
    }
}
