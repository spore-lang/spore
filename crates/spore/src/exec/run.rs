use std::process::ExitCode;

use owo_colors::OwoColorize;
use serde_json::json;

use crate::report::{report_batch_check, report_single_file_check};
use crate::target::{BuildTarget, find_project_target, resolve_build_target};
use crate::util::{fail_human, fail_message, project_exit_code, read_source_message};

pub(crate) fn exec_run(file: &str, json_output: bool) -> ExitCode {
    let result = if let Some((root, entry)) = find_project_target(file) {
        sporec_driver::run_project_with_outcome(&root, &entry)
    } else {
        let source = match read_source_message(file) {
            Ok(s) => s,
            Err(message) => return fail_message(&message, json_output),
        };
        sporec_driver::run(&source).map(sporec_driver::ProjectRunOutcome::Completed)
    };

    match result {
        Ok(sporec_driver::ProjectRunOutcome::Completed(_value)) => {
            if json_output {
                sporec_diagnostics::print_json(&json!({"status": "ok"}));
            }
            ExitCode::SUCCESS
        }
        Ok(sporec_driver::ProjectRunOutcome::Exited(code)) => {
            if json_output {
                sporec_diagnostics::print_json(&json!({"status": "ok", "exit_code": code}));
            }
            project_exit_code(code)
        }
        Err(msg) => fail_message(&msg, json_output),
    }
}

pub(crate) fn exec_build(file: Option<&str>) -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => return fail_human(&format!("cannot determine current directory: {e}")),
    };

    let target = match resolve_build_target(file, &cwd) {
        Ok(target) => target,
        Err(msg) => return fail_human(&msg),
    };

    match &target {
        BuildTarget::Project { root, entry } => {
            let success_message = format!(
                "{} compiled entry path `{entry}` successfully (interpreter mode — no binary output yet)",
                "✓".green(),
            );
            report_batch_check(
                sporec_driver::check_project(root, entry),
                false,
                false,
                &success_message,
            )
        }
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
