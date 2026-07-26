//! Application layer — UseCase orchestration services.
//!
//! Every UseCase in this crate:
//! - Is **generic over its dependencies** (typically `StoreBackend` subtraits)
//! - Contains **zero HTTP / Worker code**
//! - Is **unit-testable** with `MemoryStore`
//! - Returns domain types, not HTTP responses
//!
//! API controllers in the `api` crate parse HTTP requests, call these
//! services, and convert results to JSON responses.

pub mod graph;
pub mod radar;
pub mod semantic_search;

pub use graph::{ExpandRequest, ExpandResponse, GraphProjectionService};
pub use radar::RadarProjectionService;
pub use semantic_search::SemanticSearchService;
