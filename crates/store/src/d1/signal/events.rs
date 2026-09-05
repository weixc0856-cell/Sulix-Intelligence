//! Signal Events — timeline events for signal threads.
//!
//! These are stored in the `signal_events` table (migration 0013) and
//! provide a human-readable timeline of what happened to a signal thread
//! over its lifetime.

use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::SignalEvent;

impl crate::D1Store {
    /// Insert a timeline event for a signal thread.
    pub async fn insert_signal_event(
        &self,
        thread_id: i64,
        event_type: &str,
        payload: Option<&str>,
    ) -> Result<(), crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        self.db
            .prepare(
                "INSERT OR IGNORE INTO signal_events (thread_id, event_type, payload, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(&[
                JsValue::from_f64(thread_id as f64),
                event_type.into(),
                payload.map_or(JsValue::null(), |s| s.into()),
                JsValue::from_f64(now as f64),
            ])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    /// Load timeline events for a signal thread, newest first.
    pub async fn load_signal_events(&self, thread_id: i64, limit: u32) -> Result<Vec<SignalEvent>, crate::StoreError> {
        let result = self
            .db
            .prepare(
                "SELECT id, thread_id, event_type, payload, created_at \
                 FROM signal_events \
                 WHERE thread_id = ?1 \
                 ORDER BY created_at DESC LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(thread_id as f64), JsValue::from_f64(limit as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?;
        Ok(result.results().s_err()?)
    }
}
