use super::*;
use crate::parse;

/// Helper: parse source then format it.
fn roundtrip(source: &str) -> String {
    let module = parse(source).expect("parse failed");
    format_module(&module)
}

#[test]
fn test_simple_function() {
    let src = "fn add(a: Int, b: Int) -> Int { a + b }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_function_with_errors_roundtrips() {
    let src = "fn risky(path: Str) -> Str ! IoError | ParseError { path }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_struct_def() {
    let src = "struct Point { x: Int, y: Int }\n";
    let out = roundtrip(src);
    assert!(out.contains("struct Point {"));
    assert!(out.contains("x: Int,"));
    assert!(out.contains("y: Int,"));
}

#[test]
fn test_type_def_and_match() {
    let src = concat!(
        "type Shape {\n",
        "    Circle(Int),\n",
        "    Rect(Int, Int),\n",
        "}\n",
    );
    let out = roundtrip(src);
    assert!(out.contains("type Shape {"));
    assert!(out.contains("Circle(Int),"));
    assert!(out.contains("Rect(Int, Int),"));
}

#[test]
fn test_pipe_operator() {
    let src = "fn main() -> Int { 10 |> double }\n";
    let out = roundtrip(src);
    assert!(out.contains("10 |> double"));
}

#[test]
fn test_lambda() {
    let src = "fn apply(f: (Int) -> Int, x: Int) -> Int { f(x) }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_multi_statement_block() {
    let src = concat!(
        "fn main() -> Int {\n",
        "    let x = 1;\n",
        "    let y = 2;\n",
        "    x + y\n",
        "}\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_match_expression() {
    let src = concat!(
        "fn area(s: Shape) -> Int {\n",
        "    match s {\n",
        "        Circle(r) => r * r * 3,\n",
        "        Rect(w, h) => w * h,\n",
        "    }\n",
        "}\n",
    );
    let out = roundtrip(src);
    assert!(out.contains("match s {"));
    assert!(out.contains("Circle(r) => r * r * 3,"));
    assert!(out.contains("Rect(w, h) => w * h,"));
}

#[test]
fn test_single_match_body_stays_multiline() {
    let src = concat!(
        "fn area(s: Shape) -> Int { match s {\n",
        "    Circle(r) => r * r * 3,\n",
        "    Rect(w, h) => w * h,\n",
        "} }\n",
    );
    let expected = concat!(
        "fn area(s: Shape) -> Int {\n",
        "    match s {\n",
        "        Circle(r) => r * r * 3,\n",
        "        Rect(w, h) => w * h,\n",
        "    }\n",
        "}\n",
    );
    assert_eq!(roundtrip(src), expected);
}

#[test]
fn test_simple_if_body_stays_multiline() {
    let src = "fn classify(x: I32) -> I32 { if x < 0 { 0 } else { x } }\n";
    let expected = concat!(
        "fn classify(x: I32) -> I32 {\n",
        "    if x < 0 { 0 } else { x }\n",
        "}\n",
    );
    assert_eq!(roundtrip(src), expected);
}

#[test]
fn test_nested_if_body_stays_multiline() {
    let src = "fn classify(x: I32) -> I32 { if x < 0 { 0 } else { if x == 0 { 1 } else { 2 } } }\n";
    let expected = concat!(
        "fn classify(x: I32) -> I32 {\n",
        "    if x < 0 { 0 } else {\n",
        "        if x == 0 { 1 } else { 2 }\n",
        "    }\n",
        "}\n",
    );
    assert_eq!(roundtrip(src), expected);
}

#[test]
fn test_uses_clause() {
    let src = "fn read() -> String uses [IO, FileRead] { ?todo }\n";
    let out = roundtrip(src);
    assert!(out.contains("uses [IO, FileRead]"));
}

#[test]
fn test_hole_syntax_roundtrip() {
    let src = "fn f(x: ?) -> ? { ? }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_allows_annotation_roundtrip() {
    let src = "@allows[validate, sanitize]\nfn f() -> Int { ?todo }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_hole_level_allows_roundtrip() {
    let src = "fn f() -> Int { ?todo @allows[validate, sanitize] }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_foreign_fn_roundtrips() {
    let src = "foreign fn c_add(a: Int, b: Int) -> Int\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_foreign_fn_with_uses_roundtrips() {
    let src = "foreign fn read_file(path: String) -> String uses [FileRead]\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_foreign_fn_with_errors_roundtrips() {
    let src = "foreign fn process_run(cmd: Str, args: List[Str]) -> Str ! IoError | ExecError uses [Spawn]\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_spec_clause_normalizes_clause_order_and_preserves_item_order() {
    let src = concat!(
        "fn show[T](x: T) -> T cost [5, 0, 0, 0] spec {\n",
        "    property \"roundtrip\": |x: T| true\n",
        "    example \"block\" {\n",
        "        let y = x;\n",
        "        y == x\n",
        "    }\n",
        "} uses [Console] where T: Display { x }\n",
    );
    let expected = concat!(
        "fn show[T](x: T) -> T where T: Display uses [Console] cost [5, 0, 0, 0]\n",
        "spec {\n",
        "    property \"roundtrip\": |x: T| true\n",
        "    example \"block\" {\n",
        "        let y = x;\n",
        "        y == x\n",
        "    }\n",
        "}\n",
        "{ x }\n",
    );
    assert_eq!(roundtrip(src), expected);
}

#[test]
fn test_refinement_type_roundtrips_in_property_params() {
    let src = concat!(
        "fn abs(x: I32) -> I32\n",
        "spec {\n",
        "    property \"non_negative_identity\": |x: I32 when self >= 0| x\n",
        "}\n",
        "{\n",
        "    if x < 0 { 0 - x } else { x }\n",
        "}\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_multiple_properties_roundtrip() {
    let src = concat!(
        "fn add(a: I32, b: I32) -> I32\n",
        "spec {\n",
        "    property \"left_identity\": |a: I32, b: I32 when self == 0| a\n",
        "    property \"non_negative_identity\": |x: I32 when self >= 0| x\n",
        "}\n",
        "{ a + b }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_blank_line_between_items() {
    let src = concat!("fn a() -> Int { 1 }\n", "\n", "fn b() -> Int { 2 }\n",);
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_const_def() {
    let src = "const MAX: Int = 100\n";
    let out = roundtrip(src);
    assert!(out.contains("const MAX: Int = 100"));
}

#[test]
fn test_keyword_item_forms_roundtrip() {
    let src = concat!(
        "trait Display[T] {\n",
        "    fn show(self: T) -> String\n",
        "}\n",
        "\n",
        "effect Console {\n",
        "    fn println(msg: String) -> Unit\n",
        "}\n",
        "\n",
        "effect IO = Console | FileRead\n",
        "\n",
        "handler Console as MockConsole {\n",
        "    fn println(msg: String) -> Unit {}\n",
        "}\n",
    );
    let out = roundtrip(src);
    assert!(out.contains("trait Display[T] {"));
    assert!(out.contains("effect Console {"));
    assert!(out.contains("effect IO = Console | FileRead"));
    assert!(out.contains("handler Console as MockConsole {"));
}

#[test]
fn test_doc_comment_preserved() {
    let src = concat!(
        "/// Adds two numbers.\n",
        "fn add(a: Int, b: Int) -> Int { a + b }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_line_comment_between_items_preserved() {
    let src = concat!(
        "fn a() -> Int { 1 }\n",
        "\n",
        "// helper\n",
        "fn b() -> Int { 2 }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_multi_line_doc_comment_preserved() {
    let src = concat!(
        "/// First line.\n",
        "/// Second line.\n",
        "fn greet() -> Str { \"hi\" }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_block_comment_preserved() {
    let src = concat!("/* block comment */\n", "fn f() -> Int { 0 }\n",);
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_blank_line_between_comment_groups_preserved() {
    let src = concat!(
        "/// Module doc.\n",
        "\n",
        "/// Function doc.\n",
        "fn f() -> Int { 0 }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_trailing_comment_after_last_item() {
    let src = concat!("fn f() -> Int { 0 }\n", "\n", "// end of file\n",);
    let out = roundtrip(src);
    assert!(out.contains("// end of file"));
}
