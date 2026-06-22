//! 渲染模块 — 咨询级简报
//!
//! 抄 Reference/ 中 BCG/Deloitte/GS/McKinsey 报告结构
//! 所有输出数据集中到 TemplateData，由 template::render() 渲染

use std::collections::HashMap;

use anyhow::Result;
use chrono::Local;

use crate::clusterer::{Assumption, Theme, ThemeAnalysis, Summary};
use crate::fetcher::Article;
use crate::template::{self, TemplateData};

/// 渲染战略分析报告
pub fn render_analysis_report(
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    summary: &Summary,
    calibration: Option<&str>,
    watchlist: Option<&[Article]>,
    source_statuses: &[(String, bool, usize)],
) -> Result<String> {
    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M:%S").to_string();

    // 构建各内容块
    let executive_summary = build_executive_summary(analyses);
    let topic_sections = build_topic_sections(themes, analyses);
    let synthesis = build_synthesis(analyses);
    let decision_required = build_decision_required(analyses);
    let watchlist_block = build_watchlist_block(watchlist);
    let calibration_block = build_calibration_block(calibration);
    let source_index = build_source_index(themes, analyses);

    // Processing Status 区块
    let processing_status = build_processing_status(source_statuses);

    // YAML frontmatter metrics
    let mut metrics = HashMap::new();
    let total_articles: usize = themes.iter().map(|t| t.articles.len()).sum();
    metrics.insert("total_articles".into(), total_articles.to_string());
    metrics.insert("total_topics".into(), analyses.len().to_string());
    metrics.insert("total_watchlist".into(), watchlist.map(|w| w.len()).unwrap_or(0).to_string());
    if let Some(highest) = analyses.iter().max_by_key(|a| a.signal_strength) {
        metrics.insert("max_signal_strength".into(), highest.signal_strength.to_string());
    }
    // 蓝军风险审计信号
    let has_adverse = analyses.iter().any(|a| a.adverse.as_ref().map(|x| !x.scenario.is_empty()).unwrap_or(false));
    metrics.insert("risk_audit_passed".into(), if has_adverse { "false".into() } else { "true".into() });

    let data = TemplateData {
        date,
        time: time.clone(),
        topic_count: analyses.len(),
        article_count: total_articles,
        processing_time: time,
        executive_summary,
        topic_sections,
        synthesis,
        decision_required,
        watchlist: watchlist_block,
        calibration: calibration_block,
        source_index,
        processing_status,
        metrics,
    };

    Ok(template::render(template::ANALYSIS_TEMPLATE, &data))
}

/// 渲染每日信号聚合
pub fn render_signal_aggregation(
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    watchlist: Option<&[Article]>,
) -> Result<String> {
    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M:%S").to_string();

    let mut topic_sections = String::new();
    for (theme, analysis) in themes.iter().zip(analyses.iter()) {
        if theme.articles.is_empty() { continue; }

        topic_sections.push_str(&format!("## {}\n\n### 关键动态\n\n", theme.title));

        let best_url = theme.articles.iter().find(|a| !a.url.is_empty()).map(|a| a.url.as_str()).unwrap_or("");
        for article in &theme.articles {
            let summary = article.summary.as_deref()
                .or(article.content.as_deref())
                .unwrap_or("");
            let end = summary.floor_char_boundary(120);
            let snippet = &summary[..end];
            let url = if !article.url.is_empty() { &article.url } else { best_url };
            topic_sections.push_str(&format!("- **{}**: {}", article.title, snippet));
            if !url.is_empty() {
                topic_sections.push_str(&format!(" [{}]({})", article.source, url));
            }
            topic_sections.push('\n');
        }
        topic_sections.push('\n');

        // 分析与背景
        topic_sections.push_str("### 分析与背景\n\n");
        if !analysis.analysis_paragraph.is_empty() {
            topic_sections.push_str(&analysis.analysis_paragraph);
        } else {
            topic_sections.push_str(&analysis.impact);
        }
        topic_sections.push_str("\n\n---\n\n");
    }

    let watchlist_block = build_watchlist_block(watchlist);

    // TOC
    let toc: String = themes.iter()
        .map(|t| format!("- [{}](#{})", t.title, t.title.to_lowercase().replace(' ', "-")))
        .collect::<Vec<_>>()
        .join("\n");

    let mut metrics = HashMap::new();
    metrics.insert("total_articles".into(), themes.iter().map(|t| t.articles.len()).sum::<usize>().to_string());

    let data = TemplateData {
        date,
        time: time.clone(),
        topic_count: themes.len(),
        article_count: themes.iter().map(|t| t.articles.len()).sum(),
        processing_time: time,
        executive_summary: String::new(),
        topic_sections: format!("{}\n\n{}", toc, topic_sections),
        synthesis: String::new(),
        decision_required: String::new(),
        watchlist: watchlist_block,
        calibration: String::new(),
        source_index: String::new(),
        processing_status: String::new(),
        metrics,
    };

    Ok(template::render(template::AGGREGATION_TEMPLATE, &data))
}

