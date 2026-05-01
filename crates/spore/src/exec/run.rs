use std::process::ExitCode;

use owo_colors::OwoColorize;
use serde_json::json;

use crate::target::{BuildTarget, find_project_target, resolve_build_target};
use crate::util::{fail_human, fail_message, project_exit_code, read_source_message};

pub(crate) fn exec_run(file: &str, json_output: bool) -> ExitCode {
    let project_target = match find_project_target(file) {
        Ok(project_target) => project_target,
        Err(message) => return fail_message(&message, json_output),
    };
    let result = if let Some((root, entry)) = project_target {
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
            let artifact_path = root
                .join("target")
                .join("native")
                .join(entry)
                .with_extension("o");
            let artifact = match sporec_driver::build_project_native_object(root, entry) {
                Ok(artifact) => artifact,
                Err(message) => {
                    return fail_human(&format!(
                        "native project build unavailable for `{}` (entry `{entry}`): {message}",
                        root.display()
                    ));
                }
            };
            if let Some(parent) = artifact_path.parent()
                && let Err(error) = std::fs::create_dir_all(parent)
            {
                return fail_human(&format!(
                    "cannot create native artifact directory `{}`: {error}",
                    parent.display()
                ));
            }
            if let Err(error) = std::fs::write(&artifact_path, artifact) {
                return fail_human(&format!(
                    "cannot write native artifact `{}`: {error}",
                    artifact_path.display()
                ));
            }

            println!(
                "{} built native object `{}`",
                "✓".green(),
                artifact_path.display()
            );
            ExitCode::SUCCESS
        }
        BuildTarget::File(path) => {
            let source = match read_source_message(path) {
                Ok(s) => s,
                Err(message) => return fail_human(&message),
            };
            match sporec_driver::check_source_file(path, &source) {
                sporec_driver::SourceCheckReport::Success { source, warnings } => {
                    sporec_diagnostics::render_diagnostics_human(&source, &warnings);
                }
                sporec_driver::SourceCheckReport::Failure(
                    sporec_driver::SourceCheckFailure::Diagnostics {
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
                sporec_driver::SourceCheckReport::Failure(
                    sporec_driver::SourceCheckFailure::Message(message),
                ) => return fail_human(&message),
            }

            let artifact_path = std::path::Path::new(path).with_extension("o");
            let artifact = match sporec_driver::build_native_object(&source) {
                Ok(artifact) => artifact,
                Err(message) => {
                    return fail_human(&format!(
                        "native build unavailable for `{path}`: {message}"
                    ));
                }
            };
            if let Err(error) = std::fs::write(&artifact_path, artifact) {
                return fail_human(&format!(
                    "cannot write native artifact `{}`: {error}",
                    artifact_path.display()
                ));
            }

            println!(
                "{} built native object `{}`",
                "✓".green(),
                artifact_path.display()
            );
            ExitCode::SUCCESS
        }
    }
}
