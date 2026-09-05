use serde::{Deserialize, Serialize};

/// ConfidenceEvent — append-only 置信度变化事件。
///
/// 用于追踪 Decision / Claim / Signal 的置信度随时间演化的轨迹。
/// 每次 confidence 变化产生一条事件记录，不修改历史。
///
/// Sprint 5.8: 增加 factors_json 字段，记录因子分解。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceEvent {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: String,
    pub previous_confidence: Option<f64>,
    pub confidence: f64,
    pub reason: Option<String>,
    pub trigger_event: Option<String>,
    /// Sprint 5.8: JSON-encoded ConfidenceFactorExplanation list.
    pub factors_json: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewConfidenceEvent {
    pub entity_type: String,
    pub entity_id: String,
    pub confidence: f64,
    pub reason: Option<String>,
    pub trigger_event: Option<String>,
    /// Sprint 5.8: Optional factor explanations.
    pub factors_json: Option<String>,
}
