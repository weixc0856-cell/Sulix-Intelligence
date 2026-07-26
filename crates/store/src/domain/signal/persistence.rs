//! Signal instance persistence — V2 append-only.
//!
//! V1 methods (save_signal, load_recent_signals, load_signal_by_id,
//! entity_signals, append_signal_instance) have been removed.
//! Use upsert_signal_thread + append_signal_instance_v2 + insert_signal_event instead.

use worker::wasm_bindgen::JsValue;

#[derive(serde::Deserialize)]
struct FingerprintRow {
    score: f64,
    trend: String,
}

impl crate::D1Store {
    /// Get the latest instance's (score, trend) for a thread for change detection.
    pub async fn get_latest_instance_fingerprint(&self, thread_id: i64) -> Result<Option<(f64, String)>, crate::StoreError> {
        let row = self
            .db
            .prepare("SELECT score, trend FROM intelligence_signals WHERE signal_thread_id = ?1 ORDER BY created_at DESC LIMIT 1")
            .bind(&[JsValue::from_f64(thread_id as f64)])?
            .first::<FingerprintRow>(None)
            .await?;
        Ok(row.map(|r| (r.score, r.trend)))
    }
    /// Append a signal instance with enriched snapshot (V2).
    ///
    /// Stores additional context (avg_score, entity_id) compared to the
    /// original `append_signal_instance` so the signal timeline can show
    /// richer detail without recomputing from raw articles.
    #[allow(clippy::too_many_arguments)]
    pub async fn append_signal_instance_v2(
        &self,
        thread_id: i64,
        score: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
        avg_score: f64,
        entity_id: i64,
    ) -> Result<i64, crate::StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let row = self
            .db
            .prepare(
                "INSERT INTO intelligence_signals \
                 (signal_thread_id, anchor_entity_id, title, summary, signal_type, score, confidence, impact, \
                  trend, article_count, source_count, avg_score, created_at, updated_at) \
                 VALUES (?1, ?2, '', '', 'entity', ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) RETURNING id",
            )
            .bind(&[
                JsValue::from_f64(thread_id as f64),
                JsValue::from_f64(entity_id as f64),
                JsValue::from_f64(score),
                impact.into(),
                trend.into(),
                JsValue::from_f64(article_count as f64),
                JsValue::from_f64(source_count as f64),
                JsValue::from_f64(avg_score),
                JsValue::from_f64(now as f64),
                JsValue::from_f64(now as f64),
            ])?
            .first::<serde_json::Value>(None)
            .await?;

        row.and_then(|v| v["id"].as_i64())
            .ok_or_else(|| crate::StoreError::D1("append_signal_instance_v2 failed".into()))
    }
}
