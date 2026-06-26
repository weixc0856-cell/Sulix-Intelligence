//! MDX 渲染器 — Sulix 知识资产的持久化格式
//!
//! Rust 不再生成 HTML。所有输出转为 MDX 知识资产：
//!   output/daily/     → 每日信号
//!   output/thesis/    → 判断追踪
//!   output/research/  → Premium 研报
//!   output/memory/    → 复盘反思
//!
//! MDX 格式优势：
//! - YAML frontmatter：Git diff 友好，Astro Content Collection 原生支持
//! - Markdown 正文：人类可读，机器可解析
//! - 无需 HTML 模板引擎，无需 CSS

use crate::clusterer::{Theme, ThemeAnalysis};
use crate::engine::decision::ThesisDecision;
use crate::engine::memory::{Outcome, Reflection, Stance, Thesis};
use crate::engine::premium::PremiumReport;
use crate::renderer::helpers::yaml_escape;

/// 渲染每日信号 MDX
///
/// 每个 theme 生成一个文件，包含：
/// - YAML frontmatter: title, date, svi, asi, confidence, sources, entities, thesis_status
/// - 正文: BLUF, Thesis, Evidence 表, Assumptions, Action
pub fn render_daily_mdx(
    theme: &Theme,
    analysis: &ThemeAnalysis,
    today: &str,
    asi: f64,
    confidence: f64,
    editor_notes: &[crate::agent::editor::EditorNote],
) -> String {
    let _slug = theme
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .replace(' ', "-");

    let sources: Vec<&str> = theme.articles.iter().map(|a| a.source.as_str()).collect();
    let sources_yaml = sources
        .iter()
        .map(|s| format!("  - {}", yaml_escape(s)))
        .collect::<Vec<_>>()
        .join("\n");

    let entities: Vec<String> = analysis
        .fact_base
        .iter()
        .flat_map(|fb| fb.evidence.split_whitespace())
        .map(|w| w.to_uppercase())
        .filter(|w| {
            [
                "TSMC",
                "ASML",
                "NVIDIA",
                "OPENAI",
                "ANTHROPIC",
                "GOOGLE",
                "META",
                "MICROSOFT",
                "INTEL",
                "AMD",
                "ARM",
                "HBM",
            ]
            .contains(&w.as_str())
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let entities_yaml = entities
        .iter()
        .map(|e| format!("  - {}", e))
        .collect::<Vec<_>>()
        .join("\n");

    // Editor notes for this theme
    let theme_notes: Vec<&crate::agent::editor::EditorNote> = editor_notes
        .iter()
        .filter(|n| n.theme_title == theme.title)
        .collect();

    let mut mdx = String::new();

    // YAML frontmatter
    mdx.push_str("---\n");
    mdx.push_str(&format!("title: {}\n", yaml_escape(&theme.title)));
    mdx.push_str(&format!("date: \"{}\"\n", today));
    mdx.push_str(&format!("svi: {}\n", analysis.signal_strength));
    mdx.push_str(&format!("asi: {:.2}\n", asi));
    mdx.push_str(&format!("confidence: {:.2}\n", confidence));
    if !sources.is_empty() {
        mdx.push_str(&format!("sources:\n{}\n", sources_yaml));
    }
    if !entities.is_empty() {
        mdx.push_str(&format!("entities:\n{}\n", entities_yaml));
    }
    if !analysis.assumptions.is_empty() {
        mdx.push_str("assumptions:\n");
        for a in &analysis.assumptions {
            mdx.push_str(&format!("  - text: {}\n", yaml_escape(&a.text)));
            mdx.push_str(&format!("    load_bearing: {}\n", a.load_bearing));
            mdx.push_str(&format!(
                "    evidence_strength: {}\n",
                yaml_escape(&a.evidence_strength)
            ));
        }
    }
    mdx.push_str("---\n\n");

    // Body
    mdx.push_str(&format!("## BLUF\n\n{}\n\n", analysis.bluf));

    mdx.push_str("## Analysis\n\n");
    if !analysis.impact.is_empty() {
        mdx.push_str(&format!("**Impact:** {}\n\n", analysis.impact));
    }
    if !analysis.geopolitical_fact.is_empty() {
        mdx.push_str(&format!(
            "**Geopolitical:** {}\n\n",
            analysis.geopolitical_fact
        ));
    }
    if !analysis.supply_chain_impact.is_empty() {
        mdx.push_str(&format!(
            "**Supply Chain:** {}\n\n",
            analysis.supply_chain_impact
        ));
    }
    if !analysis.analysis_paragraph.is_empty() {
        mdx.push_str(&format!("{}\n\n", analysis.analysis_paragraph));
    }

    // Evidence table
    if !analysis.fact_base.is_empty() {
        mdx.push_str("## Evidence\n\n");
        mdx.push_str("| 证据 | 解读 | 置信度 |\n");
        mdx.push_str("|------|------|--------|\n");
        for fb in &analysis.fact_base {
            mdx.push_str(&format!(
                "| {} | {} | {} |\n",
                yaml_escape(&fb.evidence),
                yaml_escape(&fb.interpretation),
                yaml_escape(&fb.confidence)
            ));
        }
        mdx.push('\n');
    }

    // Assumptions
    if !analysis.assumptions.is_empty() {
        mdx.push_str("## Assumptions\n\n");
        for a in &analysis.assumptions {
            let icon = if a.load_bearing { "🔴" } else { "🟡" };
            mdx.push_str(&format!(
                "- {} **{}** (证据强度: {})\n",
                icon, a.text, a.evidence_strength
            ));
        }
        mdx.push('\n');
    }

    // Causal chains
    if !analysis.chains.is_empty() {
        mdx.push_str("## Causal Chains\n\n");
        for chain in &analysis.chains {
            mdx.push_str(&format!("- **Trigger:** {}\n", chain.trigger));
            mdx.push_str(&format!("  **Direct:** {}\n", chain.direct_effect));
            if !chain.chain_reaction.is_empty() {
                mdx.push_str(&format!(
                    "  **Chain:** {}\n",
                    chain.chain_reaction.join(" → ")
                ));
            }
        }
        mdx.push('\n');
    }

    // Editor notes
    if !theme_notes.is_empty() {
        mdx.push_str("## Personal Impact\n\n");
        for note in &theme_notes {
            let action_icon = match note.recommended_action.as_str() {
                "Invest" => "💰",
                "Explore" => "🔍",
                "Exit" => "🚨",
                _ => "👀",
            };
            mdx.push_str(&format!(
                "- {} {} [{}]\n",
                action_icon, note.impact, note.recommended_action
            ));
        }
        mdx.push('\n');
    }

    // Sources
    mdx.push_str("## Sources\n\n");
    for art in &theme.articles {
        mdx.push_str(&format!(
            "- [{}]({}) — {}\n",
            art.source, art.url, art.title
        ));
    }
    mdx.push('\n');

    mdx
}

/// 渲染 Thesis MDX（判断追踪更新）
pub fn render_thesis_mdx(
    thesis: &Thesis,
    outcomes: &[Outcome],
    decision: Option<&ThesisDecision>,
) -> String {
    let _slug = thesis
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .replace(' ', "-");

    // Thesis 前台状态: 简化映射，consulting-style labels
    let (status_label, status_str) = match thesis.status {
        crate::domain::thesis::ThesisStatus::Proposed => ("early-signal", "proposed"),
        crate::domain::thesis::ThesisStatus::Active => ("developing", "active"),
        crate::domain::thesis::ThesisStatus::Strengthening => ("established", "strengthening"),
        crate::domain::thesis::ThesisStatus::Weakening => ("at-risk", "weakening"),
        crate::domain::thesis::ThesisStatus::Dormant => ("under-review", "dormant"),
        crate::domain::thesis::ThesisStatus::Retired => ("archived", "retired"),
    };

    let support = thesis
        .evidences
        .iter()
        .filter(|e| e.stance == Stance::Supports)
        .count();
    let challenge = thesis
        .evidences
        .iter()
        .filter(|e| e.stance == Stance::Challenges)
        .count();

    let confidence = crate::engine::memory::compute_confidence(&thesis.evidences);

    let mut mdx = String::new();
    // Derive summary: use first load-bearing assumption text, fall back to title
    let summary = thesis
        .assumptions
        .iter()
        .find(|a| a.load_bearing)
        .map(|a| a.text.as_str())
        .unwrap_or(&thesis.title);

    mdx.push_str("---\n");
    mdx.push_str(&format!("title: {}\n", yaml_escape(&thesis.title)));
    mdx.push_str(&format!("date: \"{}\"\n", thesis.updated));
    mdx.push_str(&format!("summary: {}\n", yaml_escape(summary)));
    mdx.push_str(&format!("status: \"{}\"\n", status_str));
    mdx.push_str(&format!("status_label: \"{}\"\n", status_label));
    mdx.push_str(&format!("confidence: {:.2}\n", confidence));
    mdx.push_str(&format!("evidences: {}\n", support));
    mdx.push_str(&format!("challenges: {}\n", challenge));
    // Decision Intelligence frontmatter
    if let Some(dec) = decision {
        mdx.push_str(&format!(
            "decision: \"{}\"\n",
            dec.decision_type.label().to_lowercase()
        ));
        mdx.push_str(&format!(
            "decision_rationale: \"{}\"\n",
            yaml_escape(&dec.rationale)
        ));
        mdx.push_str(&format!("decision_horizon: \"{}\"\n", dec.horizon.as_str()));
        mdx.push_str(&format!(
            "decision_stability: \"{}\"\n",
            dec.stability.label()
        ));
    }
    // Outcome / Historical Accuracy frontmatter
    if !outcomes.is_empty() {
        let total = outcomes.len();
        let confirmed = outcomes
            .iter()
            .filter(|o| o.verdict == crate::engine::memory::OutcomeVerdict::Confirmed)
            .count();
        let partial = outcomes
            .iter()
            .filter(|o| o.verdict == crate::engine::memory::OutcomeVerdict::PartiallyConfirmed)
            .count();
        let invalidated = outcomes
            .iter()
            .filter(|o| o.verdict == crate::engine::memory::OutcomeVerdict::Invalidated)
            .count();
        let accuracy = (confirmed as f64 + partial as f64 * 0.5) / total as f64;
        mdx.push_str(&format!("outcome_total: {}\n", total));
        mdx.push_str(&format!("outcome_confirmed: {}\n", confirmed));
        mdx.push_str(&format!("outcome_partial: {}\n", partial));
        mdx.push_str(&format!("outcome_invalidated: {}\n", invalidated));
        mdx.push_str(&format!("historical_accuracy: {:.2}\n", accuracy));
    }
    mdx.push_str("---\n\n");

    mdx.push_str(&format!("## Status\n\n- **状态:** {:?}\n", thesis.status));
    mdx.push_str(&format!("- **创建:** {}\n", thesis.created));
    mdx.push_str(&format!("- **更新:** {}\n", thesis.updated));

    let support = thesis
        .evidences
        .iter()
        .filter(|e| e.stance == Stance::Supports)
        .count();
    let challenge = thesis
        .evidences
        .iter()
        .filter(|e| e.stance == Stance::Challenges)
        .count();
    mdx.push_str(&format!(
        "- **支持:** {} | **挑战:** {}\n\n",
        support, challenge
    ));

    // Assumptions
    if !thesis.assumptions.is_empty() {
        mdx.push_str("## Assumptions\n\n");
        for a in &thesis.assumptions {
            mdx.push_str(&format!(
                "- {} (承重: {}, 证据: {})\n",
                a.text, a.load_bearing, a.evidence_strength
            ));
        }
        mdx.push('\n');
    }

    // Evidence timeline
    if !thesis.evidences.is_empty() {
        mdx.push_str("## Evidence Timeline\n\n");
        mdx.push_str("| 日期 | 标题 | 立场 | 摘要 |\n");
        mdx.push_str("|------|------|------|------|\n");
        for e in thesis.evidences.iter().rev().take(20) {
            let icon = match e.stance {
                Stance::Supports => "↑ 支持",
                Stance::Challenges => "↓ 挑战",
                Stance::Neutral => "→ 中性",
            };
            mdx.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                e.date,
                yaml_escape(&e.title),
                icon,
                yaml_escape(&e.summary)
            ));
        }
        mdx.push('\n');
    }

    // Outcomes
    if !outcomes.is_empty() {
        mdx.push_str("## Outcomes\n\n");
        for o in outcomes {
            let icon = match o.verdict {
                crate::engine::memory::OutcomeVerdict::Confirmed => "✅",
                crate::engine::memory::OutcomeVerdict::PartiallyConfirmed => "🟡",
                crate::engine::memory::OutcomeVerdict::Invalidated => "❌",
                crate::engine::memory::OutcomeVerdict::Unknown => "❓",
            };
            mdx.push_str(&format!("- {} {}: {}\n", icon, o.date, o.description));
        }
        mdx.push('\n');
    }

    mdx
}

