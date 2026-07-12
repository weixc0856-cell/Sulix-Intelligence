//! LLM API 重试机制 — 指数退避

use anyhow::Result;
use std::time::Duration;

/// 最大重试次数
pub const MAX_RETRIES: u32 = 3;

/// 判断错误是否为客户端错误（不应重试）
///
/// 匹配 api.rs 中 `call_llm_inner` 的格式化错误消息：
///   "LLM API 返回错误 ({status_code}): {body}"
fn is_client_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    // 检查 400-499 范围内的状态码
    msg.contains("返回错误 (4")
}

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
                if is_client_error(&e) {
                    log::warn!("❌ Non-retryable client error: {}", e);
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
        })
        .await;
        assert!(result.is_err());
        assert_eq!(
            counter.load(std::sync::atomic::Ordering::Relaxed),
            MAX_RETRIES + 1
        );
    }

    #[tokio::test]
    async fn test_retry_skips_on_client_error() {
        // 模拟 api.rs 中的错误格式: "LLM API 返回错误 (401): ..."
        let result: anyhow::Result<i32> =
            with_retry(|| async { anyhow::bail!("LLM API 返回错误 (401): Unauthorized") }).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retry_retries_on_server_error() {
        // 服务器错误(5xx) 应重试，不是客户端错误格式
        let counter = std::sync::atomic::AtomicU32::new(0);
        let result: anyhow::Result<i32> = with_retry(|| {
            let c = &counter;
            async move {
                c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                anyhow::bail!("server error")
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(
            counter.load(std::sync::atomic::Ordering::Relaxed),
            MAX_RETRIES + 1
        );
    }
}
