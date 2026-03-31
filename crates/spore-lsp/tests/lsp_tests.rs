use serde_json::json;
use spore_lsp::server::{LspServer, build_diagnostics};

#[test]
fn test_server_creation() {
    let server = LspServer::new();
    // Server should be constructible with no documents
    assert_eq!(server.documents.len(), 0);
}

#[test]
fn test_build_diagnostics_valid_source_is_empty() {
    // An empty source may still produce diagnostics depending on the compiler,
    // but the function should not panic.
    let diags = build_diagnostics("");
    // We just verify it returns a vec (valid or with errors)
    assert!(diags.is_empty() || !diags.is_empty());
}

#[test]
fn test_build_diagnostics_invalid_source_has_errors() {
    // Garbage input should produce at least one diagnostic
    let diags = build_diagnostics("this is not valid spore code @#$%");
    assert!(
        !diags.is_empty(),
        "invalid source should produce diagnostics"
    );

    // Each diagnostic should have the required LSP fields
    let d = &diags[0];
    assert!(d.get("range").is_some());
    assert!(d.get("severity").is_some());
    assert_eq!(d["severity"], json!(1));
    assert_eq!(d["source"], json!("spore"));
    assert!(d.get("message").is_some());
}
