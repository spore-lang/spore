/// spore-parser — Spore language parser
///
/// Transforms source text into an Abstract Syntax Tree (AST).
/// Pure function: &str → Result<Ast, ParseError>
pub mod ast;
pub mod error;
pub mod formatter;
pub mod lexer;
pub mod parser;

use error::ParseError;
use lexer::Lexer;
use parser::Parser;

/// Parse Spore source code into an AST.
pub fn parse(source: &str) -> Result<ast::Module, Vec<ParseError>> {
    let tokens = Lexer::new(source).tokenize().map_err(|errs| {
        errs.into_iter()
            .map(|e| ParseError {
                message: e.message,
                span: e.span,
            })
            .collect::<Vec<_>>()
    })?;
    let mut parser = Parser::new(tokens);
    parser.parse_module().map_err(|e| vec![e])
}
