//! Tree-walking interpreter for Spore (PoC execution backend).
//!
//! Evaluates a type-checked AST directly. No compilation step —
//! this is the simplest execution model for the PoC phase.
//! Will be replaced by Cranelift codegen in the prototype phase.

use std::collections::BTreeMap;

use spore_parser::ast::*;

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
                Item::CapabilityDef(_) | Item::Import(_) => {}
            }
        }
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

            Expr::Var(name) => {
                if let Some(val) = env.lookup(name) {
                    Ok(val.clone())
                } else if self.functions.contains_key(name) {
                    // Return a closure wrapping the named function
                    let func = &self.functions[name];
                    Ok(Value::Closure(Closure {
                        params: func.params.iter().map(|p| p.name.clone()).collect(),
                        body: func.body.clone().unwrap_or(Expr::Hole(name.clone(), None)),
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

                // Direct function call by name
                if let Expr::Var(name) = callee.as_ref()
                    && self.functions.contains_key(name)
                {
                    return self.call_function(name, arg_vals);
                }
                // Method call: Expr::FieldAccess was turned into Call(FieldAccess(...), args)
                // For now just evaluate callee and call as closure
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
                // For PoC, try just evaluates the inner expression
                self.eval(expr, env)
            }

            Expr::Hole(name, _) => Err(RuntimeError::new(format!("hit unfilled hole `?{name}`"))),

            Expr::Spawn(expr) => {
                // For PoC, just evaluate synchronously
                self.eval(expr, env)
            }

            Expr::Await(expr) => {
                // For PoC, just evaluate synchronously
                self.eval(expr, env)
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
            _ => Err(RuntimeError::new(format!("cannot call {callee}"))),
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
                // For enum variants: match against Struct("VariantName", fields)
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
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}
