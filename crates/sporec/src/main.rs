use std::fs;
use std::process::ExitCode;

use bpaf::*;
use serde_json::json;
use sporec_parser::parse;
use sporec_typeck::error::{ErrorCode, all_error_codes};
use sporec_typeck::hole::{CandidateRanking, HoleInfo, HoleReport, TypeInferenceConfidence};
use sporec_typeck::{is_synthetic_hole_name, type_check};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
enum Cmd {
    Compile {
        json: bool,
        files: Vec<String>,
    },
    Holes {
        json: bool,
        file: String,
    },
    QueryHole {
        json: bool,
        file: String,
        hole: String,
    },
    Explain {
        json: bool,
        code: String,
    },
}

fn json_flag() -> impl Parser<bool> {
    long("json").help("Output results as JSON").switch()
}

fn cmd_compile_parser() -> impl Parser<Cmd> {
    let json = json_flag();
    let files = positional::<String>("FILE")
        .help(".sp or .spore source file(s) to compile")
        .some("expected at least one file");
    construct!(Cmd::Compile { json, files })
        .to_options()
        .descr("Compile one or more explicit input files")
        .command("compile")
}

fn cmd_holes_parser() -> impl Parser<Cmd> {
    let json = json_flag();
    let file = positional::<String>("FILE").help("A .sp or .spore file");
    construct!(Cmd::Holes { json, file })
        .to_options()
        .descr("List holes in a source file")
        .command("holes")
}

fn cmd_query_hole_parser() -> impl Parser<Cmd> {
    let json = json_flag();
    let file = positional::<String>("FILE").help("A .sp or .spore file");
    let hole = positional::<String>("HOLE").help("Hole name, with or without leading ?");
    construct!(Cmd::QueryHole { json, file, hole })
        .to_options()
        .descr("Inspect one named hole in a source file")
        .command("query-hole")
}

fn cmd_explain_parser() -> impl Parser<Cmd> {
    let json = json_flag();
    let code = positional::<String>("CODE").help("Diagnostic code, for example E0301");
    construct!(Cmd::Explain { json, code })
        .to_options()
        .descr("Explain one diagnostic code")
        .command("explain")
}

fn cli() -> OptionParser<Cmd> {
    construct!([
        cmd_compile_parser(),
        cmd_holes_parser(),
        cmd_query_hole_parser(),
        cmd_explain_parser(),
    ])
    .to_options()
    .version(VERSION)
    .descr("sporec — low-level Spore compiler CLI")
}

fn main() -> ExitCode {
    match cli().run() {
        Cmd::Compile { json, files } => exec_compile(&files, json),
        Cmd::Holes { json, file } => exec_holes(&file, json),
        Cmd::QueryHole { json, file, hole } => exec_query_hole(&file, &hole, json),
        Cmd::Explain { json, code } => exec_explain(&code, json),
    }
}

