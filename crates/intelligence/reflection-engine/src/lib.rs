//! Reflection Engine — Decision Learning Loop's feedback node.
//!
//! Converts Decision + Thesis + Evidence + Outcome → Lessons + Decision Rules.
//! See: docs/superpowers/specs/2026-07-25-reflection-engine-design.md
//!
//! The engine depends on no infrastructure: reflection persistence is behind
//! the domain-owned [`ReflectionRepository`] port, whose D1 adapter lives in
//! `crates/infrastructure`.

pub mod context;
pub mod error;
pub mod generator;
pub mod repository;
pub mod validation;

mod service;

pub use repository::ReflectionRepository;
pub use service::{ReflectionEngine, ReflectionJob, ReflectionResult, ReflectionTrigger};
