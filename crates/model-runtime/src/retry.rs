//! Retry with exponential backoff for transient model API errors.

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

/// HTTP status codes that are considered transient (retryable).
pub fn is_transient_status(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}
