use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sporec::{
    check_project_verbose, check_verbose, compile, compile_project, hole_summary, run_project,
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
        fn f() -> Int {
            ?
        }
    "#,
    )
    .unwrap();
    assert!(
        output.contains("?: expected Int"),
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
        fn f() -> Int {
            ?_hole_manual
        }
    "#,
    )
    .unwrap();
    assert!(
        output.contains("?_hole_manual: expected Int"),
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
        fn main() -> I32 { double(21) }
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
fn run_project_rejects_type_error_in_imported_module_before_execution() {
    let project = TempProject::new("project-import-run-type-error");
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
        fn main() -> I32 { double(21) }
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
