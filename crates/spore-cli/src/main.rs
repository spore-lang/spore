use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return usage();
    }

    match args[1].as_str() {
        "run" => cmd_run(&args[2..]),
        "check" => cmd_check(&args[2..]),
        "format" | "fmt" => cmd_format(&args[2..]),
        "holes" => cmd_holes(&args[2..]),
        "build" => cmd_build(&args[2..]),
        "watch" => cmd_watch(&args[2..]),
        "--version" | "-V" => {
            println!("spore {VERSION}");
            ExitCode::SUCCESS
        }
        "--help" | "-h" | "help" => usage(),
        other => {
            eprintln!("error: unknown command `{other}`");
            eprintln!();
            usage()
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("spore {VERSION} — the Spore language toolkit");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  spore <COMMAND> [OPTIONS]");
    eprintln!();
    eprintln!("COMMANDS:");
    eprintln!("  run <file>       Compile and execute a .spore file");
    eprintln!("  check <file...>  Type-check one or more .spore files");
    eprintln!("  format <file>    Format a .spore file (--check, --diff)");
    eprintln!("  holes <file>     Show hole report (JSON)");
    eprintln!("  build <file>     Compile a .spore file");
    eprintln!("  watch <file>     Watch a file and re-check on changes");
    eprintln!("  help             Show this help message");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --json             Output results as JSON (run, check, watch)");
    eprintln!("  --verbose          Show detailed type inference and cost info (check)");
    eprintln!("  --deny-warnings    Treat warnings as errors (check)");
    eprintln!("  -V, --version      Print version");
    eprintln!("  -h, --help         Print help");
    ExitCode::FAILURE
}

fn read_source(args: &[String]) -> Result<(String, String), ExitCode> {
    let path = match args.iter().find(|a| !a.starts_with('-')) {
        Some(p) => p.clone(),
        None => {
            eprintln!("error: missing file argument");
            return Err(ExitCode::FAILURE);
        }
    };
    let content = std::fs::read_to_string(&path).map_err(|e| {
        eprintln!("error: cannot read `{path}`: {e}");
        ExitCode::FAILURE
    })?;
    Ok((path, content))
}

fn cmd_run(args: &[String]) -> ExitCode {
    let (_, source) = match read_source(args) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let json_output = args.iter().any(|a| a == "--json");

    match sporec::run(&source) {
        Ok(value) => {
            if json_output {
                println!("{{\"status\":\"ok\",\"value\":\"{value}\"}}");
            } else {
                println!("{value}");
            }
            ExitCode::SUCCESS
        }
        Err(msg) => {
            if json_output {
                let escaped = escape_json(&msg);
                println!("{{\"status\":\"error\",\"message\":\"{escaped}\"}}");
            } else {
                eprintln!("{msg}");
            }
            ExitCode::FAILURE
        }
    }
}

fn cmd_check(args: &[String]) -> ExitCode {
    let files: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .collect();
    let json_output = args.iter().any(|a| a == "--json");
    let verbose = args.iter().any(|a| a == "--verbose");
    let deny_warnings = args.iter().any(|a| a == "--deny-warnings");

    if files.is_empty() {
        eprintln!("error: missing file argument");
        return ExitCode::FAILURE;
    }

    // Multi-file check
    if files.len() > 1 {
        match sporec::compile_files(&files) {
            Ok(output) => {
                print_warnings(&output.warnings, json_output);
                if deny_warnings && !output.warnings.is_empty() {
                    return ExitCode::FAILURE;
                }
                if json_output {
                    println!("{{\"status\":\"ok\",\"errors\":[]}}");
                } else {
                    println!("✓ no errors ({} files)", files.len());
                }
                ExitCode::SUCCESS
            }
            Err(msg) => {
                if json_output {
                    let escaped = escape_json(&msg);
                    println!("{{\"status\":\"error\",\"message\":\"{escaped}\"}}");
                } else {
                    eprintln!("{msg}");
                }
                ExitCode::FAILURE
            }
        }
    } else {
        // Single file check
        let (_, source) = match read_source(args) {
            Ok(s) => s,
            Err(code) => return code,
        };

        if verbose {
            match sporec::check_verbose(&source) {
                Ok(detail) => {
                    print!("{detail}");
                    ExitCode::SUCCESS
                }
                Err(msg) => {
                    eprintln!("{msg}");
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
                        println!("{{\"status\":\"ok\",\"errors\":[]}}");
                    } else {
                        println!("✓ no errors");
                    }
                    ExitCode::SUCCESS
                }
                Err(msg) => {
                    if json_output {
                        let escaped = escape_json(&msg);
                        println!("{{\"status\":\"error\",\"message\":\"{escaped}\"}}");
                    } else {
                        eprintln!("{msg}");
                    }
                    ExitCode::FAILURE
                }
            }
        }
    }
}

/// Print warnings to stderr (or as JSON to stdout).
fn print_warnings(warnings: &[String], json: bool) {
    for w in warnings {
        if json {
            let escaped = escape_json(w);
            println!("{{\"severity\":\"warning\",\"message\":\"{escaped}\"}}");
        } else {
            eprintln!("warning: {w}");
        }
    }
}

