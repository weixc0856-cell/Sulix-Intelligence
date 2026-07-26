//! ClaimExtractor trait — abstraction over LLM-based claim extraction.

use async_trait::async_trait;

use crate::domain::ClaimCandidate;

/// Extracts atomic, falsifiable claims from article text.
#[async_trait(?Send)]
pub trait ClaimExtractor {
    /// Extract claims from an article given its title and body.
    async fn extract(&self, title: &str, body: &str, article_id: i64) -> Result<Vec<ClaimCandidate>, String>;
}
