use sporec_diagnostics::{Diagnostic, Severity, SourceFile};
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
    Diagnostic::new("parse-error", Severity::Error, error.message.clone())
        .with_primary_span(source.span(error.span.start..error.span.end))
}

pub fn type_error_to_diagnostic(source: &SourceFile, error: &TypeError) -> Diagnostic {
    let mut diagnostic = Diagnostic::new(
        error.code.to_string(),
        map_typeck_severity(error.code.severity()),
        error.message.clone(),
    );

    if let Some(span) = error.span {
        diagnostic = diagnostic.with_primary_span(source.span(span.start..span.end));
    }

    diagnostic
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
        let report = check_source_file("src/main.sp", "fn main() -> I32 { \"oops\" }\n");

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
        let source = source_file("src/main.sp", "fn main( -> I32 { 42 }\n");
        let error = ParseError {
            message: "expected `)`".to_string(),
            span: Span::new(8, 9),
        };

        let diagnostic = parse_error_to_diagnostic(&source, &error);

        assert_eq!(diagnostic.code, "parse-error");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.message, "expected `)`");
        assert!(diagnostic.primary_span.is_some());
    }
}
