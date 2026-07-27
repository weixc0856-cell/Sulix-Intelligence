//! Observation domain — the canonical external-world record.
//!
//! Every piece of content entering Sulix passes through Observation.
//! This is the root of the provenance chain.

use serde::{Deserialize, Serialize};

/// An observation — content ingested from an external source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: i64,
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub summary: Option<String>,
    pub url: Option<String>,
    pub content_hash: String,
    pub observed_at: i64,
    pub registry_source_id: Option<i64>,
}

/// Input for creating a new observation.
#[derive(Debug, Clone)]
pub struct NewObservation {
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub summary: Option<String>,
    pub url: Option<String>,
}
