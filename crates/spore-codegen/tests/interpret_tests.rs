use spore_codegen::value::Value;
use spore_parser::parse;

fn run_main(src: &str) -> Value {
    let module = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    spore_codegen::run(&module).unwrap_or_else(|e| panic!("runtime error: {e}"))
}

fn run_fn(src: &str, name: &str, args: Vec<Value>) -> Value {
    let module = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    spore_codegen::call(&module, name, args).unwrap_or_else(|e| panic!("runtime error: {e}"))
}

// ── Literals ─────────────────────────────────────────────────────────────

#[test]
fn test_int_literal() {
    let v = run_main("fn main() -> Int { 42 }");
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_float_literal() {
    let v = run_main("fn main() -> Float { 3.14 }");
    assert_eq!(v.as_float(), Some(3.14));
}

#[test]
fn test_string_literal() {
    let v = run_main("fn main() -> String { \"hello\" }");
    assert_eq!(v.as_str(), Some("hello"));
}

#[test]
fn test_bool_literal() {
    let v = run_main("fn main() -> Bool { true }");
    assert_eq!(v.as_bool(), Some(true));
}

// ── Arithmetic ───────────────────────────────────────────────────────────

#[test]
fn test_addition() {
    let v = run_main("fn main() -> Int { 10 + 32 }");
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_precedence() {
    let v = run_main("fn main() -> Int { 2 + 3 * 4 }");
    assert_eq!(v.as_int(), Some(14));
}

#[test]
fn test_subtraction() {
    let v = run_main("fn main() -> Int { 50 - 8 }");
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_division() {
    let v = run_main("fn main() -> Int { 84 / 2 }");
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_modulo() {
    let v = run_main("fn main() -> Int { 10 % 3 }");
    assert_eq!(v.as_int(), Some(1));
}

#[test]
fn test_negation() {
    let v = run_main("fn main() -> Int { -42 }");
    assert_eq!(v.as_int(), Some(-42));
}

// ── Let bindings ─────────────────────────────────────────────────────────

#[test]
fn test_let_binding() {
    let v = run_main("fn main() -> Int { let x = 10; let y = 32; x + y }");
    assert_eq!(v.as_int(), Some(42));
}

// ── Function calls ───────────────────────────────────────────────────────

#[test]
fn test_function_call() {
    let v = run_main(
        "fn double(x: Int) -> Int { x + x }
         fn main() -> Int { double(21) }",
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_nested_calls() {
    let v = run_main(
        "fn double(x: Int) -> Int { x + x }
         fn main() -> Int { double(double(10)) + 2 }",
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_call_with_args() {
    let v = run_fn(
        "fn add(a: Int, b: Int) -> Int { a + b }",
        "add",
        vec![Value::Int(20), Value::Int(22)],
    );
    assert_eq!(v.as_int(), Some(42));
}

// ── If/else ──────────────────────────────────────────────────────────────

#[test]
fn test_if_true() {
    let v = run_main("fn main() -> Int { if true { 42 } else { 0 } }");
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_if_false() {
    let v = run_main("fn main() -> Int { if false { 0 } else { 42 } }");
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_if_comparison() {
    let v = run_main("fn main() -> Int { let x = 5; if x > 3 { 42 } else { 0 } }");
    assert_eq!(v.as_int(), Some(42));
}

// ── Match ────────────────────────────────────────────────────────────────

#[test]
fn test_match_int() {
    let v = run_main(
        r#"fn main() -> String {
            let x = 1;
            match x {
                0 => "zero",
                1 => "one",
                _ => "other"
            }
        }"#,
    );
    assert_eq!(v.as_str(), Some("one"));
}

#[test]
fn test_match_wildcard() {
    let v = run_main("fn main() -> Int { match 99 { 0 => 0, _ => 42 } }");
    assert_eq!(v.as_int(), Some(42));
}

// ── Comparison / Logical ─────────────────────────────────────────────────

#[test]
fn test_equality() {
    let v = run_main("fn main() -> Bool { 42 == 42 }");
    assert_eq!(v.as_bool(), Some(true));
}

#[test]
fn test_logical_and() {
    let v = run_main("fn main() -> Bool { true && false }");
    assert_eq!(v.as_bool(), Some(false));
}

#[test]
fn test_logical_or() {
    let v = run_main("fn main() -> Bool { false || true }");
    assert_eq!(v.as_bool(), Some(true));
}

// ── Structs ──────────────────────────────────────────────────────────────

#[test]
fn test_struct_create_and_access() {
    let v = run_main(
        "struct Point { x: Int, y: Int }
         fn main() -> Int { let p = Point { x: 40, y: 2 }; p.x + p.y }",
    );
    assert_eq!(v.as_int(), Some(42));
}

// ── Lambda / Pipe ────────────────────────────────────────────────────────

#[test]
fn test_lambda_call() {
    let v = run_main(
        "fn apply(f: (Int) -> Int, x: Int) -> Int { f(x) }
         fn main() -> Int { apply(|x: Int| x + 1, 41) }",
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_pipe() {
    let v = run_main(
        "fn double(x: Int) -> Int { x + x }
         fn main() -> Int { 21 |> double }",
    );
    assert_eq!(v.as_int(), Some(42));
}

// ── String concat ────────────────────────────────────────────────────────

#[test]
fn test_string_concat() {
    let v = run_main(r#"fn main() -> String { "hello" + " world" }"#);
    assert_eq!(v.as_str(), Some("hello world"));
}

// ── Recursion ────────────────────────────────────────────────────────────

#[test]
fn test_recursion() {
    let v = run_main(
        "fn factorial(n: Int) -> Int {
            if n <= 1 { 1 } else { n * factorial(n - 1) }
         }
         fn main() -> Int { factorial(10) }",
    );
    assert_eq!(v.as_int(), Some(3628800));
}

// ── Bitwise ──────────────────────────────────────────────────────────────

#[test]
fn test_bitwise_and() {
    let v = run_main("fn main() -> Int { 0xFF & 0x0F }");
    assert_eq!(v.as_int(), Some(0x0F));
}
