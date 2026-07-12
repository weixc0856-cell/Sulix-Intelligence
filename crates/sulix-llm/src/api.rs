//! LLM API 调用 — HTTP 请求 + 响应处理

use anyhow::Result;
use sulix_config::LlmConfig;

use crate::audit::{LLM_CALL_COUNT, LLM_INPUT_TOKENS, LLM_OUTPUT_TOKENS};
use crate::parser::parse_json_response;
use crate::retry;
use crate::types::{AnalyzedArticleRaw, ChatResponse};

/// 带指数退避重试的 API 调用，返回结构化结果
pub async fn call_with_retry(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<Vec<AnalyzedArticleRaw>> {
    retry::with_retry(|| call_completion(client, api_key, llm_config, system_prompt, user_prompt))
        .await
}

/// 调用 LLM API 返回原始文本，带指数退避重试
pub async fn call_with_retry_raw(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    retry::with_retry(|| call_raw_inner(client, api_key, llm_config, system_prompt, user_prompt))
        .await
}

/// Core LLM call: build request, send, extract content string.
async fn call_llm_inner(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
    max_tokens: u32,
    temperature: f32,
) -> Result<String> {
    LLM_CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let input_est = (system_prompt.len() + user_prompt.len()) as u64;
    LLM_INPUT_TOKENS.fetch_add(input_est / 4, std::sync::atomic::Ordering::Relaxed);

    let url = format!(
        "{}/chat/completions",
        llm_config.base_url.trim_end_matches('/')
    );

    let request_body = serde_json::json!({
        "model": llm_config.model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "max_tokens": max_tokens,
        "temperature": temperature,
        "response_format": {"type": "json_object"},
        "seed": 42
    });

    log::debug!("LLM 请求: {} ({} tokens max)", url, max_tokens);

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<body read failed: {}>", e));
        return Err(anyhow::anyhow!(
            "LLM API 返回错误 ({}): {}",
            status,
            error_text
        ));
    }

    let chat_response: ChatResponse = response.json().await?;
    let content = chat_response
        .choices
        .first()
        .map(|c| &c.message.content)
        .ok_or_else(|| anyhow::anyhow!("API 响应中没有 choices"))?
        .clone();

    LLM_OUTPUT_TOKENS.fetch_add(
        content.len() as u64 / 4,
        std::sync::atomic::Ordering::Relaxed,
    );

    Ok(content)
}

/// 不带重试的原始 API 调用 — 返回文本
async fn call_raw_inner(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    call_llm_inner(
        client,
        api_key,
        llm_config,
        system_prompt,
        user_prompt,
        llm_config.max_tokens.min(2048),
        llm_config.temperature.min(0.2),
    )
    .await
}

/// 实际调用 LLM API 并解析 JSON 响应
async fn call_completion(
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<Vec<AnalyzedArticleRaw>> {
    let content = call_llm_inner(
        client,
        api_key,
        llm_config,
        system_prompt,
        user_prompt,
        llm_config.max_tokens,
        llm_config.temperature,
    )
    .await?;

    parse_json_response(&content).map_err(|e| {
        let end = content.floor_char_boundary(content.len().min(100));
        anyhow::anyhow!("JSON 解析失败 ({}): {}...", e, &content[..end])
    })
}
