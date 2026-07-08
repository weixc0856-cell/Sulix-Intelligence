//! Publishing Agent — 发布阶段
//!
//! 从 main.rs 拆分。按 Publish Stages Contract 组织：
//!
//!   Preprocess → Generate → Infer → Persist → Emit
//!
//! 每阶段有明确的输入/输出契约，agent_publish() 仅作为协调器。
//!
//! # Contract
//! - Preprocess: 加载所有持久化状态（EventLog, Chronicle, Memory）
//! - Generate:   Premium 报告 + ASI 评分 + 合成摘要 + 认知校准 + Change Detection
//! - Infer:      Memory Engine 更新 + Hermes 分析 + Outcome 检测 + Decision Intelligence
//! - Persist:    所有 JSON/SQLite 持久化 + Decay Agent
//! - Emit:       MDX/Markdown 渲染 + 输出

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;

use crate::archive::{ChronicleDb, ChronicleEntry};
use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::config::Config;
use crate::db::Database;
use crate::domain::evidence::Stance;
use crate::domain::outcome::{Outcome, OutcomeVerdict};
use crate::domain::thesis::ThesisStatus;
use crate::domain::EditorNote;
use crate::domain::StrategicDomain;
use crate::domain::ThesisDecision;
use crate::event_log::ObjectEvent;
use crate::engine::decision::map_theses_to_decisions;
use crate::engine::investigation::generate_investigation;
use crate::engine::memory::MemoryEngine;
use crate::renderer::publisher::Publisher;
use crate::storage;
use crate::domain::artifact::ArtifactSet;

/// 发布产物计数统计（用于 manifest）
struct PublishCounts {
    assessment_count: usize,
    investigation_count: usize,
    archive_days: usize,
    total_signals: usize,
}

/// Research Agent 的输出（传递给 Publishing Agent）
pub struct ResearchOutput {
    pub themes: Vec<Theme>,
    pub analyses: Vec<ThemeAnalysis>,
    pub analyses_zh: Vec<ThemeAnalysis>,
    pub triage: crate::agent::scan::TriageResult,
    pub new_articles: Vec<crate::fetcher::Article>,
}

// ===== 5-Stage Contract: Data Structures =====

/// Stage 1: Preprocess 装载的所有持久化状态
struct StateBundle {
    event_log: crate::event_log::EventLog,
    event_log_path: PathBuf,
    chronicle: Option<ChronicleDb>,
    chronicle_path: PathBuf,
    memory_for_linking: MemoryEngine,
    memory_path: PathBuf,
    registry: crate::engine::registry::AssessmentRegistry,
    registry_path: PathBuf,
    inv_registry: crate::engine::investigation_registry::InvestigationRegistry,
    inv_registry_path: PathBuf,
}

/// Stage 2: Generate 阶段产出的所有内容
struct GeneratedAssets {
    asi_score_map: HashMap<String, (f64, f64, f64)>,
    premium_reports: Vec<crate::domain::PremiumReport>,
    belief_notes_html: String,
    editor_notes: Vec<EditorNote>,
    change_summary: crate::hermes::ChangeSummary,
    calibration_text: String,
    summary: crate::domain::theme::Summary,
    /// theme title → (primary_domain, secondary_domains) — LLM-refined when keyword confidence low
    refined_domains: HashMap<String, (StrategicDomain, Vec<StrategicDomain>)>,
}

/// Stage 3: Infer 阶段产出的认知状态
struct InferredState {
    memory: MemoryEngine,
    thesis_decisions: Vec<ThesisDecision>,
    premium_reports: Vec<crate::domain::PremiumReport>,
    asi_score_map: HashMap<String, (f64, f64, f64)>,
    editor_notes: Vec<EditorNote>,
    beliefs_html: String,
    /// 待 Emit 阶段写入的 Investigation Reports (slug, report, assessment_id, inv_id)
    investigation_reports: Vec<(String, crate::domain::investigation::InvestigationReport, Option<String>, Option<String>)>,
    /// theme title → (primary_domain, secondary_domains) — LLM-refined domains
    refined_domains: HashMap<String, (StrategicDomain, Vec<StrategicDomain>)>,
    /// Object events collected during infer stage
    events: Vec<ObjectEvent>,
}

// ===== Contract Constants =====

/// 记录 ASI 分数的最低 SVI（用于 info 日志）
/// 单调递增事件 ID 计数器（替代易碰撞的 timestamp_nanos_opt 拼接）
static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

const SVI_MIN_LOG: f64 = 6.0;
/// 生成 premium 研报的最低 SVI
const SVI_MIN_PREMIUM: u8 = 7;
/// Trend 查询天数
const TREND_DAYS: i32 = 14;

// ===== Agent Publish: Coordinator =====

