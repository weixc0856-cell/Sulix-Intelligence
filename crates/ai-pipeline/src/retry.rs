//! Retry with exponential backoff for transient LLM API errors.
//!
//! This module is pure logic — no Worker dependency. The actual sleep/retry
//! happens in the worker-entry crate's HTTP client.

/// Policy for retry behaviour.
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub base_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 3, base_delay_ms: 500 }
    }
}

/// Types that can indicate whether a retry is worthwhile.
pub trait RetryableError {
    /// Returns `true` if the operation should be retried despite this error.
    fn should_retry(&self) -> bool;
}

/// HTTP status codes that are considered transient (retryable).
pub fn is_transient_status(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}
