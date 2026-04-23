use std::collections::BTreeMap;

use sporec_diagnostics::{
    HoleCandidateJson, HoleCandidateRankingJson, HoleConfidenceJson, HoleCostBudgetJson,
    HoleDependencyEdgeJson, HoleDependencyGraphJson, HoleDependencyKind, HoleErrorClusterJson,
    HoleInfoJson, HoleLocationJson, HoleReportJson, HoleSummary, HoleTypeInferenceJson,
};
use sporec_typeck::hole::{
    CandidateRanking, EdgeKind, HoleInfo as TypeckHoleInfo, HoleReport as TypeckHoleReport,
    TypeInferenceConfidence,
};
use sporec_typeck::{is_synthetic_hole_name, type_check};

use super::join_errors;

fn load_hole_report(source: &str) -> Result<TypeckHoleReport, String> {
    let ast = sporec_parser::parse(source).map_err(join_errors)?;
    let result = type_check(&ast).map_err(join_errors)?;
    Ok(result.hole_report)
}

fn display_hole_name(name: &str) -> String {
    if is_synthetic_hole_name(name) {
        "?".to_string()
    } else {
        format!("?{name}")
    }
}

fn hole_type_inference_json(confidence: TypeInferenceConfidence) -> HoleTypeInferenceJson {
    match confidence {
        TypeInferenceConfidence::Certain => HoleTypeInferenceJson::Certain,
        TypeInferenceConfidence::Partial => HoleTypeInferenceJson::Partial,
        TypeInferenceConfidence::Unknown => HoleTypeInferenceJson::Unknown,
    }
}

fn hole_candidate_ranking_json(ranking: CandidateRanking) -> HoleCandidateRankingJson {
    match ranking {
        CandidateRanking::UniqueBest => HoleCandidateRankingJson::UniqueBest,
        CandidateRanking::Ambiguous => HoleCandidateRankingJson::Ambiguous,
        CandidateRanking::NoCandidates => HoleCandidateRankingJson::NoCandidates,
    }
}

fn hole_dependency_kind_json(kind: &EdgeKind) -> HoleDependencyKind {
    match kind {
        EdgeKind::Type => HoleDependencyKind::Type,
        EdgeKind::Value => HoleDependencyKind::Value,
        EdgeKind::Cost => HoleDependencyKind::Cost,
    }
}

fn hole_dependency_kind_rank(kind: &HoleDependencyKind) -> u8 {
    match kind {
        HoleDependencyKind::Type => 0,
        HoleDependencyKind::Value => 1,
        HoleDependencyKind::Cost => 2,
    }
}

fn hole_info_json(hole: &TypeckHoleInfo) -> HoleInfoJson {
    HoleInfoJson {
        name: hole.name.clone(),
        display_name: display_hole_name(&hole.name),
        location: hole.location.as_ref().map(|location| HoleLocationJson {
            file: location.file.clone(),
            line: location.line,
            column: location.column,
        }),
        expected_type: hole.expected_type.to_string(),
        type_inferred_from: hole.type_inferred_from.clone(),
        function: hole.function.clone(),
        enclosing_signature: hole.enclosing_signature.clone(),
        bindings: hole
            .bindings
            .iter()
            .map(|(name, ty)| (name.clone(), ty.to_string()))
            .collect(),
        binding_dependencies: hole.binding_dependencies.clone(),
        capabilities: hole.capabilities.iter().cloned().collect(),
        errors_to_handle: hole.errors_to_handle.clone(),
        cost_budget: hole.cost_budget.as_ref().map(|budget| HoleCostBudgetJson {
            budget_total: budget.budget_total,
            cost_before_hole: budget.cost_before_hole,
            budget_remaining: budget.budget_remaining,
        }),
        candidates: hole
            .candidates
            .iter()
            .map(|candidate| HoleCandidateJson {
                name: candidate.name.clone(),
                type_match: candidate.type_match,
                cost_fit: candidate.cost_fit,
                capability_fit: candidate.capability_fit,
                error_coverage: candidate.error_coverage,
                overall: candidate.overall(),
            })
            .collect(),
        dependent_holes: hole.dependent_holes.clone(),
        confidence: hole
            .confidence
            .as_ref()
            .map(|confidence| HoleConfidenceJson {
                type_inference: hole_type_inference_json(confidence.type_inference.clone()),
                candidate_ranking: hole_candidate_ranking_json(
                    confidence.candidate_ranking.clone(),
                ),
                ambiguous_count: confidence.ambiguous_count,
                recommendation: confidence.recommendation.clone(),
            }),
        error_clusters: hole
            .error_clusters
            .iter()
            .map(|cluster| HoleErrorClusterJson {
                source: cluster.source.clone(),
                errors: cluster.errors.clone(),
                handling_suggestion: cluster.handling_suggestion.clone(),
            })
            .collect(),
    }
}

