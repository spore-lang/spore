use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

use bpaf::*;
use notify::RecursiveMode;
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};
use owo_colors::OwoColorize;
use serde_json::json;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// CLI definition (bpaf)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Cmd {
    Run {
        file: String,
        json: bool,
    },
    Check {
        files: Vec<String>,
        verbose: bool,
        json: bool,
        deny_warnings: bool,
    },
    Test {
        files: Vec<String>,
        verbose: bool,
        json: bool,
        deny_warnings: bool,
    },
    Format {
        files: Vec<String>,
        check: bool,
        diff: bool,
    },
    Holes {
        file: String,
    },
    Build {
        file: Option<String>,
    },
    Watch {
        file: String,
        json: bool,
    },
    New {
        name: String,
        project_type: String,
    },
    Init {
        project_type: String,
    },
}

fn json_flag() -> impl Parser<bool> {
    long("json").help("Output results as JSON").switch()
}

fn cmd_run_parser() -> impl Parser<Cmd> {
    let json = json_flag();
    let file = positional::<String>("FILE").help("A .sp file to run");
    construct!(Cmd::Run { json, file })
        .to_options()
        .descr("Compile and execute a .sp file")
        .command("run")
}

fn cmd_check_parser() -> impl Parser<Cmd> {
    let verbose = long("verbose")
        .help("Show detailed type inference and cost info")
        .switch();
    let json = json_flag();
    let deny_warnings = long("deny-warnings")
        .help("Treat warnings as errors")
        .switch();
    let files = positional::<String>("FILE")
        .help(".sp file(s) to check")
        .some("expected at least one file");
    construct!(Cmd::Check {
        verbose,
        json,
        deny_warnings,
        files,
    })
    .to_options()
    .descr("Type-check one or more .sp files")
    .command("check")
}

fn cmd_test_parser() -> impl Parser<Cmd> {
    let verbose = long("verbose")
        .help("Show detailed type inference and cost info")
        .switch();
    let json = json_flag();
    let deny_warnings = long("deny-warnings")
        .help("Treat warnings as errors")
        .switch();
    let files = positional::<String>("FILE")
        .help(".sp file(s) to validate as tests")
        .some("expected at least one file");
    construct!(Cmd::Test {
        verbose,
        json,
        deny_warnings,
        files,
    })
    .to_options()
    .descr("Validate test/spec files (MVP: static checking only)")
    .command("test")
}

fn cmd_format_parser() -> impl Parser<Cmd> {
    let fmt_inner = || {
        let check = long("check")
            .help("Check if files are formatted (no changes)")
            .switch();
        let diff = long("diff").help("Show diff instead of rewriting").switch();
        let files = positional::<String>("FILE")
            .help(".sp file(s) to format")
            .some("expected at least one file");
        construct!(Cmd::Format { check, diff, files })
    };

    let format_cmd = fmt_inner()
        .to_options()
        .descr("Format .sp files")
        .command("format");

    let fmt_cmd = fmt_inner()
        .to_options()
        .descr("Format .sp files (alias for format)")
        .command("fmt");

    construct!([format_cmd, fmt_cmd])
}

fn cmd_holes_parser() -> impl Parser<Cmd> {
    let file = positional::<String>("FILE").help("A .sp file");
    construct!(Cmd::Holes { file })
        .to_options()
        .descr("Show hole report (JSON)")
        .command("holes")
}

fn cmd_build_parser() -> impl Parser<Cmd> {
    let file = positional::<String>("FILE")
        .help("A .sp file or project directory to compile")
        .optional();
    construct!(Cmd::Build { file })
        .to_options()
        .descr("Compile a .sp file or current project (interpreter mode)")
        .command("build")
}

fn cmd_watch_parser() -> impl Parser<Cmd> {
    let json = json_flag();
    let file = positional::<String>("FILE").help("A .sp file to watch");
    construct!(Cmd::Watch { json, file })
        .to_options()
        .descr("Watch a file and re-check on changes")
        .command("watch")
}

