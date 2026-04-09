use serde_json::json;
use spore_lsp::server::{
    LspServer, build_diagnostics, build_hover_for_symbol, collect_document_symbols,
    find_definition_in_source, word_at_position,
};

// ── Helpers ──────────────────────────────────────────────────────────

fn server_with_doc(uri: &str, source: &str) -> LspServer {
    let mut server = LspServer::new();
    server.documents.insert(uri.to_string(), source.to_string());
    server
}

const SAMPLE_SOURCE: &str = "\
fn add(a: Int, b: Int) -> Int {
    a + b
}

/// Greet a user by name.
fn greet(name: String) -> String {
    f\"Hello, {name}!\"
}

struct Point {
    x: Int,
    y: Int,
}

type Color {
    Red,
    Green,
    Blue,
}

trait Printable {
    fn to_string(self: Self) -> String
}

fn expensive(n: Int) -> Int
  cost [100, 0, 0, 0]
  uses [Memory] {
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
    assert_eq!(d["source"], json!("spore"));
    assert!(d.get("message").is_some());
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
        "fn", "let", "type", "struct", "trait", "effect", "match", "if", "import",
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
    for b in &["print", "println", "map", "filter", "fold", "len"] {
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
    let server = server_with_doc("file:///test.sp", "fn main() -> Int { 0 }");
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
        fn_names.contains(&"expensive"),
        "should contain function 'expensive'"
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
fn test_build_diagnostics_removed_capability_syntax_is_reported() {
    let diags = build_diagnostics("capability Display { fn show(self: Self) -> String }");
    assert!(
        diags.iter().any(|diag| diag["message"]
            .as_str()
            .unwrap_or_default()
            .contains("legacy `capability` syntax has been removed")),
        "expected removed capability diagnostic, got: {diags:?}"
    );
}

// ── Hover tests ──────────────────────────────────────────────────────

#[test]
fn test_hover_function_signature() {
    let hover = build_hover_for_symbol(SAMPLE_SOURCE, "add");
    assert!(hover.is_some(), "should have hover for 'add'");
    let text = hover.unwrap();
    assert!(
        text.contains("fn add(a: Int, b: Int) -> Int"),
        "hover should show signature, got: {text}"
    );
}

#[test]
fn test_hover_with_cost_annotation() {
    let hover = build_hover_for_symbol(SAMPLE_SOURCE, "expensive");
    assert!(hover.is_some(), "should have hover for 'expensive'");
    let text = hover.unwrap();
    assert!(
        text.contains("Cost"),
        "hover should mention cost, got: {text}"
    );
    assert!(
        text.contains("100"),
        "hover should show cost value, got: {text}"
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

// ── word_at_position tests ───────────────────────────────────────────

#[test]
fn test_word_at_position_basic() {
    let source = "fn hello(x: Int) -> Int { x }";
    assert_eq!(word_at_position(source, 0, 3), "hello");
    assert_eq!(word_at_position(source, 0, 0), "fn");
    assert_eq!(word_at_position(source, 0, 9), "x");
}

#[test]
fn test_word_at_position_out_of_bounds() {
    let source = "fn test() {}";
    assert_eq!(word_at_position(source, 99, 0), "");
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
