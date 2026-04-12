use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::io;
use std::ops::Range;
use std::path::Path;
use std::process::ExitCode;

use ariadne::{Color, Label, Report, ReportKind, Source};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    fn report_kind(self) -> ReportKind<'static> {
        match self {
            Severity::Error => ReportKind::Error,
            Severity::Warning => ReportKind::Warning,
            Severity::Note => ReportKind::Advice,
        }
    }

    fn color(self) -> Color {
        match self {
            Severity::Error => Color::Red,
            Severity::Warning => Color::Yellow,
            Severity::Note => Color::Blue,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticRange {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceSpan {
    pub file: String,
    pub range: DiagnosticRange,
    #[serde(skip_serializing, skip_deserializing)]
    byte_range: Option<Range<usize>>,
}

impl SourceSpan {
    pub fn byte_range(&self) -> Option<Range<usize>> {
        self.byte_range.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SecondaryLabel {
    pub span: SourceSpan,
    pub message: String,
}

impl SecondaryLabel {
    pub fn new(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelatedDiagnostic {
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl RelatedDiagnostic {
    pub fn new(message: impl Into<String>, span: Option<SourceSpan>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub primary_span: Option<SourceSpan>,
    pub secondary_labels: Vec<SecondaryLabel>,
    pub notes: Vec<String>,
    pub help: Option<String>,
    pub related: Vec<RelatedDiagnostic>,
}

impl Diagnostic {
    pub fn new(code: impl Into<String>, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity,
            message: message.into(),
            primary_span: None,
            secondary_labels: Vec::new(),
            notes: Vec::new(),
            help: None,
            related: Vec::new(),
        }
    }

    pub fn with_primary_span(mut self, span: SourceSpan) -> Self {
        self.primary_span = Some(span);
        self
    }

    pub fn with_secondary_label(mut self, label: SecondaryLabel) -> Self {
        self.secondary_labels.push(label);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_related(mut self, related: RelatedDiagnostic) -> Self {
        self.related.push(related);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportStatus {
    Ok,
    Error,
    Fail,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct JsonReport<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ReportStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<&'a Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<&'a [Diagnostic]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning_diagnostics: Option<&'a [Diagnostic]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<&'a [Diagnostic]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

impl<'a> JsonReport<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_event(mut self, event: &'static str) -> Self {
        self.event = Some(event);
        self
    }

    pub fn with_file(mut self, file: &'a str) -> Self {
        self.file = Some(file);
        self
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self
    }

    pub fn with_status(mut self, status: ReportStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_message(mut self, message: impl Into<Cow<'a, str>>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn with_diagnostic(mut self, diagnostic: &'a Diagnostic) -> Self {
        self.diagnostic = Some(diagnostic);
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: &'a [Diagnostic]) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    pub fn with_warnings(mut self, warnings: &'a [String]) -> Self {
        self.warnings = Some(warnings);
        self
    }

    pub fn with_warning_diagnostics(mut self, warning_diagnostics: &'a [Diagnostic]) -> Self {
        self.warning_diagnostics = Some(warning_diagnostics);
        self
    }

    pub fn with_errors(mut self, errors: &'a [Diagnostic]) -> Self {
        self.errors = Some(errors);
        self
    }

    pub fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HoleSummary {
    pub event: &'static str,
    pub holes_total: usize,
    pub filled_this_cycle: usize,
    pub ready_to_fill: usize,
    pub blocked: usize,
}

impl HoleSummary {
    pub fn new(
        holes_total: usize,
        filled_this_cycle: usize,
        ready_to_fill: usize,
        blocked: usize,
    ) -> Self {
        Self {
            event: "hole_graph_update",
            holes_total,
            filled_this_cycle,
            ready_to_fill,
            blocked,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("serializing hole summary")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HoleLocationJson {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoleCostBudgetJson {
    pub budget_total: Option<f64>,
    pub cost_before_hole: f64,
    pub budget_remaining: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoleCandidateJson {
    pub name: String,
    pub type_match: f64,
    pub cost_fit: f64,
    pub capability_fit: f64,
    pub error_coverage: f64,
    pub overall: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HoleTypeInferenceJson {
    Certain,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HoleCandidateRankingJson {
    UniqueBest,
    Ambiguous,
    NoCandidates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HoleConfidenceJson {
    pub type_inference: HoleTypeInferenceJson,
    pub candidate_ranking: HoleCandidateRankingJson,
    pub ambiguous_count: usize,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HoleErrorClusterJson {
    pub source: String,
    pub errors: Vec<String>,
    pub handling_suggestion: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HoleDependencyKind {
    Type,
    Value,
    Cost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HoleDependencyEdgeJson {
    pub from: String,
    pub to: String,
    pub kind: HoleDependencyKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HoleDependencyGraphJson {
    pub dependencies: BTreeMap<String, Vec<String>>,
    pub edges: Vec<HoleDependencyEdgeJson>,
    pub roots: Vec<String>,
    pub suggested_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoleInfoJson {
    pub name: String,
    pub display_name: String,
    pub location: Option<HoleLocationJson>,
    pub expected_type: String,
    pub type_inferred_from: Option<String>,
    pub function: String,
    pub enclosing_signature: Option<String>,
    pub bindings: BTreeMap<String, String>,
    pub binding_dependencies: BTreeMap<String, Vec<String>>,
    pub capabilities: Vec<String>,
    pub errors_to_handle: Vec<String>,
    pub cost_budget: Option<HoleCostBudgetJson>,
    pub candidates: Vec<HoleCandidateJson>,
    pub dependent_holes: Vec<String>,
    pub confidence: Option<HoleConfidenceJson>,
    pub error_clusters: Vec<HoleErrorClusterJson>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoleReportJson {
    pub holes: Vec<HoleInfoJson>,
    pub dependency_graph: HoleDependencyGraphJson,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LspLocation {
    pub uri: String,
    pub range: LspRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LspDiagnosticRelatedInformation {
    pub location: LspLocation,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LspDiagnostic {
    pub range: LspRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<u32>,
    pub source: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_information: Vec<LspDiagnosticRelatedInformation>,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    name: String,
    contents: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, contents: impl Into<String>) -> Self {
        let name = name.into();
        let contents = contents.into();
        let mut line_starts = vec![0];
        for (index, byte) in contents.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }
        Self {
            name,
            contents,
            line_starts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn contents(&self) -> &str {
        &self.contents
    }

    pub fn position(&self, offset: usize) -> Position {
        let clamped = offset.min(self.contents.len());
        let line_index = match self.line_starts.binary_search(&clamped) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];
        let col = self.contents[line_start..clamped].chars().count() + 1;
        Position {
            line: line_index + 1,
            col,
        }
    }

    pub fn byte_offset(&self, position: Position) -> Option<usize> {
        if position.line == 0 || position.col == 0 {
            return None;
        }
        let line_index = position.line - 1;
        let line_start = *self.line_starts.get(line_index)?;
        let raw_line_end = self
            .line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(self.contents.len());
        let line_end =
            if raw_line_end > line_start && self.contents.as_bytes()[raw_line_end - 1] == b'\n' {
                raw_line_end - 1
            } else {
                raw_line_end
            };
        let line_text = &self.contents[line_start..line_end];

        let mut current_col = 1;
        let mut byte_offset = line_start;
        if position.col == current_col {
            return Some(byte_offset);
        }
        for ch in line_text.chars() {
            byte_offset += ch.len_utf8();
            current_col += 1;
            if position.col == current_col {
                return Some(byte_offset);
            }
        }
        None
    }

    pub fn span(&self, byte_range: Range<usize>) -> SourceSpan {
        let start = byte_range.start.min(self.contents.len());
        let end = byte_range.end.min(self.contents.len()).max(start);
        SourceSpan {
            file: self.name.clone(),
            range: DiagnosticRange {
                start: self.position(start),
                end: self.position(end),
            },
            byte_range: Some(start..end),
        }
    }

    pub fn span_from_range(
        &self,
        file: impl Into<String>,
        range: DiagnosticRange,
    ) -> Option<SourceSpan> {
        let start = self.byte_offset(range.start)?;
        let end = self.byte_offset(range.end)?.max(start);
        Some(SourceSpan {
            file: file.into(),
            range,
            byte_range: Some(start..end),
        })
    }

    fn line_start(&self, line_index: usize) -> Option<usize> {
        self.line_starts.get(line_index).copied()
    }

    fn line_end(&self, line_index: usize) -> Option<usize> {
        let line_start = self.line_start(line_index)?;
        let raw_line_end = self
            .line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(self.contents.len());
        Some(
            if raw_line_end > line_start && self.contents.as_bytes()[raw_line_end - 1] == b'\n' {
                raw_line_end - 1
            } else {
                raw_line_end
            },
        )
    }
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("failed to render diagnostic")]
    Io(#[from] io::Error),
}

pub fn render_diagnostic_to_string(
    source: &SourceFile,
    diagnostic: &Diagnostic,
) -> Result<String, RenderError> {
    let mut buffer = Vec::new();
    render_diagnostic(&mut buffer, source, diagnostic)?;
    Ok(String::from_utf8(buffer).expect("ariadne output must be valid utf-8"))
}

pub fn render_diagnostic<W: io::Write>(
    writer: &mut W,
    source: &SourceFile,
    diagnostic: &Diagnostic,
) -> Result<(), RenderError> {
    let anchor = diagnostic
        .primary_span
        .as_ref()
        .and_then(|span| {
            if span.file == source.name {
                span.byte_range()
            } else {
                debug!(
                    diagnostic_code = %diagnostic.code,
                    span_file = %span.file,
                    source_file = %source.name,
                    "skipping primary span from a different source file"
                );
                None
            }
        })
        .unwrap_or(0..0);

    let mut report = Report::build(
        diagnostic.severity.report_kind(),
        (source.name.clone(), anchor.clone()),
    )
    .with_code(diagnostic.code.clone())
    .with_message(diagnostic.message.clone());

    if let Some(primary_span) = diagnostic.primary_span.as_ref()
        && primary_span.file == source.name
        && let Some(byte_range) = primary_span.byte_range()
    {
        report = report.with_label(
            Label::new((source.name.clone(), byte_range))
                .with_color(diagnostic.severity.color())
                .with_message(diagnostic.message.clone()),
        );
    }

    for label in &diagnostic.secondary_labels {
        if label.span.file != source.name {
            debug!(
                diagnostic_code = %diagnostic.code,
                span_file = %label.span.file,
                source_file = %source.name,
                "skipping secondary label from a different source file"
            );
            continue;
        }
        let Some(byte_range) = label.span.byte_range() else {
            debug!(
                diagnostic_code = %diagnostic.code,
                "skipping secondary label without byte offsets"
            );
            continue;
        };
        report = report.with_label(
            Label::new((source.name.clone(), byte_range))
                .with_color(diagnostic.severity.color())
                .with_message(label.message.clone()),
        );
    }

    for note in &diagnostic.notes {
        report = report.with_note(note.clone());
    }
    if let Some(help) = &diagnostic.help {
        report = report.with_help(help.clone());
    }

    report.finish().write(
        (source.name.clone(), Source::from(source.contents())),
        writer,
    )?;
    Ok(())
}

pub fn print_json<T: Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string(value).expect("serializing JSON output")
    );
}

pub fn diagnostic_message_line(diagnostic: &Diagnostic) -> String {
    format!("{}: {}", diagnostic.code, diagnostic.message)
}

pub fn diagnostic_message_lines(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics.iter().map(diagnostic_message_line).collect()
}

pub fn emit_warning_message(message: &str, json_output: bool) {
    if json_output {
        print_json(&json!({
            "severity": "warning",
            "message": message,
        }));
    } else {
        eprintln!("warning: {message}");
    }
}

pub fn exit_with_message_error(message: &str, json_output: bool) -> ExitCode {
    if json_output {
        print_json(
            &JsonReport::new()
                .with_status(ReportStatus::Error)
                .with_message(message),
        );
    } else {
        eprintln!("error: {message}");
    }
    ExitCode::FAILURE
}

pub fn render_diagnostics_human(source: &SourceFile, diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        let rendered = render_diagnostic_to_string(source, diagnostic)
            .unwrap_or_else(|_| format!("{}: {}", diagnostic.code, diagnostic.message));
        if rendered.ends_with('\n') {
            eprint!("{rendered}");
        } else {
            eprintln!("{rendered}");
        }
    }
}

fn diagnostic_source_name(diagnostic: &Diagnostic) -> Option<&str> {
    diagnostic
        .primary_span
        .as_ref()
        .map(|span| span.file.as_str())
        .or_else(|| {
            diagnostic
                .secondary_labels
                .first()
                .map(|label| label.span.file.as_str())
        })
}

fn is_synthetic_source(name: &str) -> bool {
    name.starts_with('<') && name.ends_with('>')
}

fn fallback_source<'a>(
    source_index: &HashMap<&'a str, &'a SourceFile>,
    sources: &'a [SourceFile],
    synthetic: &'a SourceFile,
) -> &'a SourceFile {
    source_index
        .get("<batch>")
        .copied()
        .or_else(|| {
            sources
                .iter()
                .find(|source| is_synthetic_source(source.name()))
        })
        .or_else(|| sources.first())
        .unwrap_or(synthetic)
}

fn source_for_diagnostic<'a>(
    source_index: &HashMap<&'a str, &'a SourceFile>,
    sources: &'a [SourceFile],
    synthetic: &'a SourceFile,
    diagnostic: &Diagnostic,
) -> &'a SourceFile {
    diagnostic_source_name(diagnostic)
        .and_then(|name| source_index.get(name).copied())
        .unwrap_or_else(|| fallback_source(source_index, sources, synthetic))
}

pub fn render_diagnostics_human_with_sources(sources: &[SourceFile], diagnostics: &[Diagnostic]) {
    let source_index: HashMap<&str, &SourceFile> = sources
        .iter()
        .map(|source| (source.name(), source))
        .collect();
    let synthetic = SourceFile::new("<diagnostics>", "");

    for diagnostic in diagnostics {
        let source = source_for_diagnostic(&source_index, sources, &synthetic, diagnostic);
        let rendered = render_diagnostic_to_string(source, diagnostic)
            .unwrap_or_else(|_| format!("{}: {}", diagnostic.code, diagnostic.message));
        if rendered.ends_with('\n') {
            eprint!("{rendered}");
        } else {
            eprintln!("{rendered}");
        }
    }
}

pub fn exit_with_diagnostics_error(
    source: &SourceFile,
    diagnostics: &[Diagnostic],
    json_output: bool,
) -> ExitCode {
    if json_output {
        print_json(
            &JsonReport::new()
                .with_status(ReportStatus::Error)
                .with_message(diagnostic_message_lines(diagnostics).join("\n"))
                .with_diagnostics(diagnostics),
        );
    } else {
        render_diagnostics_human(source, diagnostics);
    }
    ExitCode::FAILURE
}

pub fn exit_with_diagnostics_error_with_sources(
    sources: &[SourceFile],
    diagnostics: &[Diagnostic],
    json_output: bool,
) -> ExitCode {
    if json_output {
        print_json(
            &JsonReport::new()
                .with_status(ReportStatus::Error)
                .with_message(diagnostic_message_lines(diagnostics).join("\n"))
                .with_diagnostics(diagnostics),
        );
    } else {
        render_diagnostics_human_with_sources(sources, diagnostics);
    }
    ExitCode::FAILURE
}

fn severity_to_lsp(severity: Severity) -> u32 {
    match severity {
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Note => 3,
    }
}

fn fallback_lsp_position(position: Position) -> LspPosition {
    LspPosition {
        line: position.line.saturating_sub(1) as u32,
        character: position.col.saturating_sub(1) as u32,
    }
}

fn source_position_to_lsp(source: &SourceFile, position: Position) -> LspPosition {
    if position.line == 0 {
        return LspPosition {
            line: 0,
            character: 0,
        };
    }
    let line_index = position.line - 1;
    let Some(line_start) = source.line_start(line_index) else {
        return fallback_lsp_position(position);
    };
    let line_end = source.line_end(line_index).unwrap_or(source.contents.len());
    let byte_offset = source
        .byte_offset(position)
        .unwrap_or(line_end)
        .clamp(line_start, line_end);
    let character = source.contents[line_start..byte_offset]
        .encode_utf16()
        .count() as u32;
    LspPosition {
        line: line_index as u32,
        character,
    }
}

fn span_to_lsp_range(source: Option<&SourceFile>, span: &SourceSpan) -> LspRange {
    if let Some(source) = source.filter(|source| source.name() == span.file) {
        LspRange {
            start: source_position_to_lsp(source, span.range.start),
            end: source_position_to_lsp(source, span.range.end),
        }
    } else {
        LspRange {
            start: fallback_lsp_position(span.range.start),
            end: fallback_lsp_position(span.range.end),
        }
    }
}

fn diagnostic_file_uri(file: &str, default_uri: &str) -> String {
    if file == default_uri {
        return default_uri.to_string();
    }
    if file.starts_with("file://") || file.starts_with("untitled:") {
        return file.to_string();
    }
    if Path::new(file).is_absolute() {
        return format!("file://{file}");
    }
    default_uri.to_string()
}

pub fn lsp_diagnostic_for_source(
    source: &SourceFile,
    diagnostic: &Diagnostic,
    uri: &str,
) -> LspDiagnostic {
    let range = diagnostic
        .primary_span
        .as_ref()
        .map(|span| span_to_lsp_range(Some(source), span))
        .unwrap_or(LspRange {
            start: LspPosition {
                line: 0,
                character: 0,
            },
            end: LspPosition {
                line: 0,
                character: 0,
            },
        });
    let related_information = diagnostic
        .related
        .iter()
        .filter_map(|related| {
            let span = related.span.as_ref()?;
            Some(LspDiagnosticRelatedInformation {
                location: LspLocation {
                    uri: diagnostic_file_uri(&span.file, uri),
                    range: span_to_lsp_range(Some(source), span),
                },
                message: related.message.clone(),
            })
        })
        .collect();

    LspDiagnostic {
        range,
        severity: Some(severity_to_lsp(diagnostic.severity)),
        source: "spore",
        message: diagnostic.message.clone(),
        code: Some(diagnostic.code.clone()),
        related_information,
    }
}

pub fn lsp_diagnostics_for_source(
    source: &SourceFile,
    diagnostics: &[Diagnostic],
    uri: &str,
) -> Vec<LspDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| lsp_diagnostic_for_source(source, diagnostic, uri))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_byte_offsets_to_one_based_line_and_column_positions() {
        let source = SourceFile::new("src/demo.sp", "alpha\nbeta\n");

        assert_eq!(source.position(0), Position { line: 1, col: 1 });
        assert_eq!(source.position(5), Position { line: 1, col: 6 });
        assert_eq!(source.position(6), Position { line: 2, col: 1 });
        assert_eq!(source.position(10), Position { line: 2, col: 5 });
    }

    #[test]
    fn serializes_minimal_diagnostic_fields() {
        let source = SourceFile::new("src/demo.sp", "alpha\nbeta\n");
        let diagnostic = Diagnostic::new("E0301", Severity::Error, "type mismatch")
            .with_primary_span(source.span(6..10))
            .with_note("expected `Int`")
            .with_help("convert the value before returning");

        let value = serde_json::to_value(&diagnostic).expect("serialize diagnostic");

        assert_eq!(value["code"], "E0301");
        assert_eq!(value["severity"], "error");
        assert_eq!(value["message"], "type mismatch");
        assert_eq!(value["primary_span"]["file"], "src/demo.sp");
        assert_eq!(value["primary_span"]["range"]["start"]["line"], 2);
        assert_eq!(value["primary_span"]["range"]["start"]["col"], 1);
        assert_eq!(value["help"], "convert the value before returning");
    }

    #[test]
    fn serializes_json_report_with_canonical_diagnostics() {
        let source = SourceFile::new("src/demo.sp", "alpha\nbeta\n");
        let diagnostic = Diagnostic::new("E0301", Severity::Error, "type mismatch")
            .with_primary_span(source.span(6..10));
        let warnings = vec!["W0001: cost exceeded".to_string()];
        let empty: [Diagnostic; 0] = [];
        let report = JsonReport::new()
            .with_status(ReportStatus::Error)
            .with_message("type mismatch")
            .with_diagnostics(std::slice::from_ref(&diagnostic))
            .with_warnings(&warnings)
            .with_errors(&empty);

        let value = serde_json::to_value(&report).expect("serialize json report");

        assert_eq!(value["status"], "error");
        assert_eq!(value["message"], "type mismatch");
        assert_eq!(value["diagnostics"][0]["code"], "E0301");
        assert_eq!(value["warnings"][0], "W0001: cost exceeded");
        assert!(
            value["errors"]
                .as_array()
                .is_some_and(|errors| errors.is_empty())
        );
    }

    #[test]
    fn serializes_hole_summary_as_watch_event() {
        let summary = HoleSummary::new(3, 0, 1, 2);
        let value = serde_json::to_value(&summary).expect("serialize hole summary");

        assert_eq!(value["event"], "hole_graph_update");
        assert_eq!(value["holes_total"], 3);
        assert_eq!(value["ready_to_fill"], 1);
        assert_eq!(value["blocked"], 2);
    }

    #[test]
    fn renders_human_output_with_ariadne() {
        let source = SourceFile::new("src/demo.sp", "let answer = true\nanswer + 1\n");
        let diagnostic = Diagnostic::new("E0301", Severity::Error, "type mismatch")
            .with_primary_span(source.span(18..24))
            .with_secondary_label(SecondaryLabel::new(
                source.span(25..31),
                "addition expects a numeric left-hand side",
            ))
            .with_note("`answer` was inferred as `Bool`")
            .with_help("convert `answer` to an integer before adding");

        let rendered =
            render_diagnostic_to_string(&source, &diagnostic).expect("render diagnostic");

        assert!(rendered.contains("E0301"));
        assert!(rendered.contains("type mismatch"));
        assert!(rendered.contains("convert `answer` to an integer before adding"));
    }

    #[test]
    fn reconstructs_byte_ranges_from_serialized_positions() {
        let source = SourceFile::new("src/demo.sp", "alpha\nbeta\n");
        let original = source.span(6..10);
        let reconstructed = source
            .span_from_range(original.file.clone(), original.range.clone())
            .expect("reconstruct span");

        assert_eq!(reconstructed.byte_range(), Some(6..10));
        assert_eq!(reconstructed.range, original.range);
    }

    #[test]
    fn maps_canonical_diagnostics_to_lsp_shape() {
        let source = SourceFile::new("file:///workspace/main.sp", "alpha\nbeta\n");
        let diagnostic = Diagnostic::new("E0301", Severity::Error, "type mismatch")
            .with_primary_span(source.span(6..10))
            .with_related(RelatedDiagnostic::new(
                "related note",
                Some(source.span(0..5)),
            ));

        let value = serde_json::to_value(lsp_diagnostic_for_source(
            &source,
            &diagnostic,
            "file:///workspace/main.sp",
        ))
        .expect("serialize lsp diagnostic");

        assert_eq!(value["code"], "E0301");
        assert_eq!(value["severity"], 1);
        assert_eq!(value["range"]["start"]["line"], 1);
        assert_eq!(value["range"]["start"]["character"], 0);
        assert_eq!(
            value["relatedInformation"][0]["location"]["uri"],
            "file:///workspace/main.sp"
        );
        assert_eq!(value["relatedInformation"][0]["message"], "related note");
    }

    #[test]
    fn spanless_diagnostics_prefer_batch_source_over_first_real_file() {
        let entry = SourceFile::new("src/main.sp", "fn main() -> I32 { 0 }\n");
        let batch = SourceFile::new("<batch>", "");
        let sources = vec![
            entry,
            batch,
            SourceFile::new("lib/util.sp", "fn util() = 1\n"),
        ];
        let source_index: HashMap<&str, &SourceFile> = sources
            .iter()
            .map(|source| (source.name(), source))
            .collect();
        let synthetic = SourceFile::new("<diagnostics>", "");
        let diagnostic = Diagnostic::new(
            "module-not-found",
            Severity::Error,
            "module `missing` not found",
        );

        let source = source_for_diagnostic(&source_index, &sources, &synthetic, &diagnostic);

        assert_eq!(source.name(), "<batch>");
    }
}
