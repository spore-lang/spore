use sporec_codegen::{RuntimePlatform, value::Value};
use sporec_parser::parse;

fn run_main(src: &str) -> Value {
    let module = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    sporec_codegen::run(&module).unwrap_or_else(|e| panic!("runtime error: {e}"))
}

fn run_fn(src: &str, name: &str, args: Vec<Value>) -> Value {
    let module = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    sporec_codegen::call(&module, name, args).unwrap_or_else(|e| panic!("runtime error: {e}"))
}

fn run_project_fn(src: &str, startup: &str) -> Value {
    let module = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    sporec_codegen::run_project(&module, &[], startup)
        .unwrap_or_else(|e| panic!("runtime error: {e}"))
}

fn run_project_with_adapter(
    entry_src: &str,
    adapter_src: &str,
    startup: &str,
    adapter: &str,
) -> Value {
    let entry = parse(entry_src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    let adapter_module = parse(adapter_src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    sporec_codegen::run_project_with_adapter(
        &entry,
        &[("platform_contract".into(), adapter_module)],
        startup,
        adapter,
    )
    .unwrap_or_else(|e| panic!("runtime error: {e}"))
}

fn run_project_on_platform(
    entry_src: &str,
    imports: &[(&str, &str)],
    startup: &str,
    runtime_platform: RuntimePlatform,
) -> Value {
    let entry = parse(entry_src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    let imported = imports
        .iter()
        .map(|(path, src)| {
            (
                (*path).to_string(),
                parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}")),
            )
        })
        .collect::<Vec<_>>();
    sporec_codegen::run_project_on_platform(&entry, &imported, startup, runtime_platform)
        .unwrap_or_else(|e| panic!("runtime error: {e}"))
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
fn test_project_runtime_can_call_platform_adapter() {
    let v = run_project_with_adapter(
        "fn main() -> Int { ?entry_should_not_run }",
        "pub fn main_for_host(app_main: () -> Int) -> Int { 42 }",
        "main",
        "platform_contract.main_for_host",
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_project_runtime_basic_cli_handler_supports_package_foreign_functions() {
    let temp_path = std::env::temp_dir().display().to_string();
    let entry_src = format!(
        r#"
        import basic_cli.file

        fn main() -> Bool {{
            file_exists("{temp_path}")
        }}
        "#
    );
    let v = run_project_on_platform(
        &entry_src,
        &[(
            "basic_cli.file",
            "pub foreign fn file_exists(path: Str) -> Bool uses [FileRead]",
        )],
        "main",
        RuntimePlatform::BasicCli,
    );
    assert_eq!(v.as_bool(), Some(true));
}

#[test]
fn test_basic_cli_handler_does_not_intercept_non_foreign_functions() {
    let v = run_project_on_platform(
        r#"
        fn file_exists(x: Int) -> Int {
            x + 1
        }

        fn main() -> Int {
            file_exists(41)
        }
        "#,
        &[],
        "main",
        RuntimePlatform::BasicCli,
    );
    assert_eq!(v.as_int(), Some(42));
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

#[test]
fn test_project_run_uses_requested_startup_function() {
    let v = run_project_fn(
        "fn boot() -> Int { 42 }
         fn main() -> Int { 0 }",
        "boot",
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
    // head returns Option: Some(value) for non-empty list
    let v = run_main("fn main() -> Option[Int] { head([10, 20, 30]) }");
    // Value is Enum("Some", [Int(10)])
    match &v {
        sporec_codegen::value::Value::Enum(name, fields) => {
            assert_eq!(name, "Some");
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].as_int(), Some(10));
        }
        other => panic!("expected Some variant, got: {other:?}"),
    }

    // tail returns Option: Some(list) for non-empty list
    let v2 = run_main("fn main() -> Option[List[Int]] { tail([10, 20, 30]) }");
    match &v2 {
        sporec_codegen::value::Value::Enum(name, fields) => {
            assert_eq!(name, "Some");
            assert_eq!(fields.len(), 1);
            let list = fields[0].as_list().unwrap();
            assert_eq!(list.len(), 2);
            assert_eq!(list[0].as_int(), Some(20));
        }
        other => panic!("expected Some variant, got: {other:?}"),
    }
}

#[test]
fn test_contains() {
    let v = run_main("fn main() -> Bool { contains([1, 2, 3], 2) }");
    assert_eq!(v.as_bool(), Some(true));

    let v2 = run_main("fn main() -> Bool { contains([1, 2, 3], 5) }");
    assert_eq!(v2.as_bool(), Some(false));
}

// ── String builtins ─────────────────────────────────────────────────────

// ── Regression: head/tail of empty list returns None (Bug A7) ──────────

#[test]
fn test_head_empty_returns_none() {
    let v = run_main("fn main() -> Option[Int] { head([]) }");
    match &v {
        Value::Enum(name, fields) => {
            assert_eq!(name, "None");
            assert!(fields.is_empty());
        }
        other => panic!("expected None variant, got: {other:?}"),
    }
}

#[test]
fn test_tail_empty_returns_none() {
    let v = run_main("fn main() -> Option[List[Int]] { tail([]) }");
    match &v {
        Value::Enum(name, fields) => {
            assert_eq!(name, "None");
            assert!(fields.is_empty());
        }
        other => panic!("expected None variant, got: {other:?}"),
    }
}

#[test]
fn test_split_returns_list_str() {
    let v = run_main(r#"fn main() -> List[String] { split("a,b", ",") }"#);
    let list = v.as_list().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].as_str(), Some("a"));
    assert_eq!(list[1].as_str(), Some("b"));
}

#[test]
fn test_to_string_float() {
    let v = run_main(r#"fn main() -> String { to_string(3.14) }"#);
    assert_eq!(v.as_str(), Some("3.14"));
}

// ── String builtins (continued) ─────────────────────────────────────────

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

// ── Placeholder partial application ─────────────────────────────────────

#[test]
fn test_placeholder_single() {
    // `add(_, 5)` creates a unary closure; calling it with 3 yields 8
    let v = run_main(
        r#"
        fn add(a: Int, b: Int) -> Int { a + b }
        fn main() -> Int {
            let add5 = add(_, 5)
            add5(3)
        }
    "#,
    );
    assert_eq!(v.as_int(), Some(8));
}

#[test]
fn test_placeholder_multi() {
    // `sub(_, _)` with two placeholders creates a binary closure
    let v = run_main(
        r#"
        fn sub(a: Int, b: Int) -> Int { a - b }
        fn main() -> Int {
            let f = sub(_, _)
            f(1, 2)
        }
    "#,
    );
    assert_eq!(v.as_int(), Some(-1));
}

#[test]
fn test_placeholder_pipe() {
    // `5 |> add(_, 3)` desugars rhs to a closure, then pipe calls it with 5
    let v = run_main(
        r#"
        fn add(a: Int, b: Int) -> Int { a + b }
        fn main() -> Int {
            5 |> add(_, 3)
        }
    "#,
    );
    assert_eq!(v.as_int(), Some(8));
}

#[test]
fn test_placeholder_nested_calls() {
    // Nested partial application with composition
    let v = run_main(
        r#"
        fn add(a: Int, b: Int) -> Int { a + b }
        fn mul(a: Int, b: Int) -> Int { a * b }
        fn main() -> Int {
            let f = add(_, 10)
            let g = mul(_, 3)
            f(g(2))
        }
    "#,
    );
    // mul(2, 3) = 6, add(6, 10) = 16
    assert_eq!(v.as_int(), Some(16));
}

#[test]
fn test_placeholder_pipe_chain() {
    let v = run_main(
        r#"
        fn add(a: Int, b: Int) -> Int { a + b }
        fn mul(a: Int, b: Int) -> Int { a * b }
        fn main() -> Int {
            1 |> add(_, 2) |> mul(_, 3)
        }
    "#,
    );
    // add(1, 2) = 3, mul(3, 3) = 9
    assert_eq!(v.as_int(), Some(9));
}

// ── Stdlib: parse each stdlib file ──────────────────────────────────────

fn assert_stdlib_source_parses(name: &str, src: &str) {
    sporec_parser::parse(src).unwrap_or_else(|e| panic!("{name} parse error: {e:?}"));
}

#[test]
fn test_stdlib_sources_parse() {
    for (name, src) in [
        ("prelude.sp", include_str!("../../../stdlib/prelude.sp")),
        ("math.sp", include_str!("../../../stdlib/math.sp")),
        ("string.sp", include_str!("../../../stdlib/string.sp")),
        (
            "collections.sp",
            include_str!("../../../stdlib/collections.sp"),
        ),
        ("dict.sp", include_str!("../../../stdlib/dict.sp")),
        ("set.sp", include_str!("../../../stdlib/set.sp")),
        ("char.sp", include_str!("../../../stdlib/char.sp")),
    ] {
        assert_stdlib_source_parses(name, src);
    }
}

// ── Stdlib: runtime tests ───────────────────────────────────────────────

#[test]
fn test_stdlib_unwrap_or_some() {
    // unwrap_or comes from prelude stdlib; no need to redefine Option/unwrap_or
    let v = run_main("fn main() -> Int { unwrap_or(Some(42), 0) }");
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_stdlib_unwrap_or_none() {
    let v = run_main("fn main() -> Int { unwrap_or(None, 0) }");
    assert_eq!(v.as_int(), Some(0));
}

#[test]
fn test_stdlib_unwrap_or_result_ok() {
    let v = run_main("fn main() -> Int { unwrap_or_result(Ok(42), 0) }");
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_stdlib_unwrap_or_result_err() {
    let v = run_main(r#"fn main() -> Int { unwrap_or_result(Err("bad"), 0) }"#);
    assert_eq!(v.as_int(), Some(0));
}

#[test]
fn test_foreign_fn_runtime_error() {
    let src = r#"
        foreign fn read_file(path: String) -> String
        fn main() -> String { read_file("test.txt") }
    "#;
    let module = parse(src).unwrap();
    let err = sporec_codegen::run(&module).unwrap_err();
    assert!(
        err.to_string()
            .contains("foreign function `read_file` is not available in interpreter mode"),
        "unexpected error: {err}"
    );
}

// ── Perform / Handle effect dispatch ────────────────────────────────────

#[test]
fn test_perform_println_dispatches_to_cli_handler() {
    // perform StdIO.println should fall back to CliPlatformHandler
    let v = run_main(r#"fn main() { perform StdIO.println("hello from perform") }"#);
    assert!(matches!(v, Value::Unit));
}

#[test]
fn test_handle_intercepts_effect() {
    // handle block intercepts the perform and returns 99 instead
    let v = run_main(
        r#"
        fn main() -> Int {
            handle {
                perform StdIO.println("intercepted")
                42
            } with {
                StdIO.println(msg) => 99
            }
        }
        "#,
    );
    // The handler arm returns 99 which becomes the perform result,
    // but then the block continues with 42 as the tail.
    // Actually, the handler returns 99 from the perform call,
    // then 42 is the block tail.
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_handle_handler_sees_args() {
    let v = run_main(
        r#"
        fn main() -> Int {
            handle {
                perform Math.double(21)
            } with {
                on Math.double(x) => x + x
            }
        }
        "#,
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_handler_result_becomes_perform_value() {
    let v = run_main(
        r#"
        fn main() -> Int {
            let doubled = handle {
                perform Math.double(21)
            } with {
                on Math.double(x) => x + x
            }
            doubled
        }
        "#,
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_named_handler_instance_uses_payload_and_self() {
    let v = run_main(
        r#"
        effect Math {
            fn double(x: Int) -> Int
        }
        handler Math as DoubleMath(multiplier: Int) {
            fn double(x: Int) -> Int {
                x * self.multiplier
            }
        }
        fn main() -> Int {
            handle {
                perform Math.double(21)
            } with {
                use DoubleMath { multiplier: 2 }
            }
        }
        "#,
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_named_handler_and_inline_on_can_mix_for_different_effects() {
    let v = run_main(
        r#"
        effect Math {
            fn double(x: Int) -> Int
        }
        handler Math as DoubleMath(multiplier: Int) {
            fn double(x: Int) -> Int { x * self.multiplier }
        }
        fn main() -> Int {
            handle {
                perform StdIO.println("hello")
                perform Math.double(21)
            } with {
                use DoubleMath { multiplier: 2 },
                on StdIO.println(msg) => 0
            }
        }
        "#,
    );
    assert_eq!(v.as_int(), Some(42));
}

// ── Stdlib: extended runtime tests ──────────────────────────────────

#[test]
fn test_stdlib_map_option_some() {
    let v = run_main(
        "fn main() -> Int {
             let x = map_option(Some(21), |v: Int| v * 2);
             unwrap_or(x, 0)
         }",
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_nested_handlers_inner_shadows_outer() {
    let v = run_main(
        r#"
        fn main() -> Int {
            handle {
                handle {
                    perform Math.value()
                } with {
                    Math.value() => 42
                }
            } with {
                Math.value() => 0
            }
        }
        "#,
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_handler_arm_runs_with_current_handler_stack() {
    let v = run_main(
        r#"
        fn main() -> Int {
            handle {
                handle {
                    perform Math.a()
                } with {
                    Math.a() => perform Math.b()
                    Math.b() => 1
                }
            } with {
                Math.b() => 2
            }
        }
        "#,
    );
    assert_eq!(v.as_int(), Some(1));
}

#[test]
fn test_handler_scope_does_not_escape_block() {
    let module = sporec_parser::parse(
        r#"
        fn main() -> Int {
            handle {
                perform Math.value()
            } with {
                Math.value() => 41
            }
            perform Math.value()
        }
        "#,
    )
    .unwrap();
    let err = sporec_codegen::run(&module).unwrap_err();
    assert!(
        err.to_string().contains("unhandled effect"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_stdlib_map_option_none() {
    let v = run_main(
        "fn main() -> Int {
             let x = map_option(None, |v: Int| v * 2);
             unwrap_or(x, 0)
         }",
    );
    assert_eq!(v.as_int(), Some(0));
}

#[test]
fn test_stdlib_map_result_ok() {
    let v = run_main(
        "fn main() -> Int {
             let x = map_result(Ok(21), |v: Int| v * 2);
             unwrap_or_result(x, 0)
         }",
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_unhandled_effect_error() {
    let module = sporec_parser::parse(r#"fn main() { perform Unknown.op() }"#).unwrap();
    let err = sporec_codegen::run(&module).unwrap_err();
    assert!(
        err.to_string().contains("unhandled effect"),
        "unexpected error: {err}"
    );
}

// ── Shift bounds ─────────────────────────────────────────────────────

#[test]
fn test_shift_left_out_of_range_negative() {
    let module = sporec_parser::parse("fn main() -> Int { 1 << -1 }").unwrap();
    let err = sporec_codegen::run(&module).unwrap_err();
    assert!(
        err.to_string().contains("shift amount"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_shift_left_out_of_range_large() {
    let module = sporec_parser::parse("fn main() -> Int { 1 << 64 }").unwrap();
    let err = sporec_codegen::run(&module).unwrap_err();
    assert!(
        err.to_string().contains("shift amount"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_shift_right_out_of_range() {
    let module = sporec_parser::parse("fn main() -> Int { 1 >> 100 }").unwrap();
    let err = sporec_codegen::run(&module).unwrap_err();
    assert!(
        err.to_string().contains("shift amount"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_shift_valid_amounts() {
    let v = run_main("fn main() -> Int { 1 << 3 }");
    assert_eq!(v.as_int(), Some(8));

    let v = run_main("fn main() -> Int { 16 >> 2 }");
    assert_eq!(v.as_int(), Some(4));
}

// ── Integer overflow safety ─────────────────────────────────────────────

fn run_main_err(src: &str) -> String {
    let module = sporec_parser::parse(src).unwrap();
    sporec_codegen::run(&module).unwrap_err().to_string()
}

#[test]
fn test_add_overflow() {
    let src = &format!("fn main() -> Int {{ {} + 1 }}", i64::MAX);
    let err = run_main_err(src);
    assert!(err.contains("integer overflow"), "got: {err}");
}

#[test]
fn test_sub_overflow() {
    let src = &format!("fn main() -> Int {{ {} - 2 }}", i64::MIN + 1);
    let err = run_main_err(src);
    assert!(err.contains("integer overflow"), "got: {err}");
}

#[test]
fn test_mul_overflow() {
    let src = &format!("fn main() -> Int {{ {} * 2 }}", i64::MAX);
    let err = run_main_err(src);
    assert!(err.contains("integer overflow"), "got: {err}");
}

#[test]
fn test_neg_overflow() {
    // Construct i64::MIN at runtime then negate it — that overflows.
    let src = "fn main() -> Int { let x: Int = 0 - 9223372036854775807 - 1; -x }";
    let err = run_main_err(src);
    assert!(err.contains("integer overflow"), "got: {err}");
}

#[test]
fn test_range_too_large() {
    let src = "fn main() -> Int { let xs = range(0, 20000000); len(xs) }";
    let err = run_main_err(src);
    assert!(err.contains("range too large"), "got: {err}");
}

#[test]
fn test_stdlib_map_result_err() {
    let v = run_main(
        r#"fn main() -> Int {
             let x = map_result(Err("bad"), |v: Int| v * 2);
             unwrap_or_result(x, 0)
         }"#,
    );
    assert_eq!(v.as_int(), Some(0));
}

#[test]
fn test_stdlib_is_some_with_some() {
    let v = run_main("fn main() -> Bool { is_some(Some(99)) }");
    assert_eq!(v.as_bool(), Some(true));
}

#[test]
fn test_stdlib_is_some_with_none() {
    let v = run_main("fn main() -> Bool { is_some(None) }");
    assert_eq!(v.as_bool(), Some(false));
}

#[test]
fn test_stdlib_is_none_with_none() {
    let v = run_main("fn main() -> Bool { is_none(None) }");
    assert_eq!(v.as_bool(), Some(true));
}

#[test]
fn test_stdlib_is_none_with_some() {
    let v = run_main("fn main() -> Bool { is_none(Some(1)) }");
    assert_eq!(v.as_bool(), Some(false));
}

#[test]
fn test_stdlib_is_ok_true() {
    let v = run_main("fn main() -> Bool { is_ok(Ok(42)) }");
    assert_eq!(v.as_bool(), Some(true));
}

#[test]
fn test_stdlib_is_ok_false() {
    let v = run_main(r#"fn main() -> Bool { is_ok(Err("oops")) }"#);
    assert_eq!(v.as_bool(), Some(false));
}

#[test]
fn test_stdlib_is_err_true() {
    let v = run_main(r#"fn main() -> Bool { is_err(Err("oops")) }"#);
    assert_eq!(v.as_bool(), Some(true));
}

#[test]
fn test_stdlib_is_err_false() {
    let v = run_main("fn main() -> Bool { is_err(Ok(42)) }");
    assert_eq!(v.as_bool(), Some(false));
}

// ── Prelude: new combinator tests ───────────────────────────────────────

#[test]
fn test_stdlib_and_then_option_some() {
    let v = run_main(
        "fn safe_div(x: Int) -> Option[Int] { if x == 0 { None } else { Some(100 / x) } }
         fn main() -> Option[Int] { and_then(Some(5), |x: Int| safe_div(x)) }",
    );
    assert_eq!(v.to_string(), "Some(20)");
}

#[test]
fn test_stdlib_and_then_option_none() {
    let v = run_main(
        "fn safe_div(x: Int) -> Option[Int] { if x == 0 { None } else { Some(100 / x) } }
         fn main() -> Option[Int] { and_then(None, |x: Int| safe_div(x)) }",
    );
    assert_eq!(v.to_string(), "None");
}

#[test]
fn test_stdlib_and_then_option_chain_to_none() {
    let v = run_main(
        "fn safe_div(x: Int) -> Option[Int] { if x == 0 { None } else { Some(100 / x) } }
         fn main() -> Option[Int] { and_then(Some(0), |x: Int| safe_div(x)) }",
    );
    assert_eq!(v.to_string(), "None");
}

#[test]
fn test_stdlib_flatten_option_some_some() {
    let v = run_main("fn main() -> Option[Int] { flatten_option(Some(Some(42))) }");
    assert_eq!(v.to_string(), "Some(42)");
}

#[test]
fn test_stdlib_flatten_option_some_none() {
    let v = run_main("fn main() -> Option[Int] { flatten_option(Some(None)) }");
    assert_eq!(v.to_string(), "None");
}

#[test]
fn test_stdlib_flatten_option_none() {
    let v = run_main("fn main() -> Option[Int] { flatten_option(None) }");
    assert_eq!(v.to_string(), "None");
}

#[test]
fn test_stdlib_and_then_result_ok() {
    let v = run_main(
        "fn safe(x: Int) -> Result[Int, String] { if x > 0 { Ok(x * 2) } else { Err(\"neg\") } }
         fn main() -> Result[Int, String] { and_then_result(Ok(5), |x: Int| safe(x)) }",
    );
    assert_eq!(v.to_string(), "Ok(10)");
}

#[test]
fn test_stdlib_and_then_result_err() {
    let v = run_main(
        "fn safe(x: Int) -> Result[Int, String] { if x > 0 { Ok(x * 2) } else { Err(\"neg\") } }
         fn main() -> Result[Int, String] { and_then_result(Err(\"bad\"), |x: Int| safe(x)) }",
    );
    assert_eq!(v.to_string(), "Err(bad)");
}

#[test]
fn test_stdlib_map_err_err() {
    let v = run_main(
        "fn main() -> Int { match map_err(Err(\"bad\"), |e: String| string_length(e)) { Ok(_) => 0, Err(n) => n } }",
    );
    assert_eq!(v.as_int(), Some(3));
}

#[test]
fn test_stdlib_map_err_ok() {
    let v = run_main(
        "fn main() -> Int { match map_err(Ok(42), |e: String| string_length(e)) { Ok(n) => n, Err(_) => 0 } }",
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_stdlib_flatten_result_ok_ok() {
    let v = run_main(
        "fn main() -> Int { match flatten_result(Ok(Ok(42))) { Ok(n) => n, Err(_) => 0 } }",
    );
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_stdlib_flatten_result_ok_err() {
    let v = run_main(
        "fn main() -> Int { match flatten_result(Ok(Err(\"bad\"))) { Ok(_) => 0, Err(_) => 1 } }",
    );
    assert_eq!(v.as_int(), Some(1));
}

#[test]
fn test_stdlib_flatten_result_err() {
    let v = run_main(
        "fn main() -> Int { match flatten_result(Err(\"bad\")) { Ok(_) => 0, Err(_) => 1 } }",
    );
    assert_eq!(v.as_int(), Some(1));
}

#[test]
fn test_stdlib_compare_int() {
    let v = run_main("fn main() -> Bool { match compare(1, 2) { Less => true, _ => false } }");
    assert_eq!(v.as_bool(), Some(true));
    let v = run_main("fn main() -> Bool { match compare(5, 5) { Equal => true, _ => false } }");
    assert_eq!(v.as_bool(), Some(true));
    let v = run_main("fn main() -> Bool { match compare(9, 3) { Greater => true, _ => false } }");
    assert_eq!(v.as_bool(), Some(true));
}

#[test]
fn test_stdlib_pair() {
    let v = run_main("fn main() -> Int { let p = Pair { first: 42, second: \"hello\" }; p.first }");
    assert_eq!(v.as_int(), Some(42));
    let v =
        run_main("fn main() -> String { let p = Pair { first: 42, second: \"hello\" }; p.second }");
    assert_eq!(v.to_string(), "hello");
}

#[test]
fn test_stdlib_identity() {
    let v = run_main("fn main() -> Int { identity(42) }");
    assert_eq!(v.as_int(), Some(42));
}

// ── Spec clause ─────────────────────────────────────────────────────────

#[test]
fn test_fn_with_spec_clause_parses_and_runs() {
    // A function with a spec clause should parse and execute normally.
    // The spec block is recorded but not executed during normal interpretation.
    let src = r#"
        fn add(a: Int, b: Int) -> Int
        spec {
            example "positive inputs": add(2, 3) == 5
            example "identity":        add(0, 42) == 42
            property "commutative":    |a: Int, b: Int| add(a, b) == add(b, a)
        }
        {
            a + b
        }
        fn main() -> Int { add(10, 32) }
    "#;
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_fn_with_spec_examples_only() {
    let src = r#"
        fn double(x: Int) -> Int
        spec {
            example "zero": double(0) == 0
            example "one":  double(1) == 2
        }
        {
            x * 2
        }
        fn main() -> Int { double(21) }
    "#;
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_fn_with_spec_properties_only() {
    let src = r#"
        fn id(x: Int) -> Int
        spec {
            property "idempotent": |x: Int| id(id(x)) == id(x)
        }
        {
            x
        }
        fn main() -> Int { id(42) }
    "#;
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_fn_with_block_spec_example_parses_and_runs() {
    let src = r#"
        fn add(a: Int, b: Int) -> Int
        spec {
            example "block" {
                let sum = add(2, 3)
                sum == 5
            }
        }
        {
            a + b
        }
        fn main() -> Int { add(10, 32) }
    "#;
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_fn_without_spec_still_works() {
    // Backward compatibility: functions without spec must still work
    let src = "fn main() -> Int { 42 }";
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_fn_with_empty_spec() {
    let src = r#"
        fn noop() -> Int
        spec {
        }
        {
            0
        }
        fn main() -> Int { noop() }
    "#;
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(0));
}

#[test]
fn test_task_await_executes_spawned_expression() {
    let src = r#"
        fn main() -> Int {
            let t = spawn { 40 + 2 }
            t.await
        }
    "#;
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_channel_new_send_then_select_recv_arm() {
    let src = r#"
        fn main() -> Int {
            let pair = Channel.new[Int](buffer: 1)
            match head(pair) {
                Some(tx) => match tail(pair) {
                    Some(rest) => match head(rest) {
                        Some(rx) => {
                            tx.send(42)
                            select {
                                value from rx => value,
                                timeout(5) => 0
                            }
                        },
                        None => -3
                    },
                    None => -2
                },
                None => -1
            }
        }
    "#;
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_select_timeout_arm_runs_when_no_message_ready() {
    let src = r#"
        fn main() -> Int {
            let pair = Channel.new[Int](buffer: 1)
            match tail(pair) {
                Some(rest) => match head(rest) {
                    Some(rx) => select {
                        value from rx => value,
                        timeout(5) => 7
                    },
                    None => -2
                },
                None => -1
            }
        }
    "#;
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(7));
}

#[test]
fn test_parallel_scope_drains_spawned_tasks_on_exit() {
    let src = r#"
        fn main() -> Int {
            let pair = Channel.new[Int](buffer: 1)
            match head(pair) {
                Some(tx) => match tail(pair) {
                    Some(rest) => match head(rest) {
                        Some(rx) => {
                            parallel_scope {
                                let _task = spawn { tx.send(42) }
                                0
                            }
                            select {
                                value from rx => value,
                                timeout(5) => -1
                            }
                        },
                        None => -3
                    },
                    None => -2
                },
                None => -1
            }
        }
    "#;
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn test_parallel_scope_cancels_pending_tasks_when_body_errors() {
    let src = r#"
        fn main() -> Int {
            parallel_scope {
                let _task = spawn { throw 99 }
                throw 1
            }
        }
    "#;
    let err = run_main_err(src);
    assert!(err.contains("throw: 1"), "unexpected error: {err}");
}

#[test]
fn test_select_rotates_across_ready_recv_arms() {
    let src = r#"
        fn main() -> Int {
            let pair1 = Channel.new[Int](buffer: 2)
            let pair2 = Channel.new[Int](buffer: 2)
            match head(pair1) {
                Some(tx1) => match tail(pair1) {
                    Some(rest1) => match head(rest1) {
                        Some(rx1) => match head(pair2) {
                            Some(tx2) => match tail(pair2) {
                                Some(rest2) => match head(rest2) {
                                    Some(rx2) => {
                                        tx1.send(10)
                                        tx1.send(11)
                                        tx2.send(20)
                                        tx2.send(21)
                                        let a = select {
                                            v from rx1 => v,
                                            v from rx2 => v,
                                            timeout(1) => 0
                                        }
                                        let b = select {
                                            v from rx1 => v,
                                            v from rx2 => v,
                                            timeout(1) => 0
                                        }
                                        a + b
                                    },
                                    None => -6
                                },
                                None => -5
                            },
                            None => -4
                        },
                        None => -3
                    },
                    None => -2
                },
                None => -1
            }
        }
    "#;
    let v = run_main(src);
    assert_eq!(v.as_int(), Some(30));
}
