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
    Char(char),
    Unit,
    /// Struct instance: (type name, fields)
    Struct(String, BTreeMap<String, Value>),
    /// Closure: (param names, body AST index, captured env)
    Closure(Closure),
    /// Built-in function
    Builtin(String),
    /// List of values
    List(Vec<Value>),
    /// Enum variant instance: (variant name, fields)
    Enum(String, Vec<Value>),
    /// Map (for future use)
    Map(BTreeMap<String, Value>),
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
            Value::Char(c) => write!(f, "'{c}'"),
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
            Value::List(elems) => {
                write!(f, "[")?;
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, "]")
            }
            Value::Enum(name, fields) => {
                if fields.is_empty() {
                    write!(f, "{name}")
                } else {
                    write!(f, "{name}(")?;
                    for (i, v) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{v}")?;
                    }
                    write!(f, ")")
                }
            }
            Value::Map(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
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

    pub fn as_list(&self) -> std::result::Result<&Vec<Value>, String> {
        match self {
            Value::List(v) => Ok(v),
            _ => Err(format!("expected List, got {}", self.type_name())),
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Bool",
            Value::Str(_) => "String",
            Value::Char(_) => "Char",
            Value::Unit => "Unit",
            Value::List(_) => "List",
            Value::Struct(name, _) => name,
            Value::Enum(name, _) => name,
            Value::Closure(_) => "Closure",
            Value::Builtin(_) => "Builtin",
            Value::Map(_) => "Map",
        }
    }
}
