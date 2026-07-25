use serde::{Deserialize, Serialize};

/// A reflection row from the D1 `reflections` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reflection {
    pub id: i64,
    pub decision_id: i64,
    pub outcome_id: Option<i64>,
    pub job_id: Option<String>,
    pub status: String,
    pub artifact_key: Option<String>,
    pub result: Option<String>,
    pub quality_score: Option<f64>,
    pub generator_version: Option<String>,
    pub lessons_count: i64,
    pub rules_count: i64,
    pub generated_by: String,
    pub retry_count: i64,
    pub last_error: Option<String>,
    pub started_at: Option<i64>,
    pub lease_until: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Input for inserting a new reflection row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewReflection {
    pub decision_id: i64,
    pub outcome_id: Option<i64>,
    pub job_id: Option<String>,
    pub status: String,
}

/// Input for updating reflection status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReflection {
    pub id: i64,
    pub status: String,
    pub result: Option<String>,
    pub quality_score: Option<f64>,
    pub artifact_key: Option<String>,
    pub lessons_count: Option<i64>,
    pub rules_count: Option<i64>,
    pub retry_count: Option<i64>,
    pub last_error: Option<String>,
    pub started_at: Option<i64>,
    pub lease_until: Option<i64>,
}
