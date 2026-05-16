use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

fn assert_help_succeeds(args: &[&str], expected: &str) {
    let output = spore_cmd().args(args).output().expect("run spore help");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(expected), "stdout: {stdout}");
}

fn assert_build_succeeded(output: &Output, artifact: &Path) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        artifact.is_file(),
        "expected native artifact at {}",
        artifact.display()
    );
    let metadata = fs::metadata(artifact).expect("artifact metadata");
    assert!(metadata.len() > 0, "artifact should not be empty");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("built native object"), "stdout: {stdout}");
    assert!(
        !stdout.contains("interpreter mode"),
        "build output should not claim interpreter mode: {stdout}"
    );
}

fn write_cli_project(project: &TempProject) -> PathBuf {
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "demo"
        type = "application"

        [project]
        platform = "cli"
        default-entry = "app"

        [entries.app]
        path = "app.sp"

        [entries.repl]
        path = "tools/repl.sp"
        "#,
    );
    project.write(
        "src/app.sp",
        r#"
        import util

        fn main() -> () {
            helper()
        }
        "#,
    );
    project.write(
        "src/util.sp",
        r#"
        pub fn helper() -> () {}
        "#,
    );
    project.write(
        "src/tools/repl.sp",
        r#"
        import util

        fn main() -> () {
            helper()
        }
        "#,
    )
}

#[test]
fn help_output_does_not_panic() {
    assert_help_succeeds(&["--help"], "spore — the Spore language toolkit");
    assert_help_succeeds(&["new", "--help"], "Create a new Spore project");
    assert_help_succeeds(
        &["init", "--help"],
        "Initialize Spore project in current directory",
    );
}

