//! Summarize + tag an article and persist the result through `store`. The
//! actual model call is behind [`Summarizer`] so it can point at Workers AI,
//! or an external LLM API (whichever you decide has better quality/cost for
//! summarization) without touching this crate's callers -- only the concrete
//! impl passed in at the composition root changes.
//!
//! Embedding is intentionally NOT part of this crate: [`SummaryResult`] carries
//! only summary/tags/entities, and the outbound [`Embedder`] seam is consumed
//! by the composition root *after* `process_article` has persisted the summary
//! (see [`Embedder`] docs). Chat transport is provided by `model_runtime`
//! (`ModelProvider`), which the composition root configures.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod briefing;
pub mod retry;
pub mod tag_normalizer;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("summarizer error: {0}")]
    Summarizer(String),
    #[error("persistence error: {0}")]
    Persistence(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SummaryResult {
    pub summary: String,
    pub tags: Vec<String>,     // mapped from AI topics (for backward compat)
    pub entities: Vec<String>, // extracted named entities
}

// ---------------------------------------------------------------------------
// ArticlePersistence — outbound seam for persisting an AI summary result
// ---------------------------------------------------------------------------

/// Persistence seam for the summarization step. One use-case method only —
/// deliberately NOT a general store abstraction. Implementations live in
/// `infrastructure` (e.g. `D1ArticlePersistence`).
#[async_trait(?Send)]
pub trait ArticlePersistence {
    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), PipelineError>;
}

// ---------------------------------------------------------------------------
// Summarizer trait
// ---------------------------------------------------------------------------

#[async_trait(?Send)]
pub trait Summarizer {
    async fn summarize(&self, title: &str, body: &str) -> Result<SummaryResult, PipelineError>;
}

/// Outbound seam for computing a text embedding from an article's summary.
///
/// Embedding is deliberately separate from [`Summarizer`] / [`SummaryResult`]:
/// the text contract fed to the embedder (the `Title:/Summary:/Topics:` shape)
/// is owned by the composition root, which builds it via the `embedding` crate
/// and hands a pre-formatted string here. Keeping this crate free of any
/// embedding provider/model/Cloudflare knowledge is what lets the same code run
/// against Workers AI today and another provider tomorrow.
///
/// The composition root implements this by wrapping a concrete embedder (e.g.
/// `embedding::WorkersAiEmbedder`). Embedding runs *after* `process_article`
/// has already persisted the article's summary + tags, so a failure on this
/// seam never loses the article — the caller logs + meters it and the vector is
/// simply absent (re-embeddable by the admin rebuild endpoint).
#[async_trait(?Send)]
pub trait Embedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, PipelineError>;
}

/// Runs summarization for one article and writes the result back through an
/// [`ArticlePersistence`] seam. `score` is the rules-engine output computed
/// upstream.
///
/// Embedding is intentionally NOT performed here: it needs the freshly-built
/// summary + normalized tags, whose text contract lives in the composition
/// root. Callers run the [`Embedder`] seam after this returns (and only then,
/// so an embed failure can never skip or roll back this persistence).
pub async fn process_article(
    persistence: &impl ArticlePersistence,
    summarizer: &dyn Summarizer,
    article_id: i64,
    title: &str,
    body: &str,
    score: f64,
) -> Result<SummaryResult, PipelineError> {
    let result = summarizer.summarize(title, body).await?;
    let normalized = tag_normalizer::normalize_tags(&result.tags);
    let tags_json = serde_json::to_string(&normalized).unwrap_or_else(|_| "[]".to_string());
    let vector_id = format!("article-{article_id}");
    persistence.set_ai_summary(article_id, &result.summary, &tags_json, &vector_id, score).await?;
    Ok(result)
}

// ---------------------------------------------------------------------------
// HttpSummarizer — DeepSeek (or any OpenAI-compatible) chat via ModelProvider
// ---------------------------------------------------------------------------

pub struct HttpSummarizer {
    /// Model provider for LLM chat completions.
    provider: Box<dyn model_runtime::ModelProvider>,
}

impl HttpSummarizer {
    /// Create a new HttpSummarizer backed by a ModelProvider.
    /// The provider handles all chat completions.
    pub fn new(provider: Box<dyn model_runtime::ModelProvider>) -> Self {
        Self { provider }
    }
}

