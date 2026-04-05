use spore_codegen::value::Value;
use spore_parser::formatter::format_module;
use spore_parser::parse;
use spore_typeck::CheckResult;
use spore_typeck::module::ModuleRegistry;
use spore_typeck::{type_check, type_check_with_registry};

/// Compile Spore source code to output.
///
/// This is the core compiler pipeline:
/// 1. Parse (source text → AST)
/// 2. Type check (AST → Typed AST)
/// 3. Code gen (Typed AST → native code)
pub fn compile(source: &str) -> Result<(), String> {
    let ast = parse(source).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let _result = type_check(&ast).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok(())
}

/// Compile multiple Spore source files together with shared module resolution.
///
/// 1. Parses each source into an AST
/// 2. Builds a ModuleRegistry from all modules
/// 3. Type-checks each module with access to the shared registry
pub fn compile_files(paths: &[&str]) -> Result<(), String> {
    let mut modules = Vec::new();

    // Phase 1: Parse all files
    for path in paths {
        let source =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read `{path}`: {e}"))?;
        let ast = parse(&source).map_err(|errs| {
            let msgs: Vec<String> = errs.into_iter().map(|e| e.to_string()).collect();
            format!("{path}: {}", msgs.join("\n"))
        })?;
        modules.push((*path, ast));
    }

    // Phase 2: Build ModuleRegistry from all modules
    let mut registry = ModuleRegistry::new();
    for (_path, ast) in &modules {
        let iface = spore_typeck::build_module_interface(ast);
        registry.register(iface);
    }

    // Phase 3: Type-check each module with the shared registry
    let mut all_errors = Vec::new();
    for (path, ast) in &modules {
        if let Err(errs) = type_check_with_registry(ast, registry.clone()) {
            for e in errs {
                all_errors.push(format!("{path}: {e}"));
            }
        }
    }

    if all_errors.is_empty() {
        Ok(())
    } else {
        Err(all_errors.join("\n"))
    }
}

/// Analyze holes in Spore source and return a JSON report.
pub fn holes(source: &str) -> Result<String, String> {
    let ast = parse(source).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let result = type_check(&ast).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok(result.hole_report.to_json())
}

/// Run a Spore program by executing its `main` function.
pub fn run(source: &str) -> Result<Value, String> {
    let ast = parse(source).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let _result = type_check(&ast).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    spore_codegen::run(&ast).map_err(|e| e.to_string())
}

/// Format Spore source code.
///
/// Parses the source into an AST and then pretty-prints it back using the
/// canonical formatter.  Returns the formatted source text.
pub fn format(source: &str) -> Result<String, String> {
    let ast = parse(source).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok(format_module(&ast))
}

/// Type-check with verbose output: returns detailed analysis including type
/// inference context, capability annotations, and cost summaries.
pub fn check_verbose(source: &str) -> Result<String, String> {
    let ast = parse(source).map_err(|errs| {
        errs.into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let result = type_check(&ast).map_err(|errs| {
        errs.into_iter()
            .map(|e| format!("  {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok(format_verbose_result(&result))
}

/// Summarise a successful CheckResult for --verbose output.
fn format_verbose_result(result: &CheckResult) -> String {
    let mut out = String::new();
    out.push_str("✓ no errors\n");

    // Type inference summary
    out.push_str("\n── Type Inference ──\n");
    out.push_str(&format!(
        "  holes: {} total\n",
        result.hole_report.holes.len()
    ));
    for h in &result.hole_report.holes {
        out.push_str(&format!("    ?{}: expected {}\n", h.name, h.expected_type));
    }

    // Cost analysis
    if !result.cost_vectors.is_empty() {
        out.push_str("\n── Cost Analysis ──\n");
        for (fn_name, cv) in &result.cost_vectors {
            out.push_str(&format!(
                "  {fn_name}: compute={}, alloc={}, io={}, parallel={}\n",
                cv.compute, cv.alloc, cv.io, cv.parallel
            ));
        }
    }

    out
}

/// Return a hole graph summary suitable for NDJSON watch events.
pub fn hole_summary(source: &str) -> Option<HoleSummary> {
    let ast = parse(source).ok()?;
    let result = type_check(&ast).ok()?;
    let report = &result.hole_report;
    let graph = &report.dependency_graph;

    let holes_total = report.holes.len();
    if holes_total == 0 {
        return None;
    }

    let ready_to_fill = graph.roots().len();
    let blocked = holes_total.saturating_sub(ready_to_fill);

    Some(HoleSummary {
        holes_total,
        filled_this_cycle: 0,
        ready_to_fill,
        blocked,
    })
}

/// Summary of hole status for a single check cycle.
#[derive(Debug, Clone)]
pub struct HoleSummary {
    pub holes_total: usize,
    pub filled_this_cycle: usize,
    pub ready_to_fill: usize,
    pub blocked: usize,
}

impl HoleSummary {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"event\":\"hole_graph_update\",\"holes_total\":{},\"filled_this_cycle\":{},\"ready_to_fill\":{},\"blocked\":{}}}",
            self.holes_total, self.filled_this_cycle, self.ready_to_fill, self.blocked
        )
    }
}