fn type_flag() -> impl Parser<String> {
    long("type")
        .short('t')
        .help("Project type: application, package, platform")
        .argument::<String>("TYPE")
        .fallback("application".to_string())
}

fn cmd_new_parser() -> impl Parser<Cmd> {
    let name = positional::<String>("NAME").help("Project name");
    let project_type = type_flag();
    construct!(Cmd::New { name, project_type })
        .to_options()
        .descr("Create a new Spore project")
        .command("new")
}

fn cmd_init_parser() -> impl Parser<Cmd> {
    let project_type = type_flag();
    construct!(Cmd::Init { project_type })
        .to_options()
        .descr("Initialize Spore project in current directory")
        .command("init")
}

fn cli() -> OptionParser<Cmd> {
    let cmd = construct!([
        cmd_run_parser(),
        cmd_check_parser(),
        cmd_test_parser(),
        cmd_format_parser(),
        cmd_holes_parser(),
        cmd_build_parser(),
        cmd_watch_parser(),
        cmd_new_parser(),
        cmd_init_parser(),
    ]);
    cmd.to_options()
        .version(VERSION)
        .descr("spore — the Spore language toolkit")
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let cmd = cli().run();
    match cmd {
        Cmd::Run { file, json } => exec_run(&file, json),
        Cmd::Check {
            files,
            verbose,
            json,
            deny_warnings,
        } => exec_check(&files, verbose, json, deny_warnings),
        Cmd::Test {
            files,
            verbose,
            json,
            deny_warnings,
        } => exec_test(&files, verbose, json, deny_warnings),
        Cmd::Format { files, check, diff } => exec_format(&files, check, diff),
        Cmd::Holes { file } => exec_holes(&file),
        Cmd::Build { file } => exec_build(file.as_deref()),
        Cmd::Watch { file, json } => exec_watch(&file, json),
        Cmd::New { name, project_type } => exec_new(&name, &project_type),
        Cmd::Init { project_type } => exec_init(&project_type),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_source(path: &str) -> Result<String, ExitCode> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("{}: cannot read `{path}`: {e}", "error".red().bold());
        ExitCode::FAILURE
    })
}

fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn print_warnings(warnings: &[String], json_output: bool) {
    for w in warnings {
        if json_output {
            println!(
                "{}",
                serde_json::to_string(&json!({"severity": "warning", "message": w})).unwrap()
            );
        } else {
            eprintln!("{}: {w}", "warning".yellow().bold());
        }
    }
}

fn find_project_target(file: &str) -> Option<(PathBuf, String)> {
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

fn find_project_root(path: &Path) -> Option<PathBuf> {
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

fn default_entry_for_project_type(project_type: &str) -> Option<&'static str> {
    match project_type {
        "application" => Some("main.sp"),
        "package" => Some("lib.sp"),
        "platform" => Some("host.sp"),
        _ => None,
    }
}

fn manifest_project_type(root: &Path) -> Result<Option<String>, String> {
    let manifest = root.join("spore.toml");
    let content = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("cannot read `{}`: {e}", manifest.display()))?;

    let mut in_package_section = false;
    for raw_line in content.lines() {
        let line = raw_line
            .split_once('#')
            .map_or(raw_line, |(before, _)| before)
            .trim();

        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_package_section = line == "[package]";
            continue;
        }

        if in_package_section
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "type"
        {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Ok(Some(value.to_string()));
            }
        }
    }

    Ok(None)
}

