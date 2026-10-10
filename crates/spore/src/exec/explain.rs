use std::process::ExitCode;

use owo_colors::OwoColorize;
use serde_json::json;

#[derive(Debug, Clone, Copy)]
struct ConceptDoc {
    id: &'static str,
    title: &'static str,
    summary: &'static str,
    syntax: &'static [&'static str],
    rationale: &'static [&'static str],
    minimal_examples: &'static [&'static str],
    diagnostics: &'static [&'static str],
    related_seps: &'static [&'static str],
    see_also: &'static [&'static str],
    aliases: &'static [&'static str],
}

const CONCEPTS: &[ConceptDoc] = &[
    ConceptDoc {
        id: "signatures",
        title: "Base and Intent Signatures",
        summary: "A function signature is the machine-readable boundary for what may be called and what intent a realization must preserve.",
        syntax: &[
            "fn name[T: Bound](param: Type) -> Return ! Error",
            "uses [Effect]",
            "budget { field: integer }",
            "properties { name(params): predicate }",
        ],
        rationale: &[
            "The Base Signature contains the callable type boundary: name, type parameters, value parameters, return type, and error boundary.",
            "The Intent Signature adds effects, realization-shape budgets, and properties without mixing those concerns into the type boundary.",
        ],
        minimal_examples: &[
            "fn normalize(score: I64) -> I64\nbudget { branches: 3 }\nproperties { lower(score: I64): normalize(score) >= 0 }\n{ ?body }",
        ],
        diagnostics: &["E0xxx", "F0xxx", "B0xxx", "P0xxx"],
        related_seps: &["SEP-0001", "SEP-0002", "SEP-0003", "SEP-0004"],
        see_also: &["properties", "holes", "budget", "uses"],
        aliases: &["signature", "fn"],
    },
    ConceptDoc {
        id: "properties",
        title: "Properties",
        summary: "Properties state executable validity rules that a realization should preserve.",
        syntax: &["properties { name(params): BoolExpression }"],
        rationale: &[
            "A zero-argument property is a concrete witness.",
            "A parameterized property is a validity rule over an input space.",
        ],
        minimal_examples: &["properties {\n    ordered(xs: List[I64]): is_ordered(sort(xs))\n}"],
        diagnostics: &["P0xxx", "E0301"],
        related_seps: &["SEP-0001", "SEP-0006", "SEP-0010"],
        see_also: &["signatures", "holes", "evidence"],
        aliases: &["property", "claims", "claim"],
    },
    ConceptDoc {
        id: "holes",
        title: "Typed Holes",
        summary: "A hole is typed absence: a valid expression position that asks the compiler to report the surrounding realization context.",
        syntax: &["?", "?name", "?name: Type", "spore holes FILE"],
        rationale: &[
            "Programs with holes are partial, not broken.",
            "HoleReport exposes type, binding, effect, budget, and property context for humans and tools.",
        ],
        minimal_examples: &["fn total(xs: List[I64]) -> I64 { ?total_body }"],
        diagnostics: &["H0xxx"],
        related_seps: &["SEP-0005", "SEP-0010"],
        see_also: &["properties", "budget", "uses"],
        aliases: &["hole", "?"],
    },
    ConceptDoc {
        id: "budget",
        title: "Realization-Shape Budget",
        summary: "A budget declares named integer upper bounds over implementation shape, not Big-O notation or host resource use.",
        syntax: &["budget { branches: 3, calls: 5, holes: 0 }"],
        rationale: &[
            "Budgets make review and Agent constraints explicit before reading the body.",
            "When a field is omitted, the checker imposes no source-level upper bound for that field.",
        ],
        minimal_examples: &[
            "fn choose(x: I64) -> I64\nbudget { branches: 1, recursion: 0 }\n{ if x > 0 { x } else { 0 } }",
        ],
        diagnostics: &["B0101", "B0102", "B0103", "B0201", "B0202"],
        related_seps: &["SEP-0004"],
        see_also: &["signatures", "holes", "properties"],
        aliases: &["budgets", "realization-shape"],
    },
    ConceptDoc {
        id: "uses",
        title: "Effect Surface",
        summary: "`uses [...]` declares the outside-world effects a function may require from its caller or Platform.",
        syntax: &[
            "uses [Console, FileRead]",
            "effect Name { fn op(...) -> Type }",
        ],
        rationale: &[
            "Runtime interaction belongs in `uses`; realization shape belongs in `budget`; semantic rules belong in `properties`.",
            "Callee effects must be covered by the caller or discharged by a handler.",
        ],
        minimal_examples: &["fn greet(name: Str) -> () uses [Console] {\n    println(name)\n}"],
        diagnostics: &["F0xxx"],
        related_seps: &["SEP-0003"],
        see_also: &["handlers", "signatures", "holes"],
        aliases: &["effects", "effect"],
    },
    ConceptDoc {
        id: "handlers",
        title: "Effect Handlers",
        summary: "Handlers provide implementations for declared effects and can discharge effect requirements inside a scoped expression.",
        syntax: &[
            "handler Name for Surface { fn Effect.op(...) -> Type { ... } }",
            "handle expr with Handler { ... }",
        ],
        rationale: &[
            "Handlers keep external interaction explicit while allowing tests, Platforms, and adapters to supply concrete behavior.",
        ],
        minimal_examples: &["handle greet(\"world\") with MockConsole { output: [] }"],
        diagnostics: &["F0xxx"],
        related_seps: &["SEP-0003"],
        see_also: &["uses", "platforms"],
        aliases: &["handler"],
    },
    ConceptDoc {
        id: "evidence",
        title: "Evidence Records",
        summary: "Evidence records what a checker verified for a concrete realization and which content identities the result depends on.",
        syntax: &[
            "signature_hash",
            "intent_hash",
            "property_hash",
            "realization_hash",
            "evidence_hash",
        ],
        rationale: &[
            "Evidence is checked support, not a stamp of correctness.",
            "Stable hashes let tools distinguish callable, intent, property, realization, and checker changes.",
        ],
        minimal_examples: &["claim: budget.nesting <= 3\nresult: passed\nobserved: 2"],
        diagnostics: &["M0402"],
        related_seps: &["SEP-0006", "SEP-0008"],
        see_also: &["properties", "budget", "packages"],
        aliases: &["evidence-record", "proof"],
    },
    ConceptDoc {
        id: "packages",
        title: "Modules and Packages",
        summary: "A Spore module is one source file, and package provenance records signatures, intents, properties, realizations, evidence, and dependencies.",
        syntax: &[
            "import package.module",
            "[project]",
            "[entries.name]",
            "[dependencies]",
        ],
        rationale: &[
            "File paths are the module structure source of truth.",
            "A project default target is declared through `[project].default-entry` and `[entries.<name>]`.",
        ],
        minimal_examples: &[
            "[project]\ndefault-entry = \"app\"\n\n[entries.app]\npath = \"main.sp\"",
        ],
        diagnostics: &["M0xxx"],
        related_seps: &["SEP-0008"],
        see_also: &["signatures", "evidence", "uses"],
        aliases: &["modules", "module", "package", "project"],
    },
    ConceptDoc {
        id: "types",
        title: "Types",
        summary: "Types define the callable boundary checked before effects, budgets, properties, or evidence can be trusted.",
        syntax: &["I64", "Str", "Bool", "List[T]", "A ! E", "(A) -> B"],
        rationale: &[
            "Function signatures are explicit; local bindings may be inferred when the expected type is clear.",
        ],
        minimal_examples: &["fn id[T](x: T) -> T { x }"],
        diagnostics: &["E0xxx"],
        related_seps: &["SEP-0002"],
        see_also: &["signatures", "holes"],
        aliases: &["type", "I64", "Str", "Bool", "outcome", "List"],
    },
];

