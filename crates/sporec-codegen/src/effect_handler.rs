//! Runtime effect handlers for effect-gated operations.
//!
//! Effect handlers bridge the gap between the type-level effect system
//! (checked at compile time) and actual runtime I/O operations.

use std::collections::BTreeMap;
use std::io::Write;

use sporec_parser::ast::{Item, Module, Visibility};

use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSignal {
    Exit(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EffectOutcome {
    Value(Value),
    Signal(RuntimeSignal),
}

/// Runtime host profile used to select effect-handler coverage for project mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePlatform {
    Cli,
    PackageHost,
}

/// A runtime effect handler that provides implementations for effect-gated operations.
pub trait EffectHandler: std::fmt::Debug {
    /// Handle an effect invocation. Returns the result value.
    fn handle(&self, operation: &str, args: &[Value]) -> Result<EffectOutcome, String>;

    /// Whether this handler can service the requested operation.
    fn supports(&self, operation: &str) -> bool;
}

const CLI_OPS: &[&str] = &["print", "println", "read_line"];

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

fn require_int_arg(args: &[Value], idx: usize, operation: &str) -> Result<i64, String> {
    match require_arg(args, idx, operation)? {
        Value::Int(value) => Ok(*value),
        other => Err(format!(
            "{operation}: argument {} should be integer-valued, got {}",
            idx + 1,
            other.type_name()
        )),
    }
}

fn require_exit_code(code: i64, operation: &str) -> Result<u8, String> {
    u8::try_from(code).map_err(|_| format!("{operation}: exit code {code} is out of range for U8"))
}

fn io_error(operation: &str, error: impl std::fmt::Display) -> String {
    format!("{operation}: {error}")
}

fn exec_error(operation: &str, command: &str, error: impl std::fmt::Display) -> String {
    format!("{operation}: {command}: {error}")
}

fn canonical_package_host_operation(operation: &str) -> Option<&'static str> {
    match operation.rsplit('.').next().unwrap_or(operation) {
        "print" => Some("print"),
        "println" => Some("println"),
        "eprint" => Some("eprint"),
        "eprintln" => Some("eprintln"),
        "read_line" => Some("read_line"),
        "file_read" => Some("file_read"),
        "file_write" => Some("file_write"),
        "file_exists" => Some("file_exists"),
        "file_stat" => Some("file_stat"),
        "dir_list" => Some("dir_list"),
        "dir_mkdir" => Some("dir_mkdir"),
        "env_get" => Some("env_get"),
        "env_set" => Some("env_set"),
        "process_run" => Some("process_run"),
        "process_run_status" => Some("process_run_status"),
        "exit" => Some("exit"),
        _ => None,
    }
}

fn register_package_host_aliases(
    aliases: &mut BTreeMap<String, &'static str>,
    module_path: Option<&str>,
    module: &Module,
    require_public: bool,
) {
    for item in &module.items {
        let Item::Function(function) = item else {
            continue;
        };
        if !function.is_foreign {
            continue;
        }
        if require_public && !matches!(function.visibility, Visibility::Pub | Visibility::PubPkg) {
            continue;
        }
        let Some(canonical) = canonical_package_host_operation(&function.name) else {
            continue;
        };
        aliases.entry(function.name.clone()).or_insert(canonical);
        if let Some(module_path) = module_path {
            aliases.insert(format!("{module_path}.{}", function.name), canonical);
        }
    }
}

// ── CliPlatformHandler ──────────────────────────────────────────────────

/// The CLI platform handler — provides standard I/O operations
/// (`print`, `println`, `read_line`).
#[derive(Debug)]
pub struct CliPlatformHandler;

impl EffectHandler for CliPlatformHandler {
    fn handle(&self, operation: &str, args: &[Value]) -> Result<EffectOutcome, String> {
        match operation {
            "print" => {
                let val = require_arg(args, 0, operation)?;
                print!("{val}");
                Ok(EffectOutcome::Value(Value::Unit))
            }
            "println" => {
                let val = require_arg(args, 0, operation)?;
                println!("{val}");
                Ok(EffectOutcome::Value(Value::Unit))
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
                Ok(EffectOutcome::Value(Value::Str(buf)))
            }
            _ => Err(format!(
                "CliPlatformHandler: unknown operation `{operation}`"
            )),
        }
    }

    fn supports(&self, operation: &str) -> bool {
        CLI_OPS.contains(&operation)
    }
}

