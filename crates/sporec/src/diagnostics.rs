use spore_typeck::error::{Severity as TypeckSeverity, TypeError};
use sporec_diagnostics::{Diagnostic, Severity, SourceFile};

pub fn source_file(name: impl Into<String>, contents: impl Into<String>) -> SourceFile {
    SourceFile::new(name, contents)
}

pub fn diagnostics_for_type_errors(source: &SourceFile, errors: &[TypeError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|error| type_error_to_diagnostic(source, error))
        .collect()
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
    use spore_parser::ast::Span;
    use spore_typeck::error::{ErrorCode, TypeError};

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
}
