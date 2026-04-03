/// spore-codegen — Spore code generation / execution
///
/// PoC: tree-walking interpreter for direct AST evaluation.
/// Prototype: will add Cranelift backend for native compilation.
pub mod backend;
pub mod effect_handler;
pub mod interpret;
pub mod value;

use effect_handler::CliPlatformHandler;
use interpret::{Interpreter, RuntimeError};
use spore_parser::ast::Module;
use value::Value;

/// Execute a Spore module by calling its `main` function.
pub fn run(module: &Module) -> Result<Value, RuntimeError> {
    let mut interp = Interpreter::new();
    interp.register_effect_handler(Box::new(CliPlatformHandler));
    interp.load_module(module);
    interp.call_function("main", vec![])
}

/// Execute a named function with arguments.
pub fn call(module: &Module, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let mut interp = Interpreter::new();
    interp.register_effect_handler(Box::new(CliPlatformHandler));
    interp.load_module(module);
    interp.call_function(name, args)
}
