use async_trait::async_trait;

use crate::{Article, NewArticle, StoreError};

/// Article (Content Identity) persistence.
///
/// The `Article` type represents content identity (title, url, published_at).
/// AI-generated analysis (summary, tags, entities, embedding) and the raw
/// content R2 key are managed through the `ArticleAnalysisStore` seam
/// (`set_ai_summary`, `set_raw_content_r2_key`) until they are promoted to
/// their own domain aggregate (`ArticleAnalysis`, `SemanticIndexRecord`).
#[async_trait(?Send)]
pub trait ArticleRepository {
    /// Insert a new article (INSERT OR IGNORE on GUID).  Returns the row id,
    /// or `None` when a duplicate GUID was detected.
    async fn save_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError>;

    /// Load an article by its primary key.
    async fn find_article(&self, id: i64) -> Result<Option<Article>, StoreError>;
}
