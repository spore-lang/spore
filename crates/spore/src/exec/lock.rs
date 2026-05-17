use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use owo_colors::OwoColorize;
use sporec_driver::project::load_project_manifest;

pub(crate) fn exec_lock(path: Option<&str>, check: bool) -> ExitCode {
    let root = match path {
        Some(path) => PathBuf::from(path),
        None => match std::env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                eprintln!(
                    "{}: cannot determine current directory: {e}",
                    "error".red().bold()
                );
                return ExitCode::FAILURE;
            }
        },
    };

    match lock_project(&root, check) {
        Ok(summary) => {
            if check {
                println!(
                    "{} lockfile is up to date — {} package(s)",
                    "✓".green(),
                    summary.package_count
                );
            } else {
                println!(
                    "{} wrote {} and stored {} package(s)",
                    "✓".green(),
                    summary.lock_path.display(),
                    summary.package_count
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}: {e}", "error".red().bold());
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageLockEntry {
    name: String,
    kind: String,
    source: String,
    content_hash: String,
    store_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LockSummary {
    lock_path: PathBuf,
    package_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExistingLockEntry {
    source: String,
    store_path: String,
}

fn lock_project(root: &Path, check: bool) -> Result<LockSummary, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("cannot resolve project root `{}`: {e}", root.display()))?;
    let manifest_path = root.join("spore.toml");
    if !manifest_path.is_file() {
        return Err(format!(
            "expected `{}` to exist; run `spore init` first or pass a project path",
            manifest_path.display()
        ));
    }

    let packages = collect_packages(&root)?;
    let content = render_lockfile(&packages);
    let lock_path = root.join(".spore-lock");

    if check {
        let existing = fs::read_to_string(&lock_path)
            .map_err(|e| format!("cannot read `{}`: {e}", lock_path.display()))?;
        if existing != content {
            return Err(format!(
                "`{}` is out of date; run `spore lock`",
                lock_path.display()
            ));
        }
    } else {
        materialize_store(&root, &packages)?;
        write_if_changed(&lock_path, &content)?;
    }

    Ok(LockSummary {
        lock_path,
        package_count: packages.len(),
    })
}

fn collect_packages(root: &Path) -> Result<Vec<PackageLockEntry>, String> {
    let existing_lock = load_existing_lock(root).unwrap_or_default();
    let mut entries = Vec::new();
    let mut queue = VecDeque::from([(root.to_path_buf(), ".".to_string())]);
    let mut seen = BTreeSet::new();

    while let Some((package_root, source)) = queue.pop_front() {
        let package_root = package_root.canonicalize().map_err(|e| {
            format!(
                "cannot resolve dependency root `{}`: {e}",
                package_root.display()
            )
        })?;
        if !seen.insert(package_root.clone()) {
            continue;
        }

        let manifest = load_project_manifest(&package_root)?;
        let name = manifest.package_name.clone().unwrap_or_else(|| {
            package_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("package")
                .to_string()
        });
        let kind = manifest
            .package_type
            .clone()
            .unwrap_or_else(|| "package".to_string());
        let content_hash = package_content_hash(&package_root, &manifest.source_roots())?;
        let store_path = format!(".spore-store/packages/{content_hash}");

        entries.push(PackageLockEntry {
            name,
            kind,
            source,
            content_hash,
            store_path,
        });

        for (dep_name, dep) in manifest.dependencies {
            let (dep_root, dep_source) = if let Some(dep_path) = dep.path {
                let dep_root = resolve_path_dependency(&package_root, &dep_path);
                let dep_root = dep_root.canonicalize().map_err(|e| {
                    format!(
                        "cannot resolve dependency `{dep_name}` from `{}` at `{}`: {e}",
                        package_root.join("spore.toml").display(),
                        dep_root.display()
                    )
                })?;
                let dep_source = lock_source_path(root, &dep_root);
                (dep_root, dep_source)
            } else {
                let Some(locked) = existing_lock.get(&dep_name) else {
                    return Err(format!(
                        "dependency `{dep_name}` in `{}` is missing `path = ...` and has no matching entry in `{}`; add a local path before first lock generation",
                        package_root.join("spore.toml").display(),
                        root.join(".spore-lock").display()
                    ));
                };
                (root.join(&locked.store_path), locked.source.clone())
            };
            queue.push_back((dep_root, dep_source));
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.source.cmp(&b.source)));
    Ok(entries)
}

fn package_content_hash(root: &Path, source_roots: &[String]) -> Result<String, String> {
    let files = package_files(root, source_roots)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"spore-package-v1\n");
    for rel in files {
        let path = root.join(&rel);
        let bytes =
            fs::read(&path).map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(b"\0");
        hasher.update(&bytes);
        hasher.update(b"\n");
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn package_files(root: &Path, source_roots: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut files = vec![PathBuf::from("spore.toml")];
    for source_root in source_roots {
        let dir = root.join(source_root);
        if dir.is_dir() {
            collect_sp_files(root, &dir, &mut files)?;
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_sp_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory `{}`: {e}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot read directory entry in `{}`: {e}", dir.display()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_sp_files(root, &path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("sp") {
            let rel = path.strip_prefix(root).map_err(|e| {
                format!(
                    "cannot make `{}` relative to `{}`: {e}",
                    path.display(),
                    root.display()
                )
            })?;
            files.push(rel.to_path_buf());
        }
    }
    Ok(())
}

fn materialize_store(root: &Path, packages: &[PackageLockEntry]) -> Result<(), String> {
    for package in packages {
        let source_root = resolve_lock_source(root, &package.source);
        let manifest = load_project_manifest(&source_root)?;
        let files = package_files(&source_root, &manifest.source_roots())?;
        let dest_root = root.join(&package.store_path);
        if dest_root.exists() {
            fs::remove_dir_all(&dest_root)
                .map_err(|e| format!("cannot refresh `{}`: {e}", dest_root.display()))?;
        }
        for rel in files {
            let src = source_root.join(&rel);
            let dest = dest_root.join(&rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("cannot create `{}`: {e}", parent.display()))?;
            }
            fs::copy(&src, &dest).map_err(|e| {
                format!(
                    "cannot copy `{}` to `{}`: {e}",
                    src.display(),
                    dest.display()
                )
            })?;
        }
    }
    Ok(())
}

fn load_existing_lock(root: &Path) -> Result<BTreeMap<String, ExistingLockEntry>, String> {
    let path = root.join(".spore-lock");
    let source =
        fs::read_to_string(&path).map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
    let mut packages = BTreeMap::new();
    let mut name: Option<String> = None;
    let mut source_path: Option<String> = None;
    let mut store_path: Option<String> = None;

    let flush = |packages: &mut BTreeMap<String, ExistingLockEntry>,
                 name: &mut Option<String>,
                 source_path: &mut Option<String>,
                 store_path: &mut Option<String>| {
        if let Some(name) = name.take() {
            packages.insert(
                name,
                ExistingLockEntry {
                    source: source_path.take().unwrap_or_default(),
                    store_path: store_path.take().unwrap_or_default(),
                },
            );
        }
        *source_path = None;
        *store_path = None;
    };

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[package]]" {
            flush(&mut packages, &mut name, &mut source_path, &mut store_path);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "name" => name = Some(parse_lock_string(value)),
            "source" => source_path = Some(parse_lock_string(value)),
            "store-path" => store_path = Some(parse_lock_string(value)),
            _ => {}
        }
    }
    flush(&mut packages, &mut name, &mut source_path, &mut store_path);
    Ok(packages)
}

