use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sporec_codegen::value::Value;
use sporec_driver::{
    CheckFailure, CheckReport, check_files, check_project, check_project_verbose, check_verbose,
    compile, compile_project, hole_summary, run_project, run_project_with_outcome,
};

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("sporec-{name}-{unique}-{}", std::process::id()));
        fs::create_dir_all(root.join("src")).expect("temp project src dir");
        fs::write(root.join("spore.toml"), "name = \"temp\"\n").expect("temp project manifest");
        Self { root }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn write(&self, rel: &str, content: &str) {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dirs");
        }
        fs::write(path, content).expect("write project file");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const APP_MANIFEST_WITH_BASIC_CLI: &str = r#"
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
"#;

fn write_basic_cli_platform(project: &TempProject, contract_source: &str) {
    write_basic_cli_platform_with_handles(
        project,
        &["Console", "FileRead", "FileWrite", "Env", "Spawn"],
        contract_source,
    );
}

fn write_basic_cli_platform_with_handles(
    project: &TempProject,
    handles: &[&str],
    contract_source: &str,
) {
    let handles = handles
        .iter()
        .map(|handle| format!("\"{handle}\""))
        .collect::<Vec<_>>()
        .join(", ");
    project.write(
        "vendor/basic-cli/spore.toml",
        &format!(
            r#"
        [package]
        name = "basic-cli"
        type = "platform"

        [platform]
        contract-module = "platform_contract"
        startup-contract = "main"
        adapter-function = "main_for_host"
        handles = [{handles}]
        "#
        ),
    );
    project.write("vendor/basic-cli/src/platform_contract.sp", contract_source);
}

fn write_basic_cli_stdout_module(project: &TempProject) {
    project.write(
        "vendor/basic-cli/src/basic_cli/stdout.sp",
        r#"
        pub foreign fn println(s: Str) -> () uses [Console]
        "#,
    );
}

fn write_basic_cli_file_module(project: &TempProject) {
    project.write(
        "vendor/basic-cli/src/basic_cli/file.sp",
        r#"
        pub foreign fn file_exists(path: Str) -> Bool uses [FileRead]
        "#,
    );
}

fn write_basic_cli_cmd_module(project: &TempProject) {
    project.write(
        "vendor/basic-cli/src/basic_cli/cmd.sp",
        r#"
        pub foreign fn exit(code: I32) -> Never uses [Exit]
        "#,
    );
}

// ── Verbose output tests ────────────────────────────────────────────

#[test]
fn check_verbose_ok_includes_section_headers() {
    let output = check_verbose("fn f() -> I32 { 42 }").unwrap();
    assert!(
        output.contains("✓ no errors"),
        "verbose output should start with success marker, got: {output}"
    );
    assert!(
        output.contains("── Type Inference ──"),
        "verbose output should include type inference section, got: {output}"
    );
}

#[test]
fn check_verbose_reports_holes() {
    let output = check_verbose(
        r#"
        fn f() -> I32 {
            ?todo
        }
    "#,
    )
    .unwrap();
    assert!(
        output.contains("holes: 1 total"),
        "verbose output should report 1 hole, got: {output}"
    );
    assert!(
        output.contains("?todo"),
        "verbose output should name the hole, got: {output}"
    );
}

#[test]
fn check_verbose_uses_cost_vector_syntax() {
    let output = check_verbose("fn f(x: I32) -> I32 cost [2, 0, 0, 0] { x + x }").unwrap();
    assert!(
        output.contains("cost ["),
        "verbose output should use vector syntax, got: {output}"
    );
    assert!(
        !output.contains("compute="),
        "verbose output should avoid scalar-style fields, got: {output}"
    );
}

#[test]
fn check_verbose_hides_synthetic_hole_names() {
    let output = check_verbose(
        r#"
        fn f() -> I32 {
            ?
        }
    "#,
    )
    .unwrap();
    assert!(
        output.contains("?: expected I32"),
        "verbose output should render unnamed holes as `?`, got: {output}"
    );
    assert!(
        !output.contains("_hole"),
        "verbose output should not leak synthetic hole ids, got: {output}"
    );
}

