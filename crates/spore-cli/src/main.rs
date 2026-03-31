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
    eprintln!("  check <file>     Type-check a .spore file (no execution)");
    eprintln!("  build <file>     Compile a .spore file");
    eprintln!("  watch <file>     Watch a file and re-check on changes");
    eprintln!("  help             Show this help message");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("  --json           Output results as JSON (run, check, watch)");
    eprintln!("  -V, --version    Print version");
    eprintln!("  -h, --help       Print help");
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
    let (_, source) = match read_source(args) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let json_output = args.iter().any(|a| a == "--json");

    match sporec::compile(&source) {
        Ok(()) => {
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

fn cmd_build(args: &[String]) -> ExitCode {
    let (_, source) = match read_source(args) {
        Ok(s) => s,
        Err(code) => return code,
    };

    match sporec::compile(&source) {
        Ok(()) => {
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
        Ok(()) => {
            if json {
                println!(
                    "{{\"event\":\"check\",\"file\":\"{path}\",\"status\":\"ok\",\"errors\":[],\"timestamp\":{ts}}}"
                );
            } else {
                eprintln!("[{ts}] ✓ `{path}` — no errors");
            }
        }
        Err(msg) => {
            if json {
                let escaped = escape_json(&msg);
                println!(
                    "{{\"event\":\"check\",\"file\":\"{path}\",\"status\":\"error\",\"message\":\"{escaped}\",\"timestamp\":{ts}}}"
                );
            } else {
                eprintln!("[{ts}] ✗ `{path}`:");
                eprintln!("{msg}");
            }
        }
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
