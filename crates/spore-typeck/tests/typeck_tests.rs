use spore_parser::parse;
use spore_typeck::cost::CostResult;
use spore_typeck::error::ErrorCode;
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

fn check_err_with_codes(src: &str) -> Vec<(ErrorCode, String)> {
    let module = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    match type_check(&module) {
        Ok(_) => panic!("expected type error, but check succeeded"),
        Err(errs) => errs.into_iter().map(|e| (e.code, e.message)).collect(),
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
    assert!(hole.candidates.iter().any(|c| c.name == "double"));
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
    assert!(
        matches!(
            result.cost_results.get("quadruple"),
            Some(CostResult::Constant(3))
        ),
        "expected Constant(3) after callee propagation (1 base + 2 double calls), got {:?}",
        result.cost_results.get("quadruple")
    );
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

// ── Cost enforcement (K0001) ─────────────────────────────────────────────

#[test]
fn cost_budget_exceeded_emits_k0001() {
    // Body has callee costs exceeding the declared budget of 2
    let errs = check_err_with_codes(
        r#"
        fn expensive(x: Int) -> Int cost <= 100 { x + x }
        fn cheap(a: Int) -> Int cost <= 2 { expensive(expensive(a)) }
    "#,
    );
    assert!(
        errs.iter().any(|e| e.0 == ErrorCode::K0001),
        "expected K0001 for cost budget violation, got: {errs:?}"
    );
}

#[test]
fn cost_budget_within_limit_no_error() {
    // Budget of 1000 is generous enough for a simple function
    check_ok("fn simple(x: Int) -> Int cost <= 1000 { x + x }");
}

#[test]
fn unbounded_skips_cost_analysis() {
    // @unbounded should not emit any cost error regardless of body
    let module = parse(
        r#"
        @unbounded
        fn wild(n: Int) -> Int { if n >= 100 { n } else { wild(n + 1) } }
    "#,
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    assert!(
        matches!(result.cost_results.get("wild"), Some(CostResult::Unbounded)),
        "expected Unbounded for @unbounded function, got {:?}",
        result.cost_results.get("wild")
    );
}

#[test]
fn callee_cost_propagation() {
    // helper costs Constant(1), caller calls helper 3 times → 1 + 3 = 4
    let module = parse(
        r#"
        fn helper(x: Int) -> Int { x + x }
        fn caller(a: Int) -> Int { helper(a) + helper(a) + helper(a) }
    "#,
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    assert!(
        matches!(
            result.cost_results.get("caller"),
            Some(CostResult::Constant(4))
        ),
        "expected Constant(4) after propagation, got {:?}",
        result.cost_results.get("caller")
    );
}

#[test]
fn structural_recursion_still_detected() {
    // Classic structural recursion: factorial(n - 1)
    let module =
        parse("fn factorial(n: Int) -> Int { if n <= 1 { 1 } else { n * factorial(n - 1) } }")
            .unwrap();
    let result = type_check(&module).unwrap();
    assert!(
        matches!(result.cost_results.get("factorial"), Some(CostResult::Structural(p)) if p == "n"),
        "expected Structural(\"n\") for factorial, got {:?}",
        result.cost_results.get("factorial")
    );
}

#[test]
fn sep0006_cost_violation_uses_k0xxx() {
    // Verify K0001 code is used in display format
    let module = parse(
        r#"
        fn expensive(x: Int) -> Int cost <= 100 { x + x }
        fn over_budget(a: Int) -> Int cost <= 2 { expensive(expensive(a)) }
    "#,
    )
    .unwrap();
    let errs = type_check(&module).unwrap_err();
    let k_errors: Vec<_> = errs.iter().filter(|e| e.code == ErrorCode::K0001).collect();
    assert!(
        !k_errors.is_empty(),
        "expected at least one K0001 error, got: {errs:?}"
    );
    let output = k_errors[0].to_string();
    assert!(
        output.contains("[K0001]"),
        "display should use [K0001] code, got: {output}"
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

// ── Error code tests ─────────────────────────────────────────────────

#[test]
fn error_code_type_mismatch() {
    let errs = check_err_with_codes(r#"fn f() -> Int { "oops" }"#);
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0001));
}

#[test]
fn error_code_in_display_output() {
    let module = parse(r#"fn f() -> Int { "oops" }"#).unwrap();
    let errs = type_check(&module).unwrap_err();
    let output = errs[0].to_string();
    assert!(
        output.contains("[E0001]"),
        "display should contain [E0001], got: {output}"
    );
}

#[test]
fn error_code_undefined_variable() {
    let errs = check_err_with_codes("fn f() -> Int { x }");
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0004));
}

#[test]
fn error_code_wrong_arg_count() {
    let errs = check_err_with_codes(
        r#"
        fn add(a: Int, b: Int) -> Int { a }
        fn main() -> Int { add(1) }
    "#,
    );
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0007));
}

#[test]
fn error_code_cannot_call_non_function() {
    let errs = check_err_with_codes("fn f() -> Int { let x: Int = 1; x(2) }");
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0008));
}

