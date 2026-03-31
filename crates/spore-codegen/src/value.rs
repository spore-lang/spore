//! Runtime values for the Spore interpreter.

use std::collections::BTreeMap;
use std::fmt;

/// A runtime value in the Spore interpreter.
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Unit,
    /// Struct instance: (type name, fields)
    Struct(String, BTreeMap<String, Value>),
    /// Closure: (param names, body AST index, captured env)
    Closure(Closure),
    /// Built-in function
    Builtin(String),
}

/// A captured closure.
#[derive(Debug, Clone)]
pub struct Closure {
    pub params: Vec<String>,
    pub body: spore_parser::ast::Expr,
    pub env: BTreeMap<String, Value>,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Unit => write!(f, "()"),
            Value::Struct(name, fields) => {
                write!(f, "{name} {{ ")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, " }}")
            }
            Value::Closure(c) => write!(f, "<closure({})>", c.params.join(", ")),
            Value::Builtin(name) => write!(f, "<builtin:{name}>"),
        }
    }
}

impl Value {
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
}
