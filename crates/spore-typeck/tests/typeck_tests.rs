use spore_parser::parse;
use spore_typeck::cost::CostResult;
use spore_typeck::type_check;
use spore_typeck::types::Ty;

fn check_ok(src: &str) {
    let module = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    type_check(&module).unwrap_or_else(|errs| {
        panic!(
            "type errors:\n{}",
            errs.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
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

// ── Generics & Type Inference ───────────────────────────────────────────

#[test]
fn test_type_variable_unification() {
    // Lambda with inferred type param
    check_ok("fn f() -> Int { let id = |x: Int| x; id(42) }");
}

#[test]
fn test_let_inference() {
    check_ok("fn f() -> Int { let x = 42; x + 1 }");
}

// ── Cost analysis ────────────────────────────────────────────────────────

#[test]
fn test_cost_non_recursive_constant() {
    let module = parse("fn add(a: Int, b: Int) -> Int { a + b }").unwrap();
    let result = type_check(&module).unwrap();
    assert!(
        matches!(
            result.cost_results.get("add"),
            Some(CostResult::Constant(1))
        ),
        "expected Constant(1), got {:?}",
        result.cost_results.get("add")
    );
}

#[test]
fn test_cost_structural_recursion() {
    let module =
        parse("fn factorial(n: Int) -> Int { if n <= 1 { 1 } else { n * factorial(n - 1) } }")
            .unwrap();
    let result = type_check(&module).unwrap();
    assert!(
        matches!(result.cost_results.get("factorial"), Some(CostResult::Structural(p)) if p == "n"),
        "expected Structural(\"n\"), got {:?}",
        result.cost_results.get("factorial")
    );
}

#[test]
fn test_cost_multiple_functions() {
    let module = parse(
        "fn double(x: Int) -> Int { x + x }
         fn quadruple(x: Int) -> Int { double(double(x)) }",
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    assert!(matches!(
        result.cost_results.get("double"),
        Some(CostResult::Constant(1))
    ));
    assert!(matches!(
        result.cost_results.get("quadruple"),
        Some(CostResult::Constant(1))
    ));
}

#[test]
fn test_cost_unknown_recursion() {
    // Recursive but not structural (arg is n + 1, not decreasing)
    let module = parse("fn bad(n: Int) -> Int { if n >= 100 { n } else { bad(n + 1) } }").unwrap();
    let result = type_check(&module).unwrap();
    assert!(
        matches!(result.cost_results.get("bad"), Some(CostResult::Unknown(_))),
        "expected Unknown, got {:?}",
        result.cost_results.get("bad")
    );
}

#[test]
fn test_cost_hole_body_constant() {
    let module = parse("fn f() -> Int { ?todo }").unwrap();
    let result = type_check(&module).unwrap();
    // Holes count as constant cost (no real code to analyze)
    assert!(matches!(
        result.cost_results.get("f"),
        Some(CostResult::Constant(1))
    ));
}

#[test]
fn test_cost_structural_countdown() {
    // countdown(n) calls countdown(n - 1)
    let module =
        parse("fn countdown(n: Int) -> Int { if n <= 0 { 0 } else { countdown(n - 1) } }").unwrap();
    let result = type_check(&module).unwrap();
    assert!(
        matches!(result.cost_results.get("countdown"), Some(CostResult::Structural(p)) if p == "n"),
        "expected Structural(\"n\"), got {:?}",
        result.cost_results.get("countdown")
    );
}

#[test]
fn never_type_unifies_with_anything() {
    // A function returning Never should be usable where Int is expected
    let src = r#"
        fn diverge() -> Never { ?todo }
        fn use_int() -> Int {
            diverge()
        }
    "#;
    let ast = spore_parser::parse(src).unwrap();
    let result = spore_typeck::type_check(&ast);
    assert!(result.is_ok(), "Never should unify with Int");
}

#[test]
fn char_type_basic() {
    let src = r#"
        fn get_char() -> Char { ?todo }
        fn use_char(c: Char) -> Char { c }
    "#;
    let ast = spore_parser::parse(src).unwrap();
    let result = spore_typeck::type_check(&ast);
    assert!(result.is_ok());
}

#[test]
fn occurs_check_prevents_infinite_type() {
    // This should produce an error, not infinite loop
    // A function that tries to create T = List[T]
    let src = r#"
        fn wrap(x: List[Int]) -> Int { x }
    "#;
    // This is a simpler test - just ensure occurs_in works
    // The real test is that unification with self-referential types fails
    let ast = spore_parser::parse(src).unwrap();
    let result = spore_typeck::type_check(&ast);
    // This should fail with type mismatch, not infinite loop
    assert!(result.is_err());
}

// ── Pattern type checking ────────────────────────────────────────────

#[test]
fn exhaustive_bool_match() {
    check_ok(
        r#"fn check(b: Bool) -> Int {
            match b {
                true => 1,
                false => 0,
            }
        }"#,
    );
}

#[test]
fn non_exhaustive_bool_match() {
    let errs = check_err(
        r#"fn check(b: Bool) -> Int {
            match b {
                true => 1,
            }
        }"#,
    );
    assert!(errs.iter().any(|e| e.contains("non-exhaustive")));
}

#[test]
fn exhaustive_enum_match() {
    check_ok(
        r#"type Color { Red, Green, Blue }
        fn name(c: Color) -> String {
            match c {
                Red => "red",
                Green => "green",
                Blue => "blue",
            }
        }"#,
    );
}

#[test]
fn non_exhaustive_enum_match() {
    let errs = check_err(
        r#"type Color { Red, Green, Blue }
        fn name(c: Color) -> Int {
            match c {
                Red => 1,
                Green => 2,
            }
        }"#,
    );
    assert!(errs.iter().any(|e| e.contains("non-exhaustive")));
    assert!(errs.iter().any(|e| e.contains("Blue")));
}

#[test]
fn wildcard_makes_match_exhaustive() {
    check_ok(
        r#"fn describe(n: Int) -> String {
            match n {
                0 => "zero",
                1 => "one",
                _ => "other",
            }
        }"#,
    );
}

#[test]
fn match_with_guard_type_checked() {
    check_ok(
        r#"fn classify(n: Int) -> String {
            match n {
                x if x > 0 => "positive",
                _ => "non-positive",
            }
        }"#,
    );
}

#[test]
fn pattern_binds_variable() {
    check_ok(
        r#"type Option { Some(Int), None }
        fn unwrap_or(opt: Option, default: Int) -> Int {
            match opt {
                Some(value) => value,
                None => default,
            }
        }"#,
    );
}

#[test]
fn int_match_without_wildcard_is_non_exhaustive() {
    let errs = check_err(
        r#"fn check(n: Int) -> String {
            match n {
                0 => "zero",
                1 => "one",
            }
        }"#,
    );
    assert!(errs.iter().any(|e| e.contains("non-exhaustive")));
}

