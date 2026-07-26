//! Model Runtime — abstraction layer for LLM providers.
//!
//! Every intelligence task (summarization, claim extraction, reflection,
//! agent answer) goes through the [`ModelProvider`] trait. Providers are
//! interchangeable: [`RealDeepSeek`] for production, [`NoopProvider`] for tests.
//!
//! This crate has **no Cloudflare Worker dependency** — it is pure Rust logic
//! that can be unit-tested without a Worker runtime. The only external
//! dependency is `reqwest` for HTTP (used by `RealDeepSeek`).

pub mod deepseek;
pub mod noop;
pub mod provider;
pub mod retry;
pub mod schema;
pub mod types;

pub use deepseek::{HttpClient, RealDeepSeek};
pub use noop::NoopProvider;
pub use provider::ModelProvider;
pub use retry::{is_transient_status, RetryPolicy};
pub use schema::{claim_schema, reflection_schema, summary_schema};
pub use types::{
    ContextBlock, GenerationParams, ModelCapabilities, ModelError, ModelRequest, ModelResponse, ModelTask, TokenUsage,
};
