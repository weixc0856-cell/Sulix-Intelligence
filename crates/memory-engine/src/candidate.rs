use crate::repository::MemoryRepository;

#[derive(Debug, Clone)]
pub struct MemoryCandidate {
    pub event_id: String,
    pub reflection_id: String,
    pub decision_id: String,
    pub artifact_key: String,
    pub quality_score: f64,
    pub lesson_count: i64,
    pub rule_count: i64,
    pub occurred_at: i64,
}

pub async fn extract_candidates<R: MemoryRepository>(
    repo: &R,
    since: i64,
    limit: u32,
) -> Result<Vec<MemoryCandidate>, String> {
    let events = repo.list_reflection_events(limit).await.map_err(|e| format!("list_reflection_events failed: {e}"))?;

    let candidates: Vec<MemoryCandidate> = events
        .into_iter()
        .filter(|r| r.occurred_at > since)
        .map(|r| MemoryCandidate {
            event_id: r.event_id,
            reflection_id: r.aggregate_id,
            decision_id: String::new(),
            artifact_key: r.object_key,
            quality_score: 0.0,
            lesson_count: 0,
            rule_count: 0,
            occurred_at: r.occurred_at,
        })
        .collect();

    Ok(candidates)
}
