use std::process::ExitCode;

use owo_colors::OwoColorize;

use crate::util::read_source;

pub(crate) fn exec_format(files: &[String], check_mode: bool, diff_mode: bool) -> ExitCode {
    let mut exit = ExitCode::SUCCESS;
    for path in files {
        let source = match read_source(path) {
            Ok(s) => s,
            Err(c) => {
                exit = c;
                continue;
            }
        };

        match sporec_driver::format(&source) {
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
                } else if formatted == source {
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
