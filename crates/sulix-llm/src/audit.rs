//! LLM 调用审计计数器 — 全局 AtomicU64 统计

use std::sync::atomic::{AtomicU64, Ordering};

/// 总调用次数
pub static LLM_CALL_COUNT: AtomicU64 = AtomicU64::new(0);
/// 估计输入 token 数（字符数 / 4 粗略估计）
pub static LLM_INPUT_TOKENS: AtomicU64 = AtomicU64::new(0);
/// 估计输出 token 数
pub static LLM_OUTPUT_TOKENS: AtomicU64 = AtomicU64::new(0);

/// 获取 LLM 审计统计摘要
pub fn llm_audit_summary() -> String {
    let calls = LLM_CALL_COUNT.load(Ordering::Relaxed);
    let input = LLM_INPUT_TOKENS.load(Ordering::Relaxed);
    let output = LLM_OUTPUT_TOKENS.load(Ordering::Relaxed);
    format!(
        "LLM 调用: {} 次, 输入 ~{}k tokens, 输出 ~{}k tokens",
        calls,
        input / 1000,
        output / 1000,
    )
}
