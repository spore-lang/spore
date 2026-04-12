use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};

use serde_json::{Value, json};
use sporec_parser::ast::{CostExpr, FnDef, Item, TypeExpr};

// ── LSP Symbol Kind constants ────────────────────────────────────────
const SK_FUNCTION: u32 = 12;
const SK_STRUCT: u32 = 23;
const SK_ENUM: u32 = 10;
const SK_INTERFACE: u32 = 11;
const SK_CONSTANT: u32 = 14;

// ── Spore keywords & builtins ────────────────────────────────────────
const KEYWORDS: &[&str] = &[
    "fn",
    "let",
    "type",
    "struct",
    "trait",
    "effect",
    "match",
    "if",
    "import",
    "pub",
    "foreign",
    "perform",
    "handle",
    "const",
    "return",
    "else",
    "where",
    "cost",
    "uses",
    "spawn",
    "await",
    "impl",
    "alias",
    "mod",
    "pkg",
    "in",
    "self",
    "from",
    "when",
    "select",
    "throw",
    "parallel_scope",
];

const BUILTINS: &[&str] = &[
    "print",
    "println",
    "read_line",
    "map",
    "filter",
    "fold",
    "len",
    "push",
    "pop",
    "head",
    "tail",
    "concat",
    "sort",
    "reverse",
    "zip",
    "enumerate",
    "any",
    "all",
    "find",
    "to_string",
];

// ── Collected symbol info ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: u32,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub detail: Option<String>,
}

pub struct LspServer {
    pub documents: HashMap<String, String>,
}

impl LspServer {
    pub fn new() -> Self {
        LspServer {
            documents: HashMap::new(),
        }
    }

