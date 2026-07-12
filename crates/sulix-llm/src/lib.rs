//! LLM 分析模块 — 多 Provider 调用
//!
//! 职责拆分（参考 ripgrep 的模块化设计）:
//!   - client:  HTTP 客户端工厂
//!   - retry:   指数退避重试机制
//!   - api:     LLM API 调用（请求 + 响应处理）
//!   - parser:  多策略 JSON 解析
//!   - dispatch: Provider 派发枚举（类比 ripgrep PatternMatcher）
//!   - audit:   调用审计计数器
//!   - types:   LLM 输入/输出数据类型

pub mod api;
pub mod audit;
pub mod client;
pub mod dispatch;
pub mod parser;
pub mod retry;
pub mod types;

// ===== 向后兼容的 re-exports =====

pub use api::{call_with_retry, call_with_retry_raw};
pub use audit::{llm_audit_summary, LLM_CALL_COUNT, LLM_INPUT_TOKENS, LLM_OUTPUT_TOKENS};
pub use client::{create_client, create_llm_client, create_source_client};
pub use dispatch::{LlmChoice, LlmProviderDispatch};
pub use parser::{
    extract_json_block, extract_json_block_flexible, parse_json_lenient, parse_json_response,
};
pub use retry::{with_retry, MAX_RETRIES};
pub use types::{AnalyzedArticle, AnalyzedArticleRaw, VerticalAnalysis};
