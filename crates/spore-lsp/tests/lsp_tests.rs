use serde_json::json;
use spore_lsp::server::{
    LspServer, build_diagnostics, build_diagnostics_for_document, build_hover_for_position,
    build_hover_for_symbol, collect_document_symbols, find_definition_in_source, hole_at_position,
    word_at_position,
};

// ── Helpers ──────────────────────────────────────────────────────────

fn server_with_doc(uri: &str, source: &str) -> LspServer {
    let mut server = LspServer::new();
    server.documents.insert(uri.to_string(), source.to_string());
    server
}

const SAMPLE_SOURCE: &str = "\
fn add(a: I64, b: I64) -> I64 {
    a + b
}

/// Greet a user by name.
fn greet(name: Str) -> Str {
    f\"Hello, {name}!\"
}

struct Point {
    x: I64,
    y: I64,
}

enum Color {
    Red,
    Green,
    Blue,
}

trait Printable {
    fn to_string(self: Self) -> Str;
}

effect Memory {}

fn constrained(n: I64) -> I64
  uses [Memory]
  budget { calls: 0 }
{
    n
}
";

// ── Existing tests (kept) ────────────────────────────────────────────

#[test]
fn test_server_creation() {
    let server = LspServer::new();
    assert_eq!(server.documents.len(), 0);
}

#[test]
fn test_build_diagnostics_valid_source_is_empty() {
    let diags = build_diagnostics("");
    assert!(
        diags.is_empty(),
        "expected no diagnostics for valid source, got: {diags:?}"
    );
}

#[test]
fn test_build_diagnostics_invalid_source_has_errors() {
    let diags = build_diagnostics("this is not valid spore code @#$%");
    assert!(
        !diags.is_empty(),
        "invalid source should produce diagnostics"
    );
    let d = &diags[0];
    assert!(d.get("range").is_some());
    assert!(d.get("severity").is_some());
    assert_eq!(d["severity"], json!(1));
    assert_eq!(d["code"], json!("parse-error"));
    assert_eq!(d["source"], json!("spore"));
    assert!(d.get("message").is_some());
}

#[test]
fn test_build_diagnostics_for_document_preserves_document_uri() {
    let diags = build_diagnostics_for_document(
        "file:///workspace/main.sp",
        "fn main() -> I64 { \"oops\" }\n",
    );

    let d = &diags[0];
    assert_eq!(d["code"], json!("E0001"));
    assert_eq!(d["source"], json!("spore"));
}

// ── Completion tests ─────────────────────────────────────────────────

#[test]
fn test_completion_returns_keywords() {
    let server = server_with_doc("file:///test.sp", SAMPLE_SOURCE);
    let params = json!({
        "textDocument": { "uri": "file:///test.sp" },
        "position": { "line": 0, "character": 0 }
    });
    let result = server
        .handle_completion(&params)
        .expect("completion should return Some");
    let items = result.as_array().expect("completion should return array");

    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    for kw in &[
        "fn", "let", "type", "enum", "struct", "trait", "effect", "match", "if", "import",
    ] {
        assert!(labels.contains(kw), "missing keyword: {kw}");
    }
    // Keywords should have kind 14
    let kw_item = items.iter().find(|i| i["label"] == "fn").unwrap();
    assert_eq!(kw_item["kind"], json!(14));
}

#[test]
fn test_completion_returns_defined_functions() {
    let server = server_with_doc("file:///test.sp", SAMPLE_SOURCE);
    let params = json!({
        "textDocument": { "uri": "file:///test.sp" },
        "position": { "line": 0, "character": 0 }
    });
    let result = server.handle_completion(&params).unwrap();
    let items = result.as_array().unwrap();
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"add"), "should contain function 'add'");
    assert!(labels.contains(&"greet"), "should contain function 'greet'");
}

#[test]
fn test_completion_returns_builtins() {
    let server = server_with_doc("file:///test.sp", SAMPLE_SOURCE);
    let params = json!({
        "textDocument": { "uri": "file:///test.sp" },
        "position": { "line": 0, "character": 0 }
    });
    let result = server.handle_completion(&params).unwrap();
    let items = result.as_array().unwrap();
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    for b in &["map", "filter", "fold", "len"] {
        assert!(labels.contains(b), "missing builtin: {b}");
    }
}

