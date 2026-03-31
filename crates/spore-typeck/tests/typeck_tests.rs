use spore_parser::parse;
use spore_typeck::type_check;
use spore_typeck::types::Ty;

fn check_ok(src: &str) {
    let module = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    type_check(&module).unwrap_or_else(|errs| {
        panic!(
            "type errors:\n{}",
            errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
        )
    });
}

fn check_err(src: &str) -> Vec<String> {
    let module = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    match type_check(&module) {
        Ok(_) => panic!("expected type error, but check succeeded"),
        Err(errs) => errs.into_iter().map(|e| e.message).collect(),
    }
}

// ── Literal type inference ───────────────────────────────────────────────

#[test]
fn test_int_literal() {
    check_ok("fn f() -> Int { 42 }");
}

#[test]
fn test_float_literal() {
    check_ok("fn f() -> Float { 3.14 }");
}

#[test]
fn test_string_literal() {
    check_ok("fn f() -> String { \"hello\" }");
}

#[test]
fn test_bool_literal() {
    check_ok("fn f() -> Bool { true }");
}

// ── Type mismatch errors ─────────────────────────────────────────────────

#[test]
fn test_return_type_mismatch() {
    let errs = check_err("fn f() -> Int { \"oops\" }");
    assert!(errs[0].contains("type mismatch"));
    assert!(errs[0].contains("Int"));
    assert!(errs[0].contains("String"));
}

#[test]
fn test_return_type_mismatch_bool() {
    let errs = check_err("fn f() -> Bool { 42 }");
    assert!(errs[0].contains("type mismatch"));
}

// ── Let bindings ─────────────────────────────────────────────────────────

#[test]
fn test_let_binding() {
    check_ok("fn f() -> Int { let x = 42; x }");
}

#[test]
fn test_let_with_annotation() {
    check_ok("fn f() -> Int { let x: Int = 42; x }");
}

#[test]
fn test_let_annotation_mismatch() {
    let errs = check_err("fn f() -> Int { let x: String = 42; x }");
    assert!(errs[0].contains("type mismatch"));
}

// ── Arithmetic ───────────────────────────────────────────────────────────

#[test]
fn test_int_arithmetic() {
    check_ok("fn f() -> Int { 1 + 2 * 3 }");
}

#[test]
fn test_float_arithmetic() {
    check_ok("fn f() -> Float { 1.0 + 2.0 }");
}

#[test]
fn test_mixed_arithmetic_error() {
    let errs = check_err("fn f() -> Int { 1 + 2.0 }");
    assert!(!errs.is_empty());
}

#[test]
fn test_string_concat() {
    check_ok("fn f() -> String { \"a\" + \"b\" }");
}

// ── Comparisons ──────────────────────────────────────────────────────────

#[test]
fn test_comparison_returns_bool() {
    check_ok("fn f() -> Bool { 1 < 2 }");
}

#[test]
fn test_equality_returns_bool() {
    check_ok("fn f() -> Bool { 1 == 2 }");
}

// ── Logical operators ────────────────────────────────────────────────────

#[test]
fn test_logical_and() {
    check_ok("fn f() -> Bool { true && false }");
}

#[test]
fn test_logical_on_non_bool() {
    let errs = check_err("fn f() -> Bool { 1 && 2 }");
    assert!(!errs.is_empty());
}

// ── Unary operators ──────────────────────────────────────────────────────

#[test]
fn test_negate_int() {
    check_ok("fn f() -> Int { -42 }");
}

#[test]
fn test_not_bool() {
    check_ok("fn f() -> Bool { !true }");
}

#[test]
fn test_negate_string_error() {
    let errs = check_err("fn f() -> String { -\"hello\" }");
    assert!(errs[0].contains("negate"));
}

// ── Function calls ───────────────────────────────────────────────────────

#[test]
fn test_call_known_function() {
    check_ok(
        "fn add(a: Int, b: Int) -> Int { a + b }
         fn main() -> Int { add(1, 2) }",
    );
}

