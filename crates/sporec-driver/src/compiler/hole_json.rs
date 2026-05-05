use crate::diagnostics::source_file;
use std::collections::BTreeMap;

use sporec_diagnostics::{
    HoleCandidateCostCheckJson, HoleCandidateJson, HoleCandidateRankingJson, HoleConfidenceJson,
    HoleCostBudgetJson, HoleCostVectorJson, HoleDependencyEdgeJson, HoleDependencyGraphJson,
    HoleDependencyKind, HoleEffectContextJson, HoleErrorClusterJson, HoleInfoJson,
    HoleLocationJson, HoleReportJson, HoleResidualContextJson, HoleSummary, HoleTypeInferenceJson,
    SourceFile,
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

fn hole_location_json(source: &SourceFile, hole: &TypeckHoleInfo) -> Option<HoleLocationJson> {
    hole.location
        .as_ref()
        .map(|location| HoleLocationJson {
            file: location.file.clone(),
            line: location.line,
            column: location.column,
        })
        .or_else(|| {
            hole.span.map(|span| {
                let position = source.position(span.start);
                HoleLocationJson {
                    file: source.name().to_string(),
                    line: position.line as u32,
                    column: position.col as u32,
                }
            })
        })
}

fn hole_cost_vector_json(cost: &sporec_typeck::hole::CostVectorSurface) -> HoleCostVectorJson {
    HoleCostVectorJson {
        compute: cost.compute.clone(),
        alloc: cost.alloc.clone(),
        io: cost.io.clone(),
        parallel: cost.parallel.clone(),
    }
}

fn hole_info_json(source: &SourceFile, hole: &TypeckHoleInfo) -> HoleInfoJson {
    HoleInfoJson {
        name: hole.name.clone(),
        display_name: display_hole_name(&hole.name),
        location: hole_location_json(source, hole),
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
        available_effects: hole.available_effects.iter().cloned().collect(),
        errors_to_handle: hole.errors_to_handle.clone(),
        effect_context: hole
            .effect_context
            .as_ref()
            .map(|context| HoleEffectContextJson {
                discharged_effects: context.discharged_effects.iter().cloned().collect(),
                surviving_effects: context.surviving_effects.iter().cloned().collect(),
            }),
        cost_budget: hole.cost_budget.as_ref().map(|budget| HoleCostBudgetJson {
            budget_total: budget.budget_total,
            cost_before_hole: budget.cost_before_hole,
            budget_remaining: budget.budget_remaining,
        }),
        residual_context: hole
            .residual_context
            .as_ref()
            .map(|context| HoleResidualContextJson {
                budget_declared: context.budget_declared.as_ref().map(hole_cost_vector_json),
                cost_before: hole_cost_vector_json(&context.cost_before),
                budget_residual: context.budget_residual.as_ref().map(hole_cost_vector_json),
                fit_rule: context.fit_rule.clone(),
                note: context.note.clone(),
            }),
        candidates: hole
            .candidates
            .iter()
            .map(|candidate| HoleCandidateJson {
                name: candidate.name.clone(),
                type_match: candidate.type_match,
                cost_fit: candidate.cost_fit,
                required_effects_fit: candidate.required_effects_fit,
                error_coverage: candidate.error_coverage,
                overall: candidate.overall(),
                rejection_reasons: candidate.rejection_reasons.clone(),
                explanation: candidate.explanation.clone(),
                adjustments: candidate.adjustments.clone(),
                cost_check: candidate.cost_check.as_ref().map(|cost_check| {
                    HoleCandidateCostCheckJson {
                        candidate_cost: cost_check
                            .candidate_cost
                            .as_ref()
                            .map(hole_cost_vector_json),
                        projected_cost: cost_check
                            .projected_cost
                            .as_ref()
                            .map(hole_cost_vector_json),
                        fits_budget: cost_check.fits_budget,
                        exceeded_dimensions: cost_check.exceeded_dimensions.clone(),
                        reason: cost_check.reason.clone(),
                    }
                }),
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
    let source_file = source_file("file:///buffer.sp", source);
    Ok(HoleReportJson {
        holes: report
            .holes
            .iter()
            .map(|hole| hole_info_json(&source_file, hole))
            .collect(),
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
    let source_file = source_file(file.replace('\\', "/"), source);
    let needle = hole.strip_prefix('?').unwrap_or(hole);
    let matches: Vec<&TypeckHoleInfo> = report
        .holes
        .iter()
        .filter(|candidate| candidate.name == needle)
        .collect();

    match matches.as_slice() {
        [hole] => Ok(hole_info_json(&source_file, hole)),
        [] => Err(format!("hole `?{needle}` not found in `{file}`")),
        _ => {
            let locations = matches
                .iter()
                .map(|candidate| {
                    hole_location_json(&source_file, candidate)
                        .map(|location| {
                            format!("{}:{}:{}", location.file, location.line, location.column)
                        })
                        .unwrap_or_else(|| candidate.function.clone())
                })
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "hole `?{needle}` is ambiguous in `{file}`; matching locations: {locations}"
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
