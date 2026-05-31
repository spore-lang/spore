use std::process::ExitCode;

use owo_colors::OwoColorize;
use serde_json::json;

use crate::report::{report_batch_check, report_single_file_check};
use crate::target::{ResolvedSpTarget, resolve_project_aware_sp_targets};
use crate::util::{fail_deny_warnings, fail_human, fail_message, read_source, read_source_message};

fn report_warnings(
    warnings: &[sporec_diagnostics::Diagnostic],
    json_output: bool,
    deny_warnings: bool,
    render_human: impl FnOnce(),
) -> Option<ExitCode> {
    let warning_messages = sporec_diagnostics::diagnostic_message_lines(warnings);
    if json_output {
        for (warning, message) in warnings.iter().zip(warning_messages.iter()) {
            sporec_diagnostics::print_json(
                &sporec_diagnostics::JsonReport::new()
                    .with_severity(sporec_diagnostics::Severity::Warning)
                    .with_message(message.as_str())
                    .with_diagnostic(warning),
            );
        }
    } else {
        render_human();
    }

    if !warnings.is_empty() && deny_warnings {
        Some(fail_deny_warnings(
            &warning_messages,
            Some(warnings),
            json_output,
        ))
    } else {
        None
    }
}

fn record_property_results(
    property_results: &[sporec_driver::PropertyResult],
    verbose: bool,
    json_output: bool,
    total_passed: &mut usize,
    total_failed: &mut usize,
) {
    for r in property_results {
        let kind_label = "property";
        if r.passed {
            *total_passed += 1;
            if !json_output && verbose {
                eprintln!(
                    "  {} {} :: {} \"{}\"",
                    "✓".green(),
                    r.fn_name,
                    kind_label,
                    r.label
                );
            }
        } else {
            *total_failed += 1;
            let msg = r.error.as_deref().unwrap_or("assertion failed");
            if !json_output {
                eprintln!(
                    "  {} {} :: {} \"{}\" — {}",
                    "✗".red(),
                    r.fn_name,
                    kind_label,
                    r.label,
                    msg
                );
            }
        }
    }
}