#[test]
fn test_call_wrong_arg_type() {
    let errs = check_err(
        "fn add(a: Int, b: Int) -> Int { a + b }
         fn main() -> Int { add(1, \"x\") }",
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

#[test]
fn test_call_wrong_arg_count() {
    let errs = check_err(
        "fn add(a: Int, b: Int) -> Int { a + b }
         fn main() -> Int { add(1) }",
    );
    assert!(errs.iter().any(|e| e.contains("expects 2 arguments")));
}

// ── If expressions ───────────────────────────────────────────────────────

#[test]
fn test_if_else() {
    check_ok("fn f(x: Bool) -> Int { if x { 1 } else { 0 } }");
}

#[test]
fn test_if_branch_mismatch() {
    let errs = check_err("fn f(x: Bool) -> Int { if x { 1 } else { \"no\" } }");
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

#[test]
fn test_if_non_bool_condition() {
    let errs = check_err("fn f() -> Int { if 42 { 1 } else { 0 } }");
    assert!(errs.iter().any(|e| e.contains("Bool")));
}

// ── Match expressions ────────────────────────────────────────────────────

#[test]
fn test_match_consistent_arms() {
    check_ok(
        r#"fn f(x: Int) -> String {
            match x {
                0 => "zero",
                _ => "other"
            }
        }"#,
    );
}

#[test]
fn test_match_inconsistent_arms() {
    let errs = check_err(
        r#"fn f(x: Int) -> Int {
            match x {
                0 => 1,
                _ => "other"
            }
        }"#,
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

// ── Struct definitions and literals ──────────────────────────────────────

#[test]
fn test_struct_literal() {
    check_ok(
        "struct Point { x: Float, y: Float }
         fn origin() -> Point { Point { x: 0.0, y: 0.0 } }",
    );
}

#[test]
fn test_struct_field_type_mismatch() {
    let errs = check_err(
        "struct Point { x: Float, y: Float }
         fn bad() -> Point { Point { x: 1, y: 2 } }",
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

#[test]
fn test_struct_field_access() {
    check_ok(
        "struct Point { x: Float, y: Float }
         fn get_x(p: Point) -> Float { p.x }",
    );
}

// ── Undefined variable ───────────────────────────────────────────────────

#[test]
fn test_undefined_variable() {
    let errs = check_err("fn f() -> Int { x }");
    assert!(errs.iter().any(|e| e.contains("undefined")));
}

// ── Holes ────────────────────────────────────────────────────────────────

#[test]
fn test_hole_accepts_any_type() {
    check_ok("fn f() -> Int { ?todo }");
}

// ── Multiple functions ───────────────────────────────────────────────────

#[test]
fn test_multiple_functions() {
    check_ok(
        "fn double(x: Int) -> Int { x + x }
         fn quadruple(x: Int) -> Int { double(double(x)) }",
    );
}

// ── Scoping ──────────────────────────────────────────────────────────────

#[test]
fn test_block_scoping() {
    check_ok(
        "fn f() -> Int {
            let x = 1;
            let y = 2;
            x + y
        }",
    );
}

// ── Lambda ───────────────────────────────────────────────────────────────

#[test]
fn test_lambda_type() {
    check_ok(
        "fn apply(f: (Int) -> Int, x: Int) -> Int { f(x) }
         fn main() -> Int { apply(|x: Int| x + 1, 42) }",
    );
}

// ── Hole reports ─────────────────────────────────────────────────────────

#[test]
fn test_hole_report_basic() {
    let module = parse("fn f() -> Int { ?todo }").unwrap();
    let result = type_check(&module).unwrap();
    assert_eq!(result.hole_report.holes.len(), 1);
    assert_eq!(result.hole_report.holes[0].name, "todo");
    assert_eq!(result.hole_report.holes[0].expected_type, Ty::Int);
    assert_eq!(result.hole_report.holes[0].function, "f");
}

#[test]
fn test_hole_report_with_bindings() {
    let module = parse("fn f(x: Int) -> Int { let y = 42; ?impl_ }").unwrap();
    let result = type_check(&module).unwrap();
    let hole = &result.hole_report.holes[0];
    assert!(hole.bindings.contains_key("x"));
    assert!(hole.bindings.contains_key("y"));
}

#[test]
fn test_hole_report_suggestions() {
    let module = parse(
        "fn double(x: Int) -> Int { x + x }
         fn f() -> Int { ?todo }",
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    let hole = &result.hole_report.holes[0];
    assert!(hole.suggestions.contains(&"double".to_string()));
}

#[test]
fn test_hole_report_json() {
    let module = parse("fn f() -> Int { ?todo }").unwrap();
    let result = type_check(&module).unwrap();
    let json = result.hole_report.to_json();
    assert!(json.contains("\"name\": \"todo\""));
    assert!(json.contains("\"expected_type\": \"Int\""));
}

#[test]
fn test_multiple_holes() {
    let module = parse(
        "fn f() -> Int { ?first }
         fn g() -> String { ?second }",
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    assert_eq!(result.hole_report.holes.len(), 2);
}

// ── Capabilities / Effects ──────────────────────────────────────────────

#[test]
fn test_pure_function() {
    check_ok("fn add(a: Int, b: Int) -> Int { a + b }");
}

#[test]
fn test_function_with_capability() {
    check_ok(r#"fn fetch(url: String) -> String uses [NetRead] { "data" }"#);
}

#[test]
fn test_capability_propagation_error() {
    // A function calling a capability-requiring function must also declare those capabilities
    let errs = check_err(
        r#"fn fetch(url: String) -> String uses [NetRead] { "data" }
           fn process() -> String { fetch("http://example.com") }"#,
    );
    assert!(errs.iter().any(|e| e.contains("missing capabilities")));
    assert!(errs.iter().any(|e| e.contains("NetRead")));
}

#[test]
fn test_capability_superset_ok() {
    check_ok(
        r#"fn fetch(url: String) -> String uses [NetRead] { "data" }
           fn process() -> String uses [NetRead] { fetch("http://example.com") }"#,
    );
}

#[test]
fn test_capability_superset_multiple() {
    check_ok(
        r#"fn fetch(url: String) -> String uses [NetRead] { "data" }
           fn process() -> String uses [NetRead, FileWrite] { fetch("http://example.com") }"#,
    );
}

#[test]
fn test_pure_lambda() {
    check_ok("fn f() -> (Int) -> Int { |x: Int| x + 1 }");
}
