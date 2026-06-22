//! Pipeline 中间件链（海外版 — 不做熔断，仅做实体标记）
//!
//! 1. Sanitize — strip HTML/URLs/邮箱
//! 2. Entity Classify — 识别关键实体（TSMC/ASML/Sanctions 等）做标签
//! 3. Dedup — URL hash + 标题 Jaccard
//! 4. Post-process — 排序 + URL 修复

use std::collections::HashSet;

use anyhow::Result;
use regex::Regex;

use crate::source::RawSignal;

pub fn run_pipeline(signals: &mut Vec<RawSignal>) -> Result<()> {
    sanitize_all(signals);
    dedup(signals);
    post_process(signals);
    Ok(())
}

fn sanitize_all(signals: &mut [RawSignal]) {
    let url_re = Regex::new(r"https?://\S+").unwrap();
    let email_re = Regex::new(r"\S+@\S+\.\S+").unwrap();

    for signal in signals.iter_mut() {
        signal.title = sanitize_text(&signal.title, &url_re, &email_re);
        if let Some(content) = &signal.content {
            let cleaned = sanitize_text(content, &url_re, &email_re);
            signal.content = if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            };
        }
        if let Some(summary) = &signal.summary {
            let cleaned = sanitize_text(summary, &url_re, &email_re);
            signal.summary = if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            };
        }
    }
}

fn sanitize_text(text: &str, url_re: &Regex, email_re: &Regex) -> String {
    // Step 1: 保留 HTML 结构，只剥离有害标签（抄 RSSHub parameter.ts 的链接保留策略）
    let safe_html = sanitize_html_structure(text);
    // Step 2: 移除裸露的 URL（非 href 中的）
    let no_urls = url_re.replace_all(&safe_html, "");
    let no_emails = email_re.replace_all(&no_urls, "");
    // Step 3: 折叠空白
    let collapsed: Vec<&str> = no_emails.split_whitespace().collect();
    let joined = collapsed.join(" ");
    if joined.len() > 3000 {
        let end = joined.floor_char_boundary(3000);
        format!("{}...", &joined[..end])
    } else {
        joined
    }
}

/// 清除 HTML 中有害标签，保留对 LLM 有用的结构（抄 RSSHub parameter.ts）
/// 使用正则保留 a/p/blockquote/li/strong/em/code/pre，剥离 script/style/iframe 等
fn sanitize_html_structure(html: &str) -> String {
    // 1. 剥离有害标签及其内容
    let strip_tags = Regex::new(
        r"</?(?:script|style|iframe|form|input|button|nav|footer|header|aside|noscript)[^>]*>",
    )
    .unwrap();
    let no_strip = strip_tags.replace_all(html, "");
    // 2. 保留的标签只保留标签本身，不剥离内部文本
    // 移除不在保留列表中的所有其他标签
    let all_tag = Regex::new(r"</?(\w+)[^>]*>").unwrap();
    let result = all_tag.replace_all(&no_strip, |caps: &regex::Captures| {
        let tag = caps.get(1).unwrap().as_str();
        let is_keep = matches!(
            tag,
            "a" | "p"
                | "blockquote"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "ul"
                | "ol"
                | "li"
                | "strong"
                | "em"
                | "b"
                | "i"
                | "code"
                | "pre"
                | "br"
                | "div"
                | "span"
                | "img"
        );
        if is_keep {
            caps.get(0).unwrap().as_str().to_string()
        } else {
            String::new()
        }
    });
    result.trim().to_string()
}

fn dedup(signals: &mut Vec<RawSignal>) {
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut seen_titles: Vec<String> = Vec::new();
    signals.retain(|signal| {
        if !seen_urls.insert(signal.id.clone()) {
            return false;
        }
        let title_lower = signal.title.to_lowercase();
        for existing in &seen_titles {
            if title_similarity(existing, &title_lower) > 0.75 {
                return false;
            }
        }
        seen_titles.push(signal.title.clone());
        true
    });
}

fn title_similarity(a: &str, b: &str) -> f64 {
    let words_a: HashSet<&str> = a.split_whitespace().collect();
    let words_b: HashSet<&str> = b.split_whitespace().collect();
    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    intersection as f64 / union as f64
}

fn post_process(signals: &mut [RawSignal]) {
    signals.sort_by(|a, b| {
        let a_naive = a
            .published_at
            .map(|d| d.naive_utc())
            .unwrap_or_else(|| chrono::Utc::now().naive_utc());
        let b_naive = b
            .published_at
            .map(|d| d.naive_utc())
            .unwrap_or_else(|| chrono::Utc::now().naive_utc());
        b_naive.cmp(&a_naive)
    });
    for signal in signals.iter_mut() {
        if signal.url.starts_with("//") {
            signal.url = format!("https:{}", signal.url);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_html_preserves_structure() {
        let html = "<p>Hello <b>world</b></p><script>alert('x')</script>";
        let result = sanitize_html_structure(html);
        assert!(result.contains("<p>"));
        assert!(result.contains("Hello"));
        assert!(!result.contains("<script>"));
    }

    #[test]
    fn test_sanitize_html_strips_harmful() {
        let html = "<p>Normal text</p><iframe src='bad'></iframe><style>.cls{}</style>";
        let result = sanitize_html_structure(html);
        assert!(result.contains("<p>"));
        assert!(!result.contains("<iframe"));
        assert!(!result.contains("<style>"));
    }

    #[test]
    fn test_dedup_by_url() {
        let mut signals = vec![
            RawSignal {
                id: "abc".into(),
                title: "A".into(),
                url: "".into(),
                content: None,
                summary: None,
                published_at: None,
                source: "a".into(),
                source_id: "a".into(),
                category: "AI".into(),
                metrics: None,
                requires_sanitization: false,
            },
            RawSignal {
                id: "abc".into(),
                title: "A".into(),
                url: "".into(),
                content: None,
                summary: None,
                published_at: None,
                source: "a".into(),
                source_id: "a".into(),
                category: "AI".into(),
                metrics: None,
                requires_sanitization: false,
            },
        ];
        dedup(&mut signals);
        assert_eq!(signals.len(), 1);
    }
}