/// Publishing Agent 主入口 — 5-stage coordinator
#[allow(clippy::too_many_arguments)]
pub async fn agent_publish(
    config: &Config,
    api_key: &str,
    db: &Database,
    catalog: &crate::catalog::DataCatalog,
    data_dir: &Path,
    today: &str,
    entity_db: &mut crate::entity::EntitySanctionDb,
    research: ResearchOutput,
) -> Result<ArtifactSet> {
    let vault_base = PathBuf::from(&config.output.vault_path);

    // Stage 1: Preprocess — load all persistent state
    let mut state = publish_preprocess(data_dir, config).await;

    // Stage 2: Generate — content creation (no state mutation)
    let (themes, analyses, analyses_zh, new_articles, triage) = research.destructure();
    let generated = publish_generate(
        config, api_key, today, &themes, &analyses,
        &triage, &state,
    ).await?;
    catalog.save_step(7, "summary", &generated.summary)?;
    catalog.save_step(8, "calibration", &generated.calibration_text)?;

    // Stage 3: Infer — run cognitive engines (Memory, Hermes, Decision)
    let mut inferred = publish_infer(
        config, api_key, today,
        &themes, &analyses, &analyses_zh, &new_articles,
        &generated, &mut state, db,
    ).await?;

    // Stage 4: Persist — write all state to disk
    publish_persist(
        db, data_dir, today, entity_db,
        &mut state, &mut inferred, config,
    ).await;

    // Stage 5: Emit — render MDX/Markdown output
    publish_emit(
        config, today, vault_base.clone(),
        &themes, &analyses, &new_articles,
        &inferred,
    ).await?;

    // Collect events from infer stage
    let events = inferred.events;

    // Count MDX outputs for manifest (pre-validation snapshot)
    let mdx_path = config.output.mdx_dir.as_ref().map(PathBuf::from);
    let counts = mdx_path.as_ref().map(|p| {
        let count_md = |dir: &std::path::Path| -> usize {
            std::fs::read_dir(dir)
                .map(|d| d.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                    .count())
                .unwrap_or(0)
        };
        let count_dates = |dir: &std::path::Path| -> usize {
            std::fs::read_dir(dir)
                .map(|d| {
                    let dates: std::collections::HashSet<String> = d
                        .filter_map(|e| e.ok())
                        .filter_map(|e| e.file_name().to_str()
                            .and_then(|n| n.get(..10))
                            .filter(|s| s.chars().nth(4) == Some('-'))
                            .map(|s| s.to_string()))
                        .collect();
                    dates.len()
                }).unwrap_or(0)
        };
        PublishCounts {
            assessment_count: count_md(&p.join("thesis")),
            investigation_count: count_md(&p.join("investigation")),
            archive_days: count_dates(&p.join("daily")),
            total_signals: count_md(&p.join("daily")),
        }
    }).unwrap_or(PublishCounts { assessment_count: 0, investigation_count: 0, archive_days: 0, total_signals: 0 });

    // Build ArtifactSet — ownership transfers to delivery publisher
    let artifacts = ArtifactSet::new(
        themes, analyses, analyses_zh,
        inferred.memory,
        inferred.thesis_decisions,
        inferred.premium_reports,
        inferred.editor_notes,
        inferred.investigation_reports,
        new_articles,
        events,
        today.to_string(),
        inferred.asi_score_map,
        String::new(), // belief_notes_html
        inferred.refined_domains,
        counts.assessment_count,
        counts.investigation_count,
        0, // decision_count — filled from thesis_decisions.len() in delivery
        counts.archive_days,
        counts.total_signals,
    );

    // Final logging
    log::info!("📊 {}", crate::llm::llm_audit_summary());
    if !state.event_log.all().is_empty() {
        if let Err(e) = state.event_log.save_to_file(&state.event_log_path.to_string_lossy()) {
            log::warn!("⚠️ EventLog 保存失败: {}", e);
        }
    }

    println!("\n✅ EN 简报: {}", vault_base.join("en").join(&today[..7]).join("index.html").display());
    println!("✅ 看板: {}", vault_base.join("en").join("index.html").display());
    Ok(artifacts)
}

// ===== Stage 1: Preprocess =====

/// Preprocess: 加载所有持久化状态（EventLog, Chronicle, Memory）
async fn publish_preprocess(data_dir: &Path, config: &Config) -> StateBundle {
    let event_log_path = data_dir.join("event_log.json");
    let event_log = load_or_new_event_log(&event_log_path);

    let chronicle_path = data_dir.join("database.json");
    let chronicle = if chronicle_path.exists() {
        match ChronicleDb::load(&chronicle_path) {
            Ok(c) => Some(c),
            Err(e) => {
                log::warn!("⚠️ Chronicle 加载失败: {}", e);
                None
            }
        }
    } else {
        None
    };

    let memory_path = PathBuf::from(&config.output.vault_path).join("memory_db.json");
    let mut memory_for_linking = MemoryEngine::new(memory_path.clone());
    if let Err(e) = memory_for_linking.load() {
        log::warn!("⚠️ Memory Engine 加载失败（用于冲突链接）: {}", e);
    }

    let registry_path = PathBuf::from(&config.output.vault_path).join("assessment_registry.json");
    let registry = crate::engine::registry::AssessmentRegistry::load_or_new(&registry_path);

    let inv_registry_path = PathBuf::from(&config.output.vault_path).join("investigation_registry.json");
    let inv_registry = crate::engine::investigation_registry::InvestigationRegistry::load_or_new(&inv_registry_path);

    StateBundle {
        event_log, event_log_path,
        chronicle, chronicle_path,
        memory_for_linking, memory_path,
        registry, registry_path,
        inv_registry, inv_registry_path,
    }
}

