//! Signal Discovery Engine — semantic clustering for V2 signal detection.
//!
//! Discovers signals by clustering articles based on embedding similarity,
//! entity overlap, and temporal proximity. Runs alongside the entity-driven
//! engine and results are merged by the pipeline layer.

pub mod clustering;
pub mod converter;
pub mod retrieval;
pub mod similarity;
