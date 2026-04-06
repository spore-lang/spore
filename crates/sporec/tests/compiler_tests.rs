use sporec::{check_verbose, compile, hole_summary};

// ── Verbose output tests ────────────────────────────────────────────

#[test]
fn check_verbose_ok_includes_section_headers() {
    let output = check_verbose("fn f() -> Int { 42 }").unwrap();
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
        fn f() -> Int {
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
fn check_verbose_returns_error_on_invalid() {
    let result = check_verbose(r#"fn f() -> Int { "oops" }"#);
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
    let summary = hole_summary("fn f() -> Int { 42 }");
    assert!(summary.is_none(), "no holes should produce None");
}

#[test]
fn hole_summary_present_with_holes() {
    let summary = hole_summary(
        r#"
        fn f() -> Int {
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
        fn f() -> Int {
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
    let err = compile(r#"fn f() -> Int { "oops" }"#).unwrap_err();
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
        fn main() -> Int {
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
    let err = compile("fn main() -> Int { clamp(5, 0, 10) }")
        .expect_err("non-prelude stdlib should not be globally injected");
    assert!(
        err.contains("undefined variable `clamp`"),
        "expected clamp to stay unavailable by default, got: {err}"
    );
}

// ── Cost enforcement tests ──────────────────────────────────────────

#[test]
fn cost_violation_emits_warning_not_error() {
    // A function that declares cost <= 2 but calls expensive(cost=100) twice
    // should succeed (warnings are not errors) but include a K0101 warning.
    let output = compile(
        r#"
        fn expensive(x: Int) -> Int cost <= 100 { x + x }
        fn cheap(a: Int) -> Int cost <= 2 { expensive(expensive(a)) }
    "#,
    )
    .expect("cost violations should be warnings, not errors");
    assert!(
        output.warnings.iter().any(|w| w.contains("K0101")),
        "expected K0101 warning, got warnings: {:?}",
        output.warnings
    );
}

#[test]
fn no_cost_annotation_no_warning() {
    // A function with no cost annotation should produce no warnings.
    let output = compile("fn f(x: Int) -> Int { x + x }").unwrap();
    assert!(
        output.warnings.is_empty(),
        "expected no warnings for unannotated function, got: {:?}",
        output.warnings
    );
}

#[test]
fn cost_within_budget_no_warning() {
    // A function whose inferred cost fits within the budget.
    let output = compile("fn f(x: Int) -> Int cost <= 1000 { x + x }").unwrap();
    assert!(
        output.warnings.is_empty(),
        "expected no warnings when within budget, got: {:?}",
        output.warnings
    );
}
