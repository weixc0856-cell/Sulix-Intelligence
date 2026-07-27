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