    pub fn run(&mut self) {
        while let Some(msg) = self.read_message() {
            let method = msg.get("method").and_then(|m| m.as_str());
            let id = msg.get("id").cloned();

            match method {
                Some("initialize") => {
                    let Some(id) = id else { continue };
                    self.send_response(
                        id,
                        json!({
                            "capabilities": {
                                "textDocumentSync": {
                                    "openClose": true,
                                    "change": 1,
                                    "save": { "includeText": true }
                                },
                                "hoverProvider": true,
                                "completionProvider": {
                                    "triggerCharacters": [".", ":"]
                                },
                                "definitionProvider": true,
                                "documentSymbolProvider": true
                            },
                            "serverInfo": {
                                "name": "spore-lsp",
                                "version": "0.1.0"
                            }
                        }),
                    );
                }
                Some("initialized") => {}
                Some("textDocument/didOpen") => {
                    if let Some(params) = msg.get("params")
                        && let Some(doc) = params.get("textDocument")
                    {
                        let uri = doc["uri"].as_str().unwrap_or("").to_string();
                        let text = doc["text"].as_str().unwrap_or("").to_string();
                        self.documents.insert(uri.clone(), text.clone());
                        self.publish_diagnostics(&uri, &text);
                    }
                }
                Some("textDocument/didChange") => {
                    if let Some(params) = msg.get("params") {
                        let uri = params["textDocument"]["uri"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        if let Some(changes) = params["contentChanges"].as_array()
                            && let Some(last) = changes.last()
                        {
                            let text = last["text"].as_str().unwrap_or("").to_string();
                            self.documents.insert(uri.clone(), text.clone());
                            self.publish_diagnostics(&uri, &text);
                        }
                    }
                }
                Some("textDocument/didSave") => {
                    if let Some(params) = msg.get("params") {
                        let uri = params["textDocument"]["uri"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        if let Some(text) = params.get("text").and_then(|t| t.as_str()) {
                            self.documents.insert(uri.clone(), text.to_string());
                            self.publish_diagnostics(&uri, text);
                        } else if let Some(text) = self.documents.get(&uri).cloned() {
                            self.publish_diagnostics(&uri, &text);
                        }
                    }
                }
                Some("textDocument/hover") => {
                    let Some(id) = id else { continue };
                    let result = msg
                        .get("params")
                        .and_then(|p| self.handle_hover(p))
                        .unwrap_or(json!(null));
                    self.send_response(id, result);
                }
                Some("textDocument/completion") => {
                    let Some(id) = id else { continue };
                    let result = msg
                        .get("params")
                        .and_then(|p| self.handle_completion(p))
                        .unwrap_or(json!(null));
                    self.send_response(id, result);
                }
                Some("textDocument/definition") => {
                    let Some(id) = id else { continue };
                    let result = msg
                        .get("params")
                        .and_then(|p| self.handle_goto_definition(p))
                        .unwrap_or(json!(null));
                    self.send_response(id, result);
                }
                Some("textDocument/documentSymbol") => {
                    let Some(id) = id else { continue };
                    let result = msg
                        .get("params")
                        .and_then(|p| self.handle_document_symbol(p))
                        .unwrap_or(json!(null));
                    self.send_response(id, result);
                }
                Some("shutdown") => {
                    let Some(id) = id else { continue };
                    self.send_response(id, json!(null));
                }
                Some("exit") => {
                    break;
                }
                _ => {
                    if let Some(id) = id {
                        self.send_error(id, -32601, "method not found");
                    }
                }
            }
        }
    }

    // ── Completion ───────────────────────────────────────────────────

    pub fn handle_completion(&self, params: &Value) -> Option<Value> {
        let source = self
            .extract_text_document_uri(params)
            .map(|(_, s)| s)
            .unwrap_or_default();

        let mut items: Vec<Value> = Vec::new();

        // Keywords (kind 14 = Keyword)
        for kw in KEYWORDS {
            items.push(json!({
                "label": kw,
                "kind": 14,
                "detail": "keyword",
            }));
        }

        // Builtins (kind 3 = Function)
        for b in BUILTINS {
            items.push(json!({
                "label": b,
                "kind": 3,
                "detail": "builtin",
            }));
        }

        // Symbols from AST
        if let Ok(module) = sporec_parser::parse(&source) {
            for item in &module.items {
                match item {
                    Item::Function(f) => {
                        let sig = format_fn_signature(f);
                        items.push(json!({
                            "label": &f.name,
                            "kind": 3,
                            "detail": sig,
                        }));
                    }
                    Item::StructDef(s) => {
                        items.push(json!({
                            "label": &s.name,
                            "kind": 22, // Struct
                            "detail": "struct",
                        }));
                    }
                    Item::TypeDef(t) => {
                        items.push(json!({
                            "label": &t.name,
                            "kind": 10, // Enum
                            "detail": "type",
                        }));
                    }
                    Item::TraitDef(t) => {
                        items.push(json!({
                            "label": &t.name,
                            "kind": 8, // Interface
                            "detail": "trait",
                        }));
                    }
                    Item::EffectDef(e) => {
                        items.push(json!({
                            "label": &e.name,
                            "kind": 8, // Interface
                            "detail": "effect",
                        }));
                    }
                    Item::HandlerDef(h) => {
                        items.push(json!({
                            "label": &h.name,
                            "kind": 6, // Method
                            "detail": format!("handler for {}", h.effect),
                        }));
                    }
                    Item::Const(c) => {
                        items.push(json!({
                            "label": &c.name,
                            "kind": 21, // Constant
                            "detail": "const",
                        }));
                    }
                    _ => {}
                }
            }
        }

        Some(json!(items))
    }

    // ── Goto Definition ──────────────────────────────────────────────

    pub fn handle_goto_definition(&self, params: &Value) -> Option<Value> {
        let (uri, source, line, col) = self.extract_text_document_params(params)?;

        let word = word_at_position(&source, line, col);
        if word.is_empty() {
            return Some(json!(null));
        }

        if let Some(pos) = find_definition_in_source(&source, &word) {
            return Some(json!({
                "uri": uri,
                "range": {
                    "start": { "line": pos.0, "character": pos.1 },
                    "end": { "line": pos.0, "character": pos.1 + word.len() as u32 }
                }
            }));
        }

        Some(json!(null))
    }

    // ── Document Symbols ─────────────────────────────────────────────

    pub fn handle_document_symbol(&self, params: &Value) -> Option<Value> {
        let (_, source) = self.extract_text_document_uri(params)?;

        let symbols = collect_document_symbols(&source);
        let items: Vec<Value> = symbols
            .iter()
            .map(|s| {
                let range = json!({
                    "start": { "line": s.line, "character": s.col },
                    "end": { "line": s.end_line, "character": s.end_col }
                });
                json!({
                    "name": s.name,
                    "kind": s.kind,
                    "range": range,
                    "selectionRange": range,
                    "detail": s.detail,
                })
            })
            .collect();
        Some(json!(items))
    }

    // ── Hover ────────────────────────────────────────────────────────

    pub fn handle_hover(&self, params: &Value) -> Option<Value> {
        let (_, source, line, col) = self.extract_text_document_params(params)?;

        let word = word_at_position(&source, line, col);
        if word.is_empty() {
            return Some(json!(null));
        }

        if let Some(hover) = build_hover_for_symbol(&source, &word) {
            return Some(json!({
                "contents": {
                    "kind": "markdown",
                    "value": hover
                }
            }));
        }

        Some(json!(null))
    }

    // ── Param extraction helpers ────────────────────────────────────

    fn extract_text_document_uri(&self, params: &Value) -> Option<(String, String)> {
        let uri = params.get("textDocument")?.get("uri")?.as_str()?;
        let source = self.documents.get(uri)?.clone();
        Some((uri.to_string(), source))
    }

    fn extract_text_document_params(&self, params: &Value) -> Option<(String, String, u32, u32)> {
        let (uri, source) = self.extract_text_document_uri(params)?;
        let line = params.get("position")?.get("line")?.as_u64()? as u32;
        let col = params.get("position")?.get("character")?.as_u64()? as u32;
        Some((uri, source, line, col))
    }

    // ── I/O helpers ──────────────────────────────────────────────────

    fn read_message(&self) -> Option<Value> {
        let stdin = io::stdin();
        let mut reader = stdin.lock();

        let mut content_length: usize = 0;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).ok()? == 0 {
                return None;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(len) = trimmed.strip_prefix("Content-Length: ") {
                content_length = len.parse().ok()?;
            }
        }

        if content_length == 0 {
            return None;
        }

        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body).ok()?;

        serde_json::from_slice(&body).ok()
    }

