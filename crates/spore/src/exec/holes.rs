use std::process::ExitCode;

use owo_colors::OwoColorize;

use crate::util::read_source;

pub(crate) fn exec_holes(file: &str) -> ExitCode {
    let source = match read_source(file) {
        Ok(s) => s,
        Err(c) => return c,
    };

    match sporec_driver::holes_report(&source) {
        Ok(report) => {
            sporec_diagnostics::print_json(&report);
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{}: {msg}", "error".red().bold());
            ExitCode::FAILURE
        }
    }
}
