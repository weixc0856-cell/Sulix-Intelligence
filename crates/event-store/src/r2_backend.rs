//! R2-backed EventStore — production implementation.
//!
//! Writes events outbox-first: D1 outbox + event_archive_index immediately,
//! R2 archival happens asynchronously via the archive worker.

use async_trait::async_trait;
use object_store::ObjectStore;
use store::StoreBackend;
use worker::console_log;

use crate::{Event, EventId, EventStore, EventStoreError, keys};

/// Production EventStore backed by D1 (index + outbox) and R2 (payload archive).
pub struct EventR2Backend<S: StoreBackend> {
    pub store: S,
    pub object_store: object_store::R2Store,
}

impl<S: StoreBackend> EventR2Backend<S> {
    pub fn new(store: S, object_store: object_store::R2Store) -> Self {
        Self { store, object_store }
    }
}

#[async_trait(?Send)]
impl<S: StoreBackend + 'static> EventStore for EventR2Backend<S> {
    async fn append_event(&self, event: &Event) -> Result<EventId, EventStoreError> {
        let event_id = event.event_id.clone();
        let object_key = keys::event(&event.aggregate_type, event.occurred_at, &event_id);

        // 1. D1 outbox INSERT (durable first — outbox-first pattern)
        let payload =
            serde_json::to_string(&event).map_err(|e| EventStoreError::Serialisation(e.to_string()))?;
        self.store
            .insert_outbox(&store::NewOutbox {
                object_type: format!("event:{}", event.aggregate_type),
                object_key: object_key.clone(),
                payload,
            })
            .await?;

        // 2. D1 event_archive_index INSERT (metadata for fast query)
        self.store
            .insert_event_index(
                &event_id,
                &event.aggregate_type,
                event.aggregate_id,
                &event.event_type,
                &object_key,
                event.occurred_at,
            )
            .await?;

        Ok(event_id)
    }

    async fn load_events(
        &self,
        aggregate_type: &str,
        aggregate_id: i64,
        limit: u32,
    ) -> Result<Vec<Event>, EventStoreError> {
        // 1. D1 index: get object keys
        let index_rows = self
            .store
            .find_event_keys(aggregate_type, aggregate_id, limit)
            .await?;

        if index_rows.is_empty() {
            return Ok(Vec::new());
        }

        // 2. R2: batch fetch payloads
        let mut events: Vec<Event> = Vec::new();
        for row in &index_rows {
            match self.object_store.read_object(&row.object_key).await {
                Ok(Some(bytes)) => {
                    if let Ok(event) = serde_json::from_slice::<Event>(&bytes) {
                        events.push(event);
                    }
                }
                Ok(None) => {
                    // R2 miss — skip (archive worker may not have caught up yet)
                }
                Err(e) => {
                    // R2 error — non-fatal
                    console_log!("[event-store] R2 read failed: {e}");
                }
            }
        }

        // 3. Sort by occurred_at DESC
        events.sort_by(|a, b| b.occurred_at.cmp(&a.occurred_at));
        events.truncate(limit as usize);
        Ok(events)
    }
}
