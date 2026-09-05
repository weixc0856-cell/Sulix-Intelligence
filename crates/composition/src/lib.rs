//! Concrete wiring only — no business logic, adapters, DTOs, traits, handlers
//! or domain modules.
//!
//! `api`'s `worker::Router` handlers must name the concrete
//! [`AppServices<D1Store>`](application::AppServices) type (`worker::Router`
//! cannot route over a generic handler fn), but `api` must not depend on
//! `store` to spell `D1Store`. This crate is the single place that both
//! `application` (the generic bundle) and `store` (the concrete `D1Store`) are
//! in scope, so the production alias lives here. `api` and `worker-entry` only
//! import this alias.

/// Production service bundle: every application service wired to the D1 store.
pub type ProductionAppServices = application::AppServices<store::D1Store>;