#[test]
fn check_verbose_keeps_user_named_hole_names() {
    let output = check_verbose(
        r#"
        fn f() -> I32 {
            ?_hole_manual
        }
    "#,
    )
    .unwrap();
    assert!(
        output.contains("?_hole_manual: expected I32"),
        "verbose output should keep user-authored hole names, got: {output}"
    );
}

#[test]
fn check_verbose_returns_error_on_invalid() {
    let result = check_verbose(r#"fn f() -> I32 { "oops" }"#);
    assert!(result.is_err(), "type error should produce Err");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("E0001"),
        "error should contain new code E0001, got: {msg}"
    );
}

// ── Hole summary tests ──────────────────────────────────────────────

#[test]
fn hole_summary_none_when_no_holes() {
    let summary = hole_summary("fn f() -> I32 { 42 }");
    assert!(summary.is_none(), "no holes should produce None");
}

#[test]
fn hole_summary_present_with_holes() {
    let summary = hole_summary(
        r#"
        fn f() -> I32 {
            ?todo
        }
    "#,
    );
    assert!(summary.is_some(), "holes should produce Some");
    let s = summary.unwrap();
    assert!(s.holes_total >= 1, "should have at least 1 hole");
    assert_eq!(s.filled_this_cycle, 0, "no fills in a single cycle");
}

#[test]
fn hole_summary_json_format() {
    let summary = hole_summary(
        r#"
        fn f() -> I32 {
            ?todo
        }
    "#,
    )
    .unwrap();
    let json = summary.to_json();
    let value = serde_json::to_value(&summary).expect("serialize hole summary");
    assert!(
        json.contains("\"event\":\"hole_graph_update\""),
        "JSON should contain hole_graph_update event, got: {json}"
    );
    assert!(
        json.contains("\"holes_total\":"),
        "JSON should contain holes_total, got: {json}"
    );
    assert!(
        json.contains("\"ready_to_fill\":"),
        "JSON should contain ready_to_fill, got: {json}"
    );
    assert!(
        json.contains("\"blocked\":"),
        "JSON should contain blocked, got: {json}"
    );
    assert_eq!(value["event"], "hole_graph_update");
    assert!(value["holes_total"].as_u64().is_some());
}

// ── Error code migration sanity ─────────────────────────────────────

#[test]
fn compile_error_uses_new_codes() {
    let err = compile(r#"fn f() -> I32 { "oops" }"#).unwrap_err();
    assert!(
        err.contains("[E0001]"),
        "compile error should use 4-digit code, got: {err}"
    );
    assert!(
        !err.contains("[E001]") || err.contains("[E0001]"),
        "should not contain old 3-digit code"
    );
}

#[test]
fn compile_accepts_source_defined_prelude_items() {
    let output = compile(
        r#"
        fn main() -> I32 {
            match compare(identity(2), bool_to_int(not(false))) {
                Less => 0,
                Equal => 1,
                Greater => unwrap_or(Some(42), 0),
            }
        }
    "#,
    )
    .expect("source-defined prelude items should type-check");
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        output.warnings
    );
}

#[test]
fn compile_rejects_non_prelude_stdlib_by_default() {
    let err = compile("fn main() -> I32 { clamp(5, 0, 10) }")
        .expect_err("non-prelude stdlib should not be globally injected");
    assert!(
        err.contains("undefined variable `clamp`"),
        "expected clamp to stay unavailable by default, got: {err}"
    );
}

#[test]
fn compile_accepts_spec_clause_syntax() {
    let output = compile(
        r#"
        fn add(a: I32, b: I32) -> I32
        spec {
            example "basic": add(2, 3) == 5
            property "commutative": |a: I32, b: I32| add(a, b) == add(b, a)
        }
        {
            a + b
        }
    "#,
    )
    .expect("spec clause should compile through sporec");
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        output.warnings
    );
}

#[test]
fn compile_accepts_effect_and_handler_items() {
    let output = compile(
        r#"
        effect Console {
            fn println(msg: Str) -> ()
        }
        handler MockConsole for Console {
            fn println(msg: Str) -> () { return }
        }
        fn main() -> I32 { 0 }
    "#,
    )
    .expect("effect and handler items should compile through sporec");
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        output.warnings
    );
}

