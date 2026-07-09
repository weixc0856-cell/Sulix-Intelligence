//! Stage 3: Infer — Memory Engine, Hermes, Outcome detection, Decision Intelligence

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;

use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::config::Config;
use crate::db::Database;
use crate::domain::evidence::Stance;
use crate::domain::outcome::{ImpactLevel, Outcome, OutcomeVerdict};
use crate::domain::thesis::ThesisStatus;
use crate::domain::StrategicDomain;
use crate::domain::ThesisDecision;
use crate::event_log::ObjectEvent;
use crate::engine::decision::map_theses_to_decisions;
use crate::engine::investigation::generate_investigation;
use crate::engine::memory::MemoryEngine;
use crate::archive::{ChronicleDb, ChronicleEntry};

use super::preprocess::StateBundle;
use super::generate::GeneratedAssets;
use super::helpers::extract_entities;

/// Stage 3: Infer 阶段产出的认知状态
pub struct InferredState {
    pub memory: MemoryEngine,
    pub thesis_decisions: Vec<ThesisDecision>,
    pub premium_reports: Vec<crate::domain::PremiumReport>,
    pub asi_score_map: HashMap<String, (f64, f64, f64)>,
    pub editor_notes: Vec<crate::domain::EditorNote>,
    pub beliefs_html: String,
    pub investigation_reports: Vec<(String, crate::domain::investigation::InvestigationReport, Option<String>, Option<String>)>,
    pub refined_domains: HashMap<String, (StrategicDomain, Vec<StrategicDomain>)>,
    pub events: Vec<ObjectEvent>,
}

static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);
const TREND_DAYS: i32 = 14;

// ===== Sub-functions =====

/// 将 Change Detection 发现的冲突推入 EventLog
fn push_conflict_events(state: &mut StateBundle, change_summary: &crate::hermes::ChangeSummary) {
    for conflict in &change_summary.conflicts {
        state.event_log.push(crate::event_log::PipelineEvent {
            id: format!("evt-{}", EVENT_COUNTER.fetch_add(1, Ordering::SeqCst)),
            event_type: crate::event_log::PipelineEventType::ConflictDetected,
            timestamp: chrono::Utc::now().to_rfc3339(),
            description: format!("{}: {}", conflict.topic, conflict.today_signal),
            thesis_id: state.memory_for_linking.find_by_title(&conflict.topic).map(|t| t.id.clone()),
            related_events: vec![],
            data: serde_json::json!({"topic": conflict.topic, "prior_belief": conflict.prior_belief}),
        });
    }
}

/// 将分析结果追加到 Chronicle（加载已有记录 + 新增 en/zh 条目）
#[allow(clippy::too_many_arguments)]
fn build_chronicle(
    chronicle_path: &std::path::Path,
    analyses: &[ThemeAnalysis],
    analyses_zh: &[ThemeAnalysis],
    today: &str,
) -> ChronicleDb {
    let mut chronicle = ChronicleDb::load(chronicle_path).unwrap_or_else(|_| ChronicleDb { entries: vec![] });
    for a in analyses_zh {
        chronicle.push(ChronicleEntry {
            date: today.to_string(), topic: a.theme_title.clone(), headline: a.bluf.clone(),
            entities: extract_entities(a), signal_strength: a.signal_strength, language: "zh".into(),
        });
    }
    for analysis in analyses.iter() {
        chronicle.push(ChronicleEntry {
            date: today.to_string(), topic: analysis.theme_title.clone(), headline: analysis.bluf.clone(),
            entities: extract_entities(analysis), signal_strength: analysis.signal_strength, language: "en".into(),
        });
    }
    chronicle
}

