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
    pub tags: Vec<String>,
    /// Embedding vector to upsert into Vectorize; kept generic here so this
    /// crate doesn't hard-depend on the Vectorize binding shape.
    pub embedding: Vec<f32>,
}

// ---------------------------------------------------------------------------
// HTTP client abstraction — the only bridge to worker::Fetch
// ---------------------------------------------------------------------------

/// Minimal HTTP client that can POST a JSON body and return a parsed JSON
/// response.  Implemented in `worker-entry` using `worker::Fetch` so the
/// `ai-pipeline` crate itself stays free of the Workers runtime dependency.
#[async_trait(?Send)]
pub trait HttpClient {
    /// POST `body` (serialised as JSON) to `url` with the supplied headers.
    /// Returns a parsed JSON response on HTTP 2xx, or an error for >=400
    /// statuses / network failures.
    async fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &serde_json::Value,
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
/// `store`. `score` is the rules-engine output computed upstream (see the
/// `rules` crate) and passed in here rather than recomputed.
pub async fn process_article(
    store: &Store,
    summarizer: &dyn Summarizer,
    article_id: i64,
    title: &str,
    body: &str,
    score: f64,
) -> Result<SummaryResult, PipelineError> {
    let result = summarizer.summarize(title, body).await?;
    let tags_json = serde_json::to_string(&result.tags).unwrap_or_else(|_| "[]".to_string());

    // vector_id convention: one embedding per article, keyed by article id.
    let vector_id = format!("article-{article_id}");

    store
        .set_ai_summary(article_id, &result.summary, &tags_json, &vector_id, score)
        .await?;

    // Return SummaryResult so the caller (worker-entry) can upsert
    // `result.embedding` into Vectorize — kept out of this crate to
    // avoid coupling ai-pipeline to a specific Cloudflare binding type.
    Ok(result)
}

// ---------------------------------------------------------------------------
// HttpSummarizer — calls any OpenAI-compatible API
// ---------------------------------------------------------------------------

/// Calls any OpenAI-compatible chat-completions + embeddings API.
///
/// Transport is handled by a caller-provided [`HttpClient`] (see
/// `WorkerHttpClient` in `worker-entry`) so this struct works without
/// `worker::Fetch` and is testable with a mock client.
pub struct HttpSummarizer {
    base_url: String,
    api_key: String,
    chat_model: String,
    embedding_model: String,
    client: Box<dyn HttpClient>,
}

impl HttpSummarizer {
    pub fn new(
        base_url: String,
        api_key: String,
        chat_model: String,
        embedding_model: String,
        client: Box<dyn HttpClient>,
    ) -> Self {
        Self {
            base_url,
            api_key,
            chat_model,
            embedding_model,
            client,
        }
    }

    fn auth_headers(&self) -> Vec<(String, String)> {
        vec![
            ("Content-Type".into(), "application/json".into()),
            ("Authorization".into(), format!("Bearer {}", self.api_key)),
        ]
    }

    async fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, PipelineError> {
        let url = format!("{}{}", self.base_url, path);
        self.client.post_json(&url, &self.auth_headers(), body).await
    }
}

/// Structured extraction contract: ask the model to return exactly this
/// shape as JSON so parsing doesn't depend on fragile prose-scraping.
#[derive(Debug, Deserialize)]
struct ExtractionResult {
    summary: String,
    tags: Vec<String>,
}

#[async_trait(?Send)]
impl Summarizer for HttpSummarizer {
    async fn summarize(&self, title: &str, body: &str) -> Result<SummaryResult, PipelineError> {
        let prompt = format!(
            "Summarize this article in 2-3 sentences and give 3-5 topical tags. \
             Respond with ONLY a JSON object: {{\"summary\": string, \"tags\": string[]}}.\n\n\
             Title: {title}\n\nBody: {body}"
        );

        let chat_response = self
            .post_json(
                "/chat/completions",
                &serde_json::json!({
                    "model": self.chat_model,
                    "messages": [{"role": "user", "content": prompt}],
                    "response_format": {"type": "json_object"}
                }),
            )
            .await?;

        let content = chat_response["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                PipelineError::Summarizer(
                    "missing message content in chat response".into(),
                )
            })?;

        let extracted: ExtractionResult = serde_json::from_str(content).map_err(|e| {
            PipelineError::Summarizer(format!("bad JSON from model: {e}"))
        })?;

        // Embedding is optional (DeepSeek doesn't provide it). When no
        // model is configured, return an empty vector so the rest of the
        // pipeline still works -- Vectorize upsert will be a no-op.
        let embedding = if self.embedding_model.is_empty() {
            Vec::new()
        } else {
            match self
                .post_json(
                    "/embeddings",
                    &serde_json::json!({
                        "model": self.embedding_model,
                        "input": format!("{title}\n{}", extracted.summary)
                    }),
                )
                .await
            {
                Ok(resp) => resp["data"][0]["embedding"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect()
                    })
                    .unwrap_or_default(),
                Err(_) => {
                    // Embedding failures are non-fatal — the article still
                    // gets a summary and tags.  Callers (worker-entry) can
                    // log this via their own observability layer.
                    Vec::new()
                }
            }
        };

        Ok(SummaryResult {
            summary: extracted.summary,
            tags: extracted.tags,
            embedding,
        })
    }
}
