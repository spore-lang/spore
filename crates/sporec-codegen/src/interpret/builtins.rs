use std::collections::{BTreeMap, BTreeSet};

use sporec_parser::ast::HandleBinding;

use crate::effect_handler::EffectOutcome;
use crate::value::{TaskHandle, TaskState, Value};

use super::Interpreter;
use super::env::{Env, RuntimeEffectArm};
use super::error::{Result, RuntimeError, require_arg, require_int, require_list, require_str};

impl Interpreter {
    pub(super) fn register_spawned_task(&mut self, task: &TaskHandle) {
        if let Some(scope) = self.task_scopes.last_mut() {
            scope.push(task.clone());
        }
    }

    pub(super) fn run_task(&mut self, task: &TaskHandle) -> Result<Value> {
        let pending = {
            let state = task.state.borrow();
            match &*state {
                TaskState::Completed(value) => return Ok(value.clone()),
                TaskState::Failed(message) => return Err(RuntimeError::new(message.clone())),
                TaskState::Cancelled => return Err(RuntimeError::new("await: task was cancelled")),
                TaskState::Pending { expr, env } => (expr.clone(), env.clone()),
            }
        };
        let (expr, captured_env) = pending;
        let mut task_env = Env::from_map(captured_env);
        match self.eval(&expr, &mut task_env) {
            Ok(value) => {
                *task.state.borrow_mut() = TaskState::Completed(value.clone());
                Ok(value)
            }
            Err(err) => {
                *task.state.borrow_mut() = TaskState::Failed(err.message.clone());
                Err(err)
            }
        }
    }

    pub(super) fn cancel_task_if_pending(task: &TaskHandle) {
        let mut state = task.state.borrow_mut();
        if matches!(*state, TaskState::Pending { .. }) {
            *state = TaskState::Cancelled;
        }
    }

    pub(super) fn materialize_handle_bindings(
        &mut self,
        handlers: &[HandleBinding],
        env: &mut Env,
    ) -> Result<Vec<RuntimeEffectArm>> {
        let mut frame = Vec::new();
        let mut seen = BTreeSet::new();

        for binding in handlers {
            match binding {
                HandleBinding::On(arm) => {
                    let key = (arm.effect.clone(), arm.operation.clone());
                    if !seen.insert(key.clone()) {
                        return Err(RuntimeError::new(format!(
                            "duplicate handler binding for `{}.{}` in one `with` block",
                            key.0, key.1
                        )));
                    }
                    frame.push(RuntimeEffectArm {
                        effect: arm.effect.clone(),
                        operation: arm.operation.clone(),
                        params: arm.params.clone(),
                        body: (*arm.body).clone(),
                        captured_env: env.snapshot(),
                    });
                }
                HandleBinding::Use(handler_use) => {
                    let handler_def = self
                        .handlers
                        .get(&handler_use.handler)
                        .cloned()
                        .ok_or_else(|| {
                            RuntimeError::new(format!("unknown handler `{}`", handler_use.handler))
                        })?;

                    let mut payload = BTreeMap::new();
                    for (field, value_expr) in &handler_use.payload {
                        let value = self.eval(value_expr, env)?;
                        payload.insert(field.clone(), value);
                    }
                    for field in &handler_def.fields {
                        if !payload.contains_key(&field.name) {
                            return Err(RuntimeError::new(format!(
                                "handler `{}` is missing payload field `{}`",
                                handler_use.handler, field.name
                            )));
                        }
                    }
                    for field_name in payload.keys() {
                        if !handler_def
                            .fields
                            .iter()
                            .any(|field| &field.name == field_name)
                        {
                            return Err(RuntimeError::new(format!(
                                "handler `{}` has no payload field `{field_name}`",
                                handler_use.handler
                            )));
                        }
                    }

                    let self_value = Value::Struct(handler_def.name.clone(), payload);
                    let mut captured_env = env.snapshot();
                    captured_env.insert("self".to_string(), self_value);

                    for method in &handler_def.methods {
                        let key = (handler_def.effect.clone(), method.name.clone());
                        if !seen.insert(key.clone()) {
                            return Err(RuntimeError::new(format!(
                                "duplicate handler binding for `{}.{}` in one `with` block",
                                key.0, key.1
                            )));
                        }
                        let Some(body) = &method.body else {
                            return Err(RuntimeError::new(format!(
                                "handler `{}` method `{}` has no body",
                                handler_def.name, method.name
                            )));
                        };
                        frame.push(RuntimeEffectArm {
                            effect: handler_def.effect.clone(),
                            operation: method.name.clone(),
                            params: method
                                .params
                                .iter()
                                .map(|param| param.name.clone())
                                .collect(),
                            body: body.clone(),
                            captured_env: captured_env.clone(),
                        });
                    }
                }
            }
        }

        Ok(frame)
    }

