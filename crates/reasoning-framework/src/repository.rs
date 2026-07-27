//! Reasoning Framework — domain-owned repository trait

use async_trait::async_trait;

use crate::error::FrameworkError;
use crate::framework::{FrameworkCategory, ReasoningFramework};

/// Repository for ReasoningFramework persistence.
///
/// Defined here (not in `store`) so domain depends on nothing
/// infrastructure-specific. D1 implementation lives in infrastructure crate.
#[async_trait(?Send)]
pub trait FrameworkRepository {
    /// Find a framework by its ID.
    async fn find(&self, id: &str) -> Result<Option<ReasoningFramework>, FrameworkError>;

    /// List all frameworks in a category.
    async fn list_by_category(&self, category: FrameworkCategory) -> Result<Vec<ReasoningFramework>, FrameworkError>;

    /// List all frameworks.
    async fn list_all(&self) -> Result<Vec<ReasoningFramework>, FrameworkError>;

    /// Search frameworks by keyword in name/description.
    async fn search(&self, query: &str) -> Result<Vec<ReasoningFramework>, FrameworkError>;

    /// Seed initial frameworks (idempotent — INSERT OR IGNORE).
    async fn seed(&self, frameworks: &[ReasoningFramework]) -> Result<(), FrameworkError>;

    /// Update calibration data after an outcome is recorded.
    async fn update_calibration(
        &self,
        framework_id: &str,
        calibration_score: f64,
        usage_count: u64,
        delta_avg: f64,
    ) -> Result<(), FrameworkError>;
}
