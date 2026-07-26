//! Trait definitions for the DDD bounded-context boundaries.
//!
//! - [`repo`] — Aggregate persistence (save / find).  ~2-3 methods each.
//! - [`query`] — Read-model queries (list, radar, detail, stats).  ~5-15 methods each.
//!
//! The legacy [`StoreBackend`](crate::StoreBackend) supertrait composes all of
//! the above, so existing `T: StoreBackend` generic code continues to compile
//! without changes.

pub mod query;
pub mod repo;

pub use query::*;
pub use repo::*;
