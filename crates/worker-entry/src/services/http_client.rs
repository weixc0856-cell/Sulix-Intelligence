use async_trait::async_trait;
use worker::*;
use ai_pipeline::{HttpClient, PipelineError};

/// Bridges ai_pipeline::HttpClient over worker::Fetch
pub struct WorkerHttpClient;

#[async_trait(?Send)]
impl HttpClient for WorkerHttpClient {
    async fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, PipelineError> {
        use worker::{Fetch, Headers, Method, Request, RequestInit};
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        let wh = Headers::new();
        for (k, v) in headers {
            wh.set(k, v).map_err(|e| PipelineError::Summarizer(e.to_string()))?;
        }
        init.with_headers(wh);
        init.with_body(Some(serde_json::to_string(body).map_err(|e| PipelineError::Summarizer(e.to_string()))?.into()));
        let req = Request::new_with_init(url, &init).map_err(|e| PipelineError::Summarizer(e.to_string()))?;
        let mut resp = Fetch::Request(req).send().await.map_err(|e| PipelineError::Summarizer(e.to_string()))?;
        if resp.status_code() >= 400 {
            return Err(PipelineError::Summarizer(format!(
                "API returned {}: {}",
                resp.status_code(),
                resp.text().await.unwrap_or_default()
            )));
        }
        resp.json::<serde_json::Value>().await.map_err(|e| PipelineError::Summarizer(e.to_string()))
    }
}