fn finish_property_results(
    total_passed: usize,
    total_failed: usize,
    json_output: bool,
) -> ExitCode {
    if json_output {
        sporec_diagnostics::print_json(&json!({
            "status": if total_failed == 0 {
                sporec_diagnostics::ReportStatus::Ok
            } else {
                sporec_diagnostics::ReportStatus::Fail
            },
            "passed": total_passed,
            "failed": total_failed,
        }));
    } else {
        let total = total_passed + total_failed;
        if total == 0 {
            eprintln!("note: no properties found");
        } else if total_failed == 0 {
            eprintln!("\n{} {total} properties passed", "✓".green());
        } else {
            eprintln!(
                "\n{}: {total_failed} of {total} properties failed",
                "FAIL".red().bold()
            );
        }
    }

    if total_failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

pub(crate) fn exec_check(
    files: &[String],
    verbose: bool,
    json_output: bool,
    deny_warnings: bool,
) -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let targets = match resolve_project_aware_sp_targets(files, &cwd) {
        Ok(targets) => targets,
        Err(e) => return fail_human(&e),
    };
    if targets.is_empty() {
        if !json_output {
            eprintln!("note: no .sp files found");
        }
        return ExitCode::SUCCESS;
    }

    if targets.len() > 1 {
        // When the CLI recurses a tree (e.g. `spore check` at a repo root), we see many
        // `.sp` files. The flat `check_files` path only maps paths to a synthetic module
        // tree; it does not read `spore.toml` or apply `[dependencies]`, so project-mode
        // imports (path deps) fail. For any file that lives under a `spore.toml` project source
        // root, use `check_project` (same as single-file) so path dependencies resolve.
        use std::collections::BTreeSet;
        use std::path::PathBuf;

        let mut project_targets: BTreeSet<(String, String)> = BTreeSet::new();
        let mut standalone: Vec<String> = Vec::new();
        for target in &targets {
            match target {
                ResolvedSpTarget::Project { root, entry, .. } => {
                    project_targets.insert((root.to_string_lossy().to_string(), entry.clone()));
                }
                ResolvedSpTarget::Standalone { path } => standalone.push(path.clone()),
            }
        }

        if project_targets.is_empty() {
            let refs: Vec<&str> = standalone.iter().map(|s| s.as_str()).collect();
            let success_message = format!("{} no errors ({} files)", "✓".green(), targets.len());
            return report_batch_check(
                sporec_driver::check_files(&refs),
                json_output,
                deny_warnings,
                &success_message,
            );
        }

        let mut merged_sources = Vec::new();
        let mut merged_warnings = Vec::new();
        for (root, entry) in &project_targets {
            let root = PathBuf::from(root);
            match sporec_driver::check_project(&root, entry) {
                sporec_driver::CheckReport::Success { sources, warnings } => {
                    merged_sources.extend(sources);
                    merged_warnings.extend(warnings);
                }
                sporec_driver::CheckReport::Failure(failure) => {
                    return report_batch_check(
                        sporec_driver::CheckReport::Failure(failure),
                        json_output,
                        deny_warnings,
                        "",
                    );
                }
            }
        }

        if !standalone.is_empty() {
            let refs: Vec<&str> = standalone.iter().map(|s| s.as_str()).collect();
            match sporec_driver::check_files(&refs) {
                sporec_driver::CheckReport::Success { sources, warnings } => {
                    merged_sources.extend(sources);
                    merged_warnings.extend(warnings);
                }
                sporec_driver::CheckReport::Failure(failure) => {
                    return report_batch_check(
                        sporec_driver::CheckReport::Failure(failure),
                        json_output,
                        deny_warnings,
                        "",
                    );
                }
            }
        }

        let success_message = format!("{} no errors ({} files)", "✓".green(), targets.len());
        report_batch_check(
            sporec_driver::CheckReport::Success {
                sources: merged_sources,
                warnings: merged_warnings,
            },
            json_output,
            deny_warnings,
            &success_message,
        )
    } else {
        let target = &targets[0];
        if verbose {
            let result = match target {
                ResolvedSpTarget::Project { root, entry, .. } => {
                    if deny_warnings {
                        match sporec_driver::check_project(root, entry) {
                            sporec_driver::CheckReport::Success { sources, warnings } => {
                                if let Some(exit_code) =
                                    report_warnings(&warnings, false, true, || {
                                        sporec_diagnostics::render_diagnostics_human_with_sources(
                                            &sources, &warnings,
                                        );
                                    })
                                {
                                    return exit_code;
                                }
                            }
                            sporec_driver::CheckReport::Failure(
                                sporec_driver::CheckFailure::Diagnostics {
                                    sources,
                                    diagnostics,
                                },
                            ) => {
                                return sporec_diagnostics::exit_with_diagnostics_error_with_sources(
                                    &sources,
                                    &diagnostics,
                                    false,
                                );
                            }
                            sporec_driver::CheckReport::Failure(
                                sporec_driver::CheckFailure::Message(message),
                            ) => {
                                return fail_human(&message);
                            }
                        }
                    }
                    sporec_driver::check_project_verbose(root, entry)
                }
                ResolvedSpTarget::Standalone { path } => {
                    let source = match read_source(path) {
                        Ok(s) => s,
                        Err(c) => return c,
                    };
                    if deny_warnings {
                        match sporec_driver::check_source_file(path, &source) {
                            sporec_driver::SourceCheckReport::Success {
                                source: canonical_source,
                                warnings,
                            } => {
                                if let Some(exit_code) =
                                    report_warnings(&warnings, false, true, || {
                                        sporec_diagnostics::render_diagnostics_human(
                                            &canonical_source,
                                            &warnings,
                                        );
                                    })
                                {
                                    return exit_code;
                                }
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
                    }
                    sporec_driver::check_verbose(&source)
                }
            };

            match result {
                Ok(detail) => {
                    print!("{detail}");
                    ExitCode::SUCCESS
                }
                Err(msg) => {
                    if let ResolvedSpTarget::Project { root, entry, .. } = target {
                        match sporec_driver::check_project(root, entry) {
                            sporec_driver::CheckReport::Failure(
                                sporec_driver::CheckFailure::Diagnostics {
                                    sources,
                                    diagnostics,
                                },
                            ) => {
                                return sporec_diagnostics::exit_with_diagnostics_error_with_sources(
                                    &sources,
                                    &diagnostics,
                                    false,
                                );
                            }
                            sporec_driver::CheckReport::Failure(
                                sporec_driver::CheckFailure::Message(message),
                            ) => {
                                return fail_human(&message);
                            }
                            sporec_driver::CheckReport::Success { .. } => {}
                        }
                    }
                    fail_human(&msg)
                }
            }
        } else {
            match target {
                ResolvedSpTarget::Project { root, entry, .. } => report_batch_check(
                    sporec_driver::check_project(root, entry),
                    json_output,
                    deny_warnings,
                    "✓ no errors",
                ),
                ResolvedSpTarget::Standalone { path } => {
                    let source = match read_source_message(path) {
                        Ok(s) => s,
                        Err(message) => return fail_message(&message, json_output),
                    };
                    report_single_file_check(
                        path,
                        &source,
                        json_output,
                        deny_warnings,
                        "✓ no errors",
                    )
                }
            }
        }
    }
}

pub(crate) fn exec_test(
    files: &[String],
    verbose: bool,
    json_output: bool,
    deny_warnings: bool,
) -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let targets = match resolve_project_aware_sp_targets(files, &cwd) {
        Ok(targets) => targets,
        Err(e) => return fail_message(&e, json_output),
    };
    if targets.is_empty() {
        if !json_output {
            eprintln!("note: no .sp files found");
        }
        return ExitCode::SUCCESS;
    }

    let mut total_passed = 0usize;
    let mut total_failed = 0usize;

    if targets.len() > 1
        && targets
            .iter()
            .all(|target| matches!(target, ResolvedSpTarget::Standalone { .. }))
    {
        let paths = targets
            .iter()
            .filter_map(|target| match target {
                ResolvedSpTarget::Standalone { path } => Some(path.as_str()),
                ResolvedSpTarget::Project { .. } => None,
            })
            .collect::<Vec<_>>();
        match sporec_driver::check_files(&paths) {
            sporec_driver::CheckReport::Success { sources, warnings } => {
                if let Some(exit_code) =
                    report_warnings(&warnings, json_output, deny_warnings, || {
                        sporec_diagnostics::render_diagnostics_human_with_sources(
                            &sources, &warnings,
                        );
                    })
                {
                    return exit_code;
                }
            }
            sporec_driver::CheckReport::Failure(sporec_driver::CheckFailure::Diagnostics {
                sources,
                diagnostics,
            }) => {
                return sporec_diagnostics::exit_with_diagnostics_error_with_sources(
                    &sources,
                    &diagnostics,
                    json_output,
                );
            }
            sporec_driver::CheckReport::Failure(sporec_driver::CheckFailure::Message(message)) => {
                return fail_message(&message, json_output);
            }
        }
        let property_results = match sporec_driver::test_properties_files(&paths) {
            Ok(r) => r,
            Err(msg) => return fail_message(&msg, json_output),
        };
        record_property_results(
            &property_results,
            verbose,
            json_output,
            &mut total_passed,
            &mut total_failed,
        );
        return finish_property_results(total_passed, total_failed, json_output);
    }

    for target in &targets {
        let property_results = match target {
            ResolvedSpTarget::Project {
                path: _,
                root,
                entry,
            } => {
                match sporec_driver::check_project(root, entry) {
                    sporec_driver::CheckReport::Success { sources, warnings } => {
                        if let Some(exit_code) =
                            report_warnings(&warnings, json_output, deny_warnings, || {
                                sporec_diagnostics::render_diagnostics_human_with_sources(
                                    &sources, &warnings,
                                );
                            })
                        {
                            return exit_code;
                        }
                    }
                    sporec_driver::CheckReport::Failure(
                        sporec_driver::CheckFailure::Diagnostics {
                            sources,
                            diagnostics,
                        },
                    ) => {
                        return sporec_diagnostics::exit_with_diagnostics_error_with_sources(
                            &sources,
                            &diagnostics,
                            json_output,
                        );
                    }
                    sporec_driver::CheckReport::Failure(sporec_driver::CheckFailure::Message(
                        message,
                    )) => return fail_message(&message, json_output),
                }
                match sporec_driver::test_properties_project(root, entry) {
                    Ok(r) => r,
                    Err(msg) => return fail_message(&msg, json_output),
                }
            }
            ResolvedSpTarget::Standalone { path } => {
                let source = match read_source_message(path) {
                    Ok(s) => s,
                    Err(message) => return fail_message(&message, json_output),
                };

                match sporec_driver::check_source_file(path, &source) {
                    sporec_driver::SourceCheckReport::Success {
                        source: canonical_source,
                        warnings,
                    } => {
                        if let Some(exit_code) =
                            report_warnings(&warnings, json_output, deny_warnings, || {
                                sporec_diagnostics::render_diagnostics_human(
                                    &canonical_source,
                                    &warnings,
                                );
                            })
                        {
                            return exit_code;
                        }
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
                            json_output,
                        );
                    }
                    sporec_driver::SourceCheckReport::Failure(
                        sporec_driver::SourceCheckFailure::Message(message),
                    ) => {
                        return fail_message(&message, json_output);
                    }
                }

                match sporec_driver::test_properties(&source) {
                    Ok(r) => r,
                    Err(msg) => return fail_message(&msg, json_output),
                }
            }
        };

        record_property_results(
            &property_results,
            verbose,
            json_output,
            &mut total_passed,
            &mut total_failed,
        );
    }

    finish_property_results(total_passed, total_failed, json_output)
}