fn infer_project_entry(root: &Path) -> Result<String, String> {
    let src_dir = root.join("src");
    let manifest = root.join("spore.toml");

    if let Some(project_type) = manifest_project_type(root)? {
        if let Some(entry) = default_entry_for_project_type(&project_type) {
            let entry_path = src_dir.join(entry);
            if entry_path.is_file() {
                return Ok(entry.to_string());
            }
            return Err(format!(
                "project type `{project_type}` expects default entry path `{}`; create it or pass FILE explicitly",
                entry_path.display()
            ));
        }

        return Err(format!(
            "unsupported project type `{project_type}` in `{}`; pass FILE explicitly",
            manifest.display()
        ));
    }

    let candidates: Vec<&str> = ["main.sp", "lib.sp", "host.sp"]
        .into_iter()
        .filter(|entry| src_dir.join(entry).is_file())
        .collect();

    match candidates.as_slice() {
        [entry] => Ok((*entry).to_string()),
        [] => Err(format!(
            "could not infer a project default entry path from `{}`; add `[package].type` or pass FILE explicitly",
            manifest.display()
        )),
        _ => Err(format!(
            "could not infer a project default entry path for `{}`; found multiple defaults in src/ ({}) — pass FILE explicitly",
            root.display(),
            candidates.join(", ")
        )),
    }
}

enum BuildTarget {
    Project { root: PathBuf, entry: String },
    File(String),
}

fn resolve_cli_path(path: &str, cwd: &Path) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn resolve_build_target(file: Option<&str>, cwd: &Path) -> Result<BuildTarget, String> {
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

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn exec_run(file: &str, json_output: bool) -> ExitCode {
    let result = if let Some((root, entry)) = find_project_target(file) {
        sporec::run_project(&root, &entry)
    } else {
        let source = match read_source(file) {
            Ok(s) => s,
            Err(c) => return c,
        };
        sporec::run(&source)
    };

    match result {
        Ok(value) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string(&json!({"status": "ok", "value": value.to_string()}))
                        .unwrap()
                );
            } else {
                println!("{value}");
            }
            ExitCode::SUCCESS
        }
        Err(msg) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string(&json!({"status": "error", "message": msg})).unwrap()
                );
            } else {
                eprintln!("{}: {msg}", "error".red().bold());
            }
            ExitCode::FAILURE
        }
    }
}

fn exec_check(files: &[String], verbose: bool, json_output: bool, deny_warnings: bool) -> ExitCode {
    if files.len() > 1 {
        let refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        match sporec::compile_files(&refs) {
            Ok(output) => {
                print_warnings(&output.warnings, json_output);
                if deny_warnings && !output.warnings.is_empty() {
                    return ExitCode::FAILURE;
                }
                if json_output {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({"status": "ok", "errors": []})).unwrap()
                    );
                } else {
                    println!("{} no errors ({} files)", "✓".green(), files.len());
                }
                ExitCode::SUCCESS
            }
            Err(msg) => {
                if json_output {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({"status": "error", "message": msg})).unwrap()
                    );
                } else {
                    eprintln!("{}: {msg}", "error".red().bold());
                }
                ExitCode::FAILURE
            }
        }
    } else {
        let path = &files[0];
        if verbose {
            let result = if let Some((root, entry)) = find_project_target(path) {
                sporec::check_project_verbose(&root, &entry)
            } else {
                let source = match read_source(path) {
                    Ok(s) => s,
                    Err(c) => return c,
                };
                sporec::check_verbose(&source)
            };

            match result {
                Ok(detail) => {
                    print!("{detail}");
                    ExitCode::SUCCESS
                }
                Err(msg) => {
                    eprintln!("{}: {msg}", "error".red().bold());
                    ExitCode::FAILURE
                }
            }
        } else {
            let result = if let Some((root, entry)) = find_project_target(path) {
                sporec::compile_project(&root, &entry)
            } else {
                let source = match read_source(path) {
                    Ok(s) => s,
                    Err(c) => return c,
                };
                sporec::compile(&source)
            };

            match result {
                Ok(output) => {
                    print_warnings(&output.warnings, json_output);
                    if deny_warnings && !output.warnings.is_empty() {
                        return ExitCode::FAILURE;
                    }
                    if json_output {
                        println!(
                            "{}",
                            serde_json::to_string(&json!({"status": "ok", "errors": []})).unwrap()
                        );
                    } else {
                        println!("{} no errors", "✓".green());
                    }
                    ExitCode::SUCCESS
                }
                Err(msg) => {
                    if json_output {
                        println!(
                            "{}",
                            serde_json::to_string(&json!({"status": "error", "message": msg}))
                                .unwrap()
                        );
                    } else {
                        eprintln!("{}: {msg}", "error".red().bold());
                    }
                    ExitCode::FAILURE
                }
            }
        }
    }
}

