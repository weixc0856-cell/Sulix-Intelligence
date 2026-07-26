//! Infrastructure layer — adapters for external dependencies.
//!
//! These types bridge domain traits (LLMProvider, etc.) to concrete
//! implementations from other crates (model-runtime, etc.).

pub mod model_provider;
