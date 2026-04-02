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
