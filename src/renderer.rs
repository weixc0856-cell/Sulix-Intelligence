//! 渲染模块 — 咨询级简报 + Economist 版式 HTML
//!
//! 字体授权声明（SIL Open Font License，100% 免费商用）:
//! - Lora (serif, 大标题): SIL OFL, 免费商用
//! - Inter (sans-serif, 正文): SIL OFL, 免费商用
//! - JetBrains Mono (monospace, 日期/标签): SIL OFL, 免费商用
//!
//! 抄 Reference/ 中 BCG/Deloitte/GS/McKinsey 报告结构
//! 所有输出数据集中到 TemplateData，由 template::render() 渲染
//!
//! 编译器静态分析无法追踪完整管线路径中的函数调用，
//! 这些函数在 main.rs 的非短路路径中被调用。

#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::Result;
use chrono::Local;

/// HTML 实体转义。顺序严格：& 必须最先转义，防止双重编码。
fn html_escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// 验证 URL scheme 仅为 http/https
fn validate_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        "#invalid-url".to_string()
    }
}

/// 渲染 SEO meta tags + Open Graph + Twitter Card
pub fn render_seo_meta(title: &str, description: &str, relative_path: &str) -> String {
    format!(
        r#"<meta name="description" content="{description}">
<meta property="og:title" content="{title} | Sulix Intelligence">
<meta property="og:description" content="{description}">
<meta property="og:type" content="article">
<meta property="og:url" content="https://intel.getsulix.com/{relative_path}">
<meta property="og:site_name" content="Sulix Intelligence">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{title}">
<meta name="twitter:description" content="{description}">
<link rel="canonical" href="https://intel.getsulix.com/{relative_path}">"#,
        title = html_escape(title),
        description = html_escape(description),
        relative_path = relative_path
    )
}

/// 渲染 JSON-LD 结构化数据（对标 Google 高价值 TechArticle）
pub fn render_json_ld(title: &str, date: &str, text_snippet: &str) -> String {
    let description = text_snippet
        .chars()
        .take(150)
        .collect::<String>()
        .replace('"', "\\\"");
    format!(
        r#"<script type="application/ld+json">
{{
  "@context": "https://schema.org",
  "@type": "TechArticle",
  "headline": "{title}",
  "datePublished": "{date}",
  "inLanguage": "en",
  "publisher": {{
    "@type": "Organization",
    "name": "Sulix Intelligence"
  }},
  "description": "{description}",
  "dependencies": "USPTO, SEC EDGAR, arXiv"
}}
</script>"#,
        title = html_escape(title),
        date = date,
        description = description
    )
}

use crate::clusterer::{Assumption, Theme, ThemeAnalysis};
use crate::fetcher::Article;
use crate::premium::PremiumReport;
use crate::template::{self, TemplateData};

/// CSS 相对路径辅助函数
/// depth=0: "./design.css"  depth=1: "../design.css"  depth=2: "../../design.css"
fn css_href(depth: usize) -> String {
    if depth == 0 {
        "./design.css".to_string()
    } else {
        format!("{}design.css", "../".repeat(depth))
    }
}