#[test]
fn compile_project_rejects_type_error_in_imported_module() {
    let project = TempProject::new("project-import-type-error");
    project.write(
        "src/main.sp",
        r#"
        import utils
        fn main() -> () { double(21); return }
        "#,
    );
    project.write(
        "src/utils.sp",
        r#"
        pub fn double(x: I32) -> I32 { "oops" }
        "#,
    );

    let err = compile_project(project.root(), "main.sp")
        .expect_err("project compile should reject invalid imported module bodies");
    assert!(
        err.contains("utils.sp"),
        "expected imported module path in error, got: {err}"
    );
    assert!(
        err.contains("double") || err.contains("E0001"),
        "expected imported module type error details, got: {err}"
    );
}

#[test]
fn check_project_returns_canonical_diagnostics_for_imported_module_type_error() {
    let project = TempProject::new("project-report-import-type-error");
    project.write(
        "src/main.sp",
        r#"
        import utils
        fn main() -> () { double(21); return }
        "#,
    );
    project.write(
        "src/utils.sp",
        r#"
        pub fn double(x: I32) -> I32 { "oops" }
        "#,
    );

    match check_project(project.root(), "main.sp") {
        CheckReport::Failure(CheckFailure::Diagnostics {
            sources,
            diagnostics,
        }) => {
            assert!(
                sources.iter().any(|source| source.name() == "utils.sp"),
                "expected imported module source in diagnostic set"
            );
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .primary_span
                        .as_ref()
                        .map(|span| span.file.as_str())
                        == Some("utils.sp")
                }),
                "expected imported module diagnostic, got: {diagnostics:?}"
            );
        }
        other => panic!("expected canonical imported-module diagnostics, got: {other:?}"),
    }
}

#[test]
fn check_project_returns_canonical_parse_diagnostics_for_imported_module() {
    let project = TempProject::new("project-report-import-parse-error");
    project.write(
        "src/main.sp",
        r#"
        import utils
        fn main() -> () { return }
        "#,
    );
    project.write("src/utils.sp", "pub fn double(x: I32) -> I32 { \n");

    match check_project(project.root(), "main.sp") {
        CheckReport::Failure(CheckFailure::Diagnostics { diagnostics, .. }) => {
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "parse-error"
                        && diagnostic
                            .primary_span
                            .as_ref()
                            .map(|span| span.file.as_str())
                            == Some("utils.sp")
                }),
                "expected canonical parse diagnostic for imported module, got: {diagnostics:?}"
            );
        }
        other => panic!("expected canonical parse diagnostics, got: {other:?}"),
    }
}

#[test]
fn check_project_returns_startup_diagnostic_for_missing_entry_function() {
    let project = TempProject::new("project-report-missing-startup");
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
        fn boot() -> () {
            return
        }
        "#,
    );

    match check_project(project.root(), "app.sp") {
        CheckReport::Failure(CheckFailure::Diagnostics {
            sources,
            diagnostics,
        }) => {
            assert!(
                sources.iter().any(|source| source.name() == "app.sp"),
                "expected entry module source in diagnostic set"
            );
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "missing-startup-function"),
                "expected startup diagnostic, got: {diagnostics:?}"
            );
        }
        other => panic!("expected startup diagnostics, got: {other:?}"),
    }
}

#[test]
fn check_files_returns_canonical_parse_diagnostics() {
    let project = TempProject::new("batch-parse-error");
    project.write("src/a.sp", "fn main() -> () { return }\n");
    project.write("src/b.sp", "fn broken( -> () { return }\n");
    let first = project.root().join("src/a.sp");
    let second = project.root().join("src/b.sp");
    let refs = [
        first.to_str().expect("utf8 path"),
        second.to_str().expect("utf8 path"),
    ];

    match check_files(&refs) {
        CheckReport::Failure(CheckFailure::Diagnostics { diagnostics, .. }) => {
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "parse-error"
                        && diagnostic
                            .primary_span
                            .as_ref()
                            .map(|span| span.file.ends_with("b.sp"))
                            .unwrap_or(false)
                }),
                "expected batch parse diagnostic, got: {diagnostics:?}"
            );
        }
        other => panic!("expected canonical batch parse diagnostics, got: {other:?}"),
    }
}

