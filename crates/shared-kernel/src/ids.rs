//! Strongly-typed identifiers for all domains.
//!
//! Each id wraps a `String` and provides domain-specific formatting.
//! Using newtypes instead of raw `i64`/`String` prevents cross-domain
//! ID confusion at compile time.

use serde::{Deserialize, Serialize};

/// ID for an Observation (external knowledge artifact).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservationId(pub String);

impl ObservationId {
    pub fn new(id: i64) -> Self {
        Self(format!("OBS-{id:06}"))
    }
}

/// ID for a Signal Thread.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SignalId(pub String);

impl SignalId {
    pub fn new(id: i64) -> Self {
        Self(format!("SIG-{id:06}"))
    }
}

/// ID for a Decision.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecisionId(pub String);

impl DecisionId {
    pub fn new(id: i64) -> Self {
        Self(format!("DEC-{id:06}"))
    }
}

/// ID for an Outcome.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutcomeId(pub String);

impl OutcomeId {
    pub fn new(id: i64) -> Self {
        Self(format!("OUT-{id:06}"))
    }
}

/// ID for a Reflection.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReflectionId(pub String);

impl ReflectionId {
    pub fn new(id: i64) -> Self {
        Self(format!("REF-{id:06}"))
    }
}

/// ID for a Memory entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryId(pub String);

impl MemoryId {
    pub fn new(id: i64) -> Self {
        Self(format!("MEM-{id:06}"))
    }
}

/// ID for a Source (RSS/Atom feed).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub String);

impl SourceId {
    pub fn new(id: i64) -> Self {
        Self(format!("SRC-{id:06}"))
    }
}

/// ID for an Entity (knowledge graph).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub String);

impl EntityId {
    pub fn new(id: i64) -> Self {
        Self(format!("ENT-{id:06}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_id_types_format_with_prefix_and_zero_padding() {
        assert_eq!(ObservationId::new(1).0, "OBS-000001");
        assert_eq!(SignalId::new(1).0, "SIG-000001");
        assert_eq!(DecisionId::new(1).0, "DEC-000001");
        assert_eq!(OutcomeId::new(1).0, "OUT-000001");
        assert_eq!(ReflectionId::new(1).0, "REF-000001");
        assert_eq!(MemoryId::new(1).0, "MEM-000001");
        assert_eq!(SourceId::new(1).0, "SRC-000001");
        assert_eq!(EntityId::new(1).0, "ENT-000001");
    }

    #[test]
    fn ids_zero_pad_small_and_pass_through_large() {
        assert_eq!(DecisionId::new(42).0, "DEC-000042");
        assert_eq!(DecisionId::new(999_999).0, "DEC-999999");
        // Beyond 6 digits: no truncation, no padding.
        assert_eq!(DecisionId::new(1_000_000).0, "DEC-1000000");
        // Negative ids are a caller error (DB assigns positive ids) but still
        // format consistently: sign included, magnitude zero-padded.
        assert_eq!(DecisionId::new(-1).0, "DEC--00001");
    }

    #[test]
    fn ids_are_distinct_across_domains_even_for_same_number() {
        assert_ne!(DecisionId::new(1).0, SignalId::new(1).0);
        assert_ne!(ObservationId::new(1).0, OutcomeId::new(1).0);
    }

    #[test]
    fn id_equality_is_content_based() {
        assert_eq!(DecisionId::new(1), DecisionId::new(1));
        assert_ne!(DecisionId::new(1), DecisionId::new(2));
    }

    #[test]
    fn id_serde_round_trips() {
        let dec = DecisionId::new(42);
        assert_eq!(serde_json::from_str::<DecisionId>(&serde_json::to_string(&dec).unwrap()).unwrap(), dec);
        let sig = SignalId::new(0);
        assert_eq!(serde_json::from_str::<SignalId>(&serde_json::to_string(&sig).unwrap()).unwrap(), sig);
        let ent = EntityId::new(1_000_000);
        assert_eq!(serde_json::from_str::<EntityId>(&serde_json::to_string(&ent).unwrap()).unwrap(), ent);
    }
}
