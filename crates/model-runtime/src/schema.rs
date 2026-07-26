//! JSON schema helpers for structured model output.
//!
//! Provides factory functions for common output schemas used across
//! the intelligence pipeline (summarization, claim extraction, reflection).

/// Build a JSON schema for summarization output.
pub fn summary_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "json_object"
    })
}

/// Build a JSON schema for claim extraction output.
pub fn claim_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "json_object"
    })
}

/// Build a JSON schema for reflection output.
pub fn reflection_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "json_object"
    })
}
