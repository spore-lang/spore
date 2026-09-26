use super::*;
use crate::parse;

/// Helper: parse source then format it.
fn roundtrip(source: &str) -> String {
    let module = parse(source).expect("parse failed");
    format_module(&module)
}

#[test]
fn test_simple_function() {
    let src = "fn add(a: I64, b: I64) -> I64 { a + b }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_unit_value_roundtrips() {
    let src = "fn main() -> () { () }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_function_with_outcome_roundtrips() {
    let src = "fn risky(path: Str) -> Str ! ReadError { path }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_struct_def() {
    let src = "struct Point { x: I64, y: I64 }\n";
    let out = roundtrip(src);
    assert!(out.contains("struct Point {"));
    assert!(out.contains("x: I64,"));
    assert!(out.contains("y: I64,"));
}

#[test]
fn test_type_def_and_match() {
    let src = concat!(
        "enum Shape {\n",
        "    Circle(I64),\n",
        "    Rect(I64, I64),\n",
        "}\n",
    );
    let out = roundtrip(src);
    assert!(out.contains("enum Shape {"));
    assert!(out.contains("Circle(I64),"));
    assert!(out.contains("Rect(I64, I64),"));
}

#[test]
fn test_pipe_operator() {
    let src = "fn main() -> I64 { 10 |> double }\n";
    let out = roundtrip(src);
    assert!(out.contains("10 |> double"));
}

#[test]
fn test_lambda() {
    let src = "fn apply(f: (I64) -> I64, x: I64) -> I64 { f(x) }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_receiver_self_shorthand_roundtrips() {
    let src = "trait Show {\n    fn show(self) -> Str;\n}\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_generic_surface_and_impl_roundtrip() {
    let src = concat!(
        "surface StateIO[T] = [State[T], Log]\n",
        "\n",
        "impl[T: Eq + Hash] Set[T] {\n",
        "    fn contains(self, item: T) -> Bool;\n",
        "}\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_multi_statement_block() {
    let src = concat!(
        "fn main() -> I64 {\n",
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
        "fn area(s: Shape) -> I64 {\n",
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
        "fn area(s: Shape) -> I64 { match s {\n",
        "    Circle(r) => r * r * 3,\n",
        "    Rect(w, h) => w * h,\n",
        "} }\n",
    );
    let expected = concat!(
        "fn area(s: Shape) -> I64 {\n",
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
    let src = "fn read() -> Str uses [IO, FileRead] { ?todo }\n";
    let out = roundtrip(src);
    assert!(out.contains("uses [IO, FileRead]"));
}

#[test]
fn test_hole_syntax_roundtrip() {
    let src = "fn f(x: ?) -> ? { ?todo: I64 }\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_foreign_attribute_roundtrips() {
    let src = "@foreign\nfn c_add(a: I64, b: I64) -> I64;\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_foreign_attribute_with_uses_roundtrips() {
    let src = "@foreign\nfn read_file(path: Str) -> Str uses [FileRead];\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_foreign_attribute_with_outcome_roundtrips() {
    let src = "@foreign(\"host\", name = \"process_run\")\nfn process_run(cmd: Str, args: List[Str]) -> Str ! ProcessError uses [Spawn];\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_foreign_opaque_type_roundtrips() {
    let src = "@foreign\ntype Map[K, V];\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_generic_alias_roundtrips() {
    let src = "type PairOf[T] = Pair[T, T]\n";
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_budgeted_fn_roundtrips() {
    let src = concat!(
        "fn wild(n: I32) -> I32\n",
        "budget {\n",
        "    recursion: 0\n",
        "    calls: 1\n",
        "}\n",
        "{ n }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_intent_signature_budget_properties_and_inline_bounds_roundtrip() {
    let src = concat!(
        "fn member[T: Eq](xs: List[T], value: T) -> Bool uses [Console]\n",
        "budget {\n",
        "    branches: 2\n",
        "    holes: 0\n",
        "}\n",
        "properties {\n",
        "    empty(): true\n",
        "    agrees(xs: List[T]): true\n",
        "}\n",
        "{ contains(xs, value) }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_intent_clause_order_preserves_property_order() {
    let src = concat!(
        "fn show[T: Display](x: T) -> T uses [Console]\n",
        "budget {\n",
        "    calls: 1\n",
        "}\n",
        "properties {\n",
        "    roundtrip(x: T): true\n",
        "    stable(x: T): true\n",
        "}\n",
        "{ x }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_refinement_type_roundtrips_in_property_params() {
    let src = concat!(
        "fn abs(x: I32) -> I32\n",
        "properties {\n",
        "    non_negative_identity(x: I32 when self >= 0): x >= 0\n",
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
        "properties {\n",
        "    left_identity(a: I32, b: I32): add(0, b) == b\n",
        "    right_identity(a: I32, b: I32): add(a, 0) == a\n",
        "}\n",
        "{ a + b }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_blank_line_between_items() {
    let src = concat!("fn a() -> I64 { 1 }\n", "\n", "fn b() -> I64 { 2 }\n",);
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_const_def() {
    let src = "const MAX: I64 = 100\n";
    let out = roundtrip(src);
    assert!(out.contains("const MAX: I64 = 100"));
}

#[test]
fn test_keyword_item_forms_roundtrip() {
    let src = concat!(
        "trait Display[T] {\n",
        "    fn show(self: T) -> Str;\n",
        "}\n",
        "\n",
        "effect Console {\n",
        "    fn println(msg: Str) -> Unit;\n",
        "}\n",
        "\n",
        "surface IO = [Console, FileRead]\n",
        "\n",
        "handler MockConsole for Console {\n",
        "    fn Console.println(msg: Str) -> Unit {}\n",
        "}\n",
    );
    let out = roundtrip(src);
    assert!(out.contains("trait Display[T] {"));
    assert!(out.contains("effect Console {"));
    assert!(out.contains("surface IO = [Console, FileRead]"));
    assert!(out.contains("handler MockConsole for Console {"));
    assert!(out.contains("fn Console.println(msg: Str) -> Unit {\n    }"));
}

#[test]
fn test_doc_comment_preserved() {
    let src = concat!(
        "/// Adds two numbers.\n",
        "fn add(a: I64, b: I64) -> I64 { a + b }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_line_comment_between_items_preserved() {
    let src = concat!(
        "fn a() -> I64 { 1 }\n",
        "\n",
        "// helper\n",
        "fn b() -> I64 { 2 }\n",
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
    let src = concat!("/* block comment */\n", "fn f() -> I64 { 0 }\n",);
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_blank_line_between_comment_groups_preserved() {
    let src = concat!(
        "/// Module doc.\n",
        "\n",
        "/// Function doc.\n",
        "fn f() -> I64 { 0 }\n",
    );
    assert_eq!(roundtrip(src), src);
}

#[test]
fn test_trailing_comment_after_last_item() {
    let src = concat!("fn f() -> I64 { 0 }\n", "\n", "// end of file\n",);
    let out = roundtrip(src);
    assert!(out.contains("// end of file"));
}
