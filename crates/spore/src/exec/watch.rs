use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};
use owo_colors::OwoColorize;

use crate::report::check_and_report;
use crate::util::fail_message;

pub(crate) fn exec_watch(file: &str, json_output: bool) -> ExitCode {
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

    let mut last_content = String::new();
    if let Ok(source) = std::fs::read_to_string(file) {
        last_content = source.clone();
        check_and_report(file, &source, json_output);
    }

    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
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
                            sporec_diagnostics::print_json(
                                &sporec_diagnostics::JsonReport::new()
                                    .with_event("error")
                                    .with_file(file)
                                    .with_message(e.to_string()),
                            );
                        } else {
                            eprintln!("{}: reading `{file}`: {e}", "error".red().bold());
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                if json_output {
                    sporec_diagnostics::print_json(
                        &sporec_diagnostics::JsonReport::new()
                            .with_event("error")
                            .with_file(file)
                            .with_message(format!("{e:?}")),
                    );
                } else {
                    eprintln!("{}: watcher error: {e:?}", "error".red().bold());
                }
            }
            Err(_) => break,
        }
    }

    ExitCode::SUCCESS
}
