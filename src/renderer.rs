//! 渲染模块 — Markdown 日报生成
//!
//! 格式（红蓝模式）：
//! 1. 今日核心信号（importance >= 6 的卡片，包含 红军/蓝军/仲裁 三行）
//! 2. 📦 其他信号（importance < 6 折叠）
//! 3. 认知校准，无"最重要的 3 件事"和"今日结论"——卡片本身就是结论，不做复读。
//!
//! 格式（传统模式，无红蓝）：
//! 1. 最重要的 3 件事
//! 2. 按分类展开
//! 3. 今日结论
//! 4. 认知校准

use std::cmp::Reverse;

use anyhow::Result;
use chrono::Local;

use crate::agent::orchestrator::ArbitrationResult;
use crate::llm::{AnalyzedArticle, VerticalAnalysis};

/// 核心信号最低重要性阈值（低于此值进入折叠附录）
const CORE_THRESHOLD: u8 = 6;

/// 生成最终日报 Markdown
pub fn render_daily_report(
    analysis: &[VerticalAnalysis],
    debate: Option<&[ArbitrationResult]>,
    calibration: Option<&str>,
) -> Result<String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let mut md = String::new();

    md.push_str(&format!("# 今日创业情报 — {}\n\n", today));

    if let Some(debate_results) = debate {
        let has_content = debate_results.iter().any(|r| !r.verdict.is_empty());
        if has_content {
            return render_debate_mode(md, debate_results, calibration);
        }
    }

    render_normal_mode(md, analysis, calibration)
}

/// 红蓝模式：核心信号卡片 → 折叠附录 → 认知校准
fn render_debate_mode(
    mut md: String,
    debate_results: &[ArbitrationResult],
    calibration: Option<&str>,
) -> Result<String> {
    // 收集所有文章，按重要性降序排列
    let mut all_articles: Vec<(&str, &AnalyzedArticle)> = Vec::new();
    for result in debate_results {
        for article in &result.analysis.articles {
            all_articles.push((result.verdict.as_str(), article));
        }
    }
    all_articles.sort_by_key(|(_, a)| Reverse(a.importance));

    // 分拆核心信号和边缘信号
    let mut core_articles: Vec<(&str, &AnalyzedArticle)> = Vec::new();
    let mut edge_articles: Vec<(&str, &AnalyzedArticle)> = Vec::new();
    for (verdict, article) in all_articles {
        if article.importance < CORE_THRESHOLD {
            edge_articles.push((verdict, article));
        } else {
            core_articles.push((verdict, article));
        }
    }

    // === 今日核心信号 ===
    if !core_articles.is_empty() {
        md.push_str("## 📌 今日核心信号\n\n");
        for (_, article) in &core_articles {
            // 💬 summary（防呆：为空时用 judgment 前 50 字替代）
            let summary = if article.summary.is_empty() {
                truncate_line(&article.judgment, 50)
            } else {
                article.summary.clone()
            };
            // 🔴 红军立场（从 judgment 取第一句，防崩：太长或为空时整体截断）
            let red_stance = extract_red_stance(&article.judgment);

            md.push_str(&format!(
                "**{}** — 重要性:{}/10 | 信心:{}\n\n",
                article.title, article.importance, article.confidence
            ));
            md.push_str(&format!("💬 {}\n\n", summary));
            md.push_str(&format!("🔴 **红军**: {}\n\n", red_stance));
            if !article.blue_rebuttal.is_empty() {
                md.push_str(&format!("🔵 **蓝军**: {}\n\n", article.blue_rebuttal));
            }
            if !article.arbitration.is_empty() {
                md.push_str(&format!("⚖️ **仲裁**: {}\n\n", article.arbitration));
            }
            if !article.url.is_empty() {
                md.push_str(&format!("🔗 [原文链接]({})\n\n", article.url));
            }
            md.push_str("---\n\n");
        }
    } else {
        md.push_str("> 今日无高优先级信号。\n\n");
    }

    // === 折叠附录：低分信号（统一一个 <details>，不套娃） ===
    if !edge_articles.is_empty() {
        md.push_str(&format!(
            "<details>\n<summary>📦 其他信号 ({} 条)</summary>\n\n",
            edge_articles.len()
        ));
        for (_, article) in &edge_articles {
            let s = if article.summary.is_empty() {
                truncate_line(&article.judgment, 50)
            } else {
                article.summary.clone()
            };
            md.push_str(&format!(
                "**{}** — {}/10 | 信心:{}\n\n💬 {}\n\n---\n\n",
                article.title, article.importance, article.confidence, s,
            ));
        }
        md.push_str("</details>\n\n");
    }

    render_footer(md, calibration)
}

