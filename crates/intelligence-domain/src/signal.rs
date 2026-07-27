//! Signal domain — long-lived intelligence assets derived from claim patterns.

use serde::{Deserialize, Serialize};

/// A signal thread — a persistent intelligence asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalThread {
    pub id: i64,
    pub signal_key: String,
    pub title: String,
    pub status: SignalStatus,
    pub score: f64,
    pub trend: String,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SignalStatus {
    Active,
    Decaying,
    Resolved,
    Archived,
}

/// A signal instance — a daily score snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalInstance {
    pub id: i64,
    pub thread_id: i64,
    pub score: f64,
    pub impact: String,
    pub trend: String,
    pub recorded_at: i64,
}

/// Input for upserting a signal thread.
#[derive(Debug, Clone)]
pub struct NewSignalThread {
    pub signal_key: String,
    pub title: String,
    pub score: f64,
    pub anchor_entity_id: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_status_serde() {
        let json = serde_json::to_string(&SignalStatus::Active).unwrap();
        assert_eq!(json, "\"active\"");
        let parsed: SignalStatus = serde_json::from_str("\"archived\"").unwrap();
        assert_eq!(parsed, SignalStatus::Archived);
    }

    #[test]
    fn signal_status_from_string() {
        assert_eq!(serde_json::from_str::<SignalStatus>("\"decaying\"").unwrap(), SignalStatus::Decaying);
        assert_eq!(serde_json::from_str::<SignalStatus>("\"resolved\"").unwrap(), SignalStatus::Resolved);
    }

    #[test]
    fn signal_instance_serde() {
        let inst = SignalInstance {
            id: 1,
            thread_id: 42,
            score: 0.75,
            impact: "medium".into(),
            trend: "rising".into(),
            recorded_at: 1000,
        };
        let json = serde_json::to_string(&inst).unwrap();
        let parsed: SignalInstance = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.thread_id, 42);
        assert!((parsed.score - 0.75).abs() < f64::EPSILON);
    }
}