/// 渲染 Premium 研报 MDX
pub fn render_research_mdx(report: &PremiumReport) -> String {
    let _slug = report
        .theme_title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .replace(' ', "-");

    let mut mdx = String::new();
    mdx.push_str("---\n");
    mdx.push_str(&format!("title: {}\n", yaml_escape(&report.theme_title)));
    mdx.push_str(&format!("date: \"{}\"\n", report.date));
    mdx.push_str("stage: \"what-to-do\"\n");
    mdx.push_str("is_premium: true\n");
    mdx.push_str("---\n\n");

    mdx.push_str("## Executive Summary\n\n");
    mdx.push_str(&format!("{}\n\n", report.executive_summary));

    if !report.geopolitical_assessment.is_empty() {
        mdx.push_str("## Geopolitical Assessment\n\n");
        mdx.push_str(&format!("{}\n\n", report.geopolitical_assessment));
    }

    if !report.technical_impact.is_empty() {
        mdx.push_str("## Technical Impact\n\n");
        mdx.push_str(&format!("{}\n\n", report.technical_impact));
    }

    if !report.commercial_framework.is_empty() {
        mdx.push_str("## Commercial Framework\n\n");
        mdx.push_str(&format!("{}\n\n", report.commercial_framework));
    }

    if !report.risk_scenarios.is_empty() {
        mdx.push_str("## Risk Scenarios\n\n");
        for s in &report.risk_scenarios {
            mdx.push_str(&format!("- {}\n", s));
        }
        mdx.push('\n');
    }

    mdx
}