// ===== 内容块构建函数 =====

fn build_executive_summary(analyses: &[ThemeAnalysis]) -> String {
    if analyses.is_empty() {
        return "> 今日无聚类主题。所有信号均为孤立事件，不足以形成判断。\n\n".into();
    }
    let mut md = String::new();
    for (i, a) in analyses.iter().enumerate() {
        md.push_str(&format!(
            "{}. **{}** — {}（{} 条证据）\n",
            i + 1, a.bluf, a.impact, a.fact_base.len(),
        ));
    }
    md.push('\n');
    md
}

fn build_topic_sections(themes: &[Theme], analyses: &[ThemeAnalysis]) -> String {
    let mut md = String::new();
    for a in analyses {
        md.push_str(&format!("## 主题: {}\n\n", a.theme_title));

        // Fact Base
        if !a.fact_base.is_empty() {
            md.push_str("| 证据 | 解读 | 置信度 |\n|------|------|--------|\n");
            for fb in &a.fact_base {
                md.push_str(&format!("| {} | {} | {} |\n", fb.evidence, fb.interpretation, fb.confidence));
            }
            md.push('\n');
        }

        // 信号强度
        md.push_str(&format!("**信号强度**: {}/10 — ", a.signal_strength));
        md.push_str(match a.signal_strength {
            7..=10 => "行业机制级\n\n",
            5..=6 => "竞争格局级\n\n",
            _ => "单点事件级\n\n",
        });
        md.push_str(&format!("**影响**: {}\n\n", a.impact));

        // 承重假设
        let load_bearing: Vec<&Assumption> = a.assumptions.iter().filter(|a| a.load_bearing).collect();
        if !load_bearing.is_empty() {
            md.push_str("**承重假设**:\n");
            for asm in &load_bearing {
                md.push_str(&format!("- {}（证据强度: {}）\n", asm.text, asm.evidence_strength));
            }
            md.push('\n');
        }

        // 逆境情景
        if let Some(ref adv) = a.adverse {
            if !adv.scenario.is_empty() {
                md.push_str(&format!("**逆境情景**: {}。\n", adv.scenario));
                md.push_str(&format!("**早期预警**: {}\n\n", adv.early_warning));
            }
        }

        // 待验证
        if !a.next_tests.is_empty() {
            md.push_str("**待验证**:\n");
            for t in &a.next_tests { md.push_str(&format!("- {}\n", t)); }
            md.push('\n');
        }

        // 待回答的问题
        if !a.open_questions.is_empty() {
            md.push_str("**待回答的问题**:\n");
            for q in &a.open_questions { md.push_str(&format!("- {}\n", q)); }
            md.push('\n');
        }

        // 关联
        if !a.connections.is_empty() {
            md.push_str(&format!("**关联**: {}\n\n", a.connections.join(" → ")));
        }

        // 溯源
        if !a.source_urls.is_empty() {
            md.push_str("**溯源**:\n");
            for url in &a.source_urls { md.push_str(&format!("- {}\n", url)); }
            md.push('\n');
        }

        // 质量
        let source_count = a.source_urls.len();
        let assumption_count = a.assumptions.len();
        let has_adverse = a.adverse.as_ref().map(|x| !x.scenario.is_empty()).unwrap_or(false);
        md.push_str(&format!(
            "**质量**: {} 来源 | {} 条承重假设 | {} | {} 项待验证\n",
            source_count, assumption_count,
            if has_adverse { "1 个逆境情景" } else { "无逆境情景" },
            a.next_tests.len(),
        ));

        md.push_str("---\n\n");
    }
    md
}

