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

fn read_source_message(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read `{path}`: {e}"))
}

fn read_source(path: &str) -> Result<String, ExitCode> {
    read_source_message(path).map_err(|message| fail_human(&message))
}

fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn output_mode(json_output: bool) -> bool {
    json_output
}

fn fail_message(message: &str, json_output: bool) -> ExitCode {
    sporec_diagnostics::exit_with_message_error(message, output_mode(json_output))
}

fn fail_human(message: &str) -> ExitCode {
    sporec_diagnostics::exit_with_message_error(message, false)
}

fn print_warnings(warnings: &[String], json_output: bool) {
    for w in warnings {
        sporec_diagnostics::emit_warning_message(w, output_mode(json_output));
    }
}

fn fail_deny_warnings(
    warnings: &[String],
    warning_diagnostics: Option<&[sporec_diagnostics::Diagnostic]>,
    json_output: bool,
) -> ExitCode {
    if json_output {
        let mut payload = json!({
            "status": "error",
            "message": "warnings are denied",
            "warnings": warnings,
        });
        if let Some(diagnostics) = warning_diagnostics {
            payload["warning_diagnostics"] =
                serde_json::to_value(diagnostics).expect("serialize warning diagnostics");
        }
        sporec_diagnostics::print_json(&payload);
        ExitCode::FAILURE
    } else {
        fail_human("warnings are denied")
    }
}

