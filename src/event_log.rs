//! 管线事件日志 — 不可变审计追踪
//!
//! 记录管线中的关键事件（主题创建、合并、SVI 变化、论题更新、冲突检测），
//! 支持 JSON 持久化与最近 N 条查询。
//!
//! # Object Event（审计线）
//! 每个 DEC/ASM 创建时产生一条 ObjectEvent 纯值，通过 ArtifactSet.events 收集，
//! 由 delivery::publisher 统一 flush 到 data/events/{date}.jsonl。
//! 不做 event sourcing（不从事件重放状态）。
//! 事件只含摘要字段，全量快照在 R2 objects/ 中。

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 管线事件类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PipelineEventType {
    /// 新主题被创建
    ThemeCreated,
    /// 两个主题被合并
    ThemeMerged,
    /// 主题的 SVI 评分发生变化
    SVIChanged,
    /// 论题被更新
    ThesisUpdated,
    /// 检测到信念冲突
    ConflictDetected,
    /// 现实结果与 Thesis 判断的偏差被记录（Meta Layer）
    OutcomeRecorded,
    /// Thesis 被证伪（Meta Layer）
    ThesisRefuted,
}

// ===== Step 2: Object Event 审计日志 =====

/// 对象生命周期事件类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectEventType {
    SignalCreated,
    AssessmentCreated,
    AssessmentUpdated,
    DecisionCreated,
    DecisionUpdated,
    OutcomeRecorded,
    ThesisRefuted,
    ReflectionGenerated,
}

/// 对象生命周期事件（审计线）
/// 只含摘要字段，全量快照在 R2 objects/ 中。
/// 由 delivery::publisher 统一 flush 到 data/events/{date}.jsonl。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectEvent {
    pub event_type: ObjectEventType,
    pub object_id: String,
    pub object_type: String,
    /// 摘要字段：decision 存 {confidence, asm_id}，outcome 存 {verdict, thesis_id}
    pub summary: serde_json::Value,
    pub source: String,
    pub timestamp: String,
}

impl ObjectEvent {
    pub fn new(
        event_type: ObjectEventType,
        object_id: &str,
        object_type: &str,
        summary: serde_json::Value,
        source: &str,
    ) -> Self {
        Self {
            event_type,
            object_id: object_id.to_string(),
            object_type: object_type.to_string(),
            summary,
            source: source.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// 单条管线事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineEvent {
    /// 唯一 ID（如 UUID）
    pub id: String,
    /// 事件类型
    pub event_type: PipelineEventType,
    /// 事件时间戳（RFC 3339 格式）
    pub timestamp: String,
    /// 人类可读描述
    pub description: String,
    /// 关联论题 ID（可选，用于按 Thesis 聚合查询）
    #[serde(default)]
    pub thesis_id: Option<String>,
    /// 关联事件 ID 列表（可选，用于构建事件链）
    #[serde(default)]
    pub related_events: Vec<String>,
    /// 事件附加数据（任意 JSON）
    pub data: serde_json::Value,
}

/// 事件日志 — 不可变追加式结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLog {
    entries: Vec<PipelineEvent>,
}

impl EventLog {
    /// 创建空的事件日志
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// 追加一条事件（自动裁剪超过 10K 的最旧条目）
    pub fn push(&mut self, event: PipelineEvent) {
        self.entries.push(event);
        if self.entries.len() > 10_000 {
            self.entries.remove(0);
        }
    }

    /// 获取所有事件
    pub fn all(&self) -> &[PipelineEvent] {
        &self.entries
    }

    /// 保存到 JSON 文件
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    /// 从 JSON 文件加载
    pub fn load_from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let log: EventLog = serde_json::from_str(&content)?;
        Ok(log)
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_log_push_and_all() {
        let mut log = EventLog::new();
        assert_eq!(log.all().len(), 0);

        log.push(PipelineEvent {
            id: "evt-1".into(),
            event_type: PipelineEventType::ThemeCreated,
            timestamp: "2026-06-24T10:00:00Z".into(),
            description: "主题 'AI 商品化' 已创建".into(),
            thesis_id: None,
            related_events: vec![],
            data: serde_json::json!({"theme_id": "t1"}),
        });
        log.push(PipelineEvent {
            id: "evt-2".into(),
            event_type: PipelineEventType::SVIChanged,
            timestamp: "2026-06-24T10:05:00Z".into(),
            description: "主题 'AI 商品化' SVI 从 6 变为 8".into(),
            thesis_id: None,
            related_events: vec![],
            data: serde_json::json!({"theme_id": "t1", "old_svi": 6, "new_svi": 8}),
        });

        assert_eq!(log.all().len(), 2);
    }

    #[test]
    fn test_event_log_save_load_roundtrip() {
        let mut log = EventLog::new();
        log.push(PipelineEvent {
            id: "evt-1".into(),
            event_type: PipelineEventType::ThesisUpdated,
            timestamp: "2026-06-24T12:00:00Z".into(),
            description: "论题 '模型商品化' 状态变更为 Strengthening".into(),
            thesis_id: Some("thesis-1".into()),
            related_events: vec![],
            data: serde_json::json!({"thesis_id": "thesis-1", "new_status": "Strengthening"}),
        });

        let path = std::env::temp_dir().join("test_event_log.json");
        log.save_to_file(path.to_str().unwrap()).unwrap();

        let loaded = EventLog::load_from_file(path.to_str().unwrap()).unwrap();
        assert_eq!(loaded.all().len(), 1);
        assert_eq!(loaded.all()[0].id, "evt-1");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_outcome_recorded_event_type() {
        let evt = PipelineEvent {
            id: "evt-3".into(),
            event_type: PipelineEventType::OutcomeRecorded,
            timestamp: "2026-06-24T14:00:00Z".into(),
            description: "实际结果与 Thesis 偏差记录".into(),
            thesis_id: Some("t2".into()),
            related_events: vec!["evt-1".into()],
            data: serde_json::json!({"deviation": "高估采用率"}),
        };
        assert_eq!(evt.event_type, PipelineEventType::OutcomeRecorded);
        assert_eq!(evt.related_events.len(), 1);
    }

    #[test]
    fn test_thesis_refuted_event_type() {
        let evt = PipelineEvent {
            id: "evt-4".into(),
            event_type: PipelineEventType::ThesisRefuted,
            timestamp: "2026-06-24T15:00:00Z".into(),
            description: "论题 '模型商品化' 被证伪".into(),
            thesis_id: Some("t3".into()),
            related_events: vec!["evt-1".into()],
            data: serde_json::json!({"reason": "关键假设被证伪"}),
        };
        assert_eq!(evt.event_type, PipelineEventType::ThesisRefuted);
        assert_eq!(evt.related_events.len(), 1);
    }

    #[test]
    fn test_event_log_cap_at_10k() {
        let mut log = EventLog::new();
        // 压入 10_001 条
        for i in 0..10_001 {
            log.push(PipelineEvent {
                id: format!("evt-{}", i),
                event_type: PipelineEventType::ThemeCreated,
                timestamp: "2026-06-24T10:00:00Z".into(),
                description: "cap test".into(),
                thesis_id: None,
                related_events: vec![],
                data: serde_json::json!({}),
            });
        }
        // 应有 10_000 条（最旧的 evt-0 被移除）
        assert_eq!(log.all().len(), 10_000);
        // 最旧应为 evt-1（evt-0 已被移除）
        assert_eq!(log.all().first().unwrap().id, "evt-1");
        // 最新应为 evt-10000
        assert_eq!(log.all().last().unwrap().id, "evt-10000");
    }
}
