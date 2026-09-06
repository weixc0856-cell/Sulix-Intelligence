use async_trait::async_trait;

use crate::StoreError;

/// Article analysis/content lifecycle persistence (derived data on `articles`).
///
/// Infra adapters (e.g. the ai-pipeline persistence seam) bind this narrow
/// seam directly.  Row *insertion* stays on [`ArticleRepository`](crate::ArticleRepository);
/// this seam covers the post-ingestion writes: AI summary results and the R2
/// key of the extracted full-text body.
#[async_trait(?Send)]
pub trait ArticleAnalysisStore {
    /// Persist AI summarisation results (summary, tags, embedding vector id, score).
    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), StoreError>;

    /// Update the R2 key pointing to the article's full-text body.
    async fn set_raw_content_r2_key(&self, article_id: i64, r2_key: Option<&str>) -> Result<(), StoreError>;
}
