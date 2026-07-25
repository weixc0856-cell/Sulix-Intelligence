use ai_pipeline::{HttpClient, PipelineError};
use async_trait::async_trait;
use wasm_bindgen::JsValue;
use worker::*;

/// Bridges ai_pipeline::HttpClient over worker::Fetch with retry support.
pub struct WorkerHttpClient;

impl WorkerHttpClient {
    /// Sleep for `ms` milliseconds using js_sys Promise + setTimeout.
    async fn sleep_ms(ms: u64) -> Result<(), PipelineError> {
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            let cb = wasm_bindgen::prelude::Closure::once(move || {
                let _ = resolve.call0(&JsValue::NULL);
            });
            let global = js_sys::global();
            let set_timeout: js_sys::Function =
                js_sys::Reflect::get(&global, &"setTimeout".into()).expect("setTimeout global").into();
            let cb_js: JsValue = cb.as_ref().into();
            let delay_js = JsValue::from_f64(ms as f64);
            let _ = set_timeout.call2(&global, &cb_js, &delay_js);
            cb.forget();
        });
        wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|e| PipelineError::Summarizer(format!("sleep failed: {e:?}")))?;
        Ok(())
    }

    /// Execute an HTTP request, retrying on transient errors (429, 5xx).
    async fn execute_with_retry(url: &str, init: &worker::RequestInit) -> Result<worker::Response, PipelineError> {
        use worker::Fetch;

        let max_attempts = 3;
        let base_delay_ms = 500;

        for attempt in 0..max_attempts {
            let req = Request::new_with_init(url, init).map_err(|e| PipelineError::Summarizer(e.to_string()))?;
            let mut resp = Fetch::Request(req).send().await.map_err(|e| PipelineError::Summarizer(e.to_string()))?;

            let status = resp.status_code();

            if status < 400 {
                return Ok(resp);
            }

            if status < 500 && status != 429 {
                return Err(PipelineError::Summarizer(format!(
                    "API returned {}: {}",
                    status,
                    resp.text().await.unwrap_or_default()
                )));
            }

            if attempt + 1 >= max_attempts {
                return Err(PipelineError::Summarizer(format!(
                    "API returned {} after {max_attempts} attempts: {}",
                    status,
                    resp.text().await.unwrap_or_default()
                )));
            }

            Self::sleep_ms(base_delay_ms * (1u64 << attempt)).await?;
        }

        unreachable!()
    }
}

#[async_trait(?Send)]
impl HttpClient for WorkerHttpClient {
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
            wh.set(k, v).map_err(|e| PipelineError::Summarizer(e.to_string()))?;
        }
        init.with_headers(wh);
        init.with_body(Some(serde_json::to_string(body).map_err(|e| PipelineError::Summarizer(e.to_string()))?.into()));
        let mut resp = Self::execute_with_retry(url, &init).await?;
        resp.json::<serde_json::Value>().await.map_err(|e| PipelineError::Summarizer(e.to_string()))
    }
}
