use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct TempProject {
    root: tempfile::TempDir,
}

impl TempProject {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().expect("temp project"),
        }
    }

    fn root(&self) -> &Path {
        self.root.path()
    }

    fn write(&self, rel: &str, content: &str) -> PathBuf {
        let path = self.root().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dirs");
        }
        fs::write(&path, content).expect("write project file");
        path
    }
}

fn spore_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_spore"))
}

#[test]
fn standalone_run_ignores_return_value_by_default() {
    let project = TempProject::new();
    let file = project.write("main.sp", "fn main() -> I32 { 42 }\n");

    let output = spore_cmd()
        .args(["run", file.to_str().expect("utf-8 path")])
        .output()
        .expect("run spore");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn standalone_run_prints_only_explicit_console_output() {
    let project = TempProject::new();
    let file = project.write(
        "main.sp",
        r#"
        fn main() -> () {
            println("hello");
            return
        }
        "#,
    );

    let output = spore_cmd()
        .args(["run", file.to_str().expect("utf-8 path")])
        .output()
        .expect("run spore");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

#[test]
fn standalone_run_json_omits_completion_value() {
    let project = TempProject::new();
    let file = project.write("main.sp", "fn main() -> I32 { 42 }\n");

    let output = spore_cmd()
        .args(["run", "--json", file.to_str().expect("utf-8 path")])
        .output()
        .expect("run spore");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\":\"ok\""), "stdout: {stdout}");
    assert!(!stdout.contains("\"value\""), "stdout: {stdout}");
}

#[test]
fn project_basic_cli_exit_returns_requested_code_without_printing_value() {
    let project = TempProject::new();
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "demo"
        type = "application"

        [project]
        platform = "basic-cli"
        default-entry = "app"

        [dependencies]
        basic-cli = { path = "vendor/basic-cli" }

        [entries.app]
        path = "app.sp"
        "#,
    );
    project.write(
        "vendor/basic-cli/spore.toml",
        r#"
        [package]
        name = "basic-cli"
        type = "platform"

        [platform]
        contract-module = "platform_contract"
        startup-contract = "main"
        adapter-function = "main_for_host"
        handles = ["Exit"]
        "#,
    );
    project.write(
        "vendor/basic-cli/src/platform_contract.sp",
        r#"
        pub fn main() -> () {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> ()) -> () {
            app_main();
            return
        }
        "#,
    );
    project.write(
        "vendor/basic-cli/src/basic_cli/cmd.sp",
        r#"
        pub foreign fn exit(code: Int) -> Never uses [Exit]
        "#,
    );
    let entry = project.write(
        "src/app.sp",
        r#"
        import basic_cli.cmd

        fn exit_code() -> Int { 7 }

        fn main() -> () uses [Exit] {
            exit(exit_code())
        }
        "#,
    );

    let output = spore_cmd()
        .args(["run", entry.to_str().expect("utf-8 path")])
        .output()
        .expect("run spore");

    assert_eq!(
        output.status.code(),
        Some(7),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