/// Memory Engine 加载 + Hermes 分析
#[allow(clippy::too_many_arguments)]
fn run_memory_engine(
    state: &mut StateBundle,
    today: &str,
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    chronicle: &ChronicleDb,
    db: &Database,
    change_summary: &crate::hermes::ChangeSummary,
    refined_domains: &HashMap<String, (StrategicDomain, Vec<StrategicDomain>)>,
) -> MemoryEngine {
    let mut memory = MemoryEngine::new(state.memory_path.clone());
    if let Err(e) = memory.load() {
        let backup = format!("{}.corrupt.{}.json",
            state.memory_path.to_string_lossy(), chrono::Utc::now().format("%Y%m%d_%H%M%S"));
        log::warn!("⚠️ Memory Engine 加载失败 ({}), 备份到 {} 后重建", e, backup);
        let _ = std::fs::rename(&state.memory_path, &backup);
    }

    if let Err(e) = memory.update_from_analysis_with_registry(today, themes, analyses, &mut state.registry) {
        log::warn!("⚠️ Memory Engine 更新失败: {}", e);
    } else {
        let before = memory.theses().len();
        if !change_summary.conflicts.is_empty() {
            crate::hermes::apply_conflicts(change_summary, &mut memory, today);
        }
        if let Ok(trends) = db.get_trend(TREND_DAYS) {
            crate::hermes::analyze_trends(&trends, &mut memory, today);
        }
        crate::hermes::discover_theses(analyses, chronicle, &mut memory, today);
        log::info!("🧠 Memory Engine: {} 个 Thesis (Hermes: {} 新增)", memory.theses().len(), memory.theses().len() - before);

        if !refined_domains.is_empty() {
            for thesis in memory.theses_mut() {
                if let Some((primary, secondary)) = refined_domains.get(&thesis.title) {
                    thesis.primary_domain = *primary;
                    thesis.secondary_domains = secondary.clone();
                }
            }
        }
    }
    memory
}

/// Investigation Engine: 为活跃 Thesis 生成调研问题
async fn run_investigation_engine(
    memory: &mut MemoryEngine,
    state: &mut StateBundle,
    api_key: &str,
    config: &Config,
    today: &str,
) -> Vec<(String, crate::domain::investigation::InvestigationReport, Option<String>, Option<String>)> {
    for thesis in memory.theses().to_owned() {
        if !matches!(thesis.status, ThesisStatus::Active | ThesisStatus::Strengthening) { continue; }
        let Some(ref asm_id) = thesis.assessment_id else { continue; };
        if !memory.should_regenerate_investigation(&thesis.id) { continue; }
        match generate_investigation(&thesis, api_key, &config.llm, config.prompts.as_ref()).await {
            Ok(mut inv) => {
                let inv_id = if let Some(old_id) = state.inv_registry.find_active_by_asm(asm_id) {
                    state.inv_registry.supersede_and_register(&old_id, asm_id, &thesis.id, today)
                } else {
                    state.inv_registry.register(asm_id, &thesis.id, today)
                };
                inv.id = inv_id.clone();
                memory.upsert_investigation(inv);
                memory.set_investigation_id(&thesis.id, &inv_id);
                log::info!("🔍 Investigation {} generated for ASM {}", inv_id, asm_id);
            }
            Err(e) => log::warn!("Investigation gen failed [{}]: {}", thesis.title, e),
        }
    }

    // Build investigation reports for Emit stage
    let mut investigation_reports = Vec::new();
    for thesis in memory.theses() {
        if !matches!(thesis.status, ThesisStatus::Active | ThesisStatus::Strengthening | ThesisStatus::Weakening) { continue; }
        let slug = {
            let slug_base = crate::renderer::publisher::ascii_slug(&thesis.title);
            if slug_base.is_empty() { crate::renderer::publisher::short_id_from_thesis(&thesis.id) } else { slug_base }
        };
        let report = crate::engine::investigation::derive_investigation_report(thesis, today, None);
        investigation_reports.push((slug, report, thesis.assessment_id.clone(), thesis.investigation_id.clone()));
    }
    investigation_reports
}

