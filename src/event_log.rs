//! 管线事件日志 — 不可变审计追踪
//!
//! 记录管线中的关键事件（主题创建、合并、SVI 变化、论题更新、冲突检测），
//! 支持 JSON 持久化与最近 N 条查询。

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

    /// 追加一条事件
    pub fn push(&mut self, event: PipelineEvent) {
        self.entries.push(event);
    }

    /// 获取所有事件
    pub fn all(&self) -> &[PipelineEvent] {
        &self.entries
    }

    /// 获取最近 N 条事件（按追加顺序倒序）
    pub fn recent(&self, n: usize) -> Vec<&PipelineEvent> {
        self.entries
            .iter()
            .rev()
            .take(n)
            .collect()
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
    fn test_event_log_push_and_recent() {
        let mut log = EventLog::new();
        assert_eq!(log.all().len(), 0);

        log.push(PipelineEvent {
            id: "evt-1".into(),
            event_type: PipelineEventType::ThemeCreated,
            timestamp: "2026-06-24T10:00:00Z".into(),
            description: "主题 'AI 商品化' 已创建".into(),
            data: serde_json::json!({"theme_id": "t1"}),
        });
        log.push(PipelineEvent {
            id: "evt-2".into(),
            event_type: PipelineEventType::SVIChanged,
            timestamp: "2026-06-24T10:05:00Z".into(),
            description: "主题 'AI 商品化' SVI 从 6 变为 8".into(),
            data: serde_json::json!({"theme_id": "t1", "old_svi": 6, "new_svi": 8}),
        });

        assert_eq!(log.all().len(), 2);

        let recent = log.recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, "evt-2");
    }

    #[test]
    fn test_event_log_save_load_roundtrip() {
        let mut log = EventLog::new();
        log.push(PipelineEvent {
            id: "evt-1".into(),
            event_type: PipelineEventType::ThesisUpdated,
            timestamp: "2026-06-24T12:00:00Z".into(),
            description: "论题 '模型商品化' 状态变更为 Strengthening".into(),
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
    fn test_event_log_empty_recent() {
        let log = EventLog::new();
        let recent = log.recent(5);
        assert!(recent.is_empty());
    }
}
