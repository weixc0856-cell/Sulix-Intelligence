//! PipelineHealth — 管线运行状态报告
//!
//! 每日生成 `data/YYYY-MM-DD/pipeline_report.json`，记录各阶段输入/输出/状态。
//! 支持趋势追踪和异常检测（连续 N 天 0 themes = alert）。

use serde::{Deserialize, Serialize};
use std::path::Path;

/// 管线运行报告
///
/// 顶层聚合字段（`observation_count` 等）是前端消费的稳定接口。
/// 前端不应直接解析 `stages` 内部来获取计数——stages 是内部审计用途。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineReport {
    pub date: String,
    pub duration_seconds: f64,
    /// 原始抓取信号总数（所有源之和，去重前）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation_count: Option<usize>,
    /// 去重后今日新增信号数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_count: Option<usize>,
    /// 聚类后主题数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_count: Option<usize>,
    /// 活跃 thesis 数（非 Retired）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assessment_count: Option<usize>,
    /// 有 decision 标签的 thesis 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_count: Option<usize>,
    /// 生成的 Investigation reports 数量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub investigation_count: Option<usize>,
    /// 信号按分类的分布（前端 Observation 漏斗分解用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_counts: Option<std::collections::HashMap<String, usize>>,
    pub stages: Vec<PipelineStage>,
    pub status: PipelineStatus,
}

/// 管线阶段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStage {
    pub name: String,
    pub input_count: usize,
    pub output_count: usize,
    pub status: StageStatus,
}

/// 阶段状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StageStatus {
    Success,
    Skipped,
    Failed,
}

/// 管线整体状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PipelineStatus {
    /// 所有阶段完成且有产出
    Success,
    /// 管线完成但产出为零（0 themes 等）
    NoOutput,
    /// 部分阶段失败
    PartialFailure,
    /// 管线未完成（早期退出）
    StoppedEarly,
}

impl PipelineReport {
    /// 创建新报告
    pub fn new(date: &str) -> Self {
        Self {
            date: date.to_string(),
            duration_seconds: 0.0,
            observation_count: None,
            signal_count: None,
            theme_count: None,
            assessment_count: None,
            decision_count: None,
            investigation_count: None,
            category_counts: None,
            stages: Vec::new(),
            status: PipelineStatus::Success,
        }
    }

    /// 添加阶段记录
    pub fn add_stage(
        &mut self,
        name: impl Into<String>,
        input: usize,
        output: usize,
        status: StageStatus,
    ) {
        if status == StageStatus::Failed {
            self.status = PipelineStatus::PartialFailure;
        }
        self.stages.push(PipelineStage {
            name: name.into(),
            input_count: input,
            output_count: output,
            status,
        });
    }

    /// 保存到 data/YYYY-MM-DD/pipeline_report.json
    pub fn save(&self, data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let dir = data_dir.join(&self.date);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("pipeline_report.json");
        self.save_as_json(&path)
    }

    /// 保存到指定路径（用于 vault 同步到前端）
    pub fn save_as_json(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