// ── Goto Definition tests ────────────────────────────────────────────

#[test]
fn test_goto_definition_function() {
    let server = server_with_doc("file:///test.sp", SAMPLE_SOURCE);
    let params = json!({
        "textDocument": { "uri": "file:///test.sp" },
        "position": { "line": 0, "character": 4 }  // on 'add'
    });
    let result = server.handle_goto_definition(&params).unwrap();
    assert!(!result.is_null(), "should find definition of 'add'");
    assert_eq!(result["range"]["start"]["line"], json!(0));
    assert_eq!(result["range"]["start"]["character"], json!(3)); // after 'fn '
}

#[test]
fn test_goto_definition_type() {
    let server = server_with_doc("file:///test.sp", SAMPLE_SOURCE);
    // Find position of "Color" in source
    let pos = find_definition_in_source(SAMPLE_SOURCE, "Color");
    assert!(pos.is_some(), "should find Color definition");
    let (line, col) = pos.unwrap();

    let params = json!({
        "textDocument": { "uri": "file:///test.sp" },
        "position": { "line": line, "character": col }
    });
    let result = server.handle_goto_definition(&params).unwrap();
    assert!(!result.is_null(), "should find definition of 'Color'");
    assert_eq!(result["range"]["start"]["line"], json!(line));
}

#[test]
fn test_goto_definition_unknown_symbol() {
    let server = server_with_doc("file:///test.sp", "fn main() -> I64 { 0 }");
    let params = json!({
        "textDocument": { "uri": "file:///test.sp" },
        "position": { "line": 100, "character": 0 }
    });
    let result = server.handle_goto_definition(&params).unwrap();
    assert!(result.is_null(), "unknown symbol should return null");
}

// ── Document Symbols tests ───────────────────────────────────────────

#[test]
fn test_document_symbols_functions() {
    let symbols = collect_document_symbols(SAMPLE_SOURCE);
    let fn_names: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == 12)
        .map(|s| s.name.as_str())
        .collect();
    assert!(fn_names.contains(&"add"), "should contain function 'add'");
    assert!(
        fn_names.contains(&"greet"),
        "should contain function 'greet'"
    );
    assert!(
        fn_names.contains(&"constrained"),
        "should contain function 'constrained'"
    );
}

#[test]
fn test_document_symbols_structs() {
    let symbols = collect_document_symbols(SAMPLE_SOURCE);
    let struct_names: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == 23)
        .map(|s| s.name.as_str())
        .collect();
    assert!(
        struct_names.contains(&"Point"),
        "should contain struct 'Point'"
    );
}

#[test]
fn test_document_symbols_types() {
    let symbols = collect_document_symbols(SAMPLE_SOURCE);
    let type_names: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == 10)
        .map(|s| s.name.as_str())
        .collect();
    assert!(type_names.contains(&"Color"), "should contain type 'Color'");
}

#[test]
fn test_build_diagnostics_capability_item_reports_generic_parse_error() {
    let diags = build_diagnostics("capability Display { fn show(self: Self) -> Str }");
    assert!(
        diags.iter().any(|diag| diag["message"]
            .as_str()
            .unwrap_or_default()
            .contains("expected item")),
        "expected generic parse diagnostic, got: {diags:?}"
    );
}

// ── Hover tests ──────────────────────────────────────────────────────

#[test]
fn test_hover_function_signature() {
    let hover = build_hover_for_symbol(SAMPLE_SOURCE, "add");
    assert!(hover.is_some(), "should have hover for 'add'");
    let text = hover.unwrap();
    assert!(
        text.contains("fn add(a: I64, b: I64) -> I64"),
        "hover should show signature, got: {text}"
    );
}

#[test]
fn test_hover_with_budget_annotation() {
    let hover = build_hover_for_symbol(SAMPLE_SOURCE, "constrained");
    assert!(hover.is_some(), "should have hover for 'constrained'");
    let text = hover.unwrap();
    assert!(
        text.contains("budget"),
        "hover should mention budget, got: {text}"
    );
    assert!(
        text.contains("calls: 0"),
        "hover should show budget field, got: {text}"
    );
}

