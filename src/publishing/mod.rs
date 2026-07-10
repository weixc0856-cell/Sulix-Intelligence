//! Publishing Agent — 5-stage 发布协调器
//!
//!   Preprocess → Generate → Infer → Persist → Emit
//!
//! 每个阶段是独立的子模块，agent_publish() 仅作为协调器。

mod emit;
mod generate;
pub(crate) mod helpers;
mod infer;
pub mod layer2;
mod persist;
mod preprocess;

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::agent::scan::ClassifiedSignal;
use crate::config::Config;
use crate::db::Database;
use crate::domain::artifact::ArtifactSet;

pub use helpers::{analyze_and_validate, ResearchOutput};

/// 发布产物计数统计（用于 manifest）
struct PublishCounts {
    assessment_count: usize,
    investigation_count: usize,
    archive_days: usize,
    total_signals: usize,
}

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
    intel_signals: &[ClassifiedSignal],
) -> Result<ArtifactSet> {
    let vault_base = PathBuf::from(&config.output.vault_path);

    // Stage 1: Preprocess — load all persistent state
    let mut state = preprocess::publish_preprocess(data_dir, config).await;

    // Stage 2: Generate — content creation (no state mutation)
    let (themes, analyses, analyses_zh, new_articles, triage) = research.destructure();
    let generated =
        generate::publish_generate(config, api_key, today, &themes, &analyses, &triage, &state)
            .await?;
    catalog.save_step(7, "summary", &generated.summary)?;
    catalog.save_step(8, "calibration", &generated.calibration_text)?;

    // Stage 3: Infer — run cognitive engines (Memory, Hermes, Decision)
    let mut inferred = infer::publish_infer(
        config,
        api_key,
        today,
        &themes,
        &analyses,
        &analyses_zh,
        &new_articles,
        &generated,
        &mut state,
        db,
    )
    .await?;

    // Stage 3.5: Memory 回流 — Intel signals → Thesis 轻量匹配
    let _intel_fed = inferred.memory.feed_intel(intel_signals, today);

    // Stage 4: Persist — write all state to disk
    persist::publish_persist(
        db,
        data_dir,
        today,
        entity_db,
        &mut state,
        &mut inferred,
        config,
    )
    .await;

    // Stage 5: Emit — render MDX/Markdown output
    emit::publish_emit(
        config,
        today,
        vault_base.clone(),
        &themes,
        &analyses,
        &new_articles,
        &inferred,
    )
    .await?;

    // Stage 5.5: Translation — fill zh-cn/zh-tw MDX via LLM (transitional)
    // Object-level i18n will replace this file-level step.
    // Failure degrades gracefully — does not block EN pipeline.
    let translation_coverage = {
        let cov = crate::translation::publish_translate(&config, &api_key).await;
        if cov.failed > 0 {
            log::warn!(
                "⚠️ Translation degraded: {}/{} files failed",
                cov.failed,
                cov.total_files
            );
        }
        Some(cov)
    };

    // Collect events from infer stage
    let events = inferred.events;

    // Stage 5.75: 构建 schema 验证对象（仅活跃集）
    use crate::domain::thesis::ThesisStatus;
    let assessment_objects: Vec<_> = inferred
        .memory
        .theses()
        .iter()
        .filter(|t| {
            matches!(
                t.status,
                ThesisStatus::Active | ThesisStatus::Strengthening | ThesisStatus::Weakening
            )
        })
        .map(|t| {
            let decision = inferred
                .thesis_decisions
                .iter()
                .find(|d| d.thesis_id == t.id);
            crate::schema::mapper::thesis_to_assessment(t, decision, "en")
        })
        .collect();
    let decision_objects: Vec<_> = inferred
        .memory
        .all_decisions()
        .iter()
        .map(|r| crate::schema::mapper::decision_record_to_object(r, "en"))
        .collect();

    // Count MDX outputs for manifest (pre-validation snapshot)
    let mdx_path = config.output.mdx_dir.as_ref().map(PathBuf::from);
    let counts = mdx_path
        .as_ref()
        .map(|p| {
            let count_md = |dir: &std::path::Path| -> usize {
                std::fs::read_dir(dir)
                    .map(|d| {
                        d.filter_map(|e| e.ok())
                            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                            .count()
                    })
                    .unwrap_or(0)
            };
            let count_dates = |dir: &std::path::Path| -> usize {
                std::fs::read_dir(dir)
                    .map(|d| {
                        let dates: std::collections::HashSet<String> = d
                            .filter_map(|e| e.ok())
                            .filter_map(|e| {
                                e.file_name()
                                    .to_str()
                                    .and_then(|n| n.get(..10))
                                    .filter(|s| s.chars().nth(4) == Some('-'))
                                    .map(|s| s.to_string())
                            })
                            .collect();
                        dates.len()
                    })
                    .unwrap_or(0)
            };
            PublishCounts {
                assessment_count: count_md(&p.join("thesis")),
                investigation_count: count_md(&p.join("investigation")),
                archive_days: count_dates(&p.join("daily")),
                total_signals: count_md(&p.join("daily")),
            }
        })
        .unwrap_or(PublishCounts {
            assessment_count: 0,
            investigation_count: 0,
            archive_days: 0,
            total_signals: 0,
        });

    // Build ArtifactSet — ownership transfers to delivery publisher
    let artifacts = ArtifactSet::new(
        themes,
        analyses,
        analyses_zh,
        inferred.memory,
        inferred.thesis_decisions,
        inferred.premium_reports,
        inferred.editor_notes,
        inferred.investigation_reports,
        new_articles,
        events,
        today.to_string(),
        inferred.asi_score_map,
        String::new(),
        inferred.refined_domains,
        assessment_objects,
        decision_objects,
        counts.assessment_count,
        counts.investigation_count,
        0,
        counts.archive_days,
        counts.total_signals,
        translation_coverage,
    );

    log::info!("📊 {}", crate::llm::llm_audit_summary());
    if !state.event_log.all().is_empty() {
        if let Err(e) = state
            .event_log
            .save_to_file(&state.event_log_path.to_string_lossy())
        {
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
    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use super::helpers::extract_entities;
    use super::*;
    use crate::domain::evidence::FactBaseEntry;
    use crate::domain::ThemeAnalysis;

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
        let analysis = make_analysis(vec![FactBaseEntry {
            evidence: "TSMC is expanding capacity".into(),
            interpretation: "".into(),
            confidence: "high".into(),
        }]);
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
        let analysis = make_analysis(vec![FactBaseEntry {
            evidence: "Some random company is growing".into(),
            interpretation: "".into(),
            confidence: "low".into(),
        }]);
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