/// 传统模式（无红蓝）：最重要的 3 件事 → 核心信号 → 折叠低分 → 今日结论 → 认知校准
fn render_normal_mode(
    mut md: String,
    analysis: &[VerticalAnalysis],
    calibration: Option<&str>,
) -> Result<String> {
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
                let brief = truncate_line(&article.judgment, 120);
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

        let mut sorted = va.articles.clone();
        sorted.sort_by_key(|b| Reverse(b.importance));

        let mut high_p = Vec::new();
        let mut low_p = Vec::new();
        for a in &sorted {
            if a.importance >= CORE_THRESHOLD {
                high_p.push(a);
            } else {
                low_p.push(a);
            }
        }

        for article in &high_p {
            md.push_str(&format!("### {}\n\n", article.title));
            md.push_str(&format!(
                "**重要性**: {}/10 | **相关性**: {} | **时间跨度**: {}  \n",
                article.importance, article.relevance, article.time_horizon,
            ));
            md.push_str(&format!(
                "**建议动作**: {} | **信心等级**: {}  \n\n",
                article.action, article.confidence,
            ));
            if !article.judgment.is_empty() {
                md.push_str(&format!("**判断**:\n{}\n\n", article.judgment));
            }
            if !article.url.is_empty() {
                md.push_str(&format!("🔗 [原文链接]({})\n\n", article.url));
            }
            md.push_str("---\n\n");
        }

        if !low_p.is_empty() {
            md.push_str(&format!(
                "<details>\n<summary>📎 低优先级 ({})</summary>\n\n",
                low_p.len()
            ));
            for article in &low_p {
                md.push_str(&format!(
                    "**{}** — {}/10\n\n> {}\n\n---\n\n",
                    article.title, article.importance, article.judgment
                ));
            }
            md.push_str("</details>\n\n");
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

    render_footer(md, calibration)
}

/// 渲染底部：认知校准 + 脚注
fn render_footer(mut md: String, calibration: Option<&str>) -> Result<String> {
    if let Some(text) = calibration {
        if !text.is_empty() {
            md.push_str("────────────────────────────────────────\n\n");
            md.push_str(&format!("🤖 **认知校准**\n\n> {}\n\n", text));
            md.push_str("（不回答也没事，看到就行）\n\n");
            md.push_str("────────────────────────────────────────\n\n");
        }
    }

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

    all.sort_by_key(|b| Reverse(b.importance));
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

/// 截断一行文本到指定长度，末尾加省略号（UTF-8 安全）
fn truncate_line(text: &str, max_len: usize) -> String {
    let line = text.lines().next().unwrap_or(text);
    if line.len() > max_len {
        let end = line.floor_char_boundary(max_len);
        format!("{}...", &line[..end])
    } else {
        line.to_string()
    }
}

/// 从 judgment 中提取红军立场第一句
/// 防崩：如果第一句太长（>80字）或为空，整体截断
fn extract_red_stance(judgment: &str) -> String {
    let first = judgment
        .split(['。', '\n', '.'])
        .next()
        .unwrap_or("")
        .trim();
    if first.is_empty() || first.chars().count() > 80 {
        format!("{}...", judgment.chars().take(75).collect::<String>())
    } else {
        first.to_string()
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
            summary: String::new(),
            blue_rebuttal: String::new(),
            arbitration: String::new(),
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
    fn test_normal_mode_top3() {
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
        assert!(result.contains("最重要的 3 件事"));
    }

    #[test]
    fn test_debate_mode_shows_core_signals() {
        use crate::agent::orchestrator::ArbitrationResult;
        let mut a = mock_article("Core Signal", 9, "研究");
        a.judgment = "这是一个重要的核心信号。".into();
        a.blue_rebuttal = "蓝军对此提出质疑。".into();
        a.arbitration = "仲裁认为可以采纳。".into();
        let analysis = mock_analysis("AI", vec![a]);
        let debate = ArbitrationResult {
            category: "AI".into(),
            analysis: analysis.clone(),
            verdict: "仲裁结论".into(),
        };
        let result = render_daily_report(&[analysis], Some(&[debate]), None).unwrap();
        assert!(result.contains("今日核心信号"));
        assert!(result.contains("核心信号"));
        assert!(result.contains("蓝军对此提出质疑"));
        assert!(result.contains("仲裁认为可以采纳"));
        // Should NOT contain normal-mode sections
        assert!(!result.contains("最重要的 3 件事"));
        assert!(!result.contains("今日结论"));
    }

    #[test]
    fn test_debate_mode_collapses_low_importance() {
        use crate::agent::orchestrator::ArbitrationResult;
        let a = mock_article("Low Signal", 3, "忽略");
        let analysis = mock_analysis("AI", vec![a]);
        let debate = ArbitrationResult {
            category: "AI".into(),
            analysis: analysis,
            verdict: "无明确评级".into(),
        };
        let result = render_daily_report(&[], Some(&[debate]), None).unwrap();
        assert!(result.contains("其他信号"));
        assert!(!result.contains("今日核心信号")); // All articles < 6, so no core section
    }

    #[test]
    fn test_calibration_section_present() {
        let a = mock_article("Calib Article", 5, "观察");
        let analysis = mock_analysis("AI", vec![a]);
        let result =
            render_daily_report(&[analysis], None, Some("你为什么跳过了所有芯片新闻？")).unwrap();
        assert!(result.contains("认知校准"));
        assert!(result.contains("你为什么跳过了所有芯片新闻？"));
    }

    #[test]
    fn test_category_emoji_all() {
        let categories = [
            "AI",
            "Agent",
            "独立开发",
            "Indie",
            "芯片",
            "政策",
            "财税",
            "创业",
            "出海",
            "其他",
        ];
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
        let analyses = [analysis];
        let top3 = extract_top3(&analyses);
        assert_eq!(top3.len(), 1);
        assert_eq!(top3[0].title, "High");
    }
}
