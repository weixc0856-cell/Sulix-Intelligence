use serde::{Deserialize, Serialize};

/// ConfidenceEvent — append-only 置信度变化事件。
///
/// 用于追踪 Decision / Claim / Signal 的置信度随时间演化的轨迹。
/// 每次 confidence 变化产生一条事件记录，不修改历史。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceEvent {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: String,
    pub previous_confidence: Option<f64>,
    pub confidence: f64,
    pub reason: Option<String>,
    pub trigger_event: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewConfidenceEvent {
    pub entity_type: String,
    pub entity_id: String,
    pub confidence: f64,
    pub reason: Option<String>,
    pub trigger_event: Option<String>,
}
