use std::path::{Path, PathBuf};

pub(crate) fn find_project_target(file: &str) -> Option<(PathBuf, String)> {
    let file_path = std::fs::canonicalize(file).ok()?;
    let mut dir = file_path.parent()?;

    loop {
        let manifest = dir.join("spore.toml");
        let src_dir = dir.join("src");
        if manifest.is_file() && src_dir.is_dir() {
            let rel = file_path.strip_prefix(&src_dir).ok()?;
            return Some((dir.to_path_buf(), rel.to_string_lossy().replace('\\', "/")));
        }
        dir = dir.parent()?;
    }
}

pub(crate) fn find_project_root(path: &Path) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(path).ok()?;
    let mut dir = if canonical.is_dir() {
        canonical
    } else {
        canonical.parent()?.to_path_buf()
    };

    loop {
        let manifest = dir.join("spore.toml");
        let src_dir = dir.join("src");
        if manifest.is_file() && src_dir.is_dir() {
            return Some(dir);
        }
        dir = dir.parent()?.to_path_buf();
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

/// Resolve a list of CLI path arguments to `.sp` file paths.
///
/// - Empty `paths` defaults to `cwd` (ruff-style: operate on the current directory).
/// - Directories are recursed to find all `.sp` files.
/// - File paths are used as-is.
pub(crate) fn resolve_sp_targets(paths: &[String], cwd: &Path) -> Result<Vec<PathBuf>, String> {
    let roots: Vec<PathBuf> = if paths.is_empty() {
        vec![cwd.to_path_buf()]
    } else {
        paths.iter().map(|p| resolve_cli_path(p, cwd)).collect()
    };

    let mut result = Vec::new();
    for root in roots {
        if root.is_dir() {
            collect_sp_recursive(&root, &mut result)?;
        } else {
            result.push(root);
        }
    }
    result.sort();
    Ok(result)
}

pub(crate) fn resolve_build_target(file: Option<&str>, cwd: &Path) -> Result<BuildTarget, String> {
    match file {
        Some(path) => {
            let resolved_path = resolve_cli_path(path, cwd);
            if Path::new(path) == Path::new(".") || resolved_path.is_dir() {
                let root = find_project_root(&resolved_path).ok_or_else(|| {
                    format!(
                        "`{}` is not a Spore project directory (expected `spore.toml` and `src/`)",
                        Path::new(path).display()
                    )
                })?;
                let entry = infer_project_entry(&root)?;
                Ok(BuildTarget::Project { root, entry })
            } else if let Some((root, entry)) =
                find_project_target(resolved_path.to_string_lossy().as_ref())
            {
                Ok(BuildTarget::Project { root, entry })
            } else {
                Ok(BuildTarget::File(path.to_string()))
            }
        }
        None => {
            let root = find_project_root(cwd).ok_or_else(|| {
                "no FILE provided and current directory is not inside a Spore project; pass a .sp file or run `spore build` from a project root".to_string()
            })?;
            let entry = infer_project_entry(&root)?;
            Ok(BuildTarget::Project { root, entry })
        }
    }
}
