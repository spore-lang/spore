use sporec_codegen::{call, call_native, emit_native_object, run, run_native, value::Value};
use sporec_parser::parse;

fn parse_module(source: &str) -> sporec_parser::ast::Module {
    parse(source).unwrap_or_else(|error| panic!("parse error: {error:?}"))
}

fn assert_main_parity(source: &str) {
    let module = parse_module(source);
    let interpreted = run(&module).unwrap_or_else(|error| panic!("interpreter error: {error}"));
    let native = run_native(&module).unwrap_or_else(|error| panic!("native error: {error}"));
    assert_eq!(interpreted, native, "source:\n{source}");
}

fn assert_call_parity(source: &str, name: &str, args: Vec<Value>) {
    let module = parse_module(source);
    let interpreted = call(&module, name, args.clone())
        .unwrap_or_else(|error| panic!("interpreter error: {error}"));
    let native =
        call_native(&module, name, args).unwrap_or_else(|error| panic!("native error: {error}"));
    assert_eq!(interpreted, native, "source:\n{source}");
}

/// Expect both interpreter and native backend to return an Err with a message
/// containing `expected_fragment`.
fn assert_call_errors_with(source: &str, name: &str, args: Vec<Value>, expected_fragment: &str) {
    let module = parse_module(source);
    let interp_err = call(&module, name, args.clone()).expect_err("expected interpreter error");
    let native_err = call_native(&module, name, args).expect_err("expected native error");
    assert!(
        interp_err.to_string().contains(expected_fragment),
        "interpreter error {interp_err:?} did not contain {expected_fragment:?}"
    );
    assert!(
        native_err.to_string().contains(expected_fragment),
        "native error {native_err:?} did not contain {expected_fragment:?}"
    );
}

#[test]
fn native_backend_rejects_unknown_named_scalar_types() {
    for (source, type_name) in [
        ("fn main() -> Number { 42 }", "Number"),
        ("fn main() -> Decimal { 3.14 }", "Decimal"),
    ] {
        let module = parse_module(source);
        let err = run_native(&module).expect_err("unknown named scalar type should be unsupported");
        assert!(
            err.to_string()
                .contains(&format!("unsupported scalar type `{type_name}`")),
            "expected native backend to reject {type_name}, got {err:?}"
        );
    }
}

#[test]
fn native_backend_matches_interpreter_for_scalar_main_programs() {
    for source in [
        "fn main() -> I64 { 42 }",
        "fn main() -> Bool { 4 * 10 + 2 == 42 }",
        r#"
        fn choose(flag: Bool, left: I64, right: I64) -> I64 {
            if flag { left } else { right }
        }

        fn main() -> I64 {
            let base = 20;
            choose(base < 21, base + 22, 0)
        }
        "#,
        r#"
        fn noop() -> () { }

        fn main() -> () {
            noop();
        }
        "#,
        "fn main() -> () { () }",
        r#"
        fn gt_zero(n: I64) -> Bool { n > 0 }

        fn main() -> Bool {
            gt_zero(5) && !gt_zero(-1)
        }
        "#,
    ] {
        assert_main_parity(source);
    }
}

#[test]
fn native_backend_can_emit_object_artifacts_for_supported_scalar_modules() {
    let module = parse_module(
        r#"
        fn choose(flag: Bool) -> Bool {
            if flag { true } else { false }
        }

        fn main() -> Bool {
            choose(true)
        }
        "#,
    );
    let object = emit_native_object(&module).expect("supported scalar module should emit object");
    assert!(
        !object.is_empty(),
        "native object artifact should not be empty"
    );
}

#[test]
fn native_backend_matches_interpreter_for_direct_calls() {
    assert_call_parity(
        r#"
        fn add_then_scale(a: I64, b: I64, scale: I64) -> I64 {
            let sum = a + b;
            sum * scale
        }
        "#,
        "add_then_scale",
        vec![Value::Int(20), Value::Int(1), Value::Int(2)],
    );
    assert_call_parity(
        r#"
        fn choose(flag: Bool, x: I64, y: I64) -> I64 {
            if flag { x } else { y }
        }
        "#,
        "choose",
        vec![Value::Bool(true), Value::Int(42), Value::Int(0)],
    );
}

