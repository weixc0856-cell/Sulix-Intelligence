//! Editor Note — 分析结果对用户个人决策的影响记录
//!
//! EditorNote 原定义在 agent/editor.rs 中，但被 renderer 模块引用，
//! 造成展示层对 agent 层的依赖。作为纯数据结构，它属于领域层。
//! agent/editor.rs 通过 re-export 保持向后兼容。

use serde::{Deserialize, Serialize};

/// Editor 笔记：一条分析结果对你个人决策的影响
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorNote {
    /// 关联的用户决策问题 ID
    pub question_id: String,
    /// 关联的主题标题
    pub theme_title: String,
    /// 影响描述（人类可读）
    pub impact: String,
    /// 置信度变化 -10 到 +10
    pub confidence_delta: i8,
    /// 建议行动
    pub recommended_action: String,
    /// 该影响的依据
    pub rationale: String,
}
