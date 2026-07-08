//! ArtifactSet — pipeline 输出契约
//!
//! publishing::agent_publish 的返回值类型。ownership 从 publishing 移交到 delivery。
//! 定义在此而非 artifact/ 模块，因为 ArtifactSet 是 domain contract 而非产物。

use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::domain::ThesisDecision;
use crate::domain::EditorNote;
use crate::engine::memory::MemoryEngine;
use crate::domain::premium::PremiumReport;
use crate::event_log::ObjectEvent;

/// Pipeline 输出的全部产物集合
pub struct ArtifactSet {
    // Cognitive outputs
    pub themes: Vec<Theme>,
    pub analyses: Vec<ThemeAnalysis>,
    pub analyses_zh: Vec<ThemeAnalysis>,

    // Memory & decisions
    pub memory: MemoryEngine,
    pub thesis_decisions: Vec<ThesisDecision>,
    pub premium_reports: Vec<PremiumReport>,

    // Editor notes
    pub editor_notes: Vec<EditorNote>,

    // Investigation reports (slug, content, assessment_id, inv_id)
    pub investigation_reports: Vec<(String, crate::domain::investigation::InvestigationReport, Option<String>, Option<String>)>,

    // Raw articles (for daily signal generation)
    pub new_articles: Vec<crate::fetcher::Article>,

    // Event log entries accumulated during publishing
    pub events: Vec<ObjectEvent>,

    // Translation coverage (Layer 2, transitional)
    pub translation_coverage: Option<crate::translation::TranslationCoverage>,

    // Pipeline metadata
    pub today: String,
    pub asi_score_map: std::collections::HashMap<String, (f64, f64, f64)>,
    pub belief_notes_html: String,
    pub refined_domains: std::collections::HashMap<String, (crate::domain::StrategicDomain, Vec<crate::domain::StrategicDomain>)>,

    // Report metadata for manifest
    pub assessment_count: usize,
    pub investigation_count: usize,
    pub decision_count: usize,
    pub archive_days: usize,
    pub total_signals: usize,
}

impl ArtifactSet {
    pub fn new(
        themes: Vec<Theme>,
        analyses: Vec<ThemeAnalysis>,
        analyses_zh: Vec<ThemeAnalysis>,
        memory: MemoryEngine,
        thesis_decisions: Vec<ThesisDecision>,
        premium_reports: Vec<PremiumReport>,
        editor_notes: Vec<EditorNote>,
        investigation_reports: Vec<(String, crate::domain::investigation::InvestigationReport, Option<String>, Option<String>)>,
        new_articles: Vec<crate::fetcher::Article>,
        events: Vec<ObjectEvent>,
        today: String,
        asi_score_map: std::collections::HashMap<String, (f64, f64, f64)>,
        belief_notes_html: String,
        refined_domains: std::collections::HashMap<String, (crate::domain::StrategicDomain, Vec<crate::domain::StrategicDomain>)>,
        assessment_count: usize,
        investigation_count: usize,
        decision_count: usize,
        archive_days: usize,
        total_signals: usize,
        translation_coverage: Option<crate::translation::TranslationCoverage>,
    ) -> Self {
        Self {
            themes, analyses, analyses_zh,
            memory, thesis_decisions, premium_reports,
            editor_notes, investigation_reports, new_articles,
            events, today, asi_score_map, belief_notes_html, refined_domains,
            assessment_count, investigation_count, decision_count,
            archive_days, total_signals,
            translation_coverage,
        }
    }
}
