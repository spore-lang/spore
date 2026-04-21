use owo_colors::OwoColorize;

use crate::target::find_project_target;
use crate::util::{EMPTY_DIAGNOSTICS, fail_deny_warnings, fail_message, output_mode, timestamp};

pub(crate) fn report_batch_check(
    report: sporec_driver::CheckReport,
    json_output: bool,
    deny_warnings: bool,
    human_success_message: &str,
) -> std::process::ExitCode {
    match report {
        sporec_driver::CheckReport::Success { sources, warnings } => {
            let has_warnings = !warnings.is_empty();
            let warning_messages = sporec_diagnostics::diagnostic_message_lines(&warnings);
            if json_output {
                for (warning, message) in warnings.iter().zip(warning_messages.iter()) {
                    sporec_diagnostics::print_json(
                        &sporec_diagnostics::JsonReport::new()
                            .with_severity(sporec_diagnostics::Severity::Warning)
                            .with_message(message.as_str())
                            .with_diagnostic(warning),
                    );
                }
                if has_warnings && deny_warnings {
                    return fail_deny_warnings(&warning_messages, Some(&warnings), true);
                }
                sporec_diagnostics::print_json(
                    &sporec_diagnostics::JsonReport::new()
                        .with_status(sporec_diagnostics::ReportStatus::Ok)
                        .with_errors(EMPTY_DIAGNOSTICS),
                );
            } else {
                sporec_diagnostics::render_diagnostics_human_with_sources(&sources, &warnings);
                if has_warnings && deny_warnings {
                    return fail_deny_warnings(&warning_messages, Some(&warnings), false);
                }
                println!("{human_success_message}");
            }
            std::process::ExitCode::SUCCESS
        }
        sporec_driver::CheckReport::Failure(sporec_driver::CheckFailure::Diagnostics {
            sources,
            diagnostics,
        }) => sporec_diagnostics::exit_with_diagnostics_error_with_sources(
            &sources,
            &diagnostics,
            output_mode(json_output),
        ),
        sporec_driver::CheckReport::Failure(sporec_driver::CheckFailure::Message(message)) => {
            fail_message(&message, json_output)
        }
    }
}

pub(crate) fn report_single_file_check(
    path: &str,
    source: &str,
    json_output: bool,
    deny_warnings: bool,
    human_success_message: &str,
) -> std::process::ExitCode {
    match sporec_driver::check_source_file(path, source) {
        sporec_driver::SourceCheckReport::Success { source, warnings } => {
            let has_warnings = !warnings.is_empty();
            let warning_messages = sporec_diagnostics::diagnostic_message_lines(&warnings);
            if json_output {
                for (warning, message) in warnings.iter().zip(warning_messages.iter()) {
                    sporec_diagnostics::print_json(
                        &sporec_diagnostics::JsonReport::new()
                            .with_severity(sporec_diagnostics::Severity::Warning)
                            .with_message(message.as_str())
                            .with_diagnostic(warning),
                    );
                }
                if has_warnings && deny_warnings {
                    return fail_deny_warnings(&warning_messages, Some(&warnings), true);
                }

                sporec_diagnostics::print_json(
                    &sporec_diagnostics::JsonReport::new()
                        .with_status(sporec_diagnostics::ReportStatus::Ok)
                        .with_errors(EMPTY_DIAGNOSTICS),
                );
            } else {
                sporec_diagnostics::render_diagnostics_human(&source, &warnings);
                if has_warnings && deny_warnings {
                    return fail_deny_warnings(&warning_messages, Some(&warnings), false);
                }
                println!("{human_success_message}");
            }

            std::process::ExitCode::SUCCESS
        }
        sporec_driver::SourceCheckReport::Failure(
            sporec_driver::SourceCheckFailure::Diagnostics {
                source,
                diagnostics,
            },
        ) => sporec_diagnostics::exit_with_diagnostics_error(
            &source,
            &diagnostics,
            output_mode(json_output),
        ),
        sporec_driver::SourceCheckReport::Failure(sporec_driver::SourceCheckFailure::Message(
            message,
        )) => fail_message(&message, json_output),
    }
}