    fn send_message(&self, msg: &Value) {
        let body = match serde_json::to_string(msg) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("LSP serialize error: {e}");
                return;
            }
        };
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let _ = out.write_all(header.as_bytes());
        let _ = out.write_all(body.as_bytes());
        let _ = out.flush();
    }

    fn send_response(&self, id: Value, result: Value) {
        self.send_message(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        }));
    }

    fn send_error(&self, id: Value, code: i32, message: &str) {
        self.send_message(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message }
        }));
    }

    fn send_notification(&self, method: &str, params: Value) {
        self.send_message(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }));
    }

    fn publish_diagnostics(&self, uri: &str, source: &str) {
        let diagnostics = build_diagnostics_for_document(uri, source);
        self.send_notification(
            "textDocument/publishDiagnostics",
            json!({
                "uri": uri,
                "diagnostics": diagnostics
            }),
        );
    }
}

impl Default for LspServer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Free functions (public for testing) ──────────────────────────────

/// Build LSP diagnostics from source text by running the Spore compiler.
pub fn build_diagnostics(source: &str) -> Vec<Value> {
    build_diagnostics_for_document("file:///buffer.sp", source)
}

pub fn build_diagnostics_for_document(uri: &str, source: &str) -> Vec<Value> {
    let (source_file, diagnostics) = match sporec::check_source_file(uri, source) {
        sporec::SourceCheckReport::Success { source, warnings } => (source, warnings),
        sporec::SourceCheckReport::Failure(sporec::SourceCheckFailure::Diagnostics {
            source,
            diagnostics,
        }) => (source, diagnostics),
        sporec::SourceCheckReport::Failure(sporec::SourceCheckFailure::Message(message)) => {
            let source_file = sporec::source_file(uri, source);
            let diagnostic = sporec::Diagnostic::new(
                "lsp-diagnostic-message",
                sporec::CanonicalSeverity::Error,
                message,
            )
            .with_primary_span(source_file.span(0..0));
            (source_file, vec![diagnostic])
        }
    };

    sporec::lsp_diagnostics_for_source(&source_file, &diagnostics, uri)
        .into_iter()
        .map(|diagnostic| serde_json::to_value(diagnostic).expect("serialize lsp diagnostic"))
        .collect()
}

