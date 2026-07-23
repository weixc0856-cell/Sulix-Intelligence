//! Summarize + tag + embed a single article, then persist the result
//! through `store`. The actual model call is behind `Summarizer` so it can
//! point at Workers AI, or an external LLM API (whichever you decide has
//! better quality/cost for summarization) without touching this crate's
//! callers -- only the concrete impl passed in at the composition root
//! changes.
//!
//! HTTP transport is abstracted behind [`HttpClient`] so this crate does not
//! depend on `worker::Fetch` -- the composition root (`worker-entry`) provides
//! a worker-based implementation.

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
    pub tags: Vec<String>,     // mapped from AI topics (for backward compat)
    pub entities: Vec<String>, // extracted named entities
    pub embedding: Vec<f32>,
}

// ---------------------------------------------------------------------------
// HTTP client abstraction
// ---------------------------------------------------------------------------

#[async_trait(?Send)]
pub trait HttpClient {
    async fn post_json(
        &self, url: &str, headers: &[(String, String)], body: &serde_json::Value,
    ) -> Result<serde_json::Value, PipelineError>;
}

// ---------------------------------------------------------------------------
// Summarizer trait
// ---------------------------------------------------------------------------

#[async_trait(?Send)]
pub trait Summarizer {
    async fn summarize(&self, title: &str, body: &str) -> Result<SummaryResult, PipelineError>;
}

/// Runs summarization for one article and writes the result back through
/// `store`. `score` is the rules-engine output computed upstream.
pub async fn process_article(
    store: &Store, summarizer: &dyn Summarizer, article_id: i64,
    title: &str, body: &str, score: f64,
) -> Result<SummaryResult, PipelineError> {
    let result = summarizer.summarize(title, body).await?;
    let tags_json = serde_json::to_string(&result.tags).unwrap_or_else(|_| "[]".to_string());
    let vector_id = format!("article-{article_id}");
    store.set_ai_summary(article_id, &result.summary, &tags_json, &vector_id, score).await?;
    Ok(result)
}

// ---------------------------------------------------------------------------
// HttpSummarizer
// ---------------------------------------------------------------------------

pub struct HttpSummarizer {
    base_url: String,
    api_key: String,
    chat_model: String,
    embedding_model: String,
    client: Box<dyn HttpClient>,
}

impl HttpSummarizer {
    pub fn new(base_url: String, api_key: String, chat_model: String, embedding_model: String, client: Box<dyn HttpClient>) -> Self {
        Self { base_url, api_key, chat_model, embedding_model, client }
    }

    fn auth_headers(&self) -> Vec<(String, String)> {
        vec![("Content-Type".into(), "application/json".into()), ("Authorization".into(), format!("Bearer {}", self.api_key))]
    }

    async fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, PipelineError> {
        let url = format!("{}{}", self.base_url, path);
        self.client.post_json(&url, &self.auth_headers(), body).await
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

#[async_trait(?Send)]
impl Summarizer for HttpSummarizer {
    async fn summarize(&self, title: &str, body: &str) -> Result<SummaryResult, PipelineError> {
        let prompt = format!(
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
        );

        let chat_response = self.post_json("/chat/completions", &serde_json::json!({
            "model": self.chat_model,
            "messages": [{"role": "user", "content": prompt}],
            "response_format": {"type": "json_object"}
        })).await?;

        let content = chat_response["choices"][0]["message"]["content"]
            .as_str().ok_or_else(|| PipelineError::Summarizer("missing message content".into()))?;

        let mut extracted: ExtractionResult = serde_json::from_str(content)
            .map_err(|e| PipelineError::Summarizer(format!("bad JSON from model: {e}")))?;

        // Fallback: if model returned "tags" instead of "topics", use tags
        if extracted.topics.is_empty() && !extracted.tags.is_empty() {
            extracted.topics = extracted.tags;
        }

        let embedding = if self.embedding_model.is_empty() {
            Vec::new()
        } else {
            match self.post_json("/embeddings", &serde_json::json!({
                "model": self.embedding_model,
                "input": format!("{title}\n{}", extracted.summary)
            })).await {
                Ok(resp) => resp["data"][0]["embedding"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        };

        Ok(SummaryResult {
            summary: extracted.summary,
            tags: extracted.topics,
            entities: extracted.entities,
            embedding,
        })
    }
}
