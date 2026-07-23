//! Summarize + tag + embed a single article, then persist the result
//! through `store`. The actual model call is behind `Summarizer` so it can
//! point at Workers AI, or an external LLM API (whichever you decide has
//! better quality/cost for summarization) without touching this crate's
//! callers -- only the concrete impl passed in at the composition root
//! changes.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::{Store, StoreError};
use worker::{Fetch, Headers, Method, Request, RequestInit};

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

/// Calls any OpenAI-compatible chat-completions + embeddings API directly
/// over `worker::Fetch` -- deliberately no `async-openai` (needs Tokio,
/// doesn't target wasm32) and no `async-openai-wasm` either (its docs
/// still lean on browser `window.fetch` semantics the Workers isolate
/// doesn't provide; `window` doesn't exist there). This is the same
/// "hand-roll the HTTP call over worker::Fetch" pattern already used in
/// the `fetcher` crate, just pointed at a different API.
pub struct HttpSummarizer {
    base_url: String,
    api_key: String,
    chat_model: String,
    embedding_model: String,
}

impl HttpSummarizer {
    pub fn new(base_url: String, api_key: String, chat_model: String, embedding_model: String) -> Self {
        Self {
            base_url,
            api_key,
            chat_model,
            embedding_model,
        }
    }

    fn auth_headers(&self) -> Result<Headers, PipelineError> {
        let headers = Headers::new();
        headers
            .set("Content-Type", "application/json")
            .map_err(|e| PipelineError::Summarizer(e.to_string()))?;
        headers
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .map_err(|e| PipelineError::Summarizer(e.to_string()))?;
        Ok(headers)
    }

    async fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value, PipelineError> {
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        init.with_headers(self.auth_headers()?);
        init.with_body(Some(
            serde_json::to_string(body)
                .map_err(|e| PipelineError::Summarizer(e.to_string()))?
                .into(),
        ));

        let url = format!("{}{}", self.base_url, path);
        let req = Request::new_with_init(&url, &init).map_err(|e| PipelineError::Summarizer(e.to_string()))?;

        let mut resp = Fetch::Request(req)
            .send()
            .await
            .map_err(|e| PipelineError::Summarizer(e.to_string()))?;

        if resp.status_code() >= 400 {
            let err_body = resp.text().await.unwrap_or_default();
            return Err(PipelineError::Summarizer(format!(
                "API returned {}: {}",
                resp.status_code(),
                err_body
            )));
        }

        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| PipelineError::Summarizer(e.to_string()))
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
            .ok_or_else(|| PipelineError::Summarizer("missing message content in chat response".into()))?;

        let extracted: ExtractionResult =
            serde_json::from_str(content).map_err(|e| PipelineError::Summarizer(format!("bad JSON from model: {e}")))?;

        // Embedding is optional (DeepSeek doesn't provide it). When no
        // model is configured, return an empty vector so the rest of the
        // pipeline still works -- Vectorize upsert will be a no-op.
        let embedding = if self.embedding_model.is_empty() {
            Vec::new()
        } else {
            match self.post_json("/embeddings", &serde_json::json!({
                "model": self.embedding_model,
                "input": format!("{title}\n{}", extracted.summary)
            })).await {
                Ok(resp) => resp["data"][0]["embedding"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
                    .unwrap_or_default(),
                Err(e) => {
                    worker::console_log!("embedding call failed (non-fatal): {e}");
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
