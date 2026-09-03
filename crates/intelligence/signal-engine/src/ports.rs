//! Dependency-boundary ports for signal-engine (decoupling P3 Round 2).
//!
//! These are the edges through which the engine reaches the outside world:
//! signal persistence, candidate discovery, read-model query, the event log,
//! and semantic ANN retrieval.
//!
//! ⚠️ Temporary, use-case-specific boundary — NOT a public shared-kernel /
//! intelligence-domain capability. signal-engine is a deprecated crate
//! (`lib.rs` banner; removal scheduled Sprint 6.2E+). When the crate is
//! deleted, these ports and their infrastructure adapters are removed together.

use async_trait::async_trait;

use crate::error::SignalError;

/// A match returned by semantic ANN search.
///
/// `vector_id` is the raw stored-vector identifier returned by the index
/// (namespaced, e.g. `article-42`). Interpreting it further — e.g. recovering
/// an article id — is the domain consumer's concern, not the adapter's, so the
/// infrastructure field name (`id`) never leaks into the domain port.
#[derive(Debug, Clone)]
pub struct SimilarArticle {
    pub vector_id: String,
    pub score: f64,
}

/// Semantic ANN retrieval boundary — used by [`crate::source::SemanticDiscoverySource`].
#[async_trait(?Send)]
pub trait SemanticQuery {
    /// Find the nearest stored vectors to `vector_id` in the semantic index.
    async fn find_similar(
        &self,
        vector_id: &str,
        top_k: u32,
        min_score: f64,
    ) -> Result<Vec<SimilarArticle>, SignalError>;
}