#[test]
fn standalone_run_ignores_return_value_by_default() {
    let project = TempProject::new();
    let file = project.write("main.sp", "fn main() -> I64 { 42 }\n");

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
fn standalone_run_rejects_console_builtins() {
    // `println` is not available in standalone mode; it must be rejected at type-check time.
    let project = TempProject::new();
    let file = project.write(
        "main.sp",
        r#"
        fn main() -> () uses [Console] {
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
        !output.status.success(),
        "expected failure because println is undefined in standalone mode"
    );
}

#[test]
fn standalone_run_json_omits_completion_value() {
    let project = TempProject::new();
    let file = project.write("main.sp", "fn main() -> I64 { 42 }\n");

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
fn standalone_build_writes_native_object_file() {
    let project = TempProject::new();
    let file = project.write(
        "main.sp",
        r#"
        fn choose(flag: Bool) -> Bool {
            if flag { true } else { false }
        }

        fn main() -> Bool {
            choose(true)
        }
        "#,
    );
    let artifact = file.with_extension("o");

    let output = spore_cmd()
        .args(["build", file.to_str().expect("utf-8 path")])
        .output()
        .expect("run spore build");

    assert_build_succeeded(&output, &artifact);
}

#[test]
fn standalone_build_rejects_native_unsupported_source_explicitly() {
    let project = TempProject::new();
    let file = project.write("main.sp", "fn main() -> Str { \"hello\" }\n");

    let output = spore_cmd()
        .args(["build", file.to_str().expect("utf-8 path")])
        .output()
        .expect("run spore build");

    assert!(!output.status.success(), "expected native build failure");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("native build unavailable"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("unsupported native backend feature"),
        "stderr: {stderr}"
    );
}

#[test]
fn project_build_without_argument_uses_default_target() {
    let project = TempProject::new();
    write_cli_project(&project);
    let artifact = project.root().join("target/native/app.o");

    let output = spore_cmd()
        .arg("build")
        .current_dir(project.root())
        .output()
        .expect("run spore build");

    assert_build_succeeded(&output, &artifact);
}

#[test]
fn project_build_accepts_dot_for_project_root() {
    let project = TempProject::new();
    write_cli_project(&project);
    let artifact = project.root().join("target/native/app.o");

    let output = spore_cmd()
        .args(["build", "."])
        .current_dir(project.root())
        .output()
        .expect("run spore build");

    assert_build_succeeded(&output, &artifact);
}

#[test]
fn project_build_accepts_project_directory_argument() {
    let project = TempProject::new();
    write_cli_project(&project);
    let artifact = project.root().join("target/native/app.o");

    let output = spore_cmd()
        .args(["build", project.root().to_str().expect("utf-8 path")])
        .output()
        .expect("run spore build");

    assert_build_succeeded(&output, &artifact);
}

#[test]
fn project_build_file_in_project_writes_project_relative_artifact() {
    let project = TempProject::new();
    let module = write_cli_project(&project);
    let artifact = project.root().join("target/native/tools/repl.o");

    let output = spore_cmd()
        .args(["build", module.to_str().expect("utf-8 path")])
        .output()
        .expect("run spore build");

    assert_build_succeeded(&output, &artifact);
}

#[test]
fn project_build_surfaces_ambiguous_import_errors() {
    let project = TempProject::new();
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "demo"
        type = "application"

        [project]
        platform = "cli"
        default-entry = "app"

        [entries.app]
        path = "app.sp"
        "#,
    );
    project.write(
        "src/app.sp",
        r#"
        import left
        import right

        fn main() -> () {
            helper()
        }
        "#,
    );
    project.write("src/left.sp", "pub fn helper() -> () {}\n");
    project.write("src/right.sp", "pub fn helper() -> () {}\n");

    let output = spore_cmd()
        .arg("build")
        .current_dir(project.root())
        .output()
        .expect("run spore build");

    assert!(
        !output.status.success(),
        "expected ambiguous project build failure"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ambiguous native project import"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("helper"), "stderr: {stderr}");
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
        handled-effects = ["Exit"]
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
        pub foreign fn exit(code: U8) -> Never uses [Exit]
        "#,
    );
    let entry = project.write(
        "src/app.sp",
        r#"
        import basic_cli.cmd

        fn exit_code() -> U8 { 7u8 }

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

#[test]
fn project_test_reuses_project_aware_import_resolution() {
    let project = TempProject::new();
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "basic-cli"
        type = "platform"

        [platform]
        contract-module = "platform_contract"
        startup-contract = "main"
        adapter-function = "main_for_host"
        handled-effects = ["Console"]
        "#,
    );
    project.write(
        "src/platform_contract.sp",
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
        "src/basic_cli/stdout.sp",
        r#"
        pub foreign fn println(s: Str) -> () uses [Console]
        "#,
    );
    project.write(
        "examples/hello-app/spore.toml",
        r#"
        [package]
        name = "hello-app"
        type = "application"

        [project]
        platform = "basic-cli"
        default-entry = "app"

        [entries.app]
        path = "main.sp"

        [dependencies]
        basic-cli = { path = "../.." }
        "#,
    );
    project.write(
        "examples/hello-app/src/main.sp",
        r#"
        import basic_cli.stdout as stdout

        fn local_identity(x: I64) -> I64
        spec {
            example "basic": local_identity(42) == 42
        }
        {
            x
        }

        fn main() -> () uses [Console] {
            println("hello");
            return
        }
        "#,
    );

    let output = spore_cmd()
        .arg("test")
        .current_dir(project.root())
        .output()
        .expect("run spore test");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("1 specs passed"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn project_test_runs_specs_in_imported_embedded_law_modules() {
    let project = TempProject::new();
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "law-checks"
        type = "application"

        [project]
        platform = "cli"
        default-entry = "app"

        [entries.app]
        path = "main.sp"
        "#,
    );
    project.write(
        "src/main.sp",
        r#"
        import spore.laws

        fn main() -> () {
            canonical_members_i32([1i32, 1i32, 2i32]);
            sum3_left_assoc_i32(20i32, 10i32, 12i32);
            return
        }
        "#,
    );

    let output = spore_cmd()
        .arg("test")
        .current_dir(project.root())
        .output()
        .expect("run spore test");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("specs passed"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
