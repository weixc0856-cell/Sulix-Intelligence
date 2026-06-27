use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::domain::PremiumReport;

/// YAML 双引号字符串值：转义 `\` 和 `"` 后包裹双引号
fn yaml_quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// 渲染信号为 Markdown + YAML frontmatter（用于 Astro Content Collections）
///
/// Code Review: sources/entities 使用 serde_json 序列化防止 YAML 注入撕裂。
pub fn render_signal_markdown(theme: &Theme, analysis: &ThemeAnalysis, date: &str) -> String {
    let svi_emoji_str = match analysis.signal_strength {
        9..=10 => "\u{1f534}",
        7..=8 => "\u{1f7e0}",
        5..=6 => "\u{1f7e1}",
        3..=4 => "\u{1f7e2}",
        _ => "\u{1f535}",
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
title: {title}
date: "{date}"
status: "published"
svi: {svi}
color_tag: "{emoji}"
is_premium: {premium}
slug: {slug}
summary: {summary}
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
        title = yaml_quoted(&theme.title),
        date = date,
        svi = analysis.signal_strength,
        emoji = svi_emoji_str,
        premium = if analysis.signal_strength >= 7 {
            "true"
        } else {
            "false"
        },
        slug = yaml_quoted(&slug),
        summary = yaml_quoted(&analysis.bluf),
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

## Diplomat — Geopolitical Assessment

{diplomat}

---

## Architect — Technical Impact

{architect}

---

## Quant — Commercial Framework

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
