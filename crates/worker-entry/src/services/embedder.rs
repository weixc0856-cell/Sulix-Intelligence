use ai_pipeline::{Embedder, PipelineError};
use async_trait::async_trait;
use embedding::EmbeddingProvider;
use worker::*;

/// Composition-root adapter: `embedding::WorkersAiEmbedder` presented as
/// `ai_pipeline::Embedder`.
///
/// worker-entry is the only crate allowed to depend on both `ai-pipeline` and
/// `embedding`, so the job layer (jobs/*) sees only the ai-pipeline contract
/// and stays ignorant of Workers AI / `env.ai` specifics.
pub struct AiEmbedder {
    inner: embedding::WorkersAiEmbedder,
}

#[async_trait(?Send)]
impl Embedder for AiEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, PipelineError> {
        self.inner.embed(text).await.map_err(|e| PipelineError::Summarizer(format!("embedding: {e}")))
    }
}

/// Build the Workers AI embedder if the `AI` binding is present.
/// Returns `None` (embedding disabled) when the binding is missing — the
/// ingestion jobs log that state rather than failing.
pub fn try_build_embedder(env: &Env) -> Option<AiEmbedder> {
    if env.ai("AI").is_err() {
        console_log!("AI binding not available — article embeddings disabled");
        return None;
    }
    Some(AiEmbedder { inner: embedding::WorkersAiEmbedder::new(env) })
}
