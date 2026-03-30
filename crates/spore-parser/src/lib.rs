/// spore-parser — Spore language parser
///
/// Transforms source text into an Abstract Syntax Tree (AST).
/// Pure function: &str → Result<Ast, ParseError>
pub mod ast;

/// Parse Spore source code into an AST.
pub fn parse(source: &str) -> Result<ast::Module, String> {
    // TODO: implement lexer + parser
    let _ = source;
    Ok(ast::Module {
        name: String::new(),
        items: vec![],
    })
}