fn cmd_holes(args: &[String]) -> ExitCode {
    let (_, source) = match read_source(args) {
        Ok(s) => s,
        Err(code) => return code,
    };

    match sporec::holes(&source) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_format(args: &[String]) -> ExitCode {
    let (path, source) = match read_source(args) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let check_mode = args.iter().any(|a| a == "--check");
    let diff_mode = args.iter().any(|a| a == "--diff");

    match sporec::format(&source) {
        Ok(formatted) => {
            if check_mode {
                if formatted == source {
                    ExitCode::SUCCESS
                } else {
                    eprintln!("{path}: not formatted");
                    ExitCode::FAILURE
                }
            } else if diff_mode {
                if formatted == source {
                    println!("{path}: already formatted");
                } else {
                    print_diff(&path, &source, &formatted);
                }
                ExitCode::SUCCESS
            } else {
                // In-place formatting
                if formatted == source {
                    println!("{path}: already formatted");
                } else {
                    if let Err(e) = std::fs::write(&path, &formatted) {
                        eprintln!("error: cannot write `{path}`: {e}");
                        return ExitCode::FAILURE;
                    }
                    println!("{path}: formatted");
                }
                ExitCode::SUCCESS
            }
        }
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn print_diff(path: &str, original: &str, formatted: &str) {
    eprintln!("--- {path} (original)");
    eprintln!("+++ {path} (formatted)");
    for (i, (orig_line, fmt_line)) in original.lines().zip(formatted.lines()).enumerate() {
        if orig_line != fmt_line {
            eprintln!("@@ line {} @@", i + 1);
            eprintln!("-{orig_line}");
            eprintln!("+{fmt_line}");
        }
    }
    let orig_count = original.lines().count();
    let fmt_count = formatted.lines().count();
    if fmt_count > orig_count {
        eprintln!("@@ +{} new lines @@", fmt_count - orig_count);
        for line in formatted.lines().skip(orig_count) {
            eprintln!("+{line}");
        }
    } else if orig_count > fmt_count {
        eprintln!("@@ -{} removed lines @@", orig_count - fmt_count);
        for line in original.lines().skip(fmt_count) {
            eprintln!("-{line}");
        }
    }
}

fn cmd_build(args: &[String]) -> ExitCode {
    let (_, source) = match read_source(args) {
        Ok(s) => s,
        Err(code) => return code,
    };

    match sporec::compile(&source) {
        Ok(output) => {
            for w in &output.warnings {
                eprintln!("warning: {w}");
            }
            println!("✓ compiled successfully (interpreter mode — no binary output yet)");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_watch(args: &[String]) -> ExitCode {
    let path = match args.iter().find(|a| !a.starts_with('-')) {
        Some(p) => p.clone(),
        None => {
            eprintln!("error: missing file argument");
            return ExitCode::FAILURE;
        }
    };
    let json_output = args.iter().any(|a| a == "--json");

    if !json_output {
        eprintln!("watching `{path}` for changes (Ctrl+C to stop)");
    }

    let mut last_modified = file_modified(&path);
    let mut last_content = String::new();

    loop {
        let current_modified = file_modified(&path);
        if current_modified != last_modified {
            last_modified = current_modified;

            match std::fs::read_to_string(&path) {
                Ok(source) => {
                    if source == last_content {
                        continue;
                    }
                    last_content = source.clone();
                    check_and_report(&path, &source, json_output);
                }
                Err(e) => {
                    if json_output {
                        let escaped = escape_json(&e.to_string());
                        println!(
                            "{{\"event\":\"error\",\"file\":\"{path}\",\"message\":\"{escaped}\"}}"
                        );
                    } else {
                        eprintln!("error reading `{path}`: {e}");
                    }
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

fn check_and_report(path: &str, source: &str, json: bool) {
    let ts = timestamp();
    match sporec::compile(source) {
        Ok(output) => {
            // Emit warnings
            for w in &output.warnings {
                if json {
                    let escaped = escape_json(w);
                    println!(
                        "{{\"event\":\"warning\",\"file\":\"{path}\",\"message\":\"{escaped}\",\"timestamp\":{ts}}}"
                    );
                } else {
                    eprintln!("[{ts}] warning: {w}");
                }
            }
            if json {
                println!(
                    "{{\"event\":\"compile_result\",\"file\":\"{path}\",\"status\":\"ok\",\"errors\":[],\"timestamp\":{ts}}}"
                );
            } else {
                eprintln!("[{ts}] ✓ `{path}` — no errors");
            }
        }
        Err(msg) => {
            if json {
                let escaped = escape_json(&msg);
                println!(
                    "{{\"event\":\"compile_result\",\"file\":\"{path}\",\"status\":\"error\",\"message\":\"{escaped}\",\"timestamp\":{ts}}}"
                );
            } else {
                eprintln!("[{ts}] ✗ `{path}`:");
                eprintln!("{msg}");
            }
        }
    }

    // Emit hole_graph_update event if there are holes (JSON mode only)
    if json && let Some(summary) = sporec::hole_summary(source) {
        println!("{}", summary.to_json());
    }
}

fn file_modified(path: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
