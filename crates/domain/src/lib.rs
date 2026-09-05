//! Domain ports + contracts — infra-free persistence traits, DTOs and errors.
//!
//! Hosted here are the persistence boundaries that the application layer
//! depends on: fine-grained repository/query traits (async, `?Send`, no
//! D1/worker in any signature), the pure-serde row DTOs they operate on, and
//! the shared `StoreError`.
//!
//! This crate MUST stay free of any host (`worker`) or concrete-infrastructure
//! dependency (`store`/`vectorize`/`embedding`/`event-store`/`object-store`/
//! `infrastructure`) — the P7 architecture guard (`shared-kernel/tests/
//! architecture.rs`) enforces that.  Concrete adapters (`D1Store`,
//! `MemoryStore`) live in `store` and implement these traits; `store`
//! re-exports this crate so `store::Article` / `store::FeedRepository`
//! keep working for existing callers.

pub mod models;

pub mod traits;
