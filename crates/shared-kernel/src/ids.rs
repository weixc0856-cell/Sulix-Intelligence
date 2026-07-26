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
