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

fn sanitize_all(signals: &mut Vec<RawSignal>) {
    let url_re = Regex::new(r"https?://\S+").unwrap();
    let email_re = Regex::new(r"\S+@\S+\.\S+").unwrap();

    for signal in signals.iter_mut() {
        signal.title = sanitize_text(&signal.title, &url_re, &email_re);
        if let Some(content) = &signal.content {
            let cleaned = sanitize_text(content, &url_re, &email_re);
            signal.content = if cleaned.is_empty() { None } else { Some(cleaned) };
        }
        if let Some(summary) = &signal.summary {
            let cleaned = sanitize_text(summary, &url_re, &email_re);
            signal.summary = if cleaned.is_empty() { None } else { Some(cleaned) };
        }
    }
}

fn sanitize_text(text: &str, url_re: &Regex, email_re: &Regex) -> String {
    let no_html = strip_html_tags(text);
    let no_urls = url_re.replace_all(&no_html, "");
    let no_emails = email_re.replace_all(&no_urls, "");
    let collapsed: Vec<&str> = no_emails.split_whitespace().collect();
    let joined = collapsed.join(" ");
    if joined.len() > 3000 {
        let end = joined.floor_char_boundary(3000);
        format!("{}...", &joined[..end])
    } else {
        joined
    }
}

fn strip_html_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch { '<' => in_tag = true, '>' if in_tag => in_tag = false, _ if !in_tag => result.push(ch), _ => {} }
    }
    result.trim().to_string()
}

fn dedup(signals: &mut Vec<RawSignal>) {
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut seen_titles: Vec<String> = Vec::new();
    signals.retain(|signal| {
        if !seen_urls.insert(signal.id.clone()) { return false; }
        let title_lower = signal.title.to_lowercase();
        for existing in &seen_titles {
            if title_similarity(existing, &title_lower) > 0.75 { return false; }
        }
        seen_titles.push(signal.title.clone());
        true
    });
}

fn title_similarity(a: &str, b: &str) -> f64 {
    let words_a: HashSet<&str> = a.split_whitespace().collect();
    let words_b: HashSet<&str> = b.split_whitespace().collect();
    if words_a.is_empty() && words_b.is_empty() { return 1.0; }
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    intersection as f64 / union as f64
}

fn post_process(signals: &mut Vec<RawSignal>) {
    signals.sort_by(|a, b| {
        let a_naive = a.published_at.map(|d| d.naive_utc()).unwrap_or(chrono::NaiveDateTime::MIN);
        let b_naive = b.published_at.map(|d| d.naive_utc()).unwrap_or(chrono::NaiveDateTime::MIN);
        b_naive.cmp(&a_naive)
    });
    for signal in signals.iter_mut() {
        if signal.url.starts_with("//") { signal.url = format!("https:{}", signal.url); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html_tags("<p>Hello <b>world</b></p>"), "Hello world");
    }

    #[test]
    fn test_dedup_by_url() {
        let mut signals = vec![
            RawSignal { id: "abc".into(), title: "A".into(), url: "".into(), content: None, summary: None, published_at: None, source: "a".into(), source_id: "a".into(), category: "AI".into(), metrics: None, requires_sanitization: false },
            RawSignal { id: "abc".into(), title: "A".into(), url: "".into(), content: None, summary: None, published_at: None, source: "a".into(), source_id: "a".into(), category: "AI".into(), metrics: None, requires_sanitization: false },
        ];
        dedup(&mut signals);
        assert_eq!(signals.len(), 1);
    }
}