fn exec_compile(files: &[String], json_output: bool) -> ExitCode {
    if files.len() == 1 {
        let file = &files[0];
        let source = match read_source(file) {
            Ok(source) => source,
            Err(message) => {
                return sporec_diagnostics::exit_with_message_error(&message, json_output);
            }
        };

        return match sporec::compile(&source) {
            Ok(output) => {
                let (warning_source, warning_diagnostics) =
                    match sporec::check_source_file(file, &source) {
                        sporec::SourceCheckReport::Success { source, warnings } => {
                            (source, warnings)
                        }
                        sporec::SourceCheckReport::Failure(
                            sporec::SourceCheckFailure::Diagnostics {
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
                        sporec::SourceCheckReport::Failure(
                            sporec::SourceCheckFailure::Message(message),
                        ) => {
                            return sporec_diagnostics::exit_with_message_error(
                                &message,
                                json_output,
                            );
                        }
                    };

                if json_output {
                    sporec_diagnostics::print_json(&json!({
                        "status": "ok",
                        "warnings": output.warnings,
                        "warning_diagnostics": warning_diagnostics,
                    }));
                } else {
                    sporec_diagnostics::render_diagnostics_human(
                        &warning_source,
                        &warning_diagnostics,
                    );
                    println!("ok: no errors");
                }
                ExitCode::SUCCESS
            }
            Err(message) => match sporec::check_source_file(file, &source) {
                sporec::SourceCheckReport::Failure(sporec::SourceCheckFailure::Diagnostics {
                    source,
                    diagnostics,
                }) => sporec_diagnostics::exit_with_diagnostics_error(
                    &source,
                    &diagnostics,
                    json_output,
                ),
                sporec::SourceCheckReport::Failure(sporec::SourceCheckFailure::Message(
                    fallback,
                )) => sporec_diagnostics::exit_with_message_error(&fallback, json_output),
                sporec::SourceCheckReport::Success { .. } => {
                    sporec_diagnostics::exit_with_message_error(&message, json_output)
                }
            },
        };
    }

    let refs: Vec<&str> = files.iter().map(|file| file.as_str()).collect();
    let result = sporec::compile_files(&refs);

    match result {
        Ok(output) => {
            if json_output {
                sporec_diagnostics::print_json(&json!({
                    "status": "ok",
                    "warnings": output.warnings,
                }));
            } else {
                for warning in &output.warnings {
                    sporec_diagnostics::emit_warning_message(warning, false);
                }
                println!("ok: no errors ({} files)", files.len());
            }
            ExitCode::SUCCESS
        }
        Err(message) => sporec_diagnostics::exit_with_message_error(&message, json_output),
    }
}

fn exec_holes(file: &str, json_output: bool) -> ExitCode {
    let source = match read_source(file) {
        Ok(source) => source,
        Err(message) => {
            return sporec_diagnostics::exit_with_message_error(&message, json_output);
        }
    };

    if json_output {
        match sporec::holes_report(&source) {
            Ok(report) => {
                sporec_diagnostics::print_json(&report);
                ExitCode::SUCCESS
            }
            Err(message) => sporec_diagnostics::exit_with_message_error(&message, json_output),
        }
    } else {
        match load_hole_report(&source) {
            Ok(report) => {
                if report.holes.is_empty() {
                    println!("ok: no holes");
                } else {
                    for hole in &report.holes {
                        println!(
                            "{} :: {} (in {})",
                            display_hole_name(&hole.name),
                            hole.expected_type,
                            hole.function
                        );
                    }
                }
                ExitCode::SUCCESS
            }
            Err(message) => sporec_diagnostics::exit_with_message_error(&message, json_output),
        }
    }
}

fn exec_query_hole(file: &str, hole: &str, json_output: bool) -> ExitCode {
    let source = match read_source(file) {
        Ok(source) => source,
        Err(message) => {
            return sporec_diagnostics::exit_with_message_error(&message, json_output);
        }
    };

    if json_output {
        return match sporec::query_hole_report(file, &source, hole) {
            Ok(report) => {
                sporec_diagnostics::print_json(&report);
                ExitCode::SUCCESS
            }
            Err(message) => sporec_diagnostics::exit_with_message_error(&message, json_output),
        };
    }

    let report = match load_hole_report(&source) {
        Ok(report) => report,
        Err(message) => {
            return sporec_diagnostics::exit_with_message_error(&message, json_output);
        }
    };

    let needle = normalize_hole_name(hole);
    let matches: Vec<&HoleInfo> = report
        .holes
        .iter()
        .filter(|candidate| candidate.name == needle)
        .collect();

    match matches.as_slice() {
        [hole] => {
            println!("{}", render_hole(hole));
            ExitCode::SUCCESS
        }
        [] => sporec_diagnostics::exit_with_message_error(
            &format!("hole `?{needle}` not found in `{file}`"),
            json_output,
        ),
        _ => {
            let locations = matches
                .iter()
                .map(|candidate| candidate.function.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            sporec_diagnostics::exit_with_message_error(
                &format!(
                    "hole `?{needle}` is ambiguous in `{file}`; matching functions: {locations}"
                ),
                json_output,
            )
        }
    }
}

fn exec_explain(code: &str, json_output: bool) -> ExitCode {
    let normalized = code.trim().to_ascii_uppercase();
    let Some(error_code) = lookup_error_code(&normalized) else {
        return sporec_diagnostics::exit_with_message_error(
            &format!("unknown diagnostic code `{code}`"),
            json_output,
        );
    };

    let explanation = error_code.explain();
    let severity = error_code.severity().to_string();

    if json_output {
        sporec_diagnostics::print_json(&json!({
            "code": error_code.to_string(),
            "severity": severity,
            "summary": explanation,
        }));
    } else {
        println!("{}: {}", error_code, explanation);
        println!("severity: {severity}");
    }

    ExitCode::SUCCESS
}

fn read_source(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read `{path}`: {error}"))
}

fn load_hole_report(source: &str) -> Result<HoleReport, String> {
    let ast = parse(source).map_err(join_errors)?;
    let result = type_check(&ast).map_err(join_errors)?;
    Ok(result.hole_report)
}

fn join_errors<E: std::fmt::Display>(errors: Vec<E>) -> String {
    errors
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn lookup_error_code(code: &str) -> Option<ErrorCode> {
    all_error_codes()
        .iter()
        .copied()
        .find(|candidate| candidate.to_string() == code)
}

fn normalize_hole_name(name: &str) -> &str {
    name.strip_prefix('?').unwrap_or(name)
}

fn display_hole_name(name: &str) -> String {
    if is_synthetic_hole_name(name) {
        "?".to_string()
    } else {
        format!("?{name}")
    }
}

fn render_hole(hole: &HoleInfo) -> String {
    let mut lines = vec![
        display_hole_name(&hole.name),
        format!("  expected: {}", hole.expected_type),
        format!("  function: {}", hole.function),
    ];

    if let Some(location) = &hole.location {
        lines.push(format!(
            "  location: {}:{}:{}",
            location.file, location.line, location.column
        ));
    }

    if let Some(inferred_from) = &hole.type_inferred_from {
        lines.push(format!("  inferred from: {inferred_from}"));
    }

    if let Some(signature) = &hole.enclosing_signature {
        lines.push(format!("  signature: {signature}"));
    }

    if !hole.bindings.is_empty() {
        lines.push("  bindings:".to_string());
        for (name, ty) in &hole.bindings {
            lines.push(format!("    - {name}: {ty}"));
        }
    }

    if !hole.capabilities.is_empty() {
        lines.push(format!(
            "  capabilities: {}",
            hole.capabilities
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if !hole.errors_to_handle.is_empty() {
        lines.push(format!(
            "  errors to handle: {}",
            hole.errors_to_handle.join(", ")
        ));
    }

    if let Some(cost_budget) = &hole.cost_budget {
        lines.push(format!(
            "  cost budget: used {}, total {}, remaining {}",
            cost_budget.cost_before_hole,
            cost_budget
                .budget_total
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unbounded".to_string()),
            cost_budget
                .budget_remaining
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ));
    }

    if !hole.candidates.is_empty() {
        lines.push("  candidates:".to_string());
        for candidate in &hole.candidates {
            lines.push(format!(
                "    - {} (overall {:.2}, type {:.2}, cost {:.2}, capability {:.2}, error {:.2})",
                candidate.name,
                candidate.overall(),
                candidate.type_match,
                candidate.cost_fit,
                candidate.capability_fit,
                candidate.error_coverage
            ));
        }
    }

    if let Some(confidence) = &hole.confidence {
        let type_inference = match confidence.type_inference {
            TypeInferenceConfidence::Certain => "certain",
            TypeInferenceConfidence::Partial => "partial",
            TypeInferenceConfidence::Unknown => "unknown",
        };
        let candidate_ranking = match confidence.candidate_ranking {
            CandidateRanking::UniqueBest => "unique_best",
            CandidateRanking::Ambiguous => "ambiguous",
            CandidateRanking::NoCandidates => "no_candidates",
        };
        lines.push(format!(
            "  confidence: type={}, ranking={}, ambiguous={}",
            type_inference, candidate_ranking, confidence.ambiguous_count
        ));
        if let Some(recommendation) = &confidence.recommendation {
            lines.push(format!("  recommendation: {recommendation}"));
        }
    }

    if !hole.error_clusters.is_empty() {
        lines.push("  error clusters:".to_string());
        for cluster in &hole.error_clusters {
            lines.push(format!(
                "    - {}: {}",
                cluster.source, cluster.handling_suggestion
            ));
        }
    }

    lines.join("\n")
}