#[test]
fn native_backend_rejects_unsupported_aggregates_explicitly() {
    let module = parse_module(r#"fn main() -> Str { "hello" }"#);
    let error = run_native(&module).expect_err("strings should stay interpreter-only");
    assert!(
        error
            .to_string()
            .contains("unsupported native backend feature"),
        "expected explicit unsupported error, got: {error}"
    );
}

#[test]
fn native_backend_rejects_recursion_explicitly() {
    let module = parse_module(
        r#"
        fn countdown(n: I64) -> I64 {
            if n == 0 { 0 } else { countdown(n - 1) }
        }

        fn main() -> I64 { countdown(3) }
        "#,
    );
    let error = run_native(&module).expect_err("recursion should stay unsupported");
    assert!(
        error
            .to_string()
            .contains("recursive calls are not supported"),
        "expected recursive-call diagnostic, got: {error}"
    );
}

#[test]
fn native_backend_rejects_indirect_calls_explicitly() {
    let module = parse_module(
        r#"
        fn main() -> I64 {
            let f = |x: I64| x + 1;
            f(41)
        }
        "#,
    );
    let error = run_native(&module).expect_err("lambda calls should stay unsupported");
    assert!(
        error
            .to_string()
            .contains("unsupported native backend feature"),
        "expected explicit unsupported error, got: {error}"
    );
}

// ── Arithmetic error parity tests ────────────────────────────────────────────

const ADD_FN: &str = "fn add(a: I64, b: I64) -> I64 { a + b }";
const SUB_FN: &str = "fn sub(a: I64, b: I64) -> I64 { a - b }";
const MUL_FN: &str = "fn mul(a: I64, b: I64) -> I64 { a * b }";
const NEG_FN: &str = "fn neg(a: I64) -> I64 { -a }";
const DIV_FN: &str = "fn div(a: I64, b: I64) -> I64 { a / b }";
const MOD_FN: &str = "fn mod_(a: I64, b: I64) -> I64 { a % b }";

#[test]
fn native_add_overflow_returns_error_not_trap() {
    assert_call_errors_with(
        ADD_FN,
        "add",
        vec![Value::Int(i64::MAX), Value::Int(1)],
        "overflow",
    );
}

#[test]
fn native_sub_overflow_returns_error_not_trap() {
    assert_call_errors_with(
        SUB_FN,
        "sub",
        vec![Value::Int(i64::MIN), Value::Int(1)],
        "overflow",
    );
}

#[test]
fn native_mul_overflow_returns_error_not_trap() {
    assert_call_errors_with(
        MUL_FN,
        "mul",
        vec![Value::Int(i64::MAX), Value::Int(2)],
        "overflow",
    );
}

#[test]
fn native_neg_overflow_returns_error_not_trap() {
    assert_call_errors_with(NEG_FN, "neg", vec![Value::Int(i64::MIN)], "overflow");
}

#[test]
fn native_div_by_zero_returns_error_not_trap() {
    assert_call_errors_with(
        DIV_FN,
        "div",
        vec![Value::Int(42), Value::Int(0)],
        "division by zero",
    );
}

#[test]
fn native_div_i64_min_neg_one_returns_error_not_trap() {
    // i64::MIN / -1 overflows; must not crash the process.
    let module = parse_module(DIV_FN);
    let native_err = call_native(&module, "div", vec![Value::Int(i64::MIN), Value::Int(-1)])
        .expect_err("expected native error for i64::MIN / -1");
    assert!(
        native_err.to_string().contains("overflow"),
        "expected overflow error, got: {native_err}"
    );
}

#[test]
fn native_mod_by_zero_returns_error_not_trap() {
    assert_call_errors_with(
        MOD_FN,
        "mod_",
        vec![Value::Int(42), Value::Int(0)],
        "modulo by zero",
    );
}

#[test]
fn native_normal_arithmetic_still_correct() {
    // Verify non-error arithmetic is unaffected by the guards.
    for (src, name, args) in [
        (ADD_FN, "add", vec![Value::Int(10), Value::Int(20)]),
        (SUB_FN, "sub", vec![Value::Int(50), Value::Int(8)]),
        (MUL_FN, "mul", vec![Value::Int(6), Value::Int(7)]),
        (NEG_FN, "neg", vec![Value::Int(99)]),
        (DIV_FN, "div", vec![Value::Int(100), Value::Int(4)]),
        (MOD_FN, "mod_", vec![Value::Int(17), Value::Int(5)]),
    ] {
        assert_call_parity(src, name, args);
    }
}
