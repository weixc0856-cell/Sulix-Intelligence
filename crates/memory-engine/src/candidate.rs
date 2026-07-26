use store::{EventIndexEntry, StoreBackend};

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

pub async fn extract_candidates<S: StoreBackend>(
    store: &S,
    since: i64,
    limit: u32,
) -> Result<Vec<MemoryCandidate>, String> {
    let rows: Vec<EventIndexEntry> = store
        .find_event_keys("reflection", "", limit)
        .await
        .map_err(|e| format!("find_event_keys failed: {e}"))?;

    let candidates: Vec<MemoryCandidate> = rows
        .into_iter()
        .filter(|r| r.occurred_at > since)
        .map(|r| MemoryCandidate {
            event_id: r.event_id.clone(),
            reflection_id: r.aggregate_id.clone(),
            decision_id: String::new(),
            artifact_key: r.object_key.clone(),
            quality_score: 0.0,
            lesson_count: 0,
            rule_count: 0,
            occurred_at: r.occurred_at,
        })
        .collect();

    Ok(candidates)
}
