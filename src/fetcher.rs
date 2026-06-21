//! 抓取模块 — feed-rs 并发拉取所有 RSS 源 + 正文提取

use anyhow::Result;
use chrono::{DateTime, FixedOffset};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Arc;

use regex::Regex;

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

/// A 股关键词白名单 — 只保留包含核心增量信息的条目
/// 财联社等源单日可产出数百条，先过滤再进 LLM 管线可省 60-70% token
fn ashare_keyword_filter(articles: &mut Vec<Article>) {
    let pattern = r"(?x)
        大盘|指数|沪指|两市|万亿|成交额|成交额|分位|
        板块|轮动|概念|半导体|芯片|AI|算力|光伏|锂电|汽车|智驾|医药|券商|地产|
        主力|净流出|净流入|主力资金|融资|融券|异动|吸筹|出货|
        政策|证监会|央行|国常会|监管|产业基金|补贴|降准|降息|
        财报|预增|净利润|年报|季报|预亏|暴雷|
        北向|外资|南向|港股|恒生|
        涨停|跌停|连板|炸板|封板|打板|
        龙虎榜|游资|机构
    ";
    let re = match Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return, // 正则失效时保底放行
    };

    articles.retain(|a| {
        let title_match = re.is_match(&a.title);
        let summary_match = a
            .summary
            .as_deref()
            .map(|s| re.is_match(s))
            .unwrap_or(false);
        title_match || summary_match
    });
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

    let mut articles = articles;
    // 高吞吐 A 股源（财联社等）执行关键词预过滤，砍掉 60-70% 噪音
    if source.category == "A股" {
        let before = articles.len();
        ashare_keyword_filter(&mut articles);
        let filtered = before - articles.len();
        if filtered > 0 {
            log::info!(
                "🔍 [{}] 关键词过滤: {} → {} 篇 (移除 {} 篇噪音)",
                source.name,
                before,
                articles.len(),
                filtered
            );
        }
    }

    log::info!("✅ [{}] → {} 篇文章", source.name, articles.len());
    Ok(articles)
}

// ===== P0: 正文提取 =====

/// 为内容不足的文章抓取原文并提取正文
/// 如果 RSS 提供的 content/summary 少于一阈值，去文章 URL 获取完整 HTML 并提取正文
pub async fn enrich_articles_content(articles: &mut [Article], max_concurrency: usize) -> u32 {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_default();

    // 找出需要补充内容的文章
    let mut tasks = Vec::new();
    for (i, article) in articles.iter().enumerate() {
        let content_len = article.content.as_ref().map(|c| c.len()).unwrap_or(0);

        // 内容不足 150 字符 → 去原文抓取
        if content_len < 150 {
            let url = article.url.clone();

            // 用 index 方式访问（不能 move articles）
            tasks.push((i, url, client.clone()));
        }
    }

    if tasks.is_empty() {
        return 0;
    }

    log::info!("📄 需要补充正文: {}/{} 篇文章", tasks.len(), articles.len());

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
async fn fetch_article_content(client: &reqwest::Client, url: &str) -> Result<String> {
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("HTTP {}", response.status()));
    }

    let html = response.text().await?;

    if html.len() > 1_000_000 {
        // 超过 1MB 的页面截取前 500KB（安全处理 UTF-8 边界）
        let end = html.floor_char_boundary(500_000);
        let truncated = &html[..end];
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
                    .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
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

    // 在 max_len 附近找最后一个句号（UTF-8 安全）
    let search_end = text.floor_char_boundary(max_len);
    let end = text[..search_end]
        .rfind('。')
        .or_else(|| text[..search_end].rfind('.'));
    let end = end.or_else(|| text[..search_end].rfind('\n'));

    match end {
        Some(pos) if pos > max_len / 2 => format!("{}...", &text[..=pos]),
        _ => format!("{}...", &text[..max_len]),
    }
}

/// 用 SHA-256 对 URL 取稳定哈希（跨 Rust 版本不变）
fn simple_hash(url: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let result = hasher.finalize();
    hex_format(&result[..8]) // 取前 8 字节 → 16 位 hex
}

/// u8 切片格式化为 hex 字符串（零依赖实现）
fn hex_format(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Jaccard 相似度（字符二元组）
fn bigram_set(s: &str) -> std::collections::HashSet<(char, char)> {
    s.chars().zip(s.chars().skip(1)).collect()
}

fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let set_a = bigram_set(a);
    let set_b = bigram_set(b);
    let intersection = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// Delta 检测：同分类中标题高度相似的文章合并（不同 URL 同一新闻）
/// 保留正文最长的文章，合并来源名，丢弃其余副本
pub fn dedup_by_title(articles: &mut Vec<Article>, threshold: f64) {
    use std::collections::HashMap;
    let mut grouped: HashMap<String, Vec<Article>> = HashMap::new();
    for a in articles.drain(..) {
        grouped.entry(a.category.clone()).or_default().push(a);
    }
    let mut result: Vec<Article> = Vec::new();
    for (_cat, mut group) in grouped {
        // 正文长度降序排列（保留最全的）
        group.sort_by(|a, b| {
            let len_a = a.content.as_ref().map(|c| c.len()).unwrap_or(0);
            let len_b = b.content.as_ref().map(|c| c.len()).unwrap_or(0);
            len_b.cmp(&len_a)
        });

        let group_len = group.len();
        let mut kept: Vec<Article> = Vec::new();
        for article in group {
            let mut merged = false;
            for k in &mut kept {
                let sim = jaccard_similarity(&article.title, &k.title);
                if sim >= threshold {
                    // 合并来源名：收集所有 unique source 名称
                    let sources: Vec<&str> = k.source.split(" + ").collect();
                    if !sources.contains(&article.source.as_str()) {
                        k.source.push_str(&format!(" + {}", article.source));
                    }
                    // 如果新文章有正文而现有没有，保留正文
                    if (k.content.is_none()
                        || k.content.as_ref().map(|c| c.len()).unwrap_or(0) == 0)
                        && article.content.is_some()
                    {
                        k.content = article.content.clone();
                    }
                    merged = true;
                    log::debug!(
                        "🔀 Delta dedup: '{}' ≈ '{}' ({:.0}%) → 合并到 {}",
                        article.title,
                        k.title,
                        sim * 100.0,
                        k.source
                    );
                    break;
                }
            }
            if !merged {
                kept.push(article);
            }
        }
        let merged_count = group_len - kept.len();
        if merged_count > 0 {
            log::info!(
                "🔀 Delta dedup [{}]: {} → {} (合并 {} 篇重复)",
                _cat,
                group_len,
                kept.len(),
                merged_count
            );
        }
        result.extend(kept);
    }
    *articles = result;
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
        let html = "<html><body><main><p>This is main content that needs to be long enough to pass the 100 character threshold for extraction. Adding more filler text to ensure we reach the threshold.</p></main></body></html>";
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
    fn test_simple_hash_consistency() {
        let h1 = simple_hash("https://example.com/article");
        let h2 = simple_hash("https://example.com/article");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_simple_hash_different() {
        let h1 = simple_hash("https://example.com/a");
        let h2 = simple_hash("https://example.com/b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_simple_hash_non_empty() {
        let h = simple_hash("https://example.com/test");
        assert!(!h.is_empty());
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
}
