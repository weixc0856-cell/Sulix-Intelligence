//! 渲染模块 — Markdown 日报生成
//!
//! 将 LLM 分析结果渲染为结构化 Markdown 日报，格式：
//! 1. 今日最重要的 3 件事（按重要性排序）
//! 2. 按分类展开各条分析
//! 3. 今日结论

use anyhow::Result;
use chrono::Local;

use crate::llm::{AnalyzedArticle, VerticalAnalysis};
use crate::agent::orchestrator::ArbitrationResult;

/// 生成最终日报 Markdown
pub fn render_daily_report(
    analysis: &[VerticalAnalysis],
    debate: Option<&[ArbitrationResult]>,
    calibration: Option<&str>,
) -> Result<String> {
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

    // === Phase B: 红蓝对抗概览 ===
    if let Some(debate_results) = debate {
        let has_content = debate_results.iter().any(|r| !r.verdict.is_empty());
        if has_content {
            md.push_str("## 🔴🔵 红蓝对抗\n\n");
            for result in debate_results {
                if result.verdict.is_empty() {
                    continue;
                }
                md.push_str(&format!("### {}\n\n", category_emoji(&result.category)));
                md.push_str(&format!(
                    "**🔴 红军叙事**:\n{}\n\n",
                    result.red_summary
                ));
                md.push_str(&format!(
                    "**🔵 蓝军反驳**:\n{}\n\n",
                    result.blue_summary
                ));
                md.push_str(&format!(
                    "**⚖️ 仲裁结论**:\n> {}\n\n",
                    result.verdict
                ));
                md.push_str("---\n\n");
            }
        }
    }

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

    // === Phase C: 认知校准 ===
    if let Some(text) = calibration {
        if !text.is_empty() {
            md.push_str("────────────────────────────────────────\n\n");
            md.push_str(&format!("🤖 **认知校准**\n\n> {}\n\n", text));
            md.push_str("（不回答也没事，看到就行）\n\n");
            md.push_str("────────────────────────────────────────\n\n");
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{AnalyzedArticle, VerticalAnalysis};

    fn mock_article(title: &str, importance: u8, action: &str) -> AnalyzedArticle {
        AnalyzedArticle {
            title: title.into(),
            url: format!("https://example.com/{}", title),
            importance,
            relevance: "高".into(),
            time_horizon: "短期".into(),
            action: action.into(),
            confidence: "中".into(),
            judgment: format!("关于{}的分析判断", title),
        }
    }

    fn mock_analysis(category: &str, articles: Vec<AnalyzedArticle>) -> VerticalAnalysis {
        VerticalAnalysis {
            category: category.into(),
            articles,
        }
    }

    #[test]
    fn test_empty_analysis() {
        let result = render_daily_report(&[], None, None).unwrap();
        assert!(result.contains("今日无新增情报分析"));
        assert!(result.contains("今日创业情报"));
    }

    #[test]
    fn test_top3_extraction() {
        let articles = vec![
            mock_article("Article A", 10, "研究"),
            mock_article("Article B", 8, "观察"),
            mock_article("Article C", 6, "观察"),
            mock_article("Article D", 3, "忽略"),
        ];
        let analysis = mock_analysis("AI", articles);
        let result = render_daily_report(&[analysis], None, None).unwrap();
        assert!(result.contains("Article A"));
        assert!(result.contains("Article B"));
        assert!(result.contains("Article C"));
        // D is filtered from top3 but still rendered in category section
        assert!(result.contains("今日最重要的 3 件事"));
    }

    #[test]
    fn test_debate_section_present() {
        use crate::agent::orchestrator::ArbitrationResult;
        let a = mock_article("Debate Article", 7, "研究");
        let analysis = mock_analysis("AI", vec![a]);
        let debate = ArbitrationResult {
            category: "AI".into(),
            analysis: analysis.clone(),
            verdict: "仲裁结论：各有依据".into(),
            red_summary: "红军认为有机会".into(),
            blue_summary: "蓝军认为有风险".into(),
        };
        let result = render_daily_report(&[analysis], Some(&[debate]), None).unwrap();
        assert!(result.contains("红蓝对抗"));
        assert!(result.contains("红军认为有机会"));
        assert!(result.contains("蓝军认为有风险"));
    }

    #[test]
    fn test_calibration_section_present() {
        let a = mock_article("Calib Article", 5, "观察");
        let analysis = mock_analysis("AI", vec![a]);
        let result = render_daily_report(&[analysis], None, Some("你为什么跳过了所有芯片新闻？")).unwrap();
        assert!(result.contains("认知校准"));
        assert!(result.contains("你为什么跳过了所有芯片新闻？"));
    }

    #[test]
    fn test_category_emoji_all() {
        let categories = ["AI", "Agent", "独立开发", "Indie", "芯片", "政策", "财税", "创业", "出海", "其他"];
        for cat in &categories {
            let a = mock_article("Test", 5, "观察");
            let analysis = mock_analysis(cat, vec![a]);
            let result = render_daily_report(&[analysis], None, None).unwrap();
            assert!(!result.is_empty(), "Category {} should render", cat);
        }
    }

    #[test]
    fn test_truncate_line_short() {
        assert_eq!(truncate_line("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_line_long() {
        let result = truncate_line("hello world this is a long text", 10);
        assert!(result.len() <= 13);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_extract_top3_excludes_low_importance() {
        let articles = vec![
            mock_article("Low", 2, "忽略"),
            mock_article("High", 9, "研究"),
        ];
        let analysis = mock_analysis("AI", articles);
        let analyses = [analysis];  // bind to extend lifetime
        let top3 = extract_top3(&analyses);
        assert_eq!(top3.len(), 1);
        assert_eq!(top3[0].title, "High");
    }
}
