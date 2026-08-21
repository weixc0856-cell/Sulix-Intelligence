use ai_pipeline::PipelineError;
use async_trait::async_trait;
use wasm_bindgen::JsValue;
use worker::*;

use crate::utils::truncate_body;

/// Rich error type for HTTP client operations.
#[derive(Debug)]
pub enum HttpClientError {
    Network(String),
    InvalidContentType { content_type: String, body_excerpt: String },
    InvalidJson { body_excerpt: String, source: String },
    Status { status: u16, body_excerpt: String },
}

impl HttpClientError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::Status { status, .. } => *status == 429 || (500..=504).contains(status),
            Self::InvalidContentType { .. } => false,
            Self::InvalidJson { .. } => false,
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::Network(e) => format!("network: {e}"),
            Self::Status { status, body_excerpt } => format!("status {status}: {body_excerpt}"),
            Self::InvalidContentType { content_type, body_excerpt } => {
                format!("unexpected content-type \"{content_type}\": {body_excerpt}")
            }
            Self::InvalidJson { body_excerpt, source } => format!("json error ({source}): {body_excerpt}"),
        }
    }
}

impl From<HttpClientError> for PipelineError {
    fn from(e: HttpClientError) -> Self {
        PipelineError::Summarizer(e.summary())
    }
}

impl From<HttpClientError> for model_runtime::ModelError {
    fn from(e: HttpClientError) -> Self {
        model_runtime::ModelError::ProviderError(e.summary())
    }
}

/// Bridges ai_pipeline::HttpClient over worker::Fetch with retry + response validation.
pub struct WorkerHttpClient;

impl WorkerHttpClient {
    async fn sleep_ms(ms: u64) -> Result<(), HttpClientError> {
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            let cb = wasm_bindgen::prelude::Closure::once(move || {
                let _ = resolve.call0(&JsValue::NULL);
            });
            let global = js_sys::global();
            // Non-panicking setTimeout lookup — fallback to no-op if unavailable
            let set_timeout: js_sys::Function = js_sys::Reflect::get(&global, &"setTimeout".into())
                .map(|v| if v.is_undefined() { js_sys::Function::new_no_args("") } else { v.into() })
                .unwrap_or_else(|_| js_sys::Function::new_no_args(""));
            let cb_js: JsValue = cb.as_ref().into();
            let delay_js = JsValue::from_f64(ms as f64);
            let _ = set_timeout.call2(&global, &cb_js, &delay_js);
            cb.forget();
        });
        wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|e| HttpClientError::Network(format!("sleep failed: {e:?}")))?;
        Ok(())
    }

    /// Read response body, validate content-type, and parse JSON.
    async fn parse_json(mut resp: worker::Response) -> Result<serde_json::Value, HttpClientError> {
        let content_type = resp.headers().get("content-type").ok().flatten().unwrap_or_default().to_lowercase();

        if !content_type.contains("application/json") {
            let body = resp.text().await.map_err(|e| HttpClientError::Network(e.to_string()))?;
            return Err(HttpClientError::InvalidContentType { content_type, body_excerpt: truncate_body(&body, 4096) });
        }

        let body = resp.text().await.map_err(|e| HttpClientError::Network(e.to_string()))?;
        serde_json::from_str(&body).map_err(|e| HttpClientError::InvalidJson {
            body_excerpt: truncate_body(&body, 4096),
            source: e.to_string(),
        })
    }

    async fn execute_with_retry(url: &str, init: &worker::RequestInit) -> Result<serde_json::Value, HttpClientError> {
        use worker::Fetch;

        let max_attempts = 3;
        let base_delay_ms = 500;

        for attempt in 0..max_attempts {
            let req = Request::new_with_init(url, init).map_err(|e| HttpClientError::Network(e.to_string()))?;
            let mut resp = Fetch::Request(req).send().await.map_err(|e| HttpClientError::Network(e.to_string()))?;

            let status = resp.status_code();

            if status < 400 {
                return Self::parse_json(resp).await;
            }

            let body = resp.text().await.unwrap_or_default();
            let body_excerpt = truncate_body(&body, 4096);
            let err = HttpClientError::Status { status, body_excerpt };

            if err.is_retryable() && attempt + 1 < max_attempts {
                Self::sleep_ms(base_delay_ms * (1u64 << attempt)).await?;
                continue;
            }

            return Err(err);
        }

        unreachable!()
    }
}

#[async_trait(?Send)]
impl ai_pipeline::HttpClient for WorkerHttpClient {
    async fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, PipelineError> {
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        let wh = Headers::new();
        for (k, v) in headers {
            wh.set(k, v).map_err(|e| HttpClientError::Network(e.to_string()))?;
        }
        init.with_headers(wh);
        init.with_body(Some(serde_json::to_string(body).map_err(|e| HttpClientError::Network(e.to_string()))?.into()));
        Ok(Self::execute_with_retry(url, &init).await?)
    }
}

#[async_trait(?Send)]
impl model_runtime::HttpClient for WorkerHttpClient {
    async fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, model_runtime::ModelError> {
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        let wh = Headers::new();
        for (k, v) in headers {
            wh.set(k, v).map_err(|e| model_runtime::ModelError::ProviderError(e.to_string()))?;
        }
        init.with_body(Some(
            serde_json::to_string(body).map_err(|e| model_runtime::ModelError::ProviderError(e.to_string()))?.into(),
        ));
        Self::execute_with_retry(url, &init).await.map_err(|e| model_runtime::ModelError::ProviderError(e.summary()))
    }
}
