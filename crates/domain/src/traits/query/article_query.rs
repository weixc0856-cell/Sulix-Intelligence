//! Read-model queries for the Article domain.
//!
//! Article mutations (insert, `set_ai_summary`, `set_raw_content_r2_key`)
//! and AI-analysis methods remain on [`StoreBackend`](crate::StoreBackend)
//! until `ArticleAnalysis` and `SemanticIndexRecord` are promoted to their own
//! aggregate roots.

use async_trait::async_trait;

use crate::{Article, ArticleDetail, ArticleEmbeddingRef, PendingArticle, StoreError};

#[async_trait(?Send)]
pub trait ArticleQueryService {
    // ── Latest / Trending ──

    /// Latest articles ordered by `published_at DESC`.
    async fn latest_articles(&self, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError>;

    /// Total article count.
    async fn article_count(&self) -> Result<i64, StoreError>;

    /// Trending articles (score != 0) ordered by score DESC.
    async fn trending_articles(&self, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError>;

    /// Count of trending (scored) articles.
    async fn trending_count(&self) -> Result<i64, StoreError>;

    // ── Single / Batch ──

    /// Load a single article by primary key.
    async fn article_by_id(&self, id: i64) -> Result<Option<Article>, StoreError>;

    /// Batch-load articles by ID (used by bookmarks / `/api/articles/batch`).
    async fn articles_by_ids(&self, ids: &[i64]) -> Result<Vec<Article>, StoreError>;

    /// Article with feed name joined in (detail page).
    async fn article_detail(&self, id: i64) -> Result<Option<ArticleDetail>, StoreError>;

    /// Previous and next article relative to `id`, ordered by `published_at DESC`.
    async fn adjacent_articles(&self, id: i64) -> Result<(Option<Article>, Option<Article>), StoreError>;

    // ── Tag / Category filtering ──

    /// Articles matching a given tag.
    async fn articles_by_tag(&self, tag: &str, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError>;

    /// Articles in a given feed category.
    async fn articles_by_category(
        &self,
        category: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PendingArticle>, StoreError>;

    /// Articles sharing tags with a given article, ranked by overlap.
    async fn related_articles(&self, article_id: i64, limit: u32) -> Result<Vec<PendingArticle>, StoreError>;

    // ── Aggregations ──

    /// Recent articles (feed name + AI summary joined) ordered by
    /// `published_at DESC`, for the strategy-preview scoring endpoint.
    async fn recent_articles_for_preview(&self, limit: u32) -> Result<Vec<ArticleDetail>, StoreError>;

    /// Category → article_count mapping, ordered by count DESC.
    async fn categories_summary(&self) -> Result<Vec<(String, i64)>, StoreError>;

    /// Tag → article_count mapping, ordered alphabetically.
    async fn tags_summary(&self) -> Result<Vec<(String, i64)>, StoreError>;

    /// Get the R2 key pointing to the raw HTML body for an article.
    async fn get_raw_content_key(&self, article_id: i64) -> Result<Option<String>, StoreError>;

    // ── Embedding / ANN ──

    /// Load recent articles with Vectorize embeddings (for ANN signal discovery).
    async fn recent_embedded_articles(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<ArticleEmbeddingRef>, StoreError>;
}
