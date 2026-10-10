use sporec_diagnostics::{Diagnostic, DiagnosticRepair, Severity, SourceFile};
use sporec_parser::error::ParseError;
use sporec_parser::parse;
use sporec_typeck::error::{Severity as TypeckSeverity, TypeError};
use sporec_typeck::type_check;

pub fn source_file(name: impl Into<String>, contents: impl Into<String>) -> SourceFile {
    SourceFile::new(name, contents)
}

#[derive(Debug, Clone)]
pub enum SourceCheckReport {
    Success {
        source: SourceFile,
        warnings: Vec<Diagnostic>,
    },
    Failure(SourceCheckFailure),
}

#[derive(Debug, Clone)]
pub enum SourceCheckFailure {
    Message(String),
    Diagnostics {
        source: SourceFile,
        diagnostics: Vec<Diagnostic>,
    },
}

pub fn check_source_file(name: &str, contents: &str) -> SourceCheckReport {
    let source = source_file(name, contents);
    let ast = match parse(contents) {
        Ok(ast) => ast,
        Err(errors) => {
            return SourceCheckReport::Failure(SourceCheckFailure::Diagnostics {
                source: source.clone(),
                diagnostics: diagnostics_for_parse_errors(&source, &errors),
            });
        }
    };

    match type_check(&ast) {
        Ok(result) => {
            let warnings = diagnostics_for_type_errors(&source, &result.warnings);
            SourceCheckReport::Success { source, warnings }
        }
        Err(errors) => {
            let diagnostics = diagnostics_for_type_errors(&source, &errors);
            SourceCheckReport::Failure(SourceCheckFailure::Diagnostics {
                source,
                diagnostics,
            })
        }
    }
}

pub fn diagnostics_for_parse_errors(source: &SourceFile, errors: &[ParseError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|error| parse_error_to_diagnostic(source, error))
        .collect()
}

pub fn diagnostics_for_type_errors(source: &SourceFile, errors: &[TypeError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|error| type_error_to_diagnostic(source, error))
        .collect()
}

pub fn parse_error_to_diagnostic(source: &SourceFile, error: &ParseError) -> Diagnostic {
    let mut diagnostic = Diagnostic::new("parse-error", Severity::Error, error.message.clone())
        .with_primary_span(source.span(error.span.start..error.span.end))
        .with_explanation_key("parse-error");

    let (concept_refs, repair) = parse_teaching_metadata(&error.message);
    if !concept_refs.is_empty() {
        diagnostic = diagnostic.with_concept_refs(concept_refs);
    }
    if let Some(repair) = repair {
        diagnostic = diagnostic.with_repair(repair);
    }

    diagnostic
}

pub fn type_error_to_diagnostic(source: &SourceFile, error: &TypeError) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(
        error.code.to_string(),
        map_typeck_severity(error.code.severity()),
        error.message.clone(),
    )
    .with_concept_refs(type_error_concepts(error.code))
    .with_explanation_key(type_error_explanation_key(error.code));

    if let Some(span) = error.span {
        diagnostic = diagnostic.with_primary_span(source.span(span.start..span.end));
    }

    diagnostic
}

fn parse_teaching_metadata(message: &str) -> (Vec<String>, Option<DiagnosticRepair>) {
    if message.contains("use `budget { field: limit }`") {
        return (
            vec!["budget".to_string(), "signatures".to_string()],
            Some(DiagnosticRepair::new(
                "write intent-signature budgets as named integer fields",
                Some("budget { calls: 1 }".to_string()),
            )),
        );
    }
    if message.contains("put generic bounds inline") {
        return (
            vec!["signatures".to_string(), "types".to_string()],
            Some(DiagnosticRepair::new(
                "attach generic bounds to the type parameter list",
                Some("fn f[T: Trait](x: T) -> T".to_string()),
            )),
        );
    }
    if message.contains("use `properties { name(params): expr }`") {
        return (
            vec!["properties".to_string(), "signatures".to_string()],
            Some(DiagnosticRepair::new(
                "write source properties as named Bool predicates",
                Some("properties { valid(x: I64): predicate(x) }".to_string()),
            )),
        );
    }
    if message.contains("hole metadata annotations") {
        return (
            vec!["holes".to_string()],
            Some(DiagnosticRepair::new(
                "write holes as `?`, `?name`, or `?name: Type`",
                Some("?todo: I64".to_string()),
            )),
        );
    }
    if message.contains("function annotations") {
        return (
            vec!["signatures".to_string()],
            Some(DiagnosticRepair::new(
                "move intent into `uses`, `budget`, or `properties` clauses",
                None,
            )),
        );
    }
    (vec!["signatures".to_string()], None)
}

