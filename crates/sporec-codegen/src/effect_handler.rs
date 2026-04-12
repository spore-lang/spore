//! Runtime effect handlers for capability-gated operations.
//!
//! Effect handlers bridge the gap between the type-level capability system
//! (checked at compile time) and actual runtime I/O operations.

use crate::value::Value;
use std::io::Write;

/// Runtime host profile used to select effect-handler coverage for project mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePlatform {
    Cli,
    BasicCli,
}

/// A runtime effect handler that provides implementations for capability-gated operations.
pub trait EffectHandler: std::fmt::Debug {
    /// Handle an effect invocation. Returns the result value.
    fn handle(&self, operation: &str, args: &[Value]) -> Result<Value, String>;

    /// List the operations this handler provides.
    fn operations(&self) -> &[&str];
}

const CLI_OPS: &[&str] = &["print", "println", "read_line"];

const BASIC_CLI_OPS: &[&str] = &[
    "print",
    "println",
    "eprint",
    "eprintln",
    "read_line",
    "file_read",
    "file_write",
    "file_exists",
    "file_stat",
    "dir_list",
    "dir_mkdir",
    "env_get",
    "env_set",
    "process_run",
    "process_run_status",
    "basic_cli.stdout.print",
    "basic_cli.stdout.println",
    "basic_cli.stdout.eprint",
    "basic_cli.stdout.eprintln",
    "basic_cli.stdin.read_line",
    "basic_cli.file.file_read",
    "basic_cli.file.file_write",
    "basic_cli.file.file_exists",
    "basic_cli.file.file_stat",
    "basic_cli.dir.dir_list",
    "basic_cli.dir.dir_mkdir",
    "basic_cli.env.env_get",
    "basic_cli.env.env_set",
    "basic_cli.cmd.process_run",
    "basic_cli.cmd.process_run_status",
];

fn require_arg<'a>(args: &'a [Value], idx: usize, operation: &str) -> Result<&'a Value, String> {
    args.get(idx)
        .ok_or_else(|| format!("{operation}: missing argument {}", idx + 1))
}

fn require_str_arg<'a>(args: &'a [Value], idx: usize, operation: &str) -> Result<&'a str, String> {
    match require_arg(args, idx, operation)? {
        Value::Str(value) => Ok(value),
        other => Err(format!(
            "{operation}: argument {} should be Str, got {}",
            idx + 1,
            other.type_name()
        )),
    }
}

fn require_str_list_arg(
    args: &[Value],
    idx: usize,
    operation: &str,
) -> Result<Vec<String>, String> {
    match require_arg(args, idx, operation)? {
        Value::List(values) => values
            .iter()
            .enumerate()
            .map(|(item_idx, value)| match value {
                Value::Str(text) => Ok(text.clone()),
                other => Err(format!(
                    "{operation}: list argument {} item {} should be Str, got {}",
                    idx + 1,
                    item_idx + 1,
                    other.type_name()
                )),
            })
            .collect(),
        other => Err(format!(
            "{operation}: argument {} should be List[Str], got {}",
            idx + 1,
            other.type_name()
        )),
    }
}

fn io_error(operation: &str, error: impl std::fmt::Display) -> String {
    format!("{operation}: {error}")
}

fn exec_error(operation: &str, command: &str, error: impl std::fmt::Display) -> String {
    format!("{operation}: {command}: {error}")
}

fn normalize_basic_cli_operation(operation: &str) -> &str {
    operation.rsplit('.').next().unwrap_or(operation)
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
                let val = require_arg(args, 0, operation)?;
                print!("{val}");
                Ok(Value::Unit)
            }
            "println" => {
                let val = require_arg(args, 0, operation)?;
                println!("{val}");
                Ok(Value::Unit)
            }
            "read_line" => {
                let mut buf = String::new();
                std::io::stdin()
                    .read_line(&mut buf)
                    .map_err(|error| io_error(operation, error))?;
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
        CLI_OPS
    }
}

/// The package-backed basic-cli platform handler.
#[derive(Debug)]
pub struct BasicCliPlatformHandler;

