//! Runtime effect handlers for capability-gated operations.
//!
//! Effect handlers bridge the gap between the type-level capability system
//! (checked at compile time) and actual runtime I/O operations.

use crate::value::Value;

/// A runtime effect handler that provides implementations for capability-gated operations.
pub trait EffectHandler: std::fmt::Debug {
    /// Handle an effect invocation. Returns the result value.
    fn handle(&self, operation: &str, args: &[Value]) -> Result<Value, String>;

    /// List the operations this handler provides.
    fn operations(&self) -> &[&str];
}

// ── CliPlatformHandler ──────────────────────────────────────────────────

/// The CLI platform handler — provides standard I/O operations
/// (`print`, `println`, `read_line`).
#[derive(Debug)]
pub struct CliPlatformHandler;

impl EffectHandler for CliPlatformHandler {
    fn handle(&self, operation: &str, args: &[Value]) -> Result<Value, String> {
        match operation {
            "print" => {
                let val = args.first().ok_or("print: missing argument")?;
                print!("{val}");
                Ok(Value::Unit)
            }
            "println" => {
                let val = args.first().ok_or("println: missing argument")?;
                println!("{val}");
                Ok(Value::Unit)
            }
            "read_line" => {
                let mut buf = String::new();
                std::io::stdin()
                    .read_line(&mut buf)
                    .map_err(|e| format!("read_line: {e}"))?;
                if buf.ends_with('\n') {
                    buf.pop();
                    if buf.ends_with('\r') {
                        buf.pop();
                    }
                }
                Ok(Value::Str(buf))
            }
            _ => Err(format!(
                "CliPlatformHandler: unknown operation `{operation}`"
            )),
        }
    }

    fn operations(&self) -> &[&str] {
        &["print", "println", "read_line"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_handler_operations_list() {
        let h = CliPlatformHandler;
        let ops = h.operations();
        assert!(ops.contains(&"print"));
        assert!(ops.contains(&"println"));
        assert!(ops.contains(&"read_line"));
    }

    #[test]
    fn cli_handler_print_returns_unit() {
        let h = CliPlatformHandler;
        let result = h.handle("print", &[Value::Str("hello".into())]);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Value::Unit));
    }

    #[test]
    fn cli_handler_println_returns_unit() {
        let h = CliPlatformHandler;
        let result = h.handle("println", &[Value::Str("hello".into())]);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Value::Unit));
    }

    #[test]
    fn cli_handler_unknown_operation() {
        let h = CliPlatformHandler;
        let result = h.handle("nonexistent", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown operation"));
    }

    #[test]
    fn cli_handler_print_missing_arg() {
        let h = CliPlatformHandler;
        let result = h.handle("print", &[]);
        assert!(result.is_err());
    }
}
