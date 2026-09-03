//! D1-backed MemoryRepository — maps between Memory domain records and D1 rows.
//!
//! Lives in infrastructure (not memory-engine) to keep the domain pure.

use async_trait::async_trait;
use memory_engine::error::MemoryError;
use memory_engine::model::{MemoryEventRef, NewMemory};
use memory_engine::MemoryRepository;
use store::StoreBackend;

/// Maps Memory aggregate persistence to the D1 `memory_index` table.
pub struct D1MemoryRepository<S> {
    store: S,
}

impl<S: StoreBackend> D1MemoryRepository<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    fn to_persistence(e: store::StoreError) -> MemoryError {
        MemoryError::Persistence(e.to_string())
    }
}

#[async_trait(?Send)]
impl<S: StoreBackend> MemoryRepository for D1MemoryRepository<S> {
    async fn create_memory(&self, memory: &NewMemory) -> Result<i64, MemoryError> {
        let entry = store::NewMemory {
            memory_type: memory.memory_type.clone(),
            memory_origin: memory.memory_origin.clone(),
            statement: memory.statement.clone(),
            confidence: memory.confidence,
            stability_score: memory.stability_score,
            memory_sources: memory.memory_sources.clone(),
            artifact_key: memory.artifact_key.clone(),
            status: memory.status.clone(),
        };
        self.store.create_memory(&entry).await.map_err(Self::to_persistence)
    }

    async fn enqueue_event(
        &self,
        object_type: &str,
        object_key: &str,
        payload: &serde_json::Value,
    ) -> Result<(), MemoryError> {
        let entry = store::NewOutbox {
            object_type: object_type.to_string(),
            object_key: object_key.to_string(),
            payload: payload.to_string(),
        };
        self.store.insert_outbox(&entry).await.map_err(Self::to_persistence).map(|_| ())
    }

    async fn list_reflection_events(&self, limit: u32) -> Result<Vec<MemoryEventRef>, MemoryError> {
        let rows = self.store.find_event_keys("reflection", "", limit).await.map_err(Self::to_persistence)?;
        Ok(rows
            .into_iter()
            .map(|r| MemoryEventRef {
                event_id: r.event_id,
                aggregate_id: r.aggregate_id,
                object_key: r.object_key,
                occurred_at: r.occurred_at,
            })
            .collect())
    }
}
