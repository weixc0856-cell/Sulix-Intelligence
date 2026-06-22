//! Pipeline 中间件链（抄 Daily-News-Briefing cleanContent + RSSHub middleware）
//!
//! 对 RawSignal 做标准化处理：
//! 1. Sanitize — strip HTML/URLs/邮箱
//! 2. Compliance — A 股个股代码拦截熔断
//! 3. Dedup — 按 URL hash + 标题相似度去重
//!
//! 所有操作在 Vec<RawSignal> 上就地修改，避免多次堆分配。

use std::collections::HashSet;

use anyhow::Result;
use regex::Regex;

use crate::source::RawSignal;

/// 运行完整 Pipeline：清洗 → 合规 → 去重 → 后处理
/// 使用 &mut 就地修改，避免 clone
pub fn run_pipeline(signals: &mut Vec<RawSignal>) -> Result<()> {
    sanitize_all(signals);
    compliance_filter(signals);
    dedup(signals);
    post_process(signals);
    Ok(())
}

/// Step 1: 内容清洗（抄 Daily-News-Briefing cleanContent）
/// strip HTML 标签 → 移除 URL/邮箱 → 折叠空白 → 截断
fn sanitize_all(signals: &mut Vec<RawSignal>) {
    let url_re = Regex::new(r"https?://\S+").unwrap();
    let email_re = Regex::new(r"\S+@\S+\.\S+").unwrap();

    for signal in signals.iter_mut() {
        // 清洗 title
        signal.title = sanitize_text(&signal.title, &url_re, &email_re);

        // 清洗 content
        if let Some(content) = &signal.content {
            let cleaned = sanitize_text(content, &url_re, &email_re);
            if cleaned.is_empty() {
                signal.content = None;
            } else {
                signal.content = Some(cleaned);
            }
        }

        // 清洗 summary
        if let Some(summary) = &signal.summary {
            let cleaned = sanitize_text(summary, &url_re, &email_re);
            if cleaned.is_empty() {
                signal.summary = None;
            } else {
                signal.summary = Some(cleaned);
            }
        }
    }
}

/// 对单段文本做清洗
fn sanitize_text(text: &str, url_re: &Regex, email_re: &Regex) -> String {
    // 1. strip HTML 标签
    let no_html = strip_html_tags(text);
    // 2. 移除 URL
    let no_urls = url_re.replace_all(&no_html, "");
    // 3. 移除邮箱
    let no_emails = email_re.replace_all(&no_urls, "");
    // 4. 折叠空白（多个空格/换行 → 一个空格）
    let collapsed: Vec<&str> = no_emails.split_whitespace().collect();
    let joined = collapsed.join(" ");
    // 5. 截断到 3000 字符（UTF-8 安全）
    if joined.len() > 3000 {
        let end = joined.floor_char_boundary(3000);
        format!("{}...", &joined[..end])
    } else {
        joined
    }
}

/// 最简单的 HTML tag 剥离（不依赖 scraper，只做标签移除）
fn strip_html_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result.trim().to_string()
}

/// Step 2: 合规过滤 — A 股个股熔断 + 荐股词过滤 + 地缘政治过滤
fn compliance_filter(signals: &mut Vec<RawSignal>) {
    let stock_re = Regex::new(r"\b[0369]\d{5}\b").unwrap();
    const A_STOCK_BLACKLIST: &[&str] = &[
        "荐股", "牛股", "妖股", "涨停板", "打板", "拉升", "出货",
        "内幕", "主力拉升", "庄家", "跟庄", "抄底", "逃顶",
        "代客理财", "配资", "实盘指导", "收益",
    ];

    const GEOPOLITICAL_BLACKLIST: &[&str] = &[
        "war", "conflict", "military", "troop", "missile",
        "drone attack", "strike", "bomb", "invasion",
        "zelensky", "putin", "biden", "trump",
        "sanctions", "election", "nuclear weapon",
        "taiwan", "taiwanese",
        "俄乌", "冲突", "袭击", "制裁", "伊朗", "导弹",
        "军事", "战争", "选举", "核武",
    ];

    signals.retain(|signal| {
        let title_lower = signal.title.to_lowercase();
        let content_lower = signal.content.as_deref().unwrap_or("").to_lowercase();
        let summary_lower = signal.summary.as_deref().unwrap_or("").to_lowercase();

        if stock_re.is_match(&signal.title) {
            log::warn!("🔴 合规熔断: 标题含个股代码 [{}] {}", signal.source, signal.title);
            return false;
        }
        if stock_re.is_match(&content_lower) {
            log::warn!("🔴 合规熔断: 正文含个股代码 [{}] {}", signal.source, signal.title);
            return false;
        }

        for &word in A_STOCK_BLACKLIST {
            if title_lower.contains(word) || content_lower.contains(word) || summary_lower.contains(word) {
                log::warn!("🔴 A股熔断: 含违规词 [{}] \"{}\": {}", word, signal.source, signal.title);
                return false;
            }
        }

        for &keyword in GEOPOLITICAL_BLACKLIST {
            if title_lower.contains(keyword) {
                log::warn!("🔴 地缘熔断: 标题含敏感词 [{}] \"{}\": {}", keyword, signal.source, signal.title);
                return false;
            }
            if content_lower.contains(keyword) {
                log::warn!("🔴 地缘熔断: 正文含敏感词 [{}] \"{}\": {}", keyword, signal.source, signal.title);
                return false;
            }
        }

        true
    });
}

