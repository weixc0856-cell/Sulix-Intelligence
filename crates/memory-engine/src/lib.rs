//! Memory Engine — consolidates reflection outcomes into long-term memories.
//!
//! The engine depends on no infrastructure: memory persistence is behind the
//! domain-owned [`MemoryRepository`] port, whose D1 adapter lives in
//! `crates/infrastructure`.

pub mod candidate;
pub mod error;
pub mod evaluator;
pub mod model;
pub mod promotion;
pub mod repository;
pub mod worker;

pub use repository::MemoryRepository;
