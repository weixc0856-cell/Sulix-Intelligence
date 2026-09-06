//! D1-backed ContextRepository — maps between ContextRepository domain records
//! and D1 rows.
//!
//! Lives in infrastructure (not context-engine) to keep the domain pure.

use async_trait::async_trait;
use context_engine::error::ContextError;
use context_engine::models::{DecisionRecord, MemoryRecord, NewContextSnapshot};
use context_engine::repository::ContextRepository;
use store::{ContextSnapshotStore, DecisionQueryService, MemoryPersistence};

/// Maps context retrieval + snapshot persistence to the D1 store.
pub struct D1ContextRepository<S> {
    store: S,
}

impl<S: DecisionQueryService + MemoryPersistence + ContextSnapshotStore> D1ContextRepository<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    fn to_persistence(e: store::StoreError) -> ContextError {
        ContextError::Persistence(e.to_string())
    }
}

#[async_trait(?Send)]
impl<S: DecisionQueryService + MemoryPersistence + ContextSnapshotStore> ContextRepository for D1ContextRepository<S> {
    async fn list_decisions(&self, statuses: &[&str], limit: u32) -> Result<Vec<DecisionRecord>, ContextError> {
        if statuses.is_empty() {
            return Ok(Vec::new());
        }
        // Evidence allowlist: one equality query per allowed status, then merge on
        // the system side. Deliberately keeps DecisionQueryService single-status
        // (shared by other consumers) and avoids dynamic IN-placeholder SQL.
        let mut rows: Vec<store::Decision> = Vec::new();
        for status in statuses {
            rows.extend(self.store.list_decisions(Some(status), limit).await.map_err(Self::to_persistence)?);
        }
        let rows = merge_eligible(rows, limit);
        Ok(rows
            .into_iter()
            .map(|d| DecisionRecord {
                id: d.id,
                title: d.title,
                decision_type: d.decision_type,
                status: d.status,
                confidence: d.confidence,
            })
            .collect())
    }

    async fn list_memories(&self, status: Option<&str>, limit: u32) -> Result<Vec<MemoryRecord>, ContextError> {
        // memory_type is always None for context retrieval — the port drops it.
        let rows = self.store.list_memories(None, status, limit).await.map_err(Self::to_persistence)?;
        Ok(rows
            .into_iter()
            .map(|m| MemoryRecord {
                id: m.id,
                statement: m.statement,
                memory_type: m.memory_type,
                confidence: m.confidence,
                usage_count: m.usage_count,
            })
            .collect())
    }

    async fn save_context_snapshot(&self, snap: &NewContextSnapshot) -> Result<(), ContextError> {
        // object_key/object_size are always None: the R2 artifact path was removed
        // (no caller supplied an object store), so the D1 row keeps them NULL.
        let req = store::NewContextSnapshot {
            id: snap.id.clone(),
            query: snap.query.clone(),
            intent: snap.intent.clone(),
            domain: snap.domain.clone(),
            context_json: snap.context_json.clone(),
            object_key: None,
            object_size: None,
            evidence_refs: snap.evidence_refs.clone(),
            confidence: snap.confidence,
            user_scope: snap.user_scope.clone(),
        };
        self.store.save_context_snapshot(&req).await.map_err(Self::to_persistence)
    }
}

/// Merge per-status decision rows into one deterministic, capped evidence set.
///
/// Ordering is `created_at DESC, id DESC` — the `id` tie-breaker keeps the
/// evidence set stable across calls for equal `created_at` rows (same DB state
/// ⇒ same evidence list ⇒ reproducible Advisor context).
fn merge_eligible(rows: Vec<store::Decision>, limit: u32) -> Vec<store::Decision> {
    let mut rows = rows;
    rows.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.id.cmp(&a.id)));
    rows.truncate(limit as usize);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::Decision;

    fn decision(id: i64, created_at: i64) -> Decision {
        Decision {
            id,
            signal_thread_id: None,
            actor_id: None,
            decision_type: "test".into(),
            title: format!("decision {id}"),
            hypothesis: None,
            rationale: None,
            confidence: 0.5,
            status: "completed".into(),
            priority: "medium".into(),
            expected_outcomes: None,
            created_at,
            updated_at: created_at,
        }
    }

    #[test]
    fn merge_sorts_created_at_desc_then_id_desc() {
        let rows = vec![decision(1, 100), decision(2, 300), decision(3, 300), decision(4, 200)];
        let merged = merge_eligible(rows, 10);
        let ids: Vec<i64> = merged.iter().map(|d| d.id).collect();
        // created_at desc; the two equal-created_at rows (2,3) order by id desc (3 then 2).
        assert_eq!(ids, vec![3, 2, 4, 1]);
    }

    #[test]
    fn merge_caps_at_limit() {
        let rows = vec![decision(1, 100), decision(2, 200), decision(3, 300), decision(4, 400)];
        let merged = merge_eligible(rows, 2);
        let ids: Vec<i64> = merged.iter().map(|d| d.id).collect();
        assert_eq!(ids, vec![4, 3]);
    }

    #[test]
    fn merge_is_deterministic() {
        let rows = vec![decision(1, 100), decision(2, 100), decision(3, 100)];
        let first: Vec<i64> = merge_eligible(rows.clone(), 10).iter().map(|d| d.id).collect();
        let second: Vec<i64> = merge_eligible(rows, 10).iter().map(|d| d.id).collect();
        assert_eq!(first, second);
    }
}