/// Generic package-backed host handler keyed by imported foreign-function names.
#[derive(Debug, Default)]
pub struct PackageHostHandler {
    aliases: BTreeMap<String, &'static str>,
}

impl PackageHostHandler {
    pub fn from_modules(entry: &Module, imports: &[(String, Module)]) -> Self {
        let mut aliases = BTreeMap::new();
        register_package_host_aliases(&mut aliases, None, entry, false);
        for (module_path, module) in imports {
            register_package_host_aliases(&mut aliases, Some(module_path), module, true);
        }
        Self { aliases }
    }
}

impl EffectHandler for PackageHostHandler {
    fn handle(&self, operation: &str, args: &[Value]) -> Result<EffectOutcome, String> {
        let Some(canonical) = self.aliases.get(operation).copied() else {
            return Err(format!(
                "PackageHostHandler: unknown operation `{operation}`"
            ));
        };
        match canonical {
            "print" => {
                let text = require_str_arg(args, 0, operation)?;
                print!("{text}");
                std::io::stdout()
                    .flush()
                    .map_err(|error| io_error(operation, error))?;
                Ok(EffectOutcome::Value(Value::Unit))
            }
            "println" => {
                let text = require_str_arg(args, 0, operation)?;
                println!("{text}");
                Ok(EffectOutcome::Value(Value::Unit))
            }
            "eprint" => {
                let text = require_str_arg(args, 0, operation)?;
                eprint!("{text}");
                std::io::stderr()
                    .flush()
                    .map_err(|error| io_error(operation, error))?;
                Ok(EffectOutcome::Value(Value::Unit))
            }
            "eprintln" => {
                let text = require_str_arg(args, 0, operation)?;
                eprintln!("{text}");
                Ok(EffectOutcome::Value(Value::Unit))
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
                Ok(EffectOutcome::Value(Value::Str(buf)))
            }
            "file_read" => {
                let path = require_str_arg(args, 0, operation)?;
                let content =
                    std::fs::read_to_string(path).map_err(|error| io_error(operation, error))?;
                Ok(EffectOutcome::Value(Value::Str(content)))
            }
            "file_write" => {
                let path = require_str_arg(args, 0, operation)?;
                let content = require_str_arg(args, 1, operation)?;
                std::fs::write(path, content).map_err(|error| io_error(operation, error))?;
                Ok(EffectOutcome::Value(Value::Unit))
            }
            "file_exists" => {
                let path = require_str_arg(args, 0, operation)?;
                Ok(EffectOutcome::Value(Value::Bool(
                    std::path::Path::new(path).exists(),
                )))
            }
            "file_stat" => {
                let path = require_str_arg(args, 0, operation)?;
                let meta = std::fs::metadata(path).map_err(|error| io_error(operation, error))?;
                Ok(EffectOutcome::Value(Value::Str(format!(
                    "size={} is_dir={} is_file={}",
                    meta.len(),
                    meta.is_dir(),
                    meta.is_file()
                ))))
            }
            "dir_list" => {
                let path = require_str_arg(args, 0, operation)?;
                let mut entries = std::fs::read_dir(path)
                    .map_err(|error| io_error(operation, error))?
                    .map(|entry| {
                        entry
                            .map(|item| Value::Str(item.file_name().to_string_lossy().into_owned()))
                            .map_err(|error| io_error(operation, error))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                entries.sort_by(|a, b| match (a, b) {
                    (Value::Str(a), Value::Str(b)) => a.cmp(b),
                    _ => std::cmp::Ordering::Equal,
                });
                Ok(EffectOutcome::Value(Value::List(entries)))
            }
            "dir_mkdir" => {
                let path = require_str_arg(args, 0, operation)?;
                std::fs::create_dir_all(path).map_err(|error| io_error(operation, error))?;
                Ok(EffectOutcome::Value(Value::Unit))
            }
            "env_get" => {
                let key = require_str_arg(args, 0, operation)?;
                match std::env::var(key) {
                    Ok(value) => Ok(EffectOutcome::Value(Value::Enum(
                        "Some".into(),
                        vec![Value::Str(value)],
                    ))),
                    Err(_) => Ok(EffectOutcome::Value(Value::Enum("None".into(), vec![]))),
                }
            }
            "env_set" => {
                let key = require_str_arg(args, 0, operation)?;
                let value = require_str_arg(args, 1, operation)?;
                // SAFETY: project-mode interpreter execution is single-threaded.
                unsafe { std::env::set_var(key, value) };
                Ok(EffectOutcome::Value(Value::Unit))
            }
            "process_run" => {
                let command = require_str_arg(args, 0, operation)?;
                let process_args = require_str_list_arg(args, 1, operation)?;
                let output = std::process::Command::new(command)
                    .args(&process_args)
                    .output()
                    .map_err(|error| exec_error(operation, command, error))?;
                if output.status.success() {
                    Ok(EffectOutcome::Value(Value::Str(
                        String::from_utf8_lossy(&output.stdout).into_owned(),
                    )))
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
                let value = match status.code() {
                    Some(code) => {
                        let code = u8::try_from(code).map_err(|_| {
                            exec_error(
                                operation,
                                command,
                                format!("exit code {code} is out of range for U8"),
                            )
                        })?;
                        Value::Enum("Some".into(), vec![Value::Int(i64::from(code))])
                    }
                    None => Value::Enum("None".into(), vec![]),
                };
                Ok(EffectOutcome::Value(value))
            }
            "exit" => {
                let code = require_int_arg(args, 0, operation)?;
                let code = require_exit_code(code, operation)?;
                Ok(EffectOutcome::Signal(RuntimeSignal::Exit(code)))
            }
            _ => Err(format!(
                "PackageHostHandler: unknown operation `{operation}`"
            )),
        }
    }

    fn supports(&self, operation: &str) -> bool {
        self.aliases.contains_key(operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sporec_parser::parse;
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

    fn package_host_handler(imports: &[(&str, &str)]) -> PackageHostHandler {
        let entry = parse("fn main() -> () { return }").expect("parse entry module");
        let imports = imports
            .iter()
            .map(|(path, src)| {
                (
                    (*path).to_string(),
                    parse(src).unwrap_or_else(|error| panic!("parse error: {error:?}")),
                )
            })
            .collect::<Vec<_>>();
        PackageHostHandler::from_modules(&entry, &imports)
    }

    #[test]
    fn cli_handler_supports_builtin_operations() {
        let h = CliPlatformHandler;
        assert!(h.supports("print"));
        assert!(h.supports("println"));
        assert!(h.supports("read_line"));
    }

    #[test]
    fn cli_handler_print_returns_unit() {
        let h = CliPlatformHandler;
        let result = h.handle("print", &[Value::Str("hello".into())]);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), EffectOutcome::Value(Value::Unit)));
    }

    #[test]
    fn cli_handler_println_returns_unit() {
        let h = CliPlatformHandler;
        let result = h.handle("println", &[Value::Str("hello".into())]);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), EffectOutcome::Value(Value::Unit)));
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
    fn package_host_handler_supports_custom_package_qualified_names() {
        let h = package_host_handler(&[(
            "custom_platform.file",
            "pub foreign fn file_exists(path: Str) -> Bool uses [FileRead]",
        )]);
        assert!(h.supports("file_exists"));
        assert!(h.supports("custom_platform.file.file_exists"));
        assert!(!h.supports("custom_platform.file.unsupported"));
    }

