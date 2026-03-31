//! Hole report — structured output describing unfilled holes in a module.

use std::collections::BTreeMap;

use crate::types::Ty;

/// Information collected about a single hole.
#[derive(Debug, Clone)]
pub struct HoleInfo {
    /// Hole name (from `?name`)
    pub name: String,
    /// Inferred/expected type
    pub expected_type: Ty,
    /// The function this hole appears in
    pub function: String,
    /// Local bindings available at the hole site (name → type)
    pub bindings: BTreeMap<String, Ty>,
    /// Available functions that return the expected type
    pub suggestions: Vec<String>,
}

/// Collected report for all holes in a module.
#[derive(Debug, Clone, Default)]
pub struct HoleReport {
    pub holes: Vec<HoleInfo>,
}

impl HoleReport {
    pub fn new() -> Self {
        Self { holes: Vec::new() }
    }

    /// Serialize to JSON string (no serde dependency).
    pub fn to_json(&self) -> String {
        let mut out = String::from("{\n  \"holes\": [\n");
        for (i, h) in self.holes.iter().enumerate() {
            if i > 0 {
                out.push_str(",\n");
            }
            out.push_str("    {\n");
            out.push_str(&format!("      \"name\": {},\n", json_escape(&h.name)));
            out.push_str(&format!(
                "      \"expected_type\": {},\n",
                json_escape(&h.expected_type.to_string())
            ));
            out.push_str(&format!(
                "      \"function\": {},\n",
                json_escape(&h.function)
            ));
            out.push_str("      \"bindings\": {");
            for (j, (k, v)) in h.bindings.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!(
                    "{}: {}",
                    json_escape(k),
                    json_escape(&v.to_string())
                ));
            }
            out.push_str("},\n");
            out.push_str("      \"suggestions\": [");
            for (j, s) in h.suggestions.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_escape(s));
            }
            out.push_str("]\n");
            out.push_str("    }");
        }
        out.push_str("\n  ]\n}");
        out
    }
}

/// Escape a string for JSON output.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
