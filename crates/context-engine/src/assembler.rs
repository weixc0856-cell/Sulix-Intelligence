use crate::types::{
    AgentContext, ContextConfidence, ContextEvidence, Intent, PatternContext, ScoredDecision, ScoredMemory,
    ScoredReflection,
};

const ENGINE_VERSION: &str = "context-engine-v1";

/// Build the final AgentContext from retrieved, ranked items.
pub fn assemble(
    snapshot_id: &str,
    query: &str,
    intent: &Intent,
    decisions: Vec<ScoredDecision>,
    reflections: Vec<ScoredReflection>,
    memories: Vec<ScoredMemory>,
    patterns: Vec<PatternContext>,
) -> AgentContext {
    let now = (js_sys::Date::now() / 1000.0) as i64;

    // Build evidence lineage
    let mut evidence: Vec<ContextEvidence> = Vec::new();
    for d in &decisions {
        evidence.push(ContextEvidence {
            source_type: "decision".into(),
            source_id: d.id.clone(),
            selection_reason: "matched domain/decision_type".into(),
            relevance_score: d.relevance_score,
        });
    }
    for r in &reflections {
        evidence.push(ContextEvidence {
            source_type: "reflection".into(),
            source_id: r.id.clone(),
            selection_reason: "matched review criteria".into(),
            relevance_score: r.relevance_score,
        });
    }
    for m in &memories {
        evidence.push(ContextEvidence {
            source_type: "memory".into(),
            source_id: m.id.clone(),
            selection_reason: "matched memory_type/confidence".into(),
            relevance_score: m.relevance_score,
        });
    }

    // Compute confidence
    let n_total = (decisions.len() + reflections.len() + memories.len()) as f64;
    let coverage = (n_total / 30.0).min(1.0);
    let total_score: f64 = decisions.iter().map(|d| d.relevance_score).sum::<f64>()
        + reflections.iter().map(|r| r.relevance_score).sum::<f64>()
        + memories.iter().map(|m| m.relevance_score).sum::<f64>();
    let avg_confidence = total_score / n_total.max(1.0);
    let consistency = if patterns.len() <= 1 { 0.8 } else { 0.5 };
    let recency = 0.5;
    let overall = 0.25 * coverage + 0.25 * avg_confidence + 0.25 * recency + 0.25 * consistency;

    AgentContext {
        snapshot_id: snapshot_id.into(),
        query: query.into(),
        intent: intent.clone(),
        evidence,
        decisions,
        reflections,
        memories,
        patterns,
        confidence: ContextConfidence { overall, coverage, data_quality: avg_confidence, recency, consistency },
        engine_version: ENGINE_VERSION.into(),
        generated_at: now,
    }
}