#[test]
fn run_project_rejects_type_error_in_imported_module_before_execution() {
    let project = TempProject::new("project-import-run-type-error");
    project.write(
        "src/main.sp",
        r#"
        import utils
        fn main() -> () { double(21); return }
        "#,
    );
    project.write(
        "src/utils.sp",
        r#"
        pub fn double(x: I32) -> I32 { "oops" }
        "#,
    );

    let err = run_project(project.root(), "main.sp")
        .expect_err("project run should fail during type checking before execution");
    assert!(
        err.contains("utils.sp"),
        "expected imported module path in error, got: {err}"
    );
    assert!(
        err.contains("double") || err.contains("E0001"),
        "expected imported module type error details, got: {err}"
    );
}

#[test]
fn check_project_verbose_rejects_type_error_in_imported_module() {
    let project = TempProject::new("project-verbose-import-type-error");
    project.write(
        "src/main.sp",
        r#"
        import utils
        fn main() -> I32 { double(21) }
        "#,
    );
    project.write(
        "src/utils.sp",
        r#"
        pub fn double(x: I32) -> I32 { "oops" }
        "#,
    );

    let err = check_project_verbose(project.root(), "main.sp")
        .expect_err("project verbose check should reject invalid imported module bodies");
    assert!(
        err.contains("utils.sp"),
        "expected imported module path in error, got: {err}"
    );
    assert!(
        err.contains("double") || err.contains("E0001"),
        "expected imported module type error details, got: {err}"
    );
}

#[test]
fn check_project_verbose_includes_imported_module_sections() {
    let project = TempProject::new("project-verbose-ok");
    project.write(
        "src/main.sp",
        r#"
        import utils
        fn main() -> () { double(21); return }
        "#,
    );
    project.write(
        "src/utils.sp",
        r#"
        pub fn double(x: I32) -> I32 { x + x }
        "#,
    );

    let detail = check_project_verbose(project.root(), "main.sp")
        .expect("project verbose check should succeed for valid imported modules");
    assert!(
        detail.contains("✓ no errors"),
        "expected success marker, got: {detail}"
    );
    assert!(
        detail.contains("utils.sp"),
        "expected imported module section, got: {detail}"
    );
    assert!(
        detail.contains("main.sp"),
        "expected entry module section, got: {detail}"
    );
    assert!(
        detail.matches("── Type Inference ──").count() >= 2,
        "expected per-module verbose sections, got: {detail}"
    );
}

#[test]
fn compile_project_resolves_imports_from_path_dependency_modules() {
    let project = TempProject::new("project-import-path-dependency");
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "demo"
        type = "application"

        [project]
        platform = "cli"
        default-entry = "app"

        [dependencies]
        basic-cli = { path = "vendor/basic-cli" }

        [entries.app]
        path = "app.sp"
        "#,
    );
    project.write(
        "src/app.sp",
        r#"
        import basic_cli.stdout

        fn main() -> () {
            println("hello from dependency")
            return
        }
        "#,
    );
    project.write(
        "vendor/basic-cli/spore.toml",
        r#"
        [package]
        name = "basic-cli"
        type = "platform"
        "#,
    );
    project.write(
        "vendor/basic-cli/src/basic_cli/stdout.sp",
        r#"
        pub fn println(s: Str) -> () {
            return
        }
        "#,
    );

    let output = compile_project(project.root(), "app.sp")
        .expect("path dependency module imports should resolve during project compilation");
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        output.warnings
    );
}

#[test]
fn compile_project_accepts_platform_dependency_console_imports() {
    let project = TempProject::new("project-path-platform-console-import");
    project.write("spore.toml", APP_MANIFEST_WITH_BASIC_CLI);
    write_basic_cli_platform(
        &project,
        r#"
        pub fn main() -> () {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> ()) -> () {
            app_main()
            return
        }
        "#,
    );
    write_basic_cli_stdout_module(&project);
    project.write(
        "src/app.sp",
        r#"
        import basic_cli.stdout

        fn main() -> () uses [Console] {
            println("hello from imported console")
            return
        }
        "#,
    );

    let output = compile_project(project.root(), "app.sp")
        .expect("package-backed platform projects should use imported console modules");
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        output.warnings
    );
}

