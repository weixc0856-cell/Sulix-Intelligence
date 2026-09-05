//! Article application service — orchestrates the Article read use-cases
//! (latest / trending / batch / detail / adjacent / related / tag / category)
//! that the API routes expose under `/api/articles`.
//!
//! Generic over the narrowest store surface — [`store::ArticleQueryService`]
//! for the read model and [`store::SourceRepository`] for provenance
//! resolution.  Zero Worker / HTTP / `js_sys` code.  `search_articles` is
//! intentionally NOT here: it talks to D1 FTS directly and lives with the
//! other infrastructure-facing HTTP routes in `worker-entry`.

use store::{Article, ArticleDetail, ArticleProvenance, PendingArticle, SourceSummary, StoreError};

/// Application service for Article read use-cases.
pub struct ArticleService<S> {
    store: S,
}

impl<S> ArticleService<S>
where
    S: store::ArticleQueryService + store::SourceRepository,
{
    /// Wrap a store (or store-backed query/repository pair) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Latest articles ordered by `published_at DESC`.
    pub async fn latest(&self, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        self.store.latest_articles(limit, offset).await
    }

    /// Total article count.
    pub async fn count(&self) -> Result<i64, StoreError> {
        self.store.article_count().await
    }

    /// Articles matching a tag.
    pub async fn by_tag(&self, tag: &str, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        self.store.articles_by_tag(tag, limit, offset).await
    }

    /// Articles in a feed category.
    pub async fn by_category(
        &self,
        category: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PendingArticle>, StoreError> {
        self.store.articles_by_category(category, limit, offset).await
    }

    /// Batch-load articles by id (`/api/articles/batch`).
    pub async fn batch(&self, ids: &[i64]) -> Result<Vec<Article>, StoreError> {
        self.store.articles_by_ids(ids).await
    }

    /// Load a single article detail together with its resolved source
    /// provenance.
    ///
    /// Provenance resolution is enrichment: when the source lookup fails (or
    /// no source is linked) the article is still returned with `None`
    /// provenance (matches the historical handler behaviour, which collapsed
    /// lookup errors and misses into `None`).
    pub async fn detail(&self, id: i64) -> Result<Option<(ArticleDetail, Option<ArticleProvenance>)>, StoreError> {
        let article = match self.store.article_detail(id).await? {
            Some(a) => a,
            None => return Ok(None),
        };
        let provenance = match self.store.find_source_by_feed(article.feed_id).await {
            Ok(Some(source)) => {
                let summary: SourceSummary = source.into();
                Some(ArticleProvenance { attribution: summary.attribution.clone(), source: Some(summary) })
            }
            _ => None,
        };
        Ok(Some((article, provenance)))
    }

    /// Previous and next article relative to `id` (by `published_at DESC`).
    pub async fn adjacent(&self, id: i64) -> Result<(Option<Article>, Option<Article>), StoreError> {
        self.store.adjacent_articles(id).await
    }

    /// Articles sharing tags with `id`, ranked by overlap.
    pub async fn related(&self, id: i64, limit: u32) -> Result<Vec<PendingArticle>, StoreError> {
        self.store.related_articles(id, limit).await
    }

    /// Trending (scored) articles ordered by score DESC.
    pub async fn trending(&self, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        self.store.trending_articles(limit, offset).await
    }

    /// Count of trending (scored) articles.
    pub async fn trending_count(&self) -> Result<i64, StoreError> {
        self.store.trending_count().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    // MemoryStore's article read model is a stub (article queries return empty
    // / `None`, `article_by_id` errors), so these tests pin the service
    // contract for the paths the stub can express — missing rows surface as
    // `Ok(None)` / empty lists, never as an error.  No MemoryStore behaviour
    // is expanded here.

    #[test]
    fn detail_missing_returns_none() {
        let svc = ArticleService::new(MemoryStore::new());
        assert!(futures::executor::block_on(svc.detail(1)).expect("detail should succeed").is_none());
    }

    #[test]
    fn trending_is_empty_from_stub_backend() {
        let svc = ArticleService::new(MemoryStore::new());
        let rows = futures::executor::block_on(svc.trending(50, 0)).expect("trending should succeed");
        assert!(rows.is_empty());
        assert_eq!(futures::executor::block_on(svc.trending_count()).expect("count should succeed"), 0);
    }

    #[test]
    fn adjacent_returns_none_pairs_from_stub_backend() {
        let svc = ArticleService::new(MemoryStore::new());
        let (prev, next) = futures::executor::block_on(svc.adjacent(1)).expect("adjacent should succeed");
        assert!(prev.is_none());
        assert!(next.is_none());
    }
}
