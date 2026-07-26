use store::{NewMemory, NewOutbox, PromotionScore, StoreBackend};
use crate::candidate::MemoryCandidate;

pub async fn promote<S: StoreBackend>(
    store: &S,
    candidate: &MemoryCandidate,
    score: &PromotionScore,
    statement: &str,
) -> Result<i64, String> {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let artifact_key = format!("memory/insights/{}.json", candidate.reflection_id.replace("REF", "MEM"));

    let memory_id = store
        .create_memory(&NewMemory {
            memory_type: "strategic_pattern".into(),
            memory_origin: "derived".into(),
            statement: statement.to_string(),
            confidence: score.total as f64,
            stability_score: Some(score.stability as f64),
            memory_sources: Some(serde_json::json!([{ "source_type": "reflection", "source_id": &candidate.reflection_id }]).to_string()),
            artifact_key: Some(artifact_key.clone()),
            status: "active".into(),
        })
        .await
        .map_err(|e| format!("create_memory failed: {e}"))?;

    let event_payload = serde_json::json!({
        "memory_id": format!("MEM-{memory_id:06}"),
        "source_reflection": candidate.reflection_id,
        "score": score.total,
        "artifact_key": artifact_key,
    });
    let _ = store.insert_outbox(&NewOutbox {
        object_type: "event:memory".into(),
        object_key: format!("mem_{now}_{memory_id}"),
        payload: event_payload.to_string(),
    }).await;

    let archive_payload = serde_json::json!({
        "schema_version": 1, "artifact_type": "memory",
        "memory_id": format!("MEM-{memory_id:06}"), "memory_type": "strategic_pattern", "memory_origin": "derived",
        "claim": { "statement": statement, "type": "heuristic" },
        "belief": { "confidence": score.total, "stability": score.stability, "effective_confidence": score.total },
        "lineage": { "reflections": [candidate.reflection_id], "decisions": [candidate.decision_id] },
        "promotion": { "score": score.total }, "created_at": now,
    });
    let _ = store.insert_outbox(&NewOutbox {
        object_type: "archive:memory".into(),
        object_key: artifact_key,
        payload: archive_payload.to_string(),
    }).await;

    Ok(memory_id)
}
