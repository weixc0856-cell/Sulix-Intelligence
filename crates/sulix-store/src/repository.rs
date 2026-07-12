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

// ===== Event Store =====

/// Event Store — append-only event log
///
/// 系统中所有状态变化的不可变记录。
/// State projections (theses, decisions, signals) 从事件流派生。
pub trait EventStore {
    /// 追加单个事件
    fn append(&self, event: &contract::IntelligenceEvent) -> anyhow::Result<()>;

    /// 批量追加（同一事务内）
    fn append_many(&self, events: &[contract::IntelligenceEvent]) -> anyhow::Result<()>;

    /// 按聚合 ID 查询事件流（时间正序）
    fn event_stream(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> anyhow::Result<Vec<contract::IntelligenceEvent>>;

    /// 按事件类型查询（时间倒序）
    fn events_by_type(
        &self,
        event_type: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<contract::IntelligenceEvent>>;

    /// 查询所有事件（时间倒序，分页）
    fn find_all_events(&self, limit: usize) -> anyhow::Result<Vec<contract::IntelligenceEvent>>;
}

// ===== Unit of Work =====

/// 工作单元 — 保证事件 + 状态投影的原子写入
///
/// 使用方式:
///   let mut uow = store.transaction()?;
///   uow.events().append_many(&events)?;
///   uow.theses().save_many(&theses)?;
///   uow.commit()?;
///
/// 如果任何一步失败，调用 rollback() 回滚所有变更。
pub trait UnitOfWork {
    /// Event Store（当前事务内）
    fn events(&mut self) -> &dyn EventStore;

    /// Thesis 仓储（当前事务内）
    fn theses(&mut self) -> &dyn ThesisRepository;

    /// Decision 仓储（当前事务内）
    fn decisions(&mut self) -> &dyn DecisionRepository;

    /// 提交事务
    fn commit(&mut self) -> anyhow::Result<()>;

    /// 回滚事务
    fn rollback(&mut self) -> anyhow::Result<()>;
}

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