pub(crate) fn exec_explain(query: Option<&str>, list: bool, json_output: bool) -> ExitCode {
    if list {
        return explain_list(json_output);
    }

    let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) else {
        return fail_explain(
            "missing explain query; use `spore explain --list`",
            json_output,
        );
    };

    if let Some(code_doc) = diagnostic_doc(query) {
        return print_diagnostic_doc(code_doc, json_output);
    }

    let matches = matching_concepts(query);
    match matches.as_slice() {
        [concept] => print_concept(concept, json_output),
        [] => fail_explain(
            &format!("unknown explain query `{query}`; use `spore explain --list`"),
            json_output,
        ),
        many => {
            let ids = many
                .iter()
                .map(|concept| concept.id)
                .collect::<Vec<_>>()
                .join(", ");
            fail_explain(
                &format!("ambiguous explain query `{query}`; matches: {ids}"),
                json_output,
            )
        }
    }
}

fn explain_list(json_output: bool) -> ExitCode {
    if json_output {
        let concepts = CONCEPTS.iter().map(concept_json).collect::<Vec<_>>();
        sporec_diagnostics::print_json(&json!({
            "version": 1,
            "schema": "spore.concepts.v1",
            "concepts": concepts,
        }));
    } else {
        println!("{}", "Spore concepts".bold());
        for concept in CONCEPTS {
            println!("  {:<12} {}", concept.id, concept.summary);
        }
    }
    ExitCode::SUCCESS
}