pub(crate) fn check_and_report(path: &str, source: &str, json_output: bool) {
    let ts = timestamp();
    if let Some((root, entry)) = find_project_target(path) {
        match sporec_driver::check_project(&root, &entry) {
            sporec_driver::CheckReport::Success { sources, warnings } => {
                for warning in &warnings {
                    if json_output {
                        sporec_diagnostics::print_json(
                            &sporec_diagnostics::JsonReport::new()
                                .with_event("warning")
                                .with_file(path)
                                .with_severity(warning.severity)
                                .with_message(sporec_diagnostics::diagnostic_message_line(warning))
                                .with_diagnostic(warning)
                                .with_timestamp(ts),
                        );
                    }
                }
                if json_output {
                    sporec_diagnostics::print_json(
                        &sporec_diagnostics::JsonReport::new()
                            .with_event("compile_result")
                            .with_file(path)
                            .with_status(sporec_diagnostics::ReportStatus::Ok)
                            .with_errors(EMPTY_DIAGNOSTICS)
                            .with_timestamp(ts),
                    );
                } else {
                    if !warnings.is_empty() {
                        eprintln!("[{ts}] warnings for `{path}`:");
                        sporec_diagnostics::render_diagnostics_human_with_sources(
                            &sources, &warnings,
                        );
                    }
                    eprintln!("[{ts}] {} `{path}` — no errors", "✓".green());
                }
            }
            sporec_driver::CheckReport::Failure(sporec_driver::CheckFailure::Diagnostics {
                sources,
                diagnostics,
            }) => {
                if json_output {
                    let message =
                        sporec_diagnostics::diagnostic_message_lines(&diagnostics).join("\n");
                    sporec_diagnostics::print_json(
                        &sporec_diagnostics::JsonReport::new()
                            .with_event("compile_result")
                            .with_file(path)
                            .with_status(sporec_diagnostics::ReportStatus::Error)
                            .with_message(message)
                            .with_diagnostics(&diagnostics)
                            .with_timestamp(ts),
                    );
                } else {
                    eprintln!("[{ts}] {} `{path}`:", "✗".red());
                    sporec_diagnostics::render_diagnostics_human_with_sources(
                        &sources,
                        &diagnostics,
                    );
                }
            }
            sporec_driver::CheckReport::Failure(sporec_driver::CheckFailure::Message(message)) => {
                if json_output {
                    sporec_diagnostics::print_json(
                        &sporec_diagnostics::JsonReport::new()
                            .with_event("compile_result")
                            .with_file(path)
                            .with_status(sporec_diagnostics::ReportStatus::Error)
                            .with_message(message)
                            .with_timestamp(ts),
                    );
                } else {
                    eprintln!("[{ts}] {} `{path}`:", "✗".red());
                    eprintln!("{message}");
                }
            }
        }
    } else {
        match sporec_driver::check_source_file(path, source) {
            sporec_driver::SourceCheckReport::Success { source, warnings } => {
                if json_output {
                    for warning in warnings {
                        sporec_diagnostics::print_json(
                            &sporec_diagnostics::JsonReport::new()
                                .with_event("warning")
                                .with_file(path)
                                .with_severity(warning.severity)
                                .with_message(sporec_diagnostics::diagnostic_message_line(&warning))
                                .with_diagnostic(&warning)
                                .with_timestamp(ts),
                        );
                    }
                    sporec_diagnostics::print_json(
                        &sporec_diagnostics::JsonReport::new()
                            .with_event("compile_result")
                            .with_file(path)
                            .with_status(sporec_diagnostics::ReportStatus::Ok)
                            .with_errors(EMPTY_DIAGNOSTICS)
                            .with_timestamp(ts),
                    );
                } else {
                    if !warnings.is_empty() {
                        eprintln!("[{ts}] warnings for `{path}`:");
                        sporec_diagnostics::render_diagnostics_human(&source, &warnings);
                    }
                    eprintln!("[{ts}] {} `{path}` — no errors", "✓".green());
                }
            }
            sporec_driver::SourceCheckReport::Failure(
                sporec_driver::SourceCheckFailure::Diagnostics {
                    source,
                    diagnostics,
                },
            ) => {
                if json_output {
                    let message =
                        sporec_diagnostics::diagnostic_message_lines(&diagnostics).join("\n");
                    sporec_diagnostics::print_json(
                        &sporec_diagnostics::JsonReport::new()
                            .with_event("compile_result")
                            .with_file(path)
                            .with_status(sporec_diagnostics::ReportStatus::Error)
                            .with_message(message)
                            .with_diagnostics(&diagnostics)
                            .with_timestamp(ts),
                    );
                } else {
                    eprintln!("[{ts}] {} `{path}`:", "✗".red());
                    sporec_diagnostics::render_diagnostics_human(&source, &diagnostics);
                }
            }
            sporec_driver::SourceCheckReport::Failure(
                sporec_driver::SourceCheckFailure::Message(message),
            ) => {
                if json_output {
                    sporec_diagnostics::print_json(
                        &sporec_diagnostics::JsonReport::new()
                            .with_event("compile_result")
                            .with_file(path)
                            .with_status(sporec_diagnostics::ReportStatus::Error)
                            .with_message(message)
                            .with_timestamp(ts),
                    );
                } else {
                    eprintln!("[{ts}] {} `{path}`:", "✗".red());
                    eprintln!("{message}");
                }
            }
        }
    }

    if let Some(summary) = hole_graph_update(source, json_output) {
        sporec_diagnostics::print_json(&summary);
    }
}

pub(crate) fn hole_graph_update(
    source: &str,
    json_output: bool,
) -> Option<sporec_driver::HoleSummary> {
    if json_output {
        sporec_driver::hole_summary(source)
    } else {
        None
    }
}
