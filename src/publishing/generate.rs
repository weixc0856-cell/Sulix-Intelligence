//! Stage 2: Generate — Premium reports, ASI scores, summary, calibration, change detection

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::config::Config;
use crate::domain::EditorNote;
use crate::domain::StrategicDomain;
use crate::archive::ChronicleEntry;

use super::preprocess::StateBundle;

/// Stage 2: Generate 阶段产出的所有内容
pub struct GeneratedAssets {
    pub asi_score_map: HashMap<String, (f64, f64, f64)>,
    pub premium_reports: Vec<crate::domain::PremiumReport>,
    pub belief_notes_html: String,
    pub editor_notes: Vec<EditorNote>,
    pub change_summary: crate::hermes::ChangeSummary,
    pub calibration_text: String,
    pub summary: crate::domain::theme::Summary,
    pub refined_domains: HashMap<String, (StrategicDomain, Vec<StrategicDomain>)>,
}

const SVI_MIN_LOG: f64 = 6.0;
const SVI_MIN_PREMIUM: u8 = 7;

/// ASI 评分 + Premium 研报生成（每个主题的 SVI/ASI/Confidence 三元组 + 可选 premium HTML）
#[allow(clippy::too_many_arguments)]
async fn generate_asi_and_premium(
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    config: &Config,
    api_key: &str,
    today: &str,
) -> (HashMap<String, (f64, f64, f64)>, Vec<crate::domain::PremiumReport>) {
    let vault_base = PathBuf::from(&config.output.vault_path);
    let premium_dir = vault_base.join("premium");
    let _ = fs::create_dir_all(&premium_dir);

    let mut asi_score_map: HashMap<String, (f64, f64, f64)> = HashMap::new();
    let mut premium_reports: Vec<crate::domain::PremiumReport> = vec![];

    for (theme, analysis) in themes.iter().zip(analyses.iter()) {
        // Fallback 主题不触发 Premium 研报生成（无语义价值，不浪费 LLM）
        if theme.is_fallback {
            continue;
        }
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
        let asi_result = crate::engine::analysis::asi::calculate_asi(analysis.signal_strength, max_days_old, &asi_config);
        let confidence_config = crate::engine::analysis::asi::ConfidenceConfig::default();
        let confidence_result = crate::engine::analysis::asi::calculate_confidence(
            &analysis.evidence_level, analysis.signal_strength, theme.sources.len(), &confidence_config,
        );
        let final_val = crate::engine::analysis::asi::final_value(svi, &asi_result, &confidence_result);
        asi_score_map.insert(theme.title.clone(), (asi_result.asi, confidence_result.confidence, final_val));

        if final_val >= SVI_MIN_LOG {
            log::info!("⭐ ASI: {} (SVI={}, ASI={:.2}, Confidence={:.2}, final={:.1})",
                theme.title, svi, asi_result.asi, confidence_result.confidence, final_val);
        }
        if svi < SVI_MIN_PREMIUM { continue; }

        let theme_context: String = theme.articles.iter()
            .map(|a| format!("- [{}] {}: {}", a.source, a.title, a.summary.as_deref().unwrap_or("")))
            .collect::<Vec<_>>().join("\n");
        match crate::engine::premium::generate_premium_report(theme, &theme_context, api_key, &config.llm, config.prompts.as_ref()).await {
            Ok(report) => {
                if let Ok(html) = crate::renderer::render_premium_report(&report) {
                    let slug = theme.title.to_lowercase().replace(' ', "-");
                    let _ = fs::write(premium_dir.join(format!("{}.html", slug)), &html);
                }
                if let Some(ref sub) = config.substack {
                    if sub.enabled {
                        if let Err(e) = crate::engine::premium::push_to_substack(&report, &sub.api_key, &sub.publication_url).await {
                            log::warn!("⚠️ Substack push failed [{}]: {}", theme.title, e);
                        }
                    }
                }
                premium_reports.push(report);
            }
            Err(e) => log::warn!("⚠️ Premium 研报失败 [{}]: {}", theme.title, e),
        }
    }
    (asi_score_map, premium_reports)
}

/// Belief Engine: 从配置加载核心信念并更新
fn run_belief_engine(config: &Config, analyses: &[ThemeAnalysis], today: &str) -> String {
    let mut belief_engine = crate::engine::belief::BeliefEngineV2::new();
    if let Some(ref config_beliefs) = config.beliefs {
        let core_beliefs: Vec<crate::engine::belief::CoreBelief> = config_beliefs.iter()
            .map(|b| crate::engine::belief::CoreBelief {
                id: b.id.clone(), statement: b.statement.clone(),
                confidence: b.confidence, category: b.category.clone(), history: vec![],
            }).collect();
        belief_engine.load_from_config(&core_beliefs);
        belief_engine.update_from_analyses(analyses, today);
        let recent = belief_engine.recent_changes(5);
        if !recent.is_empty() {
            log::info!("🎯 Belief Engine: {} 项信念更新", recent.len());
        }
    }
    crate::engine::belief::render_belief_changes_html(&belief_engine)
}

