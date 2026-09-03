//! Vectorize-backed [`SemanticQuery`] adapter — bridges the signal-engine
//! semantic-discovery port onto the Cloudflare Vectorize binding.
//!
//! Lives in infrastructure so signal-engine never depends on the `#[wasm_bindgen]`
//! extern type directly (decoupling P3 Round 2).

use async_trait::async_trait;
use signal_engine::error::SignalError;
use signal_engine::ports::{SemanticQuery, SimilarArticle};
use vectorize::VectorizeIndex;

/// Adapts a [`VectorizeIndex`] binding to the domain [`SemanticQuery`].
pub struct VectorizeSemanticQuery {
    inner: VectorizeIndex,
}

impl VectorizeSemanticQuery {
    pub fn new(inner: VectorizeIndex) -> Self {
        Self { inner }
    }
}

#[async_trait(?Send)]
impl SemanticQuery for VectorizeSemanticQuery {
    async fn find_similar(
        &self,
        vector_id: &str,
        top_k: u32,
        min_score: f64,
    ) -> Result<Vec<SimilarArticle>, SignalError> {
        let matches = vectorize::query_similar_by_id(&self.inner, vector_id, top_k, min_score)
            .await
            .map_err(|e| SignalError::Semantic(e.to_string()))?;
        // Map only the raw stored-vector id + score. Parsing the id further
        // (e.g. to an article id) is the domain consumer's concern.
        Ok(matches.into_iter().map(|m| SimilarArticle { vector_id: m.id, score: m.score }).collect())
    }
}
