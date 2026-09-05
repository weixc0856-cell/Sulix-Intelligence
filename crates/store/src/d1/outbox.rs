//! Object Outbox — enqueue and drain archive-write rows.
//!
//! Every D1 state mutation that should also produce an R2 archive object
//! first writes an outbox row here.  A cron-side archive worker then drains
//! the outbox: reads pending rows, writes the payload to R2, and marks the
//! row as 'archived'.

use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::{NewOutbox, OutboxEntry};

impl crate::D1Store {
    /// Enqueue a new outbox entry.  Called within the same D1 batch as the
    /// state mutation so the outbox row is committed atomically.
    pub async fn insert_outbox(&self, entry: &NewOutbox) -> Result<i64, crate::StoreError> {
        let row = self
            .db
            .prepare(
                "INSERT INTO object_outbox (object_type, object_key, payload) \
                 VALUES (?1, ?2, ?3) RETURNING id",
            )
            .bind(&[entry.object_type.as_str().into(), entry.object_key.as_str().into(), entry.payload.as_str().into()])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;
        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| crate::StoreError::D1("insert_outbox failed: no id returned".into()))
    }

    /// Drain up to `limit` pending outbox entries, oldest first.
    ///
    /// Returns the rows that the caller should attempt to write to R2.
    /// After a successful R2 write the caller calls
    /// [`mark_outbox_archived`](Self::mark_outbox_archived).  After a
    /// failure it calls [`mark_outbox_retry`](Self::mark_outbox_retry).
    pub async fn drain_outbox(&self, limit: u32) -> Result<Vec<OutboxEntry>, crate::StoreError> {
        self.db
            .prepare(
                "SELECT id, object_type, object_key, payload, status, created_at, retry_count \
                 FROM object_outbox \
                 WHERE status = 'pending' \
                 ORDER BY created_at ASC \
                 LIMIT ?1",
            )
            .bind(&[JsValue::from_f64(limit as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()
    }

    /// Mark an outbox entry as successfully archived.
    pub async fn mark_outbox_archived(&self, id: i64) -> Result<(), crate::StoreError> {
        self.db
            .prepare("UPDATE object_outbox SET status = 'archived' WHERE id = ?1")
            .bind(&[JsValue::from_f64(id as f64)])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    /// Mark an outbox entry as failed and increment retry count.
    pub async fn mark_outbox_failed(&self, id: i64) -> Result<(), crate::StoreError> {
        self.db
            .prepare(
                "UPDATE object_outbox SET status = 'failed', retry_count = retry_count + 1 \
                 WHERE id = ?1 AND retry_count >= 3",
            )
            .bind(&[JsValue::from_f64(id as f64)])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }
}
