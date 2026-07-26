use store::StoreBackend;

use crate::types::{DecisionQuery, MemoryQuery, ReflectionQuery, ScoredDecision, ScoredMemory, ScoredReflection};

/// Retrieve decisions matching a query.
pub async fn retrieve_decisions<S: StoreBackend>(
    store: &S,
    query: &DecisionQuery,
) -> Result<Vec<ScoredDecision>, String> {
    let decisions = store.list_decisions(None, query.limit).await.map_err(|e| format!("list_decisions: {e}"))?;
    let scored: Vec<ScoredDecision> = decisions
        .into_iter()
        .filter_map(|d| {
            // Client-side domain filter (MVP; D1 doesn't do complex WHERE easily)
            if let Some(ref domain) = query.domain {
                if !d.decision_type.contains(domain) && !d.title.to_lowercase().contains(&domain.to_lowercase()) {
                    return None;
                }
            }
            Some(ScoredDecision {
                id: format!("DEC-{:06}", d.id),
                title: d.title,
                decision_type: d.decision_type,
                status: d.status,
                confidence: d.confidence,
                relevance_score: 0.0,
                rank_components: crate::types::RankComponents {
                    query_alignment: 0.0,
                    confidence: d.confidence,
                    recency: 0.0,
                    usage_frequency: 0.0,
                    user_specificity: 1.0,
                },
            })
        })
        .collect();
    Ok(scored)
}

/// Retrieve reflections matching a query.
#[allow(unused_variables)]
pub async fn retrieve_reflections<S: StoreBackend>(
    store: &S,
    query: &ReflectionQuery,
) -> Result<Vec<ScoredReflection>, String> {
    // Reflections queried by status via list_decisions then look up. For MVP: return empty vector.
    // Full impl when reflection CRUD methods are added to StoreBackend.
    Ok(Vec::new())
}

/// Retrieve memories matching a query.
pub async fn retrieve_memories<S: StoreBackend>(store: &S, query: &MemoryQuery) -> Result<Vec<ScoredMemory>, String> {
    let memories = store
        .list_memories(None, query.status.as_deref(), query.limit)
        .await
        .map_err(|e| format!("list_memories: {e}"))?;
    let scored: Vec<ScoredMemory> = memories
        .into_iter()
        .map(|m| ScoredMemory {
            id: format!("MEM-{:06}", m.id),
            statement: m.statement,
            memory_type: m.memory_type,
            confidence: m.confidence,
            relevance_score: 0.0,
            rank_components: crate::types::RankComponents {
                query_alignment: 0.0,
                confidence: m.confidence,
                recency: 0.0,
                usage_frequency: m.usage_count as f64,
                user_specificity: 1.0,
            },
        })
        .collect();
    Ok(scored)
}