#[test]
fn compile_project_rejects_platform_dependency_console_imports_without_uses() {
    let project = TempProject::new("project-path-platform-console-import-without-uses");
    project.write("spore.toml", APP_MANIFEST_WITH_BASIC_CLI);
    write_basic_cli_platform(
        &project,
        r#"
        pub fn main() -> () {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> ()) -> () {
            app_main()
            return
        }
        "#,
    );
    write_basic_cli_stdout_module(&project);
    project.write(
        "src/app.sp",
        r#"
        import basic_cli.stdout

        fn main() -> () {
            println("hello from imported console")
            return
        }
        "#,
    );

    let err = compile_project(project.root(), "app.sp")
        .expect_err("imported platform functions should preserve capability requirements");
    assert!(
        err.contains("missing capabilities"),
        "expected capability propagation error, got: {err}"
    );
    assert!(
        err.contains("Console"),
        "expected Console capability error, got: {err}"
    );
}

#[test]
fn compile_project_rejects_platform_dependency_bare_console_without_import() {
    let project = TempProject::new("project-path-platform-bare-console");
    project.write("spore.toml", APP_MANIFEST_WITH_BASIC_CLI);
    write_basic_cli_platform(
        &project,
        r#"
        pub fn main() -> () {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> ()) -> () {
            app_main()
            return
        }
        "#,
    );
    write_basic_cli_stdout_module(&project);
    project.write(
        "src/app.sp",
        r#"
        fn main() -> () {
            println("missing import")
            return
        }
        "#,
    );

    let err = compile_project(project.root(), "app.sp")
        .expect_err("package-backed platform projects should not get console prelude builtins");
    assert!(
        err.contains("undefined variable `println`"),
        "expected missing import error, got: {err}"
    );
}

#[test]
fn compile_project_rejects_startup_capabilities_outside_platform_handles() {
    let project = TempProject::new("project-platform-handles-mismatch");
    project.write("spore.toml", APP_MANIFEST_WITH_BASIC_CLI);
    write_basic_cli_platform_with_handles(
        &project,
        &["FileRead", "FileWrite", "Env", "Spawn", "Exit"],
        r#"
        pub fn main() -> () {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> ()) -> () {
            app_main()
            return
        }
        "#,
    );
    write_basic_cli_stdout_module(&project);
    project.write(
        "src/app.sp",
        r#"
        import basic_cli.stdout

        fn main() -> () uses [Console] {
            println("hello from imported console")
            return
        }
        "#,
    );

    let err = compile_project(project.root(), "app.sp")
        .expect_err("startup capabilities should be validated against [platform].handles");
    assert!(
        err.contains("[platform].handles"),
        "expected handles validation error, got: {err}"
    );
    assert!(
        err.contains("Console"),
        "expected missing Console handle, got: {err}"
    );
}

#[test]
fn compile_project_accepts_imported_effect_operations() {
    let project = TempProject::new("project-imported-effect-operations");
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "demo"
        type = "application"

        [dependencies]
        effects = { path = "vendor/effects" }
        "#,
    );
    project.write(
        "src/main.sp",
        r#"
        import effects.console

        fn main() -> () uses [Console] {
            let line = perform Console.read_line();
            line;
            return
        }
        "#,
    );
    project.write(
        "vendor/effects/spore.toml",
        r#"
        [package]
        name = "effects"
        type = "package"
        "#,
    );
    project.write(
        "vendor/effects/src/effects/console.sp",
        r#"
        pub effect Console {
            fn read_line() -> Str
        }
        "#,
    );

    let output = compile_project(project.root(), "main.sp")
        .expect("imported effect interfaces should preserve operation signatures");
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        output.warnings
    );
}

#[test]
fn compile_project_rejects_wrong_args_for_imported_effect_operations() {
    let project = TempProject::new("project-imported-effect-operations-wrong-args");
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "demo"
        type = "application"

        [dependencies]
        effects = { path = "vendor/effects" }
        "#,
    );
    project.write(
        "src/main.sp",
        r#"
        import effects.console

        fn main() -> () uses [Console] {
            perform Console.println(42)
        }
        "#,
    );
    project.write(
        "vendor/effects/spore.toml",
        r#"
        [package]
        name = "effects"
        type = "package"
        "#,
    );
    project.write(
        "vendor/effects/src/effects/console.sp",
        r#"
        pub effect Console {
            fn println(msg: Str) -> ()
        }
        "#,
    );

    let err = compile_project(project.root(), "main.sp")
        .expect_err("imported effect operation signatures should still be typechecked");
    assert!(
        err.contains("argument 1 of `Console.println`")
            || err.contains("expected `Str`, got `I32`"),
        "expected imported effect argument type error, got: {err}"
    );
}