/// Meta Layer: Outcome 检测 + Reflection 生成（Retired→Invalidated, Strengthening→PartiallyConfirmed）
fn detect_outcomes(
    memory: &mut MemoryEngine,
    today: &str,
    state: &mut StateBundle,
    infer_events: &mut Vec<ObjectEvent>,
) -> String {
    for thesis in memory.theses().to_owned() {
        if thesis.status == ThesisStatus::Retired {
            let challenge = thesis.evidences.iter().filter(|e| e.stance == Stance::Challenges).count();
            let support = thesis.evidences.iter().filter(|e| e.stance == Stance::Supports).count();
            if challenge > support {
                let (outcome, event) = Outcome::new(
                    format!("outcome-{}", chrono::Utc::now().timestamp()), String::new(),
                    thesis.id.clone(), format!("被证伪: 挑战证据 ({}) 超过支持证据 ({})", challenge, support),
                    OutcomeVerdict::Invalidated, ImpactLevel::Medium, today.to_string(),
                );
                infer_events.push(event);
                if let Err(e) = memory.record_outcome(outcome) {
                    log::warn!("⚠️ Outcome 记录失败: {}", e);
                } else {
                    state.event_log.push(crate::event_log::PipelineEvent {
                        id: format!("evt-{}", EVENT_COUNTER.fetch_add(1, Ordering::SeqCst)),
                        event_type: crate::event_log::PipelineEventType::ThesisRefuted,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        description: format!("论题 '{}' 被证伪", thesis.title),
                        thesis_id: Some(thesis.id.clone()), related_events: vec![],
                        data: serde_json::json!({"thesis_title": thesis.title, "support": support, "challenge": challenge}),
                    });
                    log::info!("🧠 Meta Layer: Thesis '{}' → Invalidated (S={}, C={})", thesis.title, support, challenge);
                }
            }
        }
        if thesis.status == ThesisStatus::Strengthening && thesis.evidences.len() >= 2 {
            let (outcome, event) = Outcome::new(
                format!("outcome-{}", chrono::Utc::now().timestamp()), String::new(),
                thesis.id.clone(), format!("证据持续积累 ({} 条)", thesis.evidences.len()),
                OutcomeVerdict::PartiallyConfirmed, ImpactLevel::Medium, today.to_string(),
            );
            infer_events.push(event);
            if let Err(e) = memory.record_outcome(outcome) {
                log::warn!("⚠️ Outcome 记录失败: {}", e);
            } else {
                state.event_log.push(crate::event_log::PipelineEvent {
                    id: format!("evt-{}", EVENT_COUNTER.fetch_add(1, Ordering::SeqCst)),
                    event_type: crate::event_log::PipelineEventType::OutcomeRecorded,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    description: format!("论题 '{}' 获得证据强化", thesis.title),
                    thesis_id: Some(thesis.id.clone()), related_events: vec![],
                    data: serde_json::json!({"thesis_title": thesis.title, "evidence_count": thesis.evidences.len()}),
                });
                log::info!("🧠 Meta Layer: Thesis '{}' → Strengthening ({} evidence)", thesis.title, thesis.evidences.len());
            }
        }
    }

    // Outcome notifications HTML
    let recent_outcomes: Vec<_> = memory.all_outcomes().iter().rev().take(3).map(|o| {
        let icon = match o.verdict {
            OutcomeVerdict::Confirmed => "✅",
            OutcomeVerdict::PartiallyConfirmed => "🟡",
            OutcomeVerdict::Invalidated => "❌",
            OutcomeVerdict::Unknown => "❓",
        };
        format!(r#"<div style="display:flex;align-items:flex-start;gap:0.5rem;padding:0.375rem 0;border-bottom:1px solid #f0f0f0;font-size:0.75rem"><span>{}</span><div><strong>{}</strong></div></div>"#, icon, o.description)
    }).collect();
    if recent_outcomes.is_empty() { String::new() } else {
        format!(r#"<div style="margin-top:0.75rem;padding:0.5rem;background:#fef2f2;border-radius:0.25rem;border-left:3px solid #ef4444">
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.6875rem;font-weight:700;text-transform:uppercase;letter-spacing:0.05em;color:#dc2626;margin-bottom:0.25rem">🎯 判断更新</div>
  {}</div>"#, recent_outcomes.join(""))
    }
}

// ===== Coordinator =====

/// Infer: Memory Engine 更新 + Hermes 分析 + Outcome 检测 + Decision Intelligence
#[allow(clippy::too_many_arguments)]
pub async fn publish_infer(
    config: &Config,
    api_key: &str,
    today: &str,
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    analyses_zh: &[ThemeAnalysis],
    new_articles: &[crate::fetcher::Article],
    generated: &GeneratedAssets,
    state: &mut StateBundle,
    db: &Database,
) -> Result<InferredState> {
    let mut infer_events: Vec<ObjectEvent> = Vec::new();

    // 1. Push conflict events to EventLog
    push_conflict_events(state, &generated.change_summary);

    // 2. Build Chronicle (load existing + append today's)
    let chronicle_path = state.chronicle_path.clone();
    let chronicle = build_chronicle(&chronicle_path, analyses, analyses_zh, today);

    // 3. Memory Engine + Hermes
    let mut memory = run_memory_engine(
        state, today, themes, analyses, &chronicle, db,
        &generated.change_summary, &generated.refined_domains,
    );

    // 4. Investigation Engine
    let investigation_reports = run_investigation_engine(
        &mut memory, state, api_key, config, today,
    ).await;

    // 5. Meta Layer: Outcomes
    let outcome_notifications_html = detect_outcomes(&mut memory, today, state, &mut infer_events);

    // 6. Decision Intelligence
    let thesis_decisions_raw = map_theses_to_decisions(&memory);
    let history_map = crate::engine::stability::build_decision_history_map(&memory);
    let consecutive_map = crate::engine::stability::build_consecutive_days_map(&memory);
    let thesis_decisions = crate::engine::stability::stability_gate(thesis_decisions_raw, &history_map, &consecutive_map);

    for d in &thesis_decisions {
        memory.record_decision(&d.thesis_id, today, d.decision_type.as_key(), d.confidence);
    }

    let high_priority: Vec<&ThesisDecision> = thesis_decisions.iter()
        .filter(|d| matches!(d.decision_type, crate::domain::action::DecisionType::Exit | crate::domain::action::DecisionType::Build))
        .collect();
    if !high_priority.is_empty() {
        log::info!("🧠 Decision Intelligence: {} 个高优先级决策", high_priority.len());
        for d in &high_priority {
            log::info!("  - {:?}: {} ({})", d.decision_type, d.thesis_title, d.rationale);
        }
    }

    // 7. Save chronicle
    chronicle.save(&chronicle_path)?;

    // 8. Decay Agent
    if let Some(ref g) = config.graveyard {
        if g.enabled {
            match crate::agent::decay::run_maintenance(db, new_articles, api_key, &config.llm, g).await {
                Ok(_) => log::info!("🪦 Decay Agent 维护完成"),
                Err(e) => log::warn!("⚠️ Decay Agent 失败: {}", e),
            }
        }
    }

    // 9. Append outcome notifications to beliefs HTML
    let mut beliefs_html = generated.belief_notes_html.clone();
    if !outcome_notifications_html.is_empty() {
        beliefs_html.push_str(&outcome_notifications_html);
    }

    Ok(InferredState {
        memory, thesis_decisions,
        premium_reports: generated.premium_reports.clone(),
        asi_score_map: generated.asi_score_map.clone(),
        editor_notes: generated.editor_notes.clone(),
        beliefs_html, investigation_reports,
        events: infer_events,
        refined_domains: generated.refined_domains.clone(),
    })
}
