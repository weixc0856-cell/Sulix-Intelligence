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
    /// 从 pipeline 输出构建 manifest（不含 counts——由 delivery 回填）
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
            contract_version: 1,
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

    /// 回填验证门后的真实计数（由 delivery::publisher 在验证通过后调用）
    pub fn with_counts(mut self, assessments: usize, investigations: usize, decisions: usize, archive_days: usize, total_signals: usize) -> Self {
        self.assessments_active = assessments;
        self.investigations = investigations;
        self.decisions = decisions;
        self.archive_days = archive_days;
        self.total_signals = total_signals;
        self.total_assessments = assessments;
        self
    }

    pub fn save_as_json(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        write_json(self, path)
    }
}