fn print_concept(concept: &ConceptDoc, json_output: bool) -> ExitCode {
    if json_output {
        sporec_diagnostics::print_json(&concept_json(concept));
    } else {
        println!("{} — {}", concept.id.bold(), concept.title);
        println!("{}", concept.summary);
        print_section("syntax", concept.syntax);
        print_section("rationale", concept.rationale);
        print_section("examples", concept.minimal_examples);
        print_section("diagnostics", concept.diagnostics);
        print_section("related SEPs", concept.related_seps);
        print_section("see also", concept.see_also);
    }
    ExitCode::SUCCESS
}

fn print_section(title: &str, lines: &[&str]) {
    if lines.is_empty() {
        return;
    }
    println!("\n{}:", title);
    for line in lines {
        for subline in line.lines() {
            println!("  {subline}");
        }
    }
}

fn concept_json(concept: &ConceptDoc) -> serde_json::Value {
    json!({
        "id": concept.id,
        "title": concept.title,
        "summary": concept.summary,
        "syntax": concept.syntax,
        "rationale": concept.rationale,
        "minimal_examples": concept.minimal_examples,
        "diagnostics": concept.diagnostics,
        "related_seps": concept.related_seps,
        "see_also": concept.see_also,
        "aliases": concept.aliases,
    })
}

fn matching_concepts(query: &str) -> Vec<&'static ConceptDoc> {
    CONCEPTS
        .iter()
        .filter(|concept| {
            concept.id.eq_ignore_ascii_case(query)
                || concept
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(query))
        })
        .collect()
}

#[derive(Debug, Clone)]
struct DiagnosticDoc {
    code: String,
    severity: String,
    summary: &'static str,
    concept_refs: Vec<&'static str>,
}

fn diagnostic_doc(query: &str) -> Option<DiagnosticDoc> {
    let normalized = query.trim().to_ascii_uppercase();
    let code = sporec_driver::all_error_codes()
        .iter()
        .copied()
        .find(|candidate| candidate.to_string() == normalized)?;
    Some(DiagnosticDoc {
        code: code.to_string(),
        severity: code.severity().to_string(),
        summary: code.explain(),
        concept_refs: diagnostic_concepts(&normalized),
    })
}

fn diagnostic_concepts(code: &str) -> Vec<&'static str> {
    match code.as_bytes().first().copied() {
        Some(b'E') | Some(b'R') => vec!["types", "signatures"],
        Some(b'F') => vec!["uses"],
        Some(b'B') => vec!["budget"],
        Some(b'H') => vec!["holes"],
        Some(b'M') => vec!["packages"],
        Some(b'P') => vec!["properties"],
        _ => Vec::new(),
    }
}

fn print_diagnostic_doc(doc: DiagnosticDoc, json_output: bool) -> ExitCode {
    if json_output {
        sporec_diagnostics::print_json(&json!({
            "code": doc.code,
            "severity": doc.severity,
            "summary": doc.summary,
            "concept_refs": doc.concept_refs,
        }));
    } else {
        println!("{}: {}", doc.code.bold(), doc.summary);
        println!("severity: {}", doc.severity);
        if !doc.concept_refs.is_empty() {
            println!(
                "learn: spore explain {}",
                doc.concept_refs.join(" | spore explain ")
            );
        }
    }
    ExitCode::SUCCESS
}

fn fail_explain(message: &str, json_output: bool) -> ExitCode {
    if json_output {
        sporec_diagnostics::print_json(&json!({
            "status": sporec_diagnostics::ReportStatus::Fail,
            "message": message,
        }));
    } else {
        eprintln!("{}: {message}", "error".red().bold());
    }
    ExitCode::FAILURE
}