fn load_or_new_event_log(path: &Path) -> crate::event_log::EventLog {
    storage::with_corrupt_recovery(
        path,
        |p| crate::event_log::EventLog::load_from_file(&p.to_string_lossy()),
        crate::event_log::EventLog::new,
    )
}

// ===== Stage 2: Generate =====

// ===== Stage 2: Generate =====

/// Generate: Premium 报告 + ASI 评分 + 合成摘要 + 认知校准 + Change Detection
#[allow(clippy::too_many_arguments)]
async fn publish_generate(
    config: &Config,
    api_key: &str,
    today: &str,
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    triage: &crate::agent::scan::TriageResult,
    state: &StateBundle,
) -> Result<GeneratedAssets> {
    let vault_base = PathBuf::from(&config.output.vault_path);
    let premium_dir = vault_base.join("premium");
    fs::create_dir_all(&premium_dir)?;

    // Premium 深度研报 + ASI 评分收集
    let mut asi_score_map: HashMap<String, (f64, f64, f64)> = HashMap::new();
    let mut premium_reports: Vec<crate::domain::PremiumReport> = vec![];
    for (theme, analysis) in themes.iter().zip(analyses.iter()) {
        let svi = crate::engine::analysis::calculate_svi(analysis, theme, &config.sources);
        let asi_config = crate::engine::analysis::asi::AsiConfig::default();
        let max_days_old = chrono::Utc::now()
            .date_naive()
            .signed_duration_since(
                chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d")
                    .unwrap_or_else(|_| chrono::Utc::now().date_naive()),
            )
            .num_days()
            .max(0);
        let asi_result = crate::engine::analysis::asi::calculate_asi(
            analysis.signal_strength,
            max_days_old,
            &asi_config,
        );
        let confidence_config = crate::engine::analysis::asi::ConfidenceConfig::default();
        let confidence_result = crate::engine::analysis::asi::calculate_confidence(
            &analysis.evidence_level,
            analysis.signal_strength,
            theme.sources.len(),
            &confidence_config,
        );
        let final_val = crate::engine::analysis::asi::final_value(svi, &asi_result, &confidence_result);
        asi_score_map.insert(
            theme.title.clone(),
            (asi_result.asi, confidence_result.confidence, final_val),
        );
        if final_val >= SVI_MIN_LOG {
            log::info!(
                "⭐ ASI: {} (SVI={}, ASI={:.2}, Confidence={:.2}, final={:.1})",
                theme.title, svi, asi_result.asi, confidence_result.confidence, final_val
            );
        }
        if svi < SVI_MIN_PREMIUM {
            continue;
        }
        let theme_context: String = theme.articles.iter()
            .map(|a| format!("- [{}] {}: {}", a.source, a.title, a.summary.as_deref().unwrap_or("")))
            .collect::<Vec<_>>()
            .join("\n");
        match crate::engine::premium::generate_premium_report(
            theme, &theme_context, api_key, &config.llm, config.prompts.as_ref(),
        ).await {
            Ok(report) => {
                if let Ok(html) = crate::renderer::render_premium_report(&report) {
                    let slug = theme.title.to_lowercase().replace(' ', "-");
                    fs::write(premium_dir.join(format!("{}.html", slug)), &html)?;
                    log::info!("📖 Premium: {} → {}.html", theme.title, slug);
                }
                if let Some(ref sub) = config.substack {
                    if sub.enabled {
                        if let Err(e) = crate::engine::premium::push_to_substack(
                            &report, &sub.api_key, &sub.publication_url,
                        ).await {
                            log::warn!("⚠️ Substack push failed [{}]: {}", theme.title, e);
                        }
                    }
                }
                premium_reports.push(report);
            }
            Err(e) => log::warn!("⚠️ Premium 研报失败 [{}]: {}", theme.title, e),
        }
    }

    // Twitter/X 推文
    if let Some(ref twitter_config) = config.twitter {
        crate::twitter::publish_tweets(themes, analyses, twitter_config).await;
    }

    // Belief Engine Phase B
    let mut belief_engine = crate::engine::belief::BeliefEngineV2::new();
    if let Some(ref config_beliefs) = config.beliefs {
        let core_beliefs: Vec<crate::engine::belief::CoreBelief> = config_beliefs.iter()
            .map(|b| crate::engine::belief::CoreBelief {
                id: b.id.clone(), statement: b.statement.clone(),
                confidence: b.confidence, category: b.category.clone(), history: vec![],
            })
            .collect();
        belief_engine.load_from_config(&core_beliefs);
        belief_engine.update_from_analyses(analyses, today);
        let recent = belief_engine.recent_changes(5);
        if !recent.is_empty() {
            log::info!("🎯 Belief Engine: {} 项信念更新", recent.len());
        }
    }
    let belief_notes_html = crate::engine::belief::render_belief_changes_html(&belief_engine);

    // 合成摘要
    let summary = crate::clusterer::synthesize(themes, analyses);
    log::info!("✅ 聚类完成: {} 个主题, {} 篇文章", summary.theme_count, summary.total_articles);

    // Markdown 输出 （保留，但将在 Emit 阶段执行）
    // 认知校准
    let calibration_text = if !analyses.is_empty() {
        let calibration_input: Vec<crate::llm::VerticalAnalysis> = analyses.iter()
            .map(|ta| crate::llm::VerticalAnalysis {
                category: ta.theme_title.clone(), articles: vec![],
            })
            .collect();
        crate::agent::calibration::calibrate(
            &calibration_input, api_key, &config.llm, config.prompts.as_ref(), "en",
        ).await?
    } else {
        String::new()
    };
    log::info!("📝 分析主题: {} 个, 信号: {} 条",
        analyses.len(),
        themes.iter().map(|t| t.articles.len()).sum::<usize>() + triage.watchlist.len(),
    );

    // Editor Notes (question_matches removed — QuestionEngine not wired)
    let editor_notes = crate::agent::editor::analyze_personal_impact(
        analyses, state.memory_for_linking.theses(),
    );
    if !editor_notes.is_empty() {
        log::info!("👤 Editor Agent: {} 项个人影响分析", editor_notes.len());
    }

    // Change Detection
    let recent_entries: Vec<ChronicleEntry> = state.chronicle.as_ref()
        .map(|c| c.sorted().into_iter().take(50).collect())
        .unwrap_or_default();
    let change_summary = if config.news_layer.as_ref().map(|n| n.llm_change_detection).unwrap_or(false) {
        crate::hermes::detect_changes_llm(&recent_entries, analyses, api_key, &config.llm)
            .await
            .inspect(|cs| log::info!("🧠 LLM change detection: {} conflicts, {} reinforced", cs.conflicts.len(), cs.reinforced.len()))
            .unwrap_or_else(|| {
                log::warn!("⚠️ LLM change detection failed, falling back to rule-based");
                crate::hermes::detect_changes_rule(&recent_entries, analyses)
            })
    } else {
        crate::hermes::detect_changes_rule(&recent_entries, analyses)
    };
    if !change_summary.conflicts.is_empty() || !change_summary.reinforced.is_empty() {
        log::info!("🔄 Change Detection: {} 冲突, {} 强化, {} 新信号",
            change_summary.conflicts.len(), change_summary.reinforced.len(), change_summary.new_signals.len());
    }

    // ── Strategic Domain Classification (with LLM refine for low-confidence topics) ──
    let mut refined_domains: HashMap<String, (StrategicDomain, Vec<StrategicDomain>)> = HashMap::new();
    for theme in themes {
        let text = format!("{} {}", theme.title,
            theme.articles.first()
                .and_then(|a| a.summary.as_deref())
                .unwrap_or(""));
        if crate::domain::StrategicDomain::is_classify_low_confidence(&text) {
            // Low keyword confidence → try LLM refine
            let system_prompt = crate::domain::StrategicDomain::llm_classification_prompt();
            match crate::llm::call_and_parse(
                api_key, &config.llm, system_prompt, &text
            ).await {
                Ok(response) => {
                    if crate::domain::StrategicDomain::validate_llm_output(&response) {
                        let (primary, secondary) = crate::domain::StrategicDomain::parse_llm_response(&response);
                        log::info!("🧠 Domain classify [{}]: {} (LLM-refined, was keyword)", theme.title, primary.label());
                        refined_domains.insert(theme.title.clone(), (primary, secondary));
                    }
                }
                Err(_) => {
                    // LLM failed → keep keyword result (no entry in refined_domains)
                }
            }
        }
    }
    if !refined_domains.is_empty() {
        log::info!("🧠 Domain classification: {} topics LLM-refined", refined_domains.len());
    }

    Ok(GeneratedAssets {
        asi_score_map, premium_reports,
        belief_notes_html,
        editor_notes, change_summary,
        calibration_text, summary,
        refined_domains,
    })
}

