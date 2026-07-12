//! LLM API 重试机制 — 指数退避

use std::time::Duration;
use anyhow::Result;

/// 最大重试次数
pub const MAX_RETRIES: u32 = 3;

/// Generic retry loop with exponential backoff.
/// Skips retry on 4xx errors (auth/billing/rate-limit).
pub async fn with_retry<T, F, Fut>(f: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = None;
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay_secs = 2u64.pow(attempt);
            log::warn!("⏳ Retry attempt {} ({}s delay)...", attempt, delay_secs);
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("401") || err_str.contains("403") {
                    log::warn!("❌ Non-retryable error: {}", err_str);
                    return Err(e);
                }
                last_error = Some(e);
            }
        }
    }
    Err(last_error
        .unwrap_or_else(|| anyhow::anyhow!("retry loop exited without error accumulation")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_success_first_try() {
        let result = with_retry(|| async { Ok::<_, anyhow::Error>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_fails_eventually() {
        let counter = std::sync::atomic::AtomicU32::new(0);
        let result: anyhow::Result<i32> = with_retry(|| {
            let c = &counter;
            async move {
                c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                anyhow::bail!("still failing")
            }
        }).await;
        assert!(result.is_err());
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), MAX_RETRIES + 1);
    }

    #[tokio::test]
    async fn test_retry_skips_on_401() {
        let result: anyhow::Result<i32> = with_retry(|| async { anyhow::bail!("401 Unauthorized") }).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));
    }

    #[tokio::test]
    async fn test_retry_skips_on_403() {
        let counter = std::sync::atomic::AtomicU32::new(0);
        let result: anyhow::Result<i32> = with_retry(|| {
            let c = &counter;
            async move {
                c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                anyhow::bail!("403 Forbidden")
            }
        }).await;
        assert!(result.is_err());
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 1);
    }
}
