use std::collections::BTreeMap;
use std::path::Path;

use super::{DependencySpec, PlatformManifest, ProjectConfig, ProjectEntry, ProjectManifest};

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
    let mut platform_handled_effects = None;
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
            Section::Platform if key == "handled-effects" => {
                platform_handled_effects = Some(parse_toml_string_array(raw_value));
            }
            Section::Platform if key == "handles" => {
                return Err(format!(
                    "unsupported legacy key `[platform].handles` in `{}`; rename it to `[platform].handled-effects`",
                    manifest_path.display()
                ));
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
            handled_effects: platform_handled_effects.unwrap_or_default(),
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

pub(super) fn normalize_entry_path(path: &str) -> Result<String, String> {
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

pub(super) fn ensure_entry_exists(root: &Path, entry_path: &str) -> Result<(), String> {
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

pub(super) fn path_stem(path: &str) -> String {
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

pub(super) fn normalize_module_path(module: &str) -> Result<String, String> {
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
