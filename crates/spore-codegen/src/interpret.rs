//! Tree-walking interpreter for Spore (PoC execution backend).
//!
//! Evaluates a type-checked AST directly. No compilation step —
//! this is the simplest execution model for the PoC phase.
//! Will be replaced by Cranelift codegen in the prototype phase.

use std::collections::BTreeMap;

use spore_parser::ast::*;

use crate::effect_handler::EffectHandler;
use crate::value::{Closure, Value};

/// Runtime error during evaluation.
#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub message: String,
}

impl RuntimeError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "runtime error: {}", self.message)
    }
}

impl std::error::Error for RuntimeError {}

type Result<T> = std::result::Result<T, RuntimeError>;

/// The tree-walking interpreter.
pub struct Interpreter {
    /// Global function definitions
    functions: BTreeMap<String, FnDef>,
    /// Global struct definitions
    structs: BTreeMap<String, StructDef>,
    /// Global type definitions
    type_defs: BTreeMap<String, TypeDef>,
    /// Effect handlers for capability-gated operations (e.g. I/O).
    effect_handlers: Vec<Box<dyn EffectHandler>>,
}

/// A local variable environment (stack of scopes).
struct Env {
    scopes: Vec<BTreeMap<String, Value>>,
}

impl Env {
    fn new() -> Self {
        Self {
            scopes: vec![BTreeMap::new()],
        }
    }

    fn from_map(map: BTreeMap<String, Value>) -> Self {
        Self { scopes: vec![map] }
    }

    fn push(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: String, val: Value) {
        self.scopes.last_mut().unwrap().insert(name, val);
    }

