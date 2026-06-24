//! 应用程序上下文 — 集中管理全局依赖
//!
//! 提供统一的访问点：LLM 客户端、数据库、信念引擎、配置。

use std::sync::Arc;

use crate::config::Config;
use crate::db::Database;
use crate::engine::memory::MemoryEngine;

/// 应用程序上下文
///
/// 通过 `Arc` 共享只读资源（Config），使用 `std::sync::Mutex` 包裹需要
/// 独占访问的可变资源（Database, MemoryEngine）。
pub struct AppContext {
    /// HTTP 客户端（用于 LLM API 调用）
    pub llm: reqwest::Client,
    /// SQLite 数据库（去重、趋势、墓地）
    pub db: std::sync::Mutex<Database>,
    /// 信念追踪引擎
    pub memory: std::sync::Mutex<MemoryEngine>,
    /// 应用配置
    pub config: Arc<Config>,
}

impl AppContext {
    /// 创建新的应用上下文
    pub fn new(
        llm: reqwest::Client,
        db: Database,
        memory: MemoryEngine,
        config: Config,
    ) -> Self {
        Self {
            llm,
            db: std::sync::Mutex::new(db),
            memory: std::sync::Mutex::new(memory),
            config: Arc::new(config),
        }
    }
}
