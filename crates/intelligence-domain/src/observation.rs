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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_serde_roundtrip() {
        let obs = Observation {
            id: 1,
            source_type: "RssFeed".into(),
            source_id: "feed-1".into(),
            title: "Test Article".into(),
            summary: Some("A summary".into()),
            url: Some("https://example.com".into()),
            content_hash: "abc123".into(),
            observed_at: 1000,
            registry_source_id: Some(1),
        };
        let json = serde_json::to_string(&obs).unwrap();
        let parsed: Observation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, "Test Article");
        assert_eq!(parsed.source_type, "RssFeed");
    }
}
