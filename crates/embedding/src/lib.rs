//! Embedding generation abstraction.
//!
//! Provides a trait-based interface for generating text embeddings,
//! independent of the specific model or provider.  The current
//! implementation uses Cloudflare Workers AI (bge-large-en-v1.5)
//! which produces 1024-dimensional vectors.

use async_trait::async_trait;
use serde::Serialize;
use worker::*;

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("embedding request failed: {0}")]
    Request(String),
    #[error("unexpected response: {0}")]
    Response(String),
}

/// Generates vector embeddings from text.
#[async_trait(?Send)]
pub trait EmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}

/// Embedder using Cloudflare Workers AI.
/// Model: @cf/baai/bge-large-en-v1.5 (1024 dimensions, free tier).
pub struct WorkersAiEmbedder {
    env: Env,
}

impl WorkersAiEmbedder {
    pub fn new(env: &Env) -> Self {
        Self { env: env.clone() }
    }
}

#[derive(Serialize)]
struct BgeInput {
    text: Vec<String>,
}

#[async_trait(?Send)]
impl EmbeddingProvider for WorkersAiEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let ai = self.env
            .ai("AI")
            .map_err(|e| EmbeddingError::Request(e.to_string()))?;

        // Workers AI returns { data: [[f32; 1024]], shape: [1, 1024] }
        // data[0] is the embedding vector directly (not wrapped in an object)
        let result: serde_json::Value = ai
            .run(
                "@cf/baai/bge-large-en-v1.5",
                BgeInput { text: vec![text.to_string()] },
            )
            .await
            .map_err(|e| EmbeddingError::Request(e.to_string()))?;

        let embedding = result["data"][0]
            .as_array()
            .ok_or_else(|| EmbeddingError::Response("missing data[0]".into()))?;

        let vec: Vec<f32> = embedding
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();

        if vec.is_empty() {
            return Err(EmbeddingError::Response("empty embedding vector".into()));
        }
        Ok(vec)
    }
}

/// Build the structured embedding input text from article fields.
pub fn build_embedding_text(
    title: &str,
    summary: &str,
    tags: &[String],
    feed_name: Option<&str>,
) -> String {
    let tags_str = if tags.is_empty() {
        String::new()
    } else {
        format!("\nTopics:\n{}", tags.join(", "))
    };
    let source = feed_name
        .map(|n| format!("\nSource:\n{}", n))
        .unwrap_or_default();
    format!("Title:\n{title}\n\nSummary:\n{summary}{tags_str}{source}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_embedding_text_constructs_correctly() {
        let text = build_embedding_text(
            "AI News",
            "Latest AI developments",
            &["AI".into(), "tech".into()],
            Some("TechCrunch"),
        );
        assert!(text.contains("Title:\nAI News"));
        assert!(text.contains("Topics:\nAI, tech"));
        assert!(text.contains("Source:\nTechCrunch"));
    }

    #[test]
    fn build_embedding_text_handles_no_tags() {
        let text = build_embedding_text("Hello", "World", &[], None);
        assert!(!text.contains("Topics"));
    }
}
