//! Reflection generators — LLM-backed reflection draft generation.

pub mod prompt;
pub mod real;
pub mod r#trait;

pub use r#trait::*;
pub use real::RealReflectionGenerator;
