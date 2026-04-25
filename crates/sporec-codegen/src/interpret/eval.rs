use std::collections::BTreeMap;
use std::{cell::RefCell, rc::Rc};

use sporec_parser::ast::*;

use crate::value::{ChannelEndpoint, ChannelState, Closure, TaskHandle, TaskState, Value};

use super::Interpreter;
use super::env::{Env, named_function_closure};
use super::error::{Result, RuntimeError, StringChunk};

impl Interpreter {
    /// Evaluate an interpolated string template (shared by f-strings and t-strings).
    pub(super) fn eval_interpolated_string<'a>(
        &mut self,
        parts: impl Iterator<Item = StringChunk<'a>>,
        env: &mut Env,
    ) -> Result<Value> {
        let mut result = String::new();
        for chunk in parts {
            match chunk {
                StringChunk::Literal(s) => result.push_str(s),
                StringChunk::Expr(e) => {
                    let val = self.eval(e, env)?;
                    result.push_str(&val.to_string());
                }
            }
        }
        Ok(Value::Str(result))
    }

    /// Evaluate an expression.
    pub(super) fn eval(&mut self, expr: &Expr, env: &mut Env) -> Result<Value> {
        match expr {
            Expr::IntLit(n) => Ok(Value::Int(*n)),
            Expr::FloatLit(f) => Ok(Value::Float(*f)),
            Expr::StrLit(s) => Ok(Value::Str(s.clone())),
            Expr::BoolLit(b) => Ok(Value::Bool(*b)),
            Expr::FString(parts) => {
                self.eval_interpolated_string(parts.iter().map(StringChunk::from), env)
            }
            Expr::TString(parts) => {
                self.eval_interpolated_string(parts.iter().map(StringChunk::from), env)
            }
            Expr::Var(name) => {
                if let Some(val) = env.lookup(name) {
                    Ok(val.clone())
                } else if let Some((_, 0)) = self.is_enum_variant(name) {
                    Ok(Value::Enum(name.clone(), vec![]))
                } else if self.functions.contains_key(name) {
                    Ok(named_function_closure(name, &self.functions[name]))
                } else {
                    Err(RuntimeError::new(format!("undefined variable `{name}`")))
                }
            }
            Expr::BinOp(lhs, op, rhs) => {
                let l = self.eval(lhs, env)?;
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
                        Value::Int(n) => match n.checked_neg() {
                            Some(v) => Ok(Value::Int(v)),
                            None => Err(RuntimeError::new(format!("integer overflow: -{n}"))),
                        },
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
                    if let Some((_, arity)) = self.is_enum_variant(name)
                        && arg_vals.len() == arity
                    {
                        return Ok(Value::Enum(name.clone(), arg_vals));
                    }
                    let dispatch_to_effect_handler = self
                        .functions
                        .get(name)
                        .is_none_or(|function| function.is_foreign);
                    if dispatch_to_effect_handler
                        && let Some(result) = self.try_dispatch_effect(name, &arg_vals)?
                    {
                        return Ok(result);
                    }
                    if let Some(result) = self.try_call_builtin(name, &arg_vals)? {
                        return Ok(result);
                    }
                    if self.functions.contains_key(name) {
                        return self.call_function(name, arg_vals);
                    }
                }

                if let Expr::FieldAccess(receiver, method) = callee.as_ref() {
                    let recv_val = self.eval(receiver, env)?;
                    let mut full_args = vec![recv_val];
                    full_args.extend(arg_vals.clone());
                    if let Some(result) = self.try_call_builtin(method, &full_args)? {
                        return Ok(result);
                    }
                }

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
                    _ => Ok(val),
                }
            }
            Expr::Hole(name, _, _, _) => {
                let label = name.as_deref().unwrap_or("_");
                Err(RuntimeError::new(format!("hit unfilled hole `?{label}`")))
            }
            Expr::Spawn(expr) => {
                let task = TaskHandle {
                    state: Rc::new(RefCell::new(TaskState::Pending {
                        expr: (**expr).clone(),
                        env: env.snapshot(),
                    })),
                };
                self.register_spawned_task(&task);
                Ok(Value::Task(task))
            }
            Expr::Await(expr) => {
                let task_val = self.eval(expr, env)?;
                match task_val {
                    Value::Task(task) => self.run_task(&task),
                    other => Err(RuntimeError::new(format!(
                        "await expects Task, got {}",
                        other.type_name()
                    ))),
                }
            }
            Expr::ChannelNew { buffer, .. } => {
                let buffer_val = self.eval(buffer, env)?;
                let raw_size = buffer_val
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("Channel.new buffer must be I32"))?;
                if raw_size < 0 {
                    return Err(RuntimeError::new(format!(
                        "Channel.new buffer must be >= 0, got {raw_size}"
                    )));
                }
                let state = Rc::new(RefCell::new(ChannelState::new(raw_size as usize)));
                Ok(Value::List(vec![
                    Value::Sender(ChannelEndpoint {
                        state: Rc::clone(&state),
                    }),
                    Value::Receiver(ChannelEndpoint { state }),
                ]))
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
            Expr::ParallelScope { lanes, body } => {
                if let Some(lanes_expr) = lanes {
                    let lanes_value = self.eval(lanes_expr, env)?;
                    let lanes_int = lanes_value
                        .as_int()
                        .ok_or_else(|| RuntimeError::new("parallel_scope lanes must be I32"))?;
                    if lanes_int <= 0 {
                        return Err(RuntimeError::new(format!(
                            "parallel_scope lanes must be > 0, got {lanes_int}"
                        )));
                    }
                }

                self.task_scopes.push(Vec::new());
                let body_result = self.eval(body, env);
                let scoped_tasks = self.task_scopes.pop().unwrap_or_default();

                match body_result {
                    Ok(value) => {
                        for task in scoped_tasks {
                            self.run_task(&task)?;
                        }
                        Ok(value)
                    }
                    Err(err) => {
                        for task in scoped_tasks {
                            Self::cancel_task_if_pending(&task);
                        }
                        Err(err)
                    }
                }
            }
            Expr::Select(arms) => {
                let mut timeout_arm: Option<(&Expr, &Expr)> = None;
                let mut recv_arms: Vec<(String, ChannelEndpoint, &Expr)> = Vec::new();
                for arm in arms {
                    match arm {
                        SelectArm::Recv {
                            binding,
                            source,
                            body,
                        } => {
                            let source_val = self.eval(source, env)?;
                            match source_val {
                                Value::Receiver(endpoint) => {
                                    recv_arms.push((binding.clone(), endpoint, body));
                                }
                                _ => {
                                    return Err(RuntimeError::new(
                                        "select recv arm expects a Receiver source",
                                    ));
                                }
                            }
                        }
                        SelectArm::Timeout { duration, body } => {
                            timeout_arm = Some((duration, body));
                        }
                    }
                }

                if !recv_arms.is_empty() {
                    let start = self.select_cursor % recv_arms.len();
                    for offset in 0..recv_arms.len() {
                        let idx = (start + offset) % recv_arms.len();
                        let (binding, endpoint, body) = &recv_arms[idx];
                        if let Some(msg) = endpoint.state.borrow_mut().queue.pop_front() {
                            self.select_cursor = idx + 1;
                            env.push();
                            env.define(binding.clone(), msg);
                            let result = self.eval(body, env);
                            env.pop();
                            return result;
                        }
                    }
                }

                if let Some((duration, body)) = timeout_arm {
                    let duration_value = self.eval(duration, env)?;
                    let duration_int = duration_value
                        .as_int()
                        .ok_or_else(|| RuntimeError::new("select timeout expects Int duration"))?;
                    if duration_int < 0 {
                        return Err(RuntimeError::new(format!(
                            "select timeout must be >= 0, got {duration_int}"
                        )));
                    }
                    self.eval(body, env)
                } else {
                    Ok(Value::Unit)
                }
            }
            Expr::Placeholder => {
                unreachable!(
                    "`_` placeholder should have been desugared into a lambda by the parser"
                )
            }
            Expr::Perform {
                effect,
                operation,
                args,
            } => {
                let arg_vals: Vec<Value> = args
                    .iter()
                    .map(|a| self.eval(a, env))
                    .collect::<Result<_>>()?;

                for frame in self.handler_stack.iter().rev() {
                    for arm in frame {
                        if arm.effect == *effect && arm.operation == *operation {
                            let arm = arm.clone();
                            let mut handler_env = Env::from_map(arm.captured_env.clone());
                            for (param, val) in arm.params.iter().zip(arg_vals.iter()) {
                                handler_env.define(param.clone(), val.clone());
                            }
                            return self.eval(&arm.body, &mut handler_env);
                        }
                    }
                }

                if let Some(result) = self.try_dispatch_effect(operation, &arg_vals)? {
                    return Ok(result);
                }

                Err(RuntimeError::new(format!(
                    "unhandled effect: {effect}.{operation}"
                )))
            }
            Expr::Handle { body, handlers } => {
                let frame = self.materialize_handle_bindings(handlers, env)?;
                self.handler_stack.push(frame);
                let result = self.eval(body, env);
                self.handler_stack.pop();
                result
            }
        }
    }

    pub(super) fn eval_binop(&mut self, l: &Value, op: &BinOp, r: &Value) -> Result<Value> {
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

    pub(super) fn int_binop(&mut self, a: i64, op: &BinOp, b: i64) -> Result<Value> {
        Ok(match op {
            BinOp::Add => match a.checked_add(b) {
                Some(v) => Value::Int(v),
                None => return Err(RuntimeError::new(format!("integer overflow: {a} + {b}"))),
            },
            BinOp::Sub => match a.checked_sub(b) {
                Some(v) => Value::Int(v),
                None => return Err(RuntimeError::new(format!("integer overflow: {a} - {b}"))),
            },
            BinOp::Mul => match a.checked_mul(b) {
                Some(v) => Value::Int(v),
                None => return Err(RuntimeError::new(format!("integer overflow: {a} * {b}"))),
            },
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
            BinOp::Shl => {
                if !(0..64).contains(&b) {
                    return Err(RuntimeError::new(format!(
                        "shift amount {b} out of range 0..63"
                    )));
                }
                Value::Int(a << b)
            }
            BinOp::Shr => {
                if !(0..64).contains(&b) {
                    return Err(RuntimeError::new(format!(
                        "shift amount {b} out of range 0..63"
                    )));
                }
                Value::Int(a >> b)
            }
            BinOp::And | BinOp::Or => unreachable!("handled by short-circuit"),
        })
    }

    pub(super) fn float_binop(&mut self, a: f64, op: &BinOp, b: f64) -> Result<Value> {
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

    pub(super) fn call_value(&mut self, callee: &Value, args: Vec<Value>) -> Result<Value> {
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

    /// Check if a name is a known enum variant. Returns (type_name, arity).
    pub(super) fn is_enum_variant(&mut self, name: &str) -> Option<(&str, usize)> {
        for (type_name, typedef) in &self.type_defs {
            for variant in &typedef.variants {
                if variant.name == name {
                    return Some((type_name, variant.fields.len()));
                }
            }
        }
        None
    }
}