/// Extract the word (identifier) at a given (line, col) position.
pub fn word_at_position(source: &str, line: u32, col: u32) -> String {
    let Some(line_text) = source.lines().nth(line as usize) else {
        return String::new();
    };
    let col = col as usize;
    if col >= line_text.len() {
        return String::new();
    }
    let bytes = line_text.as_bytes();
    let mut start = col;
    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = col;
    while end < bytes.len() && is_ident_char(bytes[end]) {
        end += 1;
    }
    if start == end {
        return String::new();
    }
    line_text[start..end].to_string()
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Search source text for a definition of `name` (e.g. `fn name`, `type name`,
/// `struct name`, `trait name`, `effect name`). Returns `(line, col)` of the
/// name token.
pub fn find_definition_in_source(source: &str, name: &str) -> Option<(u32, u32)> {
    let prefixes = [
        "fn ",
        "type ",
        "struct ",
        "trait ",
        "effect ",
        "const ",
        "pub fn ",
        "pub type ",
        "pub struct ",
        "pub trait ",
        "pub effect ",
        "pub const ",
    ];
    for (line_no, line_text) in source.lines().enumerate() {
        let trimmed = line_text.trim_start();
        for prefix in &prefixes {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let def_name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if def_name == name {
                    // Find the column of the name within the original line
                    if let Some(idx) = line_text.find(&format!("{prefix}{name}")) {
                        let col = idx + prefix.len();
                        return Some((line_no as u32, col as u32));
                    }
                }
            }
        }
    }
    None
}

/// Collect all top-level symbol definitions with their positions.
pub fn collect_document_symbols(source: &str) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();

    // Parse AST to get symbol names and kinds
    let Ok(module) = sporec_parser::parse(source) else {
        return symbols;
    };

    for item in &module.items {
        let (name, kind, detail) = match item {
            Item::Function(f) => (f.name.clone(), SK_FUNCTION, Some(format_fn_signature(f))),
            Item::StructDef(s) => (s.name.clone(), SK_STRUCT, Some("struct".into())),
            Item::TypeDef(t) => (t.name.clone(), SK_ENUM, Some("type".into())),
            Item::TraitDef(t) => (t.name.clone(), SK_INTERFACE, Some("trait".into())),
            Item::EffectDef(e) => (e.name.clone(), SK_INTERFACE, Some("effect".into())),
            Item::HandlerDef(h) => (
                h.name.clone(),
                SK_FUNCTION,
                Some(format!("handler for {}", h.effect)),
            ),
            Item::Const(c) => (c.name.clone(), SK_CONSTANT, Some("const".into())),
            _ => continue,
        };

        if let Some((line, col)) = find_definition_in_source(source, &name) {
            symbols.push(SymbolInfo {
                end_line: line,
                end_col: col + name.len() as u32,
                name,
                kind,
                line,
                col,
                detail,
            });
        }
    }

    symbols
}

/// Build hover markdown for a symbol found in `source`.
pub fn build_hover_for_symbol(source: &str, name: &str) -> Option<String> {
    let module = sporec_parser::parse(source).ok()?;

    for item in &module.items {
        match item {
            Item::Function(f) if f.name == name => {
                let mut parts = Vec::new();

                // Doc comment
                if let Some(doc) = extract_doc_comment(source, name) {
                    parts.push(doc);
                    parts.push(String::new());
                }

                // Signature
                parts.push(format!("```spore\n{}\n```", format_fn_full(f)));

                // Cost annotation
                if let Some(ref cost) = f.cost_clause {
                    parts.push(format!(
                        "\n**Cost:** `cost [{}, {}, {}, {}]`",
                        format_cost_expr(&cost.compute),
                        format_cost_expr(&cost.alloc),
                        format_cost_expr(&cost.io),
                        format_cost_expr(&cost.parallel)
                    ));
                }

                // Uses clause
                if let Some(ref uses) = f.uses_clause {
                    parts.push(format!("\n**Uses:** `[{}]`", uses.resources.join(", ")));
                }

                return Some(parts.join("\n"));
            }
            Item::StructDef(s) if s.name == name => {
                let mut parts = Vec::new();
                if let Some(doc) = extract_doc_comment(source, name) {
                    parts.push(doc);
                    parts.push(String::new());
                }
                let fields: Vec<String> = s
                    .fields
                    .iter()
                    .map(|f| format!("    {}: {}", f.name, format_type_expr(&f.ty)))
                    .collect();
                parts.push(format!(
                    "```spore\nstruct {} {{\n{}\n}}\n```",
                    s.name,
                    fields.join(",\n")
                ));
                return Some(parts.join("\n"));
            }
            Item::TypeDef(t) if t.name == name => {
                let mut parts = Vec::new();
                if let Some(doc) = extract_doc_comment(source, name) {
                    parts.push(doc);
                    parts.push(String::new());
                }
                let variants: Vec<String> = t
                    .variants
                    .iter()
                    .map(|v| {
                        if v.fields.is_empty() {
                            format!("    {}", v.name)
                        } else {
                            let fs: Vec<String> = v.fields.iter().map(format_type_expr).collect();
                            format!("    {}({})", v.name, fs.join(", "))
                        }
                    })
                    .collect();
                parts.push(format!(
                    "```spore\ntype {} {{\n{}\n}}\n```",
                    t.name,
                    variants.join(",\n")
                ));
                return Some(parts.join("\n"));
            }
            Item::TraitDef(t) if t.name == name => {
                let mut parts = Vec::new();
                if let Some(doc) = extract_doc_comment(source, name) {
                    parts.push(doc);
                    parts.push(String::new());
                }
                let methods: Vec<String> = t
                    .methods
                    .iter()
                    .map(|m| format!("    {}", format_fn_signature(m)))
                    .collect();
                parts.push(format!(
                    "```spore\ntrait {} {{\n{}\n}}\n```",
                    t.name,
                    methods.join("\n")
                ));
                return Some(parts.join("\n"));
            }
            Item::EffectDef(e) if e.name == name => {
                let mut parts = Vec::new();
                if let Some(doc) = extract_doc_comment(source, name) {
                    parts.push(doc);
                    parts.push(String::new());
                }
                let ops: Vec<String> = e
                    .operations
                    .iter()
                    .map(|m| format!("    {}", format_fn_signature(m)))
                    .collect();
                parts.push(format!(
                    "```spore\neffect {} {{\n{}\n}}\n```",
                    e.name,
                    ops.join("\n")
                ));
                return Some(parts.join("\n"));
            }
            _ => {}
        }
    }

    None
}