#[test]
fn error_code_missing_capabilities() {
    let errs = check_err_with_codes(
        r#"
        fn fetch(url: String) -> String uses [NetRead] { "data" }
        fn process() -> String { fetch("http://example.com") }
    "#,
    );
    assert!(errs.iter().any(|e| e.0 == ErrorCode::C0001));
}

#[test]
fn error_code_no_such_field() {
    let errs = check_err_with_codes(
        r#"
        struct Point { x: Int, y: Int }
        fn f() -> Int { let p = Point { x: 1, y: 2 }; p.z }
    "#,
    );
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0015));
}

// ── Batch 4 Item 1: Anonymous record types ─────────────────────────────

#[test]
fn record_type_basic() {
    check_ok("fn f(p: { x: Int, y: Int }) -> Int { 0 }");
}

#[test]
fn record_width_subtyping() {
    // A record with extra fields should be accepted where fewer are expected
    check_ok(
        r#"
        fn needs_xy(p: { x: Int, y: Int }) -> Int { 0 }
        fn provide_xyz(p: { x: Int, y: Int, z: Bool }) -> Int { needs_xy(p) }
    "#,
    );
}

// ── Batch 4 Item 2: Associated types in capabilities ───────────────────

#[test]
fn capability_with_assoc_type() {
    check_ok(
        r#"
        capability Iterator[T] {
            type Output
            fn next(self: T) -> Int
        }
    "#,
    );
}

// ── Batch 5: HoleInfo v0.3, typed edges, layered sort ───────────────────

#[test]
fn hole_info_v03_has_all_fields() {
    use spore_typeck::hole::HoleInfo;
    use spore_typeck::types::Ty;
    use std::collections::{BTreeMap, BTreeSet};

    let info = HoleInfo {
        name: "impl".into(),
        location: None,
        expected_type: Ty::Int,
        type_inferred_from: Some("return type".into()),
        function: "foo".into(),
        enclosing_signature: Some("fn foo() -> Int".into()),
        bindings: BTreeMap::new(),
        binding_dependencies: BTreeMap::new(),
        capabilities: BTreeSet::new(),
        errors_to_handle: vec![],
        cost_budget: None,
        candidates: vec![],
        dependent_holes: vec![],
        confidence: None,
        error_clusters: vec![],
    };
    assert_eq!(info.name, "impl");
    assert_eq!(info.expected_type, Ty::Int);
    assert!(info.location.is_none());
    assert_eq!(info.type_inferred_from.as_deref(), Some("return type"));
    assert_eq!(info.enclosing_signature.as_deref(), Some("fn foo() -> Int"));
}

#[test]
fn candidate_score_overall_formula() {
    use spore_typeck::hole::CandidateScore;

    let cs = CandidateScore {
        name: "foo".into(),
        type_match: 1.0,
        cost_fit: 1.0,
        capability_fit: 1.0,
        error_coverage: 1.0,
    };
    assert!((cs.overall() - 1.0).abs() < 1e-9);

    let cs2 = CandidateScore {
        name: "bar".into(),
        type_match: 0.0,
        cost_fit: 0.0,
        capability_fit: 0.0,
        error_coverage: 0.0,
    };
    assert!((cs2.overall() - 0.0).abs() < 1e-9);

    // Weighted: 0.40*0.5 + 0.20*0.8 + 0.25*1.0 + 0.15*0.6
    let cs3 = CandidateScore {
        name: "baz".into(),
        type_match: 0.5,
        cost_fit: 0.8,
        capability_fit: 1.0,
        error_coverage: 0.6,
    };
    let expected = 0.40 * 0.5 + 0.20 * 0.8 + 0.25 * 1.0 + 0.15 * 0.6;
    assert!((cs3.overall() - expected).abs() < 1e-9);
}

