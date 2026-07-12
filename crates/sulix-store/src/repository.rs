//! Repository traits — 领域持久化契约
//!
//! 定义存储层与领域层之间的接口。
//! 每个 trait 对应 sulix-contract 中的一个领域类型。
//!
//! 设计原则：
//! - 不依赖任何存储实现（SQLite / D1 / Postgres 均可实现）
//! - 方法命名面向领域查询（find_active, find_by_date），
//!   而非存储操作（select, insert）
//! - 使用 impl Future 模式（同 PipelineStep trait），
//!   不引入 async_trait 依赖
//!
//! 架构引用：ADR-011 (Database as State, Object Storage as Artifact)

use sulix_contract as contract;

// ===== Thesis Repository =====

/// Thesis 持久化契约
///
/// 当前唯一实现: SqliteThesisRepository (crate::sqlite::thesis)
/// 未来可能: D1ThesisRepository (Cloudflare), PgThesisRepository
///
/// 注: 不要求 Send + Sync（rusqlite Connection 不是 Sync），
///     仓储在 pipeline 完成后同步使用，无需跨线程 Send。
pub trait ThesisRepository {
    /// 保存单个 Thesis（INSERT OR REPLACE）
    fn save(&self, thesis: &contract::Thesis) -> anyhow::Result<()>;

    /// 批量保存
    fn save_many(&self, theses: &[contract::Thesis]) -> anyhow::Result<()>;

    /// 查询所有活跃 Thesis（非 Dormant/Retired）
    fn find_active(&self) -> anyhow::Result<Vec<contract::Thesis>>;

    /// 按 ID 查询
    fn find_by_id(&self, id: &str) -> anyhow::Result<Option<contract::Thesis>>;

    /// 查询全部
    fn find_all(&self) -> anyhow::Result<Vec<contract::Thesis>>;
}

// ===== Decision Repository =====

/// Decision 持久化契约
pub trait DecisionRepository {
    /// 保存单个 Decision
    fn save(&self, decision: &contract::Decision) -> anyhow::Result<()>;

    /// 批量保存
    fn save_many(&self, decisions: &[contract::Decision]) -> anyhow::Result<()>;

    /// 按 Thesis ID 查询相关决策
    fn find_by_thesis_id(&self, thesis_id: &str) -> anyhow::Result<Vec<contract::Decision>>;

    /// 查询最新 N 条决策
    fn find_latest(&self, limit: usize) -> anyhow::Result<Vec<contract::Decision>>;

    /// 查询全部
    fn find_all(&self) -> anyhow::Result<Vec<contract::Decision>>;
}

// ===== Signal Repository =====

/// Signal 持久化契约
pub trait SignalRepository {
    /// 保存单个 Signal
    fn save(&self, signal: &contract::Signal) -> anyhow::Result<()>;

    /// 批量保存
    fn save_many(&self, signals: &[contract::Signal]) -> anyhow::Result<()>;

    /// 按日期查询（created_at 前缀匹配）
    fn find_by_date(&self, date: &str) -> anyhow::Result<Vec<contract::Signal>>;

    /// 查询全部
    fn find_all(&self) -> anyhow::Result<Vec<contract::Signal>>;
}