/// Extract `///` doc comments immediately preceding a definition of `name`.
pub fn extract_doc_comment(source: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();

    // Find the definition line
    let def_line = find_definition_in_source(source, name)?.0 as usize;

    let mut doc_lines = Vec::new();
    let mut i = def_line;
    while i > 0 {
        i -= 1;
        let trimmed = lines[i].trim_start();
        if let Some(comment) = trimmed.strip_prefix("///") {
            doc_lines.push(comment.strip_prefix(' ').unwrap_or(comment).to_string());
        } else {
            break;
        }
    }

    if doc_lines.is_empty() {
        return None;
    }

    doc_lines.reverse();
    Some(doc_lines.join("\n"))
}

// ── Formatting helpers ───────────────────────────────────────────────

/// Short signature: `fn name(param: Type, ...) -> RetType`
pub fn format_fn_signature(f: &FnDef) -> String {
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, format_type_expr(&p.ty)))
        .collect();
    let ret = f
        .return_type
        .as_ref()
        .map(|t| format!(" -> {}", format_type_expr(t)))
        .unwrap_or_default();
    format!("fn {}({}){}", f.name, params.join(", "), ret)
}

/// Full signature with clauses.
fn format_fn_full(f: &FnDef) -> String {
    let mut sig = format_fn_signature(f);
    if let Some(ref cost) = f.cost_clause {
        sig.push_str(&format!(
            "\n  cost [{}, {}, {}, {}]",
            format_cost_expr(&cost.compute),
            format_cost_expr(&cost.alloc),
            format_cost_expr(&cost.io),
            format_cost_expr(&cost.parallel)
        ));
    }
    if let Some(ref uses) = f.uses_clause {
        sig.push_str(&format!("\n  uses [{}]", uses.resources.join(", ")));
    }
    sig
}

pub fn format_type_expr(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named(n) => n.clone(),
        TypeExpr::Hole(name) => format!("?{}", name.clone().unwrap_or_default()),
        TypeExpr::Generic(n, args) => {
            let a: Vec<String> = args.iter().map(format_type_expr).collect();
            format!("{}[{}]", n, a.join(", "))
        }
        TypeExpr::Tuple(elems) => {
            let e: Vec<String> = elems.iter().map(format_type_expr).collect();
            format!("({})", e.join(", "))
        }
        TypeExpr::Function(params, ret, _errors) => {
            let p: Vec<String> = params.iter().map(format_type_expr).collect();
            format!("({}) -> {}", p.join(", "), format_type_expr(ret))
        }
        TypeExpr::Refinement(base, binding, _pred) => {
            format!("{{ {}: {} when ... }}", binding, format_type_expr(base))
        }
        TypeExpr::Record(fields) => {
            let f: Vec<String> = fields
                .iter()
                .map(|(n, t)| format!("{}: {}", n, format_type_expr(t)))
                .collect();
            format!("{{ {} }}", f.join(", "))
        }
    }
}

pub fn format_cost_expr(cost: &CostExpr) -> String {
    match cost {
        CostExpr::Literal(n) => n.to_string(),
        CostExpr::Var(v) => v.clone(),
        CostExpr::Linear(v) => format!("O({v})"),
    }
}
