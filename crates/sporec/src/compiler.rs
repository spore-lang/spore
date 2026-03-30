use spore_parser::parse;

/// Compile Spore source code to output.
///
/// This is the core compiler pipeline:
/// 1. Parse (source text → AST)
/// 2. Type check (AST → Typed AST)
/// 3. Code gen (Typed AST → native code)
pub fn compile(source: &str) -> Result<(), String> {
    let _ast = parse(source)?;
    // TODO: type checking
    // TODO: code generation
    Ok(())
}
