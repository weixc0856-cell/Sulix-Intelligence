//! Reflection Engine — Decision Learning Loop's feedback node.
//!
//! Converts Decision + Thesis + Evidence + Outcome → Lessons + Decision Rules.
//! See: docs/superpowers/specs/2026-07-25-reflection-engine-design.md

pub mod context;
pub mod generator;
pub mod validation;

mod service;

pub use service::{ReflectionEngine, ReflectionJob, ReflectionResult, ReflectionTrigger};