#[test]
fn compile_legacy_project_resolves_transitive_path_dependency_imports() {
    let project = TempProject::new("legacy-project-transitive-path-deps");
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "demo"
        type = "application"

        [dependencies]
        dep-a = { path = "vendor/dep-a" }
        "#,
    );
    project.write(
        "src/main.sp",
        r#"
        import dep_a.wrapper

        fn main() -> () {
            print_message()
            return
        }
        "#,
    );
    project.write(
        "vendor/dep-a/spore.toml",
        r#"
        [package]
        name = "dep-a"
        type = "package"

        [dependencies]
        dep-b = { path = "../dep-b" }
        "#,
    );
    project.write(
        "vendor/dep-a/src/dep_a/wrapper.sp",
        r#"
        import dep_b.message

        pub fn print_message() -> () {
            message()
            return
        }
        "#,
    );
    project.write(
        "vendor/dep-b/spore.toml",
        r#"
        [package]
        name = "dep-b"
        type = "package"
        "#,
    );
    project.write(
        "vendor/dep-b/src/dep_b/message.sp",
        r#"
        pub fn message() -> () {
            return
        }
        "#,
    );

    let output = compile_project(project.root(), "main.sp")
        .expect("legacy projects should resolve direct and transitive path dependency imports");
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        output.warnings
    );
}

#[test]
fn compile_project_rejects_declared_entry_without_required_startup() {
    let project = TempProject::new("project-missing-startup");
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
        fn boot() -> () {
            return
        }
        "#,
    );

    let err = compile_project(project.root(), "app.sp")
        .expect_err("declared entry without main should fail startup validation");
    assert!(
        err.contains("required startup function `main`"),
        "expected startup validation error, got: {err}"
    );
}

#[test]
fn compile_project_accepts_alias_equivalent_startup_signature() {
    let project = TempProject::new("project-alias-startup");
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
        fn main() -> Unit {
            return
        }

        alias Unit = ()
        "#,
    );

    let output = compile_project(project.root(), "app.sp")
        .expect("alias-equivalent startup signature should pass validation");
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        output.warnings
    );
}

#[test]
fn compile_project_accepts_platform_dependency_startup_contract() {
    let project = TempProject::new("project-path-platform-ok");
    project.write("spore.toml", APP_MANIFEST_WITH_BASIC_CLI);
    write_basic_cli_platform(
        &project,
        r#"
        pub fn main() -> () {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> ()) -> () {
            app_main()
            return
        }
        "#,
    );
    project.write(
        "src/app.sp",
        r#"
        fn main() -> Unit {
            return
        }

        alias Unit = ()
        "#,
    );

    let output = compile_project(project.root(), "app.sp")
        .expect("path dependency platform contract should validate startup");
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        output.warnings
    );
}

#[test]
fn run_project_routes_package_platforms_through_adapter() {
    let project = TempProject::new("project-path-platform-runtime-adapter");
    project.write("spore.toml", APP_MANIFEST_WITH_BASIC_CLI);
    project.write(
        "vendor/basic-cli/src/basic_cli/runtime.sp",
        r#"
        import basic_cli.runtime_inner

        pub fn value() -> I32 {
            helper()
        }
        "#,
    );
    project.write(
        "vendor/basic-cli/src/basic_cli/runtime_inner.sp",
        r#"
        pub fn helper() -> I32 {
            42
        }
        "#,
    );
    write_basic_cli_platform(
        &project,
        r#"
        import basic_cli.runtime

        pub fn main() -> I32 {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> I32) -> I32 {
            value()
        }
        "#,
    );
    project.write(
        "src/app.sp",
        r#"
        fn main() -> I32 {
            ?entry_startup_should_not_run_directly
        }

        fn main_for_host(app_main: () -> I32) -> I32 {
            ?entry_adapter_shadow_should_not_run
        }
        "#,
    );

    let value = run_project(project.root(), "app.sp")
        .expect("package-backed runtime should invoke the platform adapter");
    assert_eq!(value.as_int(), Some(42));
    assert!(
        matches!(value, Value::Int(42)),
        "expected adapter return value, got: {value:?}"
    );
}