impl EffectHandler for BasicCliPlatformHandler {
    fn handle(&self, operation: &str, args: &[Value]) -> Result<Value, String> {
        match normalize_basic_cli_operation(operation) {
            "print" => {
                let text = require_str_arg(args, 0, operation)?;
                print!("{text}");
                std::io::stdout()
                    .flush()
                    .map_err(|error| io_error(operation, error))?;
                Ok(Value::Unit)
            }
            "println" => {
                let text = require_str_arg(args, 0, operation)?;
                println!("{text}");
                Ok(Value::Unit)
            }
            "eprint" => {
                let text = require_str_arg(args, 0, operation)?;
                eprint!("{text}");
                std::io::stderr()
                    .flush()
                    .map_err(|error| io_error(operation, error))?;
                Ok(Value::Unit)
            }
            "eprintln" => {
                let text = require_str_arg(args, 0, operation)?;
                eprintln!("{text}");
                Ok(Value::Unit)
            }
            "read_line" => {
                let mut buf = String::new();
                std::io::stdin()
                    .read_line(&mut buf)
                    .map_err(|error| io_error(operation, error))?;
                if buf.ends_with('\n') {
                    buf.pop();
                    if buf.ends_with('\r') {
                        buf.pop();
                    }
                }
                Ok(Value::Str(buf))
            }
            "file_read" => {
                let path = require_str_arg(args, 0, operation)?;
                let content =
                    std::fs::read_to_string(path).map_err(|error| io_error(operation, error))?;
                Ok(Value::Str(content))
            }
            "file_write" => {
                let path = require_str_arg(args, 0, operation)?;
                let content = require_str_arg(args, 1, operation)?;
                std::fs::write(path, content).map_err(|error| io_error(operation, error))?;
                Ok(Value::Unit)
            }
            "file_exists" => {
                let path = require_str_arg(args, 0, operation)?;
                Ok(Value::Bool(std::path::Path::new(path).exists()))
            }
            "file_stat" => {
                let path = require_str_arg(args, 0, operation)?;
                let meta = std::fs::metadata(path).map_err(|error| io_error(operation, error))?;
                Ok(Value::Str(format!(
                    "size={} is_dir={} is_file={}",
                    meta.len(),
                    meta.is_dir(),
                    meta.is_file()
                )))
            }
            "dir_list" => {
                let path = require_str_arg(args, 0, operation)?;
                let entries = std::fs::read_dir(path)
                    .map_err(|error| io_error(operation, error))?
                    .map(|entry| {
                        entry
                            .map(|item| Value::Str(item.file_name().to_string_lossy().into_owned()))
                            .map_err(|error| io_error(operation, error))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::List(entries))
            }
            "dir_mkdir" => {
                let path = require_str_arg(args, 0, operation)?;
                std::fs::create_dir_all(path).map_err(|error| io_error(operation, error))?;
                Ok(Value::Unit)
            }
            "env_get" => {
                let key = require_str_arg(args, 0, operation)?;
                match std::env::var(key) {
                    Ok(value) => Ok(Value::Enum("Some".into(), vec![Value::Str(value)])),
                    Err(_) => Ok(Value::Enum("None".into(), vec![])),
                }
            }
            "env_set" => {
                let key = require_str_arg(args, 0, operation)?;
                let value = require_str_arg(args, 1, operation)?;
                // SAFETY: project-mode interpreter execution is single-threaded.
                unsafe { std::env::set_var(key, value) };
                Ok(Value::Unit)
            }
            "process_run" => {
                let command = require_str_arg(args, 0, operation)?;
                let process_args = require_str_list_arg(args, 1, operation)?;
                let output = std::process::Command::new(command)
                    .args(&process_args)
                    .output()
                    .map_err(|error| exec_error(operation, command, error))?;
                if output.status.success() {
                    Ok(Value::Str(
                        String::from_utf8_lossy(&output.stdout).into_owned(),
                    ))
                } else {
                    Err(exec_error(
                        operation,
                        command,
                        format!(
                            "exited with {}: {}",
                            output.status,
                            String::from_utf8_lossy(&output.stderr)
                        ),
                    ))
                }
            }
            "process_run_status" => {
                let command = require_str_arg(args, 0, operation)?;
                let process_args = require_str_list_arg(args, 1, operation)?;
                let status = std::process::Command::new(command)
                    .args(&process_args)
                    .status()
                    .map_err(|error| exec_error(operation, command, error))?;
                Ok(Value::Int(status.code().unwrap_or(-1) as i64))
            }
            _ => Err(format!(
                "BasicCliPlatformHandler: unknown operation `{operation}`"
            )),
        }
    }

    fn operations(&self) -> &[&str] {
        BASIC_CLI_OPS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "sporec-codegen-{name}-{unique}-{}",
            std::process::id()
        ))
    }

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

    #[test]
    fn basic_cli_handler_operations_include_package_qualified_names() {
        let h = BasicCliPlatformHandler;
        let ops = h.operations();
        assert!(ops.contains(&"file_exists"));
        assert!(ops.contains(&"basic_cli.file.file_exists"));
    }

    #[test]
    fn basic_cli_handler_file_exists_returns_bool() {
        let h = BasicCliPlatformHandler;
        let path = temp_path("file-exists");
        std::fs::write(&path, "hello").expect("write temp file");
        let result = h
            .handle(
                "basic_cli.file.file_exists",
                &[Value::Str(path.display().to_string())],
            )
            .expect("file_exists should succeed");
        assert_eq!(result, Value::Bool(true));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn basic_cli_handler_env_get_returns_option_enum() {
        let h = BasicCliPlatformHandler;
        let key = format!("SPORE_CODEGEN_TEST_{}", std::process::id());
        // SAFETY: test process is single-threaded at this point for this variable.
        unsafe { std::env::set_var(&key, "hello") };
        let result = h
            .handle("env_get", &[Value::Str(key.clone())])
            .expect("env_get should succeed");
        assert_eq!(
            result,
            Value::Enum("Some".into(), vec![Value::Str("hello".into())])
        );
        // SAFETY: paired cleanup for the test-only variable above.
        unsafe { std::env::remove_var(&key) };
    }
}