    /// Try dispatching an operation through registered effect handlers.
    pub(super) fn try_dispatch_effect(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<Value>> {
        for handler in &self.effect_handlers {
            if handler.operations().contains(&name) {
                let result = handler.handle(name, args).map_err(RuntimeError::new)?;
                return match result {
                    EffectOutcome::Value(value) => Ok(Some(value)),
                    EffectOutcome::Signal(signal) => Err(RuntimeError::signal(signal)),
                };
            }
        }
        Ok(None)
    }

    pub(super) fn try_call_builtin(&mut self, name: &str, args: &[Value]) -> Result<Option<Value>> {
        match name {
            "send" => {
                let sender = require_arg(args, 0, "send")?;
                let value = require_arg(args, 1, "send")?;
                match sender {
                    Value::Sender(endpoint) => {
                        let mut state = endpoint.state.borrow_mut();
                        if state.closed {
                            return Err(RuntimeError::new("send: channel is closed"));
                        }
                        if state.buffer > 0 && state.queue.len() >= state.buffer {
                            return Err(RuntimeError::new("send: channel buffer is full"));
                        }
                        state.queue.push_back(value.clone());
                        Ok(Some(Value::Unit))
                    }
                    other => Err(RuntimeError::new(format!(
                        "send: expected Sender, got {}",
                        other.type_name()
                    ))),
                }
            }
            "recv" => {
                let receiver = require_arg(args, 0, "recv")?;
                match receiver {
                    Value::Receiver(endpoint) => {
                        let mut state = endpoint.state.borrow_mut();
                        if let Some(value) = state.queue.pop_front() {
                            Ok(Some(value))
                        } else {
                            Err(RuntimeError::new("recv: channel is empty"))
                        }
                    }
                    other => Err(RuntimeError::new(format!(
                        "recv: expected Receiver, got {}",
                        other.type_name()
                    ))),
                }
            }
            "close" => {
                let endpoint = require_arg(args, 0, "close")?;
                match endpoint {
                    Value::Sender(ch) | Value::Receiver(ch) => {
                        ch.state.borrow_mut().closed = true;
                        Ok(Some(Value::Unit))
                    }
                    other => Err(RuntimeError::new(format!(
                        "close: expected Sender or Receiver, got {}",
                        other.type_name()
                    ))),
                }
            }

            "len" => {
                let val = require_arg(args, 0, "len")?;
                match val {
                    Value::List(v) => Ok(Some(Value::Int(v.len() as i64))),
                    Value::Str(s) => Ok(Some(Value::Int(s.len() as i64))),
                    _ => Err(RuntimeError::new(format!(
                        "len: expected List or String, got {}",
                        val.type_name()
                    ))),
                }
            }
            "map" => {
                let list = require_list(args, 0, "map")?;
                let f = require_arg(args, 1, "map")?;
                let results: Vec<Value> = list
                    .iter()
                    .map(|item| self.call_value(f, vec![item.clone()]))
                    .collect::<Result<_>>()?;
                Ok(Some(Value::List(results)))
            }
            "filter" => {
                let list = require_list(args, 0, "filter")?;
                let pred = require_arg(args, 1, "filter")?;
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
                let list = require_list(args, 0, "fold")?;
                let init = require_arg(args, 1, "fold")?.clone();
                let f = require_arg(args, 2, "fold")?;
                let mut acc = init;
                for item in list {
                    acc = self.call_value(f, vec![acc, item.clone()])?;
                }
                Ok(Some(acc))
            }
            "each" => {
                let list = require_list(args, 0, "each")?;
                let f = require_arg(args, 1, "each")?;
                for item in list {
                    self.call_value(f, vec![item.clone()])?;
                }
                Ok(Some(Value::Unit))
            }
            "append" => {
                let list = require_list(args, 0, "append")?;
                let item = require_arg(args, 1, "append")?;
                let mut new_list = list.clone();
                new_list.push(item.clone());
                Ok(Some(Value::List(new_list)))
            }
            "prepend" => {
                let item = require_arg(args, 0, "prepend")?;
                let list = require_list(args, 1, "prepend")?;
                let mut new_list = vec![item.clone()];
                new_list.extend(list.iter().cloned());
                Ok(Some(Value::List(new_list)))
            }
            "head" => {
                let list = require_list(args, 0, "head")?;
                match list.first().cloned() {
                    Some(val) => Ok(Some(Value::Enum("Some".into(), vec![val]))),
                    None => Ok(Some(Value::Enum("None".into(), vec![]))),
                }
            }
            "tail" => {
                let list = require_list(args, 0, "tail")?;
                if list.is_empty() {
                    Ok(Some(Value::Enum("None".into(), vec![])))
                } else {
                    Ok(Some(Value::Enum(
                        "Some".into(),
                        vec![Value::List(list[1..].to_vec())],
                    )))
                }
            }
            "reverse" => {
                let list = require_list(args, 0, "reverse")?;
                let mut rev = list.clone();
                rev.reverse();
                Ok(Some(Value::List(rev)))
            }
            "range" => {
                let start = require_int(args, 0, "range")?;
                let end = require_int(args, 1, "range")?;
                let size = (end - start).unsigned_abs() as usize;
                if size > 10_000_000 {
                    return Err(RuntimeError::new(format!(
                        "range too large: {size} elements (max 10000000)"
                    )));
                }
                let list: Vec<Value> = (start..end).map(Value::Int).collect();
                Ok(Some(Value::List(list)))
            }
            "contains" => {
                let list = require_list(args, 0, "contains")?;
                let item = require_arg(args, 1, "contains")?;
                let found = list.iter().any(|v| v == item);
                Ok(Some(Value::Bool(found)))
            }

            "string_length" => {
                let s = require_str(args, 0, "string_length")?;
                Ok(Some(Value::Int(s.len() as i64)))
            }
            "split" => {
                let s = require_str(args, 0, "split")?;
                let sep = require_str(args, 1, "split")?;
                let parts: Vec<Value> = s.split(sep).map(|p| Value::Str(p.to_owned())).collect();
                Ok(Some(Value::List(parts)))
            }
            "trim" => {
                let s = require_str(args, 0, "trim")?;
                Ok(Some(Value::Str(s.trim().to_owned())))
            }
            "to_upper" => {
                let s = require_str(args, 0, "to_upper")?;
                Ok(Some(Value::Str(s.to_uppercase())))
            }
            "to_lower" => {
                let s = require_str(args, 0, "to_lower")?;
                Ok(Some(Value::Str(s.to_lowercase())))
            }
            "starts_with" => {
                let s = require_str(args, 0, "starts_with")?;
                let prefix = require_str(args, 1, "starts_with")?;
                Ok(Some(Value::Bool(s.starts_with(prefix))))
            }
            "ends_with" => {
                let s = require_str(args, 0, "ends_with")?;
                let suffix = require_str(args, 1, "ends_with")?;
                Ok(Some(Value::Bool(s.ends_with(suffix))))
            }
            "char_at" => {
                let s = require_str(args, 0, "char_at")?;
                let idx_i64 = require_int(args, 1, "char_at")?;
                if idx_i64 < 0 {
                    return Ok(Some(Value::Enum("None".into(), vec![])));
                }
                let idx = idx_i64 as usize;
                match s.chars().nth(idx) {
                    Some(ch) => Ok(Some(Value::Enum(
                        "Some".into(),
                        vec![Value::Str(ch.to_string())],
                    ))),
                    None => Ok(Some(Value::Enum("None".into(), vec![]))),
                }
            }
            "substring" => {
                let s = require_str(args, 0, "substring")?;
                let start_i64 = require_int(args, 1, "substring")?;
                if start_i64 < 0 {
                    return Err(RuntimeError::new(format!(
                        "substring: start cannot be negative, got {start_i64}"
                    )));
                }
                let start = start_i64 as usize;
                let end_i64 = require_int(args, 2, "substring")?;
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
                let s = require_str(args, 0, "replace")?;
                let from = require_str(args, 1, "replace")?;
                let to = require_str(args, 2, "replace")?;
                Ok(Some(Value::Str(s.replace(from, to))))
            }
            "to_string" => {
                let val = require_arg(args, 0, "to_string")?;
                Ok(Some(Value::Str(val.to_string())))
            }

            "abs" => {
                let n = require_int(args, 0, "abs")?;
                Ok(Some(Value::Int(n.saturating_abs())))
            }
            "min" => {
                let a = require_int(args, 0, "min")?;
                let b = require_int(args, 1, "min")?;
                Ok(Some(Value::Int(a.min(b))))
            }
            "max" => {
                let a = require_int(args, 0, "max")?;
                let b = require_int(args, 1, "max")?;
                Ok(Some(Value::Int(a.max(b))))
            }

            "concat" => {
                let a = require_list(args, 0, "concat")?.clone();
                let b = require_list(args, 1, "concat")?;
                let mut result = a;
                result.extend(b.iter().cloned());
                Ok(Some(Value::List(result)))
            }

            "string_index_of" => {
                let haystack = require_str(args, 0, "string_index_of")?;
                let needle = require_str(args, 1, "string_index_of")?;
                match haystack.find(needle) {
                    Some(pos) => Ok(Some(Value::Int(pos as i64))),
                    None => Ok(Some(Value::Int(-1))),
                }
            }

            "char_to_int" => {
                let s = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("char_to_int: missing arg"))?
                    .as_str()
                    .ok_or_else(|| RuntimeError::new("char_to_int: expected String"))?;
                let ch = s
                    .chars()
                    .next()
                    .ok_or_else(|| RuntimeError::new("char_to_int: empty string"))?;
                Ok(Some(Value::Int(ch as i64)))
            }
            "int_to_char" => {
                let n = args
                    .first()
                    .ok_or_else(|| RuntimeError::new("int_to_char: missing arg"))?
                    .as_int()
                    .ok_or_else(|| RuntimeError::new("int_to_char: expected Int"))?;
                let ch = char::from_u32(n as u32)
                    .ok_or_else(|| RuntimeError::new("int_to_char: invalid code point"))?;
                Ok(Some(Value::Str(ch.to_string())))
            }

            _ => Ok(None),
        }
    }
}
