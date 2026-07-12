//! sulix-store — 持久化层
//!
//! 实现 ADR-011 (Database as State, Object Storage as Artifact):
//!   - 领域 Repository trait 在 `repository` 模块
//!   - SQLite 实现 在 `sqlite` 模块
//!
//! 当前状态:
//!   - Phase 1: SQLite 本地存储，与现有 JSON/MDX 双写
//!   - Phase 2+: D1 / Postgres 支持
//!
//! 架构引用:
//!   - PipelineStep trait (step.rs) — impl Future 模式
//!   - ripgrep Searcher/Matcher — 稳定内核 + 可替换边界

pub mod repository;
pub mod sqlite;

pub use repository::{DecisionRepository, EventStore, SignalRepository, ThesisRepository, UnitOfWork};
pub use sqlite::SqliteStore;
