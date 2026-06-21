//! 渲染模块 — 咨询级交付简报
//!
//! 参考标准：麦肯锡 Action Title → BLUF → Key Variables
//!           高盛 现状 → 催化剂 → 损益冲击 → 风险审计
//!
//! 格式（红蓝模式）：
//! 1. 🎯 今日行动变化（执行摘要）
//! 2. 各问题卡片（Action Title → 核心洞察 → 关键变量 → 压力测试）
//! 3. 📋 今日无关信息
//! 4. 🤖 认知校准
//!
//! 格式（传统模式）：
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
    theses: &[String],
) -> Result<String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let mut md = String::new();

    md.push_str(&format!("# 今日认知更新 — {}\n\n", today));

    if let Some(debate_results) = debate {
        let has_content = debate_results.iter().any(|r| !r.verdict.is_empty());
        if has_content {
            return render_debate_mode(md, debate_results, calibration, theses);
        }
    }

    render_normal_mode(md, analysis, calibration)
}

/// 咨询级交付模式（麦肯锡 Action Title → BLUF → Key Variables）
fn render_debate_mode(
    mut md: String,
    debate_results: &[ArbitrationResult],
    calibration: Option<&str>,
    _theses: &[String],
) -> Result<String> {
    let mut all_articles: Vec<&AnalyzedArticle> = Vec::new();
    for result in debate_results {
        for article in &result.analysis.articles {
            all_articles.push(article);
        }
    }
    all_articles.sort_by_key(|a| Reverse(a.importance));

    if all_articles.is_empty() {
        md.push_str("> 今日无新证据。\n\n");
        return render_footer(md, calibration);
    }

    // 按 question_id 分组
    use std::collections::BTreeMap;
    let mut by_question: BTreeMap<String, Vec<&AnalyzedArticle>> = BTreeMap::new();
    let mut unmatched: Vec<&AnalyzedArticle> = Vec::new();
    for article in all_articles {
        if article.belief_id.is_empty() {
            unmatched.push(article);
        } else {
            by_question
                .entry(article.belief_id.clone())
                .or_default()
                .push(article);
        }
    }

    // 🎯 今日行动变化（执行摘要）
    let has_challenge = by_question
        .values()
        .flatten()
        .any(|a| a.evidence_type == "challenge");
    md.push_str("## 🎯 今日行动变化\n\n");
    if has_challenge {
        md.push_str("🔴 **需要重新评估** — 部分问题出现挑战性证据\n\n");
    } else {
        md.push_str("🟢 **无需调整** — 继续执行当前策略\n\n");
    }
    md.push_str("---\n\n");

    // 各问题卡片（McKinsey 格式）
    for (qid, articles) in &by_question {
        md.push_str(&format!("### {}\n\n", qid));

        for article in articles {
            // Action Title — 从 judgment 取第一句作为带判断的标题
            let action_title = extract_red_stance(&article.judgment);
            let summary = if article.summary.is_empty() {
                truncate_line(&article.judgment, 50)
            } else {
                article.summary.clone()
            };

            // BLUF + Key Variables 行
            md.push_str(&format!("**{}**\n\n", action_title));
            md.push_str(&format!("💡 **核心洞察**: {}\n\n", summary));

            // 关键变量（效率变动 + 长尾隐患）
            md.push_str("**关键变量**:\n");
            let red_first = extract_red_stance(&article.judgment);
            md.push_str(&format!("  ▸ **{}**\n", red_first));
            if !article.blue_rebuttal.is_empty() {
                md.push_str(&format!(
                    "  ▸ **{}**\n",
                    truncate_line(&article.blue_rebuttal, 120)
                ));
            }
            md.push('\n');

            // 压力测试 & 信心等级
            let conf_display = if article.confidence.starts_with('L') {
                article.confidence.clone()
            } else {
                format!("L{}", article.confidence)
            };
            let stress = if article.arbitration.is_empty() {
                format!("建议: {} | 信心: {}", article.action, conf_display)
            } else {
                format!("{} | 信心:{}", article.arbitration, conf_display)
            };
            md.push_str(&format!("**压力测试**: {}\n\n", stress));

            if !article.url.is_empty() {
                md.push_str(&format!("🔗 [原文链接]({})\n\n", article.url));
            }
        }
        md.push_str("---\n\n");
    }

    // 📋 今日无关信息
    if !unmatched.is_empty() {
        md.push_str("### 📋 今日无关信息\n\n");
        for article in &unmatched {
            md.push_str(&format!("• {} — 未回答任何当前问题\n", article.title));
        }
        md.push('\n');
        md.push_str("---\n\n");
    }

    // 认知校准 + 脚注
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
        chrono::Local::now().format("%Y-%m-%d %H:%M"),
    ));
    Ok(md)
}

