//! Event Archive Index — metadata index for the Memory Event Stream (R2).
//!
//! Every event written via EventStore is recorded here so consumers can
//! query by (aggregate_type, aggregate_id) without listing R2 prefixes.

use worker::wasm_bindgen::JsValue;

use crate::{EventIndexEntry, StoreError};

impl crate::D1Store {
    /// Insert a row into the event_archive_index (metadata for an R2-stored event).
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_event_index(
        &self,
        event_id: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        event_type: &str,
        object_key: &str,
        occurred_at: i64,
    ) -> Result<(), StoreError> {
        self.db
            .prepare(
                "INSERT OR IGNORE INTO event_archive_index \
                 (event_id, aggregate_type, aggregate_id, event_type, object_key, occurred_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(&[
                event_id.into(),
                aggregate_type.into(),
                aggregate_id.into(),
                event_type.into(),
                object_key.into(),
                JsValue::from_f64(occurred_at as f64),
            ])?
            .run()
            .await?;
        Ok(())
    }

    /// Find event index entries for an aggregate, newest first.
    pub async fn find_event_keys(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        limit: u32,
    ) -> Result<Vec<EventIndexEntry>, StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT id, event_id, aggregate_type, aggregate_id, event_type, object_key, occurred_at, created_at \
                 FROM event_archive_index \
                 WHERE aggregate_type = ?1 AND aggregate_id = ?2 \
                 ORDER BY occurred_at DESC \
                 LIMIT ?3",
            )
            .bind(&[aggregate_type.into(), aggregate_id.into(), JsValue::from_f64(limit as f64)])?
            .all()
            .await?
            .results()?)
    }
}