fn render_lockfile(packages: &[PackageLockEntry]) -> String {
    let mut out = String::from("# Auto-generated by `spore lock`. Do not edit.\nversion = 1\n\n");
    for package in packages {
        out.push_str("[[package]]\n");
        out.push_str(&format!(
            "name = \"{}\"\n",
            escape_toml_string(&package.name)
        ));
        out.push_str(&format!(
            "kind = \"{}\"\n",
            escape_toml_string(&package.kind)
        ));
        out.push_str(&format!(
            "source = \"{}\"\n",
            escape_toml_string(&package.source)
        ));
        out.push_str(&format!("content-hash = \"{}\"\n", package.content_hash));
        out.push_str(&format!(
            "store-path = \"{}\"\n\n",
            escape_toml_string(&package.store_path)
        ));
    }
    out
}

fn write_if_changed(path: &Path, content: &str) -> Result<(), String> {
    if matches!(fs::read_to_string(path), Ok(existing) if existing == content) {
        return Ok(());
    }
    let mut file =
        fs::File::create(path).map_err(|e| format!("cannot create `{}`: {e}", path.display()))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("cannot write `{}`: {e}", path.display()))
}

fn resolve_path_dependency(root: &Path, dep_path: &str) -> PathBuf {
    let path = Path::new(dep_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn resolve_lock_source(root: &Path, source: &str) -> PathBuf {
    if source == "." {
        root.to_path_buf()
    } else {
        resolve_path_dependency(root, source)
    }
}

fn lock_source_path(root: &Path, path: &Path) -> String {
    relative_to_root(root, path)
        .filter(|source| !source.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().replace('\\', "/"))
}

fn relative_to_root(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
}

fn parse_lock_string(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_project_writes_lockfile_and_store_for_path_dependencies() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("vendor/basic-cli/src")).unwrap();
        fs::write(
            root.join("spore.toml"),
            r#"
            [package]
            name = "app"
            type = "application"

            [project]
            platform = "basic-cli"
            default-entry = "app"

            [dependencies]
            basic-cli = { path = "vendor/basic-cli" }

            [entries.app]
            path = "main.sp"
            "#,
        )
        .unwrap();
        fs::write(root.join("src/main.sp"), "fn main() -> () { return }\n").unwrap();
        fs::write(
            root.join("vendor/basic-cli/spore.toml"),
            r#"
            [package]
            name = "basic-cli"
            type = "platform"
            "#,
        )
        .unwrap();
        fs::write(
            root.join("vendor/basic-cli/src/host.sp"),
            "pub fn main() -> () { return }\n",
        )
        .unwrap();

        let summary = lock_project(root, false).unwrap();
        assert_eq!(summary.package_count, 2);
        let lock = fs::read_to_string(root.join(".spore-lock")).unwrap();
        assert!(lock.contains("name = \"app\""));
        assert!(lock.contains("name = \"basic-cli\""));
        assert!(lock.contains("content-hash = \""));
        assert!(lock.contains("store-path = \".spore-store/packages/"));
        assert!(root.join(".spore-store/packages").is_dir());

        lock_project(root, true).unwrap();

        fs::write(
            root.join("spore.toml"),
            r#"
            [package]
            name = "app"
            type = "application"

            [project]
            platform = "basic-cli"
            default-entry = "app"

            [dependencies]
            basic-cli = { content-hash = "locked" }

            [entries.app]
            path = "main.sp"
            "#,
        )
        .unwrap();
        lock_project(root, false).unwrap();
        lock_project(root, true).unwrap();
    }

    #[test]
    fn lock_project_materializes_transitive_dependency_paths_relative_to_dependency_package() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("app");
        let deps = tmp.path().join("deps");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(deps.join("dep-a/src")).unwrap();
        fs::create_dir_all(deps.join("dep-b/src")).unwrap();
        fs::write(
            app.join("spore.toml"),
            r#"
            [package]
            name = "app"
            type = "package"

            [dependencies]
            dep-a = { path = "../deps/dep-a" }
            "#,
        )
        .unwrap();
        fs::write(app.join("src/lib.sp"), "pub fn app() -> I32 { 1 }\n").unwrap();
        fs::write(
            deps.join("dep-a/spore.toml"),
            r#"
            [package]
            name = "dep-a"
            type = "package"

            [dependencies]
            dep-b = { path = "../dep-b" }
            "#,
        )
        .unwrap();
        fs::write(deps.join("dep-a/src/dep_a.sp"), "pub fn a() -> I32 { 1 }\n").unwrap();
        fs::write(
            deps.join("dep-b/spore.toml"),
            r#"
            [package]
            name = "dep-b"
            type = "package"
            "#,
        )
        .unwrap();
        fs::write(deps.join("dep-b/src/dep_b.sp"), "pub fn b() -> I32 { 2 }\n").unwrap();

        let summary = lock_project(&app, false).unwrap();
        assert_eq!(summary.package_count, 3);
        let dep_b_source = deps
            .join("dep-b")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let lock = fs::read_to_string(app.join(".spore-lock")).unwrap();
        assert!(lock.contains("name = \"dep-b\""));
        assert!(lock.contains(&format!("source = \"{dep_b_source}\"")));
    }

    #[test]
    fn lock_project_check_rejects_outdated_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("spore.toml"),
            r#"
            [package]
            name = "pkg"
            type = "package"

            [dependencies]
            "#,
        )
        .unwrap();
        fs::write(root.join("src/lib.sp"), "pub fn one() -> I32 { 1 }\n").unwrap();

        lock_project(root, false).unwrap();
        fs::write(root.join("src/lib.sp"), "pub fn one() -> I32 { 2 }\n").unwrap();
        let err = lock_project(root, true).unwrap_err();
        assert!(err.contains("out of date"));
    }
}