fn type_error_concepts(code: sporec_typeck::error::ErrorCode) -> Vec<String> {
    let code = code.to_string();
    match code.as_bytes().first().copied() {
        Some(b'E') | Some(b'R') => vec!["types".to_string(), "signatures".to_string()],
        Some(b'F') => vec!["uses".to_string()],
        Some(b'B') => vec!["budget".to_string()],
        Some(b'H') => vec!["holes".to_string()],
        Some(b'M') => vec!["packages".to_string()],
        Some(b'P') => vec!["properties".to_string()],
        _ => Vec::new(),
    }
}

fn type_error_explanation_key(code: sporec_typeck::error::ErrorCode) -> String {
    format!("diagnostic.{}", code.to_string().to_ascii_lowercase())
}

fn map_typeck_severity(severity: TypeckSeverity) -> Severity {
    match severity {
        TypeckSeverity::Error => Severity::Error,
        TypeckSeverity::Warning => Severity::Warning,
        TypeckSeverity::Info => Severity::Note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sporec_parser::ast::Span;
    use sporec_parser::error::ParseError;
    use sporec_typeck::error::{ErrorCode, TypeError};

    #[test]
    fn converts_type_error_into_canonical_diagnostic() {
        let source = source_file("src/main.sp", "let answer = 42\nanswer + true\n");
        let error = TypeError::with_span(ErrorCode::E0301, "type mismatch", Span::new(16, 22));

        let diagnostic = type_error_to_diagnostic(&source, &error);

        assert_eq!(diagnostic.code, "E0301");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.message, "type mismatch");
        assert_eq!(
            diagnostic.concept_refs,
            vec!["types".to_string(), "signatures".to_string()]
        );
        assert_eq!(
            diagnostic.explanation_key.as_deref(),
            Some("diagnostic.e0301")
        );

        let span = diagnostic.primary_span.expect("primary span");
        assert_eq!(span.file, "src/main.sp");
        assert_eq!(span.range.start.line, 2);
        assert_eq!(span.range.start.col, 1);
        assert_eq!(span.range.end.line, 2);
        assert_eq!(span.range.end.col, 7);
        assert_eq!(span.byte_range(), Some(16..22));
    }

    #[test]
    fn check_source_file_returns_canonical_type_diagnostics() {
        let report = check_source_file("src/main.sp", "fn main() -> I64 { \"oops\" }\n");

        match report {
            SourceCheckReport::Success { .. } => panic!("expected failure"),
            SourceCheckReport::Failure(SourceCheckFailure::Message(message)) => {
                panic!("expected canonical diagnostics, got message: {message}");
            }
            SourceCheckReport::Failure(SourceCheckFailure::Diagnostics {
                source,
                diagnostics,
            }) => {
                assert_eq!(source.name(), "src/main.sp");
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].code, "E0001");
                assert_eq!(diagnostics[0].severity, Severity::Error);
            }
        }
    }

    #[test]
    fn converts_parse_error_into_canonical_diagnostic() {
        let source = source_file("src/main.sp", "fn main( -> I64 { 42 }\n");
        let error = ParseError {
            message: "expected `)`".to_string(),
            span: Span::new(8, 9),
        };

        let diagnostic = parse_error_to_diagnostic(&source, &error);

        assert_eq!(diagnostic.code, "parse-error");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.message, "expected `)`");
        assert!(diagnostic.primary_span.is_some());
        assert_eq!(diagnostic.concept_refs, vec!["signatures".to_string()]);
        assert_eq!(diagnostic.explanation_key.as_deref(), Some("parse-error"));
    }

    #[test]
    fn converts_removed_cost_clause_parse_error_into_budget_teaching_metadata() {
        let source = source_file("src/main.sp", "fn f() -> I64 cost [O(n)] { 1 }\n");
        let error = ParseError {
            message: "use `budget { field: limit }` for signature budgets".to_string(),
            span: Span::new(14, 18),
        };

        let diagnostic = parse_error_to_diagnostic(&source, &error);

        assert_eq!(
            diagnostic.concept_refs,
            vec!["budget".to_string(), "signatures".to_string()]
        );
        let repair = diagnostic.repair.expect("repair");
        assert!(repair.message.contains("named integer fields"));
        assert_eq!(repair.replacement.as_deref(), Some("budget { calls: 1 }"));
    }
}