fn hole_dependency_graph_json(
    graph: &sporec_typeck::hole::HoleDependencyGraph,
) -> HoleDependencyGraphJson {
    let dependencies = graph
        .dependencies
        .iter()
        .map(|(hole, deps)| {
            let mut deps = deps.iter().cloned().collect::<Vec<_>>();
            deps.sort();
            (hole.clone(), deps)
        })
        .collect::<BTreeMap<_, _>>();

    let mut edges = graph
        .edges
        .iter()
        .map(|edge| HoleDependencyEdgeJson {
            from: edge.from.clone(),
            to: edge.to.clone(),
            kind: hole_dependency_kind_json(&edge.kind),
        })
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        (&left.from, &left.to, hole_dependency_kind_rank(&left.kind)).cmp(&(
            &right.from,
            &right.to,
            hole_dependency_kind_rank(&right.kind),
        ))
    });

    HoleDependencyGraphJson {
        dependencies,
        edges,
        roots: graph.roots(),
        suggested_order: graph.topological_order(),
    }
}

/// Analyze holes in Spore source and return the shared JSON report payload.
pub fn holes_report(source: &str) -> Result<HoleReportJson, String> {
    let report = load_hole_report(source)?;
    Ok(HoleReportJson {
        holes: report.holes.iter().map(hole_info_json).collect(),
        dependency_graph: hole_dependency_graph_json(&report.dependency_graph),
    })
}

/// Analyze holes in Spore source and return a JSON report.
pub fn holes(source: &str) -> Result<String, String> {
    let report = holes_report(source)?;
    serde_json::to_string(&report).map_err(|error| error.to_string())
}

/// Inspect a named hole and return the shared JSON payload used by `query-hole`.
pub fn query_hole_report(file: &str, source: &str, hole: &str) -> Result<HoleInfoJson, String> {
    let report = load_hole_report(source)?;
    let needle = hole.strip_prefix('?').unwrap_or(hole);
    let matches: Vec<&TypeckHoleInfo> = report
        .holes
        .iter()
        .filter(|candidate| candidate.name == needle)
        .collect();

    match matches.as_slice() {
        [hole] => Ok(hole_info_json(hole)),
        [] => Err(format!("hole `?{needle}` not found in `{file}`")),
        _ => {
            let locations = matches
                .iter()
                .map(|candidate| candidate.function.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "hole `?{needle}` is ambiguous in `{file}`; matching functions: {locations}"
            ))
        }
    }
}

/// Return a hole graph summary suitable for NDJSON watch events.
pub fn hole_summary(source: &str) -> Option<HoleSummary> {
    let report = load_hole_report(source).ok()?;
    let graph = &report.dependency_graph;

    let holes_total = report.holes.len();
    if holes_total == 0 {
        return None;
    }

    let ready_to_fill = graph.roots().len();
    let blocked = holes_total.saturating_sub(ready_to_fill);

    Some(HoleSummary::new(holes_total, 0, ready_to_fill, blocked))
}