// ===== Stage 3: Infer =====

/// Infer: Memory Engine 更新 + Hermes 分析 + Outcome 检测 + Decision Intelligence
#[allow(clippy::too_many_arguments)]
async fn publish_infer(
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

    // Push conflict events to EventLog
    for conflict in &generated.change_summary.conflicts {
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

    // Chronicle 构建
    let chronicle_path = state.chronicle_path.clone();
    let mut chronicle = ChronicleDb::load(&chronicle_path)?;
    for a in analyses_zh {
        chronicle.push(ChronicleEntry {
            date: today.to_string(),
            topic: a.theme_title.clone(),
            headline: a.bluf.clone(),
            entities: extract_entities(a),
            signal_strength: a.signal_strength,
            language: "zh".into(),
        });
    }
    for analysis in analyses.iter() {
        chronicle.push(ChronicleEntry {
            date: today.to_string(),
            topic: analysis.theme_title.clone(),
            headline: analysis.bluf.clone(),
            entities: extract_entities(analysis),
            signal_strength: analysis.signal_strength,
            language: "en".into(),
        });
    }

    // Memory Engine 信念追踪 + Hermes 分析
    let mut memory = MemoryEngine::new(state.memory_path.clone());
    if let Err(e) = memory.load() {
        let backup = format!("{}.corrupt.{}.json",
            state.memory_path.to_string_lossy(),
            chrono::Utc::now().format("%Y%m%d_%H%M%S"));
        log::warn!("⚠️ Memory Engine 加载失败 ({}), 备份到 {} 后重建", e, backup);
        let _ = std::fs::rename(&state.memory_path, &backup);
    }

    if let Err(e) = memory.update_from_analysis_with_registry(today, themes, analyses, &mut state.registry) {
        log::warn!("⚠️ Memory Engine 更新失败: {}", e);
    } else {
        let before = memory.theses().len();
        if !generated.change_summary.conflicts.is_empty() {
            crate::hermes::apply_conflicts(&generated.change_summary, &mut memory, today);
        }
        if let Ok(trends) = db.get_trend(TREND_DAYS) {
            crate::hermes::analyze_trends(&trends, &mut memory, today);
        }
        crate::hermes::discover_theses(analyses, &chronicle, &mut memory, today);
        log::info!("🧠 Memory Engine: {} 个 Thesis (Hermes: {} 新增)",
            memory.theses().len(), memory.theses().len() - before);

        // Apply LLM-refined domain classifications (overrides keyword-based defaults)
        if !generated.refined_domains.is_empty() {
            for thesis in memory.theses_mut() {
                if let Some((primary, secondary)) = generated.refined_domains.get(&thesis.title) {
                    thesis.primary_domain = *primary;
                    thesis.secondary_domains = secondary.clone();
                }
            }
        }
    }

    // Investigation Engine
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

    // Build investigation reports for Emit stage (不再在此写盘，移至 Stage 5)
    let mut investigation_reports = Vec::new();
    for thesis in memory.theses() {
        if !matches!(thesis.status, ThesisStatus::Active | ThesisStatus::Strengthening | ThesisStatus::Weakening) {
            continue;
        }
        let slug = {
            let slug_base = crate::renderer::publisher::ascii_slug(&thesis.title);
            if slug_base.is_empty() {
                crate::renderer::publisher::short_id_from_thesis(&thesis.id)
            } else {
                slug_base
            }
        };
        let report = crate::engine::investigation::derive_investigation_report(
            thesis, today, None,
        );
        investigation_reports.push((slug, report, thesis.assessment_id.clone(), thesis.investigation_id.clone()));
    }

    // Meta Layer: Outcome 检测 & Reflection 生成
    for thesis in memory.theses().to_owned() {
        if thesis.status == ThesisStatus::Retired {
            let challenge = thesis.evidences.iter().filter(|e| e.stance == Stance::Challenges).count();
            let support = thesis.evidences.iter().filter(|e| e.stance == Stance::Supports).count();
            if challenge > support {
                let (outcome, event) = Outcome::new(
                    format!("outcome-{}", chrono::Utc::now().timestamp()),
                    thesis.id.clone(),
                    format!("被证伪: 挑战证据 ({}) 超过支持证据 ({})", challenge, support),
                    OutcomeVerdict::Invalidated,
                    today.to_string(),
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
                        thesis_id: Some(thesis.id.clone()),
                        related_events: vec![],
                        data: serde_json::json!({"thesis_title": thesis.title, "support": support, "challenge": challenge}),
                    });
                    log::info!("🧠 Meta Layer: Thesis '{}' → Invalidated (S={}, C={})", thesis.title, support, challenge);
                }
            }
        }
        if thesis.status == ThesisStatus::Strengthening && thesis.evidences.len() >= 2 {
            let (outcome, event) = Outcome::new(
                format!("outcome-{}", chrono::Utc::now().timestamp()),
                thesis.id.clone(),
                format!("证据持续积累 ({} 条)", thesis.evidences.len()),
                OutcomeVerdict::PartiallyConfirmed,
                today.to_string(),
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
                    thesis_id: Some(thesis.id.clone()),
                    related_events: vec![],
                    data: serde_json::json!({"thesis_title": thesis.title, "evidence_count": thesis.evidences.len()}),
                });
                log::info!("🧠 Meta Layer: Thesis '{}' → Strengthening ({} evidence)", thesis.title, thesis.evidences.len());
            }
        }
    }

    // 生成置信度变化通知
    let outcome_notifications_html = {
        let recent_outcomes: Vec<_> = memory.all_outcomes().iter().rev().take(3).map(|o| {
            let icon = match o.verdict {
                OutcomeVerdict::Confirmed => "✅",
                OutcomeVerdict::PartiallyConfirmed => "🟡",
                OutcomeVerdict::Invalidated => "❌",
                OutcomeVerdict::Unknown => "❓",
            };
            format!(r#"<div style="display:flex;align-items:flex-start;gap:0.5rem;padding:0.375rem 0;border-bottom:1px solid #f0f0f0;font-size:0.75rem">
  <span>{}</span><div><strong>{}</strong></div>
</div>"#, icon, o.description)
        }).collect();
        if recent_outcomes.is_empty() {
            String::new()
        } else {
            format!(r#"<div style="margin-top:0.75rem;padding:0.5rem;background:#fef2f2;border-radius:0.25rem;border-left:3px solid #ef4444">
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.6875rem;font-weight:700;text-transform:uppercase;letter-spacing:0.05em;color:#dc2626;margin-bottom:0.25rem">🎯 判断更新</div>
  {}</div>"#, recent_outcomes.join(""))
        }
    };

    // Decision Intelligence
    let thesis_decisions_raw = map_theses_to_decisions(&memory);

    // Stability Layer v1: smooth decisions before recording
    let history_map = crate::engine::stability::build_decision_history_map(&memory);
    let consecutive_map = crate::engine::stability::build_consecutive_days_map(&memory);
    let thesis_decisions = crate::engine::stability::stability_gate(
        thesis_decisions_raw,
        &history_map,
        &consecutive_map,
    );

    for d in &thesis_decisions {
        memory.record_decision(&d.thesis_id, today, d.decision_type.as_key(), d.confidence);
    }

    // Log high-priority decisions
    let high_priority: Vec<&ThesisDecision> = thesis_decisions.iter()
        .filter(|d| matches!(d.decision_type, crate::domain::action::DecisionType::Exit | crate::domain::action::DecisionType::Build))
        .collect();
    if !high_priority.is_empty() {
        log::info!("🧠 Decision Intelligence: {} 个高优先级决策", high_priority.len());
        for d in &high_priority {
            log::info!("  - {:?}: {} ({})", d.decision_type, d.thesis_title, d.rationale);
        }
    }

    // Save chronicle to disk (part of Infer since Chronicle feeds into future runs)
    chronicle.save(&chronicle_path)?;

    // Decay Agent (runs after all infer logic so state is current)
    if let Some(ref g) = config.graveyard {
        if g.enabled {
            match crate::agent::decay::run_maintenance(db, new_articles, api_key, &config.llm, g).await {
                Ok(_) => log::info!("🪦 Decay Agent 维护完成"),
                Err(e) => log::warn!("⚠️ Decay Agent 失败: {}", e),
            }
        }
    }

    // 将置信度变化通知追加到 belief_notes_html
    let mut beliefs_html = generated.belief_notes_html.clone();
    if !outcome_notifications_html.is_empty() {
        beliefs_html.push_str(&outcome_notifications_html);
    }

    Ok(InferredState {
        memory,
        thesis_decisions,
        premium_reports: generated.premium_reports.clone(),
        asi_score_map: generated.asi_score_map.clone(),
        editor_notes: generated.editor_notes.clone(),
        beliefs_html,
        investigation_reports,
        events: infer_events,
        refined_domains: generated.refined_domains.clone(),
    })
}

// ===== Stage 4: Persist =====

/// Persist: 所有持久化写入（Memory, Registry, EntityDb, Decision, EventLog, SQLite report）
async fn publish_persist(
    db: &Database,
    data_dir: &Path,
    today: &str,
    entity_db: &mut crate::entity::EntitySanctionDb,
    state: &mut StateBundle,
    inferred: &mut InferredState,
    config: &Config,
) {
    // Memory Engine save
    if let Err(e) = inferred.memory.save() {
        log::warn!("⚠️ Memory Engine 保存失败: {}", e);
    }

    // Assessment Registry save
    if let Err(e) = state.registry.save(&state.registry_path) {
        log::warn!("⚠️ Assessment Registry 保存失败: {}", e);
    } else {
        log::info!("📋 Assessment Registry: {} assessments, next ID: ASM-{:04}",
            state.registry.assessments.len(), state.registry.core.next_id);
    }

    // Decision Registry save
    let dec_registry_path = PathBuf::from(&config.output.vault_path).join("decision_registry.json");
    let mut dec_registry = crate::engine::decision_registry::DecisionRegistry::load_or_new(&dec_registry_path);
    {
        let theses_snapshot: Vec<_> = inferred.memory.theses().iter()
            .map(|t| (t.id.clone(), t.assessment_id.clone()))
            .collect();
        for td in &inferred.thesis_decisions {
            if let Some((_, Some(asm_id))) = theses_snapshot.iter().find(|(id, _)| id == &td.thesis_id) {
                if let Some(event) = inferred.memory.record_or_update_decision(td, asm_id, today, &mut dec_registry) {
                    inferred.events.push(event);
                }
            }
        }
    }
    if let Err(e) = dec_registry.save(&dec_registry_path) {
        log::warn!("⚠️ Decision Registry 保存失败: {}", e);
    } else {
        log::info!("🎯 Decision Registry: {} decisions, next ID: DEC-{:04}",
            dec_registry.decisions.len(), dec_registry.core.next_id);
    }

    // Investigation Registry save
    if let Err(e) = state.inv_registry.save(&state.inv_registry_path) {
        log::warn!("⚠️ Investigation Registry 保存失败: {}", e);
    } else {
        log::info!("📋 Investigation Registry: {} investigations, next ID: INV-{:04}",
            state.inv_registry.investigations.len(), state.inv_registry.core.next_id);
    }

    // EntitySanctionDb save
    let entity_db_path = data_dir.join("entity_db.json");
    if let Err(e) = entity_db.save_to_file(&entity_db_path.to_string_lossy()) {
        log::warn!("⚠️ EntitySanctionDb 保存失败: {}", e);
    }

    // SQLite report
    if let Err(e) = db.record_report(today, &format!("Daily brief - {} topics", inferred.memory.theses().len()), 0) {
        log::warn!("⚠️ DB report 记录失败: {}", e);
    }

}

// ===== Stage 5: Emit =====

/// Emit: Markdown + MDX 渲染 + 输出
#[allow(clippy::too_many_arguments)]
async fn publish_emit(
    config: &Config,
    today: &str,
    vault_base: PathBuf,
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    new_articles: &[crate::fetcher::Article],
    inferred: &InferredState,
) -> Result<()> {
    // Markdown 输出
    let md_ctx = crate::renderer::publisher::PublishContext {
        themes: themes.to_vec(),
        analyses: analyses.to_vec(),
        date: today.to_string(),
        locale: "en".to_string(),
        theses: vec![],
        reports: vec![],
        canonical_decisions: vec![],
        asi_scores: HashMap::new(),
        editor_notes: vec![],
        belief_notes_html: String::new(),
        articles: vec![],
        mdx_output_dir: None,
        output_dir: vault_base.clone(),
        reflections: vec![],
        thesis_decisions: vec![],
        outcomes: vec![],
    };
    crate::renderer::publisher::MarkdownPublisher::new().publish(&md_ctx)?;
    log::info!("📝 Markdown 输出: {} 个主题", themes.len());

    // MDX 输出（主要输出格式）
    if let Some(ref mdx_out) = config.output.mdx_dir {
        let mdx_ctx = crate::renderer::publisher::PublishContext {
            themes: themes.to_vec(),
            analyses: analyses.to_vec(),
            date: today.to_string(),
            locale: "en".to_string(),
            theses: inferred.memory.theses().to_vec(),
            reports: inferred.premium_reports.clone(),
            canonical_decisions: inferred.memory.all_decisions().to_vec(),
            asi_scores: inferred.asi_score_map.clone(),
            editor_notes: inferred.editor_notes.clone(),
            belief_notes_html: inferred.beliefs_html.clone(),
            articles: new_articles.to_vec(),
            mdx_output_dir: Some(PathBuf::from(mdx_out)),
            output_dir: vault_base.clone(),
            reflections: inferred.memory.all_reflections().to_vec(),
            thesis_decisions: inferred.thesis_decisions.clone(),
            outcomes: inferred.memory.all_outcomes().to_vec(),
        };
        if let Err(e) = crate::renderer::publisher::MdxPublisher::new().publish(&mdx_ctx) {
            log::warn!("⚠️ MDX 输出失败: {}", e);
        }

        // Investigation MDX（Stage 5 Emit 阶段统一写入，不再在 Infer 阶段写盘）
        if !inferred.investigation_reports.is_empty() {
            let inv_dir = std::path::Path::new(mdx_out).join("investigation");
            if let Err(e) = std::fs::create_dir_all(&inv_dir) {
                log::warn!("⚠️ Cannot create investigation dir: {}", e);
            } else {
                for (slug, report, assessment_id, inv_id) in &inferred.investigation_reports {
                    let mdx = crate::renderer::mdx::render_investigation_mdx(
                        report, slug, assessment_id.as_deref(), inv_id.as_deref(), "en",
                    );
                    if let Err(e) = std::fs::write(inv_dir.join(format!("{}.md", slug)), &mdx) {
                        log::warn!("⚠️ Investigation MDX write failed [{}]: {}", slug, e);
                    }
                }
                log::info!("📝 Investigation MDX: {} 篇", inferred.investigation_reports.len());
            }
        }
    }

    Ok(())
}

// ===== Helpers =====

/// Extract entities from analysis fact_base (deduplicated entity list per analysis)
fn extract_entities(analysis: &ThemeAnalysis) -> Vec<String> {
    let known = crate::entity::known_entities();
    let mut entities = Vec::new();
    for fb in &analysis.fact_base {
        for word in fb.evidence.split_whitespace() {
            let upper = word.to_uppercase();
            if known.contains(&upper.as_str()) && !entities.contains(&upper) {
                entities.push(upper);
            }
        }
    }
    entities
}


/// 主题分析 + 蓝军验证辅助函数
/// 由 main.rs 的 agent_research 调用
pub async fn analyze_and_validate(
    theme: &Theme,
    api_key: &str,
    llm_config: &crate::config::LlmConfig,
    prompts: Option<&crate::config::PromptsConfig>,
    language: &str,
) -> Option<ThemeAnalysis> {
    let mut analysis = match crate::engine::analysis::analyze_theme(theme, api_key, llm_config, language, prompts).await {
        Ok(a) => a,
        Err(e) => {
            log::warn!("⚠️ 主题分析失败 [{}|{}]: {}", language, theme.title, e);
            return None;
        }
    };
    match crate::engine::analysis::challenge_theme(&analysis, api_key, llm_config, prompts).await {
        Ok((assumptions, adverse, next_tests, open_questions)) => {
            analysis.assumptions = assumptions;
            analysis.adverse = adverse;
            analysis.next_tests = next_tests;
            analysis.open_questions = open_questions;
        }
        Err(e) => log::warn!("⚠️ 蓝军验证失败 [{}|{}], 使用无蓝军分析: {}", language, theme.title, e),
    }
    Some(analysis)
}

// ===== ResearchOutput destructure helper =====
impl ResearchOutput {
    #[allow(clippy::type_complexity)]
    fn destructure(self) -> (Vec<Theme>, Vec<ThemeAnalysis>, Vec<ThemeAnalysis>, Vec<crate::fetcher::Article>, crate::agent::scan::TriageResult) {
        (self.themes, self.analyses, self.analyses_zh, self.new_articles, self.triage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evidence::FactBaseEntry;

    fn make_analysis(fact_base: Vec<FactBaseEntry>) -> ThemeAnalysis {
        ThemeAnalysis {
            theme_id: "test-001".into(),
            theme_title: "Test Theme".into(),
            bluf: "".into(),
            impact: "".into(),
            geopolitical_fact: "".into(),
            supply_chain_impact: "".into(),
            analysis_paragraph: "".into(),
            evidence_level: "strong".into(),
            signal_strength: 5,
            fact_base,
            connections: vec![],
            source_urls: vec![],
            assumptions: vec![],
            adverse: None,
            next_tests: vec![],
            open_questions: vec![],
            chains: vec![],
            what_to_do: "".into(),
            what_to_watch: "".into(),
            falsification_conditions: vec![],
        }
    }

    #[test]
    fn test_extract_entities_known_entity() {
        let analysis = make_analysis(vec![
            FactBaseEntry {
                evidence: "TSMC is expanding capacity".into(),
                interpretation: "".into(),
                confidence: "high".into(),
            },
        ]);
        let entities = extract_entities(&analysis);
        assert_eq!(entities, vec!["TSMC"]);
    }

    #[test]
    fn test_extract_entities_empty_fact_base() {
        let analysis = make_analysis(vec![]);
        let entities = extract_entities(&analysis);
        assert!(entities.is_empty());
    }

    #[test]
    fn test_extract_entities_deduplication() {
        let analysis = make_analysis(vec![
            FactBaseEntry {
                evidence: "TSMC and NVIDIA both expanding".into(),
                interpretation: "".into(),
                confidence: "high".into(),
            },
            FactBaseEntry {
                evidence: "TSMC leads the market".into(),
                interpretation: "".into(),
                confidence: "medium".into(),
            },
        ]);
        let entities = extract_entities(&analysis);
        assert_eq!(entities.len(), 2);
        assert!(entities.contains(&"TSMC".to_string()));
        assert!(entities.contains(&"NVIDIA".to_string()));
    }

    #[test]
    fn test_extract_entities_no_known_entities() {
        let analysis = make_analysis(vec![
            FactBaseEntry {
                evidence: "Some random company is growing".into(),
                interpretation: "".into(),
                confidence: "low".into(),
            },
        ]);
        let entities = extract_entities(&analysis);
        assert!(entities.is_empty());
    }

    #[test]
    fn test_research_output_destructure() {
        let output = ResearchOutput {
            themes: vec![],
            analyses: vec![],
            analyses_zh: vec![],
            new_articles: vec![],
            triage: crate::agent::scan::TriageResult {
                insight: vec![],
                watchlist: vec![],
                signal_memory: vec![],
            },
        };
        let (t, a, az, art, tr) = output.destructure();
        assert!(t.is_empty());
        assert!(a.is_empty());
        assert!(az.is_empty());
        assert!(art.is_empty());
        assert!(tr.insight.is_empty());
    }
}