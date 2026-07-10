//! ContentManifest — 存储快照
//!
//! 由 delivery::publisher 在验证门后生成（此时计数 = 真实上传数）。
//! 不是 publishing 阶段的产物。

use serde::{Deserialize, Serialize};
use std::path::Path;

/// 写入 JSON 文件的辅助函数
fn write_json<T: Serialize>(value: &T, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Content Manifest — 全站内容状态的权威数据源
///
/// 这是 sulix-engine ↔ sulix-web 之间的**唯一契约文件**。
/// Phase 0: 写入 output/manifest.json，同步到 frontend/public/manifest.json。
/// Phase 1+: manifest 退化为系统快照，业务数据由 Worker API 提供。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentManifest {
    pub contract_version: u32,
    pub version: u32,
    pub generated_at: String,
    pub date: String,
    pub daily_today: usize,
    pub assessments_active: usize,
    pub investigations: usize,
    pub decisions: usize,
    pub archive_days: usize,
    pub total_signals: usize,
    pub total_assessments: usize,
    /// 漏斗指标（信号生产管道各阶段计数）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funnel_fetched: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funnel_deduped: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funnel_scored: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funnel_tier2_intel: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funnel_tier3_research: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_calls_total: Option<u64>,
    pub pipeline_status: String,
    /// CI run ID (GITHUB_RUN_ID)，本地运行为 "local"
    #[serde(default)]
    pub pipeline_run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_observation_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_signal_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_counts: Option<std::collections::HashMap<String, usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_public_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stages: Option<Vec<crate::engine::pipeline_health::PipelineStage>>,
}

impl ContentManifest {
    pub const CONTRACT_VERSION: u32 = 2; // v2 adds funnel metrics
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        date: &str,
        prev_version: u32,
        frontend_public_dir: Option<String>,
        status: &str,
        observation_count: Option<usize>,
        signal_count: Option<usize>,
        duration_seconds: Option<f64>,
        stages: Option<Vec<crate::engine::pipeline_health::PipelineStage>>,
    ) -> Self {
        Self {
            contract_version: Self::CONTRACT_VERSION,
            version: prev_version + 1,
            generated_at: chrono::Utc::now().to_rfc3339(),
            date: date.to_string(),
            daily_today: signal_count.unwrap_or(0),
            assessments_active: 0,
            investigations: 0,
            decisions: 0,
            archive_days: 0,
            total_signals: 0,
            total_assessments: 0,
            funnel_fetched: None,
            funnel_deduped: None,
            funnel_scored: None,
            funnel_tier2_intel: None,
            funnel_tier3_research: None,
            llm_calls_total: None,
            pipeline_status: status.to_string(),
            pipeline_run_id: std::env::var("GITHUB_RUN_ID").unwrap_or_else(|_| "local".to_string()),
            pipeline_observation_count: observation_count,
            pipeline_signal_count: signal_count,
            output_counts: None,
            frontend_public_dir,
            duration_seconds,
            stages,
        }
    }

    /// 回填验证门后的真实计数
    pub fn with_counts(
        mut self,
        assessments: usize,
        investigations: usize,
        decisions: usize,
        archive_days: usize,
        total_signals: usize,
    ) -> Self {
        self.assessments_active = assessments;
        self.investigations = investigations;
        self.decisions = decisions;
        self.archive_days = archive_days;
        self.total_signals = total_signals;
        self.total_assessments = assessments;
        self
    }

    /// 回填漏斗指标
    pub fn with_funnel(
        mut self,
        fetched: usize,
        deduped: usize,
        scored: usize,
        tier2: usize,
        tier3: usize,
        llm_calls: u64,
    ) -> Self {
        self.funnel_fetched = Some(fetched);
        self.funnel_deduped = Some(deduped);
        self.funnel_scored = Some(scored);
        self.funnel_tier2_intel = Some(tier2);
        self.funnel_tier3_research = Some(tier3);
        self.llm_calls_total = Some(llm_calls);
        self
    }

    pub fn save_as_json(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        write_json(self, path)
    }
}
