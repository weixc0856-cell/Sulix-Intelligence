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

use crate::domain::decision::ThesisDecision;
use crate::domain::evidence::Stance;
use crate::domain::investigation::InvestigationReport;
use crate::domain::outcome::Outcome;
use crate::domain::reflection::Reflection;
use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::domain::thesis::{LifecycleEventKind, Thesis};
use crate::domain::EditorNote;
use crate::domain::PremiumReport;
use crate::renderer::helpers::yaml_escape;

/// 渲染每日信号 MDX
///
/// 每个 theme 生成一个文件，包含：
/// - YAML frontmatter: title, date, locale, svi, asi, confidence, sources, entities, related_thesis
/// - 正文: BLUF, Thesis, Evidence 表, Assumptions, Action
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_daily_mdx(
    theme: &Theme,
    analysis: &ThemeAnalysis,
    today: &str,
    locale: &str,
    asi: f64,
    confidence: f64,
    editor_notes: &[EditorNote],
    related_slug: Option<&str>,
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

    let known = crate::entity::known_entities();
    let entities: Vec<String> = analysis
        .fact_base
        .iter()
        .flat_map(|fb| fb.evidence.split_whitespace())
        .map(|w| w.to_uppercase())
        .filter(|w| known.contains(&w.as_str()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let entities_yaml = entities
        .iter()
        .map(|e| format!("  - {}", e))
        .collect::<Vec<_>>()
        .join("\n");

    // Editor notes for this theme
    let theme_notes: Vec<&EditorNote> = editor_notes
        .iter()
        .filter(|n| n.theme_title == theme.title)
        .collect();

    let mut mdx = String::new();

    // YAML frontmatter
    mdx.push_str("---\n");
    mdx.push_str(&format!("title: {}\n", yaml_escape(&theme.title)));
    mdx.push_str(&format!("date: \"{}\"\n", today));
    mdx.push_str(&format!("locale: \"{}\"\n", locale));
    mdx.push_str("type: daily\n");
    // StrategicDomain classification from theme title + analysis content
    let (daily_primary, daily_secondary) =
        crate::domain::StrategicDomain::classify(&format!("{} {}", theme.title, analysis.bluf));
    mdx.push_str(&format!(
        "primary_domain: \"{}\"\n",
        daily_primary.label().to_lowercase()
    ));
    if !daily_secondary.is_empty() {
        mdx.push_str("secondary_domains:\n");
        for sd in &daily_secondary {
            mdx.push_str(&format!("  - \"{}\"\n", sd.label().to_lowercase()));
        }
    }
    mdx.push_str(&format!("svi: {}\n", analysis.signal_strength));
    mdx.push_str(&format!("asi: {:.2}\n", asi));
    mdx.push_str(&format!("confidence: {:.2}\n", confidence));
    if !sources.is_empty() {
        mdx.push_str(&format!("sources:\n{}\n", sources_yaml));
    }
    if !entities.is_empty() {
        mdx.push_str(&format!("entities:\n{}\n", entities_yaml));
    }
    if let Some(slug) = related_slug {
        mdx.push_str(&format!("related_thesis: \"{}\"\n", slug));
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
pub(crate) fn render_thesis_mdx(
    thesis: &Thesis,
    outcomes: &[Outcome],
    decision: Option<&ThesisDecision>,
    decision_record: Option<&crate::domain::DecisionRecord>,
    locale: &str,
) -> String {
    let _slug = thesis
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .replace(' ', "-");

    // Thesis status string for frontmatter
    let status_str = match thesis.status {
        crate::domain::thesis::ThesisStatus::Proposed => "proposed",
        crate::domain::thesis::ThesisStatus::Active => "active",
        crate::domain::thesis::ThesisStatus::Strengthening => "strengthening",
        crate::domain::thesis::ThesisStatus::Weakening => "weakening",
        crate::domain::thesis::ThesisStatus::Dormant => "dormant",
        crate::domain::thesis::ThesisStatus::Retired => "retired",
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

    let confidence = crate::domain::compute_confidence(&thesis.evidences);

    // 计算近 7 天新增证据数（用于前端"Why Now?"）
    let today_date = chrono::NaiveDate::parse_from_str(&thesis.updated, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::Local::now().date_naive());
    let evidences_recent = thesis
        .evidences
        .iter()
        .filter(|e| {
            chrono::NaiveDate::parse_from_str(&e.date, "%Y-%m-%d")
                .is_ok_and(|d| (today_date - d).num_days() <= 7)
        })
        .count();

    // 计算置信度 delta（vs 7 天前，用于 What Changed）
    let week_ago = today_date - chrono::Duration::days(7);
    let old_confidence = thesis
        .confidence_history
        .iter()
        .rfind(|snap| {
            chrono::NaiveDate::parse_from_str(&snap.date, "%Y-%m-%d").is_ok_and(|d| d <= week_ago)
        })
        .map(|snap| snap.value)
        .unwrap_or(confidence);
    let confidence_delta = confidence - old_confidence;

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
    mdx.push_str(&format!("created: \"{}\"\n", thesis.created));
    mdx.push_str(&format!("locale: \"{}\"\n", locale));
    mdx.push_str("type: thesis\n");
    mdx.push_str(&format!(
        "primary_domain: \"{}\"\n",
        thesis.primary_domain.label().to_lowercase()
    ));
    if !thesis.secondary_domains.is_empty() {
        mdx.push_str("secondary_domains:\n");
        for sd in &thesis.secondary_domains {
            mdx.push_str(&format!("  - \"{}\"\n", sd.label().to_lowercase()));
        }
    }
    if let Some(ref asm_id) = thesis.assessment_id {
        mdx.push_str(&format!("assessment_id: \"{}\"\n", asm_id));
    }
    if let Some(dec) = decision_record {
        mdx.push_str(&format!("dec_id: \"{}\"\n", dec.id));
    }
    if let Some(ref inv_id) = thesis.investigation_id {
        mdx.push_str(&format!("inv_id: \"{}\"\n", inv_id));
    }
    // 管理生命周期事件（最近 5 条，逆序）
    if !thesis.lifecycle_events.is_empty() {
        mdx.push_str("lifecycle:\n");
        for ev in thesis.lifecycle_events.iter().rev().take(5) {
            let event_str = match &ev.kind {
                LifecycleEventKind::Created => "Created".to_string(),
                LifecycleEventKind::Updated { note } => format!("Updated: {}", note),
                LifecycleEventKind::Merged { into } => format!("Merged into {}", into),
                LifecycleEventKind::Archived { reason } => format!("Archived: {}", reason),
                LifecycleEventKind::Invalidated { reason } => format!("Invalidated: {}", reason),
            };
            mdx.push_str(&format!(
                "  - date: \"{}\"\n    event: {}\n",
                ev.date,
                yaml_escape(&event_str)
            ));
        }
    }
    mdx.push_str(&format!("summary: {}\n", yaml_escape(summary)));
    mdx.push_str(&format!("status: \"{}\"\n", status_str));
    mdx.push_str(&format!("confidence: {:.2}\n", confidence));
    mdx.push_str(&format!("evidences: {}\n", support));
    mdx.push_str(&format!("challenges: {}\n", challenge));
    if evidences_recent > 0 {
        mdx.push_str(&format!("evidences_recent: {}\n", evidences_recent));
    }
    if confidence_delta.abs() > 0.005 {
        mdx.push_str(&format!("confidence_delta: {:.3}\n", confidence_delta));
    }
    // 决策连续天数（Stability Layer：前端显示 "Stable N days"）
    if !thesis.decision_history.is_empty() {
        let last_type = thesis
            .decision_history
            .last()
            .map(|s| s.decision_type.as_str())
            .unwrap_or("");
        let decision_days = thesis
            .decision_history
            .iter()
            .rev()
            .take_while(|s| s.decision_type == last_type)
            .count();
        if decision_days >= 2 {
            mdx.push_str(&format!("decision_days: {}\n", decision_days));
        }
    }
    // 来源归因（top-3 不重复来源，供前端"4 independent sources"展示）
    {
        let mut seen = std::collections::HashSet::new();
        let unique_sources: Vec<String> = thesis
            .evidences
            .iter()
            .filter_map(|e| {
                let s = e.source.trim().to_string();
                if !s.is_empty() && seen.insert(s.clone()) {
                    Some(s)
                } else {
                    None
                }
            })
            .take(3)
            .collect();
        if !unique_sources.is_empty() {
            mdx.push_str("sources:\n");
            for s in &unique_sources {
                mdx.push_str(&format!("  - {}\n", yaml_escape(s)));
            }
        }
    }
    // 承重假设（Key Judgements 用，暴露到 frontmatter 供前端 Key Judgements 区块）
    if !thesis.assumptions.is_empty() {
        mdx.push_str("assumptions:\n");
        for a in thesis.assumptions.iter().take(3) {
            mdx.push_str(&format!(
                "  - text: {}\n    load_bearing: {}\n    evidence_strength: {}\n",
                yaml_escape(&a.text),
                a.load_bearing,
                yaml_escape(&a.evidence_strength)
            ));
        }
    }
    // 证伪条件（First Principle: Falsifiability）
    if !thesis.falsification_conditions.is_empty() {
        mdx.push_str("falsification_conditions:\n");
        for fc in &thesis.falsification_conditions {
            mdx.push_str(&format!("  - {}\n", yaml_escape(fc)));
        }
    }
    // Decision Intelligence frontmatter
    if let Some(dec) = decision {
        mdx.push_str(&format!("decision: \"{}\"\n", dec.decision_type.as_key()));
        mdx.push_str(&format!(
            "decision_rationale: {}\n",
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
            .filter(|o| o.verdict == crate::domain::outcome::OutcomeVerdict::Confirmed)
            .count();
        let partial = outcomes
            .iter()
            .filter(|o| o.verdict == crate::domain::outcome::OutcomeVerdict::PartiallyConfirmed)
            .count();
        let invalidated = outcomes
            .iter()
            .filter(|o| o.verdict == crate::domain::outcome::OutcomeVerdict::Invalidated)
            .count();
        let accuracy = (confirmed as f64 + partial as f64 * 0.5) / total as f64;
        mdx.push_str(&format!("outcome_total: {}\n", total));
        mdx.push_str(&format!("outcome_confirmed: {}\n", confirmed));
        mdx.push_str(&format!("outcome_partial: {}\n", partial));
        mdx.push_str(&format!("outcome_invalidated: {}\n", invalidated));
        mdx.push_str(&format!("historical_accuracy: {:.2}\n", accuracy));
    }
    // Revision History: 最近 5 条置信度快照（前端 Revision History 展示用）
    {
        let recent_confidence: Vec<_> = thesis.confidence_history.iter().rev().take(5).collect();
        if !recent_confidence.is_empty() {
            mdx.push_str("confidence_history:\n");
            for snap in &recent_confidence {
                mdx.push_str(&format!(
                    "  - date: \"{}\"\n    value: {:.2}\n    reason: {}\n",
                    snap.date,
                    snap.value,
                    yaml_escape(&snap.reason)
                ));
            }
        }
    }
    // Decision History: 决策变化事件（去掉相邻重复，保留真正的切换）
    // Assessment Ledger 用：提供决策时间线
    {
        let mut last_decision: Option<String> = None;
        let decision_changes: Vec<_> = thesis
            .decision_history
            .iter()
            .filter(|snap| {
                if Some(&snap.decision_type) != last_decision.as_ref() {
                    last_decision = Some(snap.decision_type.clone());
                    true
                } else {
                    false
                }
            })
            .collect();
        if !decision_changes.is_empty() {
            mdx.push_str("decision_changes:\n");
            for snap in &decision_changes {
                mdx.push_str(&format!(
                    "  - date: \"{}\"\n    decision: \"{}\"\n    confidence: {:.2}\n",
                    snap.date, snap.decision_type, snap.confidence
                ));
            }
        }
        // 保留 decision_history_recent 作为向后兼容
        let recent_decisions: Vec<_> = thesis.decision_history.iter().rev().take(3).collect();
        if !recent_decisions.is_empty() {
            mdx.push_str("decision_history_recent:\n");
            for snap in &recent_decisions {
                mdx.push_str(&format!(
                    "  - date: \"{}\"\n    decision: \"{}\"\n    confidence: {:.2}\n",
                    snap.date, snap.decision_type, snap.confidence
                ));
            }
        }
    }
    // Evidence Attribution — 支持/反对证据摘要（Evidence 区块详细化用）
    {
        let supporting: Vec<String> = thesis
            .evidences
            .iter()
            .filter(|e| e.stance == Stance::Supports)
            .rev()
            .take(5)
            .map(|e| format!("[{}] {}", e.source, e.summary))
            .collect();
        if !supporting.is_empty() {
            mdx.push_str("supporting_evidence:\n");
            for e in &supporting {
                mdx.push_str(&format!("  - {}\n", yaml_escape(e)));
            }
        }
        let conflicting: Vec<String> = thesis
            .evidences
            .iter()
            .filter(|e| e.stance == Stance::Challenges)
            .rev()
            .take(3)
            .map(|e| format!("[{}] {}", e.source, e.summary))
            .collect();
        if !conflicting.is_empty() {
            mdx.push_str("conflicting_evidence:\n");
            for e in &conflicting {
                mdx.push_str(&format!("  - {}\n", yaml_escape(e)));
            }
        }
    }
    // ── Revision History (Git-style unified timeline) ──
    {
        let revisions = crate::domain::revision::build_revision_history(thesis);
        let meaningful: Vec<_> = revisions.iter().filter(|v| v.is_meaningful()).collect();
        if !meaningful.is_empty() {
            mdx.push_str("revision_history:\n");
            for v in meaningful.iter().rev().take(10) {
                mdx.push_str(&format!(
                    "  - version: {}\n    date: \"{}\"\n    confidence: {:.1}\n",
                    v.version, v.date, v.confidence
                ));
                if let Some(delta) = v.confidence_delta {
                    mdx.push_str(&format!("    confidence_delta: {:.1}\n", delta));
                }
                if let Some((ref from, ref to)) = v.decision_change {
                    mdx.push_str(&format!("    decision_change: \"{}->{}\"\n", from, to));
                }
                if let Some((ref _from, ref to)) = v.status_change {
                    mdx.push_str(&format!("    status_change: \"{}\"\n", yaml_escape(to)));
                }
                if !v.evidence_added.is_empty() {
                    mdx.push_str(&format!("    evidence_added: {}\n", v.evidence_added.len()));
                }
                if !v.challenges_added.is_empty() {
                    mdx.push_str(&format!(
                        "    challenges_added: {}\n",
                        v.challenges_added.len()
                    ));
                }
                mdx.push_str(&format!("    summary: {}\n", yaml_escape(&v.summary())));
            }
        }
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
                crate::domain::outcome::OutcomeVerdict::Confirmed => "✅",
                crate::domain::outcome::OutcomeVerdict::PartiallyConfirmed => "🟡",
                crate::domain::outcome::OutcomeVerdict::Invalidated => "❌",
                crate::domain::outcome::OutcomeVerdict::Unknown => "❓",
            };
            mdx.push_str(&format!("- {} {}: {}\n", icon, o.date, o.description));
        }
        mdx.push('\n');
    }

    mdx
}

/// 渲染 Premium 研报 MDX
pub(crate) fn render_research_mdx(report: &PremiumReport, locale: &str) -> String {
    let _slug = report
        .theme_title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .replace(' ', "-");

    let mut mdx = String::new();
    mdx.push_str("---\n");
    mdx.push_str(&format!("title: {}\n", yaml_escape(&report.theme_title)));
    mdx.push_str(&format!("date: \"{}\"\n", report.date));
    mdx.push_str(&format!("locale: \"{}\"\n", locale));
    mdx.push_str(&format!("stage: \"{}\"\n", report.stage));
    mdx.push_str(&format!("is_premium: {}\n", report.is_premium));
    mdx.push_str("type: research\n");
    let (research_primary, research_secondary) = crate::domain::StrategicDomain::classify(
        &format!("{} {}", report.theme_title, report.executive_summary),
    );
    mdx.push_str(&format!(
        "primary_domain: \"{}\"\n",
        research_primary.label().to_lowercase()
    ));
    if !research_secondary.is_empty() {
        mdx.push_str("secondary_domains:\n");
        for sd in &research_secondary {
            mdx.push_str(&format!("  - \"{}\"\n", sd.label().to_lowercase()));
        }
    }
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
pub(crate) fn render_digest_mdx(
    articles: &[crate::fetcher::Article],
    today: &str,
    locale: &str,
) -> String {
    let mut mdx = String::new();
    mdx.push_str("---\n");
    mdx.push_str(&format!("title: \"Daily Signal Digest — {}\"\n", today));
    mdx.push_str(&format!("date: \"{}\"\n", today));
    mdx.push_str(&format!("locale: \"{}\"\n", locale));
    mdx.push_str("type: digest\n");
    mdx.push_str(&format!("article_count: {}\n", articles.len()));
    mdx.push_str("---\n\n");

    mdx.push_str(&format!("## Signal Feed — {} articles\n\n", articles.len()));

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
pub(crate) fn render_reflection_mdx(
    reflection: &Reflection,
    thesis_title: &str,
    assessment_id: Option<&str>,
    locale: &str,
) -> String {
    let _slug = format!("reflection-{}", reflection.id.replace(':', "-"));

    let mut mdx = String::new();
    mdx.push_str("---\n");
    let reflection_title = format!("Reflection: {}", thesis_title);
    mdx.push_str(&format!("title: {}\n", yaml_escape(&reflection_title)));
    mdx.push_str(&format!("date: \"{}\"\n", reflection.created_at));
    mdx.push_str(&format!("locale: \"{}\"\n", locale));
    mdx.push_str("type: reflection\n");
    mdx.push_str(&format!(
        "primary_domain: \"{}\"\n",
        reflection.primary_domain.label().to_lowercase()
    ));
    let thesis_ref = assessment_id.unwrap_or(thesis_title);
    mdx.push_str(&format!("thesis_ref: {}\n", yaml_escape(thesis_ref)));
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

/// 渲染 Investigation Report MDX — "为什么相信这个判断"
///
/// 结构：Core Question → Supporting Evidence → Counter Evidence
///       → Key Unknowns → Falsification Conditions → Preliminary Conclusion
/// 输出到 output/investigation/{slug}.md
pub(crate) fn render_investigation_mdx(
    report: &InvestigationReport,
    slug: &str,
    assessment_id: Option<&str>,
    inv_id: Option<&str>,
    locale: &str,
) -> String {
    let mut mdx = String::new();
    mdx.push_str("---\n");
    mdx.push_str(&format!(
        "title: {}\n",
        yaml_escape(&format!("Investigation: {}", report.thesis_title))
    ));
    mdx.push_str(&format!("date: \"{}\"\n", report.date));
    mdx.push_str(&format!("locale: \"{}\"\n", locale));
    mdx.push_str("type: investigation\n");
    mdx.push_str(&format!(
        "primary_domain: \"{}\"\n",
        report.primary_domain.label().to_lowercase()
    ));
    if let Some(id) = inv_id {
        mdx.push_str(&format!("inv_id: \"{}\"\n", id));
    }
    mdx.push_str(&format!("status: \"{}\"\n", report.status));
    mdx.push_str(&format!(
        "question: {}\n",
        yaml_escape(&report.core_question)
    ));
    // thesis_ref: 优先用稳定的 ASM-ID，fallback 到 title-derived slug
    let thesis_ref = assessment_id.unwrap_or(slug);
    mdx.push_str(&format!("thesis_ref: {}\n", yaml_escape(thesis_ref)));
    if !report.supporting_evidence.is_empty() {
        mdx.push_str(&format!(
            "supporting_count: {}\n",
            report.supporting_evidence.len()
        ));
    }
    if !report.counter_evidence.is_empty() {
        mdx.push_str(&format!(
            "counter_count: {}\n",
            report.counter_evidence.len()
        ));
    }
    mdx.push_str("---\n\n");

    mdx.push_str(&format!("## Core Question\n\n{}\n\n", report.core_question));

    if !report.supporting_evidence.is_empty() {
        mdx.push_str("## Supporting Evidence\n\n");
        for e in &report.supporting_evidence {
            mdx.push_str(&format!("- {}\n", e));
        }
        mdx.push('\n');
    }

    if !report.counter_evidence.is_empty() {
        mdx.push_str("## Counter Evidence\n\n");
        for e in &report.counter_evidence {
            mdx.push_str(&format!("- {}\n", e));
        }
        mdx.push('\n');
    }

    if !report.key_unknowns.is_empty() {
        mdx.push_str("## Key Unknowns\n\n");
        for u in &report.key_unknowns {
            mdx.push_str(&format!("- {}\n", u));
        }
        mdx.push('\n');
    }

    if !report.falsification_conditions.is_empty() {
        mdx.push_str("## Falsification Conditions\n\n");
        for fc in &report.falsification_conditions {
            mdx.push_str(&format!("- {}\n", fc));
        }
        mdx.push('\n');
    }

    mdx.push_str("## Preliminary Conclusion\n\n");
    mdx.push_str(&format!("{}\n", report.preliminary_conclusion));

    mdx
}

/// 渲染 canonical Decision MDX (DEC-XXXX standalone file)
///
/// 输出到 output/decision/DEC-XXXX.md
pub(crate) fn render_decision_mdx(dec: &crate::domain::DecisionRecord, locale: &str) -> String {
    let mut mdx = String::new();
    mdx.push_str("---\n");
    mdx.push_str(&format!(
        "title: {}\n",
        yaml_escape(&format!(
            "Decision {}: {}",
            dec.id,
            dec.decision_type.to_uppercase()
        ))
    ));
    mdx.push_str(&format!("dec_id: \"{}\"\n", dec.id));
    mdx.push_str(&format!("asm_id: \"{}\"\n", dec.asm_id));
    mdx.push_str(&format!("decision: \"{}\"\n", dec.decision_type));
    mdx.push_str(&format!("horizon: \"{}\"\n", dec.horizon));
    mdx.push_str(&format!("confidence: {:.2}\n", dec.confidence));
    mdx.push_str(&format!("stability: \"{}\"\n", dec.stability));
    let state_str = match &dec.state {
        crate::domain::DecisionState::Active => "active".to_string(),
        crate::domain::DecisionState::Archived { reason } => format!("archived: {}", reason),
        crate::domain::DecisionState::Superseded { by } => format!("superseded-by: {}", by),
        crate::domain::DecisionState::Expired => "expired".to_string(),
    };
    mdx.push_str(&format!("state: \"{}\"\n", state_str));
    mdx.push_str(&format!("created: \"{}\"\n", dec.created));
    mdx.push_str(&format!("updated: \"{}\"\n", dec.updated));
    mdx.push_str(&format!("locale: \"{}\"\n", locale));
    mdx.push_str("type: decision\n");
    mdx.push_str(&format!(
        "primary_domain: \"{}\"\n",
        dec.primary_domain.label().to_lowercase()
    ));
    mdx.push_str(&format!("rationale: {}\n", yaml_escape(&dec.rationale)));
    if !dec.decision_history.is_empty() {
        mdx.push_str("transitions:\n");
        for t in &dec.decision_history {
            mdx.push_str(&format!(
                "  - date: \"{}\"\n    from: \"{}\"\n    to: \"{}\"\n    confidence: {:.2}\n",
                t.date, t.from, t.to, t.confidence
            ));
        }
    }
    mdx.push_str("---\n\n");
    mdx.push_str(&format!("## Decision {}\n\n", dec.id));
    mdx.push_str(&format!(
        "**{}** — Linked to [{}]\n\n",
        dec.decision_type.to_uppercase(),
        dec.asm_id
    ));
    mdx.push_str(&format!(
        "Confidence: {:.0}%  |  Horizon: {}  |  Stability: {}\n\n",
        dec.confidence * 100.0,
        dec.horizon,
        dec.stability
    ));
    mdx.push_str(&format!("> {}\n\n", dec.rationale));
    if dec.decision_history.len() > 1 {
        mdx.push_str("## Transition History\n\n");
        for t in &dec.decision_history {
            mdx.push_str(&format!(
                "- `{}` {} → {} ({:.0}% confidence)\n",
                t.date,
                t.from,
                t.to,
                t.confidence * 100.0
            ));
        }
        mdx.push('\n');
    }
    mdx
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::domain::decision::DecisionRecord;
    use crate::domain::investigation::InvestigationReport;
    use crate::domain::reflection::Reflection;
    use crate::domain::PremiumReport;

    #[test]
    fn test_research_mdx_frontmatter() {
        let report = PremiumReport {
            theme_title: "AI Governance 2026".into(),
            date: "2026-06-26".into(),
            executive_summary: "Summary text".into(),
            geopolitical_assessment: "Geo text".into(),
            technical_impact: "Tech text".into(),
            commercial_framework: "Commerce text".into(),
            risk_scenarios: vec!["Risk 1".into()],
            sources: vec!["Source 1".into()],
            stage: "what-to-do".into(),
            is_premium: true,
        };
        let mdx = render_research_mdx(&report, "en");
        assert!(mdx.starts_with("---\n"));
        assert!(mdx.contains("title: AI Governance 2026"));
        assert!(mdx.contains("date: \"2026-06-26\""));
        assert!(mdx.contains("stage: \"what-to-do\""));
        assert!(mdx.contains("is_premium: true"));
        assert!(mdx.contains("---\n\n"));
        assert!(mdx.contains("Executive Summary"));
        assert!(mdx.contains("Summary text"));
    }

    #[test]
    fn test_reflection_mdx_frontmatter() {
        let reflection = Reflection {
            id: "ref:test-001".into(),
            thesis_id: "thesis-001".into(),
            outcome_id: "out-001".into(),
            verdict: "confirmed".into(),
            error_reason: "".into(),
            lessons: vec!["Lesson 1".into(), "Lesson 2".into()],
            confidence_at_creation: 0.75,
            confidence_now: 0.85,
            created_at: "2026-06-26".into(),
            primary_domain: crate::domain::StrategicDomain::default(),
            secondary_domains: vec![],
        };
        let mdx = render_reflection_mdx(&reflection, "Test Thesis", None, "en");
        assert!(mdx.starts_with("---\n"));
        assert!(mdx.contains("title: \"Reflection: Test Thesis\""));
        assert!(mdx.contains("date: \"2026-06-26\""));
        assert!(mdx.contains("thesis_ref: Test Thesis"));
        assert!(mdx.contains("verdict: \"confirmed\""));
        assert!(mdx.contains("confidence_at_creation: 0.75"));
        assert!(mdx.contains("confidence_now: 0.85"));
        assert!(mdx.contains("type: reflection"));
    }

    #[test]
    fn test_investigation_mdx_frontmatter() {
        let report = InvestigationReport {
            thesis_id: "thesis-001".into(),
            thesis_title: "AI Governance".into(),
            date: "2026-06-26".into(),
            core_question: "Will AI governance frameworks converge?".into(),
            supporting_evidence: vec!["EU AI Act enacted".into()],
            counter_evidence: vec!["US lags behind".into()],
            key_unknowns: vec!["China approach unclear".into()],
            falsification_conditions: vec!["No convergence by 2027".into()],
            preliminary_conclusion: "Likely to converge".into(),
            status: "active".to_string(),
            primary_domain: crate::domain::StrategicDomain::default(),
            secondary_domains: vec![],
        };
        let mdx =
            render_investigation_mdx(&report, "test-slug", Some("ASM-001"), Some("INV-001"), "en");
        assert!(mdx.starts_with("---\n"));
        assert!(mdx.contains("title: \"Investigation: AI Governance\""));
        assert!(mdx.contains("date: \"2026-06-26\""));
        assert!(mdx.contains("inv_id: \"INV-001\""));
        assert!(mdx.contains("status: \"active\""));
        assert!(mdx.contains("question: Will AI governance frameworks converge?"));
        assert!(mdx.contains("thesis_ref: ASM-001"));
        assert!(mdx.contains("supporting_count: 1"));
        assert!(mdx.contains("counter_count: 1"));
    }

    #[test]
    fn test_decision_mdx_frontmatter() {
        let dec = DecisionRecord {
            id: "DEC-001".into(),
            asm_id: "ASM-001".into(),
            thesis_id: "thesis-001".into(),
            decision_type: "build".into(),
            horizon: "90d".into(),
            confidence: 0.82,
            stability: "stable".into(),
            rationale: "Strong evidence base".into(),
            state: crate::domain::DecisionState::Active,
            created: "2026-06-01".into(),
            updated: "2026-06-26".into(),
            outcome_ids: vec![],
            decision_history: vec![],
            primary_domain: crate::domain::StrategicDomain::default(),
            secondary_domains: vec![],
        };
        let mdx = render_decision_mdx(&dec, "en");
        assert!(mdx.starts_with("---\n"));
        assert!(mdx.contains("title: \"Decision DEC-001: BUILD\""));
        assert!(mdx.contains("dec_id: \"DEC-001\""));
        assert!(mdx.contains("asm_id: \"ASM-001\""));
        assert!(mdx.contains("decision: \"build\""));
        assert!(mdx.contains("horizon: \"90d\""));
        assert!(mdx.contains("confidence: 0.82"));
        assert!(mdx.contains("stability: \"stable\""));
        assert!(mdx.contains("state: \"active\""));
        assert!(mdx.contains("created: \"2026-06-01\""));
        assert!(mdx.contains("updated: \"2026-06-26\""));
        assert!(mdx.contains("rationale: Strong evidence base"));
    }
}