#[test]
fn run_project_dispatches_basic_cli_package_foreign_functions() {
    let project = TempProject::new("project-path-platform-runtime-host-dispatch");
    project.write("spore.toml", APP_MANIFEST_WITH_BASIC_CLI);
    write_basic_cli_platform(
        &project,
        r#"
        pub fn main() -> Bool {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> Bool) -> Bool {
            app_main()
        }
        "#,
    );
    write_basic_cli_file_module(&project);
    project.write(
        "src/app.sp",
        &format!(
            r#"
            import basic_cli.file

            fn main() -> Bool uses [FileRead] {{
                file_exists("{}")
            }}
            "#,
            project.root().join("spore.toml").display()
        ),
    );

    let value = run_project(project.root(), "app.sp")
        .expect("package-backed runtime should dispatch basic-cli foreign functions");
    assert_eq!(value.as_bool(), Some(true));
}

#[test]
fn run_project_with_outcome_returns_basic_cli_exit_code() {
    let project = TempProject::new("project-basic-cli-exit-outcome");
    project.write("spore.toml", APP_MANIFEST_WITH_BASIC_CLI);
    write_basic_cli_platform_with_handles(
        &project,
        &["Console", "FileRead", "FileWrite", "Env", "Spawn", "Exit"],
        r#"
        pub fn main() -> () {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> ()) -> () {
            app_main()
            return
        }
        "#,
    );
    write_basic_cli_cmd_module(&project);
    project.write(
        "src/app.sp",
        r#"
        import basic_cli.cmd

        fn exit_code() -> I32 { 7 }

        fn main() -> () uses [Exit] {
            exit(exit_code())
        }
        "#,
    );

    let outcome = run_project_with_outcome(project.root(), "app.sp")
        .expect("basic-cli exit should surface as a structured project outcome");
    assert_eq!(outcome, sporec_driver::ProjectRunOutcome::Exited(7));
}

#[test]
fn run_project_rejects_unknown_package_platform_host_binding() {
    let project = TempProject::new("project-unknown-package-platform-runtime");
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "demo"
        type = "application"

        [project]
        platform = "custom-platform"
        default-entry = "app"

        [dependencies]
        custom-platform = { path = "vendor/custom-platform" }

        [entries.app]
        path = "app.sp"
        "#,
    );
    project.write(
        "vendor/custom-platform/spore.toml",
        r#"
        [package]
        name = "custom-platform"
        type = "platform"

        [platform]
        contract-module = "platform_contract"
        startup-contract = "main"
        adapter-function = "main_for_host"
        handles = ["NetConnect"]
        "#,
    );
    project.write(
        "vendor/custom-platform/src/platform_contract.sp",
        r#"
        pub fn main() -> () {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> ()) -> () {
            app_main()
            return
        }
        "#,
    );
    project.write("src/app.sp", "fn main() -> () { return }\n");

    let err = run_project(project.root(), "app.sp")
        .expect_err("unsupported package platforms should fail explicitly at runtime");
    assert!(
        err.contains("runtime host binding for package platform `custom-platform`"),
        "expected explicit package platform runtime error, got: {err}"
    );
}

#[test]
fn compile_project_rejects_missing_startup_against_platform_dependency_contract() {
    let project = TempProject::new("project-path-platform-missing-startup");
    project.write("spore.toml", APP_MANIFEST_WITH_BASIC_CLI);
    write_basic_cli_platform(
        &project,
        r#"
        pub fn main() -> () {
            ?platform_startup_contract
        }

        pub fn main_for_host(app_main: () -> ()) -> () {
            app_main()
            return
        }
        "#,
    );
    project.write(
        "src/app.sp",
        r#"
        fn boot() -> () {
            return
        }
        "#,
    );

    let err = compile_project(project.root(), "app.sp")
        .expect_err("missing contract startup should fail validation");
    assert!(
        err.contains("required startup function `main`"),
        "expected startup validation error, got: {err}"
    );
    assert!(
        err.contains("basic-cli"),
        "expected platform contract context in error, got: {err}"
    );
}

