//! Publishing Agent — 发布阶段
//!
//! 从 main.rs 拆分。负责：
//! - Premium 报告生成
//! - 变更检测 & EventLog
//! - HTML/Markdown 渲染（通过 Publisher trait）
//! - Chronicle 构建 & 看板
//! - Memory Engine 信念追踪
//! - Decay Agent 记忆墓地维护

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::archive::{ChronicleDb, ChronicleEntry};
use crate::clusterer::{Theme, ThemeAnalysis};
use crate::config::Config;
use crate::db::Database;
use crate::decision_engine::Decision;
use crate::engine::decision::{map_theses_to_decisions, ThesisDecision};
use crate::engine::memory::{MemoryEngine, Outcome, OutcomeVerdict, Stance, ThesisStatus};
use crate::renderer::publisher::Publisher;

/// Research Agent 的输出（传递给 Publishing Agent）
pub struct ResearchOutput {
    pub themes: Vec<Theme>,
    pub analyses: Vec<ThemeAnalysis>,
    pub analyses_zh: Vec<ThemeAnalysis>,
    pub decisions: Vec<Decision>,
    pub triage: crate::agent::scan::TriageResult,
    pub total_new: usize,
    pub new_articles: Vec<crate::fetcher::Article>,
    pub question_matches: Vec<crate::question_engine::QuestionMatch>,
}

/// Extract entities from analysis fact_base (deduplicated entity list per analysis)
fn extract_entities(analysis: &ThemeAnalysis) -> Vec<String> {
    let known_entities = [
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
    ];
    let mut entities = Vec::new();
    for fb in &analysis.fact_base {
        for word in fb.evidence.split_whitespace() {
            let upper = word.to_uppercase();
            if known_entities.contains(&upper.as_str()) && !entities.contains(&upper) {
                entities.push(upper);
            }
        }
    }
    entities
}

