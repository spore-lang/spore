use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

#[test]
fn compile_succeeds_on_valid_file() {
    let temp = TempDir::new("compile-ok");
    let file = temp.write("main.sp", "fn main() -> I32 { 42 }\n");

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
    let file = temp.write("main.sp", "fn main() -> I32 { \"oops\" }\n");

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
    let file = temp.write("main.sp", "fn main() -> I32 { \"oops\" }\n");

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
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\":\"error\""), "stdout: {stdout}");
    assert!(stdout.contains("\"diagnostics\""), "stdout: {stdout}");
    assert!(stdout.contains("\"code\":\"E0001\""), "stdout: {stdout}");
}

#[test]
fn compile_json_parse_failures_include_canonical_diagnostics() {
    let temp = TempDir::new("compile-json-parse-fail");
    let file = temp.write("main.sp", "fn main( -> I32 { 42 }\n");

    let output = sporec_cmd()
        .args(["compile", "--json", file.to_str().unwrap()])
        .output()
        .expect("run sporec compile --json");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"diagnostics\""), "stdout: {stdout}");
    assert!(
        stdout.contains("\"code\":\"parse-error\""),
        "stdout: {stdout}"
    );
}

#[test]
fn compile_multiple_files_resolves_imports() {
    let temp = TempDir::new("compile-multi-imports");
    let main = temp.write(
        "main.sp",
        r#"
        import foo
        fn main() -> I32 { foo() }
        "#,
    );
    let foo = temp.write("foo.sp", "pub fn foo() -> I32 { 1 }\n");

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
        fn main() -> I32 { foo() }
        "#,
    );
    let foo = temp.write("foo.spore", "pub fn foo() -> I32 { 1 }\n");

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
        fn main() -> I32 {
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"holes\""), "stdout: {stdout}");
    assert!(
        stdout.contains("\"name\": \"todo\"") || stdout.contains("\"name\":\"todo\""),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("\"dependency_graph\""), "stdout: {stdout}");
    assert!(
        stdout.contains("\"display_name\": \"?todo\"")
            || stdout.contains("\"display_name\":\"?todo\""),
        "stdout: {stdout}"
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
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("\"status\":\"error\""), "stdout: {stdout}");
        assert!(
            stdout.contains("cannot read `does-not-exist.sp`"),
            "stdout: {stdout}"
        );
    }
}

#[test]
fn query_hole_json_finds_named_hole() {
    let temp = TempDir::new("query-hole-ok");
    let file = temp.write(
        "main.sp",
        r#"
        fn main() -> I32 {
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"name\":\"todo\"") || stdout.contains("\"name\": \"todo\""),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("\"display_name\":\"?todo\"")
            || stdout.contains("\"display_name\": \"?todo\""),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("\"expected_type\""), "stdout: {stdout}");
}

#[test]
fn query_hole_missing_exits_non_zero() {
    let temp = TempDir::new("query-hole-missing");
    let file = temp.write("main.sp", "fn main() -> I32 { 42 }\n");

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
