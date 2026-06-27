use serde::{Deserialize, Serialize};

/// 问题-主题匹配结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionMatch {
    pub question_id: String,
    pub question_text: String,
    /// 相关性评分 0-10。只有实质性新数据或路径扭转才允许 ≥ 7
    pub relevance: u8,
    /// 简要推理过程
    pub reasoning: String,
    /// 该匹配对决策的影响: Support / Challenge / Neutral
    pub evidence_type: String,
}
