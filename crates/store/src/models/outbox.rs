use serde::{Deserialize, Serialize};

/// A row in the `object_outbox` table — a pending R2 archive write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEntry {
    pub id: i64,
    pub object_type: String,
    pub object_key: String,
    pub payload: String,
    pub status: String,
    pub created_at: i64,
    pub retry_count: i64,
}

/// Parameters for enqueuing a new outbox entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewOutbox {
    pub object_type: String,
    pub object_key: String,
    pub payload: String,
}
