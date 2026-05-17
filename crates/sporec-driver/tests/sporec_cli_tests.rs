use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

struct TempDir {
    root: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("sporec-cli-{name}-{unique}-{}", std::process::id()));
        fs::create_dir_all(&root).expect("temp dir");
        Self { root }
    }

    fn write(&self, rel: &str, content: &str) -> PathBuf {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dirs");
        }
        fs::write(&path, content).expect("write test file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn sporec_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sporec"))
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected JSON stdout ({error}), got: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

#[test]
fn compile_succeeds_on_valid_file() {
    let temp = TempDir::new("compile-ok");
    let file = temp.write("main.sp", "fn main() -> I64 { 42 }\n");

    let output = sporec_cmd()
        .args(["compile", file.to_str().unwrap()])
        .output()
        .expect("run sporec compile");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("ok: no errors"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn compile_fails_on_invalid_file() {
    let temp = TempDir::new("compile-fail");
    let file = temp.write("main.sp", "fn main() -> I64 { \"oops\" }\n");

    let output = sporec_cmd()
        .args(["compile", file.to_str().unwrap()])
        .output()
        .expect("run sporec compile");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("E0001"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn compile_json_failures_include_canonical_diagnostics() {
    let temp = TempDir::new("compile-json-fail");
    let file = temp.write("main.sp", "fn main() -> I64 { \"oops\" }\n");

    let output = sporec_cmd()
        .args(["compile", "--json", file.to_str().unwrap()])
        .output()
        .expect("run sporec compile --json");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr should stay empty for JSON errors, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(&output);
    assert_eq!(json["status"], "error");
    let diagnostics = json["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "E0001"),
        "json: {json}"
    );
}

#[test]
fn compile_json_parse_failures_include_canonical_diagnostics() {
    let temp = TempDir::new("compile-json-parse-fail");
    let file = temp.write("main.sp", "fn main( -> I64 { 42 }\n");

    let output = sporec_cmd()
        .args(["compile", "--json", file.to_str().unwrap()])
        .output()
        .expect("run sporec compile --json");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let json = stdout_json(&output);
    let diagnostics = json["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "parse-error"),
        "json: {json}"
    );
}

#[test]
fn compile_multiple_files_resolves_imports() {
    let temp = TempDir::new("compile-multi-imports");
    let main = temp.write(
        "main.sp",
        r#"
        import foo
        fn main() -> I64 { foo() }
        "#,
    );
    let foo = temp.write("foo.sp", "pub fn foo() -> I64 { 1 }\n");

    let output = sporec_cmd()
        .args(["compile", main.to_str().unwrap(), foo.to_str().unwrap()])
        .output()
        .expect("run sporec compile for multiple files");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("ok: no errors (2 files)"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn compile_multiple_spore_files_resolves_imports() {
    let temp = TempDir::new("compile-multi-spore-imports");
    let main = temp.write(
        "main.spore",
        r#"
        import foo
        fn main() -> I64 { foo() }
        "#,
    );
    let foo = temp.write("foo.spore", "pub fn foo() -> I64 { 1 }\n");

    let output = sporec_cmd()
        .args(["compile", main.to_str().unwrap(), foo.to_str().unwrap()])
        .output()
        .expect("run sporec compile for multiple .spore files");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("ok: no errors (2 files)"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn holes_json_contains_holes_key() {
    let temp = TempDir::new("holes-json");
    let file = temp.write(
        "main.sp",
        r#"
        fn main() -> I64 {
            ?todo
        }
        "#,
    );

    let output = sporec_cmd()
        .args(["holes", "--json", file.to_str().unwrap()])
        .output()
        .expect("run sporec holes");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(&output);
    let holes = json["holes"].as_array().expect("holes should be an array");
    assert_eq!(holes.len(), 1, "json: {json}");
    assert_eq!(holes[0]["name"], "todo");
    assert_eq!(holes[0]["display_name"], "?todo");
    assert_eq!(holes[0]["location"]["line"], 3);
    assert!(
        json["dependency_graph"].is_object(),
        "dependency graph should be present: {json}"
    );
}

#[test]
fn json_commands_report_read_errors_as_json() {
    for args in [
        vec!["compile", "--json", "does-not-exist.sp"],
        vec!["holes", "--json", "does-not-exist.sp"],
        vec!["query-hole", "--json", "does-not-exist.sp", "?todo"],
    ] {
        let output = sporec_cmd()
            .args(&args)
            .output()
            .expect("run sporec JSON command");

        assert!(!output.status.success(), "args: {:?}", args);
        assert!(
            output.stderr.is_empty(),
            "stderr should stay empty for JSON errors, got: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json = stdout_json(&output);
        assert_eq!(json["status"], "error");
        assert!(
            json["message"]
                .as_str()
                .is_some_and(|message| message.contains("cannot read `does-not-exist.sp`")),
            "json: {json}"
        );
    }
}

#[test]
fn query_hole_json_finds_named_hole() {
    let temp = TempDir::new("query-hole-ok");
    let file = temp.write(
        "main.sp",
        r#"
        fn main() -> I64 {
            ?todo
        }
        "#,
    );

    let output = sporec_cmd()
        .args(["query-hole", "--json", file.to_str().unwrap(), "?todo"])
        .output()
        .expect("run sporec query-hole");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(&output);
    assert_eq!(json["name"], "todo");
    assert_eq!(json["display_name"], "?todo");
    assert_eq!(json["location"]["line"], 3);
    assert_eq!(json["expected_type"], "I64");
}

#[test]
fn query_hole_json_includes_checked_residual_context() {
    let temp = TempDir::new("query-hole-cost");
    let file = temp.write(
        "main.sp",
        r#"
        fn cheap() -> I64 cost [1, 0, 0, 0] { 1 + 1 }
        fn costly() -> I64 cost [10, 0, 0, 0] { cheap() + cheap() + cheap() }
        fn main() -> I64 cost [6, 0, 0, 0] {
            let seed = cheap();
            ?todo
        }
        "#,
    );

    let output = sporec_cmd()
        .args(["query-hole", "--json", file.to_str().unwrap(), "?todo"])
        .output()
        .expect("run sporec query-hole");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(&output);
    assert_eq!(
        json["residual_context"]["fit_rule"],
        "before + candidate <= budget"
    );
    let candidates = json["candidates"]
        .as_array()
        .expect("candidates should be an array");
    assert!(
        candidates.iter().any(|candidate| {
            candidate["cost_check"]["fits_budget"] == false
                && candidate["cost_check"]["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.contains("exceeds budget in compute"))
        }),
        "json: {json}"
    );
}

#[test]
fn query_hole_json_includes_effect_and_rejection_context() {
    let temp = TempDir::new("query-hole-effects");
    let file = temp.write(
        "main.sp",
        r#"
        effect Console {
            fn println(msg: Str) -> ()
        }
        effect Debug {
            fn trace(msg: Str) -> ()
        }
        fn pure() -> I64 { 1 }
        fn noisy() -> I64 uses [Debug] { 2 }
        fn main() -> I64 uses [IO] {
            handle {
                ?todo
            } with {
                on Console.println(msg) => { msg; }
            }
        }
        "#,
    );

    let output = sporec_cmd()
        .args(["query-hole", "--json", file.to_str().unwrap(), "?todo"])
        .output()
        .expect("run sporec query-hole");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(&output);
    assert_eq!(
        json["effect_context"]["discharged_effects"],
        serde_json::json!(["Console"])
    );
    assert!(
        json["effect_context"]["surviving_effects"]
            .as_array()
            .is_some_and(|effects| effects.iter().any(|effect| effect.as_str() == Some("IO"))),
        "json: {json}"
    );
    let candidates = json["candidates"]
        .as_array()
        .expect("candidates should be an array");
    assert!(
        candidates.iter().any(|candidate| {
            candidate["rejection_reasons"]
                .as_array()
                .is_some_and(|reasons| {
                    reasons.iter().any(|reason| {
                        reason
                            .as_str()
                            .is_some_and(|reason| reason.contains("requires effects [Debug]"))
                    })
                })
        }),
        "json: {json}"
    );
}

#[test]
fn query_hole_missing_exits_non_zero() {
    let temp = TempDir::new("query-hole-missing");
    let file = temp.write("main.sp", "fn main() -> I64 { 42 }\n");

    let output = sporec_cmd()
        .args(["query-hole", file.to_str().unwrap(), "?missing"])
        .output()
        .expect("run sporec query-hole");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not found"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn explain_prints_code_and_severity() {
    let output = sporec_cmd()
        .args(["explain", "E0001"])
        .output()
        .expect("run sporec explain");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("E0001"), "stdout: {stdout}");
    assert!(stdout.contains("severity:"), "stdout: {stdout}");
}
