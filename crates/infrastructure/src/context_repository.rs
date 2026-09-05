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
    async fn list_decisions(&self, limit: u32) -> Result<Vec<DecisionRecord>, ContextError> {
        let rows = self.store.list_decisions(None, limit).await.map_err(Self::to_persistence)?;
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
