use std::process::ExitCode;

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const EMPTY_DIAGNOSTICS: &[sporec_diagnostics::Diagnostic] = &[];

pub(crate) fn read_source_message(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read `{path}`: {e}"))
}

pub(crate) fn read_source(path: &str) -> Result<String, ExitCode> {
    read_source_message(path).map_err(|message| fail_human(&message))
}

pub(crate) fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(crate) fn output_mode(json_output: bool) -> bool {
    json_output
}

pub(crate) fn fail_message(message: &str, json_output: bool) -> ExitCode {
    sporec_diagnostics::exit_with_message_error(message, output_mode(json_output))
}

pub(crate) fn fail_human(message: &str) -> ExitCode {
    sporec_diagnostics::exit_with_message_error(message, false)
}

pub(crate) fn project_exit_code(code: u8) -> ExitCode {
    ExitCode::from(code)
}

pub(crate) fn fail_deny_warnings(
    warnings: &[String],
    warning_diagnostics: Option<&[sporec_diagnostics::Diagnostic]>,
    json_output: bool,
) -> ExitCode {
    if json_output {
        let mut report = sporec_diagnostics::JsonReport::new()
            .with_status(sporec_diagnostics::ReportStatus::Error)
            .with_message("warnings are denied")
            .with_warnings(warnings);
        if let Some(diagnostics) = warning_diagnostics {
            report = report.with_warning_diagnostics(diagnostics);
        }
        sporec_diagnostics::print_json(&report);
        ExitCode::FAILURE
    } else {
        fail_human("warnings are denied")
    }
}