/// 渲染每日文章摘要 MDX（所有去重后文章的列表页）
///
/// 输出到 output/daily/digest-{date}.mdx，供前端 Signal Feed 展示。
pub fn render_digest_mdx(articles: &[crate::fetcher::Article], today: &str) -> String {
    let mut mdx = String::new();
    mdx.push_str("---\n");
    mdx.push_str(&format!("title: \"Daily Signal Digest — {}\"\n", today));
    mdx.push_str(&format!("date: \"{}\"\n", today));
    mdx.push_str("type: digest\n");
    mdx.push_str(&format!("article_count: {}\n", articles.len()));
    mdx.push_str("---\n\n");

    mdx.push_str(&format!(
        "## Signal Feed — {} articles\n\n",
        articles.len()
    ));

    for article in articles {
        let source = yaml_escape(&article.source);
        let title = yaml_escape(&article.title);
        let summary = article
            .summary
            .as_deref()
            .or(article.content.as_deref())
            .map(|s| {
                let trimmed = s.trim();
                if trimmed.len() > 160 {
                    format!("{}…", &trimmed[..160])
                } else {
                    trimmed.to_string()
                }
            })
            .unwrap_or_default();
        let date_str = article
            .published_at
            .map(|d| d.format("%m-%d").to_string())
            .unwrap_or_default();

        mdx.push_str(&format!(
            "### [{title}]({url})\n\n**{source}** · {date_str}\n\n{summary}\n\n---\n\n",
            title = title,
            url = article.url,
            source = source,
            date_str = date_str,
            summary = summary,
        ));
    }

    mdx
}

