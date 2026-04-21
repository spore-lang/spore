//! Tree-walking interpreter for Spore (PoC execution backend).
//!
//! Evaluates a type-checked AST directly. No compilation step —
//! this is the simplest execution model for the PoC phase.
//! Will be replaced by Cranelift codegen in the prototype phase.

mod builtins;
mod env;
mod error;
mod eval;
mod pattern;

use std::collections::BTreeMap;

use sporec_parser::ast::*;

use crate::effect_handler::EffectHandler;
use crate::value::{TaskHandle, Value};

use env::{Env, RuntimeEffectArm, named_function_closure};
use error::Result;
pub use error::RuntimeError;

/// The tree-walking interpreter.
pub struct Interpreter {
    /// Global function definitions
    functions: BTreeMap<String, FnDef>,
    /// Global struct definitions
    structs: BTreeMap<String, StructDef>,
    /// Global type definitions
    type_defs: BTreeMap<String, TypeDef>,
    /// Global named handler definitions.
    handlers: BTreeMap<String, HandlerDef>,
    /// Effect handlers for capability-gated operations (e.g. I/O).
    effect_handlers: Vec<Box<dyn EffectHandler>>,
    /// Stack of handler frames installed by `handle ... with { ... }`.
    handler_stack: Vec<Vec<RuntimeEffectArm>>,
    /// Active task scopes for `parallel_scope`.
    task_scopes: Vec<Vec<TaskHandle>>,
    /// Rotation cursor used to avoid fixed-priority `select` behavior.
    select_cursor: usize,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            structs: BTreeMap::new(),
            type_defs: BTreeMap::new(),
            handlers: BTreeMap::new(),
            effect_handlers: Vec::new(),
            handler_stack: Vec::new(),
            task_scopes: Vec::new(),
            select_cursor: 0,
        }
    }

    /// Load the prelude (embedded at compile time).
    pub fn load_prelude(&mut self) {
        let source = include_str!("../../../../stdlib/prelude.sp");
        if let Ok(module) = sporec_parser::parse(source) {
            self.load_module(&module);
        }
    }

    /// Load a module's declarations.
    pub fn load_module(&mut self, module: &Module) {
        for item in &module.items {
            match item {
                Item::Function(f) => {
                    self.functions.insert(f.name.clone(), f.clone());
                }
                Item::StructDef(s) => {
                    self.structs.insert(s.name.clone(), s.clone());
                }
                Item::TypeDef(t) => {
                    self.type_defs.insert(t.name.clone(), t.clone());
                }
                Item::HandlerDef(h) => {
                    self.handlers.insert(h.name.clone(), h.clone());
                }
                Item::CapabilityDef(_)
                | Item::ImplDef(_)
                | Item::Import(_)
                | Item::Const(_)
                | Item::Alias(_)
                | Item::CapabilityAlias { .. }
                | Item::TraitDef(_)
                | Item::EffectDef(_)
                | Item::EffectAlias(_) => {}
            }
        }
    }

    /// Load public functions, structs, and types from an imported module.
    ///
    /// Symbols are registered under both their qualified name
    /// (`module_path.name`) and their unqualified name so that imported
    /// code can be called directly.
    pub fn load_module_functions(&mut self, module_path: &str, module: &Module) {
        for item in &module.items {
            match item {
                Item::Function(f)
                    if matches!(f.visibility, Visibility::Pub | Visibility::PubPkg) =>
                {
                    let qualified = format!("{module_path}.{}", f.name);
                    self.functions.insert(qualified, f.clone());
                    self.functions
                        .entry(f.name.clone())
                        .or_insert_with(|| f.clone());
                }
                Item::StructDef(s)
                    if matches!(s.visibility, Visibility::Pub | Visibility::PubPkg) =>
                {
                    self.structs
                        .entry(s.name.clone())
                        .or_insert_with(|| s.clone());
                }
                Item::TypeDef(t)
                    if matches!(t.visibility, Visibility::Pub | Visibility::PubPkg) =>
                {
                    self.type_defs
                        .entry(t.name.clone())
                        .or_insert_with(|| t.clone());
                }
                Item::HandlerDef(h) => {
                    let qualified = format!("{module_path}.{}", h.name);
                    self.handlers.insert(qualified, h.clone());
                    self.handlers
                        .entry(h.name.clone())
                        .or_insert_with(|| h.clone());
                }
                _ => {}
            }
        }
    }

    /// Register an effect handler for capability-gated operations.
    pub fn register_effect_handler(&mut self, handler: Box<dyn EffectHandler>) {
        self.effect_handlers.push(handler);
    }

    /// Get a named function as a first-class closure value.
    pub fn named_function_value(&self, name: &str) -> Result<Value> {
        let func = self
            .functions
            .get(name)
            .ok_or_else(|| RuntimeError::new(format!("unknown function `{name}`")))?;
        Ok(named_function_closure(name, func))
    }

    /// Call a named function with arguments.
    pub fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value> {
        let func = self
            .functions
            .get(name)
            .ok_or_else(|| RuntimeError::new(format!("undefined function `{name}`")))?
            .clone();

        if func.params.len() != args.len() {
            return Err(RuntimeError::new(format!(
                "function `{name}` expects {} args, got {}",
                func.params.len(),
                args.len()
            )));
        }

        let mut env = Env::new();
        for (param, arg) in func.params.iter().zip(args) {
            env.define(param.name.clone(), arg);
        }

        match &func.body {
            Some(body) => self.eval(body, &mut env),
            None if func.is_foreign => Err(RuntimeError::new(format!(
                "foreign function `{name}` is not available in interpreter mode"
            ))),
            None => Err(RuntimeError::new(format!(
                "function `{name}` has no body (hole)"
            ))),
        }
    }

    /// Evaluate an expression in a fresh environment (for spec clauses).
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value> {
        let mut env = Env::new();
        self.eval(expr, &mut env)
    }

    /// Return all functions that have a spec clause, paired with their name.
    pub fn functions_with_specs(&self) -> Vec<(String, FnDef)> {
        self.functions
            .iter()
            .filter(|(_, f)| f.spec_clause.is_some())
            .map(|(name, f)| (name.clone(), f.clone()))
            .collect()
    }

    /// Public wrapper around `call_value` for use by spec evaluation.
    pub fn call_value_pub(&mut self, callee: &Value, args: Vec<Value>) -> Result<Value> {
        self.call_value(callee, args)
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}
