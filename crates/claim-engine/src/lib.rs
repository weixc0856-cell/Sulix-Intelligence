//! Claim Engine — Evidence → Claim intelligence pipeline.
//!
//! ╔══════════════════════════════════════════════════════════════════╗
//! ║  DEPRECATED — Sprint 6.2D                                      ║
//! ║                                                                  ║
//! ║  Domain types have moved to `intelligence-domain` crate.        ║
//! ║  This crate is retained for backward compat:                    ║
//! ║  - ClaimExtractor / LlmClaimExtractor (LLM logic)               ║
//! ║  - Confidence calculator pipeline                               ║
//! ║                                                                  ║
//! ║  New code should use `intelligence_domain::*` for domain types. ║
//! ║  TODO (Sprint 6.2E+): migrate consumers then remove this crate. ║
//! ╚══════════════════════════════════════════════════════════════════╝
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
