use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

impl crate::D1Store {
    /// Evaluate lifecycle transitions for all active/decaying threads.
    pub async fn update_signal_lifecycle(&self, now: i64) -> Result<(), crate::StoreError> {
        self.db
            .prepare("UPDATE signal_threads SET status = 'decaying', updated_at = ?1 WHERE status = 'active' AND last_seen_at < ?2")
            .bind(&[JsValue::from_f64(now as f64), JsValue::from_f64((now - 7 * 86400) as f64)]).s_err()?
            .run().await.s_err()?;
        self.db
            .prepare("UPDATE signal_threads SET status = 'resolved', updated_at = ?1 WHERE status = 'decaying' AND last_seen_at < ?2")
            .bind(&[JsValue::from_f64(now as f64), JsValue::from_f64((now - 14 * 86400) as f64)]).s_err()?
            .run().await.s_err()?;
        self.db
            .prepare("UPDATE signal_threads SET status = 'active', updated_at = ?1 WHERE status = 'decaying' AND last_seen_at >= ?2")
            .bind(&[JsValue::from_f64(now as f64), JsValue::from_f64((now - 3 * 86400) as f64)]).s_err()?
            .run().await.s_err()?;
        self.db
            .prepare("UPDATE signal_threads SET status = 'archived', updated_at = ?1 WHERE status = 'resolved' AND last_seen_at < ?2")
            .bind(&[JsValue::from_f64(now as f64), JsValue::from_f64((now - 30 * 86400) as f64)]).s_err()?
            .run().await.s_err()?;
        Ok(())
    }
}
