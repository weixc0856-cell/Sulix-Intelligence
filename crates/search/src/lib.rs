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

/// Pure query-parameter struct for building FTS5 search WHERE clauses.
pub struct SearchFilters<'a> {
    pub query: &'a str,
    pub tag: Option<&'a str>,
    pub category: Option<&'a str>,
}

/// Build the WHERE clause and bind-value list for a filtered FTS5 search.
/// Returns `(where_sql, bind_values, next_param_index)`.
pub(crate) fn build_search_where(filters: &SearchFilters) -> (String, Vec<String>, u32) {
    let mut parts = vec!["articles_fts MATCH ?1".to_string()];
    let mut binds = vec![filters.query.to_string()];
    let mut idx = 2u32;

    if let Some(tag) = filters.tag {
        let pattern = format!("%\"{}\"%", tag);
        parts.push(format!("a.ai_tags LIKE ?{idx}"));
        binds.push(pattern);
        idx += 1;
    }
    if let Some(cat) = filters.category {
        parts.push(format!("f.category = ?{idx}"));
        binds.push(cat.to_string());
        idx += 1;
    }

    (parts.join(" AND "), binds, idx)
}

/// Select the ORDER BY clause for search results.
pub(crate) fn search_order_clause(sort: Option<&str>) -> &'static str {
    match sort {
        Some("score") => "a.score DESC, articles_fts.rank",
        _ => "articles_fts.rank",
    }
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
        let filters = SearchFilters { query, tag, category };
        let (where_clause, str_binds, idx) = build_search_where(&filters);
        let mut bind_vals: Vec<JsValue> = str_binds.into_iter().map(|s| s.into()).collect();

        let order = search_order_clause(sort);
        let ofs = idx + 1;
        let limit_idx = idx;
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

    /// Count total matching results for a query (same filters as search_filtered,
    /// without pagination). Used to return `total` in API responses.
    pub async fn search_count(
        &self,
        query: &str,
        tag: Option<&str>,
        category: Option<&str>,
    ) -> Result<i64, SearchError> {
        let filters = SearchFilters { query, tag, category };
        let (where_clause, str_binds, _idx) = build_search_where(&filters);
        let bind_vals: Vec<JsValue> = str_binds.into_iter().map(|s| s.into()).collect();
        let sql = format!(
            "SELECT COUNT(*) AS cnt
             FROM articles_fts
             JOIN articles a ON a.id = articles_fts.rowid
             LEFT JOIN feeds f ON f.id = a.feed_id
             WHERE {where_clause}"
        );
        let stmt = self.db.prepare(&sql).bind(&bind_vals)?;
        let row = stmt.first::<serde_json::Value>(None).await?;
        Ok(row.and_then(|v| v["cnt"].as_i64()).unwrap_or(0))
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

#[cfg(test)]
mod tests {
    use super::*;

    // -- build_search_where --

    #[test]
    fn where_query_only() {
        let f = SearchFilters { query: "AI", tag: None, category: None };
        let (sql, binds, idx) = build_search_where(&f);
        assert_eq!(sql, "articles_fts MATCH ?1");
        assert_eq!(binds, vec!["AI"]);
        assert_eq!(idx, 2);
    }

    #[test]
    fn where_query_and_tag() {
        let f = SearchFilters { query: "rust", tag: Some("programming"), category: None };
        let (sql, binds, _) = build_search_where(&f);
        assert!(sql.contains("a.ai_tags LIKE ?2"));
        assert_eq!(binds.len(), 2);
        assert_eq!(binds[1], r#"%"programming"%"#);
    }

    #[test]
    fn where_query_and_category() {
        let f = SearchFilters { query: "AI", tag: None, category: Some("tech") };
        let (sql, binds, _) = build_search_where(&f);
        assert!(sql.contains("f.category = ?2"));
        assert_eq!(binds.len(), 2);
        assert_eq!(binds[1], "tech");
    }

    #[test]
    fn where_all_filters() {
        let f = SearchFilters { query: "AI", tag: Some("safety"), category: Some("tech") };
        let (sql, binds, idx) = build_search_where(&f);
        assert!(sql.contains("articles_fts MATCH ?1"));
        assert!(sql.contains("a.ai_tags LIKE ?2"));
        assert!(sql.contains("f.category = ?3"));
        assert_eq!(binds.len(), 3);
        assert_eq!(idx, 4);
    }

    #[test]
    fn where_empty_tag() {
        let f = SearchFilters { query: "AI", tag: Some(""), category: None };
        let (sql, binds, _) = build_search_where(&f);
        assert!(sql.contains("LIKE ?2"));
        assert_eq!(binds[1], r#"%""%"#);
    }

    #[test]
    fn where_empty_category() {
        let f = SearchFilters { query: "AI", tag: None, category: Some("") };
        let (sql, binds, _) = build_search_where(&f);
        assert!(sql.contains("f.category = ?2"));
        assert_eq!(binds[1], "");
    }

    // -- search_order_clause --

    #[test]
    fn order_default_rank() {
        assert_eq!(search_order_clause(None), "articles_fts.rank");
        assert_eq!(search_order_clause(Some("date")), "articles_fts.rank");
    }

    #[test]
    fn order_by_score() {
        assert_eq!(
            search_order_clause(Some("score")),
            "a.score DESC, articles_fts.rank"
        );
    }
}