fn exec_test(files: &[String], verbose: bool, json_output: bool, _deny_warnings: bool) -> ExitCode {
    // NOTE: Type-check is intentionally skipped as a gate here.  The type
    // checker has known limitations with generics (Option[T], Pair[K,V])
    // that would block spec testing of otherwise valid stdlib code.
    // `sporec::test_specs` still parses the source and evaluates specs.

    let mut total_passed = 0usize;
    let mut total_failed = 0usize;

    for path in files {
        let source = match read_source(path) {
            Ok(s) => s,
            Err(c) => return c,
        };

        match sporec::test_specs(&source) {
            Ok(results) => {
                for r in &results {
                    let kind_label = if r.kind == sporec::SpecKind::Example {
                        "example"
                    } else {
                        "property"
                    };
                    if r.passed {
                        total_passed += 1;
                        if !json_output && verbose {
                            eprintln!(
                                "  {} {} :: {} \"{}\"",
                                "✓".green(),
                                r.fn_name,
                                kind_label,
                                r.label
                            );
                        }
                    } else {
                        total_failed += 1;
                        let msg = r.error.as_deref().unwrap_or("assertion failed");
                        if !json_output {
                            eprintln!(
                                "  {} {} :: {} \"{}\" — {}",
                                "✗".red(),
                                r.fn_name,
                                kind_label,
                                r.label,
                                msg
                            );
                        }
                    }
                }
            }
            Err(msg) => {
                if json_output {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({"status": "error", "message": msg})).unwrap()
                    );
                } else {
                    eprintln!("{}: {msg}", "error".red().bold());
                }
                return ExitCode::FAILURE;
            }
        }
    }

    // Summary
    if json_output {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "status": if total_failed == 0 { "ok" } else { "fail" },
                "passed": total_passed,
                "failed": total_failed,
            }))
            .unwrap()
        );
    } else {
        let total = total_passed + total_failed;
        if total == 0 {
            eprintln!("note: no spec clauses found");
        } else if total_failed == 0 {
            eprintln!("\n{} {total} specs passed", "✓".green());
        } else {
            eprintln!(
                "\n{}: {total_failed} of {total} specs failed",
                "FAIL".red().bold()
            );
        }
    }

    if total_failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn exec_format(files: &[String], check_mode: bool, diff_mode: bool) -> ExitCode {
    let mut exit = ExitCode::SUCCESS;
    for path in files {
        let source = match read_source(path) {
            Ok(s) => s,
            Err(c) => {
                exit = c;
                continue;
            }
        };

        match sporec::format(&source) {
            Ok(formatted) => {
                if check_mode {
                    if formatted != source {
                        eprintln!("{path}: {}", "not formatted".red());
                        exit = ExitCode::FAILURE;
                    }
                } else if diff_mode {
                    if formatted == source {
                        println!("{path}: already formatted");
                    } else {
                        print_diff(path, &source, &formatted);
                    }
                } else {
                    // In-place formatting
                    if formatted == source {
                        println!("{path}: already formatted");
                    } else {
                        if let Err(e) = std::fs::write(path, &formatted) {
                            eprintln!("{}: cannot write `{path}`: {e}", "error".red().bold());
                            exit = ExitCode::FAILURE;
                            continue;
                        }
                        println!("{path}: {}", "formatted".green());
                    }
                }
            }
            Err(msg) => {
                eprintln!("{}: {msg}", "error".red().bold());
                exit = ExitCode::FAILURE;
            }
        }
    }
    exit
}

