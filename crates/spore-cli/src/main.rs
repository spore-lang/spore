use std::path::Path;
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
    Format {
        files: Vec<String>,
        check: bool,
        diff: bool,
    },
    Holes {
        file: String,
    },
    Build {
        file: String,
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
    let file = positional::<String>("FILE").help("A .spore file to run");
    construct!(Cmd::Run { json, file })
        .to_options()
        .descr("Compile and execute a .spore file")
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
        .help(".spore file(s) to check")
        .some("expected at least one file");
    construct!(Cmd::Check {
        verbose,
        json,
        deny_warnings,
        files,
    })
    .to_options()
    .descr("Type-check one or more .spore files")
    .command("check")
}

fn cmd_format_parser() -> impl Parser<Cmd> {
    let fmt_inner = || {
        let check = long("check")
            .help("Check if files are formatted (no changes)")
            .switch();
        let diff = long("diff").help("Show diff instead of rewriting").switch();
        let files = positional::<String>("FILE")
            .help(".spore file(s) to format")
            .some("expected at least one file");
        construct!(Cmd::Format { check, diff, files })
    };

    let format_cmd = fmt_inner()
        .to_options()
        .descr("Format .spore files")
        .command("format");

    let fmt_cmd = fmt_inner()
        .to_options()
        .descr("Format .spore files (alias for format)")
        .command("fmt");

    construct!([format_cmd, fmt_cmd])
}

fn cmd_holes_parser() -> impl Parser<Cmd> {
    let file = positional::<String>("FILE").help("A .spore file");
    construct!(Cmd::Holes { file })
        .to_options()
        .descr("Show hole report (JSON)")
        .command("holes")
}

fn cmd_build_parser() -> impl Parser<Cmd> {
    let file = positional::<String>("FILE").help("A .spore file to compile");
    construct!(Cmd::Build { file })
        .to_options()
        .descr("Compile a .spore file")
        .command("build")
}

fn cmd_watch_parser() -> impl Parser<Cmd> {
    let json = json_flag();
    let file = positional::<String>("FILE").help("A .spore file to watch");
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
        Cmd::Format { files, check, diff } => exec_format(&files, check, diff),
        Cmd::Holes { file } => exec_holes(&file),
        Cmd::Build { file } => exec_build(&file),
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

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn exec_run(file: &str, json_output: bool) -> ExitCode {
    let source = match read_source(file) {
        Ok(s) => s,
        Err(c) => return c,
    };

    match sporec::run(&source) {
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
        let source = match read_source(path) {
            Ok(s) => s,
            Err(c) => return c,
        };

        if verbose {
            match sporec::check_verbose(&source) {
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
            match sporec::compile(&source) {
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

fn exec_build(file: &str) -> ExitCode {
    let source = match read_source(file) {
        Ok(s) => s,
        Err(c) => return c,
    };

    match sporec::compile(&source) {
        Ok(output) => {
            for w in &output.warnings {
                eprintln!("{}: {w}", "warning".yellow().bold());
            }
            println!(
                "{} compiled successfully (interpreter mode — no binary output yet)",
                "✓".green()
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
    match sporec::compile(source) {
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
            "/// Add two integers.\npub fn add(a: Int, b: Int) -> Int cost <= 1 =\n    a + b\n"
                .to_string(),
        ),
        "platform" => (
            "host.sp",
            "/// Platform entry point.\n/// The platform provides effect handlers for the application.\npub fn main_for_host(app_main: fn() -> Unit) -> Unit =\n    app_main()\n"
                .to_string(),
        ),
        _ => (
            "main.sp",
            format!("fn main() -> Unit =\n    println(\"Hello from {name}!\")\n"),
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
}
