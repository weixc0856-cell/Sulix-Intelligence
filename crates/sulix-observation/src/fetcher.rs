//! 文本提取模块 — HTML 解析与正文抽取
//!
//! 保留函数:
//! - Article 结构体（全系统共用）
//! - enrich_articles_content（正文提取，由 P0 需求驱动）
//! - extract_text_from_html（article/main/p 三优先）

use anyhow::Result;
use chrono::{DateTime, FixedOffset};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

/// 统一文章结构（全系统共用，由 RawSignal 转换而来）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub source: String,
    pub title: String,
    pub url: String,
    pub content: Option<String>,
    pub summary: Option<String>,
    pub published_at: Option<DateTime<FixedOffset>>,
    pub category: String,
    #[serde(default)]
    pub wiki_summary: Option<String>,
    #[serde(default)]
    pub evidence_type: String,
    /// 是否为内参学习源，前端不展示溯源链接
    #[serde(default)]
    pub is_internal: bool,
}

// ===== 正文提取（P0） =====

/// 为内容不足的文章抓取原文并提取正文
pub async fn enrich_articles_content(articles: &mut [Article], max_concurrency: usize) -> u32 {
    use std::sync::Arc;

    let client = crate::client::global_client().clone();

    let mut tasks = Vec::new();
    for (i, article) in articles.iter().enumerate() {
        let content_len = article.content.as_ref().map(|c| c.len()).unwrap_or(0);
        if content_len < 150 {
            tasks.push((i, article.url.clone(), client.clone()));
        }
    }

    if tasks.is_empty() {
        return 0;
    }

    log::info!("📄 需要补充正文: {}/{} 篇文章", tasks.len(), articles.len());

    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    let mut handles = Vec::new();
    for (idx, url, client) in tasks {
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = match sem.acquire().await {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("Semaphore closed, skipping fetch [{}]: {}", url, e);
                    return None;
                }
            };
            match fetch_article_content(&client, &url).await {
                Ok(text) => Some((idx, text)),
                Err(e) => {
                    log::debug!("⚠️ 正文提取失败 [{}]: {}", url, e);
                    None
                }
            }
        }));
    }

    let mut enriched_count = 0u32;
    for handle in handles {
        match handle.await {
            Ok(Some((idx, text))) => {
                if !text.is_empty() {
                    articles[idx].content = Some(text);
                    enriched_count += 1;
                }
            }
            Ok(None) => {}
            Err(e) => log::warn!("Fetch task panicked or cancelled: {:?}", e),
        }
    }

    log::info!("✅ 正文补充完成: {} 篇", enriched_count);
    enriched_count
}

async fn fetch_article_content(client: &reqwest::Client, url: &str) -> Result<String> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("HTTP {}", response.status()));
    }
    let html = response.text().await?;
    if html.len() > 1_000_000 {
        let end = html.floor_char_boundary(500_000);
        Ok(extract_text_from_html(&html[..end]))
    } else {
        Ok(extract_text_from_html(&html))
    }
}

/// 从 HTML 中提取正文文本（顺序：article → main → p 集合 → body）
pub fn extract_text_from_html(html: &str) -> String {
    let doc = Html::parse_document(html);
    if let Ok(sel) = Selector::parse("article") {
        if let Some(el) = doc.select(&sel).next() {
            let text = collect_text(el.text());
            if text.len() > 100 {
                return limit_text(text, 3000);
            }
        }
    }
    if let Ok(sel) = Selector::parse("main") {
        if let Some(el) = doc.select(&sel).next() {
            let text = collect_text(el.text());
            if text.len() > 100 {
                return limit_text(text, 3000);
            }
        }
    }
    if let Ok(sel) = Selector::parse("body") {
        if let Some(body) = doc.select(&sel).next() {
            if let Ok(p_sel) = Selector::parse("p") {
                let paragraphs: Vec<String> = body
                    .select(&p_sel)
                    .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
                    .filter(|t| t.len() > 20)
                    .collect();
                if !paragraphs.is_empty() {
                    let text = paragraphs.join("\n\n");
                    if text.len() > 50 {
                        return limit_text(text, 3000);
                    }
                }
            }
            let text = collect_text(body.text());
            return limit_text(text, 3000);
        }
    }
    String::new()
}

fn collect_text<'a>(iter: impl Iterator<Item = &'a str>) -> String {
    iter.collect::<Vec<_>>().join(" ")
}

fn limit_text(text: String, max_len: usize) -> String {
    if text.len() <= max_len {
        return text;
    }
    let search_end = text.floor_char_boundary(max_len);
    let end = text[..search_end]
        .rfind('。')
        .or_else(|| text[..search_end].rfind('.'));
    let end = end.or_else(|| text[..search_end].rfind('\n'));
    match end {
        Some(pos) if pos > max_len / 2 => format!("{}...", &text[..=pos]),
        _ => format!("{}...", &text[..search_end]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_article_tag() {
        let html = "<html><body><article><p>This is article content that is long enough to pass the 100 char threshold. Let me add more text here to make sure we cross that barrier easily.</p></article></body></html>";
        let text = extract_text_from_html(html);
        assert!(text.contains("article content"));
        assert!(text.len() > 100);
    }

    #[test]
    fn test_extract_main_tag() {
        let html = "<html><body><main><p>This is main content that needs to be long enough to pass the 100 char threshold for extraction. Adding more filler text to ensure we reach the threshold.</p></main></body></html>";
        let text = extract_text_from_html(html);
        assert!(text.contains("main content"));
    }

    #[test]
    fn test_extract_p_tags() {
        let html = "<html><body><p>First paragraph with enough text to pass the 20 char minimum.</p><p>Second paragraph that also has enough text for the test.</p></body></html>";
        let text = extract_text_from_html(html);
        assert!(text.contains("First paragraph"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn test_extract_empty_html() {
        let text = extract_text_from_html("<html><head></head><body></body></html>");
        assert!(text.is_empty() || text.trim().is_empty());
    }

    #[test]
    fn test_limit_text_short() {
        let text = "Hello world.".to_string();
        assert_eq!(limit_text(text.clone(), 100), text);
    }

    #[test]
    fn test_limit_text_sentence_boundary() {
        let text = "First sentence. Second sentence. Third sentence.".to_string();
        let result = limit_text(text, 20);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_extract_cjk_text() {
        let html = "<html><body><article><p>大模型能力正在快速接近闭源模型水平。这是2025年最重要的趋势之一。开源模型的能力提升正在改变整个行业的竞争格局。</p></article></body></html>";
        let text = extract_text_from_html(html);
        assert!(text.contains("大模型能力"));
        assert!(text.len() > 20);
    }
}


