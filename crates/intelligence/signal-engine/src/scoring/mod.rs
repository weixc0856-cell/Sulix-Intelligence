//! Signal scoring — impact levels, health, and V2 semantic scoring.

pub mod health;
mod impact;
pub mod semantic;

pub use impact::score_to_impact;
