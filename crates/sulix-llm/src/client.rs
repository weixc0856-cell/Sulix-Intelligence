//! LLM HTTP 客户端工厂

use std::time::Duration;
use anyhow::Result;

/// Create a reqwest Client with the given timeout in seconds.
pub fn create_client(timeout_secs: u64) -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?)
}

/// Convenience: LLM API calls with 30-second timeout.
pub fn create_llm_client() -> Result<reqwest::Client> {
    create_client(30)
}

/// Convenience: external source fetches with 60-second timeout.
pub fn create_source_client() -> Result<reqwest::Client> {
    create_client(60)
}