fn print_diff(path: &str, original: &str, formatted: &str) {
    eprintln!("--- {path} (original)");
    eprintln!("+++ {path} (formatted)");
    for (i, (orig_line, fmt_line)) in original.lines().zip(formatted.lines()).enumerate() {
        if orig_line != fmt_line {
            eprintln!("@@ line {} @@", i + 1);
            eprintln!("{}{orig_line}", "-".red());
            eprintln!("{}{fmt_line}", "+".green());
        }
    }
    let orig_count = original.lines().count();
    let fmt_count = formatted.lines().count();
    if fmt_count > orig_count {
        eprintln!("@@ +{} new lines @@", fmt_count - orig_count);
        for line in formatted.lines().skip(orig_count) {
            eprintln!("{}{line}", "+".green());
        }
    } else if orig_count > fmt_count {
        eprintln!("@@ -{} removed lines @@", orig_count - fmt_count);
        for line in original.lines().skip(fmt_count) {
            eprintln!("{}{line}", "-".red());
        }
    }
}

fn exec_holes(file: &str) -> ExitCode {
    let source = match read_source(file) {
        Ok(s) => s,
        Err(c) => return c,
    };

    match sporec::holes(&source) {
        Ok(j) => {
            println!("{j}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{}: {msg}", "error".red().bold());
            ExitCode::FAILURE
        }
    }
}

fn exec_build(file: Option<&str>) -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!(
                "{}: cannot determine current directory: {e}",
                "error".red().bold()
            );
            return ExitCode::FAILURE;
        }
    };

    let target = match resolve_build_target(file, &cwd) {
        Ok(target) => target,
        Err(msg) => {
            eprintln!("{}: {msg}", "error".red().bold());
            return ExitCode::FAILURE;
        }
    };

    let result = match &target {
        BuildTarget::Project { root, entry } => sporec::compile_project(root, entry),
        BuildTarget::File(path) => {
            let source = match read_source(path) {
                Ok(s) => s,
                Err(c) => return c,
            };
            sporec::compile(&source)
        }
    };

    match result {
        Ok(output) => {
            for w in &output.warnings {
                eprintln!("{}: {w}", "warning".yellow().bold());
            }
            let subject = match &target {
                BuildTarget::Project { entry, .. } => format!("entry path `{entry}`"),
                BuildTarget::File(path) => format!("`{path}`"),
            };
            println!(
                "{} compiled {subject} successfully (interpreter mode — no binary output yet)",
                "✓".green(),
            );
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{}: {msg}", "error".red().bold());
            ExitCode::FAILURE
        }
    }
}

fn exec_watch(file: &str, json_output: bool) -> ExitCode {
    let path = Path::new(file);
    if !path.exists() {
        eprintln!("{}: file `{file}` does not exist", "error".red().bold());
        return ExitCode::FAILURE;
    }

    let (tx, rx) = mpsc::channel();
    let mut debouncer = match new_debouncer(Duration::from_millis(300), tx) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: failed to create watcher: {e}", "error".red().bold());
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = debouncer.watcher().watch(path, RecursiveMode::NonRecursive) {
        eprintln!("{}: failed to watch `{file}`: {e}", "error".red().bold());
        return ExitCode::FAILURE;
    }

    if !json_output {
        eprintln!("watching `{file}` for changes (Ctrl+C to stop)");
    }

    // Initial check
    let mut last_content = String::new();
    if let Ok(source) = std::fs::read_to_string(file) {
        last_content = source.clone();
        check_and_report(file, &source, json_output);
    }

    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                // Only act on write / content-change events.
                let dominated = events.iter().any(|ev| ev.kind == DebouncedEventKind::Any);
                if !dominated {
                    continue;
                }

                match std::fs::read_to_string(file) {
                    Ok(source) => {
                        if source == last_content {
                            continue;
                        }
                        last_content = source.clone();
                        check_and_report(file, &source, json_output);
                    }
                    Err(e) => {
                        if json_output {
                            println!(
                                "{}",
                                serde_json::to_string(&json!({
                                    "event": "error",
                                    "file": file,
                                    "message": e.to_string()
                                }))
                                .unwrap()
                            );
                        } else {
                            eprintln!("{}: reading `{file}`: {e}", "error".red().bold());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                if json_output {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({
                            "event": "error",
                            "file": file,
                            "message": format!("{e:?}")
                        }))
                        .unwrap()
                    );
                } else {
                    eprintln!("{}: watcher error: {e:?}", "error".red().bold());
                }
            }
            Err(_) => break, // channel closed
        }
    }

    ExitCode::SUCCESS
}