#[test]
fn test_hover_with_doc_comment() {
    let hover = build_hover_for_symbol(SAMPLE_SOURCE, "greet");
    assert!(hover.is_some(), "should have hover for 'greet'");
    let text = hover.unwrap();
    assert!(
        text.contains("Greet a user by name"),
        "hover should include doc comment, got: {text}"
    );
}

#[test]
fn test_hover_returns_hole_information() {
    let source = "fn fill() -> I64 { ?todo }\n";
    let server = server_with_doc("file:///test.sp", source);
    let params = json!({
        "textDocument": { "uri": "file:///test.sp" },
        "position": { "line": 0, "character": 20 }
    });

    let result = server.handle_hover(&params).expect("hover result");
    let text = result["contents"]["value"]
        .as_str()
        .expect("hover markdown text");

    assert!(
        text.contains("Typed hole"),
        "expected hole hover, got: {text}"
    );
    assert!(
        text.contains("?todo : I64"),
        "expected hole type, got: {text}"
    );
    assert!(
        text.contains("fill"),
        "expected function context, got: {text}"
    );
}

#[test]
fn test_hover_prefers_hole_in_current_function() {
    let source = "\
fn first() -> I64 { ?todo }
fn second() -> Str { ?todo }
";

    let first = build_hover_for_position(source, 0, 20).expect("first hole hover");
    let second = build_hover_for_position(source, 1, 26).expect("second hole hover");

    assert!(
        first.contains("?todo : I64"),
        "expected first hole type, got: {first}"
    );
    assert!(
        second.contains("?todo : Str"),
        "expected second hole type, got: {second}"
    );
}

#[test]
fn test_hover_distinguishes_named_holes_in_same_function() {
    let source = "fn pair() -> I64 { let lhs = ?todo; let rhs = ?todo; lhs }\n";
    let positions: Vec<(u32, u32)> = source
        .lines()
        .enumerate()
        .flat_map(|(line, text)| {
            text.match_indices("?todo")
                .map(move |(col, _)| (line as u32, col as u32))
        })
        .collect();

    let first =
        build_hover_for_position(source, positions[0].0, positions[0].1).expect("first named hole");
    let second = build_hover_for_position(source, positions[1].0, positions[1].1)
        .expect("second named hole");

    assert!(
        !first.contains("lhs: I64"),
        "first named hole should not include later binding, got: {first}"
    );
    assert!(
        second.contains("lhs: I64"),
        "second named hole should include prior binding, got: {second}"
    );
}

#[test]
fn test_hover_distinguishes_unnamed_holes_in_same_function() {
    let source = "fn pair() -> I64 { let a = ?; let b = ?; a }\n";

    let hole_lines: Vec<(u32, u32)> = source
        .lines()
        .enumerate()
        .flat_map(|(line, text)| {
            text.match_indices('?')
                .map(move |(col, _)| (line as u32, col as u32))
        })
        .collect();

    let first = build_hover_for_position(source, hole_lines[0].0, hole_lines[0].1)
        .expect("first unnamed hole hover");
    let second = build_hover_for_position(source, hole_lines[1].0, hole_lines[1].1)
        .expect("second unnamed hole hover");

    assert!(
        first.contains("? : I64"),
        "expected first hole hover, got: {first}"
    );
    assert!(
        second.contains("? : I64"),
        "expected second hole hover, got: {second}"
    );
    assert!(
        !first.contains("a: I64"),
        "first hole should not include later binding, got: {first}"
    );
    assert!(
        second.contains("a: I64"),
        "second hole should include prior binding, got: {second}"
    );
}

#[test]
fn test_hover_shows_hole_context() {
    let source = r#"
fn main() -> I64 {
    let seed = 2;
    ?todo
}
"#;
    let line = source
        .lines()
        .enumerate()
        .find_map(|(index, text)| text.find("?todo").map(|col| (index as u32, col as u32)))
        .expect("hole position");
    let hover = build_hover_for_position(source, line.0, line.1).expect("hole hover");

    assert!(
        hover.contains("?todo : I64"),
        "expected hole type, got: {hover}"
    );
    assert!(
        hover.contains("seed: I64"),
        "expected visible binding, got: {hover}"
    );
}

