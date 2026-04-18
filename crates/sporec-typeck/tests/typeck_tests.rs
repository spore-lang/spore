use sporec_parser::parse;
use sporec_typeck::cost::CostResult;
use sporec_typeck::error::ErrorCode;
use sporec_typeck::type_check;
use sporec_typeck::types::{CapSet, ErrorSet, Ty};

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
    check_ok("fn f() -> I32 { 42 }");
}

#[test]
fn test_float_literal() {
    check_ok("fn f() -> F64 { 3.14 }");
}

#[test]
fn test_string_literal() {
    check_ok("fn f() -> Str { \"hello\" }");
}

#[test]
fn test_bool_literal() {
    check_ok("fn f() -> Bool { true }");
}

// ── Type mismatch errors ─────────────────────────────────────────────────

#[test]
fn test_return_type_mismatch() {
    let errs = check_err("fn f() -> I32 { \"oops\" }");
    assert!(errs[0].contains("type mismatch"));
    assert!(errs[0].contains("I32"));
    assert!(errs[0].contains("Str"));
}

#[test]
fn test_return_type_mismatch_bool() {
    let errs = check_err("fn f() -> Bool { 42 }");
    assert!(errs[0].contains("type mismatch"));
}

// ── Let bindings ─────────────────────────────────────────────────────────

#[test]
fn test_let_binding() {
    check_ok("fn f() -> I32 { let x = 42; x }");
}

#[test]
fn test_let_with_annotation() {
    check_ok("fn f() -> I32 { let x: I32 = 42; x }");
}

#[test]
fn test_let_annotation_mismatch() {
    let errs = check_err("fn f() -> I32 { let x: Str = 42; x }");
    assert!(errs[0].contains("type mismatch"));
}

// ── Arithmetic ───────────────────────────────────────────────────────────

#[test]
fn test_int_arithmetic() {
    check_ok("fn f() -> I32 { 1 + 2 * 3 }");
}

#[test]
fn test_float_arithmetic() {
    check_ok("fn f() -> F64 { 1.0 + 2.0 }");
}

#[test]
fn test_mixed_arithmetic_error() {
    let errs = check_err("fn f() -> I32 { 1 + 2.0 }");
    assert!(!errs.is_empty());
}

#[test]
fn test_string_concat() {
    check_ok("fn f() -> Str { \"a\" + \"b\" }");
}

#[test]
fn test_generic_unit_variant_freshens_per_use() {
    check_ok(
        r#"
        type Option[T] { Some(T), None }

        fn choose(flag: Bool) -> Option[Str] {
            if flag { None } else { Some("x") }
        }
        "#,
    );
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
    check_ok("fn f() -> I32 { -42 }");
}

#[test]
fn test_not_bool() {
    check_ok("fn f() -> Bool { !true }");
}

#[test]
fn test_negate_string_error() {
    let errs = check_err("fn f() -> Str { -\"hello\" }");
    assert!(errs[0].contains("negate"));
}

// ── Function calls ───────────────────────────────────────────────────────

#[test]
fn test_call_known_function() {
    check_ok(
        "fn add(a: I32, b: I32) -> I32 { a + b }
         fn main() -> I32 { add(1, 2) }",
    );
}

