//! Application services — orchestration layer between API and domain.
//!
//! Each service is generic over its dependencies (repository traits, outbox
//! publishers) so callers in `worker-entry` can wire concrete infrastructure
//! implementations at the composition root.

pub mod articles;
pub mod briefing;
pub mod claims;
pub mod compliance;
pub mod confidence;
pub mod decision;
pub mod entities;
pub mod feeds;
pub mod observations;
pub mod rules;
pub mod sources;
pub mod strategies;
pub mod system;
pub mod trust;