#[test]
fn test_hover_shows_typed_hole_annotation() {
    let source = r#"
fn main() -> I64 {
    ?todo: I64
}
"#;
    let line = source
        .lines()
        .enumerate()
        .find_map(|(index, text)| text.find("?todo").map(|col| (index as u32, col as u32)))
        .expect("hole position");
    let hover = build_hover_for_position(source, line.0, line.1).expect("hole hover");

    assert!(
        hover.contains("?todo : I64"),
        "expected typed hole hover, got: {hover}"
    );
}

#[test]
fn test_hover_shows_handler_discharge_context() {
    let source = r#"
effect Console {
    fn println(msg: Str) -> ();
}
effect Clock {
    fn now() -> I64;
}
surface IO = [Console, Clock]
fn main() -> I64 uses IO {
    handle {
        ?todo
    } with {
        on Console.println(msg) => { msg; }
    }
}
"#;
    let line = source
        .lines()
        .enumerate()
        .find_map(|(index, text)| text.find("?todo").map(|col| (index as u32, col as u32)))
        .expect("hole position");
    let hover = build_hover_for_position(source, line.0, line.1).expect("hole hover");

    assert!(
        hover.contains("Discharged by enclosing handlers"),
        "expected handler discharge section, got: {hover}"
    );
    assert!(
        hover.contains("Effects after handler discharge"),
        "expected post-discharge effects section, got: {hover}"
    );
}

// ── word_at_position tests ───────────────────────────────────────────

#[test]
fn test_word_at_position_basic() {
    let source = "fn hello(x: I64) -> I64 { x }";
    assert_eq!(word_at_position(source, 0, 3), "hello");
    assert_eq!(word_at_position(source, 0, 0), "fn");
    assert_eq!(word_at_position(source, 0, 9), "x");
}

#[test]
fn test_word_at_position_out_of_bounds() {
    let source = "fn test() {}";
    assert_eq!(word_at_position(source, 99, 0), "");
}

#[test]
fn test_hole_at_position_named_hole() {
    let source = "fn fill() -> I64 { ?todo }";
    let hole = hole_at_position(source, 0, 20).expect("hole at position");
    assert_eq!(hole.display_name, "?todo");
    assert_eq!(hole.name.as_deref(), Some("todo"));
}

#[test]
fn test_hole_at_position_unnamed_hole() {
    let source = "fn fill() -> I64 { ? }";
    let hole = hole_at_position(source, 0, 20).expect("hole at position");
    assert_eq!(hole.display_name, "?");
    assert_eq!(hole.name, None);
}

#[test]
fn test_hole_at_position_before_hole_returns_none() {
    let source = "fn fill() -> I64 {  ?todo }";
    let hole_col = source.find("?todo").expect("hole start") as u32;
    assert!(hole_at_position(source, 0, hole_col - 1).is_none());
}

// ── Safety tests (no panics on malformed input) ──────────────────────

#[test]
fn test_malformed_request_no_panic() {
    let server = server_with_doc("file:///test.sp", SAMPLE_SOURCE);
    let params = json!({});
    assert_eq!(server.handle_hover(&params), None);
    assert_eq!(server.handle_goto_definition(&params), None);
    assert_eq!(server.handle_document_symbol(&params), None);
    // Completion still returns keywords even with empty params
    let completion = server.handle_completion(&params);
    assert!(completion.is_some());
}

#[test]
fn test_invalid_uri_no_panic() {
    let server = server_with_doc("file:///test.sp", SAMPLE_SOURCE);
    let params = json!({
        "textDocument": { "uri": "file:///nonexistent.sp" },
        "position": { "line": 0, "character": 0 }
    });
    assert_eq!(server.handle_goto_definition(&params), None);
    assert_eq!(server.handle_hover(&params), None);
    assert_eq!(server.handle_document_symbol(&params), None);
}

#[test]
fn test_missing_position_no_panic() {
    let server = server_with_doc("file:///test.sp", SAMPLE_SOURCE);
    let params = json!({
        "textDocument": { "uri": "file:///test.sp" }
    });
    // No position → goto_definition and hover return None
    assert_eq!(server.handle_goto_definition(&params), None);
    assert_eq!(server.handle_hover(&params), None);
    // Completion still works (doesn't need position)
    let completion = server.handle_completion(&params);
    assert!(completion.is_some());
}