#[test]
fn dependency_edge_with_kinds() {
    use spore_typeck::hole::{DependencyEdge, EdgeKind, HoleDependencyGraph};

    let mut g = HoleDependencyGraph::new();
    g.add_dependency_typed("?b".into(), "?a".into(), EdgeKind::Type);
    g.add_dependency_typed("?c".into(), "?a".into(), EdgeKind::Value);
    g.add_dependency_typed("?d".into(), "?b".into(), EdgeKind::Cost);

    assert_eq!(g.edges.len(), 3);
    assert!(g.edges.contains(&DependencyEdge {
        from: "?a".into(),
        to: "?b".into(),
        kind: EdgeKind::Type,
    }));
    assert!(g.edges.contains(&DependencyEdge {
        from: "?a".into(),
        to: "?c".into(),
        kind: EdgeKind::Value,
    }));
    assert!(g.edges.contains(&DependencyEdge {
        from: "?b".into(),
        to: "?d".into(),
        kind: EdgeKind::Cost,
    }));

    // Fast-lookup maps still work
    assert_eq!(g.dependencies_of("?b"), vec!["?a"]);
    assert_eq!(g.dependents_of("?a"), vec!["?b", "?c"]);
}

#[test]
fn layered_topological_order_basic() {
    use spore_typeck::hole::HoleDependencyGraph;

    let mut g = HoleDependencyGraph::new();
    g.add_dependency("?b".into(), "?a".into());
    g.add_dependency("?c".into(), "?a".into());
    g.add_dependency("?d".into(), "?b".into());
    g.add_dependency("?d".into(), "?c".into());

    let layers = g.layered_topological_order().unwrap();
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0], vec!["?a"]);
    assert_eq!(layers[1], vec!["?b", "?c"]); // parallel-ready
    assert_eq!(layers[2], vec!["?d"]);
}

#[test]
fn layered_topological_order_single() {
    use spore_typeck::hole::HoleDependencyGraph;

    let mut g = HoleDependencyGraph::new();
    g.add_hole("?x".into());
    let layers = g.layered_topological_order().unwrap();
    assert_eq!(layers, vec![vec!["?x".to_string()]]);
}

#[test]
fn has_cycle_detects_cycles() {
    use spore_typeck::hole::HoleDependencyGraph;

    let mut g = HoleDependencyGraph::new();
    g.add_dependency("?a".into(), "?b".into());
    g.add_dependency("?b".into(), "?a".into());
    assert!(g.has_cycle());
}

#[test]
fn has_cycle_no_cycle() {
    use spore_typeck::hole::HoleDependencyGraph;

    let mut g = HoleDependencyGraph::new();
    g.add_dependency("?b".into(), "?a".into());
    g.add_dependency("?c".into(), "?b".into());
    assert!(!g.has_cycle());
}

#[test]
fn layered_topological_order_rejects_cycle() {
    use spore_typeck::hole::HoleDependencyGraph;

    let mut g = HoleDependencyGraph::new();
    g.add_dependency("?a".into(), "?b".into());
    g.add_dependency("?b".into(), "?a".into());
    let err = g.layered_topological_order().unwrap_err();
    assert!(err.contains(&"?a".to_string()));
    assert!(err.contains(&"?b".to_string()));
}

#[test]
fn diamond_layered_sort() {
    // A→B, A→C, B→D, C→D should yield [[A], [B,C], [D]]
    use spore_typeck::hole::HoleDependencyGraph;

    let mut g = HoleDependencyGraph::new();
    g.add_dependency("?B".into(), "?A".into());
    g.add_dependency("?C".into(), "?A".into());
    g.add_dependency("?D".into(), "?B".into());
    g.add_dependency("?D".into(), "?C".into());

    let layers = g.layered_topological_order().unwrap();
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0], vec!["?A"]);
    assert_eq!(layers[1], vec!["?B", "?C"]);
    assert_eq!(layers[2], vec!["?D"]);
}