/// AI response shape: topics (category-level) + entities (specific names).
/// Tags in SummaryResult get filled from topics for backward compat.
#[derive(Debug, Deserialize)]
struct ExtractionResult {
    summary: String,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    entities: Vec<String>,
    // Fallback: if the model still returns "tags", accept it as topics
    #[serde(default)]
    tags: Vec<String>,
}

/// Build the summarization prompt sent to the LLM.
pub(crate) fn build_summarize_prompt(title: &str, body: &str) -> String {
    format!(
        "Analyze this article and return JSON. RULES:\n\
         1) summary: 2-3 sentences.\n\
         2) topics: broad subject areas, 3-5 items. \
         Use Title Case, singular. DO NOT include company names, \
         product names, version numbers, or CVE IDs here.\n\
         3) entities: specific named items (companies, products, people, CVE IDs).\n\n\
         Examples of good topics: \"AI Safety\", \"Cloud Security\", \"Enterprise AI\"\n\
         Examples of bad topics: \"OpenAI\", \"GPT-5\", \"CVE-2026-xxxx\"\n\n\
         Respond ONLY with JSON: \
         {{\"summary\": string, \"topics\": string[], \"entities\": string[]}}.\n\n\
         Title: {title}\n\nBody: {body}"
    )
}

#[async_trait(?Send)]
impl Summarizer for HttpSummarizer {
    async fn summarize(&self, title: &str, body: &str) -> Result<SummaryResult, PipelineError> {
        let prompt = build_summarize_prompt(title, body);

        // Delegate chat completion to the ModelProvider
        let request = model_runtime::ModelRequest {
            task: model_runtime::ModelTask::Summarization,
            system_prompt: prompt,
            context: vec![],
            output_schema: Some(model_runtime::summary_schema()),
            parameters: model_runtime::GenerationParams { temperature: 0.3, max_tokens: 1024 },
        };

        let response = self.provider.generate(request).await.map_err(|e| PipelineError::Summarizer(e.to_string()))?;

        let content = response.parsed.map(|v| v.to_string()).unwrap_or(response.text);

        let mut extracted: ExtractionResult = serde_json::from_str(&content)
            .map_err(|e| PipelineError::Summarizer(format!("bad JSON from model: {e}")))?;

        // Fallback: if model returned "tags" instead of "topics", use tags
        if extracted.topics.is_empty() && !extracted.tags.is_empty() {
            extracted.topics = extracted.tags;
        }

        Ok(SummaryResult { summary: extracted.summary, tags: extracted.topics, entities: extracted.entities })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_contains_title() {
        let prompt = build_summarize_prompt("Test Title", "Test body text");
        assert!(prompt.contains("Test Title"));
    }

    #[test]
    fn build_prompt_contains_body() {
        let prompt = build_summarize_prompt("Title", "Some article content here");
        assert!(prompt.contains("Some article content here"));
    }

    #[test]
    fn build_prompt_contains_json_keys() {
        let prompt = build_summarize_prompt("T", "B");
        assert!(prompt.contains("summary"));
        assert!(prompt.contains("topics"));
        assert!(prompt.contains("entities"));
    }

    #[test]
    fn build_prompt_handles_special_chars() {
        let prompt = build_summarize_prompt("Quotes \"and\" braces {}", "Text with\nnewlines");
        assert!(prompt.contains("Quotes"));
        assert!(prompt.contains("braces"));
        assert!(prompt.contains("Text with"));
    }

    /// Regression guard for the embedding unification: a summary JSON payload
    /// that still carries an `embedding` array (as the old DeepSeek path
    /// produced) must deserialize into the new 3-field SummaryResult, proving
    /// the deleted field is safe for any persisted/cached payloads.
    #[test]
    fn summary_result_ignores_stray_embedding_in_json() {
        let raw = serde_json::json!({
            "summary": "S",
            "tags": ["a"],
            "entities": ["b"],
            "embedding": [0.1, 0.2, 0.3],
        });
        let r: SummaryResult = serde_json::from_value(raw).unwrap();
        assert_eq!(r.summary, "S");
        assert_eq!(r.tags, vec!["a"]);
        assert_eq!(r.entities, vec!["b"]);
    }
}