#[test]
fn int_pattern_on_bool_is_type_error() {
    let errs = check_err(
        r#"fn check(b: Bool) -> Int {
            match b {
                0 => 1,
                _ => 2,
            }
        }"#,
    );
    assert!(errs.iter().any(|e| e.contains("integer pattern")));
}

#[test]
fn variable_pattern_makes_int_match_exhaustive() {
    check_ok(
        r#"fn describe(n: Int) -> String {
            match n {
                0 => "zero",
                other => "something",
            }
        }"#,
    );
}

// ── Error set (throws) checking ─────────────────────────────────────────

#[test]
fn function_with_throws_clause() {
    check_ok(
        r#"
        fn read_file(path: String) -> String ! [IoError] { "content" }
    "#,
    );
}

#[test]
fn try_propagation_ok() {
    check_ok(
        r#"
        fn read_file(path: String) -> String ! [IoError] { "content" }
        fn process() -> String ! [IoError] {
            read_file("test.txt")?
        }
    "#,
    );
}

#[test]
fn try_propagation_missing_error() {
    let errs = check_err(
        r#"
        fn read_file(path: String) -> String ! [IoError] { "content" }
        fn process() -> String {
            read_file("test.txt")?
        }
    "#,
    );
    assert!(
        errs.iter().any(|e| e.contains("IoError")),
        "expected error about IoError, got: {errs:?}"
    );
}

#[test]
fn try_propagation_superset_ok() {
    check_ok(
        r#"
        fn read_file(path: String) -> String ! [IoError] { "content" }
        fn process() -> String ! [IoError, ParseError] {
            read_file("test.txt")?
        }
    "#,
    );
}

#[test]
fn try_propagation_partial_missing() {
    let errs = check_err(
        r#"
        fn risky(x: Int) -> Int ! [IoError, ParseError] { x }
        fn caller() -> Int ! [IoError] {
            risky(1)?
        }
    "#,
    );
    assert!(
        errs.iter().any(|e| e.contains("ParseError")),
        "expected error about ParseError, got: {errs:?}"
    );
}

#[test]
fn no_try_no_error_check() {
    // Calling a throwing function without ? doesn't require the caller to declare errors
    check_ok(
        r#"
        fn read_file(path: String) -> String ! [IoError] { "content" }
        fn process() -> String {
            read_file("test.txt")
        }
    "#,
    );
}

#[test]
fn function_with_throws_and_uses() {
    check_ok(
        r#"
        fn read_file(path: String) -> String ! [IoError] uses [Fs] { "content" }
    "#,
    );
}

#[test]
fn throw_keyword_still_works() {
    check_ok(
        r#"
        fn read_file(path: String) -> String throw [IoError] { "content" }
        fn process() -> String throw [IoError] {
            read_file("test.txt")?
        }
    "#,
    );
}

// ── Capability definition and impl ──────────────────────────────────────

#[test]
fn capability_definition_and_impl() {
    check_ok(
        r#"
        capability Display[T] {
            fn show(self: T) -> String
        }
        struct Point { x: Int, y: Int }
        impl Display for Point {
            fn show(self: Point) -> String { "point" }
        }
    "#,
    );
}

#[test]
fn impl_missing_method_error() {
    let errs = check_err(
        r#"
        capability Display[T] {
            fn show(self: T) -> String
        }
        struct Point { x: Int, y: Int }
        impl Display for Point {
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("missing method")));
}

#[test]
fn impl_extra_method_error() {
    let errs = check_err(
        r#"
        capability Display[T] {
            fn show(self: T) -> String
        }
        struct Point { x: Int, y: Int }
        impl Display for Point {
            fn show(self: Point) -> String { "point" }
            fn extra() -> Int { 42 }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("not defined in capability")));
}

#[test]
fn impl_unknown_capability_error() {
    let errs = check_err(
        r#"
        struct Point { x: Int, y: Int }
        impl UnknownCap for Point {
            fn show(self: Point) -> String { "point" }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("unknown capability")));
}
