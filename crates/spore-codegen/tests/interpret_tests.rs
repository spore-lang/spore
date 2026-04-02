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
#[allow(clippy::approx_constant)]
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

// ── Enum constructors ───────────────────────────────────────────────────

#[test]
fn test_enum_zero_arg() {
    let v = run_main(
        "type Color { Red, Green, Blue }
         fn main() -> Color { Red }",
    );
    assert_eq!(v.to_string(), "Red");
}

#[test]
fn test_enum_with_fields() {
    let v = run_main(
        "type Option[T] { Some(T), None }
         fn main() -> Option[Int] { Some(42) }",
    );
    assert_eq!(v.to_string(), "Some(42)");
}

#[test]
fn test_enum_match() {
    let v = run_main(
        "type Option[T] { Some(T), None }
         fn main() -> Int {
             let x = Some(42);
             match x {
                 Some(n) => n,
                 None => 0,
             }
         }",
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_enum_match_zero_arg() {
    let v = run_main(
        "type Option[T] { Some(T), None }
         fn main() -> Int {
             let x = None;
             match x {
                 Some(n) => n,
                 None => 0,
             }
         }",
    );
    assert_eq!(v.as_int(), Some(0));
}

// ── Try operator ────────────────────────────────────────────────────────

#[test]
fn test_try_ok() {
    let v = run_main(
        "type Result[T, E] { Ok(T), Err(E) }
         fn main() -> Int {
             let r = Ok(42);
             r?
         }",
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
#[should_panic(expected = "uncaught error")]
fn test_try_err() {
    run_main(
        r#"type Result[T, E] { Ok(T), Err(E) }
         fn main() -> Int {
             let r = Err("bad");
             r?
         }"#,
    );
}

// ── List builtins ───────────────────────────────────────────────────────

#[test]
fn test_len() {
    let v = run_main("fn main() -> Int { len([1, 2, 3]) }");
    assert_eq!(v.as_int(), Some(3));
}

#[test]
fn test_map() {
    let v = run_main("fn main() -> List[Int] { map([1, 2, 3], |x: Int| x * 2) }");
    let list = v.as_list().unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].as_int(), Some(2));
    assert_eq!(list[1].as_int(), Some(4));
    assert_eq!(list[2].as_int(), Some(6));
}

#[test]
fn test_filter() {
    let v = run_main("fn main() -> List[Int] { filter([1, 2, 3, 4], |x: Int| x > 2) }");
    let list = v.as_list().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].as_int(), Some(3));
    assert_eq!(list[1].as_int(), Some(4));
}

#[test]
fn test_fold() {
    let v = run_main("fn main() -> Int { fold([1, 2, 3], 0, |acc: Int, x: Int| acc + x) }");
    assert_eq!(v.as_int(), Some(6));
}

#[test]
fn test_range() {
    let v = run_main("fn main() -> List[Int] { range(0, 5) }");
    let list = v.as_list().unwrap();
    assert_eq!(list.len(), 5);
    for (i, item) in list.iter().enumerate() {
        assert_eq!(item.as_int(), Some(i as i64));
    }
}

#[test]
fn test_append() {
    let v = run_main("fn main() -> List[Int] { append([1, 2], 3) }");
    let list = v.as_list().unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[2].as_int(), Some(3));
}

#[test]
fn test_reverse() {
    let v = run_main("fn main() -> List[Int] { reverse([1, 2, 3]) }");
    let list = v.as_list().unwrap();
    assert_eq!(list[0].as_int(), Some(3));
    assert_eq!(list[1].as_int(), Some(2));
    assert_eq!(list[2].as_int(), Some(1));
}

#[test]
fn test_head_tail() {
    let v = run_main("fn main() -> Int { head([10, 20, 30]) }");
    assert_eq!(v.as_int(), Some(10));

    let v2 = run_main("fn main() -> List[Int] { tail([10, 20, 30]) }");
    let list = v2.as_list().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].as_int(), Some(20));
}

#[test]
fn test_contains() {
    let v = run_main("fn main() -> Bool { contains([1, 2, 3], 2) }");
    assert_eq!(v.as_bool(), Some(true));

    let v2 = run_main("fn main() -> Bool { contains([1, 2, 3], 5) }");
    assert_eq!(v2.as_bool(), Some(false));
}

// ── String builtins ─────────────────────────────────────────────────────

#[test]
fn test_string_length() {
    let v = run_main(r#"fn main() -> Int { string_length("hello") }"#);
    assert_eq!(v.as_int(), Some(5));
}

#[test]
fn test_trim() {
    let v = run_main(r#"fn main() -> String { trim("  hi  ") }"#);
    assert_eq!(v.as_str(), Some("hi"));
}

#[test]
fn test_to_upper_lower() {
    let v = run_main(r#"fn main() -> String { to_upper("hello") }"#);
    assert_eq!(v.as_str(), Some("HELLO"));

    let v2 = run_main(r#"fn main() -> String { to_lower("HELLO") }"#);
    assert_eq!(v2.as_str(), Some("hello"));
}

#[test]
fn test_split() {
    let v = run_main(r#"fn main() -> List[String] { split("a,b,c", ",") }"#);
    let list = v.as_list().unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].as_str(), Some("a"));
    assert_eq!(list[2].as_str(), Some("c"));
}

#[test]
fn test_starts_ends_with() {
    let v = run_main(r#"fn main() -> Bool { starts_with("hello", "hel") }"#);
    assert_eq!(v.as_bool(), Some(true));

    let v2 = run_main(r#"fn main() -> Bool { ends_with("hello", "llo") }"#);
    assert_eq!(v2.as_bool(), Some(true));
}

#[test]
fn test_replace() {
    let v = run_main(r#"fn main() -> String { replace("hello world", "world", "spore") }"#);
    assert_eq!(v.as_str(), Some("hello spore"));
}

#[test]
fn test_to_string() {
    let v = run_main(r#"fn main() -> String { to_string(42) }"#);
    assert_eq!(v.as_str(), Some("42"));
}

#[test]
fn test_substring() {
    let v = run_main(r#"fn main() -> String { substring("hello", 1, 4) }"#);
    assert_eq!(v.as_str(), Some("ell"));
}

// ── Math builtins ───────────────────────────────────────────────────────

#[test]
fn test_abs() {
    let v = run_main("fn main() -> Int { abs(-5) }");
    assert_eq!(v.as_int(), Some(5));
}

#[test]
fn test_min_max() {
    let v = run_main("fn main() -> Int { min(3, 7) }");
    assert_eq!(v.as_int(), Some(3));

    let v2 = run_main("fn main() -> Int { max(3, 7) }");
    assert_eq!(v2.as_int(), Some(7));
}

// ── IO builtins ─────────────────────────────────────────────────────────

#[test]
fn test_println_runs() {
    let v = run_main(r#"fn main() -> Unit { println("hello") }"#);
    // println returns Unit
    assert_eq!(v.to_string(), "()");
}

// ── Each builtin ────────────────────────────────────────────────────────

#[test]
fn test_each() {
    // each should return Unit; we just verify it doesn't crash
    let v = run_main(r#"fn main() -> Unit { each([1, 2, 3], |x: Int| println(to_string(x))) }"#);
    assert_eq!(v.to_string(), "()");
}
