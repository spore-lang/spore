use sporec_codegen::{call, call_native, run, run_native, value::Value};
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