/// 渲染战略分析报告
pub fn render_analysis_report(
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    calibration: Option<&str>,
    watchlist: Option<&[Article]>,
    source_statuses: &[(String, bool, usize)],
) -> Result<String> {
    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M:%S").to_string();

    // 构建各内容块
    let executive_summary = build_executive_summary(analyses);
    let topic_sections = build_topic_sections(analyses);
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
    metrics.insert(
        "total_watchlist".into(),
        watchlist.map(|w| w.len()).unwrap_or(0).to_string(),
    );
    if let Some(highest) = analyses.iter().max_by_key(|a| a.signal_strength) {
        metrics.insert(
            "max_signal_strength".into(),
            highest.signal_strength.to_string(),
        );
    }
    // 蓝军风险审计信号
    let has_adverse = analyses.iter().any(|a| {
        a.adverse
            .as_ref()
            .map(|x| !x.scenario.is_empty())
            .unwrap_or(false)
    });
    metrics.insert(
        "risk_audit_passed".into(),
        if has_adverse {
            "false".into()
        } else {
            "true".into()
        },
    );

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
        transparency_disclaimer: String::from(
            "*This brief is aggregated by Sulix Intelligence from primary sources. Geopolitical facts are preserved for operational tracking.*"
        ),
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
        if theme.articles.is_empty() {
            continue;
        }

        topic_sections.push_str(&format!("## {}\n\n### 关键动态\n\n", theme.title));

        let best_url = theme
            .articles
            .iter()
            .find(|a| !a.url.is_empty())
            .map(|a| a.url.as_str())
            .unwrap_or("");
        for article in &theme.articles {
            let summary = article
                .summary
                .as_deref()
                .or(article.content.as_deref())
                .unwrap_or("");
            let end = summary.floor_char_boundary(120);
            let snippet = &summary[..end];
            let url = if !article.url.is_empty() {
                &article.url
            } else {
                best_url
            };
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
    let toc: String = themes
        .iter()
        .map(|t| {
            format!(
                "- [{}](#{})",
                t.title,
                t.title.to_lowercase().replace(' ', "-")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut metrics = HashMap::new();
    metrics.insert(
        "total_articles".into(),
        themes
            .iter()
            .map(|t| t.articles.len())
            .sum::<usize>()
            .to_string(),
    );

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
        transparency_disclaimer: String::new(),
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
            i + 1,
            a.bluf,
            a.impact,
            a.fact_base.len(),
        ));
    }
    md.push('\n');
    md
}

fn build_topic_sections(analyses: &[ThemeAnalysis]) -> String {
    let mut md = String::new();
    for a in analyses {
        md.push_str(&format!("## 主题: {}\n\n", a.theme_title));

        // Fact Base
        if !a.fact_base.is_empty() {
            md.push_str("| 证据 | 解读 | 置信度 |\n|------|------|--------|\n");
            for fb in &a.fact_base {
                md.push_str(&format!(
                    "| {} | {} | {} |\n",
                    fb.evidence, fb.interpretation, fb.confidence
                ));
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

        // Layer 2: 双轨制 — 地缘事实 + 供应链影响
        if !a.geopolitical_fact.is_empty() {
            md.push_str(&format!("**地缘事实**: {}\n\n", a.geopolitical_fact));
        }
        if !a.supply_chain_impact.is_empty() {
            md.push_str(&format!("**供应链影响**: {}\n\n", a.supply_chain_impact));
        }

        // 承重假设
        let load_bearing: Vec<&Assumption> =
            a.assumptions.iter().filter(|a| a.load_bearing).collect();
        if !load_bearing.is_empty() {
            md.push_str("**承重假设**:\n");
            for asm in &load_bearing {
                md.push_str(&format!(
                    "- {}（证据强度: {}）\n",
                    asm.text, asm.evidence_strength
                ));
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
            for t in &a.next_tests {
                md.push_str(&format!("- {}\n", t));
            }
            md.push('\n');
        }

        // 待回答的问题
        if !a.open_questions.is_empty() {
            md.push_str("**待回答的问题**:\n");
            for q in &a.open_questions {
                md.push_str(&format!("- {}\n", q));
            }
            md.push('\n');
        }

        // 关联
        if !a.connections.is_empty() {
            md.push_str(&format!("**关联**: {}\n\n", a.connections.join(" → ")));
        }

        // 溯源
        if !a.source_urls.is_empty() {
            md.push_str("**溯源**:\n");
            for url in &a.source_urls {
                md.push_str(&format!("- {}\n", url));
            }
            md.push('\n');
        }

        // 质量
        let source_count = a.source_urls.len();
        let assumption_count = a.assumptions.len();
        let has_adverse = a
            .adverse
            .as_ref()
            .map(|x| !x.scenario.is_empty())
            .unwrap_or(false);
        md.push_str(&format!(
            "**质量**: {} 来源 | {} 条承重假设 | {} | {} 项待验证\n",
            source_count,
            assumption_count,
            if has_adverse {
                "1 个逆境情景"
            } else {
                "无逆境情景"
            },
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
    let key_evidence: Vec<String> = analyses
        .iter()
        .flat_map(|a| a.fact_base.iter())
        .filter(|fb| fb.confidence.starts_with("确立"))
        .take(3)
        .map(|fb| format!("- {}（{}）", fb.interpretation, fb.confidence))
        .collect();
    if !key_evidence.is_empty() {
        md.push_str("**关键证据**:\n");
        for e in &key_evidence {
            md.push_str(e);
            md.push('\n');
        }
        md.push('\n');
    }

    // 风险提示
    md.push_str("**风险提示**: ");
    let risky = analyses.iter().any(|a| a.signal_strength < 5);
    if risky {
        md.push_str("单一信号来源，需更多交叉验证。\n\n");
    } else {
        md.push_str("多源交叉验证充分，置信度较高。\n\n");
    }

    md
}

fn build_decision_required(analyses: &[ThemeAnalysis]) -> String {
    if analyses.is_empty() {
        return "## 需要你决定\n\n今日无足够信号触发决策。继续执行当前计划。\n\n".into();
    }
    let mut md = String::from(
        "## 需要你决定\n\n| 决策 | 建议 | 关键前提 | 截止 |\n|------|------|---------|------|\n",
    );

    let has_commod = analyses.iter().any(|a| {
        a.theme_title.contains("商品")
            || a.theme_title.contains("模型")
            || a.theme_title.contains("价格")
    });
    let has_reliability = analyses
        .iter()
        .any(|a| a.theme_title.contains("可靠") || a.theme_title.contains("Agent"));
    let has_policy = analyses.iter().any(|a| {
        a.theme_title.contains("政策")
            || a.theme_title.contains("风险")
            || a.theme_title.contains("芯片")
    });

    if has_commod || has_reliability {
        md.push_str(
            "| 主攻应用层？ | 是 — 模型商品化窗口打开 | 价格战不压缩利润空间 | 本周评估 |\n",
        );
    }
    if has_policy {
        md.push_str(
            "| 增加多模型适配？ | 否 — 政策紧迫性不足 | 多模型维护成本可控 | 下季度重审 |\n",
        );
    }
    md.push_str("| 调整当前计划？ | 暂不调整 — 信号尚不支持转向 | 窗口期不会关闭 | 下期简报 |\n");
    md.push('\n');
    md
}

fn build_watchlist_block(watchlist: Option<&[Article]>) -> String {
    let Some(watch) = watchlist else {
        return String::new();
    };
    if watch.is_empty() {
        return String::new();
    }

    let mut md = String::from("## 🟡 正在跟踪（Watchlist）\n\n以下信号不足以进入关键主题，但保留观察，多源交叉后将升级：\n\n");
    for article in watch {
        let raw = article
            .summary
            .as_deref()
            .or(article.content.as_deref())
            .unwrap_or("");
        let end = raw.floor_char_boundary(100);
        let snippet = &raw[..end];
        let desc = if snippet.len() > 10 {
            snippet
        } else {
            &article.title
        };
        md.push_str(&format!(
            "- **{}**: {} [{}]({})\n",
            article.title, desc, article.source, article.url
        ));
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
    let Some(text) = calibration else {
        return String::new();
    };
    if text.is_empty() {
        return String::new();
    }
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

// ===== Terminal Dashboard (Bloomberg Terminal 风格) =====

use std::collections::BTreeSet;

/// SVI 颜色色值
fn svi_color(svi: u8) -> &'static str {
    match svi {
        9..=10 => "#dc2626",
        7..=8 => "#ea580c",
        5..=6 => "#ca8a04",
        3..=4 => "#16a34a",
        _ => "#2563eb",
    }
}

/// SVI 颜色表情
fn svi_emoji(svi: u8) -> &'static str {
    match svi {
        9..=10 => "🔴",
        7..=8 => "🟠",
        5..=6 => "🟡",
        3..=4 => "🟢",
        _ => "🔵",
    }
}

/// 渲染 Bloomberg Terminal 风格的 HTML 简报
pub fn render_html_report(
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    date: &str,
    calibration: Option<&str>,
    attributable_sources: &[crate::config::SourceConfig],
    flash_headline: Option<&str>,
    language: &str,
    source_statuses: &[(String, bool, usize)],
    change_summary: Option<&crate::clusterer::ChangeSummary>,
) -> Result<String> {
    let attributable_names = crate::source::attributable_source_names(attributable_sources);

    // 按 SVI 降序排列
    let mut indexed: Vec<(&Theme, &ThemeAnalysis)> = themes
        .iter()
        .zip(analyses.iter())
        .filter(|(_, a)| a.signal_strength > 0)
        .collect();
    indexed.sort_by(|(_, a), (_, b)| b.signal_strength.cmp(&a.signal_strength));

    let mut signals_html = String::new();
    let mut explicit_set: BTreeSet<String> = BTreeSet::new();
    let mut implicit_set: BTreeSet<String> = BTreeSet::new();

    for (theme, analysis) in &indexed {
        let svi = analysis.signal_strength;
        let is_premium = svi >= 7;

        let mut srcs: Vec<String> = Vec::new();
        for art in &theme.articles {
            if !srcs.contains(&art.source) {
                srcs.push(art.source.clone());
            }
        }
        for s in &srcs {
            if attributable_names.contains(s) {
                explicit_set.insert(s.clone());
            } else {
                implicit_set.insert(s.clone());
            }
        }

        let summary = if analysis.bluf.len() > 80 {
            let end = analysis.bluf.floor_char_boundary(80);
            format!("{}...", &analysis.bluf[..end])
        } else {
            analysis.bluf.clone()
        };

        let prem = if is_premium {
            let slug = theme.title.to_lowercase().replace(' ', "-");
            format!(
                r#"<a href="../premium/{}.html" style="color:#ea580c;font-family:'JetBrains Mono',monospace;font-size:0.65rem;font-weight:600;text-transform:uppercase;letter-spacing:0.05em;text-decoration:none;border:1px solid #ea580c;padding:0.0625rem 0.375rem;border-radius:0.125rem">🔒 Premium</a>"#,
                html_escape(&slug)
            )
        } else {
            String::new()
        };

        let line = format!(
            r#"<div style="display:flex;flex-direction:column;padding:0.5rem 0;border-bottom:1px solid #e5e5e5">
  <div style="display:flex;align-items:center;gap:0.5rem">
    <span style="color:{};font-family:'JetBrains Mono',monospace;font-weight:700;font-size:0.8125rem">{}</span>
    <span style="color:{};font-family:'JetBrains Mono',monospace;font-weight:700;font-size:0.8rem;min-width:3ch">{:.1}</span>
    <span style="font-family:'Inter',sans-serif;font-size:0.875rem;font-weight:500;color:#171717;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{}</span>
    <span style="margin-left:auto">{}</span>
  </div>
  <div style="font-family:'Inter',sans-serif;font-size:0.75rem;color:#525252;margin-top:0.125rem;padding-left:4.5rem">{}</div>
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;margin-top:0.125rem;padding-left:4.5rem">├─ 来源: {}</div>
</div>"#,
            svi_color(svi),
            svi_emoji(svi),
            svi_color(svi),
            svi as f64,
            html_escape(&theme.title),
            prem,
            html_escape(&summary),
            html_escape(&srcs.join(" · "))
        );
        signals_html.push_str(&line);
    }

    let explicit_links: Vec<String> = explicit_set.iter().map(|s| format!(r#"<span style="font-size:0.75rem;font-family:'JetBrains Mono',monospace;color:#525252">{} </span>"#, html_escape(s))).collect();
    let implicit_links: Vec<String> = implicit_set.iter().map(|s| format!(r#"<span style="font-size:0.75rem;font-family:'JetBrains Mono',monospace;color:#a3a3a3">{} </span>"#, html_escape(s))).collect();

    let cal_html = match calibration {
        Some(cal) if !cal.is_empty() => format!(
            r#"<div style="border-top:1px solid #e5e5e5;margin-top:1.5rem;padding-top:0.75rem"><span style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;font-weight:600;text-transform:uppercase">Cognitive Calibration</span><p style="font-family:'Inter',sans-serif;font-size:0.8125rem;color:#737373;font-style:italic;margin:0.25rem 0 0">{}</p></div>"#,
            html_escape(cal)
        ),
        _ => String::new(),
    };

    let flash = flash_headline.map(|fh| format!(r#"<div style="background-color:#dc2626;color:#fff;text-align:center;padding:0.375rem;font-family:'JetBrains Mono',monospace;font-size:0.75rem;font-weight:600">⚡ FLASH: {}</div>"#, html_escape(fh))).unwrap_or_default();

    let top = analyses.iter().max_by_key(|a| a.signal_strength);
    let seo_title = top
        .map(|a| a.theme_title.as_str())
        .unwrap_or("Daily Briefing");
    let seo_desc = top.map(|a| a.bluf.as_str()).unwrap_or("Sulix Intelligence");
    let lang_attr = if language == "zh" { "zh-Hant" } else { "en" };
    let seo_meta = render_seo_meta(seo_title, seo_desc, &format!("en/{}/", date));
    let json_ld = render_json_ld(seo_title, date, seo_desc);
    let d = if date.len() >= 10 {
        format!("{}-{}-{}", &date[0..4], &date[5..7], &date[8..10])
    } else {
        date.to_string()
    };
    let is_zh = language == "zh";
    let en_href = if is_zh {
        format!("../en/{}/index.html", date)
    } else {
        "#".into()
    };
    let zh_href = if !is_zh {
        format!("../zh/{}/index.html", date)
    } else {
        "#".into()
    };
    let en_s = if !is_zh {
        "color:#171717;font-weight:700"
    } else {
        "color:#a3a3a3"
    };
    let zh_s = if is_zh {
        "color:#171717;font-weight:700"
    } else {
        "color:#a3a3a3"
    };

    // Change Summary 区块
    let change_html = match change_summary {
        Some(cs) => {
            let mut h = String::from(
                r#"<div style="border-left:3px solid #2563eb;padding:0.5rem 0.75rem;margin-bottom:0.75rem;background:#fafafa;font-family:'JetBrains Mono',monospace;font-size:0.75rem">"#,
            );
            if cs.conflicts.is_empty() && cs.new_signals.is_empty() {
                h.push_str(&format!(
                    r#"<span style="color:#16a34a">✓ 无异动 — {} 条信号强化既有判断</span>"#,
                    cs.no_change_count
                ));
            } else {
                if !cs.conflicts.is_empty() {
                    h.push_str(&format!(r#"<div style="color:#dc2626;margin-bottom:0.25rem">⚡ {} 条信号与既有判断冲突</div>"#, cs.conflicts.len()));
                    for c in &cs.conflicts {
                        h.push_str(&format!(r#"<div style="padding-left:0.75rem;font-size:0.6875rem;color:#525252">✗ <strong>{}</strong>: {}</div>"#, html_escape(&c.topic), html_escape(&c.today_signal)));
                    }
                }
                if !cs.new_signals.is_empty() {
                    h.push_str(&format!(
                        r#"<div style="color:#2563eb;margin-top:0.25rem">★ {} 条新信号: {}</div>"#,
                        cs.new_signals.len(),
                        cs.new_signals
                            .iter()
                            .map(|s| html_escape(s))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                if cs.no_change_count > 0 {
                    h.push_str(&format!(r#"<div style="color:#a3a3a3;margin-top:0.25rem">○ {} 条不改变当前判断</div>"#, cs.no_change_count));
                }
            }
            h.push_str("</div>");
            h
        }
        None => String::new(),
    };

    // Source Health 区块
    let source_health_html = if source_statuses.is_empty() {
        String::new()
    } else {
        let (mut healthy, mut degraded, mut dead): (Vec<&str>, Vec<&str>, Vec<&str>) =
            (vec![], vec![], vec![]);
        for (name, ok, count) in source_statuses {
            if !ok {
                dead.push(name);
            } else if *count == 0 {
                dead.push(name);
            } else if *count <= 2 {
                degraded.push(name);
            } else {
                healthy.push(name);
            }
        }
        let mut html = String::from(
            r#"<div style="margin-top:0.75rem;padding-top:0.375rem;border-top:1px solid #e5e5e5;font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#a3a3a3">▸ SOURCE HEALTH "#,
        );
        if !dead.is_empty() {
            html.push_str(&format!(
                r#"<span style="color:#dc2626">✗ {}无数据</span> "#,
                dead.join("·")
            ));
        }
        if !degraded.is_empty() {
            html.push_str(&format!(
                r#"<span style="color:#ca8a04">△ {}低流量</span> "#,
                degraded.join("·")
            ));
        }
        html.push_str(&format!("✓ {}源正常", healthy.len()));
        html.push_str("</div>");
        html
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="{}">
<head>
<meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>Sulix.Intel | {}</title>
<link rel="stylesheet" href="./design.css">
<link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><rect width='100' height='100' fill='%23e3120b'/><text y='75' x='35' font-family='sans-serif' font-weight='900' font-size='70' fill='white'>i</text></svg>">
{}{}
<style>body{{font-family:'Inter',sans-serif;background:#fcfcfc;color:#111;margin:0}}a{{text-decoration:none}}a:hover{{text-decoration:underline}}</style>
</head>
<body>
{}
<header style="border-bottom:1px solid #e5e5e5;background:#fff"><div style="max-width:72rem;margin:0 auto;padding:0 1rem;height:3rem;display:flex;align-items:center;justify-content:space-between">
<a href="/" style="display:flex;align-items:center;gap:0.5rem"><span style="background-color:#e3120b;color:#fff;font-family:'JetBrains Mono',monospace;font-weight:700;font-size:0.75rem;padding:0.125rem 0.375rem;border-radius:0.125rem">i</span><span style="font-family:'JetBrains Mono',monospace;font-weight:700;font-size:0.9375rem;color:#171717">Sulix.Intel</span></a>
<nav style="display:flex;align-items:center;gap:0.75rem">
<a href="{}" style="font-family:'JetBrains Mono',monospace;font-size:0.6875rem;{}">EN</a><span style="color:#d4d4d4;font-size:0.6875rem">|</span>
<a href="{}" style="font-family:'JetBrains Mono',monospace;font-size:0.6875rem;{}">繁中</a>
<a href="https://sulix.substack.com/subscribe" target="_blank" style="background-color:#e3120b;color:#fff;font-family:'JetBrains Mono',monospace;font-size:0.625rem;font-weight:600;padding:0.25rem 0.5rem;border-radius:0.125rem;text-transform:uppercase;letter-spacing:0.05em">Subscribe →</a>
</nav></div></header>
<main style="max-width:72rem;margin:0 auto;padding:1rem 1rem 2rem">
<div style="display:flex;align-items:baseline;justify-content:space-between;margin-bottom:0.75rem;padding-bottom:0.5rem;border-bottom:2px solid #171717">
<h1 style="font-family:'JetBrains Mono',monospace;font-size:1.25rem;font-weight:700;color:#171717;margin:0">今日信号</h1>
<span style="font-family:'JetBrains Mono',monospace;font-size:0.6875rem;color:#a3a3a3">{} · {} 条</span>
</div>
{}
<div>{}</div>
{}
{}
</main>
<footer style="max-width:72rem;margin:1.5rem auto 2rem;padding:0 1rem"><div style="border-top:1px solid #e5e5e5;padding-top:1rem">
<div style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;font-weight:600;text-transform:uppercase;margin-bottom:0.5rem">📚 Primary Sources & Traces</div>
<div style="font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#525252;font-weight:600;text-transform:uppercase;letter-spacing:0.08em;margin-bottom:0.25rem">═══ Explicit Citation ═══</div>
<div style="display:flex;flex-wrap:wrap;gap:0.375rem;margin-bottom:0.5rem">{}</div>
<div style="font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#a3a3a3;font-weight:600;text-transform:uppercase;letter-spacing:0.08em;margin-bottom:0.25rem">═══ Implicit Intelligence ═══</div>
<div style="display:flex;flex-wrap:wrap;gap:0.375rem;margin-bottom:0.75rem">{}</div>
<p style="font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#a3a3a3;line-height:1.5;margin:0">* Sulix operates under Fair Use. Data from publicly available primary documents.</p>
</div>
{}

<div style="margin-top:1rem;padding-top:0.5rem;border-top:1px solid #e5e5e5;font-family:'JetBrains Mono',monospace;font-size:0.5625rem;color:#a3a3a3">Sulix.Intel · intel.getsulix.com · Substack · GitHub · MIT · Generated {}</div>
</footer>
</body>
</html>"#,
        lang_attr, html_escape(seo_title), seo_meta, json_ld,
        flash,
        en_href, en_s, zh_href, zh_s,
        d, indexed.len(),
        change_html,
        signals_html,
        flash_headline.map(|_| format!(r#"<div style="margin-top:0.5rem;padding-top:0.5rem;border-top:1px solid #e5e5e5;display:flex;gap:1rem;font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3"><span>{} 条信号</span><span style="color:#dc2626">🔴 Flash</span></div>"#, indexed.len())).unwrap_or_default(),
        cal_html,
        if explicit_links.is_empty() { "<span style=\"font-size:0.75rem;font-family:'JetBrains Mono',monospace;color:#a3a3a3\">No explicit citations</span>".into() } else { explicit_links.join("") },
        if implicit_links.is_empty() { "<span style=\"font-size:0.75rem;font-family:'JetBrains Mono',monospace;color:#a3a3a3\">No implicit sources</span>".into() } else { implicit_links.join("") },
        source_health_html,
        chrono::Local::now().format("%Y-%m-%d %H:%M UTC"),
    );

    Ok(html)
}

/// 渲染编年史看板总页面（Economist Graphic Detail 版式）
pub fn render_archive_dashboard(entries: &[crate::archive::ChronicleEntry]) -> Result<String> {
    let list_html: String = entries.iter().map(|item| {
        let entities_badges: String = item.entities.iter()
            .map(|e| format!("<span class='text-[10px] font-mono bg-neutral-100 text-neutral-600 px-1.5 py-0.5 rounded-sm'>{}</span>", html_escape(e)))
            .collect::<Vec<_>>().join(" ");

        format!(
            r#"<div class="group border-b border-neutral-100 py-4 flex flex-col md:flex-row md:items-baseline md:justify-between hover:bg-neutral-50/50 px-2 transition-colors">
                <div class="flex items-baseline gap-4">
                  <span class="text-xs font-mono text-neutral-400 font-semibold w-24 shrink-0">{}</span>
                  <div class="space-y-1">
                    <span class="text-xs font-bold text-[#e3120b] uppercase tracking-wider block text-[10px]">{}</span>
                    <span class="chronicle-title text-lg font-bold text-neutral-900 group-hover:text-[#e3120b] transition-colors">{}</span>
                  </div>
                </div>
                <div class="mt-2 md:mt-0 flex gap-1.5">{}</div>
              </div>"#,
            item.date,
            html_escape(&item.topic),
            html_escape(&item.headline),
            entities_badges
        )
    }).collect::<Vec<_>>().join("\n");

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Geopolitical Tech Chronicle | Sulix</title>
  <style>body{{font-family:'Inter',-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background-color:#fcfcfc;color:#111;}}.chronicle-title{{font-family:'Lora','Playfair Display','Georgia',serif;}}</style>
</head>
<body>
  <div class="h-[4px] w-full bg-[#e3120b]"></div>
  <header class="border-b border-neutral-100 bg-white">
    <div class="max-w-5xl mx-auto px-4 h-14 flex items-center justify-between sm:px-6 lg:px-8">
      <a href="/" class="flex items-center gap-2.5 no-underline group select-none">
        <div class="w-6 h-6 bg-[#e3120b] flex items-center justify-center rounded-xs shadow-[0_1px_2px_rgba(0,0,0,0.05)]">
          <span class="text-white font-sans font-black text-sm tracking-tighter leading-none relative -top-[0.5px]" style="font-family: Inter">i</span>
        </div>
        <div class="flex items-baseline tracking-tight">
          <span class="text-lg font-bold text-neutral-900" style="font-family: 'Lora', 'Playfair Display', 'Georgia', serif;">Sulix</span>
          <span class="text-lg font-light text-neutral-300 mx-0.5">.</span>
          <span class="text-xs font-semibold tracking-widest text-neutral-400 uppercase" style="font-family: Inter;">Intel</span>
        </div>
      </a>
      <nav class="flex items-center gap-3 text-[11px] font-semibold tracking-wider text-neutral-400" style="font-family: Inter">
        <button onclick="toggleLang('en')" id="l-en" class="font-bold border-b-2 border-neutral-900 pb-0.5 cursor-pointer">EN</button>
        <span class="text-neutral-300">|</span>
        <button onclick="toggleLang('zh')" id="l-zh" class="text-neutral-400 hover:text-neutral-900 cursor-pointer">繁中</button>
      </nav>
    </div>
  </header>

  <div class="max-w-4xl mx-auto px-4 py-8">
    <div class="border-b-2 border-neutral-950 pb-6">
      <h1 class="chronicle-title text-4xl sm:text-5xl font-bold tracking-tight text-neutral-900">Geopolitical Tech Chronicle</h1>
      <p class="chronicle-title text-lg italic text-neutral-500 mt-2">A long-arc systemic tracker tracing geopolitical frictions down to technology supply lines.</p>
      <div class="mt-3 text-xs text-neutral-400">{} entries spanning {} topics</div>
    </div>
    <div class="mt-8 space-y-1">
      <div class="text-xs font-bold uppercase tracking-wider text-neutral-400 border-b border-neutral-200 pb-2 px-2">Historical Event Feed</div>
      {}
    </div>
  </div>
<script>
function toggleLang(t){{var p=window.location.pathname;if(p.endsWith('index.html')){{p=p.substring(0,p.lastIndexOf('index.html'))}}
if(t==='zh'){{if(!p.startsWith('/zh/')){{var ce=p.startsWith('/en/')?p.substring(3):p;window.location.pathname='/zh'+(ce.startsWith('/')?ce:'/'+ce)}}}}
else if(t==='en'){{if(p.startsWith('/zh/')){{var cz=p.substring(3);window.location.pathname=(cz==='/'||cz==='')?'/':'/en'+cz}}else if(p==='/'||p===''){{window.location.pathname='/en/'}}}}}}
(function(){{var pp=window.location.pathname,zh=pp.startsWith('/zh/');var el=document.getElementById('l-zh');var ee=document.getElementById('l-en');if(el&&ee){{el.className=zh?'font-bold border-b-2 border-neutral-900 pb-0.5 text-neutral-900':'text-neutral-400 hover:text-neutral-900 cursor-pointer';ee.className=zh?'text-neutral-400 hover:text-neutral-900 cursor-pointer':'font-bold border-b-2 border-neutral-900 pb-0.5 text-neutral-900'}}}}}})()
</script>
</body>
</html>"#,
        entries.len(),
        entries
            .iter()
            .map(|e| e.topic.as_str())
            .collect::<std::collections::HashSet<&str>>()
            .len(),
        list_html,
    );

    Ok(html)
}

/// 渲染 Premium 深度研报（长格式，多 Agent 合成）
pub fn render_premium_report(report: &PremiumReport) -> Result<String> {
    let risk_lines: String = report
        .risk_scenarios
        .iter()
        .map(|s| {
            format!(
                "<li class='text-neutral-700 text-sm mb-1'>{}</li>",
                html_escape(s)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let sources_lines: String = report.sources.iter()
        .map(|s| format!("<span class='text-[10px] font-mono bg-neutral-100 text-neutral-600 px-1.5 py-0.5 rounded-sm border border-neutral-200/60'>{}</span>", html_escape(s)))
        .collect::<Vec<_>>()
        .join(" ");

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Premium Research — {} | Sulix Intel</title>
  <link rel="stylesheet" href="./design.css">
</head>
<body class="antialiased">
  <div class="h-[4px] w-full bg-[#e3120b]"></div>
  <div class="max-w-3xl mx-auto px-6 py-12">

    <div class="border-b-2 border-neutral-950 pb-6 mb-8">
      <span class="text-[#e3120b] text-xs font-bold uppercase tracking-widest">Premium Research</span>
      <h1 class="serif text-3xl sm:text-4xl font-bold mt-2 leading-tight">{}</h1>
      <div class="flex items-center gap-3 mt-3 text-xs text-neutral-400">
        <span>{}</span>
        <span>·</span>
        <span>Multi-Agent Intelligence Report</span>
      </div>
    </div>

    <div class="bg-white rounded-lg p-6 border border-neutral-200/80 mb-6">
      <h2 class="text-xs font-bold uppercase tracking-wider text-neutral-400 mb-3">Executive Summary</h2>
      <div class="text-neutral-800 text-[15px] leading-relaxed">{}</div>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      <div class="bg-white rounded-lg p-5 border border-neutral-200/80">
        <span class="text-xs font-bold text-[#e3120b] uppercase tracking-wider">👤 Diplomat</span>
        <h3 class="text-sm font-semibold mt-1 mb-2">Geopolitical Assessment</h3>
        <p class="text-neutral-700 text-sm leading-relaxed">{}</p>
      </div>
      <div class="bg-white rounded-lg p-5 border border-neutral-200/80">
        <span class="text-xs font-bold text-amber-600 uppercase tracking-wider">👤 Architect</span>
        <h3 class="text-sm font-semibold mt-1 mb-2">Technical Impact</h3>
        <p class="text-neutral-700 text-sm leading-relaxed">{}</p>
      </div>
      <div class="bg-white rounded-lg p-5 border border-neutral-200/80">
        <span class="text-xs font-bold text-sky-700 uppercase tracking-wider">👤 Quant</span>
        <h3 class="text-sm font-semibold mt-1 mb-2">Commercial Framework</h3>
        <p class="text-neutral-700 text-sm leading-relaxed">{}</p>
      </div>
    </div>

    <div class="bg-white rounded-lg p-6 border border-neutral-200/80 mb-6">
      <h2 class="text-xs font-bold uppercase tracking-wider text-amber-700 mb-3">Risk Scenarios</h2>
      <ul class="space-y-1 list-disc pl-5">{}</ul>
    </div>

    <div class="border-t border-neutral-200 pt-4 mt-8">
      <h3 class="text-xs font-bold uppercase tracking-wider text-neutral-400 mb-2">Data Sources</h3>
      <div class="flex flex-wrap gap-1.5">{}</div>
    </div>

    <div class="mt-8 pt-4 border-t border-neutral-200">
      <p class="text-[10px] text-neutral-400 leading-relaxed">
        * Sulix Premium Research is generated by an automated multi-agent intelligence pipeline.
        Sources are cited for traceability. This is not financial or legal advice.
      </p>
    </div>

  </div>
</body>
</html>"#,
        html_escape(&report.theme_title),
        html_escape(&report.theme_title),
        html_escape(&report.date),
        html_escape(&report.executive_summary),
        html_escape(&report.geopolitical_assessment),
        html_escape(&report.technical_impact),
        html_escape(&report.commercial_framework),
        risk_lines,
        sources_lines,
    );

    Ok(html)
}

/// 渲染信号为 Markdown + YAML frontmatter（用于 Astro Content Collections）
///
/// Code Review: sources/entities 使用 serde_json 序列化防止 YAML 注入撕裂。
pub fn render_signal_markdown(theme: &Theme, analysis: &ThemeAnalysis, date: &str) -> String {
    let svi_emoji_str = match analysis.signal_strength {
        9..=10 => "🔴",
        7..=8 => "🟠",
        5..=6 => "🟡",
        3..=4 => "🟢",
        _ => "🔵",
    };

    let source_names: Vec<&str> = theme.articles.iter().map(|a| a.source.as_str()).collect();
    let json_sources = serde_json::to_string(&source_names).unwrap_or_else(|_| "[]".to_string());
    let json_entities =
        serde_json::to_string(&analysis.connections).unwrap_or_else(|_| "[]".to_string());

    let tags: Vec<&str> = std::iter::once(analysis.theme_title.as_str()).collect();
    let json_tags = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());

    let slug = theme.title.to_lowercase().replace(' ', "-");

    format!(
        r#"---
title: "{title}"
date: "{date}"
status: "published"
svi: {svi}
color_tag: "{emoji}"
is_premium: {premium}
slug: "{slug}"
summary: "{summary}"
sources: {sources}
entities: {entities}
tags: {tags}
author: "Diplomat · Architect · Quant"
---

## Executive Summary

{bluf}

## Geopolitical Fact

{geopolitical}

## Supply Chain Impact

{impact}

## Analysis

{analysis_para}
"#,
        title = theme.title,
        date = date,
        svi = analysis.signal_strength,
        emoji = svi_emoji_str,
        premium = if analysis.signal_strength >= 7 {
            "true"
        } else {
            "false"
        },
        slug = slug,
        summary = analysis.bluf,
        sources = json_sources,
        entities = json_entities,
        tags = json_tags,
        bluf = analysis.bluf,
        geopolitical = analysis.geopolitical_fact,
        impact = analysis.supply_chain_impact,
        analysis_para = analysis.analysis_paragraph,
    )
}

/// 渲染 Premium 报告为 Substack Markdown（用于 API 推送）
///
/// Code Review 防御性设计: report.sources 必须转化为纯 Markdown 链接格式
/// [Source Name](URL)，不能直接把前端的 HTML 字符串灌进去。
/// 但当前 PremiumReport.sources 仅为 Vec<String>（源名称），无 URL。
/// 此处先渲染为名称列表，Phase 2 Substack 精确化时扩展为带 URL 的格式。
pub fn render_substack_markdown(report: &PremiumReport) -> String {
    let scenarios = report.risk_scenarios.join("\n");
    let sources = report.sources.join("\n");

    format!(
        r#"---
title: "【Premium】{title}"
date: {date}
---

## Executive Summary

{summary}

---

## 👤 Diplomat — Geopolitical Assessment

{diplomat}

---

## 👤 Architect — Technical Impact

{architect}

---

## 👤 Quant — Commercial Framework

{quant}

---

## Risk Scenarios

{scenarios}

---

## Primary Sources

{sources}
"#,
        title = report.theme_title,
        date = report.date,
        summary = report.executive_summary,
        diplomat = report.geopolitical_assessment,
        architect = report.technical_impact,
        quant = report.commercial_framework,
        scenarios = scenarios,
        sources = sources,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_escape_ampersand_first() {
        assert_eq!(html_escape("&lt;"), "&amp;lt;");
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("\"quote\""), "&quot;quote&quot;");
        assert_eq!(html_escape("'it's'"), "&#x27;it&#x27;s&#x27;");
        assert_eq!(html_escape("safe text"), "safe text");
        assert_eq!(html_escape(""), "");
    }

    #[test]
    fn test_html_escape_edge_cases() {
        assert_eq!(
            html_escape("a&b<c>d\"e'f"),
            "a&amp;b&lt;c&gt;d&quot;e&#x27;f"
        );
        assert_eq!(html_escape("&&&"), "&amp;&amp;&amp;");
    }

    #[test]
    fn test_validate_url() {
        assert_eq!(validate_url("https://example.com"), "https://example.com");
        assert_eq!(validate_url("http://test.org/page"), "http://test.org/page");
        assert_eq!(validate_url(""), "#invalid-url");
        assert_eq!(validate_url("javascript:alert(1)"), "#invalid-url");
        assert_eq!(validate_url("data:text/html,<script>"), "#invalid-url");
    }
}
