//! Feed CRUD — feed lifecycle management.
//! Analytics and Rules have been extracted to `feed_analytics.rs` and `feed_rules.rs`.

use crate::s_err::StoreResultExt;
use worker::wasm_bindgen::JsValue;

use crate::{D1Store, Feed, StoreError};

impl D1Store {
    pub async fn feeds_due_for_fetch(&self, now: i64, category: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        let (sql, _has_cat) = if category.is_some() {
            ("SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds WHERE status = 'active' AND category = ?1 AND (last_fetched_at IS NULL OR ?2 - last_fetched_at >= fetch_interval_sec)", true)
        } else {
            ("SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds WHERE status = 'active' AND (last_fetched_at IS NULL OR ?1 - last_fetched_at >= fetch_interval_sec)", false)
        };
        let stmt = self.db.prepare(sql);
        let stmt = if let Some(cat) = category {
            stmt.bind(&[cat.into(), JsValue::from_f64(now as f64)]).s_err()?
        } else {
            stmt.bind(&[JsValue::from_f64(now as f64)]).s_err()?
        };
        stmt.all().await.s_err()?.results().s_err()
    }

    pub async fn all_feeds(&self, status_filter: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        let sql = if status_filter.is_some() {
            "SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds WHERE status = ?1 ORDER BY last_fetched_at DESC"
        } else {
            "SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds ORDER BY last_fetched_at DESC"
        };
        let stmt = self.db.prepare(sql);
        let stmt = if let Some(sf) = status_filter { stmt.bind(&[sf.into()]).s_err()? } else { stmt };
        stmt.all().await.s_err()?.results().s_err()
    }

    pub async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        self.db.prepare("SELECT id, url, title, category, fetch_interval_sec, last_fetched_at, etag, last_modified, status, extraction_level FROM feeds WHERE id = ?1").bind(&[JsValue::from_f64(id as f64)]).s_err()?.first::<Feed>(None).await.s_err()
    }

    pub async fn insert_feed(
        &self,
        url: &str,
        title: &str,
        category: &str,
        interval: i64,
    ) -> Result<Option<i64>, StoreError> {
        let row = self.db.prepare("INSERT OR IGNORE INTO feeds (url, title, category, fetch_interval_sec) VALUES (?1, ?2, ?3, ?4) RETURNING id").bind(&[url.into(), title.into(), category.into(), JsValue::from_f64(interval as f64)]).s_err()?.first::<serde_json::Value>(None).await.s_err()?;
        Ok(row.and_then(|v| v["id"].as_i64()))
    }

    pub async fn update_feed(
        &self,
        id: i64,
        title: Option<&str>,
        category: Option<&str>,
        interval: Option<i64>,
        extraction_level: Option<&str>,
    ) -> Result<(), StoreError> {
        let mut parts: Vec<String> = Vec::new();
        let mut vals: Vec<JsValue> = Vec::new();
        if let Some(v) = title {
            parts.push("title = ?".into());
            vals.push(v.into());
        }
        if let Some(v) = category {
            parts.push("category = ?".into());
            vals.push(v.into());
        }
        if let Some(v) = interval {
            parts.push("fetch_interval_sec = ?".into());
            vals.push(JsValue::from_f64(v as f64));
        }
        if let Some(v) = extraction_level {
            parts.push("extraction_level = ?".into());
            vals.push(v.into());
        }
        if parts.is_empty() {
            return Ok(());
        }
        vals.push(JsValue::from_f64(id as f64));
        self.db
            .prepare(format!("UPDATE feeds SET {} WHERE id = ?", parts.join(", ")))
            .bind(&vals)
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    pub async fn set_feed_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        self.db
            .prepare("UPDATE feeds SET status = ?1 WHERE id = ?2")
            .bind(&[status.into(), JsValue::from_f64(id as f64)])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    pub async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError> {
        self.db.prepare("UPDATE feeds SET last_fetched_at = ?1, etag = COALESCE(?2, etag), last_modified = COALESCE(?3, last_modified) WHERE id = ?4")
            .bind(&[JsValue::from_f64(fetched_at as f64), etag.map_or(JsValue::null(), |v| v.into()), last_modified.map_or(JsValue::null(), |v| v.into()), JsValue::from_f64(feed_id as f64)]).s_err()?.run().await.s_err()?;
        Ok(())
    }
}
