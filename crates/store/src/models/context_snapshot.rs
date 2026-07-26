use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub id: String,
    pub query: String,
    pub intent: String,
    pub domain: Option<String>,
    pub engine_version: String,
    pub context_json: String,
    pub evidence_refs: Option<String>,
    pub confidence: f64,
    pub user_scope: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewContextSnapshot {
    pub id: String,
    pub query: String,
    pub intent: String,
    pub domain: Option<String>,
    pub context_json: String,
    pub evidence_refs: Option<String>,
    pub confidence: f64,
    pub user_scope: Option<String>,
}