#[test]
fn check_project_returns_invalid_platform_contract_diagnostic_for_non_hole_startup() {
    let project = TempProject::new("project-path-platform-invalid-contract");
    project.write("spore.toml", APP_MANIFEST_WITH_BASIC_CLI);
    write_basic_cli_platform(
        &project,
        r#"
        pub fn main() -> () {
            return
        }

        pub fn main_for_host(app_main: () -> ()) -> () {
            app_main()
            return
        }
        "#,
    );
    project.write("src/app.sp", "fn main() -> () { return }\n");

    match check_project(project.root(), "app.sp") {
        CheckReport::Failure(CheckFailure::Diagnostics { diagnostics, .. }) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "invalid-platform-contract"),
                "expected invalid platform contract diagnostic, got: {diagnostics:?}"
            );
        }
        other => panic!("expected invalid platform contract diagnostic, got: {other:?}"),
    }
}

#[test]
fn compile_project_allows_non_entry_module_in_manifest_project() {
    let project = TempProject::new("project-non-entry-check");
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
        path = "main.sp"
        "#,
    );
    project.write("src/main.sp", "fn main() -> () { return }\n");
    project.write("src/lib/util.sp", "pub fn helper() -> I32 { 42 }\n");

    let output = compile_project(project.root(), "lib/util.sp")
        .expect("non-entry modules should still compile in project context");
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        output.warnings
    );
}

#[test]
fn run_project_rejects_non_entry_module_in_manifest_project() {
    let project = TempProject::new("project-non-entry-run");
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
        path = "main.sp"
        "#,
    );
    project.write("src/main.sp", "fn main() -> () { return }\n");
    project.write("src/lib/util.sp", "pub fn helper() -> I32 { 42 }\n");

    let err = run_project(project.root(), "lib/util.sp")
        .expect_err("non-entry module should not become runnable");
    assert!(
        err.contains("not runnable"),
        "expected non-runnable project module error, got: {err}"
    );
}

#[test]
fn run_project_rejects_non_runnable_legacy_package_entry() {
    let project = TempProject::new("legacy-package-run");
    project.write(
        "spore.toml",
        r#"
        [package]
        name = "demo"
        type = "package"
        "#,
    );
    project.write(
        "src/lib.sp",
        "pub fn add(a: I32, b: I32) -> I32 { a + b }\n",
    );

    let err = run_project(project.root(), "lib.sp")
        .expect_err("legacy package entry should not be runnable");
    assert!(
        err.contains("not runnable"),
        "expected non-runnable package error, got: {err}"
    );
}

// ── Cost enforcement tests ──────────────────────────────────────────

#[test]
fn cost_violation_emits_warning_not_error() {
    // A function that declares cost [2, 0, 0, 0] but calls expensive(cost=100) twice
    // should succeed (warnings are not errors) but include a K0101 warning.
    let output = compile(
        r#"
        fn expensive(x: I32) -> I32 cost [100, 0, 0, 0] { x + x }
        fn cheap(a: I32) -> I32 cost [2, 0, 0, 0] { expensive(expensive(a)) }
    "#,
    )
    .expect("cost violations should be warnings, not errors");
    assert!(
        output.warnings.iter().any(|w| w.contains("K0101")),
        "expected K0101 warning, got warnings: {:?}",
        output.warnings
    );
    assert!(
        output
            .warnings
            .iter()
            .any(|w| w.contains("actual cost [") && w.contains("declared cost [")),
        "expected vector-native warning text, got warnings: {:?}",
        output.warnings
    );
}

#[test]
fn no_cost_annotation_no_warning() {
    // A function with no cost annotation should produce no warnings.
    let output = compile("fn f(x: I32) -> I32 { x + x }").unwrap();
    assert!(
        output.warnings.is_empty(),
        "expected no warnings for unannotated function, got: {:?}",
        output.warnings
    );
}

#[test]
fn cost_within_budget_no_warning() {
    // A function whose inferred cost fits within the budget.
    let output = compile("fn f(x: I32) -> I32 cost [1000, 0, 0, 0] { x + x }").unwrap();
    assert!(
        output.warnings.is_empty(),
        "expected no warnings when within budget, got: {:?}",
        output.warnings
    );
}