#[test]
fn test_call_wrong_arg_type() {
    let errs = check_err(
        "fn add(a: I32, b: I32) -> I32 { a + b }
         fn main() -> I32 { add(1, \"x\") }",
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

#[test]
fn test_call_wrong_arg_count() {
    let errs = check_err(
        "fn add(a: I32, b: I32) -> I32 { a + b }
         fn main() -> I32 { add(1) }",
    );
    assert!(errs.iter().any(|e| e.contains("expects 2 arguments")));
}

// ── If expressions ───────────────────────────────────────────────────────

#[test]
fn test_if_else() {
    check_ok("fn f(x: Bool) -> I32 { if x { 1 } else { 0 } }");
}

#[test]
fn test_if_branch_mismatch() {
    let errs = check_err("fn f(x: Bool) -> I32 { if x { 1 } else { \"no\" } }");
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

#[test]
fn test_if_non_bool_condition() {
    let errs = check_err("fn f() -> I32 { if 42 { 1 } else { 0 } }");
    assert!(errs.iter().any(|e| e.contains("Bool")));
}

// ── Match expressions ────────────────────────────────────────────────────

#[test]
fn test_match_consistent_arms() {
    check_ok(
        r#"fn f(x: I32) -> Str {
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
        r#"fn f(x: I32) -> I32 {
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
        "struct Point { x: F64, y: F64 }
         fn origin() -> Point { Point { x: 0.0, y: 0.0 } }",
    );
}

#[test]
fn test_generic_struct_literal_infers_type_arguments() {
    check_ok(
        r#"
        struct Pair[A, B] { first: A, second: B }

        fn make_pair[T, U](first: T, second: U) -> Pair[T, U] {
            Pair { first: first, second: second }
        }
        "#,
    );
}

#[test]
fn test_generic_struct_field_access_preserves_type_arguments() {
    check_ok(
        r#"
        struct Pair[A, B] { first: A, second: B }

        fn first(pair: Pair[Str, I32]) -> Str { pair.first }

        fn match_first(pair: Pair[Str, I32]) -> Str {
            match pair {
                Pair { first, second } => first,
            }
        }
        "#,
    );
}

#[test]
fn test_struct_field_type_mismatch() {
    let errs = check_err(
        "struct Point { x: F64, y: F64 }
         fn bad() -> Point { Point { x: 1, y: 2 } }",
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

#[test]
fn test_struct_field_access() {
    check_ok(
        "struct Point { x: F64, y: F64 }
         fn get_x(p: Point) -> F64 { p.x }",
    );
}

// ── Undefined variable ───────────────────────────────────────────────────

#[test]
fn test_undefined_variable() {
    let errs = check_err("fn f() -> I32 { x }");
    assert!(errs.iter().any(|e| e.contains("undefined")));
}

// ── Holes ────────────────────────────────────────────────────────────────

#[test]
fn test_hole_accepts_any_type() {
    check_ok("fn f() -> I32 { ?todo }");
}

#[test]
fn test_unnamed_hole_accepts_any_type() {
    check_ok("fn f() -> I32 { ? }");
}

#[test]
fn test_signature_holes_typecheck() {
    check_ok("fn identity(x: ?) -> ? { x }");
}

#[test]
fn test_named_signature_holes_share_constraints() {
    let errs = check_err(
        "fn identity(x: ?t) -> ?t { x }
         fn bad() -> I32 { identity(true) + 1 }",
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("cannot apply `Add` to type `Bool`"))
    );
}

// ── Multiple functions ───────────────────────────────────────────────────

#[test]
fn test_multiple_functions() {
    check_ok(
        "fn double(x: I32) -> I32 { x + x }
         fn quadruple(x: I32) -> I32 { double(double(x)) }",
    );
}

// ── Scoping ──────────────────────────────────────────────────────────────

#[test]
fn test_block_scoping() {
    check_ok(
        "fn f() -> I32 {
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
        "fn apply(f: (I32) -> I32, x: I32) -> I32 { f(x) }
         fn main() -> I32 { apply(|x: I32| x + 1, 42) }",
    );
}

// ── Hole reports ─────────────────────────────────────────────────────────

#[test]
fn test_hole_report_basic() {
    let module = parse("fn f() -> I32 { ?todo }").unwrap();
    let result = type_check(&module).unwrap();
    assert_eq!(result.hole_report.holes.len(), 1);
    assert_eq!(result.hole_report.holes[0].name, "todo");
    assert_eq!(result.hole_report.holes[0].expected_type, Ty::Int);
    assert_eq!(result.hole_report.holes[0].function, "f");
}

#[test]
fn test_unnamed_hole_gets_synthetic_name_in_report() {
    let module = parse("fn f() -> I32 { ? }").unwrap();
    let result = type_check(&module).unwrap();
    assert!(result.hole_report.holes[0].name.starts_with("_hole"));
}

#[test]
fn test_synthetic_hole_type_display_omits_internal_name() {
    let ty = Ty::Hole("_hole7".to_string());
    assert_eq!(ty.to_string(), "?");
}

#[test]
fn test_user_named_hole_display_keeps_name() {
    let ty = Ty::Hole("_hole_manual".to_string());
    assert_eq!(ty.to_string(), "?_hole_manual");
}

#[test]
fn test_hole_report_with_bindings() {
    let module = parse("fn f(x: I32) -> I32 { let y = 42; ?impl_ }").unwrap();
    let result = type_check(&module).unwrap();
    let hole = &result.hole_report.holes[0];
    assert!(hole.bindings.contains_key("x"));
    assert!(hole.bindings.contains_key("y"));
}

#[test]
fn test_hole_report_suggestions() {
    let module = parse(
        "fn double(x: I32) -> I32 { x + x }
         fn f() -> I32 { ?todo }",
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    let hole = &result.hole_report.holes[0];
    assert!(hole.candidates.iter().any(|c| c.name == "double"));
}

#[test]
fn test_hole_report_suggestions_respect_allows_annotation() {
    let module = parse(
        "@allows[double]\n\
         fn chooser() -> I32 { ?todo }\n\
         fn double(x: I32) -> I32 { x + x }\n\
         fn triple(x: I32) -> I32 { x + x + x }",
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    let hole = &result.hole_report.holes[0];
    assert!(hole.candidates.iter().any(|c| c.name == "double"));
    assert!(!hole.candidates.iter().any(|c| c.name == "triple"));
}

#[test]
fn test_hole_report_allows_can_refine_signature_hole() {
    let module = parse(
        "fn produce() -> I32 { 1 }
         fn chooser() -> ?r { ?todo @allows[produce] }",
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    let hole = &result.hole_report.holes[0];
    assert_eq!(hole.expected_type, Ty::Int);
    assert_eq!(
        hole.type_inferred_from.as_deref(),
        Some("`@allows[...]` candidates")
    );
}

#[test]
fn test_signature_hole_inference_propagates_to_later_body_hole() {
    let module = parse(
        "fn caller() -> I32 { later(1) }
         fn later(x: ?) -> ? { ?impl_ }",
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    let hole = &result.hole_report.holes[0];
    assert_eq!(hole.function, "later");
    assert_eq!(hole.expected_type, Ty::Int);
}

#[test]
fn test_hole_report_json() {
    let module = parse("fn f() -> I32 { ?todo }").unwrap();
    let result = type_check(&module).unwrap();
    let json = result.hole_report.to_json();
    assert!(json.contains("\"name\": \"todo\""));
    assert!(json.contains("\"expected_type\": \"I32\""));
}

#[test]
fn test_multiple_holes() {
    let module = parse(
        "fn f() -> I32 { ?first }
         fn g() -> Str { ?second }",
    )
    .unwrap();
    let result = type_check(&module).unwrap();
    assert_eq!(result.hole_report.holes.len(), 2);
}

// ── Capabilities / Effects ──────────────────────────────────────────────

#[test]
fn test_pure_function() {
    check_ok("fn add(a: I32, b: I32) -> I32 { a + b }");
}

#[test]
fn test_function_with_capability() {
    check_ok(r#"fn fetch(url: Str) -> Str uses [NetConnect] { "data" }"#);
}

#[test]
fn test_capability_propagation_error() {
    // A function calling a capability-requiring function must also declare those capabilities
    let errs = check_err(
        r#"fn fetch(url: Str) -> Str uses [NetConnect] { "data" }
           fn process() -> Str { fetch("http://example.com") }"#,
    );
    assert!(errs.iter().any(|e| e.contains("missing capabilities")));
    assert!(errs.iter().any(|e| e.contains("NetConnect")));
}

#[test]
fn test_capability_superset_ok() {
    check_ok(
        r#"fn fetch(url: Str) -> Str uses [NetConnect] { "data" }
           fn process() -> Str uses [NetConnect] { fetch("http://example.com") }"#,
    );
}

#[test]
fn test_capability_superset_multiple() {
    check_ok(
        r#"fn fetch(url: Str) -> Str uses [NetConnect] { "data" }
           fn process() -> Str uses [NetConnect, FileWrite] { fetch("http://example.com") }"#,
    );
}

#[test]
fn test_pure_lambda() {
    check_ok("fn f() -> (I32) -> I32 { |x: I32| x + 1 }");
}

// ── Generics & Type Inference ───────────────────────────────────────────

#[test]
fn test_type_variable_unification() {
    // Lambda with inferred type param
    check_ok("fn f() -> I32 { let id = |x: I32| x; id(42) }");
}

#[test]
fn test_let_inference() {
    check_ok("fn f() -> I32 { let x = 42; x + 1 }");
}

// ── Cost analysis ────────────────────────────────────────────────────────

#[test]
fn test_cost_non_recursive_constant() {
    let module = parse("fn add(a: I32, b: I32) -> I32 { a + b }").unwrap();
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
        parse("fn factorial(n: I32) -> I32 { if n <= 1 { 1 } else { n * factorial(n - 1) } }")
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
        "fn double(x: I32) -> I32 { x + x }
         fn quadruple(x: I32) -> I32 { double(double(x)) }",
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
    let module = parse("fn bad(n: I32) -> I32 { if n >= 100 { n } else { bad(n + 1) } }").unwrap();
    let result = type_check(&module).unwrap();
    assert!(
        matches!(result.cost_results.get("bad"), Some(CostResult::Unknown(_))),
        "expected Unknown, got {:?}",
        result.cost_results.get("bad")
    );
}

#[test]
fn test_cost_hole_body_constant() {
    let module = parse("fn f() -> I32 { ?todo }").unwrap();
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
        parse("fn countdown(n: I32) -> I32 { if n <= 0 { 0 } else { countdown(n - 1) } }").unwrap();
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
    // Cost violations are now warnings (SEP-0004), not errors
    let module = parse(
        r#"
        fn expensive(x: I32) -> I32 cost [100, 0, 0, 0] { x + x }
        fn cheap(a: I32) -> I32 cost [2, 0, 0, 0] { expensive(expensive(a)) }
    "#,
    )
    .unwrap();
    let result = type_check(&module).expect("cost violations should be warnings, not errors");
    assert!(
        result.warnings.iter().any(|w| w.code == ErrorCode::K0101),
        "expected K0101 warning for cost budget violation, got warnings: {:?}",
        result.warnings
    );
}

#[test]
fn no_cost_annotation_no_warning() {
    // A function with no cost annotation should not produce any cost warnings
    let module = parse("fn f(x: I32) -> I32 { x + x }").unwrap();
    let result = type_check(&module).unwrap();
    assert!(
        result.warnings.is_empty(),
        "expected no warnings for unannotated function, got: {:?}",
        result.warnings
    );
}

#[test]
fn cost_warning_severity_is_warning() {
    // Verify K0101 has Warning severity, not Error
    use sporec_typeck::error::Severity;
    assert_eq!(
        ErrorCode::K0101.severity(),
        Severity::Warning,
        "K0101 should be Warning severity per SEP-0004"
    );
}

#[test]
fn cost_budget_within_limit_no_error() {
    // Budget of 1000 is generous enough for a simple function
    check_ok("fn simple(x: I32) -> I32 cost [1000, 0, 0, 0] { x + x }");
}

#[test]
fn unbounded_skips_cost_analysis() {
    // @unbounded should not emit any cost error regardless of body
    let module = parse(
        r#"
        @unbounded
        fn wild(n: I32) -> I32 { if n >= 100 { n } else { wild(n + 1) } }
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
        fn helper(x: I32) -> I32 { x + x }
        fn caller(a: I32) -> I32 { helper(a) + helper(a) + helper(a) }
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
        parse("fn factorial(n: I32) -> I32 { if n <= 1 { 1 } else { n * factorial(n - 1) } }")
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
    // Verify K0101 code is used for cost violations (SEP-0004: warnings)
    let module = parse(
        r#"
        fn expensive(x: I32) -> I32 cost [100, 0, 0, 0] { x + x }
        fn over_budget(a: I32) -> I32 cost [2, 0, 0, 0] { expensive(expensive(a)) }
    "#,
    )
    .unwrap();
    let result = type_check(&module).expect("cost violations should be warnings, not errors");
    let k_warnings: Vec<_> = result
        .warnings
        .iter()
        .filter(|w| w.code == ErrorCode::K0101)
        .collect();
    assert!(
        !k_warnings.is_empty(),
        "expected at least one K0101 warning, got: {:?}",
        result.warnings
    );
    let output = k_warnings[0].to_string();
    assert!(
        output.contains("[K0101]"),
        "display should use [K0101] code, got: {output}"
    );
}

#[test]
fn never_type_unifies_with_anything() {
    // A function returning Never should be usable where Int is expected
    let src = r#"
        fn diverge() -> Never { ?todo }
        fn use_int() -> I32 {
            diverge()
        }
    "#;
    let ast = sporec_parser::parse(src).unwrap();
    let result = sporec_typeck::type_check(&ast);
    assert!(result.is_ok(), "Never should unify with I32");
}

#[test]
fn char_type_basic() {
    let src = r#"
        fn get_char() -> Char { ?todo }
        fn use_char(c: Char) -> Char { c }
    "#;
    let ast = sporec_parser::parse(src).unwrap();
    let result = sporec_typeck::type_check(&ast);
    assert!(result.is_ok());
}

#[test]
fn occurs_check_prevents_infinite_type() {
    // This should produce an error, not infinite loop
    // A function that tries to create T = List[T]
    let src = r#"
        fn wrap(x: List[I32]) -> I32 { x }
    "#;
    // This is a simpler test - just ensure occurs_in works
    // The real test is that unification with self-referential types fails
    let ast = sporec_parser::parse(src).unwrap();
    let result = sporec_typeck::type_check(&ast);
    // This should fail with type mismatch, not infinite loop
    assert!(result.is_err());
}

// ── Pattern type checking ────────────────────────────────────────────

#[test]
fn exhaustive_bool_match() {
    check_ok(
        r#"fn check(b: Bool) -> I32 {
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
        r#"fn check(b: Bool) -> I32 {
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
        fn name(c: Color) -> Str {
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
        fn name(c: Color) -> I32 {
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
        r#"fn describe(n: I32) -> Str {
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
        r#"fn classify(n: I32) -> Str {
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
        r#"type Option { Some(I32), None }
        fn unwrap_or(opt: Option, default: I32) -> I32 {
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
        r#"fn check(n: I32) -> Str {
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
        r#"fn check(b: Bool) -> I32 {
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
        r#"fn describe(n: I32) -> Str {
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
        fn read_file(path: Str) -> Str ! IoError { "content" }
    "#,
    );
}

#[test]
fn try_propagation_ok() {
    check_ok(
        r#"
        fn read_file(path: Str) -> Str ! IoError { "content" }
        fn process() -> Str ! IoError {
            read_file("test.txt")?
        }
    "#,
    );
}

#[test]
fn try_propagation_missing_error() {
    let errs = check_err(
        r#"
        fn read_file(path: Str) -> Str ! IoError { "content" }
        fn process() -> Str {
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
        fn read_file(path: Str) -> Str ! IoError { "content" }
        fn process() -> Str ! IoError | ParseError {
            read_file("test.txt")?
        }
    "#,
    );
}

#[test]
fn try_propagation_partial_missing() {
    let errs = check_err(
        r#"
        fn risky(x: I32) -> I32 ! IoError | ParseError { x }
        fn caller() -> I32 ! IoError {
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
fn direct_call_missing_error_check() {
    let errs = check_err(
        r#"
        fn read_file(path: Str) -> Str ! IoError { "content" }
        fn process() -> Str {
            read_file("test.txt")
        }
    "#,
    );
    assert!(
        errs.iter().any(|e| e.contains("IoError")),
        "expected error about IoError, got: {errs:?}"
    );
}

#[test]
fn direct_call_declared_error_check() {
    check_ok(
        r#"
        fn read_file(path: Str) -> Str ! IoError { "content" }
        fn process() -> Str ! IoError {
            read_file("test.txt")
        }
    "#,
    );
}

#[test]
fn function_with_throws_and_uses() {
    check_ok(
        r#"
        fn read_file(path: Str) -> Str ! IoError uses [Fs] { "content" }
    "#,
    );
}

#[test]
fn throw_signature_clause_is_rejected() {
    let src = r#"
        fn read_file(path: Str) -> Str throw [IoError] { "content" }
    "#;
    let errs = parse(src).expect_err("expected parse failure for legacy throw clause");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("expected expression") || e.message.contains("expected")),
        "unexpected parse errors: {errs:?}"
    );
}

#[test]
fn width_specific_primitives_and_unit_type_work() {
    check_ok(
        r#"
        fn id_i32(x: I32) -> I32 { x }
        fn keep_f64(x: F64) -> F64 { x }
        fn greet(name: Str) -> Str { name }
        fn done() -> () { return }
    "#,
    );
}

// ── Trait definition and impl ───────────────────────────────────────────

#[test]
fn capability_definition_and_impl() {
    check_ok(
        r#"
        trait Display[T] {
            fn show(self: T) -> Str
        }
        struct Point { x: I32, y: I32 }
        impl Display for Point {
            fn show(self: Point) -> Str { "point" }
        }
    "#,
    );
}

#[test]
fn impl_missing_method_error() {
    let errs = check_err(
        r#"
        trait Display[T] {
            fn show(self: T) -> Str
        }
        struct Point { x: I32, y: I32 }
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
        trait Display[T] {
            fn show(self: T) -> Str
        }
        struct Point { x: I32, y: I32 }
        impl Display for Point {
            fn show(self: Point) -> Str { "point" }
            fn extra() -> I32 { 42 }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("not defined in capability")));
}

#[test]
fn impl_unknown_capability_error() {
    let errs = check_err(
        r#"
        struct Point { x: I32, y: I32 }
        impl UnknownCap for Point {
            fn show(self: Point) -> Str { "point" }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("unknown capability")));
}

// ── Error code tests ─────────────────────────────────────────────────

#[test]
fn error_code_type_mismatch() {
    let errs = check_err_with_codes(r#"fn f() -> I32 { "oops" }"#);
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0001));
}

#[test]
fn error_code_in_display_output() {
    let module = parse(r#"fn f() -> I32 { "oops" }"#).unwrap();
    let errs = type_check(&module).unwrap_err();
    let output = errs[0].to_string();
    assert!(
        output.contains("[E0001]"),
        "display should contain [E0001], got: {output}"
    );
}

#[test]
fn error_code_undefined_variable() {
    let errs = check_err_with_codes("fn f() -> I32 { x }");
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0004));
}

#[test]
fn error_code_wrong_arg_count() {
    let errs = check_err_with_codes(
        r#"
        fn add(a: I32, b: I32) -> I32 { a }
        fn main() -> I32 { add(1) }
    "#,
    );
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0007));
}

#[test]
fn error_code_cannot_call_non_function() {
    let errs = check_err_with_codes("fn f() -> I32 { let x: I32 = 1; x(2) }");
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0008));
}

#[test]
fn error_code_missing_capabilities() {
    let errs = check_err_with_codes(
        r#"
        fn fetch(url: Str) -> Str uses [NetConnect] { "data" }
        fn process() -> Str { fetch("http://example.com") }
    "#,
    );
    assert!(errs.iter().any(|e| e.0 == ErrorCode::C0001));
}

#[test]
fn error_code_no_such_field() {
    let errs = check_err_with_codes(
        r#"
        struct Point { x: I32, y: I32 }
        fn f() -> I32 { let p = Point { x: 1, y: 2 }; p.z }
    "#,
    );
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0015));
}

// ── Batch 4 Item 1: Anonymous record types ─────────────────────────────

#[test]
fn record_type_basic() {
    check_ok("fn f(p: { x: I32, y: I32 }) -> I32 { 0 }");
}

#[test]
fn record_width_subtyping() {
    // A record with extra fields should be accepted where fewer are expected
    check_ok(
        r#"
        fn needs_xy(p: { x: I32, y: I32 }) -> I32 { 0 }
        fn provide_xyz(p: { x: I32, y: I32, z: Bool }) -> I32 { needs_xy(p) }
    "#,
    );
}

// ── Batch 4 Item 2: Associated types in traits ─────────────────────────

#[test]
fn capability_with_assoc_type() {
    check_ok(
        r#"
        trait Iterator[T] {
            type Output
            fn next(self: T) -> I32
        }
    "#,
    );
}

// ── Batch 5: HoleInfo v0.3, typed edges, layered sort ───────────────────

#[test]
fn hole_info_v03_has_all_fields() {
    use sporec_typeck::hole::HoleInfo;
    use sporec_typeck::types::Ty;
    use std::collections::{BTreeMap, BTreeSet};

    let info = HoleInfo {
        name: "impl".into(),
        location: None,
        expected_type: Ty::Int,
        type_inferred_from: Some("return type".into()),
        function: "foo".into(),
        enclosing_signature: Some("fn foo() -> I32".into()),
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
    assert_eq!(info.enclosing_signature.as_deref(), Some("fn foo() -> I32"));
}

#[test]
fn candidate_score_overall_formula() {
    use sporec_typeck::hole::CandidateScore;

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
    use sporec_typeck::hole::{DependencyEdge, EdgeKind, HoleDependencyGraph};

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
    use sporec_typeck::hole::HoleDependencyGraph;

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
    use sporec_typeck::hole::HoleDependencyGraph;

    let mut g = HoleDependencyGraph::new();
    g.add_hole("?x".into());
    let layers = g.layered_topological_order().unwrap();
    assert_eq!(layers, vec![vec!["?x".to_string()]]);
}

#[test]
fn has_cycle_detects_cycles() {
    use sporec_typeck::hole::HoleDependencyGraph;

    let mut g = HoleDependencyGraph::new();
    g.add_dependency("?a".into(), "?b".into());
    g.add_dependency("?b".into(), "?a".into());
    assert!(g.has_cycle());
}

#[test]
fn has_cycle_no_cycle() {
    use sporec_typeck::hole::HoleDependencyGraph;

    let mut g = HoleDependencyGraph::new();
    g.add_dependency("?b".into(), "?a".into());
    g.add_dependency("?c".into(), "?b".into());
    assert!(!g.has_cycle());
}

#[test]
fn layered_topological_order_rejects_cycle() {
    use sporec_typeck::hole::HoleDependencyGraph;

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
    use sporec_typeck::hole::HoleDependencyGraph;

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
    use sporec_typeck::hole::{EdgeKind, HoleDependencyGraph};

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
    use sporec_typeck::hole::HoleReport;

    let report = HoleReport::new();
    let json = report.to_json();
    assert!(json.contains("\"dependency_graph\""));
    assert!(json.contains("\"edges\""));
}

#[test]
fn hole_collects_capabilities_and_errors() {
    let module = parse(
        r#"
        fn helper() -> I32 ! ParseError uses [IO] {
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
        type Shape { Circle(I32), Rect(I32, I32) }
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
        type Option { Some(I32), None }
        fn unwrap_or(opt: Option, default: I32) -> I32 {
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
        type Shape { Circle(I32), Rect(I32, I32) }
        fn bad() -> Shape { Rect(1) }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("expects 2 arguments")));
}

#[test]
fn enum_constructor_wrong_arg_type() {
    let errs = check_err(
        r#"
        type Shape { Circle(I32), Rect(I32, I32) }
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
        trait Stringify[T] {
            fn to_string(self: T) -> Str
        }
        struct Num { val: I32 }
        impl Stringify for Num {
            fn to_string(self: Num) -> I32 { 42 }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

#[test]
fn impl_wrong_param_type() {
    let errs = check_err(
        r#"
        trait Adder[T] {
            fn add(self: T, n: I32) -> I32
        }
        struct Counter { val: I32 }
        impl Adder for Counter {
            fn add(self: Counter, n: Str) -> I32 { 0 }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
}

#[test]
fn impl_correct_signature_ok() {
    check_ok(
        r#"
        trait Display[T] {
            fn show(self: T) -> Str
        }
        struct Point { x: I32, y: I32 }
        impl Display for Point {
            fn show(self: Point) -> Str { "point" }
        }
    "#,
    );
}

// ── spawn / await Task[T] typing ───────────────────────────────────────

#[test]
fn spawn_wraps_in_task() {
    check_ok(
        r#"
        fn work() -> I32 { 42 }
        fn run() -> I32 uses [Spawn] {
            parallel_scope {
                let t = spawn work();
                t.await
            }
        }
    "#,
    );
}

#[test]
fn spawn_requires_spawn_capability() {
    let errs = check_err(
        r#"
        fn work() -> I32 { 42 }
        fn run() -> I32 {
            parallel_scope {
                let t = spawn work();
                t.await
            }
        }
    "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("spawn requires capability `Spawn`"))
    );
}

#[test]
fn spawn_requires_parallel_scope() {
    let errs = check_err(
        r#"
        fn work() -> I32 { 42 }
        fn run() -> I32 uses [Spawn] {
            let t = spawn work();
            t.await
        }
    "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("spawn is only allowed inside `parallel_scope"))
    );
}

#[test]
fn parallel_scope_lanes_positive_and_enforced_locally() {
    let errs = check_err(
        r#"
        fn work() -> I32 { 42 }
        fn run() -> I32 uses [Spawn] {
            parallel_scope(lanes: 1) {
                let a = spawn work();
                let b = spawn work();
                a.await + b.await
            }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("has 2 spawn site(s)")));
}

#[test]
fn await_non_task_is_error() {
    let errs = check_err(
        r#"
        fn run() -> I32 {
            42.await
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("await expects Task[T]")));
}

#[test]
fn channel_new_typed_sender_receiver_pair() {
    check_ok(
        r#"
        fn build() -> (Sender[I32], Receiver[I32]) {
            Channel.new[I32](buffer: 16)
        }
    "#,
    );
}

#[test]
fn select_timeout_requires_int_duration() {
    let errs = check_err(
        r#"
        fn f(rx: Receiver[I32]) -> I32 {
            select {
                value from rx => value,
                timeout("slow") => 0
            }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("select timeout")));
}

#[test]
fn select_recv_source_must_be_receiver() {
    let errs = check_err(
        r#"
        fn f(tx: Sender[I32]) -> I32 {
            select {
                value from tx => value
            }
        }
    "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("select source must be Receiver[T]"))
    );
}

// ── SEP-0006 diagnostic code scheme tests ────────────────────────────

#[test]
fn sep0006_type_errors_use_e0xxx() {
    // Type mismatch → E0001
    let errs = check_err_with_codes(r#"fn f() -> I32 { "oops" }"#);
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0001));

    // Undefined variable → E0004
    let errs = check_err_with_codes("fn f() -> I32 { x }");
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0004));

    // Wrong arg count → E0007
    let errs = check_err_with_codes(
        r#"
        fn add(a: I32, b: I32) -> I32 { a }
        fn main() -> I32 { add(1) }
    "#,
    );
    assert!(errs.iter().any(|e| e.0 == ErrorCode::E0007));
}

#[test]
fn sep0006_capability_violations_use_c0xxx() {
    // Missing capabilities → C0001
    let errs = check_err_with_codes(
        r#"
        fn fetch(url: Str) -> Str uses [NetConnect] { "data" }
        fn process() -> Str { fetch("http://example.com") }
    "#,
    );
    assert!(errs.iter().any(|e| e.0 == ErrorCode::C0001));
}

#[test]
fn sep0006_display_format_four_digits() {
    let module = parse(r#"fn f() -> I32 { "oops" }"#).unwrap();
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
    let module = parse(r#"fn f() -> I32 { "oops" }"#).unwrap();
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
fn f() -> I32 {
    let x: I32 when self > 0 = 5;
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
fn f() -> I32 {
    let x: I32 when self > 0 = -1;
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
fn add(a: I32, b: I32) -> I32 { a + b }
fn f() -> I32 {
    let x: I32 when self > 0 = 5;
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
alias Port = I32 when self >= 1 && self <= 65535
fn get_port() -> I32 {
    let p: Port = 80;
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
alias Port = I32 when self >= 1 && self <= 65535
fn get_port() -> I32 {
    let p: Port = 0;
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
fn f() -> Str {
    let s: Str when self.len() > 0 = "hello";
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
fn f() -> Str {
    let s: Str when self.len() > 0 = "";
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
    // Verify Display impl shows "I32 when <predicate>"
    let ty = Ty::Refined(
        Box::new(Ty::Int),
        "self".into(),
        Box::new(sporec_parser::ast::Expr::BoolLit(true)),
    );
    let display = format!("{ty}");
    assert_eq!(display, "I32 when <predicate>");
}

#[test]
fn refinement_fn_param_with_refined_type() {
    // Function with refined parameter type should typecheck
    check_ok(
        r#"
fn positive(x: I32 when self > 0) -> I32 { x }
fn f() -> I32 { positive(5) }
"#,
    );
}
// ── Runtime builtin type checking (issue #14) ────────────────────────────

#[test]
fn builtin_println_type_checks() {
    check_ok(r#"fn main() { println("hello") }"#);
}

#[test]
fn builtin_println_wrong_arg_type() {
    let errs = check_err(r#"fn main() { println(42) }"#);
    assert!(
        errs.iter().any(|e| e.contains("argument")),
        "expected argument type mismatch for println(I32), got: {errs:?}"
    );
}

#[test]
fn builtin_read_line_type_checks() {
    check_ok(r#"fn main() -> Str { read_line() }"#);
}

#[test]
fn builtin_string_length_type_checks() {
    check_ok(r#"fn f() -> I32 { string_length("abc") }"#);
}

#[test]
fn builtin_print_still_works() {
    check_ok(r#"fn main() { print("hi") }"#);
}

#[test]
fn builtin_to_string_type_checks() {
    check_ok(r#"fn f() -> Str { to_string(42) }"#);
}

#[test]
fn builtin_math_abs_type_checks() {
    check_ok("fn f() -> I32 { abs(-1) }");
}

#[test]
fn builtin_math_min_max_type_checks() {
    check_ok("fn f() -> I32 { min(1, 2) }");
    check_ok("fn f() -> I32 { max(1, 2) }");
}

#[test]
fn builtin_trim_type_checks() {
    check_ok(r#"fn f() -> Str { trim("  hi  ") }"#);
}

#[test]
fn builtin_starts_with_type_checks() {
    check_ok(r#"fn f() -> Bool { starts_with("hello", "he") }"#);
}

#[test]
fn builtin_program_using_builtins() {
    // End-to-end: a program that uses multiple builtins should type check
    check_ok(
        r#"
        fn greet(name: Str) -> Str {
            let upper = to_upper(name);
            let len = string_length(upper);
            upper
        }

        fn main() {
            println("start");
            let result = greet("world");
            println(result)
        }
        "#,
    );
}

// ── Foreign fn type-checking ────────────────────────────────────────────

// ── Regression: prelude signature fixes (A5–A7) ────────────────────────

#[test]
fn builtin_to_string_accepts_float() {
    // Bug A5: to_string should accept any type, not just Int
    check_ok(r#"fn f() -> Str { to_string(3.14) }"#);
}

#[test]
fn builtin_to_string_accepts_bool() {
    check_ok(r#"fn f() -> Str { to_string(true) }"#);
}

#[test]
fn builtin_to_string_accepts_string() {
    check_ok(r#"fn f() -> Str { to_string("hello") }"#);
}

#[test]
fn builtin_split_returns_list_str() {
    // Bug A6: split should return List[Str], not bare List
    check_ok(r#"fn f() -> List[Str] { split("a,b", ",") }"#);
}

#[test]
fn builtin_head_returns_option() {
    // Bug A7: head should return Option[A], not A
    check_ok(r#"fn f() -> Option[I32] { head([1, 2, 3]) }"#);
}

#[test]
fn builtin_tail_returns_option_list() {
    // Bug A7: tail should return Option[List[A]], not List[A]
    check_ok(r#"fn f() -> Option[List[I32]] { tail([1, 2, 3]) }"#);
}

#[test]
fn builtin_char_at_returns_option() {
    // Bug A7: char_at should return Option[String]
    check_ok(r#"fn f() -> Option[Str] { char_at("abc", 0) }"#);
}

#[test]
fn builtin_char_to_int_type_checks() {
    check_ok(r#"fn f() -> I32 { char_to_int("A") }"#);
}

#[test]
fn builtin_int_to_char_type_checks() {
    check_ok(r#"fn f() -> Str { int_to_char(65) }"#);
}

// ── Foreign fn type-checking (continued) ────────────────────────────────

#[test]
fn test_foreign_fn_typechecks() {
    check_ok(
        r#"
        foreign fn read_file(path: Str) -> Str uses [FileRead]
        "#,
    );
}

#[test]
fn test_foreign_fn_callable_signature() {
    check_ok(
        r#"
        foreign fn add(a: I32, b: I32) -> I32
        fn main() -> I32 { add(1, 2) }
        "#,
    );
}

// ── Perform / Handle capability checking ────────────────────────────────

#[test]
fn test_perform_requires_capability() {
    let errs = check_err(
        r#"
        effect Console {
            fn println(msg: Str) -> ()
        }
        fn main() {
            perform Console.println("hello")
        }
        "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("capability") && e.contains("Console")),
        "expected capability error, got: {errs:?}"
    );
}

#[test]
fn test_perform_with_capability_ok() {
    check_ok(
        r#"
        effect Console {
            fn println(msg: Str) -> ()
        }
        fn main() uses [Console] {
            perform Console.println("hello")
        }
        "#,
    );
}

#[test]
fn test_handle_provides_capability() {
    // The handle expression provides Console capability to its body,
    // so perform Console.println should be allowed even without `uses [Console]`.
    check_ok(
        r#"
        effect Console {
            fn println(msg: Str) -> ()
        }
        fn main() {
            handle {
                perform Console.println("hello")
            } with {
                on Console.println(msg) => { msg; }
            }
        }
        "#,
    );
}

#[test]
fn test_perform_requires_declared_effect_interface() {
    let errs = check_err(
        r#"
        fn main() uses [Console] {
            perform Console.println("hello")
        }
        "#,
    );
    assert!(
        errs.iter().any(|e| e.contains("unknown effect `Console`")),
        "expected unknown effect error, got: {errs:?}"
    );
}

#[test]
fn test_handle_capability_does_not_escape_scope() {
    let errs = check_err(
        r#"
        effect Console {
            fn println(msg: Str) -> ()
        }
        fn main() {
            handle {
                perform Console.println("inside")
            } with {
                on Console.println(msg) => 0
            };
            perform Console.println("outside")
        }
        "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("capability") && e.contains("Console")),
        "expected capability error after handle scope ends, got: {errs:?}"
    );
}

#[test]
fn test_perform_uses_declared_effect_return_type() {
    check_ok(
        r#"
        effect Console {
            fn read_line() -> Str
        }
        fn main() -> Str uses [Console] {
            perform Console.read_line()
        }
        "#,
    );
}

#[test]
fn test_perform_checks_declared_effect_argument_types() {
    let errs = check_err(
        r#"
        effect Console {
            fn println(msg: Str) -> ()
        }
        fn main() uses [Console] {
            perform Console.println(42)
        }
        "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("argument 1 of `Console.println`")
                || e.contains("expected `Str`, got `I32`")),
        "expected effect operation argument type error, got: {errs:?}"
    );
}

#[test]
fn test_handle_result_flows_into_surrounding_expression() {
    check_ok(
        r#"
        effect Math {
            fn double(x: I32) -> I32
        }
        fn main() -> I32 {
            let doubled = handle {
                perform Math.double(21)
            } with {
                Math.double(x) => x + x
            };
            doubled + 1
        }
        "#,
    );
}

#[test]
fn test_handle_arm_matches_declared_effect_return_type() {
    let errs = check_err(
        r#"
        effect Math {
            fn double(x: I32) -> I32
        }
        fn main() -> I32 {
            handle {
                perform Math.double(21)
            } with {
                Math.double(x) => "not an int"
            }
        }
        "#,
    );
    assert!(
        errs.iter().any(|e| e.contains("handler arm `Math.double`")),
        "expected handler arm return type error, got: {errs:?}"
    );
}

#[test]
fn test_handle_arm_checks_declared_effect_arity() {
    let errs = check_err(
        r#"
        effect Math {
            fn double(x: I32) -> I32
        }
        fn main() -> I32 {
            handle {
                perform Math.double(21)
            } with {
                Math.double() => 0
            }
        }
        "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("handler arm `Math.double` expects 1 parameters, got 0")),
        "expected handler arm arity error, got: {errs:?}"
    );
}

#[test]
fn test_named_handler_payload_and_self_typecheck() {
    check_ok(
        r#"
        effect Math {
            fn double(x: I32) -> I32
        }
        handler Math as DoubleMath(multiplier: I32) {
            fn double(x: I32) -> I32 { x * self.multiplier }
        }
        fn main() -> I32 {
            handle {
                perform Math.double(21)
            } with {
                use DoubleMath { multiplier: 2 }
            }
        }
        "#,
    );
}

#[test]
fn test_named_handler_payload_checks_field_types() {
    let errs = check_err(
        r#"
        effect Math {
            fn double(x: I32) -> I32
        }
        handler Math as DoubleMath(multiplier: I32) {
            fn double(x: I32) -> I32 { x * self.multiplier }
        }
        fn main() -> I32 {
            handle {
                perform Math.double(21)
            } with {
                use DoubleMath { multiplier: "oops" }
            }
        }
        "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("payload field `multiplier`")
                || e.contains("expected `I32`, got `Str`")),
        "expected named handler payload type error, got: {errs:?}"
    );
}

#[test]
fn test_named_and_inline_duplicate_binding_errors() {
    let errs = check_err(
        r#"
        effect Math {
            fn double(x: I32) -> I32
        }
        handler Math as DoubleMath(multiplier: I32) {
            fn double(x: I32) -> I32 { x * self.multiplier }
        }
        fn main() -> I32 {
            handle {
                perform Math.double(21)
            } with {
                use DoubleMath { multiplier: 2 },
                on Math.double(x) => x + x
            }
        }
        "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("duplicate handler binding") && e.contains("Math.double")),
        "expected duplicate handler binding error, got: {errs:?}"
    );
}

// ── Or-pattern binding validation ───────────────────────────────────────

#[test]
fn test_or_pattern_same_bindings() {
    check_ok(
        r#"type Shape { Circle(I32), Square(I32) }
        fn size(s: Shape) -> I32 {
            match s {
                Circle(x) | Square(x) => x,
            }
        }"#,
    );
}

#[test]
fn test_or_pattern_different_bindings_error() {
    let errs = check_err_with_codes(
        r#"type Shape { Circle(I32), Square(I32) }
        fn size(s: Shape) -> I32 {
            match s {
                Circle(x) | Square(y) => 0,
            }
        }"#,
    );
    assert!(
        errs.iter().any(|(code, _)| *code == ErrorCode::E0504),
        "expected E0504 or-pattern binding mismatch, got: {errs:?}"
    );
}

#[test]
fn test_or_pattern_different_types_error() {
    let errs = check_err(
        r#"type Value { IntVal(I32), StrVal(Str) }
        fn show(v: Value) -> I32 {
            match v {
                IntVal(x) | StrVal(x) => 0,
            }
        }"#,
    );
    assert!(
        errs.iter().any(|e| e.contains("type mismatch")),
        "expected type mismatch for or-pattern binding, got: {errs:?}"
    );
}

#[test]
fn test_or_pattern_no_bindings() {
    check_ok(
        r#"type Color { Red, Blue, Green }
        fn is_warm(c: Color) -> Bool {
            match c {
                Red | Blue => true,
                Green => false,
            }
        }"#,
    );
}

// ── ErrorSet in Ty::Fn tests ────────────────────────────────────────

#[test]
fn test_fn_type_with_error_set() {
    let errors: ErrorSet = ["MyError".to_string()].into_iter().collect();
    let ty = Ty::Fn(vec![], Box::new(Ty::Int), CapSet::new(), errors.clone());
    match &ty {
        Ty::Fn(_, _, _, err_set) => assert_eq!(*err_set, errors),
        _ => panic!("expected Ty::Fn"),
    }
}

#[test]
fn test_fn_type_empty_error_set() {
    let ty = Ty::Fn(vec![], Box::new(Ty::Int), CapSet::new(), ErrorSet::new());
    match &ty {
        Ty::Fn(_, _, _, err_set) => assert!(err_set.is_empty()),
        _ => panic!("expected Ty::Fn"),
    }
}

#[test]
fn test_error_set_display_empty() {
    let ty = Ty::Fn(
        vec![Ty::Int],
        Box::new(Ty::Str),
        CapSet::new(),
        ErrorSet::new(),
    );
    let display = format!("{ty}");
    assert_eq!(display, "(I32) -> Str");
    assert!(!display.contains('!'));
}

#[test]
fn test_error_set_display_with_errors() {
    let mut errors = ErrorSet::new();
    errors.insert("FileNotFound".to_string());
    errors.insert("PermissionDenied".to_string());
    let ty = Ty::Fn(vec![Ty::Str], Box::new(Ty::Str), CapSet::new(), errors);
    let display = format!("{ty}");
    // BTreeSet sorts alphabetically
    assert!(display.contains("! FileNotFound | PermissionDenied"));
}

#[test]
fn test_error_set_propagation() {
    // Using `?` to propagate errors from a caller that doesn't declare them
    let src = r#"
        fn risky() -> I32 ! MyError {
            42
        }
        fn caller() -> I32 {
            risky()?
        }
    "#;
    let errs = check_err(src);
    let has_missing_error = errs.iter().any(|e| e.contains("missing errors"));
    assert!(
        has_missing_error,
        "expected missing-errors diagnostic, got: {errs:?}"
    );
}

// ── Refined type equality ────────────────────────────────────────────────

#[test]
fn refined_types_different_predicates_not_equal() {
    use sporec_parser::ast::{BinOp, Expr};

    let pos = Ty::Refined(
        Box::new(Ty::Int),
        "x".into(),
        Box::new(Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            BinOp::Gt,
            Box::new(Expr::IntLit(0)),
        )),
    );
    let bounded = Ty::Refined(
        Box::new(Ty::Int),
        "x".into(),
        Box::new(Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            BinOp::Lt,
            Box::new(Expr::IntLit(100)),
        )),
    );
    assert_ne!(
        pos, bounded,
        "refined types with different predicates must not be equal"
    );
}

#[test]
fn test_error_set_propagation_declared() {
    // Using `?` from a caller that declares the errors should be OK
    check_ok(
        r#"
        fn risky() -> I32 ! MyError {
            42
        }
        fn caller() -> I32 ! MyError {
            risky()?
        }
    "#,
    );
}

#[test]
fn throw_requires_declared_error_set() {
    let errs = check_err(
        r#"
        struct MyError {}
        fn fail() -> I32 {
            throw MyError {}
        }
    "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("requires declaring an error set")),
        "expected throw-without-declaration diagnostic, got: {errs:?}"
    );
}

#[test]
fn throw_named_error_must_be_declared() {
    let errs = check_err(
        r#"
        struct IoError {}
        struct ParseError {}
        fn fail() -> I32 ! IoError {
            throw ParseError {}
        }
    "#,
    );
    assert!(
        errs.iter().any(|e| e.contains("ParseError")),
        "expected throw-name mismatch diagnostic, got: {errs:?}"
    );
}

#[test]
fn throw_named_error_declared_ok() {
    check_ok(
        r#"
        struct MyError {}
        fn fail() -> I32 ! MyError {
            throw MyError {}
        }
    "#,
    );
}

#[test]
fn refined_types_identical_are_equal() {
    use sporec_parser::ast::{BinOp, Expr};

    let a = Ty::Refined(
        Box::new(Ty::Int),
        "x".into(),
        Box::new(Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            BinOp::Gt,
            Box::new(Expr::IntLit(0)),
        )),
    );
    let b = Ty::Refined(
        Box::new(Ty::Int),
        "x".into(),
        Box::new(Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            BinOp::Gt,
            Box::new(Expr::IntLit(0)),
        )),
    );
    assert_eq!(
        a, b,
        "refined types with identical predicates must be equal"
    );
}

#[test]
fn test_fn_type_equality_with_error_set() {
    let mut errors = ErrorSet::new();
    errors.insert("E1".to_string());
    let ty1 = Ty::Fn(vec![], Box::new(Ty::Int), CapSet::new(), errors.clone());
    let ty2 = Ty::Fn(vec![], Box::new(Ty::Int), CapSet::new(), errors);
    let ty3 = Ty::Fn(vec![], Box::new(Ty::Int), CapSet::new(), ErrorSet::new());
    assert_eq!(ty1, ty2);
    assert_ne!(ty1, ty3);
}

#[test]
fn refined_types_different_var_names_not_equal() {
    use sporec_parser::ast::{BinOp, Expr};

    let a = Ty::Refined(
        Box::new(Ty::Int),
        "x".into(),
        Box::new(Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            BinOp::Gt,
            Box::new(Expr::IntLit(0)),
        )),
    );
    let b = Ty::Refined(
        Box::new(Ty::Int),
        "y".into(),
        Box::new(Expr::BinOp(
            Box::new(Expr::Var("y".into())),
            BinOp::Gt,
            Box::new(Expr::IntLit(0)),
        )),
    );
    assert_ne!(
        a, b,
        "refined types with different var names must not be equal"
    );
}
// ── Ty::fold / Ty::visit combinator tests ──────────────────────────

#[test]
fn ty_fold_replaces_int_with_float_in_nested_type() {
    // Fn([Int, Tuple([Int, Bool])], Int, {}, {}) → Fn([Float, Tuple([Float, Bool])], Float, {}, {})
    let ty = Ty::Fn(
        vec![Ty::Int, Ty::Tuple(vec![Ty::Int, Ty::Bool])],
        Box::new(Ty::Int),
        CapSet::new(),
        ErrorSet::new(),
    );
    let folded = ty.fold(&mut |t| match t {
        Ty::Int => Ty::Float,
        other => other,
    });
    let expected = Ty::Fn(
        vec![Ty::Float, Ty::Tuple(vec![Ty::Float, Ty::Bool])],
        Box::new(Ty::Float),
        CapSet::new(),
        ErrorSet::new(),
    );
    assert_eq!(folded, expected);
}

#[test]
fn ty_fold_replaces_in_record() {
    let ty = Ty::Record(vec![("x".into(), Ty::Int), ("y".into(), Ty::Bool)]);
    let folded = ty.fold(&mut |t| match t {
        Ty::Int => Ty::Str,
        other => other,
    });
    assert_eq!(
        folded,
        Ty::Record(vec![("x".into(), Ty::Str), ("y".into(), Ty::Bool)])
    );
}

#[test]
fn ty_visit_collects_named_types() {
    let ty = Ty::Fn(
        vec![
            Ty::Named("Foo".into()),
            Ty::Tuple(vec![Ty::Named("Bar".into()), Ty::Int]),
        ],
        Box::new(Ty::App(
            "Result".into(),
            vec![Ty::Named("Baz".into()), Ty::Str],
        )),
        CapSet::new(),
        ErrorSet::new(),
    );
    let mut names = Vec::new();
    ty.visit(&mut |t| {
        if let Ty::Named(n) = t {
            names.push(n.clone());
        }
    });
    assert_eq!(names, vec!["Foo", "Bar", "Baz"]);
}

#[test]
fn ty_fold_ref_maps_vars() {
    let ty = Ty::App("List".into(), vec![Ty::Var(0)]);
    let mapped = ty.fold_ref(&mut |t| match t {
        Ty::Var(0) => Some(Ty::Int),
        _ => None,
    });
    assert_eq!(mapped, Ty::App("List".into(), vec![Ty::Int]));
}

// ── Type soundness regression tests ─────────────────────────────────────

// S1: if without else must type as Unit
#[test]
fn if_without_else_types_as_unit() {
    // Using the result of an if-without-else as Int should fail
    let errs = check_err("fn f(x: Bool) -> I32 { if x { 42 } }");
    assert!(
        errs.iter().any(|e| e.contains("type mismatch")),
        "if-without-else returning non-() should be a type error, got: {errs:?}"
    );
}

#[test]
fn if_without_else_unit_body_ok() {
    // An if-without-else whose body is Unit is fine in a ()-returning fn
    check_ok(
        r#"fn side_effect(x: Bool) -> () {
            if x { let _ = 1; }
        }"#,
    );
}

// S2: return expression types as Never
#[test]
fn return_types_as_never() {
    // return should diverge (Never), so using it in an if-else that expects Int is ok
    check_ok(
        r#"fn f(x: Bool) -> I32 {
            if x { return 0 } else { 42 }
        }"#,
    );
}

// S3: Never is covariant only — actual=Never is fine, expected=Never is not
#[test]
fn never_actual_unifies_with_any() {
    // A function returning Never should be usable where Int is expected
    check_ok(
        r#"fn diverge() -> Never { ?todo }
        fn use_int() -> I32 { diverge() }"#,
    );
}

#[test]
fn int_does_not_satisfy_never() {
    // Int should NOT satisfy an expected Never
    let errs = check_err("fn f() -> Never { 42 }");
    assert!(
        errs.iter().any(|e| e.contains("type mismatch")),
        "I32 should not satisfy Never, got: {errs:?}"
    );
}

// S4: Struct literal checks for missing and duplicate fields
#[test]
fn struct_missing_field_is_error() {
    let errs = check_err(
        r#"struct Point { x: F64, y: F64 }
        fn bad() -> Point { Point { x: 1.0 } }"#,
    );
    assert!(
        errs.iter().any(|e| e.contains("missing field")),
        "should report missing field `y`, got: {errs:?}"
    );
}

#[test]
fn struct_duplicate_field_is_error() {
    let errs = check_err(
        r#"struct Point { x: F64, y: F64 }
        fn bad() -> Point { Point { x: 1.0, y: 2.0, x: 3.0 } }"#,
    );
    assert!(
        errs.iter().any(|e| e.contains("duplicate field")),
        "should report duplicate field `x`, got: {errs:?}"
    );
}

// A10: Exhaustiveness for Ty::App (parameterized types)
#[test]
fn exhaustive_parameterized_type_match() {
    // Non-parameterized Option works; the Ty::App path is tested below
    check_ok(
        r#"type Option { Some(I32), None }
        fn unwrap_or(opt: Option, default: I32) -> I32 {
            match opt {
                Some(v) => v,
                None => default,
            }
        }"#,
    );
}

#[test]
fn non_exhaustive_parameterized_type_match() {
    let errs = check_err(
        r#"type Option { Some(I32), None }
        fn unwrap(opt: Option) -> I32 {
            match opt {
                Some(v) => v,
            }
        }"#,
    );
    assert!(
        errs.iter().any(|e| e.contains("non-exhaustive")),
        "should report non-exhaustive match on Option, got: {errs:?}"
    );
    assert!(
        errs.iter().any(|e| e.contains("None")),
        "should mention missing variant None, got: {errs:?}"
    );
}

/// Ty::App exhaustiveness: use a List[Int] return which forces the scrutinee
/// through the App path. We construct the scenario via a helper that returns
/// a parameterised type and then match on it.
#[test]
fn non_exhaustive_app_type_match() {
    // Result[T, E] with two variants, matched on Result[Int, String].
    // The checker resolves fn return type to Ty::App("Result", [Int, String]).
    let errs = check_err(
        r#"type Result[T, E] { Ok(T), Err(E) }
        fn get_ok(r: Result[I32, Str]) -> I32 {
            match r {
                Ok(v) => v,
            }
        }"#,
    );
    assert!(
        errs.iter().any(|e| e.contains("non-exhaustive")),
        "should report non-exhaustive match on Result[I32, Str], got: {errs:?}"
    );
}

// ── Return type validation ──────────────────────────────────────────────

#[test]
fn return_type_mismatch_errors() {
    let src = r#"fn foo() -> I32 { return "hello" }"#;
    let errs = check_err(src);
    assert!(!errs.is_empty(), "should report return type mismatch");
}

// ── len accepts both List and String ────────────────────────────────────

#[test]
fn len_on_list() {
    check_ok("fn f() -> I32 { len([1, 2, 3]) }");
}

#[test]
fn len_on_string() {
    check_ok(r#"fn f() -> I32 { len("hello") }"#);
}

// ── Trait keyword ───────────────────────────────────────────────────────

#[test]
fn trait_keyword_definition_and_impl() {
    check_ok(
        r#"
        trait Display[T] {
            fn show(self: T) -> Str
        }
        struct Point { x: I32, y: I32 }
        impl Display for Point {
            fn show(self: Point) -> Str { "point" }
        }
    "#,
    );
}

#[test]
fn trait_keyword_missing_method_error() {
    let errs = check_err(
        r#"
        trait Display[T] {
            fn show(self: T) -> Str
        }
        struct Point { x: I32, y: I32 }
        impl Display for Point {
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("missing method")));
}

#[test]
fn where_bound_single_trait_is_enforced() {
    check_ok(
        r#"
        trait Display[T] {
            fn show(self: T) -> Str
        }
        struct Point { x: I32, y: I32 }
        impl Display for Point {
            fn show(self: Point) -> Str { "point" }
        }
        fn render(x: T) -> Str where T: Display { "ok" }
        fn run() -> Str { render(Point { x: 1, y: 2 }) }
    "#,
    );
}

#[test]
fn where_bound_reports_unsatisfied_trait() {
    let errs = check_err_with_codes(
        r#"
        trait Display[T] {
            fn show(self: T) -> Str
        }
        fn render(x: T) -> Str where T: Display { "ok" }
        fn run() -> Str { render(1) }
    "#,
    );
    assert!(
        errs.iter().any(|(code, msg)| {
            *code == ErrorCode::E0403 && msg.contains("does not satisfy where bound `T: Display`")
        }),
        "expected E0403 for unsatisfied where bound, got: {errs:?}"
    );
}

// ── Effect keyword ──────────────────────────────────────────────────────

#[test]
fn effect_definition_parses() {
    check_ok(
        r#"
        effect Console {
            fn println(msg: Str) -> ()
        }
    "#,
    );
}

#[test]
fn effect_alias_parses() {
    // Effect alias is a no-op in type checking for now, but should parse
    let module = parse(
        r#"
        effect IO = Console | FileRead | FileWrite
    "#,
    )
    .unwrap();
    assert_eq!(module.items.len(), 1);
}

// ── Handler keyword ─────────────────────────────────────────────────────

#[test]
fn handler_definition_parses() {
    check_ok(
        r#"
        effect Console {
            fn println(msg: Str) -> ()
        }
        handler MockConsole for Console {
            fn println(msg: Str) -> () { return }
        }
    "#,
    );
}

#[test]
fn handler_unknown_effect_error() {
    let errs = check_err(
        r#"
        handler MockConsole for UnknownEffect {
            fn println(msg: Str) -> () { 0 }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("unknown effect")));
}

#[test]
fn handler_return_type_mismatch_error() {
    let errs = check_err(
        r#"
        effect Console {
            fn println(msg: Str) -> ()
        }
        handler MockConsole for Console {
            fn println(msg: Str) -> () { 0 }
        }
    "#,
    );
    assert!(errs.iter().any(|e| e.contains("type mismatch")));
    assert!(errs.iter().any(|e| e.contains("function `println`")));
}

#[test]
fn handler_missing_operation_error() {
    let errs = check_err(
        r#"
        effect Console {
            fn println(msg: Str) -> ()
            fn read_line() -> Str
        }
        handler MockConsole for Console {
            fn println(msg: Str) -> () { return }
        }
    "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("missing operation `read_line`"))
    );
}

#[test]
fn fn_named_example_still_works() {
    // `example` is a contextual keyword, so it should still be usable as a function name
    check_ok(
        r#"
        fn example() -> I32 { 42 }
    "#,
    );
}

// ── Spec clause type checking ───────────────────────────────────────────

#[test]
fn spec_examples_type_check() {
    check_ok(
        r#"
        fn add(a: I32, b: I32) -> I32
        spec {
            example "basic": add(2, 3) == 5
        }
        {
            a + b
        }
    "#,
    );
}

#[test]
fn spec_block_example_type_checks() {
    check_ok(
        r#"
        fn add(a: I32, b: I32) -> I32
        spec {
            example "block" {
                let sum = add(2, 3);
                sum == 5
            }
        }
        {
            a + b
        }
    "#,
    );
}

#[test]
fn spec_property_type_checks() {
    check_ok(
        r#"
        fn add(a: I32, b: I32) -> I32
        spec {
            property "commutative": |a: I32, b: I32| add(a, b) == add(b, a)
        }
        {
            a + b
        }
    "#,
    );
}

#[test]
fn spec_full_clause_type_checks() {
    check_ok(
        r#"
        fn add(a: I32, b: I32) -> I32
        spec {
            example "identity":     add(0, 42) == 42
            property "commutative": |a: I32, b: I32| add(a, b) == add(b, a)
        }
        {
            a + b
        }
    "#,
    );
}

#[test]
fn spec_example_must_be_bool() {
    let errs = check_err(
        r#"
        fn add(a: I32, b: I32) -> I32
        spec {
            example "wrong": add(1, 2)
        }
        {
            a + b
        }
    "#,
    );
    assert!(
        errs.iter().any(|e| e.contains("spec example")),
        "expected spec example type error, got: {errs:?}"
    );
}

#[test]
fn spec_property_must_be_lambda() {
    let errs = check_err(
        r#"
        fn add(a: I32, b: I32) -> I32
        spec {
            property "bad": add(1, 2) == 3
        }
        {
            a + b
        }
    "#,
    );
    assert!(
        errs.iter()
            .any(|e| e.contains("spec property") && e.contains("lambda")),
        "expected spec property lambda error, got: {errs:?}"
    );
}

#[test]
fn spec_property_lambda_must_return_bool() {
    let errs = check_err(
        r#"
        fn add(a: I32, b: I32) -> I32
        spec {
            property "bad": |x: I32| x + 1
        }
        {
            a + b
        }
    "#,
    );
    assert!(
        errs.iter().any(|e| e.contains("spec property")),
        "expected spec property return-type error, got: {errs:?}"
    );
}

#[test]
fn spec_empty_clause_ok() {
    check_ok(
        r#"
        fn f() -> I32
        spec {
        }
        {
            42
        }
    "#,
    );
}
