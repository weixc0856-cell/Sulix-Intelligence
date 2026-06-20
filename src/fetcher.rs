//! 抓取模块 — feed-rs 并发拉取所有 RSS 源 + 正文提取

use anyhow::Result;
use chrono::{DateTime, FixedOffset};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Arc;

use crate::config::SourceConfig;

/// 统一文章结构
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
}

/// 并发拉取所有已启用的 RSS 源
pub async fn fetch_all_sources(sources: &[SourceConfig]) -> Result<Vec<Article>> {
    let enabled_sources: Vec<&SourceConfig> = sources.iter().filter(|s| s.enabled).collect();

    if enabled_sources.is_empty() {
        return Ok(Vec::new());
    }

    let mut tasks = Vec::new();

    for source in enabled_sources {
        let source = Arc::new(source.clone());
        tasks.push(tokio::spawn(async move {
            fetch_single_source(&source).await.unwrap_or_else(|e| {
                log::warn!("⚠️ 源 [{}] 抓取失败: {}", source.name, e);
                Vec::new()
            })
        }));
    }

    let mut all_articles = Vec::new();
    for task in tasks {
        all_articles.extend(task.await.unwrap_or_default());
    }

    all_articles.sort_by(|a, b| {
        b.published_at
            .unwrap_or(DateTime::<FixedOffset>::MIN_UTC.into())
            .cmp(
                &a.published_at
                    .unwrap_or(DateTime::<FixedOffset>::MIN_UTC.into()),
            )
    });

    Ok(all_articles)
}

/// 拉取单个 RSS 源
async fn fetch_single_source(source: &SourceConfig) -> Result<Vec<Article>> {
    log::debug!("抓取 [{}] → {}", source.name, source.url);

    let client = reqwest::Client::builder()
        .user_agent("Sulix-Intel/0.1")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client.get(&source.url).send().await?;
    let bytes = response.bytes().await?;
    let feed = feed_rs::parser::parse(Cursor::new(&bytes))?;

    let articles: Vec<Article> = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let title = entry.title.map(|t| t.content).unwrap_or_default();
            if title.is_empty() {
                return None;
            }

            let url = entry
                .links
                .iter()
                .find(|l| l.rel.as_deref() == Some("alternate") || l.rel.is_none())
                .or_else(|| entry.links.first())
                .map(|l| l.href.clone())?;

            let id = simple_hash(&url);

            let content = entry
                .content
                .and_then(|c| c.body)
                .or_else(|| entry.summary.as_ref().map(|s| s.content.clone()));

            let summary = entry.summary.map(|s| s.content);

            Some(Article {
                id,
                source: source.name.clone(),
                title,
                url,
                content,
                summary,
                published_at: entry.published.map(|d| d.fixed_offset()),
                category: source.category.clone(),
            })
        })
        .collect();

    log::info!("✅ [{}] → {} 篇文章", source.name, articles.len());
    Ok(articles)
}

// ===== P0: 正文提取 =====

/// 为内容不足的文章抓取原文并提取正文
/// 如果 RSS 提供的 content/summary 少于一阈值，去文章 URL 获取完整 HTML 并提取正文
pub async fn enrich_articles_content(
    articles: &mut [Article],
    max_concurrency: usize,
) -> u32 {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_default();

    // 找出需要补充内容的文章
    let mut tasks = Vec::new();
    for i in 0..articles.len() {
        let content_len = articles[i]
            .content
            .as_ref()
            .map(|c| c.len())
            .unwrap_or(0);

        // 内容不足 150 字符 → 去原文抓取
        if content_len < 150 {
            let url = articles[i].url.clone();
            let idx = i;

            // 此时不能直接 move articles，用 index 方式
            tasks.push((idx, url, client.clone()));
        }
    }

    if tasks.is_empty() {
        return 0;
    }

    log::info!(
        "📄 需要补充正文: {}/{} 篇文章",
        tasks.len(),
        articles.len()
    );

    // 用 Semaphore 限制并发
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    let mut handles = Vec::new();

    for (idx, url, client) in tasks {
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
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
        if let Some((idx, text)) = handle.await.unwrap_or(None) {
            if !text.is_empty() {
                articles[idx].content = Some(text);
                enriched_count += 1;
            }
        }
    }

    log::info!("✅ 正文补充完成: {} 篇", enriched_count);
    enriched_count
}

/// 抓取文章 URL 的 HTML 并提取正文
async fn fetch_article_content(
    client: &reqwest::Client,
    url: &str,
) -> Result<String> {
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("HTTP {}", response.status()));
    }

    let html = response.text().await?;

    if html.len() > 1_000_000 {
        // 超过 1MB 的页面截取前 500KB
        let truncated = &html[..500_000];
        Ok(extract_text_from_html(truncated))
    } else {
        Ok(extract_text_from_html(&html))
    }
}

/// 从 HTML 中提取正文文本（顺序：article → main → p 集合 → body）
fn extract_text_from_html(html: &str) -> String {
    let doc = Html::parse_document(html);

    // 1. 尝试 <article>
    if let Ok(sel) = Selector::parse("article") {
        if let Some(el) = doc.select(&sel).next() {
            let text = collect_text(el.text());
            if text.len() > 100 {
                return limit_text(text, 3000);
            }
        }
    }

    // 2. 尝试 <main>
    if let Ok(sel) = Selector::parse("main") {
        if let Some(el) = doc.select(&sel).next() {
            let text = collect_text(el.text());
            if text.len() > 100 {
                return limit_text(text, 3000);
            }
        }
    }

    // 3. 尝试 <body> 下所有 <p> 标签
    if let Ok(sel) = Selector::parse("body") {
        if let Some(body) = doc.select(&sel).next() {
            // 从 body 中提取所有 p 标签
            if let Ok(p_sel) = Selector::parse("p") {
                let paragraphs: Vec<String> = body
                    .select(&p_sel)
                    .map(|el| {
                        el.text()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .trim()
                            .to_string()
                    })
                    .filter(|t| t.len() > 20) // 过滤短段落
                    .collect();

                if !paragraphs.is_empty() {
                    let text = paragraphs.join("\n\n");
                    if text.len() > 50 {
                        return limit_text(text, 3000);
                    }
                }
            }

            // 4. 没有 p 标签 → body 全文
            let text = collect_text(body.text());
            return limit_text(text, 3000);
        }
    }

    String::new()
}

/// 将 text() 迭代器收集为字符串
fn collect_text<'a>(iter: impl Iterator<Item = &'a str>) -> String {
    iter.collect::<Vec<_>>().join(" ")
}

/// 限制文本长度，保留完整句子边界
fn limit_text(text: String, max_len: usize) -> String {
    if text.len() <= max_len {
        return text;
    }

    // 在 max_len 附近找最后一个句号
    let end = text[..max_len].rfind('。').or_else(|| text[..max_len].rfind('.'));
    let end = end.or_else(|| text[..max_len].rfind('\n'));

    match end {
        Some(pos) if pos > max_len / 2 => format!("{}...", &text[..=pos]),
        _ => format!("{}...", &text[..max_len]),
    }
}

/// 简单的 URL hash 作为 ID
fn simple_hash(url: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