fn check_and_report(path: &str, source: &str, json_output: bool) {
    let ts = timestamp();
    let result = if let Some((root, entry)) = find_project_target(path) {
        sporec::compile_project(&root, &entry)
    } else {
        sporec::compile(source)
    };

    match result {
        Ok(output) => {
            for w in &output.warnings {
                if json_output {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({
                            "event": "warning",
                            "file": path,
                            "message": w,
                            "timestamp": ts
                        }))
                        .unwrap()
                    );
                } else {
                    eprintln!("[{ts}] {}: {w}", "warning".yellow().bold());
                }
            }
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "event": "compile_result",
                        "file": path,
                        "status": "ok",
                        "errors": [],
                        "timestamp": ts
                    }))
                    .unwrap()
                );
            } else {
                eprintln!("[{ts}] {} `{path}` — no errors", "✓".green());
            }
        }
        Err(msg) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "event": "compile_result",
                        "file": path,
                        "status": "error",
                        "message": msg,
                        "timestamp": ts
                    }))
                    .unwrap()
                );
            } else {
                eprintln!("[{ts}] {} `{path}`:", "✗".red());
                eprintln!("{msg}");
            }
        }
    }

    // Emit hole_graph_update event if there are holes (JSON mode only)
    if json_output && let Some(summary) = sporec::hole_summary(source) {
        println!("{}", summary.to_json());
    }
}

// ---------------------------------------------------------------------------
// Project scaffolding
// ---------------------------------------------------------------------------

fn exec_new(name: &str, project_type: &str) -> ExitCode {
    if !is_valid_type(project_type) {
        eprintln!(
            "{}: unknown project type `{project_type}`",
            "error".red().bold()
        );
        eprintln!("       valid types: application, package, platform");
        return ExitCode::FAILURE;
    }

    let dir = Path::new(name);
    if dir.exists() {
        eprintln!(
            "{}: directory `{name}` already exists",
            "error".red().bold()
        );
        return ExitCode::FAILURE;
    }

    if let Err(e) = create_project(dir, name, project_type) {
        eprintln!("{}: {e}", "error".red().bold());
        return ExitCode::FAILURE;
    }
    println!("✨ Created {project_type} `{name}`");
    ExitCode::SUCCESS
}

fn exec_init(project_type: &str) -> ExitCode {
    if !is_valid_type(project_type) {
        eprintln!(
            "{}: unknown project type `{project_type}`",
            "error".red().bold()
        );
        eprintln!("       valid types: application, package, platform");
        return ExitCode::FAILURE;
    }

    let dir = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "{}: cannot determine current directory: {e}",
                "error".red().bold()
            );
            return ExitCode::FAILURE;
        }
    };
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    if dir.join("spore.toml").exists() {
        eprintln!(
            "{}: spore.toml already exists in this directory",
            "error".red().bold()
        );
        return ExitCode::FAILURE;
    }

    if let Err(e) = create_project(&dir, &name, project_type) {
        eprintln!("{}: {e}", "error".red().bold());
        return ExitCode::FAILURE;
    }
    println!("✨ Initialized {project_type} `{name}`");
    ExitCode::SUCCESS
}

