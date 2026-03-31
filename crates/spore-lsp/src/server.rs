use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};

use serde_json::{Value, json};

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
                    self.send_response(
                        id.unwrap(),
                        json!({
                            "capabilities": {
                                "textDocumentSync": {
                                    "openClose": true,
                                    "change": 1,
                                    "save": { "includeText": true }
                                },
                                "hoverProvider": true
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
                    self.send_response(id.unwrap(), json!(null));
                }
                Some("shutdown") => {
                    self.send_response(id.unwrap(), json!(null));
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
        let body = serde_json::to_string(msg).unwrap();
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
        let diagnostics = build_diagnostics(source);
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

/// Build LSP diagnostics from source text by running the Spore compiler.
/// Returns a `Vec<Value>` of LSP Diagnostic objects.
pub fn build_diagnostics(source: &str) -> Vec<Value> {
    match sporec::compile(source) {
        Ok(()) => vec![],
        Err(err_msg) => err_msg
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                json!({
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 0 }
                    },
                    "severity": 1,
                    "source": "spore",
                    "message": line
                })
            })
            .collect(),
    }
}
