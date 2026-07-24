//! Per-pipeline-run timing and counter metrics.
//!
//! A single [`PipelineMetrics`] instance is created at the start of each
//! `process_one_feed` invocation; callers record durations and event
//! counts via the convenience methods below.  At the end of the pipeline
//! run the accumulator is exposed via `/api/pipeline/status`.

/// Accumulated timing and counter data for one pipeline cycle.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct PipelineMetrics {
    // Timings (milliseconds, cumulative across articles in the batch)
    pub fetch_ms: f64,
    pub parse_ms: f64,
    pub store_ms: f64,
    pub llm_ms: f64,
    pub embedding_ms: f64,
    pub r2_ms: f64,
    // Counters
    pub articles_fetched: u32,
    pub articles_new: u32,
    pub articles_dup: u32,
    pub errors: u32,
}

impl PipelineMetrics {
    /// Record the elapsed duration (in milliseconds) for a named step.
    /// `start` should be a value from `js_sys::Date::now()`.
    pub fn record_ms(&mut self, step: &str, elapsed_ms: f64) {
        match step {
            "fetch" => self.fetch_ms += elapsed_ms,
            "parse" => self.parse_ms += elapsed_ms,
            "store" => self.store_ms += elapsed_ms,
            "llm" => self.llm_ms += elapsed_ms,
            "embedding" => self.embedding_ms += elapsed_ms,
            "r2" => self.r2_ms += elapsed_ms,
            _ => {}
        }
    }

    /// Helper: compute duration from a `js_sys::Date::now()` timestamp.
    pub fn since(start: f64) -> f64 {
        js_sys::Date::now() - start
    }

    /// Take a one-field snapshot encoded as a JSON object (for API responses).
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "fetch_ms":      (self.fetch_ms * 10.0).round() / 10.0,
            "parse_ms":      (self.parse_ms * 10.0).round() / 10.0,
            "store_ms":      (self.store_ms * 10.0).round() / 10.0,
            "llm_ms":        (self.llm_ms * 10.0).round() / 10.0,
            "embedding_ms":  (self.embedding_ms * 10.0).round() / 10.0,
            "r2_ms":         (self.r2_ms * 10.0).round() / 10.0,
            "articles_fetched": self.articles_fetched,
            "articles_new":     self.articles_new,
            "articles_dup":     self.articles_dup,
            "errors":           self.errors,
        })
    }
}
