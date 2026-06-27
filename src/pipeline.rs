//! Pipeline 中间件链（海外版 — 合规基础过滤 + 实体标记）
//!
//! 1. Sanitize — strip HTML/URLs/邮箱
//! 2. Entity Classify — 识别关键实体（TSMC/ASML/Sanctions 等）做标签
//! 3. Dedup — URL hash + 标题 Jaccard
//! 4. Post-process — 排序 + URL 修复

use std::collections::HashSet;

use anyhow::Result;
use regex::Regex;
use serde::Serialize;

use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::source::RawSignal;

/// 运行管线（使用配置的去重阈值）
pub fn run_pipeline_with_config(
    signals: &mut Vec<RawSignal>,
    dedup_config: Option<&crate::config::DedupConfig>,
) -> Result<()> {
    let threshold = dedup_config
        .map(|c| c.title_similarity_threshold)
        .unwrap_or(0.75);
    sanitize_all(signals);
    // 合规过滤（A 股代码等）仅在展示时应用，不在持久化前执行，
    // 避免因误报（如匹配美国邮编 60601）导致数据不可逆丢失。
    // 展示端在 HTML 渲染时单独调用 compliance_filter。
    dedup(signals, threshold);
    post_process(signals);
    Ok(())
}

// ===== Phase 3: 离线证据快照（ArchiveBox 式不可变证据日志）=====

/// 证据条目（按事件追加到 JSONL）
#[derive(Debug, Clone, Serialize)]
pub struct EvidenceSnapshot {
    /// UUID v7 格式凭证 ID
    pub id: String,
    /// 信号源名称
    pub source: String,
    /// 原文标题
    pub title: String,
    /// 原文 URL
    pub url: String,
    /// 抓取时的原始内容
    pub raw_content: Option<String>,
    /// 触发事件快照的 SVI 值
    pub svi: u8,
    /// 所属主题（主题分析后填充）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_title: Option<String>,
    /// 分析结论（主题分析后填充）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bluf: Option<String>,
    /// 时间戳
    pub captured_at: String,
}

/// 触发离线证据快照（SVI >= 7 时自动调用）
///
/// Expert Refinement (ArchiveBox 法务防蒸发):
/// 当 SVI >= 7，将原始信号作为不可变 JSONL 证据日志写入 evidence/ 目录。
/// 即使现实世界源被删/改，付费会员仍可在后台调用离线铁证。
pub fn capture_evidence_snapshot(signal: &RawSignal, svi: u8, vault_path: &str) -> Result<()> {
    let evidence_dir = std::path::Path::new(vault_path).join("evidence");
    std::fs::create_dir_all(&evidence_dir)?;

    let snapshot = EvidenceSnapshot {
        id: format!("ev-{}", chrono::Utc::now().timestamp()),
        source: signal.source.clone(),
        title: signal.title.clone(),
        url: signal.url.clone(),
        raw_content: signal.content.clone(),
        svi,
        theme_title: None,
        bluf: None,
        captured_at: chrono::Local::now().to_rfc3339(),
    };

    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let log_path = evidence_dir.join(format!("{}.jsonl", date));

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let line = serde_json::to_string(&snapshot)?;
    use std::io::Write;
    writeln!(file, "{}", line)?;

    Ok(())
}

/// 为主题文章创建可回溯的证据快照（附带分析上下文）
///
/// 在主题分析完成后调用，每篇主题文章 + 分析结论一并记入证据链。
pub fn capture_topic_evidence(
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    vault_path: &str,
) -> Result<()> {
    let evidence_dir = std::path::Path::new(vault_path).join("evidence");
    std::fs::create_dir_all(&evidence_dir)?;
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let log_path = evidence_dir.join(format!("{}.jsonl", date));

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    use std::io::Write;

    for (theme, analysis) in themes.iter().zip(analyses.iter()) {
        for article in &theme.articles {
            let snapshot = EvidenceSnapshot {
                id: format!("ev-topic-{}", chrono::Utc::now().timestamp()),
                source: article.source.clone(),
                title: article.title.clone(),
                url: article.url.clone(),
                raw_content: article.content.clone().or_else(|| article.summary.clone()),
                svi: analysis.signal_strength,
                theme_title: Some(theme.title.clone()),
                bluf: Some(analysis.bluf.clone()),
                captured_at: chrono::Local::now().to_rfc3339(),
            };
            let line = serde_json::to_string(&snapshot)?;
            writeln!(file, "{}", line)?;
        }
    }

    Ok(())
}

fn sanitize_all(signals: &mut [RawSignal]) {
    let url_re = Regex::new(r"https?://\S+").expect("valid URL regex");
    let email_re = Regex::new(r"\S+@\S+\.\S+").expect("valid email regex");

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
    .expect("valid strip_tags regex");
    let no_strip = strip_tags.replace_all(html, "");
    // 2. 保留的标签只保留标签本身，不剥离内部文本
    // 移除不在保留列表中的所有其他标签
    let all_tag = Regex::new(r"</?(\w+)[^>]*>").expect("valid HTML tag regex");
    let result = all_tag.replace_all(&no_strip, |caps: &regex::Captures| {
        let tag = caps.get(1).map(|m| m.as_str()).unwrap_or("");
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
            caps.get(0).expect("capture group 0 always exists on match").as_str().to_string()
        } else {
            String::new()
        }
    });
    result.trim().to_string()
}

fn dedup(signals: &mut Vec<RawSignal>, threshold: f64) {
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut seen_titles: Vec<String> = Vec::new();
    signals.retain(|signal| {
        if !seen_urls.insert(signal.id.clone()) {
            return false;
        }
        let title_lower = signal.title.to_lowercase();
        for existing in &seen_titles {
            if title_similarity(existing, &title_lower) > threshold {
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
                is_internal: false,
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
                is_internal: false,
            },
        ];
        dedup(&mut signals, 0.75);
        assert_eq!(signals.len(), 1);
    }
}
