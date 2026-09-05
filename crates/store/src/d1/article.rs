use crate::s_err::StoreResultExt;
use serde::Deserialize;
use serde::Serialize;
use worker::wasm_bindgen::JsValue;

/// A minimal article row for backfill processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillArticle {
    pub id: i64,
    pub title: String,
    pub score: f64,
    pub raw_content_r2_key: Option<String>,
    pub vector_id: Option<String>,
}

impl crate::D1Store {
    /// Query articles without AI summaries for backfill processing.
    /// Self-limiting via LIMIT, resumable via cursor (minimum id).
    pub async fn get_backfill_candidates(
        &self,
        cursor: i64,
        limit: u32,
    ) -> Result<Vec<BackfillArticle>, crate::StoreError> {
        self.db
            .prepare(
                "SELECT id, title, score, raw_content_r2_key, vector_id \
                 FROM articles WHERE id > ?1 AND ai_summary = '' \
                 ORDER BY id ASC LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(cursor as f64), JsValue::from_f64(limit as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results::<BackfillArticle>()
            .s_err()
    }
    /// Every `guid` currently stored for a feed.
    ///
    /// Ingestion uses this to skip already-ingested entries up front instead
    /// of issuing one `INSERT OR IGNORE` per entry. Each D1 query counts as a
    /// subrequest to a Cloudflare service (hard-capped at 1000 per invocation
    /// on the Workers Free plan), so a large feed re-fetched wholesale used to
    /// burn ~1 query per historical entry and hit the cap before finishing.
    pub async fn guids_for_feed(&self, feed_id: i64) -> Result<Vec<String>, crate::StoreError> {
        let rows = self
            .db
            .prepare("SELECT guid FROM articles WHERE feed_id = ?1")
            .bind(&[JsValue::from_f64(feed_id as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results::<serde_json::Value>()
            .s_err()?;
        Ok(rows.into_iter().filter_map(|v| v["guid"].as_str().map(String::from)).collect())
    }

    pub async fn insert_article(&self, article: &crate::NewArticle) -> Result<Option<i64>, crate::StoreError> {
        let row = self
            .db
            .prepare("INSERT OR IGNORE INTO articles (feed_id, guid, title, url, published_at, raw_content_r2_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING id")
            .bind(&[
            JsValue::from_f64(article.feed_id as f64),
            article.guid.clone().into(),
            article.title.clone().into(),
            article.url.clone().map_or(JsValue::null(), |v| v.into()),
            article.published_at.map_or(JsValue::null(), |v| JsValue::from_f64(v as f64)),
            article.raw_content_r2_key.clone().map_or(JsValue::null(), |v| v.into()),
        ]).s_err()?
            .first::<serde_json::Value>(None)
            .await.s_err()?;
        Ok(row.and_then(|v| v["id"].as_i64()))
    }

    pub async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), crate::StoreError> {
        self.db
            .prepare("UPDATE articles SET ai_summary = ?1, ai_tags = ?2, vector_id = ?3, score = ?4 WHERE id = ?5")
            .bind(&[
                summary.into(),
                tags_json.into(),
                vector_id.into(),
                JsValue::from_f64(score),
                JsValue::from_f64(article_id as f64),
            ])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    pub async fn get_raw_content_key(&self, article_id: i64) -> Result<Option<String>, crate::StoreError> {
        #[derive(Deserialize)]
        struct Row {
            raw_content_r2_key: Option<String>,
        }
        Ok(self
            .db
            .prepare("SELECT raw_content_r2_key FROM articles WHERE id = ?1")
            .bind(&[JsValue::from_f64(article_id as f64)])
            .s_err()?
            .first::<Row>(None)
            .await
            .s_err()?
            .and_then(|r| r.raw_content_r2_key))
    }

    pub async fn set_raw_content_r2_key(&self, article_id: i64, r2_key: Option<&str>) -> Result<(), crate::StoreError> {
        self.db
            .prepare("UPDATE articles SET raw_content_r2_key = ?1 WHERE id = ?2")
            .bind(&[r2_key.into(), JsValue::from_f64(article_id as f64)])
            .s_err()?
            .run()
            .await
            .s_err()?;
        Ok(())
    }

    pub async fn latest_articles(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<crate::PendingArticle>, crate::StoreError> {
        self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles ORDER BY published_at DESC LIMIT ?1 OFFSET ?2",
        ).bind(&[JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)]).s_err()?.all().await.s_err()?.results().s_err()
    }

    pub async fn article_count(&self) -> Result<i64, crate::StoreError> {
        let row =
            self.db.prepare("SELECT COUNT(*) AS cnt FROM articles").first::<serde_json::Value>(None).await.s_err()?;
        Ok(row.and_then(|v| v["cnt"].as_i64()).unwrap_or(0))
    }

    pub async fn trending_articles(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<crate::PendingArticle>, crate::StoreError> {
        self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE score != 0 ORDER BY score DESC, published_at DESC LIMIT ?1 OFFSET ?2",
        ).bind(&[JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)]).s_err()?.all().await.s_err()?.results().s_err()
    }

    pub async fn trending_count(&self) -> Result<i64, crate::StoreError> {
        let row = self
            .db
            .prepare("SELECT COUNT(*) AS cnt FROM articles WHERE score != 0")
            .first::<serde_json::Value>(None)
            .await
            .s_err()?;
        Ok(row.and_then(|v| v["cnt"].as_i64()).unwrap_or(0))
    }

    pub async fn article_by_id(&self, id: i64) -> Result<Option<crate::Article>, crate::StoreError> {
        self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE id = ?1",
        ).bind(&[JsValue::from_f64(id as f64)]).s_err()?.first::<crate::Article>(None).await.s_err()
    }

    /// Batch fetch by IDs.  Used by the bookmarks / batch endpoint.
    pub async fn articles_by_ids(&self, ids: &[i64]) -> Result<Vec<crate::Article>, crate::StoreError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE id IN ({})",
            crate::in_placeholders(ids.len())
        );
        let binds: Vec<JsValue> = ids.iter().map(|id| JsValue::from_f64(*id as f64)).collect();
        self.db.prepare(&sql).bind(&binds).s_err()?.all().await.s_err()?.results().s_err()
    }

    /// Article with feed metadata joined in for the detail page.
    pub async fn article_detail(&self, id: i64) -> Result<Option<crate::ArticleDetail>, crate::StoreError> {
        self.db.prepare(
            "SELECT a.id, a.feed_id, f.title AS feed_name, a.guid, a.title, a.url, a.published_at, a.ai_summary, a.ai_tags, a.score
             FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id WHERE a.id = ?1",
        ).bind(&[JsValue::from_f64(id as f64)]).s_err()?.first::<crate::ArticleDetail>(None).await.s_err()
    }

    /// Get previous and next article relative to a given article id,
    /// ordered by published_at DESC.  Returns (prev, next) — both may be None.
    pub async fn adjacent_articles(
        &self,
        id: i64,
    ) -> Result<(Option<crate::Article>, Option<crate::Article>), crate::StoreError> {
        let prev = self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE published_at < (SELECT COALESCE(published_at, 0) FROM articles WHERE id = ?1) ORDER BY published_at DESC LIMIT 1"
        ).bind(&[JsValue::from_f64(id as f64)]).s_err()?.first::<crate::Article>(None).await.s_err()?;
        let next = self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE published_at > (SELECT COALESCE(published_at, 0) FROM articles WHERE id = ?1) ORDER BY published_at ASC LIMIT 1"
        ).bind(&[JsValue::from_f64(id as f64)]).s_err()?.first::<crate::Article>(None).await.s_err()?;
        Ok((prev, next))
    }

    pub async fn articles_by_tag(
        &self,
        tag: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<crate::PendingArticle>, crate::StoreError> {
        let pattern = crate::tag_like_pattern(tag);
        self.db.prepare(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE ai_tags LIKE ?1 ESCAPE '\' ORDER BY published_at DESC LIMIT ?2 OFFSET ?3",
        ).bind(&[pattern.into(), JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)]).s_err()?.all().await.s_err()?.results().s_err()
    }

    pub async fn articles_by_category(
        &self,
        category: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<crate::PendingArticle>, crate::StoreError> {
        self.db.prepare(
            "SELECT a.id, a.feed_id, a.guid, a.title, a.url, a.published_at, a.ai_summary, a.ai_tags, a.score FROM articles a JOIN feeds f ON f.id = a.feed_id WHERE f.category = ?1 ORDER BY a.published_at DESC LIMIT ?2 OFFSET ?3",
        ).bind(&[category.into(), JsValue::from_f64(limit as f64), JsValue::from_f64(offset as f64)]).s_err()?.all().await.s_err()?.results().s_err()
    }

    pub async fn categories_summary(&self) -> Result<Vec<(String, i64)>, crate::StoreError> {
        #[derive(Deserialize)]
        struct Row {
            category: String,
            article_count: i64,
        }
        let rows: Vec<Row> = self.db.prepare(
            "SELECT f.category, COUNT(a.id) AS article_count FROM feeds f LEFT JOIN articles a ON a.feed_id = f.id WHERE f.category IS NOT NULL AND f.category != '' GROUP BY f.category ORDER BY article_count DESC",
        ).all().await.s_err()?.results().s_err()?;
        Ok(rows.into_iter().map(|r| (r.category, r.article_count)).collect())
    }

    /// Find articles sharing tags with a given article, ordered by match
    /// count desc then recency.  Returns empty when source has no tags.
    pub async fn related_articles(
        &self,
        article_id: i64,
        limit: u32,
    ) -> Result<Vec<crate::PendingArticle>, crate::StoreError> {
        let src = self
            .db
            .prepare("SELECT ai_tags FROM articles WHERE id = ?1")
            .bind(&[JsValue::from_f64(article_id as f64)])
            .s_err()?;
        let tags_json = match src.first::<String>(None).await.s_err()? {
            Some(t) => t,
            None => return Ok(Vec::new()),
        };
        let tags: Vec<String> = match serde_json::from_str(&tags_json) {
            Ok(t) => t,
            Err(_) => return Ok(Vec::new()),
        };
        if tags.is_empty() {
            return Ok(Vec::new());
        }
        let conds: Vec<String> = tags
            .iter()
            .map(|t| {
                let escaped = crate::escape_like(t);
                format!("ai_tags LIKE '%\"{}%' ESCAPE '\\'", escaped.replace('\'', "''"))
            })
            .collect();
        let sql = format!(
            "SELECT id, feed_id, guid, title, url, published_at, ai_summary, ai_tags, score FROM articles WHERE id != ?1 AND ({}) ORDER BY ({} DESC), published_at DESC LIMIT ?2",
            conds.join(" OR "),
            conds.iter().map(|c| format!("CASE WHEN {} THEN 1 ELSE 0 END", c)).collect::<Vec<_>>().join(" + "),
        );
        self.db
            .prepare(&sql)
            .bind(&[JsValue::from_f64(article_id as f64), JsValue::from_f64(limit as f64)])
            .s_err()?
            .all()
            .await
            .s_err()?
            .results()
            .s_err()
    }

    /// Load recent articles that have Vectorize embeddings for ANN discovery.
    pub async fn recent_embedded_articles(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<crate::ArticleEmbeddingRef>, crate::StoreError> {
        let cutoff = now - days * 86400;
        self
            .db
            .prepare(
                "SELECT a.id AS article_id, a.vector_id, a.published_at, a.feed_id AS source_id, \
                 COALESCE((SELECT json_group_array(ae.entity_id) FROM article_entities ae WHERE ae.article_id = a.id), '[]') AS entity_ids \
                 FROM articles a \
                 WHERE a.vector_id IS NOT NULL AND a.published_at >= ?1 \
                 ORDER BY a.published_at DESC LIMIT ?2",
            )
            .bind(&[JsValue::from_f64(cutoff as f64), JsValue::from_f64(limit as f64)]).s_err()?
            .all()
            .await.s_err()?
            .results().s_err()
    }
}