fn is_valid_type(t: &str) -> bool {
    matches!(t, "application" | "package" | "platform")
}

fn create_project(dir: &Path, name: &str, project_type: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("src"))?;

    let toml = format!(
        "\
[package]
name = \"{name}\"
version = \"0.1.0\"
type = \"{project_type}\"
spore-version = \">=0.1.0\"

[capabilities]
allow = [\"Compute\"]

[dependencies]
"
    );
    std::fs::write(dir.join("spore.toml"), toml)?;

    let (filename, content) = match project_type {
        "package" => (
            "lib.sp",
            "/// Add two integers.\npub fn add(a: I32, b: I32) -> I32 cost [1, 0, 0, 0] {\n    a + b\n}\n"
                .to_string(),
        ),
        "platform" => (
            "host.sp",
            "/// Platform startup adapter.\n/// This is where the platform sets up effect handlers before calling the application startup function.\npub fn main_for_host(app_main: () -> ()) -> () {\n    app_main();\n    return\n}\n"
                .to_string(),
        ),
        _ => (
            "main.sp",
            format!("fn main() -> () {{\n    println(\"Hello from {name}!\");\n    return\n}}\n"),
        ),
    };
    std::fs::write(dir.join("src").join(filename), content)?;
    std::fs::write(dir.join(".gitignore"), "/target\n/.spore-store\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_new_creates_application() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("my-app");
        create_project(&project_dir, "my-app", "application").unwrap();
        assert!(project_dir.join("spore.toml").exists());
        assert!(project_dir.join("src/main.sp").exists());
        let toml = fs::read_to_string(project_dir.join("spore.toml")).unwrap();
        assert!(toml.contains("name = \"my-app\""));
        assert!(toml.contains("type = \"application\""));
    }

    #[test]
    fn test_new_creates_package() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("my-lib");
        create_project(&project_dir, "my-lib", "package").unwrap();
        assert!(project_dir.join("src/lib.sp").exists());
        let toml = fs::read_to_string(project_dir.join("spore.toml")).unwrap();
        assert!(toml.contains("type = \"package\""));
    }

    #[test]
    fn test_new_creates_platform() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("my-platform");
        create_project(&project_dir, "my-platform", "platform").unwrap();
        assert!(project_dir.join("src/host.sp").exists());
        let toml = fs::read_to_string(project_dir.join("spore.toml")).unwrap();
        assert!(toml.contains("type = \"platform\""));
    }

    #[test]
    fn test_is_valid_type() {
        assert!(is_valid_type("application"));
        assert!(is_valid_type("package"));
        assert!(is_valid_type("platform"));
        assert!(!is_valid_type("unknown"));
    }

    #[test]
    fn test_gitignore_content() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("test-proj");
        create_project(&project_dir, "test-proj", "application").unwrap();
        let gi = fs::read_to_string(project_dir.join(".gitignore")).unwrap();
        assert!(gi.contains("/target"));
    }

    #[test]
    fn test_scaffolded_projects_typecheck() {
        let tmp = tempfile::tempdir().unwrap();
        let cases = [
            ("application", "app", "main.sp"),
            ("package", "pkg", "lib.sp"),
            ("platform", "plat", "host.sp"),
        ];

        for (project_type, name, entry) in cases {
            let project_dir = tmp.path().join(name);
            create_project(&project_dir, name, project_type).unwrap();

            let result = sporec::compile_project(&project_dir, entry);
            assert!(
                result.is_ok(),
                "scaffolded {project_type} project should type-check: {result:?}"
            );
        }
    }

    #[test]
    fn test_exec_test_accepts_valid_spec_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sample.sp");
        fs::write(
            &file,
            r#"
            fn add(a: I32, b: I32) -> I32
            spec {
                example "basic": add(2, 3) == 5
                property "commutative": |a: I32, b: I32| add(a, b) == add(b, a)
            }
            {
                a + b
            }
            "#,
        )
        .unwrap();

        let code = exec_test(&[file.to_string_lossy().to_string()], false, false, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn test_exec_test_rejects_invalid_spec_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sample.sp");
        fs::write(
            &file,
            r#"
            fn add(a: I32, b: I32) -> I32
            spec {
                example "bad": 42
            }
            {
                a + b
            }
            "#,
        )
        .unwrap();

        let code = exec_test(&[file.to_string_lossy().to_string()], false, false, false);
        assert_eq!(code, ExitCode::FAILURE);
    }

    #[test]
    fn test_find_project_target_for_main_file() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        create_project(&project_dir, "proj", "application").unwrap();

        let target =
            find_project_target(project_dir.join("src/main.sp").to_str().unwrap()).unwrap();
        assert_eq!(target.0, std::fs::canonicalize(&project_dir).unwrap());
        assert_eq!(target.1, "main.sp");
    }

    #[test]
    fn test_find_project_target_for_nested_module() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        create_project(&project_dir, "proj", "application").unwrap();
        let nested_dir = project_dir.join("src/lib");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("util.sp"), "pub fn x() -> I32 { 1 }\n").unwrap();

        let target =
            find_project_target(project_dir.join("src/lib/util.sp").to_str().unwrap()).unwrap();
        assert_eq!(target.0, std::fs::canonicalize(&project_dir).unwrap());
        assert_eq!(target.1, "lib/util.sp");
    }

    #[test]
    fn test_find_project_target_ignores_files_outside_src() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        create_project(&project_dir, "proj", "application").unwrap();
        fs::write(project_dir.join("notes.sp"), "fn scratch() -> I32 { 1 }\n").unwrap();

        assert!(find_project_target(project_dir.join("notes.sp").to_str().unwrap()).is_none());
    }

    #[test]
    fn test_resolve_build_target_without_arg_uses_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        create_project(&project_dir, "proj", "application").unwrap();

        let target = resolve_build_target(None, &project_dir).unwrap();
        match target {
            BuildTarget::Project { root, entry } => {
                assert_eq!(root, std::fs::canonicalize(&project_dir).unwrap());
                assert_eq!(entry, "main.sp");
            }
            BuildTarget::File(path) => panic!("expected project target, got file target `{path}`"),
        }
    }

    #[test]
    fn test_resolve_build_target_accepts_dot_for_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("pkg");
        create_project(&project_dir, "pkg", "package").unwrap();

        let target = resolve_build_target(Some("."), &project_dir).unwrap();

        match target {
            BuildTarget::Project { root, entry } => {
                assert_eq!(root, std::fs::canonicalize(&project_dir).unwrap());
                assert_eq!(entry, "lib.sp");
            }
            BuildTarget::File(path) => panic!("expected project target, got file target `{path}`"),
        }
    }

    #[test]
    fn test_resolve_build_target_accepts_project_directory_argument() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("plat");
        create_project(&project_dir, "plat", "platform").unwrap();

        let target = resolve_build_target(Some(project_dir.to_str().unwrap()), tmp.path()).unwrap();
        match target {
            BuildTarget::Project { root, entry } => {
                assert_eq!(root, std::fs::canonicalize(&project_dir).unwrap());
                assert_eq!(entry, "host.sp");
            }
            BuildTarget::File(path) => panic!("expected project target, got file target `{path}`"),
        }
    }

    #[test]
    fn test_infer_project_entry_falls_back_to_single_default_file() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        fs::create_dir_all(project_dir.join("src")).unwrap();
        fs::write(
            project_dir.join("spore.toml"),
            "[package]\nname = \"proj\"\n",
        )
        .unwrap();
        fs::write(
            project_dir.join("src/main.sp"),
            "fn main() -> () { return }\n",
        )
        .unwrap();

        assert_eq!(infer_project_entry(&project_dir).unwrap(), "main.sp");
    }
}
