//! Summarize + tag + embed a single article, then persist the result
//! through `store`. The actual model call is behind `Summarizer` so it can
//! point at Workers AI, or an external LLM API (whichever you decide has
//! better quality/cost for summarization) without touching this crate's
//! callers -- only the concrete impl passed in at the composition root
//! changes.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::{Store, StoreError};

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("summarizer error: {0}")]
    Summarizer(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SummaryResult {
    pub summary: String,
    pub tags: Vec<String>,
    /// Embedding vector to upsert into Vectorize; kept generic here so this
    /// crate doesn't hard-depend on the Vectorize binding shape.
    pub embedding: Vec<f32>,
}

#[async_trait(?Send)]
pub trait Summarizer {
    async fn summarize(&self, title: &str, body: &str) -> Result<SummaryResult, PipelineError>;
}

/// Runs summarization for one article and writes the result back through
/// `store`. `score` is the rules-engine output computed upstream (see the
/// `rules` crate) and passed in here rather than recomputed.
pub async fn process_article(
    store: &Store,
    summarizer: &dyn Summarizer,
    article_id: i64,
    title: &str,
    body: &str,
    score: f64,
) -> Result<(), PipelineError> {
    let result = summarizer.summarize(title, body).await?;
    let tags_json = serde_json::to_string(&result.tags).unwrap_or_else(|_| "[]".to_string());

    // vector_id convention: one embedding per article, keyed by article id.
    let vector_id = format!("article-{article_id}");

    store
        .set_ai_summary(article_id, &result.summary, &tags_json, &vector_id, score)
        .await?;

    // Upserting `result.embedding` into Vectorize under `vector_id` happens
    // at the worker-entry composition root, where the Vectorize binding is
    // actually available -- kept out of this crate to avoid coupling
    // ai-pipeline directly to a specific Cloudflare binding type.
    Ok(())
}
