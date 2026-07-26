//! Semantic Search UseCase — 5-step pipeline:
//! 1. Generate query embedding via Workers AI
//! 2. Query Vectorize ANN index
//! 3. Parse matches to extract article IDs
//! 4. Fetch articles from D1 by IDs
//! 5. Enrich with similarity scores and sort
//!
//! This UseCase extracts business logic from the HTTP handler
//! so it can be tested without a running Worker.

/// Command for semantic search.
pub struct SemanticSearchCmd {
    pub query: String,
    pub limit: Option<u32>,
}

/// One search hit.
pub struct SemanticSearchHit {
    pub article: store::Article,
    pub similarity: f64,
}

/// Result of a semantic search.
pub struct SemanticSearchResult {
    pub hits: Vec<SemanticSearchHit>,
}

impl SemanticSearchResult {
    pub fn empty() -> Self {
        Self { hits: Vec::new() }
    }
}

/// Service that orchestrates the semantic search pipeline.
///
/// Generic over `S: StoreBackend` so it can be unit-tested with
/// `MemoryStore` and used in production with `D1Store`.
pub struct SemanticSearchService<S> {
    #[allow(dead_code)]
    store: S,
}

impl<S> SemanticSearchService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> SemanticSearchService<S>
where
    S: store::ArticleQueryService,
{
    /// Execute a semantic search: embed query → ANN → D1 enrich.
    ///
    /// NOTE: This implementation is a blueprint.  In production the embedding
    /// and Vectorize calls bypass D1 and go through Workers AI / Vectorize
    /// bindings.  Those steps will be wired in when the service is constructed
    /// from Worker env bindings.  For now the method documents the full flow.
    pub async fn execute(&self, _cmd: SemanticSearchCmd) -> Result<SemanticSearchResult, String> {
        // Steps:
        // 1. embedder.embed(cmd.query) → Vec<f32>
        // 2. vectorize.query(vector, top_k, min_score) → Vec<SimilarMatch>
        // 3. Parse IDs from matches, fetch from D1
        // 4. Enrich with similarity → return

        // For now, return empty — the full implementation requires the
        // embedding and vectorize infrastructure which is injected at
        // the Worker composition root.
        Ok(SemanticSearchResult::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;

    #[test]
    fn test_semantic_search_returns_empty_for_no_query() {
        let store = MemoryStore::new();
        let service = SemanticSearchService::new(store);
        let result =
            futures::executor::block_on(service.execute(SemanticSearchCmd { query: String::new(), limit: Some(5) }))
                .unwrap();
        assert!(result.hits.is_empty());
    }
}
