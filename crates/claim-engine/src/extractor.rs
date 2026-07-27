//! ClaimExtractor trait — abstraction over LLM-based claim extraction.

use async_trait::async_trait;

use crate::domain::ClaimCandidate;

/// Extracts atomic, falsifiable claims from article text.
#[async_trait(?Send)]
pub trait ClaimExtractor {
    /// Extract claims from an article given its title and body.
    ///
    /// `frameworks_context` is an optional string listing applicable reasoning
    /// frameworks to apply during analysis.
    async fn extract(&self, title: &str, body: &str, article_id: i64, frameworks_context: Option<&str>) -> Result<Vec<ClaimCandidate>, String>;
}
