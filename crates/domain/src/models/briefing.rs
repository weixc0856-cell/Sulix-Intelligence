use serde::{Deserialize, Serialize};

/// Summary of a historical briefing for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingSummary {
    pub id: i64,
    pub date: String,
    pub generated_at: i64,
    pub signal_count: u32,
    pub created_at: i64,
}
