use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

impl crate::D1Store {
    /// List articles linked to an entity (Evidence timeline).
    pub async fn entity_articles(
        &self,
        entity_id: i64,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<crate::EntityArticle>, crate::StoreError> {
        Ok(self
            .db
            .prepare(
                "SELECT a.id, a.title, a.url, a.published_at, a.ai_summary, a.score, f.title AS feed_name \
                 FROM article_entities ae \
                 JOIN articles a ON a.id = ae.article_id \
                 LEFT JOIN feeds f ON f.id = a.feed_id \
                 WHERE ae.entity_id = ?1 \
                 ORDER BY a.published_at DESC \
                 LIMIT ?2 OFFSET ?3",
            )
            .bind(&[
                JsValue::from_f64(entity_id as f64),
                JsValue::from_f64(limit as f64),
                JsValue::from_f64(offset as f64),
            ])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()?)
    }

    /// Activity summary for an entity over the last N days.
    pub async fn entity_activity_summary(
        &self,
        entity_id: i64,
        now: i64,
        days: i64,
    ) -> Result<crate::EntityActivitySummary, crate::StoreError> {
        let cutoff = now - days * 86400;
        let row: Option<serde_json::Value> = self
            .db
            .prepare(
                "SELECT COUNT(*) AS article_count, \
                        COUNT(DISTINCT a.feed_id) AS source_count, \
                        COALESCE(AVG(a.score), 0) AS avg_score, \
                        COALESCE(MAX(a.score), 0) AS max_score, \
                        MIN(a.published_at) AS first_seen_at, \
                        MAX(a.published_at) AS last_seen_at \
                 FROM article_entities ae \
                 JOIN articles a ON a.id = ae.article_id \
                 WHERE ae.entity_id = ?1 AND a.published_at >= ?2",
            )
            .bind(&[JsValue::from_f64(entity_id as f64), JsValue::from_f64(cutoff as f64)])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;

        let v = row.unwrap_or_default();
        let article_count = v["article_count"].as_i64().unwrap_or(0);
        let source_count = v["source_count"].as_i64().unwrap_or(0);
        let avg_score = v["avg_score"].as_f64().unwrap_or(0.0);
        let max_score = v["max_score"].as_f64().unwrap_or(0.0);
        let first_seen_at = v["first_seen_at"].as_i64();
        let last_seen_at = v["last_seen_at"].as_i64();

        // Compute trend: compare articles in last 3 days vs 3-6 days ago
        let recent_cutoff = now - 3 * 86400;
        let earlier_cutoff = now - 6 * 86400;
        let recent: i64 = self
            .db
            .prepare(
                "SELECT COUNT(*) AS cnt FROM article_entities ae \
                 JOIN articles a ON a.id = ae.article_id \
                 WHERE ae.entity_id = ?1 AND a.published_at >= ?2",
            )
            .bind(&[JsValue::from_f64(entity_id as f64), JsValue::from_f64(recent_cutoff as f64)])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?
            .and_then(|r| r["cnt"].as_i64())
            .unwrap_or(0);

        let earlier: i64 = self
            .db
            .prepare(
                "SELECT COUNT(*) AS cnt FROM article_entities ae \
                 JOIN articles a ON a.id = ae.article_id \
                 WHERE ae.entity_id = ?1 AND a.published_at >= ?2 AND a.published_at < ?3",
            )
            .bind(&[
                JsValue::from_f64(entity_id as f64),
                JsValue::from_f64(earlier_cutoff as f64),
                JsValue::from_f64(recent_cutoff as f64),
            ])
            .s_err()?
            .first::<serde_json::Value>(None)
            .await
            .s_err()?
            .and_then(|r| r["cnt"].as_i64())
            .unwrap_or(0);

        let trend = if earlier == 0 || recent > earlier * 12 / 10 {
            "rising"
        } else if recent < earlier * 8 / 10 {
            "declining"
        } else {
            "stable"
        };

        Ok(crate::EntityActivitySummary {
            article_count,
            source_count,
            avg_score,
            max_score,
            first_seen_at,
            last_seen_at,
            trend: trend.into(),
        })
    }
}
