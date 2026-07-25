//! Analytics / Statistics queries for the dashboard — extracted from feed.rs.

use serde::Deserialize;
use worker::wasm_bindgen::JsValue;

use crate::{
    in_placeholders, is_cron_healthy, ArticleDetail, D1Store, DayCount, FeedStats, HealthStats, PendingArticle,
    ScoreDist, StoreError,
};

impl D1Store {
    pub async fn tags_summary(&self) -> Result<Vec<(String, i64)>, StoreError> {
        #[derive(Deserialize)]
        struct Row {
            ai_tags: String,
        }
        let rows: Vec<Row> = self
            .db
            .prepare("SELECT ai_tags FROM articles WHERE ai_tags IS NOT NULL AND ai_tags != '[]'")
            .all()
            .await?
            .results()?;
        let mut map: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
        for row in &rows {
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&row.ai_tags) {
                for tag in tags {
                    *map.entry(tag).or_default() += 1;
                }
            }
        }
        Ok(map.into_iter().collect())
    }

    pub async fn feed_stats(&self) -> Result<Vec<FeedStats>, StoreError> {
        Ok(self.db.prepare(
            "SELECT f.id, f.title, f.url, f.category, f.status, f.last_fetched_at, COUNT(a.id) AS article_count FROM feeds f LEFT JOIN articles a ON a.feed_id = f.id GROUP BY f.id ORDER BY f.last_fetched_at DESC",
        ).all().await?.results()?)
    }

    pub async fn health_stats(&self) -> Result<HealthStats, StoreError> {
        self.db.prepare(
            "SELECT (SELECT COUNT(*) FROM feeds) AS feed_count, (SELECT COUNT(*) FROM feeds WHERE status = 'active') AS active_feed_count, (SELECT COUNT(*) FROM articles) AS article_count, (SELECT MAX(last_fetched_at) FROM feeds) AS last_cron_run_at",
        ).first::<HealthStats>(None).await?.ok_or_else(|| StoreError::D1("health_stats returned no row".into()))
    }

    pub async fn score_distribution(&self) -> Result<ScoreDist, StoreError> {
        Ok(self.db.prepare(
            "SELECT CAST(SUM(CASE WHEN score >= 8 THEN 1 ELSE 0 END) AS INTEGER) AS top, CAST(SUM(CASE WHEN score >= 5 AND score < 8 THEN 1 ELSE 0 END) AS INTEGER) AS medium, CAST(SUM(CASE WHEN score > 0 AND score < 5 THEN 1 ELSE 0 END) AS INTEGER) AS low, CAST(SUM(CASE WHEN score = 0 THEN 1 ELSE 0 END) AS INTEGER) AS unscored FROM articles",
        ).first::<ScoreDist>(None).await?.unwrap_or(ScoreDist { top: 0, medium: 0, low: 0, unscored: 0 }))
    }

    pub async fn pipeline_status(&self, now: i64) -> Result<serde_json::Value, StoreError> {
        let health = self.health_stats().await?;
        let dist = self.score_distribution().await.unwrap_or(ScoreDist { top: 0, medium: 0, low: 0, unscored: 0 });
        let problem_feeds: i64 = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM feeds WHERE status != 'active' OR last_fetched_at IS NULL")
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|v| v["cnt"].as_i64())
            .unwrap_or(0);
        let with_summary: i64 = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM articles WHERE ai_summary IS NOT NULL AND ai_summary != ''")
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|v| v["cnt"].as_i64())
            .unwrap_or(0);
        let scored: i64 = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM articles WHERE score != 0")
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|v| v["cnt"].as_i64())
            .unwrap_or(0);
        let high_score: i64 = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM articles WHERE score >= 8")
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|v| v["cnt"].as_i64())
            .unwrap_or(0);
        let embedded: i64 = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM articles WHERE vector_id IS NOT NULL")
            .first::<serde_json::Value>(None)
            .await?
            .and_then(|v| v["cnt"].as_i64())
            .unwrap_or(0);
        Ok(serde_json::json!({
            "cron": { "last_run_at": health.last_cron_run_at, "healthy": is_cron_healthy(health.last_cron_run_at, now) },
            "feeds": { "total": health.feed_count, "active": health.active_feed_count, "problem_feeds": problem_feeds },
            "articles": { "total": health.article_count, "with_summary": with_summary, "scored": scored, "high_score": high_score, "unscored": dist.unscored },
            "embedding_coverage": { "total": health.article_count, "embedded": embedded, "pending": health.article_count.saturating_sub(embedded) },
        }))
    }

    pub async fn article_trend(&self, days: i64) -> Result<Vec<DayCount>, StoreError> {
        Ok(self.db.prepare("SELECT DATE(published_at, 'unixepoch') AS day, COUNT(*) AS cnt FROM articles WHERE published_at IS NOT NULL GROUP BY day ORDER BY day DESC LIMIT ?1").bind(&[JsValue::from_f64(days as f64)])?.all().await?.results()?)
    }

    pub async fn pending_ai_articles(&self, batch_size: u32) -> Result<Vec<PendingArticle>, StoreError> {
        Ok(self.db.prepare("SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score, raw_content_r2_key FROM articles WHERE (ai_summary IS NULL OR ai_summary = '') ORDER BY published_at ASC LIMIT ?1").bind(&[JsValue::from_f64(batch_size as f64)])?.all().await?.results()?)
    }

    pub async fn expired_article_r2_keys(&self, now: i64, days: i64) -> Result<Vec<String>, StoreError> {
        let cutoff = now - days * 86400;
        let rows: Vec<serde_json::Value> = self.db.prepare("SELECT raw_content_r2_key FROM articles WHERE published_at < ?1 AND ai_summary != '' AND ai_summary IS NOT NULL AND raw_content_r2_key IS NOT NULL").bind(&[JsValue::from_f64(cutoff as f64)])?.all().await?.results()?;
        Ok(rows.iter().filter_map(|r| r["raw_content_r2_key"].as_str().map(String::from)).collect())
    }

    pub async fn expire_old_articles(&self, now: i64, days: i64) -> Result<u64, StoreError> {
        let cutoff = now - days * 86400;
        let result = self
            .db
            .prepare("DELETE FROM articles WHERE published_at < ?1 AND ai_summary != '' AND ai_summary IS NOT NULL")
            .bind(&[JsValue::from_f64(cutoff as f64)])?
            .run()
            .await?;
        Ok(result.meta().ok().flatten().and_then(|m| m.changes).unwrap_or(0) as u64)
    }

    pub async fn recent_articles_for_preview(&self, limit: u32) -> Result<Vec<ArticleDetail>, StoreError> {
        Ok(self.db.prepare("SELECT a.id, a.feed_id, f.title AS feed_name, a.guid, a.title, a.url, a.published_at, a.ai_summary, a.ai_tags, a.score FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id WHERE a.title != '' ORDER BY a.published_at DESC LIMIT ?1").bind(&[JsValue::from_f64(limit as f64)])?.all().await?.results()?)
    }

    pub async fn article_ids_exist(&self, ids: &[i64]) -> Result<std::collections::HashSet<i64>, StoreError> {
        if ids.is_empty() {
            return Ok(std::collections::HashSet::new());
        }
        let placeholders = in_placeholders(ids.len());
        let sql = format!("SELECT id FROM articles WHERE id IN ({placeholders})");
        let mut stmt = self.db.prepare(&sql);
        let vals: Vec<JsValue> = ids.iter().map(|id| JsValue::from_f64(*id as f64)).collect();
        stmt = stmt.bind(&vals)?;
        let rows: Vec<serde_json::Value> = stmt.all().await?.results()?;
        Ok(rows.iter().filter_map(|r| r["id"].as_i64()).collect())
    }
}
