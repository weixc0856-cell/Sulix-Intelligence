//! Strategy-preview application service.
//!
//! Evaluates a proposed signal strategy against recent articles and returns
//! matched results so users can see impact before saving (`/api/strategies/preview`).
//!
//! The scoring primitive itself stays in the `rules` crate, which the API
//! layer still owns: this service fetches the candidate articles through
//! [`store::ArticleQueryService`] and applies a caller-supplied `score`
//! closure.  That keeps the `application → rules` edge from appearing — the
//! rule parsing / `rules::score` invocation lives in the route handler.
//!
//! Zero Worker / HTTP / `js_sys` code.  The preview path touches D1 only.

use store::{ArticleDetail, PreviewMatch, PreviewResult, StoreError};

/// Application service for the signal-strategy preview use-case.
pub struct StrategyPreviewService<S> {
    store: S,
}

impl<S> StrategyPreviewService<S>
where
    S: store::ArticleQueryService,
{
    /// Wrap a store (or store-backed query service) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Run a preview of a proposed strategy against the `fetch_limit` most
    /// recent articles.
    ///
    /// `score` returns the score delta a given article would receive (the
    /// result of `rules::score` for the proposed temporary rule); articles
    /// whose delta is non-zero are reported as matched.  `matched_reason` is
    /// the human-readable condition description attached to every match, and
    /// `signal_type` is echoed through unchanged on the result.
    pub async fn preview(
        &self,
        fetch_limit: u32,
        signal_type: Option<String>,
        matched_reason: String,
        score: impl Fn(&ArticleDetail) -> f64,
    ) -> Result<PreviewResult, StoreError> {
        let articles = self.store.recent_articles_for_preview(fetch_limit).await?;

        let total = articles.len() as i64;
        let mut matched: i64 = 0;
        let mut items: Vec<PreviewMatch> = Vec::new();

        for article in &articles {
            let change = score(article);
            if change != 0.0 {
                matched += 1;
                items.push(PreviewMatch {
                    id: article.id,
                    title: article.title.clone(),
                    url: article.url.clone(),
                    published_at: article.published_at,
                    feed_name: article.feed_name.clone(),
                    score_change: change,
                    matched_reason: matched_reason.clone(),
                });
            }
        }

        Ok(PreviewResult { total, matched, signal_type, items })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    // MemoryStore's article read model is a stub (`recent_articles_for_preview`
    // returns an empty list), so these tests pin the service contract for the
    // paths the stub can express — an empty candidate set yields an empty
    // result, never an error.  No MemoryStore behaviour is expanded here.

    #[test]
    fn preview_over_empty_candidates_is_ok_and_empty() {
        let svc = StrategyPreviewService::new(MemoryStore::new());
        let result =
            futures::executor::block_on(svc.preview(100, Some("alert".into()), "title contains X".into(), |_| 3.0))
                .expect("preview should succeed over empty candidates");
        assert_eq!(result.total, 0);
        assert_eq!(result.matched, 0);
        assert!(result.items.is_empty());
        assert_eq!(result.signal_type.as_deref(), Some("alert"));
    }

    #[test]
    fn preview_signal_type_is_echoed() {
        let svc = StrategyPreviewService::new(MemoryStore::new());
        let result = futures::executor::block_on(svc.preview(50, None, "reason".into(), |_| 0.0))
            .expect("preview should succeed");
        assert!(result.signal_type.is_none());
    }
}
