//! Claim Engine — Evidence → Claim intelligence pipeline.
//!
//! Extracts atomic, falsifiable claims from articles using LLM reasoning,
//! then evaluates confidence using the ConfidenceEngine v2 factors.

pub mod confidence;
pub mod domain;
pub mod extractor;
pub mod llm;
pub mod parser;
pub mod prompt;

pub use confidence::evaluate_claim_confidence;
pub use domain::{ClaimCandidate, ClaimType, EvidenceRef, Uncertainty};
pub use extractor::ClaimExtractor;
pub use llm::LlmClaimExtractor;