/// Premium 报告 → 合成摘要 → Markdown 输出 → 变更检测 → HTML 渲染 → Chronicle → Decay Agent
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
    _source_statuses: Vec<(String, bool, usize)>,
) -> Result<()> {
    const TREND_DAYS: i32 = 14;

    // 初始化事件日志（含损坏备份保护）
    let event_log_path = data_dir.join("event_log.json");
    let mut event_log = if event_log_path.exists() {
        match crate::event_log::EventLog::load_from_file(&event_log_path.to_string_lossy()) {
            Ok(log) => log,
            Err(e) => {
                let backup = format!(
                    "{}.corrupt.{}",
                    event_log_path.to_string_lossy(),
                    chrono::Utc::now().format("%Y%m%d_%H%M%S")
                );
                log::warn!("⚠️ EventLog 加载失败 ({}), 备份到 {} 后重建", e, backup);
                let _ = std::fs::rename(&event_log_path, &backup);
                crate::event_log::EventLog::new()
            }
        }
    } else {
        crate::event_log::EventLog::new()
    };

    let ResearchOutput {
        themes,
        analyses,
        analyses_zh,
        decisions: _decisions,
        triage,
        total_new,
        new_articles,
        question_matches,
    } = research;

    // Premium 深度研报 + ASI 评分收集
    let vault_base = PathBuf::from(&config.output.vault_path);
    let premium_dir = vault_base.join("premium");
    fs::create_dir_all(&premium_dir)?;
    let mut asi_score_map: HashMap<String, (f64, f64, f64)> = HashMap::new();
    for (theme, analysis) in themes.iter().zip(analyses.iter()) {
        let svi = crate::clusterer::calculate_svi(analysis, theme, &config.sources);
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
            &question_matches,
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
        let final_val =
            crate::engine::analysis::asi::final_value(svi, &asi_result, &confidence_result);
        asi_score_map.insert(
            theme.title.clone(),
            (asi_result.asi, confidence_result.confidence, final_val),
        );
        if final_val >= 6.0 {
            log::info!(
                "⭐ ASI: {} (SVI={}, ASI={:.2}, Confidence={:.2}, final={:.1})",
                theme.title,
                svi,
                asi_result.asi,
                confidence_result.confidence,
                final_val
            );
        }
        if svi < 7 {
            continue;
        }
        let theme_context: String = theme
            .articles
            .iter()
            .map(|a| {
                format!(
                    "- [{}] {}: {}",
                    a.source,
                    a.title,
                    a.summary.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        match crate::premium::generate_premium_report(
            theme,
            &theme_context,
            api_key,
            &config.llm,
            config.prompts.as_ref(),
        )
        .await
        {
            Ok(report) => {
                if let Ok(html) = crate::renderer::render_premium_report(&report) {
                    let slug = theme.title.to_lowercase().replace(' ', "-");
                    fs::write(premium_dir.join(format!("{}.html", slug)), &html)?;
                    log::info!("📖 Premium: {} → {}.html", theme.title, slug);
                }
                if let Some(sub) = &config.substack {
                    if sub.enabled {
                        if let Err(e) = crate::premium::push_to_substack(
                            &report,
                            &sub.api_key,
                            &sub.publication_url,
                        )
                        .await
                        {
                            log::warn!("⚠️ Substack push failed [{}]: {}", theme.title, e);
                        }
                    }
                }
            }
            Err(e) => log::warn!("⚠️ Premium 研报失败 [{}]: {}", theme.title, e),
        }
    }

    // Twitter/X 推文
    if let Some(ref twitter_config) = config.twitter {
        crate::twitter::publish_tweets(&themes, &analyses, twitter_config).await;
    }

    // Belief Engine Phase B
    let mut belief_engine = crate::engine::belief::BeliefEngineV2::new();
    if let Some(ref config_beliefs) = config.beliefs {
        let core_beliefs: Vec<crate::engine::belief::CoreBelief> = config_beliefs
            .iter()
            .map(|b| crate::engine::belief::CoreBelief {
                id: b.id.clone(),
                statement: b.statement.clone(),
                confidence: b.confidence,
                category: b.category.clone(),
                history: vec![],
            })
            .collect();
        belief_engine.load_from_config(&core_beliefs);
        belief_engine.update_from_analyses(&analyses, today);
        let recent = belief_engine.recent_changes(5);
        if !recent.is_empty() {
            log::info!("🎯 Belief Engine: {} 项信念更新", recent.len());
        }
    }
    let mut belief_notes_html = crate::engine::belief::render_belief_changes_html(&belief_engine);

    // 合成摘要
    let summary = crate::clusterer::synthesize(&themes, &analyses);
    log::info!(
        "✅ 聚类完成: {} 个主题, {} 篇文章",
        summary.theme_count,
        summary.total_articles
    );
    catalog.save_step(7, "summary", &summary)?;

    // Markdown 输出（通过 MarkdownPublisher）
    let md_ctx = crate::renderer::publisher::PublishContext {
        themes: themes.clone(),
        analyses: analyses.clone(),
        analyses_zh: vec![],
        date: today.to_string(),
        language: "en".into(),
        calibration: None,
        attributable_sources: vec![],
        flash_headline: None,
        change_summary: None,
        theses: vec![],
        report: None,
        archive_entries: vec![],
        archive_entries_zh: vec![],
        source_statuses: vec![],
        decisions: vec![],
        asi_scores: HashMap::new(),
        editor_notes: vec![],
        belief_notes_html: String::new(),
        css_content: String::new(),
        articles: vec![],
        watchlist_count: 0,
        mdx_output_dir: None,
        output_dir: PathBuf::from(&config.output.vault_path),
        reflections: vec![],
        thesis_decisions: vec![],
    };
    crate::renderer::publisher::MarkdownPublisher::new().publish(&md_ctx)?;
    log::info!("📝 Markdown 输出: {} 个主题", themes.len());

    // 认知校准
    let calibration_text = if !analyses.is_empty() {
        let calibration_input: Vec<crate::llm::VerticalAnalysis> = analyses
            .iter()
            .map(|ta| crate::llm::VerticalAnalysis {
                category: ta.theme_title.clone(),
                articles: vec![],
            })
            .collect();
        crate::agent::calibration::calibrate(
            &calibration_input,
            api_key,
            &config.llm,
            config.prompts.as_ref(),
            "en",
        )
        .await?
    } else {
        String::new()
    };
    catalog.save_step(8, "calibration", &calibration_text)?;
    log::info!(
        "📝 分析主题: {} 个, 信号: {} 条",
        analyses.len(),
        themes.iter().map(|t| t.articles.len()).sum::<usize>() + triage.watchlist.len()
    );

    // Change Detection + Memory Engine
    let memory_for_linking = {
        let mem_path = PathBuf::from(&config.output.vault_path).join("memory_db.json");
        let mut mem = MemoryEngine::new(mem_path);
        if let Err(e) = mem.load() {
            log::warn!("⚠️ Memory Engine 加载失败（用于冲突链接）: {}", e);
        }
        mem
    };

    let editor_notes = crate::agent::editor::analyze_personal_impact(
        &question_matches,
        &analyses,
        memory_for_linking.theses(),
    );
    if !editor_notes.is_empty() {
        log::info!("👤 Editor Agent: {} 项个人影响分析", editor_notes.len());
    }

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
    let recent_entries: Vec<ChronicleEntry> = chronicle
        .as_ref()
        .map(|c| c.sorted().into_iter().take(50).collect())
        .unwrap_or_default();
    let change_summary = if config
        .news_layer
        .as_ref()
        .map(|n| n.llm_change_detection)
        .unwrap_or(false)
    {
        crate::clusterer::detect_changes_llm(&recent_entries, &analyses, api_key, &config.llm)
            .await
            .inspect(|cs| {
                log::info!(
                    "🧠 LLM change detection: {} conflicts, {} reinforced",
                    cs.conflicts.len(),
                    cs.reinforced.len()
                )
            })
            .unwrap_or_else(|| {
                log::warn!("⚠️ LLM change detection failed, falling back to rule-based");
                crate::clusterer::detect_changes_rule(&recent_entries, &analyses)
            })
    } else {
        crate::clusterer::detect_changes_rule(&recent_entries, &analyses)
    };
    for conflict in &change_summary.conflicts {
        event_log.push(crate::event_log::PipelineEvent {
            id: format!("evt-{}-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0), chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
            event_type: crate::event_log::PipelineEventType::ConflictDetected,
            timestamp: chrono::Utc::now().to_rfc3339(),
            description: format!("{}: {}", conflict.topic, conflict.today_signal),
            thesis_id: memory_for_linking.find_by_title(&conflict.topic).map(|t| t.id.clone()),
            related_events: vec![],
            data: serde_json::json!({"topic": conflict.topic, "prior_belief": conflict.prior_belief}),
        });
    }
    if !change_summary.conflicts.is_empty() || !change_summary.reinforced.is_empty() {
        log::info!(
            "🔄 Change Detection: {} 冲突, {} 强化, {} 新信号",
            change_summary.conflicts.len(),
            change_summary.reinforced.len(),
            change_summary.new_signals.len()
        );
    }

    // Chronicle 构建
    let db_dir = data_dir.join(&today[..7]);
    fs::create_dir_all(&db_dir)
        .unwrap_or_else(|e| log::warn!("无法创建数据目录 {:?}: {}", db_dir, e));
    let mut chronicle = ChronicleDb::load(&chronicle_path)?;

    for a in &analyses_zh {
        let entities = extract_entities(a);
        chronicle.push(ChronicleEntry {
            date: today.to_string(),
            topic: a.theme_title.clone(),
            headline: a.bluf.clone(),
            entities,
            signal_strength: a.signal_strength,
            language: "zh".into(),
        });
    }
    for (analysis, _) in analyses.iter().zip(themes.iter()) {
        let entities = extract_entities(analysis);
        chronicle.push(ChronicleEntry {
            date: today.to_string(),
            topic: analysis.theme_title.clone(),
            headline: analysis.bluf.clone(),
            entities,
            signal_strength: analysis.signal_strength,
            language: "en".into(),
        });
    }
    chronicle.save(&chronicle_path)?;

    // Decay Agent
    if let Some(ref g) = config.graveyard {
        if g.enabled {
            match crate::agent::decay::run_maintenance(db, &new_articles, api_key, &config.llm, g)
                .await
            {
                Ok(_) => log::info!("🪦 Decay Agent 维护完成"),
                Err(e) => log::warn!("⚠️ Decay Agent 失败: {}", e),
            }
        }
    }

    db.record_report(
        today,
        &format!("Daily brief - {} topics", analyses.len()),
        total_new,
    )?;

    let entity_db_path = data_dir.join("entity_db.json");
    if let Err(e) = entity_db.save_to_file(&entity_db_path.to_string_lossy()) {
        log::warn!("⚠️ EntitySanctionDb 保存失败: {}", e);
    }

    // Memory Engine 信念追踪 + Hermes 分析
    let outcome_notifications_html: String;
    {
        let memory_path = PathBuf::from(&config.output.vault_path).join("memory_db.json");
        let mut memory = MemoryEngine::new(memory_path.clone());
        if let Err(e) = memory.load() {
            let backup = format!(
                "{}.corrupt.{}.json",
                memory_path.to_string_lossy(),
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            );
            log::warn!(
                "⚠️ Memory Engine 加载失败 ({}), 备份到 {} 后重建",
                e,
                backup
            );
            let _ = std::fs::rename(&memory_path, &backup);
        }
        if let Err(e) = memory.update_from_analysis(today, &themes, &analyses) {
            log::warn!("⚠️ Memory Engine 更新失败: {}", e);
        } else {
            let before = memory.theses().len();
            if !change_summary.conflicts.is_empty() {
                crate::hermes::apply_conflicts(&change_summary, &mut memory, today);
            }
            if let Ok(trends) = db.get_trend(TREND_DAYS) {
                crate::hermes::analyze_trends(&trends, &mut memory, today);
            }
            crate::hermes::discover_theses(&analyses, &chronicle, &mut memory, today);
            log::info!(
                "🧠 Memory Engine: {} 个 Thesis (Hermes: {} 新增)",
                memory.theses().len(),
                memory.theses().len() - before
            );
        }

        // Meta Layer: Outcome 检测 & Reflection 生成
        for thesis in memory.theses().to_owned() {
            if thesis.status == ThesisStatus::Retired {
                let challenge = thesis
                    .evidences
                    .iter()
                    .filter(|e| e.stance == Stance::Challenges)
                    .count();
                let support = thesis
                    .evidences
                    .iter()
                    .filter(|e| e.stance == Stance::Supports)
                    .count();
                if challenge > support {
                    let outcome = Outcome {
                        id: format!("outcome-{}", chrono::Utc::now().timestamp()),
                        thesis_id: thesis.id.clone(),
                        description: format!(
                            "被证伪: 挑战证据 ({}) 超过支持证据 ({})",
                            challenge, support
                        ),
                        verdict: OutcomeVerdict::Invalidated,
                        date: today.to_string(),
                        supporting_evidence: vec![],
                    };
                    if let Err(e) = memory.record_outcome(outcome) {
                        log::warn!("⚠️ Outcome 记录失败: {}", e);
                    } else {
                        event_log.push(crate::event_log::PipelineEvent {
                            id: format!("evt-{}-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0), chrono::Utc::now().timestamp()),
                            event_type: crate::event_log::PipelineEventType::ThesisRefuted,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            description: format!("论题 '{}' 被证伪", thesis.title),
                            thesis_id: Some(thesis.id.clone()),
                            related_events: vec![],
                            data: serde_json::json!({"thesis_title": thesis.title, "support": support, "challenge": challenge}),
                        });
                        log::info!(
                            "🧠 Meta Layer: Thesis '{}' → Invalidated (S={}, C={})",
                            thesis.title,
                            support,
                            challenge
                        );
                    }
                }
            }
            if thesis.status == ThesisStatus::Strengthening && thesis.evidences.len() >= 2 {
                let outcome = Outcome {
                    id: format!("outcome-{}", chrono::Utc::now().timestamp()),
                    thesis_id: thesis.id.clone(),
                    description: format!("证据持续积累 ({} 条)", thesis.evidences.len()),
                    verdict: OutcomeVerdict::PartiallyConfirmed,
                    date: today.to_string(),
                    supporting_evidence: vec![],
                };
                if let Err(e) = memory.record_outcome(outcome) {
                    log::warn!("⚠️ Outcome 记录失败: {}", e);
                } else {
                    event_log.push(crate::event_log::PipelineEvent {
                        id: format!("evt-{}-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0), chrono::Utc::now().timestamp()),
                        event_type: crate::event_log::PipelineEventType::OutcomeRecorded,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        description: format!("论题 '{}' 获得证据强化", thesis.title),
                        thesis_id: Some(thesis.id.clone()),
                        related_events: vec![],
                        data: serde_json::json!({"thesis_title": thesis.title, "evidence_count": thesis.evidences.len()}),
                    });
                    log::info!(
                        "🧠 Meta Layer: Thesis '{}' → Strengthening ({} evidence)",
                        thesis.title,
                        thesis.evidences.len()
                    );
                }
            }
        }
        // 生成置信度变化通知
        let recent_outcomes: Vec<_> = memory.all_outcomes().iter().rev().take(3).map(|o| {
            let icon = match o.verdict { OutcomeVerdict::Confirmed => "✅", OutcomeVerdict::PartiallyConfirmed => "🟡", OutcomeVerdict::Invalidated => "❌", OutcomeVerdict::Unknown => "❓" };
            format!(r#"<div style="display:flex;align-items:flex-start;gap:0.5rem;padding:0.375rem 0;border-bottom:1px solid #f0f0f0;font-size:0.75rem">
  <span>{}</span><div><strong>{}</strong></div>
</div>"#, icon, o.description)
        }).collect();
        outcome_notifications_html = if recent_outcomes.is_empty() {
            String::new()
        } else {
            format!(
                r#"<div style="margin-top:0.75rem;padding:0.5rem;background:#fef2f2;border-radius:0.25rem;border-left:3px solid #ef4444">
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.6875rem;font-weight:700;text-transform:uppercase;letter-spacing:0.05em;color:#dc2626;margin-bottom:0.25rem">🎯 判断更新</div>
  {}</div>"#,
                recent_outcomes.join("")
            )
        };

        if let Err(e) = memory.save() {
            log::warn!("⚠️ Memory Engine 保存失败: {}", e);
        }

        // Decision Intelligence: Thesis → Decision 映射
        let thesis_decisions = map_theses_to_decisions(&memory);
        if !thesis_decisions.is_empty() {
            let high_priority: Vec<&ThesisDecision> = thesis_decisions
                .iter()
                .filter(|d| {
                    matches!(
                        d.decision_type,
                        crate::domain::action::DecisionType::Exit
                            | crate::domain::action::DecisionType::Build
                    )
                })
                .collect();
            if !high_priority.is_empty() {
                log::info!(
                    "🧠 Decision Intelligence: {} 个高优先级决策",
                    high_priority.len()
                );
                for d in &high_priority {
                    log::info!(
                        "  - {:?}: {} ({})",
                        d.decision_type,
                        d.thesis_title,
                        d.rationale
                    );
                }
            }
        }

        // MDX 输出（主要输出格式）
        if let Some(ref mdx_out) = config.output.mdx_dir {
            let mdx_ctx = crate::renderer::publisher::PublishContext {
                themes: themes.clone(),
                analyses: analyses.clone(),
                analyses_zh: vec![],
                date: today.to_string(),
                language: "en".into(),
                calibration: None,
                attributable_sources: vec![],
                flash_headline: None,
                change_summary: None,
                theses: memory.theses().to_vec(),
                report: None,
                archive_entries: vec![],
                archive_entries_zh: vec![],
                source_statuses: vec![],
                decisions: vec![],
                asi_scores: asi_score_map.clone(),
                editor_notes: editor_notes.clone(),
                belief_notes_html: String::new(),
                css_content: String::new(),
                articles: vec![],
                watchlist_count: 0,
                mdx_output_dir: Some(PathBuf::from(mdx_out)),
                output_dir: vault_base.clone(),
                reflections: memory.all_reflections().to_vec(),
                thesis_decisions: thesis_decisions.clone(),
            };
            if let Err(e) = crate::renderer::publisher::MdxPublisher::new().publish(&mdx_ctx) {
                log::warn!("⚠️ MDX 输出失败: {}", e);
            }
        }
    }
    // 将置信度变化通知追加到 belief_notes_html
    if !outcome_notifications_html.is_empty() {
        belief_notes_html.push_str(&outcome_notifications_html);
    }

    log::info!("📊 {}", crate::llm::llm_audit_summary());

    if !event_log.all().is_empty() {
        if let Err(e) = event_log.save_to_file(&event_log_path.to_string_lossy()) {
            log::warn!("⚠️ EventLog 保存失败: {}", e);
        }
    }

    println!(
        "\n✅ EN 简报: {}",
        vault_base
            .join("en")
            .join(&today[..7])
            .join("index.html")
            .display()
    );
    println!(
        "✅ 看板: {}",
        vault_base.join("en").join("index.html").display()
    );
    Ok(())
}