    #[test]
    fn package_host_handler_file_exists_returns_bool() {
        let h = package_host_handler(&[(
            "custom_platform.file",
            "pub foreign fn file_exists(path: Str) -> Bool uses [FileRead]",
        )]);
        let path = temp_path("file-exists");
        std::fs::write(&path, "hello").expect("write temp file");
        let result = h
            .handle(
                "custom_platform.file.file_exists",
                &[Value::Str(path.display().to_string())],
            )
            .expect("file_exists should succeed");
        assert_eq!(result, EffectOutcome::Value(Value::Bool(true)));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn package_host_handler_env_get_returns_option_enum() {
        let h = package_host_handler(&[(
            "custom_platform.env",
            "pub foreign fn env_get(key: Str) -> Option[Str] uses [Env]",
        )]);
        let key = format!("SPORE_CODEGEN_TEST_{}", std::process::id());
        // SAFETY: test process is single-threaded at this point for this variable.
        unsafe { std::env::set_var(&key, "hello") };
        let result = h
            .handle("env_get", &[Value::Str(key.clone())])
            .expect("env_get should succeed");
        assert_eq!(
            result,
            EffectOutcome::Value(Value::Enum("Some".into(), vec![Value::Str("hello".into())],))
        );
        // SAFETY: paired cleanup for the test-only variable above.
        unsafe { std::env::remove_var(&key) };
    }

    #[test]
    fn package_host_handler_dir_list_returns_stable_sorted_names() {
        let h = package_host_handler(&[(
            "custom_platform.dir",
            "pub foreign fn dir_list(path: Str) -> List[Str] uses [FileRead]",
        )]);
        let dir = temp_path("dir-list");
        std::fs::create_dir_all(&dir).expect("create temporary directory");
        std::fs::write(dir.join("zeta.txt"), "z").expect("write zeta");
        std::fs::write(dir.join("alpha.txt"), "a").expect("write alpha");

        let result = h
            .handle(
                "dir_list",
                &[Value::Str(dir.to_string_lossy().into_owned())],
            )
            .expect("dir_list should succeed");

        let EffectOutcome::Value(Value::List(entries)) = result else {
            panic!("expected list result");
        };

        assert_eq!(
            entries,
            vec![
                Value::Str("alpha.txt".into()),
                Value::Str("zeta.txt".into())
            ]
        );

        std::fs::remove_dir_all(&dir).expect("cleanup temporary directory");
    }

    #[test]
    fn package_host_handler_process_run_status_returns_some_code() {
        let h = package_host_handler(&[(
            "custom_platform.cmd",
            "pub foreign fn process_run_status(cmd: Str, args: List[Str]) -> Option[U8] uses [Spawn]",
        )]);
        let result = h
            .handle(
                "process_run_status",
                &[Value::Str("true".into()), Value::List(vec![])],
            )
            .expect("process_run_status should succeed");

        assert_eq!(
            result,
            EffectOutcome::Value(Value::Enum("Some".into(), vec![Value::Int(0)]))
        );
    }

    #[test]
    #[cfg(unix)]
    fn package_host_handler_process_run_status_signal_returns_none() {
        let h = package_host_handler(&[(
            "custom_platform.cmd",
            "pub foreign fn process_run_status(cmd: Str, args: List[Str]) -> Option[U8] uses [Spawn]",
        )]);
        let result = h
            .handle(
                "process_run_status",
                &[
                    Value::Str("sh".into()),
                    Value::List(vec![
                        Value::Str("-c".into()),
                        Value::Str("kill -TERM $$".into()),
                    ]),
                ],
            )
            .expect("process_run_status should report signal termination");

        assert_eq!(
            result,
            EffectOutcome::Value(Value::Enum("None".into(), vec![]))
        );
    }

    #[test]
    fn package_host_handler_exit_returns_structured_signal() {
        let h = package_host_handler(&[(
            "custom_platform.cmd",
            "pub foreign fn exit(code: U8) -> Never uses [Exit]",
        )]);
        let result = h
            .handle("custom_platform.cmd.exit", &[Value::Int(7)])
            .expect("exit should succeed");
        assert_eq!(result, EffectOutcome::Signal(RuntimeSignal::Exit(7)));
    }

    #[test]
    fn exit_defensively_rejects_malformed_runtime_codes() {
        let h = package_host_handler(&[(
            "custom_platform.cmd",
            "pub foreign fn exit(code: U8) -> Never uses [Exit]",
        )]);
        let result_high = h.handle("exit", &[Value::Int(256)]);
        assert!(result_high.is_err(), "exit code 256 should be rejected");
        assert!(
            result_high.unwrap_err().contains("out of range"),
            "error message should mention out of range"
        );

        let result_neg = h.handle("exit", &[Value::Int(-1)]);
        assert!(result_neg.is_err(), "exit code -1 should be rejected");
        assert!(
            result_neg.unwrap_err().contains("out of range"),
            "error message should mention out of range"
        );
    }

    #[test]
    fn exit_code_in_range_returns_signal() {
        let h = package_host_handler(&[(
            "custom_platform.cmd",
            "pub foreign fn exit(code: U8) -> Never uses [Exit]",
        )]);

        let result_zero = h
            .handle("exit", &[Value::Int(0)])
            .expect("exit code 0 should succeed");
        assert_eq!(result_zero, EffectOutcome::Signal(RuntimeSignal::Exit(0)));

        let result_max = h
            .handle("exit", &[Value::Int(255)])
            .expect("exit code 255 should succeed");
        assert_eq!(result_max, EffectOutcome::Signal(RuntimeSignal::Exit(255)));
    }
}