fn build_synthesis(analyses: &[ThemeAnalysis]) -> String {
    if analyses.is_empty() {
        return "## 综合判断\n\n**结论**: 今日无足够信号形成综合判断。\n\n".into();
    }
    let mut md = String::from("## 综合判断\n\n");
    if let Some(highest) = analyses.iter().max_by_key(|a| a.signal_strength) {
        md.push_str(&format!("**结论**: {}。\n\n", highest.bluf));
    } else if let Some(first) = analyses.first() {
        md.push_str(&format!("**结论**: {}。\n\n", first.bluf));
    }

    // 关键证据
    let key_evidence: Vec<String> = analyses.iter()
        .flat_map(|a| a.fact_base.iter())
        .filter(|fb| fb.confidence.starts_with("确立"))
        .take(3)
        .map(|fb| format!("- {}（{}）", fb.interpretation, fb.confidence))
        .collect();
    if !key_evidence.is_empty() {
        md.push_str("**关键证据**:\n");
        for e in &key_evidence { md.push_str(e); md.push('\n'); }
        md.push('\n');
    }

    // 风险提示
    md.push_str("**风险提示**: ");
    let risky = analyses.iter().any(|a| a.signal_strength < 5);
    if risky { md.push_str("单一信号来源，需更多交叉验证。\n\n"); }
    else { md.push_str("多源交叉验证充分，置信度较高。\n\n"); }

    md
}

fn build_decision_required(analyses: &[ThemeAnalysis]) -> String {
    if analyses.is_empty() {
        return "## 需要你决定\n\n今日无足够信号触发决策。继续执行当前计划。\n\n".into();
    }
    let mut md = String::from("## 需要你决定\n\n| 决策 | 建议 | 关键前提 | 截止 |\n|------|------|---------|------|\n");

    let has_commod = analyses.iter().any(|a|
        a.theme_title.contains("商品") || a.theme_title.contains("模型") || a.theme_title.contains("价格"));
    let has_reliability = analyses.iter().any(|a|
        a.theme_title.contains("可靠") || a.theme_title.contains("Agent"));
    let has_policy = analyses.iter().any(|a|
        a.theme_title.contains("政策") || a.theme_title.contains("风险") || a.theme_title.contains("芯片"));

    if has_commod || has_reliability {
        md.push_str("| 主攻应用层？ | 是 — 模型商品化窗口打开 | 价格战不压缩利润空间 | 本周评估 |\n");
    }
    if has_policy {
        md.push_str("| 增加多模型适配？ | 否 — 政策紧迫性不足 | 多模型维护成本可控 | 下季度重审 |\n");
    }
    md.push_str("| 调整当前计划？ | 暂不调整 — 信号尚不支持转向 | 窗口期不会关闭 | 下期简报 |\n");
    md.push('\n');
    md
}

fn build_watchlist_block(watchlist: Option<&[Article]>) -> String {
    let Some(watch) = watchlist else { return String::new(); };
    if watch.is_empty() { return String::new(); }

    let mut md = String::from("## 🟡 正在跟踪（Watchlist）\n\n以下信号不足以进入关键主题，但保留观察，多源交叉后将升级：\n\n");
    for article in watch {
        let raw = article.summary.as_deref()
            .or(article.content.as_deref())
            .unwrap_or("");
        let end = raw.floor_char_boundary(100);
        let snippet = &raw[..end];
        let desc = if snippet.len() > 10 { snippet } else { &article.title };
        md.push_str(&format!("- **{}**: {} [{}]({})\n", article.title, desc, article.source, article.url));
    }
    md.push('\n');
    md.push_str("---\n\n");
    md
}

fn build_processing_status(statuses: &[(String, bool, usize)]) -> String {
    if statuses.is_empty() {
        return String::new();
    }
    let mut md = String::from("## 处理状态\n\n| 源 | 状态 | 信号数 |\n|----|------|--------|\n");
    for (name, ok, count) in statuses {
        let icon = if *ok { "✅" } else { "❌" };
        md.push_str(&format!("| {} | {} | {} |\n", name, icon, count));
    }
    md.push('\n');
    md.push_str("---\n\n");
    md
}

fn build_calibration_block(calibration: Option<&str>) -> String {
    let Some(text) = calibration else { return String::new(); };
    if text.is_empty() { return String::new(); }
    format!(
        "────────────────────────────────────────\n\n🤖 **认知校准**\n\n> {}\n\n（不回答也没事，看到就行）\n\n────────────────────────────────────────\n\n",
        text
    )
}

fn build_source_index(themes: &[Theme], analyses: &[ThemeAnalysis]) -> String {
    let mut md = String::new();
    md.push_str("| 信号 | 来源 |\n|------|------|\n");
    for a in analyses {
        if let Some(t) = themes.iter().find(|t| t.id == a.theme_id) {
            for art in &t.articles {
                md.push_str(&format!("| {} | {} |\n", art.title, art.source));
            }
        }
    }
    md.push('\n');
    md
}
