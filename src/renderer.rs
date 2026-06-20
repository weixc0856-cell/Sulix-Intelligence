//! 渲染模块 — Markdown 日报生成
//!
//! 将 LLM 分析结果渲染为结构化 Markdown 日报，格式：
//! 1. 今日最重要的 3 件事（按重要性排序）
//! 2. 按分类展开各条分析
//! 3. 今日结论

use anyhow::Result;
use chrono::Local;

use crate::llm::{AnalyzedArticle, VerticalAnalysis};

/// 生成最终日报 Markdown
pub fn render_daily_report(analysis: &[VerticalAnalysis]) -> Result<String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let mut md = String::new();

    // 标题
    md.push_str(&format!("# 今日创业情报 — {}\n\n", today));

    // === 最重要的 3 件事 ===
    md.push_str("## 📌 今日最重要的 3 件事\n\n");

    let top3 = extract_top3(analysis);
    if top3.is_empty() {
        md.push_str("> 今日无新增情报分析。\n\n");
    } else {
        for (i, article) in top3.iter().enumerate() {
            md.push_str(&format!(
                "{}. **{}** — 重要性:{}/10 | 建议:{} | 信心:{}\n",
                i + 1,
                article.title,
                article.importance,
                article.action,
                article.confidence,
            ));
            if !article.judgment.is_empty() {
                let brief = truncate_line(&article.judgment, 80);
                md.push_str(&format!("   > {}\n", brief));
            }
            md.push('\n');
        }
    }

    md.push_str("---\n\n");

    // === 按分类展开 ===
    for va in analysis {
        if va.articles.is_empty() {
            continue;
        }

        md.push_str(&format!("## {}\n\n", category_emoji(&va.category)));

        // 按重要性降序排列
        let mut sorted = va.articles.clone();
        sorted.sort_by(|a, b| b.importance.cmp(&a.importance));

        for article in &sorted {
            md.push_str(&format!("### {}\n\n", article.title));

            // 元信息行
            md.push_str(&format!(
                "**重要性**: {}/10 | **相关性**: {} | **时间跨度**: {}  \n",
                article.importance, article.relevance, article.time_horizon,
            ));
            md.push_str(&format!(
                "**建议动作**: {} | **信心等级**: {}  \n\n",
                article.action, article.confidence,
            ));

            // 判断
            if !article.judgment.is_empty() {
                md.push_str(&format!("**判断**:\n{}\n\n", article.judgment));
            }

            // 原文链接
            if !article.url.is_empty() {
                md.push_str(&format!("🔗 [原文链接]({})\n\n", article.url));
            }

            md.push_str("---\n\n");
        }
    }

    // === 今日结论 ===
    md.push_str("## 💡 今日结论\n\n");
    if top3.is_empty() {
        md.push_str("> 今日无重要情报。\n");
    } else {
        md.push_str("> 今天最重要的信号是：\n");
        for article in &top3 {
            let brief = truncate_line(&article.judgment, 100);
            md.push_str(&format!("> - **{}** — {}\n", article.title, brief));
        }
        md.push('\n');
    }

    // 脚注
    md.push_str("---\n\n");
    md.push_str(&format!(
        "*由 Sulix Intelligence 自动生成于 {}. Powered by DeepSeek.*\n",
        Local::now().format("%Y-%m-%d %H:%M"),
    ));

    Ok(md)
}

/// 从所有分析结果中提取最重要的 3 条（按 importance 降序）
fn extract_top3(analysis: &[VerticalAnalysis]) -> Vec<&AnalyzedArticle> {
    let mut all: Vec<&AnalyzedArticle> = analysis
        .iter()
        .flat_map(|va| va.articles.iter())
        .filter(|a| !a.action.contains("忽略") && a.importance >= 4)
        .collect();

    all.sort_by(|a, b| b.importance.cmp(&a.importance));
    all.into_iter().take(3).collect()
}

/// 分类对应的 emoji
fn category_emoji(category: &str) -> String {
    match category {
        c if c.contains("AI") || c.contains("Agent") => "🤖 AI & Agent".into(),
        c if c.contains("独立") || c.contains("Indie") => "💻 独立开发".into(),
        c if c.contains("芯片") => "🔬 芯片 & 硬件".into(),
        c if c.contains("政策") => "🏛️ 政策 & 法规".into(),
        c if c.contains("财税") => "💰 财税".into(),
        c if c.contains("创业") => "🚀 创业 & 融资".into(),
        c if c.contains("出海") => "🌍 出海".into(),
        _ => format!("📋 {}", category),
    }
}

/// 截断一行文本到指定长度，末尾加省略号
fn truncate_line(text: &str, max_len: usize) -> String {
    let line = text.lines().next().unwrap_or(text);
    if line.len() > max_len {
        format!("{}...", &line[..max_len])
    } else {
        line.to_string()
    }
}