/// 渲染底部
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

/// 生成 HTML 静态内参页面（Tailwind 样式，麦肯锡级交付格式）
pub fn render_html_report(
    analysis: &[VerticalAnalysis],
    debate: Option<&[ArbitrationResult]>,
    calibration: Option<&str>,
) -> Result<String> {
    let today = Local::now().format("%Y-%m-%d %H:%M").to_string();
    let date_en = Local::now().format("%Y-%m-%d").to_string();
    let mut body = String::new();

    // 收集并排序文章
    let mut all_articles: Vec<&AnalyzedArticle> = Vec::new();
    if let Some(debate_results) = debate {
        for result in debate_results {
            for article in &result.analysis.articles {
                all_articles.push(article);
            }
        }
    } else {
        for va in analysis {
            for article in &va.articles {
                all_articles.push(article);
            }
        }
    }
    all_articles.sort_by_key(|a| Reverse(a.importance));

    let mut core_articles: Vec<&AnalyzedArticle> = Vec::new();
    let mut edge_articles: Vec<&AnalyzedArticle> = Vec::new();
    for a in all_articles {
        if a.importance >= CORE_THRESHOLD {
            core_articles.push(a);
        } else {
            edge_articles.push(a);
        }
    }

    // 信念看板 BLUF
    let total_count = core_articles.len() + edge_articles.len();
    if total_count > 0 {
        body.push_str(&format!(
            r#"<div class="mb-6 bg-slate-900 text-white p-4 rounded-lg">
    <p class="text-xs font-bold tracking-wider uppercase opacity-60">💡 幕僚长今日信念看板</p>
    <p class="text-sm mt-1">今日 {} 条信息匹配到信念系统。</p>
</div>
"#,
            total_count
        ));
    }

    // 核心信号卡片（consulting-grade 格式）
    if !core_articles.is_empty() {
        body.push_str("<div class=\"space-y-6\">\n");
        for article in &core_articles {
            let summary = if article.summary.is_empty() {
                truncate_line(&article.judgment, 50)
            } else {
                article.summary.clone()
            };
            let red_stance = extract_red_stance(&article.judgment);
            let safe_id = article
                .title
                .replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "");

            body.push_str(&format!(r#"<div class="border border-slate-200 bg-white p-5 rounded-lg shadow-sm" id="core-{}">
    <div class="flex items-start justify-between mb-3">
        <h2 class="text-base font-bold text-slate-900 leading-snug">{}</h2>
        <div class="shrink-0 ml-3 flex gap-1">
            {}
            <span class="text-xs font-mono font-bold px-2 py-0.5 rounded {}">{}</span>
        </div>
    </div>
    <div class="mb-3 bg-slate-50 p-3 rounded-lg border-l-4 border-slate-700">
        <span class="text-xs font-bold text-slate-800 tracking-tight block">💡 核心洞察 (BLUF)</span>
        <p class="text-sm text-slate-700 mt-1 leading-relaxed">{}</p>
    </div>
    <div class="mb-3 bg-white p-3 border border-slate-100 rounded-lg space-y-2">
        <span class="text-xs font-bold text-slate-800 tracking-tight block">📊 核心变量压力测试</span>
        <div><div class="flex justify-between text-[11px] text-slate-500 mb-0.5"><span>资金/效率红利</span><span class="font-bold text-emerald-700">{}%</span></div><div class="w-full bg-slate-100 h-1.5 rounded-full overflow-hidden"><div class="bg-emerald-600 h-full rounded-full" style="width:{}%"></div></div></div>
        <div><div class="flex justify-between text-[11px] text-slate-500 mb-0.5"><span>政策/技术共振</span><span class="font-bold text-blue-700">{}%</span></div><div class="w-full bg-slate-100 h-1.5 rounded-full overflow-hidden"><div class="bg-blue-600 h-full rounded-full" style="width:{}%"></div></div></div>
        <div><div class="flex justify-between text-[11px] text-slate-500 mb-0.5"><span>壁垒/长尾风险</span><span class="font-bold text-rose-700">{}%</span></div><div class="w-full bg-slate-100 h-1.5 rounded-full overflow-hidden"><div class="bg-rose-500 h-full rounded-full" style="width:{}%"></div></div></div>
    </div>
    <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 text-xs border-t border-slate-100 pt-3 mt-3">
        <div class="bg-emerald-50 p-3 rounded"><span class="font-bold text-emerald-700">📊 效率变动/资本红利</span><p class="text-emerald-900 mt-1 leading-relaxed">{}</p></div>
        <div class="bg-rose-50 p-3 rounded"><span class="font-bold text-rose-700">📊 长尾隐患/壁垒审计</span><p class="text-rose-900 mt-1 leading-relaxed">{}</p></div>
    </div>
    <div class="mt-3 pt-3 border-t border-dashed border-slate-200">
        <span class="text-xs font-bold text-slate-800">⚖️ 战略执行建议</span>
        <p class="text-xs text-slate-700 mt-1 font-medium">{}</p>
    </div>
    <div class="mt-2 text-xs text-slate-700 font-medium">
        🎯 <span class="font-bold">决策结论</span>: {} 信心:{}
    </div>
</div>
"#,
                safe_id,
                article.title,
                strategic_badge(&article.strategic_level),
                badge_color(&article.confidence),
                article.confidence,
                summary,
                article.capital_score.min(100),
                article.capital_score.min(100),
                article.policy_score.min(100),
                article.policy_score.min(100),
                article.risk_score.min(100),
                article.risk_score.min(100),
                red_stance,
                if article.blue_rebuttal.is_empty() { "蓝军未就此条提出反驳".to_string() } else { article.blue_rebuttal.clone() },
                if article.arbitration.is_empty() { format!("重要性: {}/10 | 建议: {} | 信心: {}", article.importance, article.action, article.confidence) } else { article.arbitration.clone() },
                truncate_line(&article.judgment, 200),
                article.confidence,
            ));
        }
        body.push_str("</div>\n");
    } else {
        body.push_str("<p class=\"text-slate-400 text-sm\">今日无高优先级信号。</p>\n");
    }

    // 折叠附录：低分信号
    if !edge_articles.is_empty() {
        body.push_str(&format!(
            r#"<details class="mt-8 border border-slate-200 bg-white rounded-lg p-4">
    <summary class="text-sm font-medium text-slate-500 cursor-pointer">📦 其他信号 ({} 条)</summary>
    <div class="mt-3 space-y-3">
"#,
            edge_articles.len()
        ));
        for article in &edge_articles {
            let s = if article.summary.is_empty() {
                truncate_line(&article.judgment, 50)
            } else {
                article.summary.clone()
            };
            body.push_str(&format!(
                r#"        <div class="border-b border-slate-100 pb-2 last:border-0">
            <span class="text-xs font-mono font-bold px-1.5 py-0.5 rounded {}">{}</span>
            <span class="text-sm font-medium ml-2">{}</span>
            <p class="text-xs text-slate-500 mt-1">💬 {}</p>
        </div>
"#,
                badge_color(&article.confidence),
                article.confidence,
                article.title,
                s
            ));
        }
        body.push_str("    </div>\n</details>\n");
    }

    // 认知校准
    let calibration_html = if let Some(text) = calibration {
        format!(
            r#"<div class="mt-8 border-l-4 border-slate-300 bg-slate-50 p-4 rounded-r-lg">
    <p class="text-xs font-bold text-slate-400 uppercase tracking-wider mb-1">🤖 认知校准</p>
    <p class="text-sm text-slate-700 italic">{}</p>
</div>
"#,
            text
        )
    } else {
        String::new()
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Sulix Intelligence — 每日内参 {}</title>
<script src="https://cdn.tailwindcss.com">
</script>
</head>
<body class="bg-gray-50 text-slate-900 antialiased">
<div class="max-w-2xl mx-auto px-4 py-8">
    <header class="border-b-2 border-slate-900 pb-4 mb-8 flex items-end justify-between">
        <div>
            <h1 class="text-xl font-bold tracking-tight">SULIX INTELLIGENCE</h1>
            <p class="text-xs text-slate-400 mt-0.5">每日策略内参</p>
        </div>
        <time class="text-xs text-slate-400 font-mono">{}</time>
    </header>

    <h2 class="text-sm font-bold text-slate-500 uppercase tracking-wider mb-4">📌 今日核心信号</h2>
    {}

    {}
</div>
<footer class="max-w-2xl mx-auto px-4 pb-8 text-center">
    <p class="text-xs text-slate-300">由 Sulix Intelligence 自动生成 · Powered by DeepSeek</p>
</footer>
</body>
</html>"#,
        date_en, today, body, calibration_html
    );

    Ok(html)
}

