use serde::{Deserialize, Serialize};

/// A row from the `event_archive_index` table — metadata for an archived event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventIndexEntry {
    pub id: i64,
    pub event_id: String,
    pub aggregate_type: String,
    pub aggregate_id: i64,
    pub event_type: String,
    pub object_key: String,
    pub occurred_at: i64,
    pub created_at: i64,
}
