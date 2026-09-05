//! D1-backed ArticlePersistence — maps the ai-pipeline summary-persistence
//! seam to the D1 `articles` table.
//!
//! Unlike the owning adapters (e.g. `D1ReflectionRepository`), this adapter
//! borrows the store: the ingestion pipeline shares one D1Store across the
//! whole feed batch, so the adapter is built per call site from `&S`.

use ai_pipeline::{ArticlePersistence, PipelineError};
use async_trait::async_trait;
use store::ArticleAnalysisStore;

/// Maps `set_ai_summary` to the D1 articles table.
pub struct D1ArticlePersistence<'a, S: ArticleAnalysisStore> {
    store: &'a S,
}

impl<'a, S: ArticleAnalysisStore> D1ArticlePersistence<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }
}

#[async_trait(?Send)]
impl<S: ArticleAnalysisStore> ArticlePersistence for D1ArticlePersistence<'_, S> {
    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), PipelineError> {
        self.store
            .set_ai_summary(article_id, summary, tags_json, vector_id, score)
            .await
            .map_err(|e| PipelineError::Persistence(e.to_string()))
    }
}