/// 战略等级对应的 badge HTML
fn strategic_badge(level: &str) -> String {
    match level {
        "S" => "<span class=\"text-xs font-mono font-bold px-2 py-0.5 rounded bg-purple-100 text-purple-800\">S</span>".into(),
        "A" => "<span class=\"text-xs font-mono font-bold px-2 py-0.5 rounded bg-blue-100 text-blue-800\">A</span>".into(),
        "B" => "<span class=\"text-xs font-mono font-bold px-2 py-0.5 rounded bg-slate-100 text-slate-600\">B</span>".into(),
        _ => String::new(),
    }
}

/// 信心等级对应的 badge 颜色
fn badge_color(confidence: &str) -> &'static str {
    if confidence.contains('1') || confidence.contains('2') || confidence == "高" {
        "bg-green-100 text-green-800"
    } else if confidence.contains('4') || confidence.contains('5') || confidence == "低" {
        "bg-red-100 text-red-800"
    } else {
        "bg-amber-100 text-amber-800"
    }
}

/// 传统模式（无红蓝）：最重要的 3 件事 → 按分类展开 → 今日结论 → 认知校准
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
                article.action, article.confidence
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

/// 从 judgment 中提取 Action Title（第一句）
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
            strategic_level: String::new(),
            blue_rebuttal: String::new(),
            arbitration: String::new(),
            belief_id: String::new(),
            evidence_type: String::new(),
            capital_score: 0,
            policy_score: 0,
            risk_score: 0,
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
        let result = render_daily_report(&[], None, None, &[]).unwrap();
        assert!(result.contains("今日无新增情报分析"));
        assert!(result.contains("今日认知更新"));
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
        let result = render_daily_report(&[analysis], None, None, &[]).unwrap();
        assert!(result.contains("Article A"));
        assert!(result.contains("Article B"));
        assert!(result.contains("Article C"));
        assert!(result.contains("最重要的 3 件事"));
    }

    #[test]
    fn test_debate_mode_shows_action_change() {
        use crate::agent::orchestrator::ArbitrationResult;
        let mut a = mock_article("Core Signal", 9, "研究");
        a.judgment = "这是一个重要的核心信号。".into();
        a.blue_rebuttal = "蓝军对此提出质疑。".into();
        a.arbitration = "仲裁认为可以采纳。".into();
        a.belief_id = "d1".into();
        a.evidence_type = "support".into();
        let analysis = mock_analysis("AI", vec![a]);
        let debate = ArbitrationResult {
            category: "AI".into(),
            analysis: analysis.clone(),
            verdict: "仲裁结论".into(),
        };
        let result = render_daily_report(&[analysis], Some(&[debate]), None, &[]).unwrap();
        assert!(result.contains("行动变化"));
        assert!(result.contains("Core Signal"));
        assert!(result.contains("蓝军对此提出质疑"));
        // Should NOT contain normal-mode sections
        assert!(!result.contains("最重要的 3 件事"));
        assert!(!result.contains("今日结论"));
    }

    #[test]
    fn test_debate_mode_collapses_low_importance() {
        use crate::agent::orchestrator::ArbitrationResult;
        let a = mock_article("Low Signal", 3, "忽略");
        let analysis = mock_analysis("AI", vec![a]);
        let analysis2 = analysis.clone();
        let debate = ArbitrationResult {
            category: "AI".into(),
            analysis,
            verdict: "无明确评级".into(),
        };
        let result = render_daily_report(&[analysis2], Some(&[debate]), None, &[]).unwrap();
        assert!(result.contains("行动变化"));
        assert!(result.contains("Low Signal"));
    }

    #[test]
    fn test_calibration_section_present() {
        let a = mock_article("Calib Article", 5, "观察");
        let analysis = mock_analysis("AI", vec![a]);
        let result =
            render_daily_report(&[analysis], None, Some("你为什么跳过了所有芯片新闻？"), &[])
                .unwrap();
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
            let result = render_daily_report(&[analysis], None, None, &[]).unwrap();
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