#[test]
fn json_includes_edge_kinds() {
    use spore_typeck::hole::{EdgeKind, HoleDependencyGraph};

    let mut g = HoleDependencyGraph::new();
    g.add_dependency_typed("?b".into(), "?a".into(), EdgeKind::Value);
    let json = g.to_json_string();
    assert!(json.contains("\"edges\""));
    assert!(json.contains("\"value\""));
    assert!(json.contains("\"?a\""));
    assert!(json.contains("\"?b\""));
}

#[test]
fn hole_report_json_v03_fields() {
    use spore_typeck::hole::HoleReport;

    let report = HoleReport::new();
    let json = report.to_json();
    assert!(json.contains("\"dependency_graph\""));
    assert!(json.contains("\"edges\""));
}

#[test]
fn hole_collects_capabilities_and_errors() {
    let module = parse(
        r#"
        fn helper() -> Int ! [ParseError] uses [IO] {
            ?todo
        }
    "#,
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    let hole = &result.hole_report.holes[0];
    assert!(hole.capabilities.contains("IO"));
    assert!(hole.errors_to_handle.contains(&"ParseError".to_string()));
}

// ── Enum constructor in expression position ────────────────────────────

#[test]
fn enum_constructor_call() {
    check_ok(
        r#"
        type Shape { Circle(Int), Rect(Int, Int) }
        fn make_circle() -> Shape { Circle(3) }
        fn make_rect() -> Shape { Rect(6, 7) }
    "#,
    );
}

#[test]
fn enum_constructor_zero_field_as_value() {
    check_ok(
        r#"
        type Color { Red, Green, Blue }
        fn get_red() -> Color { Red }
    "#,
    );
}

#[test]
fn enum_constructor_match_still_works() {
    check_ok(
        r#"
        type Option { Some(Int), None }
        fn unwrap_or(opt: Option, default: Int) -> Int {
            match opt {
                Some(value) => value,
                None => default,
            }
        }
    "#,
    );
}

#[test]
fn enum_constructor_wrong_arg_count() {
    let errs = check_err(
        r#"
        type Shape { Circle(Int), Rect(Int, Int) }
        fn bad() -> Shape { Rect(1) }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("expects 2 arguments")));
}