    fn lookup(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    /// Snapshot all visible bindings (for closure capture).
    fn snapshot(&self) -> BTreeMap<String, Value> {
        let mut map = BTreeMap::new();
        for scope in &self.scopes {
            for (k, v) in scope {
                map.insert(k.clone(), v.clone());
            }
        }
        map
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            structs: BTreeMap::new(),
            type_defs: BTreeMap::new(),
            effect_handlers: Vec::new(),
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
                Item::CapabilityDef(_)
                | Item::ImplDef(_)
                | Item::Import(_)
                | Item::Const(_)
                | Item::Alias(_)
                | Item::CapabilityAlias { .. } => {}
            }
        }
    }

    /// Register an effect handler for capability-gated operations.
    pub fn register_effect_handler(&mut self, handler: Box<dyn EffectHandler>) {
        self.effect_handlers.push(handler);
    }

    /// Try dispatching an operation through registered effect handlers.
    fn try_dispatch_effect(&self, name: &str, args: &[Value]) -> Result<Option<Value>> {
        for handler in &self.effect_handlers {
            if handler.operations().contains(&name) {
                let result = handler.handle(name, args).map_err(RuntimeError::new)?;
                return Ok(Some(result));
            }
        }
        Ok(None)
    }

    /// Call a named function with arguments.
    pub fn call_function(&self, name: &str, args: Vec<Value>) -> Result<Value> {
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

    /// Evaluate an expression.
    fn eval(&self, expr: &Expr, env: &mut Env) -> Result<Value> {
        match expr {
            Expr::IntLit(n) => Ok(Value::Int(*n)),
            Expr::FloatLit(f) => Ok(Value::Float(*f)),
            Expr::StrLit(s) => Ok(Value::Str(s.clone())),
            Expr::BoolLit(b) => Ok(Value::Bool(*b)),
            Expr::FString(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        FStringPart::Literal(s) => result.push_str(s),
                        FStringPart::Expr(e) => {
                            let val = self.eval(e, env)?;
                            result.push_str(&val.to_string());
                        }
                    }
                }
                Ok(Value::Str(result))
            }
            Expr::TString(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        TStringPart::Literal(s) => result.push_str(s),
                        TStringPart::Expr(e) => {
                            let val = self.eval(e, env)?;
                            result.push_str(&val.to_string());
                        }
                    }
                }
                Ok(Value::Str(result))
            }

            Expr::Var(name) => {
                if let Some(val) = env.lookup(name) {
                    Ok(val.clone())
                } else if let Some((_, 0)) = self.is_enum_variant(name) {
                    Ok(Value::Enum(name.clone(), vec![]))
                } else if self.functions.contains_key(name) {
                    // Return a closure wrapping the named function
                    let func = &self.functions[name];
                    Ok(Value::Closure(Closure {
                        params: func.params.iter().map(|p| p.name.clone()).collect(),
                        body: func
                            .body
                            .clone()
                            .unwrap_or(Expr::Hole(name.clone(), None, None)),
                        env: BTreeMap::new(),
                    }))
                } else {
                    Err(RuntimeError::new(format!("undefined variable `{name}`")))
                }
            }

            Expr::BinOp(lhs, op, rhs) => {
                let l = self.eval(lhs, env)?;
                // Short-circuit for logical operators
                match op {
                    BinOp::And => {
                        return if l.as_bool().unwrap_or(false) {
                            self.eval(rhs, env)
                        } else {
                            Ok(Value::Bool(false))
                        };
                    }
                    BinOp::Or => {
                        return if l.as_bool().unwrap_or(false) {
                            Ok(Value::Bool(true))
                        } else {
                            self.eval(rhs, env)
                        };
                    }
                    _ => {}
                }
                let r = self.eval(rhs, env)?;
                self.eval_binop(&l, op, &r)
            }

            Expr::UnaryOp(op, expr) => {
                let val = self.eval(expr, env)?;
                match op {
                    UnaryOp::Neg => match val {
                        Value::Int(n) => Ok(Value::Int(-n)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        _ => Err(RuntimeError::new("cannot negate non-numeric")),
                    },
                    UnaryOp::Not => match val {
                        Value::Bool(b) => Ok(Value::Bool(!b)),
                        _ => Err(RuntimeError::new("cannot apply ! to non-bool")),
                    },
                    UnaryOp::BitNot => match val {
                        Value::Int(n) => Ok(Value::Int(!n)),
                        _ => Err(RuntimeError::new("cannot apply ~ to non-int")),
                    },
                }
            }

            Expr::Call(callee, args) => {
                let arg_vals: Vec<Value> = args
                    .iter()
                    .map(|a| self.eval(a, env))
                    .collect::<Result<_>>()?;

                if let Expr::Var(name) = callee.as_ref() {
                    // 1. Enum variant constructor
                    if let Some((_, arity)) = self.is_enum_variant(name)
                        && arg_vals.len() == arity
                    {
                        return Ok(Value::Enum(name.clone(), arg_vals));
                    }
                    // 2. Effect handler dispatch (capability-gated I/O)
                    if let Some(result) = self.try_dispatch_effect(name, &arg_vals)? {
                        return Ok(result);
                    }
                    // 3. Builtin function (pure Compute operations)
                    if let Some(result) = self.try_call_builtin(name, &arg_vals)? {
                        return Ok(result);
                    }
                    // 4. User-defined function
                    if self.functions.contains_key(name) {
                        return self.call_function(name, arg_vals);
                    }
                }

                // 4. Method call: receiver.method(args)
                if let Expr::FieldAccess(receiver, method) = callee.as_ref() {
                    let recv_val = self.eval(receiver, env)?;
                    let mut full_args = vec![recv_val];
                    full_args.extend(arg_vals.clone());
                    if let Some(result) = self.try_call_builtin(method, &full_args)? {
                        return Ok(result);
                    }
                }

                // 5. Closure / value call
                let callee_val = self.eval(callee, env)?;
                self.call_value(&callee_val, arg_vals)
            }

            Expr::Lambda(params, body) => {
                let captured = env.snapshot();
                Ok(Value::Closure(Closure {
                    params: params.iter().map(|p| p.name.clone()).collect(),
                    body: *body.clone(),
                    env: captured,
                }))
            }

            Expr::If(cond, then_branch, else_branch) => {
                let cond_val = self.eval(cond, env)?;
                if cond_val.as_bool().unwrap_or(false) {
                    self.eval(then_branch, env)
                } else if let Some(else_expr) = else_branch {
                    self.eval(else_expr, env)
                } else {
                    Ok(Value::Unit)
                }
            }

            Expr::Match(scrutinee, arms) => {
                let val = self.eval(scrutinee, env)?;
                for arm in arms {
                    if let Some(bindings) = self.match_pattern(&arm.pattern, &val) {
                        // Check guard
                        if let Some(guard) = &arm.guard {
                            env.push();
                            for (name, v) in &bindings {
                                env.define(name.clone(), v.clone());
                            }
                            let guard_val = self.eval(guard, env)?;
                            env.pop();
                            if !guard_val.as_bool().unwrap_or(false) {
                                continue;
                            }
                        }
                        env.push();
                        for (name, v) in bindings {
                            env.define(name, v);
                        }
                        let result = self.eval(&arm.body, env)?;
                        env.pop();
                        return Ok(result);
                    }
                }
                Err(RuntimeError::new("non-exhaustive match"))
            }

            Expr::Block(stmts, tail) => {
                env.push();
                for stmt in stmts {
                    match stmt {
                        Stmt::Let(name, _, init) => {
                            let val = self.eval(init, env)?;
                            env.define(name.clone(), val);
                        }
                        Stmt::Expr(e) => {
                            self.eval(e, env)?;
                        }
                    }
                }
                let result = if let Some(tail_expr) = tail {
                    self.eval(tail_expr, env)?
                } else {
                    Value::Unit
                };
                env.pop();
                Ok(result)
            }

            Expr::Pipe(lhs, rhs) => {
                let arg = self.eval(lhs, env)?;
                let func = self.eval(rhs, env)?;
                self.call_value(&func, vec![arg])
            }

            Expr::FieldAccess(expr, field) => {
                let val = self.eval(expr, env)?;
                match val {
                    Value::Struct(_, ref fields) => fields
                        .get(field)
                        .cloned()
                        .ok_or_else(|| RuntimeError::new(format!("no field `{field}`"))),
                    _ => Err(RuntimeError::new(format!(
                        "cannot access field `{field}` on {val}"
                    ))),
                }
            }

            Expr::StructLit(name, fields) => {
                let mut map = BTreeMap::new();
                for (fname, fexpr) in fields {
                    let val = self.eval(fexpr, env)?;
                    map.insert(fname.clone(), val);
                }
                Ok(Value::Struct(name.clone(), map))
            }

            Expr::Try(expr) => {
                let val = self.eval(expr, env)?;
                match &val {
                    Value::Enum(variant, fields) if variant == "Ok" && fields.len() == 1 => {
                        Ok(fields[0].clone())
                    }
                    Value::Enum(variant, _) if variant == "Err" => {
                        Err(RuntimeError::new(format!("uncaught error: {val}")))
                    }
                    // If not a Result, pass through (backward compat)
                    _ => Ok(val),
                }
            }

            Expr::Hole(name, _, _) => {
                Err(RuntimeError::new(format!("hit unfilled hole `?{name}`")))
            }

            Expr::Spawn(expr) => {
                // For PoC, just evaluate synchronously
                self.eval(expr, env)
            }

            Expr::Await(expr) => {
                // For PoC, just evaluate synchronously
                self.eval(expr, env)
            }

            Expr::Return(expr) => {
                if let Some(inner) = expr {
                    self.eval(inner, env)
                } else {
                    Ok(Value::Unit)
                }
            }

            Expr::Throw(expr) => {
                let val = self.eval(expr, env)?;
                Err(RuntimeError::new(format!("throw: {val}")))
            }

            Expr::List(elems) => {
                let vals: Vec<Value> = elems
                    .iter()
                    .map(|e| self.eval(e, env))
                    .collect::<Result<_>>()?;
                Ok(Value::List(vals))
            }

            Expr::CharLit(c) => Ok(Value::Char(*c)),

            Expr::ParallelScope { body, .. } => {
                // PoC: synchronous execution
                self.eval(body, env)
            }

            Expr::Select(arms) => {
                // PoC: evaluate first arm synchronously
                if let Some(arm) = arms.first() {
                    let source_val = self.eval(&arm.source, env)?;
                    env.push();
                    env.define(arm.binding.clone(), source_val);
                    let result = self.eval(&arm.body, env)?;
                    env.pop();
                    Ok(result)
                } else {
                    Ok(Value::Unit)
                }
            }

            Expr::Placeholder => {
                unreachable!(
                    "`_` placeholder should have been desugared into a lambda by the parser"
                )
            }
        }
    }

    // ── Binary operations ───────────────────────────────────────────

    fn eval_binop(&self, l: &Value, op: &BinOp, r: &Value) -> Result<Value> {
        match (l, r) {
            (Value::Int(a), Value::Int(b)) => self.int_binop(*a, op, *b),
            (Value::Float(a), Value::Float(b)) => self.float_binop(*a, op, *b),
            (Value::Str(a), Value::Str(b)) => match op {
                BinOp::Add => Ok(Value::Str(format!("{a}{b}"))),
                BinOp::Eq => Ok(Value::Bool(a == b)),
                BinOp::Ne => Ok(Value::Bool(a != b)),
                _ => Err(RuntimeError::new(format!("cannot apply {op:?} to strings"))),
            },
            (Value::Bool(a), Value::Bool(b)) => match op {
                BinOp::Eq => Ok(Value::Bool(a == b)),
                BinOp::Ne => Ok(Value::Bool(a != b)),
                _ => Err(RuntimeError::new(format!(
                    "cannot apply {op:?} to booleans"
                ))),
            },
            _ => Err(RuntimeError::new(format!(
                "type mismatch in binop: {l} {op:?} {r}"
            ))),
        }
    }

    fn int_binop(&self, a: i64, op: &BinOp, b: i64) -> Result<Value> {
        Ok(match op {
            BinOp::Add => Value::Int(a + b),
            BinOp::Sub => Value::Int(a - b),
            BinOp::Mul => Value::Int(a * b),
            BinOp::Div => {
                if b == 0 {
                    return Err(RuntimeError::new("division by zero"));
                }
                Value::Int(a / b)
            }
            BinOp::Mod => {
                if b == 0 {
                    return Err(RuntimeError::new("modulo by zero"));
                }
                Value::Int(a % b)
            }
            BinOp::Eq => Value::Bool(a == b),
            BinOp::Ne => Value::Bool(a != b),
            BinOp::Lt => Value::Bool(a < b),
            BinOp::Gt => Value::Bool(a > b),
            BinOp::Le => Value::Bool(a <= b),
            BinOp::Ge => Value::Bool(a >= b),
            BinOp::BitAnd => Value::Int(a & b),
            BinOp::BitOr => Value::Int(a | b),
            BinOp::BitXor => Value::Int(a ^ b),
            BinOp::Shl => Value::Int(a << b),
            BinOp::Shr => Value::Int(a >> b),
            BinOp::And | BinOp::Or => unreachable!("handled by short-circuit"),
        })
    }

    fn float_binop(&self, a: f64, op: &BinOp, b: f64) -> Result<Value> {
        Ok(match op {
            BinOp::Add => Value::Float(a + b),
            BinOp::Sub => Value::Float(a - b),
            BinOp::Mul => Value::Float(a * b),
            BinOp::Div => Value::Float(a / b),
            BinOp::Mod => Value::Float(a % b),
            BinOp::Eq => Value::Bool(a == b),
            BinOp::Ne => Value::Bool(a != b),
            BinOp::Lt => Value::Bool(a < b),
            BinOp::Gt => Value::Bool(a > b),
            BinOp::Le => Value::Bool(a <= b),
            BinOp::Ge => Value::Bool(a >= b),
            _ => return Err(RuntimeError::new(format!("cannot apply {op:?} to floats"))),
        })
    }

    // ── Call a Value as a function ──────────────────────────────────

    fn call_value(&self, callee: &Value, args: Vec<Value>) -> Result<Value> {
        match callee {
            Value::Closure(closure) => {
                if closure.params.len() != args.len() {
                    return Err(RuntimeError::new(format!(
                        "closure expects {} args, got {}",
                        closure.params.len(),
                        args.len()
                    )));
                }
                let mut env = Env::from_map(closure.env.clone());
                for (name, val) in closure.params.iter().zip(args) {
                    env.define(name.clone(), val);
                }
                self.eval(&closure.body, &mut env)
            }
            Value::Builtin(name) => self
                .try_call_builtin(name, &args)?
                .ok_or_else(|| RuntimeError::new(format!("unknown builtin `{name}`"))),
            _ => Err(RuntimeError::new(format!("cannot call {callee}"))),
        }
    }

    // ── Enum variant lookup ───────────────────────────────────────

    /// Check if a name is a known enum variant. Returns (type_name, arity).
    fn is_enum_variant(&self, name: &str) -> Option<(&str, usize)> {
        for (type_name, typedef) in &self.type_defs {
            for variant in &typedef.variants {
                if variant.name == name {
                    return Some((type_name, variant.fields.len()));
                }
            }
        }
        None
    }

    // ── Builtin functions ───────────────────────────────────────────

    fn try_call_builtin(&self, name: &str, args: &[Value]) -> Result<Option<Value>> {
        match name {
            // ── List builtins ──────────────────────────────────────
            "len" => {
                let list = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("len: missing argument"))?;
                match list {
                    Value::List(v) => Ok(Some(Value::Int(v.len() as i64))),
                    Value::Str(s) => Ok(Some(Value::Int(s.len() as i64))),
                    _ => Err(RuntimeError::new(format!(
                        "len: expected List or String, got {}",
                        list.type_name()
                    ))),
                }
            }
            "map" => {
                let list = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("map: missing list"))?
                    .as_list()
                    .map_err(RuntimeError::new)?;
                let f = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("map: missing function"))?;
                let results: Vec<Value> = list
                    .iter()
                    .map(|item| self.call_value(f, vec![item.clone()]))
                    .collect::<Result<_>>()?;
                Ok(Some(Value::List(results)))
            }
            "filter" => {
                let list = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("filter: missing list"))?
                    .as_list()
                    .map_err(RuntimeError::new)?;
                let pred = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("filter: missing predicate"))?;
                let mut results = Vec::new();
                for item in list {
                    let v = self.call_value(pred, vec![item.clone()])?;
                    if v.as_bool().unwrap_or(false) {
                        results.push(item.clone());
                    }
                }
                Ok(Some(Value::List(results)))
            }
            "fold" => {
                let list = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("fold: missing list"))?
                    .as_list()
                    .map_err(RuntimeError::new)?;
                let init = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("fold: missing init"))?
                    .clone();
                let f = args
                    .get(2)
                    .ok_or_else(|| RuntimeError::new("fold: missing function"))?;
                let mut acc = init;
                for item in list {
                    acc = self.call_value(f, vec![acc, item.clone()])?;
                }
                Ok(Some(acc))
            }
            "each" => {
                let list = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("each: missing list"))?
                    .as_list()
                    .map_err(RuntimeError::new)?;
                let f = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("each: missing function"))?;
                for item in list {
                    self.call_value(f, vec![item.clone()])?;
                }
                Ok(Some(Value::Unit))
            }
            "append" => {
                let list = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("append: missing list"))?
                    .as_list()
                    .map_err(RuntimeError::new)?;
                let item = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("append: missing item"))?;
                let mut new_list = list.clone();
                new_list.push(item.clone());
                Ok(Some(Value::List(new_list)))
            }
            "prepend" => {
                let item = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("prepend: missing item"))?;
                let list = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("prepend: missing list"))?
                    .as_list()
                    .map_err(RuntimeError::new)?;
                let mut new_list = vec![item.clone()];
                new_list.extend(list.iter().cloned());
                Ok(Some(Value::List(new_list)))
            }
            "head" => {
                let list = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("head: missing list"))?
                    .as_list()
                    .map_err(RuntimeError::new)?;
                list.first()
                    .cloned()
                    .map(Some)
                    .ok_or_else(|| RuntimeError::new("head: empty list"))
            }
            "tail" => {
                let list = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("tail: missing list"))?
                    .as_list()
                    .map_err(RuntimeError::new)?;
                if list.is_empty() {
                    return Err(RuntimeError::new("tail: empty list"));
                }
                Ok(Some(Value::List(list[1..].to_vec())))
            }
            "reverse" => {
                let list = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("reverse: missing list"))?
                    .as_list()
                    .map_err(RuntimeError::new)?;
                let mut rev = list.clone();
                rev.reverse();
                Ok(Some(Value::List(rev)))
            }
            "range" => {
                let start = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("range: missing start"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("range: start must be Int"))?;
                let end = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("range: missing end"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("range: end must be Int"))?;
                let list: Vec<Value> = (start..end).map(Value::Int).collect();
                Ok(Some(Value::List(list)))
            }
            "contains" => {
                let list = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("contains: missing list"))?
                    .as_list()
                    .map_err(RuntimeError::new)?;
                let item = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("contains: missing item"))?;
                let found = list.iter().any(|v| value_eq(v, item));
                Ok(Some(Value::Bool(found)))
            }

            // ── String builtins ────────────────────────────────────
            "string_length" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("string_length: missing arg"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("string_length: expected String"))?;
                Ok(Some(Value::Int(s.len() as i64)))
            }
            "split" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("split: missing string"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("split: expected String"))?
                    .to_owned();
                let sep = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("split: missing separator"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("split: separator must be String"))?
                    .to_owned();
                let parts: Vec<Value> = s.split(&sep).map(|p| Value::Str(p.to_owned())).collect();
                Ok(Some(Value::List(parts)))
            }
            "trim" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("trim: missing arg"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("trim: expected String"))?;
                Ok(Some(Value::Str(s.trim().to_owned())))
            }
            "to_upper" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("to_upper: missing arg"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("to_upper: expected String"))?;
                Ok(Some(Value::Str(s.to_uppercase())))
            }
            "to_lower" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("to_lower: missing arg"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("to_lower: expected String"))?;
                Ok(Some(Value::Str(s.to_lowercase())))
            }
            "starts_with" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("starts_with: missing string"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("starts_with: expected String"))?
                    .to_owned();
                let prefix = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("starts_with: missing prefix"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("starts_with: prefix must be String"))?
                    .to_owned();
                Ok(Some(Value::Bool(s.starts_with(&prefix))))
            }
            "ends_with" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("ends_with: missing string"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("ends_with: expected String"))?
                    .to_owned();
                let suffix = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("ends_with: missing suffix"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("ends_with: suffix must be String"))?
                    .to_owned();
                Ok(Some(Value::Bool(s.ends_with(&suffix))))
            }
            "char_at" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("char_at: missing string"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("char_at: expected String"))?
                    .to_owned();
                let idx_i64 = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("char_at: missing index"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("char_at: index must be Int"))?;
                if idx_i64 < 0 {
                    return Err(RuntimeError::new(format!(
                        "char_at: index cannot be negative, got {idx_i64}"
                    )));
                }
                let idx = idx_i64 as usize;
                let ch = s.chars().nth(idx).ok_or_else(|| {
                    RuntimeError::new(format!("char_at: index {idx} out of bounds"))
                })?;
                Ok(Some(Value::Str(ch.to_string())))
            }
            "substring" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("substring: missing string"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("substring: expected String"))?
                    .to_owned();
                let start_i64 = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("substring: missing start"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("substring: start must be Int"))?;
                if start_i64 < 0 {
                    return Err(RuntimeError::new(format!(
                        "substring: start cannot be negative, got {start_i64}"
                    )));
                }
                let start = start_i64 as usize;
                let end_i64 = args
                    .get(2)
                    .ok_or_else(|| RuntimeError::new("substring: missing end"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("substring: end must be Int"))?;
                if end_i64 < 0 {
                    return Err(RuntimeError::new(format!(
                        "substring: end cannot be negative, got {end_i64}"
                    )));
                }
                let end = end_i64 as usize;
                let sub: String = s
                    .chars()
                    .skip(start)
                    .take(end.saturating_sub(start))
                    .collect();
                Ok(Some(Value::Str(sub)))
            }
            "replace" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("replace: missing string"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("replace: expected String"))?
                    .to_owned();
                let from = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("replace: missing from"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("replace: from must be String"))?
                    .to_owned();
                let to = args
                    .get(2)
                    .ok_or_else(|| RuntimeError::new("replace: missing to"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("replace: to must be String"))?
                    .to_owned();
                Ok(Some(Value::Str(s.replace(&from, &to))))
            }
            "to_string" => {
                let val = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("to_string: missing arg"))?;
                Ok(Some(Value::Str(val.to_string())))
            }

            // ── Math builtins ──────────────────────────────────────
            "abs" => {
                let n = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("abs: missing arg"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("abs: expected Int"))?;
                Ok(Some(Value::Int(n.saturating_abs())))
            }
            "min" => {
                let a = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("min: missing first arg"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("min: expected Int"))?;
                let b = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("min: missing second arg"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("min: expected Int"))?;
                Ok(Some(Value::Int(a.min(b))))
            }
            "max" => {
                let a = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("max: missing first arg"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("max: expected Int"))?;
                let b = args
                    .get(1)
                    .ok_or_else(|| RuntimeError::new("max: missing second arg"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("max: expected Int"))?;
                Ok(Some(Value::Int(a.max(b))))
            }

            // ── IO operations are dispatched through effect handlers ──
            // (print, println, read_line are handled by CliPlatformHandler)
            _ => Ok(None),
        }
    }

    // ── Pattern matching ────────────────────────────────────────────

    fn match_pattern(&self, pat: &Pattern, val: &Value) -> Option<Vec<(String, Value)>> {
        match pat {
            Pattern::Wildcard => Some(vec![]),
            Pattern::Var(name) => Some(vec![(name.clone(), val.clone())]),
            Pattern::IntLit(n) => {
                if val.as_int() == Some(*n) {
                    Some(vec![])
                } else {
                    None
                }
            }
            Pattern::StrLit(s) => {
                if val.as_str() == Some(s) {
                    Some(vec![])
                } else {
                    None
                }
            }
            Pattern::BoolLit(b) => {
                if val.as_bool() == Some(*b) {
                    Some(vec![])
                } else {
                    None
                }
            }
            Pattern::Constructor(name, sub_pats) => {
                // Match against Enum variant
                if let Value::Enum(vname, fields) = val {
                    if vname != name {
                        return None;
                    }
                    if fields.len() != sub_pats.len() {
                        return None;
                    }
                    let mut bindings = Vec::new();
                    for (sp, field_val) in sub_pats.iter().zip(fields.iter()) {
                        let sub_bindings = self.match_pattern(sp, field_val)?;
                        bindings.extend(sub_bindings);
                    }
                    return Some(bindings);
                }
                // Also match against Struct with numeric field names (legacy)
                if let Value::Struct(vname, fields) = val {
                    if vname != name {
                        return None;
                    }
                    let mut bindings = Vec::new();
                    for (i, sp) in sub_pats.iter().enumerate() {
                        let field_val = fields.get(&i.to_string())?;
                        let sub_bindings = self.match_pattern(sp, field_val)?;
                        bindings.extend(sub_bindings);
                    }
                    Some(bindings)
                } else if sub_pats.is_empty()
                    && matches!(val, Value::Enum(vname, fields) if vname == name && fields.is_empty())
                {
                    Some(vec![])
                } else {
                    None
                }
            }
            Pattern::Struct(name, field_pats) => {
                if let Value::Struct(sname, fields) = val {
                    if sname != name {
                        return None;
                    }
                    let mut bindings = Vec::new();
                    for (fname, fpat) in field_pats {
                        let fval = fields.get(fname)?;
                        let sub_bindings = self.match_pattern(fpat, fval)?;
                        bindings.extend(sub_bindings);
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
            Pattern::Or(alternatives) => {
                for alt in alternatives {
                    if let Some(bindings) = self.match_pattern(alt, val) {
                        return Some(bindings);
                    }
                }
                None
            }
            Pattern::List(elements, rest) => {
                if let Value::List(items) = val {
                    if rest.is_some() {
                        if items.len() < elements.len() {
                            return None;
                        }
                    } else if items.len() != elements.len() {
                        return None;
                    }
                    let mut bindings = Vec::new();
                    for (pat, item) in elements.iter().zip(items.iter()) {
                        let sub = self.match_pattern(pat, item)?;
                        bindings.extend(sub);
                    }
                    if let Some(rest_name) = rest {
                        let rest_items = items[elements.len()..].to_vec();
                        bindings.push((rest_name.clone(), Value::List(rest_items)));
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
        }
    }
}

/// Structural equality for values (used by `contains`).
fn value_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Char(x), Value::Char(y)) => x == y,
        (Value::Unit, Value::Unit) => true,
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| value_eq(a, b))
        }
        (Value::Enum(n1, f1), Value::Enum(n2, f2)) => {
            n1 == n2
                && f1.len() == f2.len()
                && f1.iter().zip(f2.iter()).all(|(a, b)| value_eq(a, b))
        }
        _ => false,
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}