fn report_single_file_check(
    path: &str,
    source: &str,
    json_output: bool,
    deny_warnings: bool,
    human_success_message: &str,
) -> ExitCode {
    match sporec::check_source_file(path, source) {
        sporec::SourceCheckReport::Success { source, warnings } => {
            let has_warnings = !warnings.is_empty();
            let warning_messages = sporec_diagnostics::diagnostic_message_lines(&warnings);
            if json_output {
                for (warning, message) in warnings.iter().zip(warning_messages.iter()) {
                    sporec_diagnostics::print_json(&json!({
                        "severity": "warning",
                        "message": message,
                        "diagnostic": warning,
                    }));
                }
                if has_warnings && deny_warnings {
                    return fail_deny_warnings(&warning_messages, Some(&warnings), true);
                }

                sporec_diagnostics::print_json(&json!({"status": "ok", "errors": []}));
            } else {
                sporec_diagnostics::render_diagnostics_human(&source, &warnings);
                if has_warnings && deny_warnings {
                    return fail_deny_warnings(&warning_messages, Some(&warnings), false);
                }
                println!("{human_success_message}");
            }

            ExitCode::SUCCESS
        }
        sporec::SourceCheckReport::Failure(sporec::SourceCheckFailure::Diagnostics {
            source,
            diagnostics,
        }) => sporec_diagnostics::exit_with_diagnostics_error(
            &source,
            &diagnostics,
            output_mode(json_output),
        ),
        sporec::SourceCheckReport::Failure(sporec::SourceCheckFailure::Message(message)) => {
            fail_message(&message, json_output)
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

fn infer_project_entry(root: &Path) -> Result<String, String> {
    sporec::resolve_default_project_target(root).map(|target| target.entry_path)
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
        let source = match read_source_message(file) {
            Ok(s) => s,
            Err(message) => return fail_message(&message, json_output),
        };
        sporec::run(&source)
    };

    match result {
        Ok(value) => {
            if json_output {
                sporec_diagnostics::print_json(
                    &json!({"status": "ok", "value": value.to_string()}),
                );
            } else {
                println!("{value}");
            }
            ExitCode::SUCCESS
        }
        Err(msg) => fail_message(&msg, json_output),
    }
}

fn exec_check(files: &[String], verbose: bool, json_output: bool, deny_warnings: bool) -> ExitCode {
    if files.len() > 1 {
        let refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        match sporec::compile_files(&refs) {
            Ok(output) => {
                print_warnings(&output.warnings, json_output);
                if deny_warnings && !output.warnings.is_empty() {
                    return fail_deny_warnings(&output.warnings, None, json_output);
                }
                if json_output {
                    sporec_diagnostics::print_json(&json!({"status": "ok", "errors": []}));
                } else {
                    println!("{} no errors ({} files)", "✓".green(), files.len());
                }
                ExitCode::SUCCESS
            }
            Err(msg) => fail_message(&msg, json_output),
        }
    } else {
        let path = &files[0];
        if verbose {
            let result = if let Some((root, entry)) = find_project_target(path) {
                if deny_warnings {
                    match sporec::compile_project(&root, &entry) {
                        Ok(output) => {
                            if !output.warnings.is_empty() {
                                print_warnings(&output.warnings, false);
                                return fail_deny_warnings(&output.warnings, None, false);
                            }
                        }
                        Err(msg) => return fail_human(&msg),
                    }
                }
                sporec::check_project_verbose(&root, &entry)
            } else {
                let source = match read_source(path) {
                    Ok(s) => s,
                    Err(c) => return c,
                };
                if deny_warnings {
                    match sporec::check_source_file(path, &source) {
                        sporec::SourceCheckReport::Success {
                            source: canonical_source,
                            warnings,
                        } => {
                            if !warnings.is_empty() {
                                let warning_messages =
                                    sporec_diagnostics::diagnostic_message_lines(&warnings);
                                sporec_diagnostics::render_diagnostics_human(
                                    &canonical_source,
                                    &warnings,
                                );
                                return fail_deny_warnings(
                                    &warning_messages,
                                    Some(&warnings),
                                    false,
                                );
                            }
                        }
                        sporec::SourceCheckReport::Failure(
                            sporec::SourceCheckFailure::Diagnostics {
                                source,
                                diagnostics,
                            },
                        ) => {
                            return sporec_diagnostics::exit_with_diagnostics_error(
                                &source,
                                &diagnostics,
                                false,
                            );
                        }
                        sporec::SourceCheckReport::Failure(
                            sporec::SourceCheckFailure::Message(message),
                        ) => return fail_human(&message),
                    }
                }
                sporec::check_verbose(&source)
            };

            match result {
                Ok(detail) => {
                    print!("{detail}");
                    ExitCode::SUCCESS
                }
                Err(msg) => fail_human(&msg),
            }
        } else {
            if let Some((root, entry)) = find_project_target(path) {
                match sporec::compile_project(&root, &entry) {
                    Ok(output) => {
                        print_warnings(&output.warnings, json_output);
                        if deny_warnings && !output.warnings.is_empty() {
                            return fail_deny_warnings(&output.warnings, None, json_output);
                        }
                        if json_output {
                            sporec_diagnostics::print_json(&json!({"status": "ok", "errors": []}));
                        } else {
                            println!("{} no errors", "✓".green());
                        }
                        ExitCode::SUCCESS
                    }
                    Err(msg) => fail_message(&msg, json_output),
                }
            } else {
                let source = match read_source_message(path) {
                    Ok(s) => s,
                    Err(message) => return fail_message(&message, json_output),
                };
                report_single_file_check(path, &source, json_output, deny_warnings, "✓ no errors")
            }
        }
    }
}

fn exec_test(files: &[String], verbose: bool, json_output: bool, deny_warnings: bool) -> ExitCode {
    let mut total_passed = 0usize;
    let mut total_failed = 0usize;

    for path in files {
        let source = match read_source_message(path) {
            Ok(s) => s,
            Err(message) => return fail_message(&message, json_output),
        };

        match sporec::check_source_file(path, &source) {
            sporec::SourceCheckReport::Success {
                source: canonical_source,
                warnings,
            } => {
                let warning_messages = sporec_diagnostics::diagnostic_message_lines(&warnings);
                if json_output {
                    for (warning, message) in warnings.iter().zip(warning_messages.iter()) {
                        sporec_diagnostics::print_json(&json!({
                            "severity": "warning",
                            "message": message,
                            "diagnostic": warning,
                        }));
                    }
                } else {
                    sporec_diagnostics::render_diagnostics_human(&canonical_source, &warnings);
                }

                if !warnings.is_empty() && deny_warnings {
                    return fail_deny_warnings(&warning_messages, Some(&warnings), json_output);
                }
            }
            sporec::SourceCheckReport::Failure(sporec::SourceCheckFailure::Diagnostics {
                source,
                diagnostics,
            }) => {
                return sporec_diagnostics::exit_with_diagnostics_error(
                    &source,
                    &diagnostics,
                    json_output,
                );
            }
            sporec::SourceCheckReport::Failure(sporec::SourceCheckFailure::Message(message)) => {
                return fail_message(&message, json_output);
            }
        }

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
            Err(msg) => return fail_message(&msg, json_output),
        }
    }

    // Summary
    if json_output {
        sporec_diagnostics::print_json(&json!({
            "status": if total_failed == 0 { "ok" } else { "fail" },
            "passed": total_passed,
            "failed": total_failed,
        }));
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
        Err(e) => return fail_human(&format!("cannot determine current directory: {e}")),
    };

    let target = match resolve_build_target(file, &cwd) {
        Ok(target) => target,
        Err(msg) => return fail_human(&msg),
    };

    match &target {
        BuildTarget::Project { root, entry } => match sporec::compile_project(root, entry) {
            Ok(output) => {
                print_warnings(&output.warnings, false);
                println!(
                    "{} compiled entry path `{entry}` successfully (interpreter mode — no binary output yet)",
                    "✓".green(),
                );
                ExitCode::SUCCESS
            }
            Err(msg) => fail_human(&msg),
        },
        BuildTarget::File(path) => {
            let source = match read_source_message(path) {
                Ok(s) => s,
                Err(message) => return fail_human(&message),
            };
            let success_message = format!(
                "{} compiled `{path}` successfully (interpreter mode — no binary output yet)",
                "✓".green(),
            );
            report_single_file_check(path, &source, false, false, &success_message)
        }
    }
}

fn exec_watch(file: &str, json_output: bool) -> ExitCode {
    let path = Path::new(file);
    if !path.exists() {
        return fail_message(&format!("file `{file}` does not exist"), json_output);
    }

    let (tx, rx) = mpsc::channel();
    let mut debouncer = match new_debouncer(Duration::from_millis(300), tx) {
        Ok(d) => d,
        Err(e) => return fail_message(&format!("failed to create watcher: {e}"), json_output),
    };

    if let Err(e) = debouncer.watcher().watch(path, RecursiveMode::NonRecursive) {
        return fail_message(&format!("failed to watch `{file}`: {e}"), json_output);
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
                            sporec_diagnostics::print_json(&json!({
                                "event": "error",
                                "file": file,
                                "message": e.to_string(),
                            }));
                        } else {
                            eprintln!("{}: reading `{file}`: {e}", "error".red().bold());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                if json_output {
                    sporec_diagnostics::print_json(&json!({
                        "event": "error",
                        "file": file,
                        "message": format!("{e:?}"),
                    }));
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
    if let Some((root, entry)) = find_project_target(path) {
        match sporec::compile_project(&root, &entry) {
            Ok(output) => {
                for w in &output.warnings {
                    if json_output {
                        sporec_diagnostics::print_json(&json!({
                            "event": "warning",
                            "file": path,
                            "message": w,
                            "timestamp": ts,
                        }));
                    } else {
                        eprintln!("[{ts}] {}: {w}", "warning".yellow().bold());
                    }
                }
                if json_output {
                    sporec_diagnostics::print_json(&json!({
                        "event": "compile_result",
                        "file": path,
                        "status": "ok",
                        "errors": [],
                        "timestamp": ts,
                    }));
                } else {
                    eprintln!("[{ts}] {} `{path}` — no errors", "✓".green());
                }
            }
            Err(msg) => {
                if json_output {
                    sporec_diagnostics::print_json(&json!({
                        "event": "compile_result",
                        "file": path,
                        "status": "error",
                        "message": msg,
                        "timestamp": ts,
                    }));
                } else {
                    eprintln!("[{ts}] {} `{path}`:", "✗".red());
                    eprintln!("{msg}");
                }
            }
        }
    } else {
        match sporec::check_source_file(path, source) {
            sporec::SourceCheckReport::Success { source, warnings } => {
                if json_output {
                    for warning in warnings {
                        sporec_diagnostics::print_json(&json!({
                            "event": "warning",
                            "file": path,
                            "message": sporec_diagnostics::diagnostic_message_line(&warning),
                            "diagnostic": warning,
                            "timestamp": ts,
                        }));
                    }
                    sporec_diagnostics::print_json(&json!({
                        "event": "compile_result",
                        "file": path,
                        "status": "ok",
                        "errors": [],
                        "timestamp": ts,
                    }));
                } else {
                    if !warnings.is_empty() {
                        eprintln!("[{ts}] warnings for `{path}`:");
                        sporec_diagnostics::render_diagnostics_human(&source, &warnings);
                    }
                    eprintln!("[{ts}] {} `{path}` — no errors", "✓".green());
                }
            }
            sporec::SourceCheckReport::Failure(sporec::SourceCheckFailure::Diagnostics {
                source,
                diagnostics,
            }) => {
                if json_output {
                    let message =
                        sporec_diagnostics::diagnostic_message_lines(&diagnostics).join("\n");
                    sporec_diagnostics::print_json(&json!({
                        "event": "compile_result",
                        "file": path,
                        "status": "error",
                        "message": message,
                        "diagnostics": diagnostics,
                        "timestamp": ts,
                    }));
                } else {
                    eprintln!("[{ts}] {} `{path}`:", "✗".red());
                    sporec_diagnostics::render_diagnostics_human(&source, &diagnostics);
                }
            }
            sporec::SourceCheckReport::Failure(sporec::SourceCheckFailure::Message(message)) => {
                if json_output {
                    sporec_diagnostics::print_json(&json!({
                        "event": "compile_result",
                        "file": path,
                        "status": "error",
                        "message": message,
                        "timestamp": ts,
                    }));
                } else {
                    eprintln!("[{ts}] {} `{path}`:", "✗".red());
                    eprintln!("{message}");
                }
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

    let manifest_header = format!(
        "\
[package]
name = \"{name}\"
version = \"0.1.0\"
type = \"{project_type}\"
spore-version = \">=0.1.0\"
"
    );
    let project_config = match project_type {
        "application" => {
            "\n[project]\nplatform = \"cli\"\ndefault-entry = \"app\"\n\n[entries.app]\npath = \"main.sp\"\n".to_string()
        }
        "platform" => {
            "\n[project]\nplatform = \"cli\"\ndefault-entry = \"host\"\n\n[entries.host]\npath = \"host.sp\"\n".to_string()
        }
        _ => String::new(),
    };
    let toml = format!(
        "{manifest_header}{project_config}\n[capabilities]\nallow = [\"Compute\"]\n\n[dependencies]\n"
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
            "/// Platform host entry.\n/// This placeholder satisfies the current CLI startup contract while runtime host wiring is still evolving.\npub fn main() -> () {\n    return\n}\n"
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
        assert!(toml.contains("[project]"));
        assert!(toml.contains("default-entry = \"app\""));
        assert!(toml.contains("[entries.app]"));
    }

    #[test]
    fn test_new_creates_package() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("my-lib");
        create_project(&project_dir, "my-lib", "package").unwrap();
        assert!(project_dir.join("src/lib.sp").exists());
        let toml = fs::read_to_string(project_dir.join("spore.toml")).unwrap();
        assert!(toml.contains("type = \"package\""));
        assert!(!toml.contains("[project]"));
    }

    #[test]
    fn test_new_creates_platform() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("my-platform");
        create_project(&project_dir, "my-platform", "platform").unwrap();
        assert!(project_dir.join("src/host.sp").exists());
        let toml = fs::read_to_string(project_dir.join("spore.toml")).unwrap();
        assert!(toml.contains("type = \"platform\""));
        assert!(toml.contains("[project]"));
        assert!(toml.contains("default-entry = \"host\""));
        assert!(toml.contains("[entries.host]"));
        let host = fs::read_to_string(project_dir.join("src/host.sp")).unwrap();
        assert!(host.contains("pub fn main() -> ()"));
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
    fn test_exec_test_rejects_type_errors_before_running_specs() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sample.sp");
        fs::write(
            &file,
            r#"
            fn add(a: I32, b: I32) -> I32
            spec {
                example "basic": add(2, 3) == 5
            }
            {
                "oops"
            }
            "#,
        )
        .unwrap();

        let code = exec_test(&[file.to_string_lossy().to_string()], false, false, false);
        assert_eq!(code, ExitCode::FAILURE);
    }

    #[test]
    fn test_exec_test_denies_warnings_when_requested() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sample.sp");
        fs::write(
            &file,
            r#"
            fn expensive(x: I32) -> I32 cost [100, 0, 0, 0] {
                x + x
            }

            fn cheap(a: I32) -> I32 cost [2, 0, 0, 0]
            spec {
                example "basic": cheap(1) == 4
            }
            {
                expensive(expensive(a))
            }
            "#,
        )
        .unwrap();

        let code = exec_test(&[file.to_string_lossy().to_string()], false, false, true);
        assert_eq!(code, ExitCode::FAILURE);
    }

    #[test]
    fn test_exec_check_verbose_denies_warnings_when_requested() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sample.sp");
        fs::write(
            &file,
            r#"
            fn expensive(x: I32) -> I32 cost [100, 0, 0, 0] {
                x + x
            }

            fn cheap(a: I32) -> I32 cost [2, 0, 0, 0] {
                expensive(expensive(a))
            }
            "#,
        )
        .unwrap();

        let code = exec_check(&[file.to_string_lossy().to_string()], true, false, true);
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
