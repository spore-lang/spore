use std::collections::BTreeMap;

use sporec_parser::ast::{Expr, FnDef};

use crate::value::{Closure, Value};

pub(super) struct Env {
    scopes: Vec<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeEffectArm {
    pub(super) effect: String,
    pub(super) operation: String,
    pub(super) params: Vec<String>,
    pub(super) body: Expr,
    pub(super) captured_env: BTreeMap<String, Value>,
}

impl Env {
    pub(super) fn new() -> Self {
        Self {
            scopes: vec![BTreeMap::new()],
        }
    }

    pub(super) fn from_map(map: BTreeMap<String, Value>) -> Self {
        Self { scopes: vec![map] }
    }

    pub(super) fn push(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    pub(super) fn pop(&mut self) {
        self.scopes.pop();
    }

    pub(super) fn define(&mut self, name: String, val: Value) {
        self.scopes.last_mut().unwrap().insert(name, val);
    }

    pub(super) fn lookup(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    /// Snapshot all visible bindings (for closure capture).
    pub(super) fn snapshot(&self) -> BTreeMap<String, Value> {
        let mut map = BTreeMap::new();
        for scope in &self.scopes {
            for (k, v) in scope {
                map.insert(k.clone(), v.clone());
            }
        }
        map
    }
}

pub(super) fn named_function_closure(name: &str, func: &FnDef) -> Value {
    Value::Closure(Closure {
        params: func.params.iter().map(|p| p.name.clone()).collect(),
        body: func
            .body
            .clone()
            .unwrap_or(Expr::Hole(Some(name.to_string()), None, None, None)),
        env: BTreeMap::new(),
    })
}
