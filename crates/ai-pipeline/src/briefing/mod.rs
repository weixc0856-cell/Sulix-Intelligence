//! Daily Intelligence Brief generator.
//!
//! Transforms signals from [`store::signals_today`] into a structured
//! LLM-synthesised briefing with insights, recommendations, and evidence.

mod generator;
mod parser;
mod prompt;
pub mod types;

pub use generator::generate_daily_brief;
pub use types::SignalCandidate;
