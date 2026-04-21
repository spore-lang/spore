use sporec_parser::ast::{Expr, FStringPart, TStringPart};

use crate::effect_handler::RuntimeSignal;
use crate::value::Value;

/// Runtime error during evaluation.
#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub message: String,
    signal: Option<RuntimeSignal>,
}

impl RuntimeError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            signal: None,
        }
    }

    pub fn signal(signal: RuntimeSignal) -> Self {
        Self {
            message: format!("runtime signal: {signal:?}"),
            signal: Some(signal),
        }
    }

    pub fn runtime_signal(&self) -> Option<RuntimeSignal> {
        self.signal
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "runtime error: {}", self.message)
    }
}

impl std::error::Error for RuntimeError {}

pub(super) type Result<T> = std::result::Result<T, RuntimeError>;

pub(super) fn require_arg<'a>(args: &'a [Value], idx: usize, name: &str) -> Result<&'a Value> {
    args.get(idx)
        .ok_or_else(|| RuntimeError::new(format!("{name}: missing argument {idx}")))
}

pub(super) fn require_int(args: &[Value], idx: usize, name: &str) -> Result<i64> {
    require_arg(args, idx, name)?
        .as_int()
        .ok_or_else(|| RuntimeError::new(format!("{name}: argument {idx} must be I32")))
}

pub(super) fn require_str<'a>(args: &'a [Value], idx: usize, name: &str) -> Result<&'a str> {
    require_arg(args, idx, name)?
        .as_str()
        .ok_or_else(|| RuntimeError::new(format!("{name}: argument {idx} must be Str")))
}

pub(super) fn require_list<'a>(
    args: &'a [Value],
    idx: usize,
    name: &str,
) -> Result<&'a Vec<Value>> {
    require_arg(args, idx, name)?
        .as_list()
        .map_err(|e| RuntimeError::new(format!("{name}: argument {idx}: {e}")))
}

/// A borrowed view of a single chunk in an interpolated string (f-string or t-string).
pub(super) enum StringChunk<'a> {
    Literal(&'a str),
    Expr(&'a Expr),
}

impl<'a> From<&'a FStringPart> for StringChunk<'a> {
    fn from(p: &'a FStringPart) -> Self {
        match p {
            FStringPart::Literal(s) => StringChunk::Literal(s),
            FStringPart::Expr(e) => StringChunk::Expr(e),
        }
    }
}

impl<'a> From<&'a TStringPart> for StringChunk<'a> {
    fn from(p: &'a TStringPart) -> Self {
        match p {
            TStringPart::Literal(s) => StringChunk::Literal(s),
            TStringPart::Expr(e) => StringChunk::Expr(e),
        }
    }
}