/// Step 3: 去重 — 按 URL hash 去重（保留第一条）
/// 附加标题相似度去重（Jaccard + 阈值 0.75）
fn dedup(signals: &mut Vec<RawSignal>) {
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut seen_titles: Vec<String> = Vec::new();

    signals.retain(|signal| {
        // 1. URL 精确去重
        if !seen_urls.insert(signal.id.clone()) {
            return false;
        }

        // 2. 标题相似度去重
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

/// 标题相似度（基于公共单词的 Jaccard 距离，轻量版）
fn title_similarity(a: &str, b: &str) -> f64 {
    let words_a: HashSet<&str> = a.split_whitespace().collect();
    let words_b: HashSet<&str> = b.split_whitespace().collect();
    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }
    let intersection: usize = words_a.intersection(&words_b).count();
    let union: usize = words_a.union(&words_b).count();
    intersection as f64 / union as f64
}

/// Step 4: 后处理（抄 RSSHub parameter.ts 模式）
/// - 按 published_at 降序排列
/// - 修复相对 URL（//xxx → https:xxx）
fn post_process(signals: &mut Vec<RawSignal>) {
    // 1. 按发布时间降序（最新的在前）
    signals.sort_by(|a, b| {
        let a_naive = a.published_at.map(|d| d.naive_utc()).unwrap_or(chrono::NaiveDateTime::MIN);
        let b_naive = b.published_at.map(|d| d.naive_utc()).unwrap_or(chrono::NaiveDateTime::MIN);
        b_naive.cmp(&a_naive)
    });

    // 2. 修复相对 URL（//xxx → https:xxx）
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
    fn test_strip_html() {
        let html = "<p>Hello <b>world</b></p>";
        assert_eq!(strip_html_tags(html), "Hello world");
    }

    #[test]
    fn test_sanitize_removes_urls() {
        let url_re = Regex::new(r"https?://\S+").unwrap();
        let email_re = Regex::new(r"\S+@\S+\.\S+").unwrap();
        let text = "Check this https://example.com/article for details";
        let result = sanitize_text(text, &url_re, &email_re);
        assert!(!result.contains("https://"));
    }

    #[test]
    fn test_compliance_filters_stock_codes() {
        let mut signals = vec![
            RawSignal {
                id: "1".into(),
                title: "600519 茅台大涨".into(),
                url: "".into(),
                content: None,
                summary: None,
                published_at: None,
                source: "test".into(),
                source_id: "test".into(),
                category: "A股".into(),
                metrics: None,
            },
            RawSignal {
                id: "2".into(),
                title: "正常标题".into(),
                url: "".into(),
                content: None,
                summary: None,
                published_at: None,
                source: "test".into(),
                source_id: "test".into(),
                category: "AI".into(),
                metrics: None,
            },
        ];
        compliance_filter(&mut signals);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].title, "正常标题");
    }

    #[test]
    fn test_dedup_by_url() {
        let mut signals = vec![
            RawSignal { id: "abc".into(), title: "Article A".into(), url: "".into(), content: None, summary: None, published_at: None, source: "a".into(), source_id: "a".into(), category: "AI".into(), metrics: None },
            RawSignal { id: "abc".into(), title: "Article A".into(), url: "".into(), content: None, summary: None, published_at: None, source: "a".into(), source_id: "a".into(), category: "AI".into(), metrics: None },
        ];
        dedup(&mut signals);
        assert_eq!(signals.len(), 1);
    }

    #[test]
    fn test_title_similarity() {
        let a = title_similarity("hello world", "hello world");
        assert!((a - 1.0).abs() < 0.01);
        let b = title_similarity("hello world", "hello rust");
        assert!(b > 0.3 && b < 0.8);
    }
}
