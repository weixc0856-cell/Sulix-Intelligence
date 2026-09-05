//! System/aggregation application service.
//!
//! Backs the `/api/system/*` read endpoints: health, dashboard, stats,
//! categories, tags, pipeline status, debug-feed listing and the legacy
//! intelligence-signals feed.  Every method here is a pure D1 read (or a
//! two-read composition); KV cache-aside / KV-enriched presentation stays in
//! the route layer.  `now` is supplied by the caller for every time-window
//! query — this service never reads the runtime clock.
//!
//! Zero Worker / HTTP / `js_sys` code; unit-testable with `MemoryStore`.

use store::{DayCount, Feed, FeedStats, HealthStats, ScoreDist, StoreError, TodaySignal};

/// Application service for the system/aggregation read use-cases.
pub struct SystemService<S> {
    store: S,
}

impl<S> SystemService<S>
where
    S: store::FeedQueryService + store::ArticleQueryService + store::SignalQueryService,
{
    /// Wrap a store (or store-backed query-service set) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Feeds due for fetch at `now` (no category filter) — `/api/system/debug`.
    pub async fn feeds_due(&self, now: i64) -> Result<Vec<Feed>, StoreError> {
        self.store.feeds_due_for_fetch(now, None).await
    }

    /// Aggregate health indicators — `/api/system/health`.
    pub async fn health_stats(&self) -> Result<HealthStats, StoreError> {
        self.store.health_stats().await
    }

    /// Health + per-feed stats for the dashboard — `/api/system/dashboard`.
    /// Short-circuits: any failing read fails the dashboard as a whole.
    pub async fn dashboard(&self) -> Result<(HealthStats, Vec<FeedStats>), StoreError> {
        Ok((self.store.health_stats().await?, self.store.feed_stats().await?))
    }

    /// Score distribution + 14-day article trend — `/api/system/stats`.
    pub async fn score_stats(&self) -> Result<(ScoreDist, Vec<DayCount>), StoreError> {
        Ok((self.store.score_distribution().await?, self.store.article_trend(14).await?))
    }

    /// Pipeline status read model — `/api/pipeline/status`.  KV pipeline-timing
    /// metrics are enriched by the route, not here.
    pub async fn pipeline_status(&self, now: i64) -> Result<serde_json::Value, StoreError> {
        self.store.pipeline_status(now).await
    }

    /// Category → article-count mapping — `/api/system/categories`.
    pub async fn categories(&self) -> Result<Vec<(String, i64)>, StoreError> {
        self.store.categories_summary().await
    }

    /// Tag → count mapping — `/api/system/tags`.
    pub async fn tags(&self) -> Result<Vec<(String, i64)>, StoreError> {
        self.store.tags_summary().await
    }

    /// Today's intelligence signals (entity-anchored, V1 format) at `now` —
    /// `/api/system/signals`.
    pub async fn signals_today(&self, now: i64) -> Result<Vec<TodaySignal>, StoreError> {
        self.store.signals_today(now).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    // MemoryStore's read models are stubs (health_stats / score_distribution
    // return `Err("not implemented")`; the list queries return empty), so these
    // tests pin the service contract for the paths the stub can express — empty
    // list reads surface as `Ok`/empty, never as an error.  No MemoryStore
    // behaviour is expanded here.

    #[test]
    fn feeds_due_is_empty_from_stub_backend() {
        let svc = SystemService::new(MemoryStore::new());
        let feeds = futures::executor::block_on(svc.feeds_due(1_000_000)).expect("feeds_due should succeed");
        assert!(feeds.is_empty());
    }

    #[test]
    fn categories_and_tags_are_empty_from_stub_backend() {
        let svc = SystemService::new(MemoryStore::new());
        assert!(futures::executor::block_on(svc.categories()).expect("categories should succeed").is_empty());
        assert!(futures::executor::block_on(svc.tags()).expect("tags should succeed").is_empty());
    }

    #[test]
    fn pipeline_status_is_ok_json_from_stub_backend() {
        let svc = SystemService::new(MemoryStore::new());
        let status =
            futures::executor::block_on(svc.pipeline_status(1_000_000)).expect("pipeline_status should succeed");
        assert!(status.is_object());
    }

    #[test]
    fn signals_today_is_empty_from_stub_backend() {
        let svc = SystemService::new(MemoryStore::new());
        let signals = futures::executor::block_on(svc.signals_today(1_000_000)).expect("signals_today should succeed");
        assert!(signals.is_empty());
    }
}