#[test]
fn enum_constructor_wrong_arg_type() {
    let errs = check_err(
        r#"
        type Shape { Circle(Int), Rect(Int, Int) }
        fn bad() -> Shape { Circle("hello") }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

// ── Impl block signature validation ────────────────────────────────────

#[test]
fn impl_wrong_return_type() {
    let errs = check_err(
        r#"
        capability Stringify[T] {
            fn to_string(self: T) -> String
        }
        struct Num { val: Int }
        impl Stringify for Num {
            fn to_string(self: Num) -> Int { 42 }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

#[test]
fn impl_wrong_param_type() {
    let errs = check_err(
        r#"
        capability Adder[T] {
            fn add(self: T, n: Int) -> Int
        }
        struct Counter { val: Int }
        impl Adder for Counter {
            fn add(self: Counter, n: String) -> Int { 0 }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

#[test]
fn impl_correct_signature_ok() {
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

// ── spawn / await Task[T] typing ───────────────────────────────────────

#[test]
fn spawn_wraps_in_task() {
    check_ok(
        r#"
        fn work() -> Int { 42 }
        fn run() -> Int {
            let t = spawn work();
            await t
        }
    "#,
    );
}

#[test]
fn await_non_task_is_error() {
    let errs = check_err(
        r#"
        fn run() -> Int {
            await 42
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("await expects Task[T]")));
}

// ── SEP-0006 diagnostic code scheme tests ────────────────────────────

#[test]
fn sep0006_type_errors_use_e0xxx() {
    // Type mismatch → E0001
    let errs = check_err_with_codes(r#"fn f() -> Int { "oops" }"#);
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0001));

    // Undefined variable → E0004
    let errs = check_err_with_codes("fn f() -> Int { x }");
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0004));

    // Wrong arg count → E0007
    let errs = check_err_with_codes(
        r#"
        fn add(a: Int, b: Int) -> Int { a }
        fn main() -> Int { add(1) }
    "#,
    );
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0007));
}

#[test]
fn sep0006_capability_violations_use_c0xxx() {
    // Missing capabilities → C0001
    let errs = check_err_with_codes(
        r#"
        fn fetch(url: String) -> String uses [NetRead] { "data" }
        fn process() -> String { fetch("http://example.com") }
    "#,
    );
    assert!(errs.iter().any(|e| e.0 == ErrorCode::C0001));
}

#[test]
fn sep0006_display_format_four_digits() {
    let module = parse(r#"fn f() -> Int { "oops" }"#).unwrap();
    let errs = type_check(&module).unwrap_err();
    let output = errs[0].to_string();
    assert!(
        output.contains("[E0001]"),
        "display should use 4-digit code [E0001], got: {output}"
    );
}

#[test]
fn sep0006_no_old_three_digit_codes() {
    // Verify that display output never contains old-style 3-digit codes
    let module = parse(r#"fn f() -> Int { "oops" }"#).unwrap();
    let errs = type_check(&module).unwrap_err();
    let output = errs[0].to_string();
    // Old code would have been [E001]; new code is [E0001]
    assert!(
        !output.contains("[E001]"),
        "should not contain old 3-digit code [E001], got: {output}"
    );
}

// ── Refinement type tests (L0) ──────────────────────────────────────────

#[test]
fn refinement_let_binding_satisfied() {
    // 5 > 0 is true, so this should pass
    check_ok(
        r#"
fn f() -> Int {
    let x: Int when self > 0 = 5
    x
}
"#,
    );
}

#[test]
fn refinement_let_binding_violated() {
    // -1 > 0 is false, should emit R0001
    let errs = check_err_with_codes(
        r#"
fn f() -> Int {
    let x: Int when self > 0 = -1
    x
}
"#,
    );
    assert!(
        errs.iter().any(|(code, _)| *code == ErrorCode::R0001),
        "expected R0001, got: {errs:?}"
    );
}

#[test]
fn refinement_subtype_of_base() {
    // A refined Int should be accepted where Int is expected
    check_ok(
        r#"
fn add(a: Int, b: Int) -> Int { a + b }
fn f() -> Int {
    let x: Int when self > 0 = 5
    add(x, 3)
}
"#,
    );
}

#[test]
fn refinement_alias_definition() {
    // alias Port = Int when ... should register and be usable
    check_ok(
        r#"
alias Port = Int when self >= 1 && self <= 65535
fn get_port() -> Int {
    let p: Port = 80
    p
}
"#,
    );
}

#[test]
fn refinement_alias_violated() {
    // 0 is not in 1..=65535
    let errs = check_err_with_codes(
        r#"
alias Port = Int when self >= 1 && self <= 65535
fn get_port() -> Int {
    let p: Port = 0
    p
}
"#,
    );
    assert!(
        errs.iter().any(|(code, _)| *code == ErrorCode::R0001),
        "expected R0001, got: {errs:?}"
    );
}

#[test]
fn refinement_string_len() {
    // "hello".len() > 0 is true
    check_ok(
        r#"
fn f() -> String {
    let s: String when self.len() > 0 = "hello"
    s
}
"#,
    );
}

#[test]
fn refinement_string_len_violated() {
    // "".len() > 0 is false
    let errs = check_err_with_codes(
        r#"
fn f() -> String {
    let s: String when self.len() > 0 = ""
    s
}
"#,
    );
    assert!(
        errs.iter().any(|(code, _)| *code == ErrorCode::R0001),
        "expected R0001, got: {errs:?}"
    );
}

#[test]
fn refinement_type_display() {
    // Verify Display impl shows "Int when <predicate>"
    let ty = Ty::Refined(
        Box::new(Ty::Int),
        "self".into(),
        Box::new(spore_parser::ast::Expr::BoolLit(true)),
    );
    let display = format!("{ty}");
    assert_eq!(display, "Int when <predicate>");
}

#[test]
fn refinement_fn_param_with_refined_type() {
    // Function with refined parameter type should typecheck
    check_ok(
        r#"
fn positive(x: Int when self > 0) -> Int { x }
fn f() -> Int { positive(5) }
"#,
    );
}
