//! Search abstraction. `D1FtsSearch` is the only implementation for now
//! (basic keyword search over articles_fts). If keyword+BM25 ever stops
//! being enough, add an `ExternalSearch` implementation of the same trait
//! and swap it in at the `api` crate's composition root -- nothing else
//! in the codebase needs to change.

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
    pub article_id: i64,
    pub title: String,
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
}

#[async_trait(?Send)]
impl<'a> ArticleSearch for D1FtsSearch<'a> {
    async fn search(&self, query: &str, limit: u32) -> Result<Vec<SearchHit>, SearchError> {
        // articles_fts.rowid lines up with articles.id via the
        // content_rowid='id' declaration in the migration.
        let stmt = self.db.prepare(
            "SELECT a.id AS article_id, a.title AS title, articles_fts.rank AS rank
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
