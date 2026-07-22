//! Search abstraction. `D1FtsSearch` is the only implementation for now.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;
use worker::D1Database;

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("d1 error: {0}")]
    D1(String),
}

impl From<worker::Error> for SearchError {
    fn from(e: worker::Error) -> Self {
        SearchError::D1(e.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: i64,
    pub feed_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub ai_summary: String,
    pub ai_tags: Option<String>,
    pub score: f64,
    pub rank: f64,
}

#[async_trait(?Send)]
pub trait ArticleSearch {
    async fn search(&self, query: &str, limit: u32) -> Result<Vec<SearchHit>, SearchError>;
}

pub struct D1FtsSearch<'a> {
    db: &'a D1Database,
}

impl<'a> D1FtsSearch<'a> {
    pub fn new(db: &'a D1Database) -> Self {
        Self { db }
    }

    /// FTS5 search with optional tag/category/sort/offset.
    pub async fn search_filtered(
        &self,
        query: &str,
        limit: u32,
        offset: u32,
        tag: Option<&str>,
        category: Option<&str>,
        sort: Option<&str>,
    ) -> Result<Vec<SearchHit>, SearchError> {
        let mut where_parts = vec!["articles_fts MATCH ?1".to_string()];
        let mut bind_vals: Vec<JsValue> = vec![query.into()];
        let mut idx = 2u32;

        let tag_pattern = tag.map(|t| format!("%\"{}\"%", t));
        if let Some(ref p) = tag_pattern {
            where_parts.push(format!("a.ai_tags LIKE ?{idx}"));
            bind_vals.push(p.clone().into());
            idx += 1;
        }

        if let Some(cat) = category {
            where_parts.push(format!("f.category = ?{idx}"));
            bind_vals.push(cat.into());
            idx += 1;
        }

        let where_clause = where_parts.join(" AND ");
        let order = match sort {
            Some("score") => "a.score DESC, articles_fts.rank",
            _ => "articles_fts.rank",
        };
        let ofs = idx + 1;
        let limit_idx = idx;
        idx = ofs; // update for offset bind
        let sql = format!(
            "SELECT a.id, a.feed_id, a.title, a.url, a.published_at,
                    a.ai_summary, a.ai_tags, a.score,
                    articles_fts.rank
             FROM articles_fts
             JOIN articles a ON a.id = articles_fts.rowid
             LEFT JOIN feeds f ON f.id = a.feed_id
             WHERE {where_clause}
             ORDER BY {order}
             LIMIT ?{limit_idx} OFFSET ?{ofs}"
        );
        bind_vals.push(JsValue::from_f64(limit as f64));
        bind_vals.push(JsValue::from_f64(offset as f64));

        let stmt = self.db.prepare(&sql).bind(&bind_vals)?;
        let result = stmt.all().await?;
        Ok(result.results::<SearchHit>()?)
    }
}

#[async_trait(?Send)]
impl<'a> ArticleSearch for D1FtsSearch<'a> {
    async fn search(&self, query: &str, limit: u32) -> Result<Vec<SearchHit>, SearchError> {
        let stmt = self.db.prepare(
            "SELECT a.id, a.feed_id, a.title, a.url, a.published_at,
                    a.ai_summary, a.ai_tags, a.score,
                    articles_fts.rank
             FROM articles_fts
             JOIN articles a ON a.id = articles_fts.rowid
             WHERE articles_fts MATCH ?1
             ORDER BY articles_fts.rank
             LIMIT ?2",
        );
        let stmt = stmt.bind(&[query.into(), JsValue::from_f64(limit as f64)])?;
        let result = stmt.all().await?;
        Ok(result.results::<SearchHit>()?)
    }
}
