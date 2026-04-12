//! Error types for lexing and parsing.

use crate::lexer::Span;

#[derive(Debug, Clone)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "lex error at {}-{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "parse error at {}-{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

impl std::error::Error for LexError {}
impl std::error::Error for ParseError {}
