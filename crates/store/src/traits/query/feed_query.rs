//! Read-model queries for the Feed domain.
//!
//! Feed lifecycle mutations (record_fetch_result, set_feed_status) and
//! rule CRUD remain on [`StoreBackend`](crate::StoreBackend) until they are
//! promoted to their own domain services.

use async_trait::async_trait;

use crate::{DayCount, Feed, FeedStats, HealthStats, ScoreDist, StoreError};

#[async_trait(?Send)]
pub trait FeedQueryService {
    /// Feeds whose `last_fetched_at + fetch_interval_sec` has elapsed.
    /// Optional category filter.
    async fn feeds_due_for_fetch(&self, now: i64, category: Option<&str>) -> Result<Vec<Feed>, StoreError>;

    /// List all feeds, optionally filtered by status.
    async fn all_feeds(&self, status_filter: Option<&str>) -> Result<Vec<Feed>, StoreError>;

    /// Per-feed article counts (for feed management UI).
    async fn feed_stats(&self) -> Result<Vec<FeedStats>, StoreError>;

    /// Aggregate health indicators (feed/article counts, last cron run).
    async fn health_stats(&self) -> Result<HealthStats, StoreError>;

    /// Pipeline status as raw JSON (used by `/api/pipeline/status`).
    async fn pipeline_status(&self, now: i64) -> Result<serde_json::Value, StoreError>;

    /// Score distribution across all articles.
    async fn score_distribution(&self) -> Result<ScoreDist, StoreError>;

    /// Article publication counts per day for the last N days.
    async fn article_trend(&self, days: i64) -> Result<Vec<DayCount>, StoreError>;
}
