//! Daily Intelligence Brief generator.
//!
//! Transforms signal inputs (owned [`types::BriefSignalInput`] DTOs, mapped
//! from store rows by the composition root) into a structured LLM-synthesised
//! briefing with insights, recommendations, and evidence.

pub mod context;
pub mod converter;
mod generator;
mod parser;
mod prompt;
pub mod types;

pub use generator::generate_daily_brief;
pub use types::{BriefArticleInput, BriefSignalInput, EvidenceArticle, RelatedEntityInput, SignalCandidate};
