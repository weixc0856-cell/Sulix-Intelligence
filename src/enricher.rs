//! Enricher 模块 — 外部知识上下文注入
//!
//! 当前实现:
//! - Wikipedia API 查询：为文章标题匹配的 Wikipedia 词条摘要，
//!   注入到 LLM 的 prompt 中作为背景上下文（技术词锚定）。
//!
//! 设计原则:
//! - 轻量级：只取 Wikipedia REST API 的 summary 字段（导语段）
//! - 防呆：失败时静默跳过，不影响主线流程
//! - 先查中文 Wikipedia，回退到英文

use anyhow::Result;

use crate::fetcher::Article;

/// 为文章批量查询 Wikipedia 摘要
///
/// 遍历文章，对每篇文章的标题尝试查询 Wikipedia。
/// 先查中文版，回退到英文版。查询结果写入 article.wiki_summary。
pub async fn enrich_with_wikipedia(articles: &mut [Article], max_concurrency: usize) -> u32 {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    let mut handles = Vec::new();

    for (i, article) in articles.iter().enumerate() {
        if article.wiki_summary.is_some() {
            continue; // 已有值则跳过
        }
        let title = article.title.clone();
        let sem = semaphore.clone();
        let client = client.clone();

        handles.push(tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .expect("semaphore closed within pipeline context");
            match fetch_wiki_summary(&client, &title).await {
                Ok(Some(summary)) => Some((i, summary)),
                _ => None,
            }
        }));
    }

    let mut enriched = 0u32;
    for handle in handles {
        if let Some((idx, summary)) = handle.await.unwrap_or(None) {
            if !summary.is_empty() {
                // 截断到 500 字以控制 token 消耗
                let end = summary.floor_char_boundary(500);
                articles[idx].wiki_summary = Some(summary[..end].to_string());
                enriched += 1;
            }
        }
    }

    if enriched > 0 {
        log::info!("📖 Wikipedia 上下文注入: {} 篇", enriched);
    }

    enriched
}

/// 查询 Wikipedia API 获取词条摘要
async fn fetch_wiki_summary(client: &reqwest::Client, title: &str) -> Result<Option<String>> {
    // 从标题中提取候选词：取前 60 个字符，去掉特殊字符
    let term: String = title
        .chars()
        .take(60)
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect();
    if term.trim().is_empty() {
        return Ok(None);
    }

    let encoded = urlencoding(&term);

    // 先查中文 Wikipedia
    let url = format!(
        "https://zh.wikipedia.org/api/rest_v1/page/summary/{}",
        encoded
    );
    match fetch_summary(client, &url).await {
        Ok(Some(s)) => return Ok(Some(s)),
        Ok(None) => { /* fall through to English */ }
        Err(_) => { /* fall through */ }
    }

    // 回退到英文 Wikipedia
    let url_en = format!(
        "https://en.wikipedia.org/api/rest_v1/page/summary/{}",
        encoded
    );
    fetch_summary(client, &url_en).await
}

/// 执行一次 Wikipedia REST API 请求，提取 extract 字段
async fn fetch_summary(client: &reqwest::Client, url: &str) -> Result<Option<String>> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Ok(None);
    }
    let data: serde_json::Value = response.json().await?;
    match data["extract"].as_str() {
        Some(s) if !s.is_empty() => Ok(Some(s.to_string())),
        _ => Ok(None),
    }
}

/// 简单的 URL 编码（避免引入依赖）
fn urlencoding(input: &str) -> String {
    let mut result = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push_str("%20"),
            _ => result.push_str(&format!("%{:02X}", byte)),
        }
    }
    result
}