/// 渲染复盘反思 MDX
pub fn render_reflection_mdx(reflection: &Reflection, thesis_title: &str) -> String {
    let _slug = format!("reflection-{}", reflection.id.replace(':', "-"));

    let mut mdx = String::new();
    mdx.push_str("---\n");
    mdx.push_str(&format!(
        "title: \"Reflection: {}\"\n",
        yaml_escape(thesis_title)
    ));
    mdx.push_str(&format!("date: \"{}\"\n", reflection.created_at));
    mdx.push_str("type: reflection\n");
    mdx.push_str(&format!("thesis_ref: {}\n", yaml_escape(thesis_title)));
    mdx.push_str(&format!("verdict: \"{}\"\n", reflection.verdict));
    mdx.push_str(&format!(
        "confidence_at_creation: {:.2}\n",
        reflection.confidence_at_creation
    ));
    mdx.push_str(&format!(
        "confidence_now: {:.2}\n",
        reflection.confidence_now
    ));
    mdx.push_str("lessons:\n");
    for l in &reflection.lessons {
        mdx.push_str(&format!("  - {}\n", yaml_escape(l)));
    }
    mdx.push_str("---\n\n");

    mdx.push_str(&format!(
        "## Error Analysis\n\n{}\n\n",
        reflection.error_reason
    ));

    if !reflection.lessons.is_empty() {
        mdx.push_str("## Lessons Learned\n\n");
        for l in &reflection.lessons {
            mdx.push_str(&format!("- {}\n", l));
        }
        mdx.push('\n');
    }

    mdx
}