/// 认知校准：生成扎心问题
async fn generate_calibration(analyses: &[ThemeAnalysis], api_key: &str, config: &Config) -> Result<String> {
    if analyses.is_empty() { return Ok(String::new()); }
    let calibration_input: Vec<crate::llm::VerticalAnalysis> = analyses.iter()
        .map(|ta| crate::llm::VerticalAnalysis { category: ta.theme_title.clone(), articles: vec![] }).collect();
    crate::agent::calibration::calibrate(&calibration_input, api_key, &config.llm, config.prompts.as_ref(), "en").await
}

/// Change Detection: 检测今日信号与历史的冲突/强化关系
/// 支持 LLM 语义版（异步）和规则版（同步 fallback）
async fn run_change_detection(
    state: &StateBundle,
    analyses: &[ThemeAnalysis],
    config: &Config,
    api_key: &str,
) -> crate::hermes::ChangeSummary {
    let recent_entries: Vec<ChronicleEntry> = state.chronicle.as_ref()
        .map(|c| c.sorted().into_iter().take(50).collect()).unwrap_or_default();

    let change_summary = if config.news_layer.as_ref().map(|n| n.llm_change_detection).unwrap_or(false) {
        crate::hermes::detect_changes_llm(&recent_entries, analyses, api_key, &config.llm).await
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
    change_summary
}

/// Strategic Domain Classification: LLM refine for low-confidence topics
async fn refine_domains(themes: &[Theme], config: &Config, api_key: &str) -> HashMap<String, (StrategicDomain, Vec<StrategicDomain>)> {
    let mut refined_domains: HashMap<String, (StrategicDomain, Vec<StrategicDomain>)> = HashMap::new();
    for theme in themes {
        let text = format!("{} {}", theme.title,
            theme.articles.first().and_then(|a| a.summary.as_deref()).unwrap_or(""));
        if crate::domain::StrategicDomain::is_classify_low_confidence(&text) {
            let system_prompt = crate::domain::StrategicDomain::llm_classification_prompt();
            match crate::llm::call_and_parse(api_key, &config.llm, system_prompt, &text).await {
                Ok(response) if crate::domain::StrategicDomain::validate_llm_output(&response) => {
                    let (primary, secondary) = crate::domain::StrategicDomain::parse_llm_response(&response);
                    log::info!("🧠 Domain classify [{}]: {} (LLM-refined)", theme.title, primary.label());
                    refined_domains.insert(theme.title.clone(), (primary, secondary));
                }
                _ => {}
            }
        }
    }
    if !refined_domains.is_empty() {
        log::info!("🧠 Domain classification: {} topics LLM-refined", refined_domains.len());
    }
    refined_domains
}

/// Generate: Premium 报告 + ASI 评分 + 合成摘要 + 认知校准 + Change Detection
pub async fn publish_generate(
    config: &Config,
    api_key: &str,
    today: &str,
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    triage: &crate::agent::scan::TriageResult,
    state: &StateBundle,
) -> Result<GeneratedAssets> {
    // 1. ASI 评分 + Premium 研报
    let (asi_score_map, premium_reports) = generate_asi_and_premium(themes, analyses, config, api_key, today).await;

    // 2. Twitter/X 推文
    if let Some(ref twitter_config) = config.twitter {
        crate::twitter::publish_tweets(themes, analyses, twitter_config).await;
    }

    // 3. Belief Engine
    let belief_notes_html = run_belief_engine(config, analyses, today);

    // 4. 合成摘要
    let summary = crate::clusterer::synthesize(themes, analyses);
    log::info!("✅ 聚类完成: {} 个主题, {} 篇文章", summary.theme_count, summary.total_articles);

    // 5. 认知校准
    let calibration_text = generate_calibration(analyses, api_key, config).await?;
    log::info!("📝 分析主题: {} 个, 信号: {} 条",
        analyses.len(), themes.iter().map(|t| t.articles.len()).sum::<usize>() + triage.watchlist.len());

    // 6. Editor Notes
    let editor_notes = crate::agent::editor::analyze_personal_impact(analyses, state.memory_for_linking.theses());
    if !editor_notes.is_empty() {
        log::info!("👤 Editor Agent: {} 项个人影响分析", editor_notes.len());
    }

    // 7. Change Detection
    let change_summary = run_change_detection(state, analyses, config, api_key).await;

    // 8. Strategic Domain Classification
    let refined_domains = refine_domains(themes, config, api_key).await;

    Ok(GeneratedAssets {
        asi_score_map, premium_reports, belief_notes_html,
        editor_notes, change_summary, calibration_text,
        summary, refined_domains,
    })
}
