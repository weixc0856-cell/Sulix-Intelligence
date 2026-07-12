//! SQLite 持久化实现 — 领域仓储
//!
//! 每个 Repository 实现在同个 SQLite 连接上操作各自的数据表。
//! SqliteStore 是单一入口点，打开连接并初始化所有表结构。
//!
//! 架构原则（ADR-011）:
//! - Database = 认知状态（theses, decisions, signals）
//! - R2 = 不可变资产（原始文章，导出 MDX）

use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection};

use sulix_contract as contract;

use crate::repository::{DecisionRepository, EventStore, SignalRepository, ThesisRepository, UnitOfWork};

// ===== Schema =====

/// 初始化存储层所需的全部表
///
/// 使用 CREATE TABLE IF NOT EXISTS，可重复调用不会报错。
fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA busy_timeout = 5000;")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS theses (
            id              TEXT PRIMARY KEY,
            claim           TEXT NOT NULL,
            confidence      REAL NOT NULL DEFAULT 0.0,
            status          TEXT NOT NULL DEFAULT 'Proposed',
            evidence        TEXT NOT NULL DEFAULT '[]',
            falsification_conditions TEXT NOT NULL DEFAULT '[]',
            time_horizon    TEXT NOT NULL DEFAULT '12_months',
            theme           TEXT,
            belief_statement TEXT,
            summary         TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_theses_status ON theses(status);
        CREATE INDEX IF NOT EXISTS idx_theses_theme  ON theses(theme);

        CREATE TABLE IF NOT EXISTS decisions (
            id              TEXT PRIMARY KEY,
            thesis_id       TEXT NOT NULL,
            action          TEXT NOT NULL,
            confidence      REAL NOT NULL,
            horizon         TEXT NOT NULL,
            reasoning       TEXT NOT NULL DEFAULT '',
            made_at         TEXT NOT NULL,
            rule_passed     INTEGER NOT NULL DEFAULT 1,
            requires_review INTEGER NOT NULL DEFAULT 0,
            review_reason   TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (thesis_id) REFERENCES theses(id)
        );

        CREATE INDEX IF NOT EXISTS idx_decisions_thesis_id ON decisions(thesis_id);
        CREATE INDEX IF NOT EXISTS idx_decisions_made_at   ON decisions(made_at);

        CREATE TABLE IF NOT EXISTS signals (
            id              TEXT PRIMARY KEY,
            observation_id  TEXT NOT NULL,
            importance      REAL NOT NULL DEFAULT 0.0,
            domain          TEXT NOT NULL DEFAULT '',
            category        TEXT NOT NULL DEFAULT 'context_update',
            why             TEXT NOT NULL DEFAULT '',
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_signals_created_at ON signals(created_at);

        CREATE TABLE IF NOT EXISTS intelligence_events (
            id              TEXT PRIMARY KEY,
            aggregate_type  TEXT NOT NULL,
            aggregate_id    TEXT NOT NULL,
            event_type      TEXT NOT NULL,
            payload         TEXT NOT NULL,
            source          TEXT NOT NULL DEFAULT '',
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_events_aggregate
            ON intelligence_events(aggregate_type, aggregate_id);
        CREATE INDEX IF NOT EXISTS idx_events_type
            ON intelligence_events(event_type);
        CREATE INDEX IF NOT EXISTS idx_events_created
            ON intelligence_events(created_at);",
    )?;

    Ok(())
}

// ===== SqliteStore — 统一入口 =====

/// SQLite 存储 — 打开一个连接并提供所有仓储
pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    /// 打开或创建 SQLite 数据库，初始化所有表
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref())?;
        init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// 从已有连接创建（用于测试，传入 :memory:）
    pub fn from_conn(conn: Connection) -> Result<Self> {
        init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// 获取 Thesis 仓储
    pub fn theses(&self) -> SqliteThesisRepository<'_> {
        SqliteThesisRepository { conn: &self.conn }
    }

    /// 获取 Decision 仓储
    pub fn decisions(&self) -> SqliteDecisionRepository<'_> {
        SqliteDecisionRepository { conn: &self.conn }
    }

    /// 获取 Signal 仓储
    pub fn signals(&self) -> SqliteSignalRepository<'_> {
        SqliteSignalRepository { conn: &self.conn }
    }

    /// 获取 Event Store
    pub fn event_store(&self) -> SqliteEventStore<'_> {
        SqliteEventStore { conn: &self.conn }
    }

    /// 创建工作单元（事务）
    ///
    /// 使用方式:
    ///   let mut uow = store.transaction()?;
    ///   uow.event_store().append(&event)?;
    ///   uow.thesis_repo().save(&thesis)?;
    ///   uow.commit()?;
    pub fn transaction(&self) -> Result<SqliteUnitOfWork<'_>> {
        SqliteUnitOfWork::begin(&self.conn)
    }
}

// ===== SqliteEventStore =====

/// SQLite 实现的 Event Store（append-only）
pub struct SqliteEventStore<'a> {
    conn: &'a Connection,
}

impl EventStore for SqliteEventStore<'_> {
    fn append(&self, event: &contract::IntelligenceEvent) -> Result<()> {
        let payload_json = serde_json::to_string(&event.payload)?;
        self.conn.execute(
            "INSERT INTO intelligence_events (id, aggregate_type, aggregate_id, event_type, payload, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO NOTHING",
            params![
                event.id,
                event.aggregate_type,
                event.aggregate_id,
                event.event_type,
                payload_json,
                event.source,
                event.created_at,
            ],
        )?;
        Ok(())
    }

    fn append_many(&self, events: &[contract::IntelligenceEvent]) -> Result<()> {
        for event in events {
            self.append(event)?;
        }
        Ok(())
    }

    fn event_stream(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<contract::IntelligenceEvent>> {
        self.conn
            .prepare(
                "SELECT id, aggregate_type, aggregate_id, event_type, payload, source, created_at
                 FROM intelligence_events
                 WHERE aggregate_type = ?1 AND aggregate_id = ?2
                 ORDER BY created_at ASC",
            )?
            .query_map(params![aggregate_type, aggregate_id], row_to_event)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn events_by_type(&self, event_type: &str, limit: usize) -> Result<Vec<contract::IntelligenceEvent>> {
        self.conn
            .prepare(
                "SELECT id, aggregate_type, aggregate_id, event_type, payload, source, created_at
                 FROM intelligence_events
                 WHERE event_type = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?
            .query_map(params![event_type, limit as i64], row_to_event)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn find_all_events(&self, limit: usize) -> Result<Vec<contract::IntelligenceEvent>> {
        self.conn
            .prepare(
                "SELECT id, aggregate_type, aggregate_id, event_type, payload, source, created_at
                 FROM intelligence_events
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )?
            .query_map(params![limit as i64], row_to_event)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn row_to_event(row: &rusqlite::Row) -> rusqlite::Result<contract::IntelligenceEvent> {
    let payload_str: String = row.get(4)?;
    let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();
    Ok(contract::IntelligenceEvent {
        id: row.get(0)?,
        aggregate_type: row.get(1)?,
        aggregate_id: row.get(2)?,
        event_type: row.get(3)?,
        payload,
        source: row.get(5)?,
        created_at: row.get(6)?,
    })
}

// ===== SqliteUnitOfWork =====

/// SQLite 工作单元 — 基于 Connection 的事务包装
///
/// 使用方式:
///   let mut uow = SqliteUnitOfWork::begin(&conn)?;
///   uow.event_store().append(&event)?;
///   uow.thesis_repo().save(&thesis)?;
///   uow.commit()?;
///
/// 实现: 使用 Connection::execute_batch("BEGIN TRANSACTION") 管理事务边界，
/// 避免 rusqlite Transaction 类型的自引用生命周期问题。
/// Drop 时自动回滚未提交的事务（RAII 模式）。
pub struct SqliteUnitOfWork<'a> {
    conn: &'a Connection,
    events: SqliteEventStore<'a>,
    theses: SqliteThesisRepository<'a>,
    decisions: SqliteDecisionRepository<'a>,
    in_transaction: bool,
}

impl<'a> SqliteUnitOfWork<'a> {
    pub fn begin(conn: &'a Connection) -> Result<Self> {
        conn.execute_batch("BEGIN TRANSACTION")?;
        let events = SqliteEventStore { conn };
        let theses = SqliteThesisRepository { conn };
        let decisions = SqliteDecisionRepository { conn };
        Ok(Self {
            conn,
            events,
            theses,
            decisions,
            in_transaction: true,
        })
    }

    pub fn event_store(&mut self) -> &mut SqliteEventStore<'a> {
        &mut self.events
    }

    pub fn thesis_repo(&mut self) -> &mut SqliteThesisRepository<'a> {
        &mut self.theses
    }

    pub fn decision_repo(&mut self) -> &mut SqliteDecisionRepository<'a> {
        &mut self.decisions
    }

    pub fn commit(&mut self) -> Result<()> {
        if self.in_transaction {
            self.conn.execute_batch("COMMIT")?;
            self.in_transaction = false;
        }
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        if self.in_transaction {
            self.conn.execute_batch("ROLLBACK")?;
            self.in_transaction = false;
        }
        Ok(())
    }
}

impl UnitOfWork for SqliteUnitOfWork<'_> {
    fn events(&mut self) -> &dyn EventStore {
        &self.events as &dyn EventStore
    }

    fn theses(&mut self) -> &dyn ThesisRepository {
        &self.theses as &dyn ThesisRepository
    }

    fn decisions(&mut self) -> &dyn DecisionRepository {
        &self.decisions as &dyn DecisionRepository
    }

    fn commit(&mut self) -> Result<()> {
        SqliteUnitOfWork::commit(self)
    }

    fn rollback(&mut self) -> Result<()> {
        SqliteUnitOfWork::rollback(self)
    }
}

impl Drop for SqliteUnitOfWork<'_> {
    fn drop(&mut self) {
        if self.in_transaction {
            let _ = self.conn.execute_batch("ROLLBACK");
        }
    }
}

// ===== Thesis Repository =====

/// SQLite 实现的 Thesis 仓储
pub struct SqliteThesisRepository<'a> {
    conn: &'a Connection,
}

impl ThesisRepository for SqliteThesisRepository<'_> {
    fn save(&self, thesis: &contract::Thesis) -> Result<()> {
        let evidence_json = serde_json::to_string(&thesis.evidence)?;
        let fc_json = serde_json::to_string(&thesis.falsification_conditions)?;
        let status_str = format!("{:?}", thesis.status);
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        self.conn.execute(
            "INSERT INTO theses (id, claim, confidence, status, evidence,
                                 falsification_conditions, time_horizon, theme,
                                 belief_statement, summary, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
             ON CONFLICT(id) DO UPDATE SET
                claim = excluded.claim,
                confidence = excluded.confidence,
                status = excluded.status,
                evidence = excluded.evidence,
                falsification_conditions = excluded.falsification_conditions,
                theme = excluded.theme,
                belief_statement = excluded.belief_statement,
                summary = excluded.summary,
                updated_at = excluded.updated_at",
            params![
                thesis.id,
                thesis.claim,
                thesis.confidence,
                status_str,
                evidence_json,
                fc_json,
                thesis.time_horizon,
                thesis.theme,
                thesis.belief_statement,
                thesis.summary,
                now,
            ],
        )?;
        Ok(())
    }

    fn save_many(&self, theses: &[contract::Thesis]) -> Result<()> {
        for thesis in theses {
            self.save(thesis)?;
        }
        Ok(())
    }

    fn find_active(&self) -> Result<Vec<contract::Thesis>> {
        self.conn
            .prepare(
                "SELECT id, claim, confidence, status, evidence,
                        falsification_conditions, time_horizon, theme,
                        belief_statement, summary
                 FROM theses
                 WHERE status NOT IN ('Dormant', 'Retired')
                 ORDER BY updated_at DESC",
            )?
            .query_map([], row_to_thesis)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn find_by_id(&self, id: &str) -> Result<Option<contract::Thesis>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, claim, confidence, status, evidence,
                    falsification_conditions, time_horizon, theme,
                    belief_statement, summary
             FROM theses WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], row_to_thesis)?;
        match rows.next() {
            Some(Ok(thesis)) => Ok(Some(thesis)),
            _ => Ok(None),
        }
    }

    fn find_all(&self) -> Result<Vec<contract::Thesis>> {
        self.conn
            .prepare(
                "SELECT id, claim, confidence, status, evidence,
                        falsification_conditions, time_horizon, theme,
                        belief_statement, summary
                 FROM theses
                 ORDER BY updated_at DESC",
            )?
            .query_map([], row_to_thesis)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn row_to_thesis(row: &rusqlite::Row) -> rusqlite::Result<contract::Thesis> {
    let status_str: String = row.get(3)?;
    let evidence_json: String = row.get(4)?;
    let fc_json: String = row.get(5)?;

    let status = parse_thesis_status(&status_str);
    let evidence: Vec<String> = serde_json::from_str(&evidence_json).unwrap_or_default();
    let falsification_conditions: Vec<String> =
        serde_json::from_str(&fc_json).unwrap_or_default();

    Ok(contract::Thesis {
        id: row.get(0)?,
        claim: row.get(1)?,
        confidence: row.get(2)?,
        evidence,
        status,
        falsification_conditions,
        time_horizon: row.get(6)?,
        theme: row.get(7)?,
        belief_statement: row.get(8)?,
        summary: row.get(9)?,
    })
}

fn parse_thesis_status(s: &str) -> contract::ThesisStatus {
    match s {
        "Proposed" => contract::ThesisStatus::Proposed,
        "Active" => contract::ThesisStatus::Active,
        "Strengthening" => contract::ThesisStatus::Strengthening,
        "Weakening" => contract::ThesisStatus::Weakening,
        "Pending" => contract::ThesisStatus::Pending,
        "Confirmed" => contract::ThesisStatus::Confirmed,
        "Invalidated" => contract::ThesisStatus::Invalidated,
        "Dormant" => contract::ThesisStatus::Dormant,
        "Retired" => contract::ThesisStatus::Retired,
        _ => {
            log::warn!("未知 ThesisStatus: '{}', 默认为 Active", s);
            contract::ThesisStatus::Active
        }
    }
}

// ===== Decision Repository =====

/// SQLite 实现的 Decision 仓储
pub struct SqliteDecisionRepository<'a> {
    conn: &'a Connection,
}

impl DecisionRepository for SqliteDecisionRepository<'_> {
    fn save(&self, decision: &contract::Decision) -> Result<()> {
        let action_str = format!("{:?}", decision.action);
        let horizon_str = format!("{:?}", decision.horizon);
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        self.conn.execute(
            "INSERT INTO decisions (id, thesis_id, action, confidence, horizon,
                                    reasoning, made_at, rule_passed, requires_review,
                                    review_reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                thesis_id = excluded.thesis_id,
                action = excluded.action,
                confidence = excluded.confidence,
                horizon = excluded.horizon,
                reasoning = excluded.reasoning,
                made_at = excluded.made_at,
                rule_passed = excluded.rule_passed,
                requires_review = excluded.requires_review,
                review_reason = excluded.review_reason",
            params![
                decision.id,
                decision.thesis_id,
                action_str,
                decision.confidence,
                horizon_str,
                decision.reasoning,
                decision.made_at,
                decision.rule_passed as i32,
                decision.requires_review as i32,
                decision.review_reason,
                now,
            ],
        )?;
        Ok(())
    }

    fn save_many(&self, decisions: &[contract::Decision]) -> Result<()> {
        for decision in decisions {
            self.save(decision)?;
        }
        Ok(())
    }

    fn find_by_thesis_id(&self, thesis_id: &str) -> Result<Vec<contract::Decision>> {
        self.conn
            .prepare(
                "SELECT id, thesis_id, action, confidence, horizon,
                        reasoning, made_at, rule_passed, requires_review, review_reason
                 FROM decisions WHERE thesis_id = ?1
                 ORDER BY made_at DESC",
            )?
            .query_map(params![thesis_id], row_to_decision)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn find_latest(&self, limit: usize) -> Result<Vec<contract::Decision>> {
        self.conn
            .prepare(
                "SELECT id, thesis_id, action, confidence, horizon,
                        reasoning, made_at, rule_passed, requires_review, review_reason
                 FROM decisions
                 ORDER BY made_at DESC
                 LIMIT ?1",
            )?
            .query_map(params![limit as i64], row_to_decision)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn find_all(&self) -> Result<Vec<contract::Decision>> {
        self.conn
            .prepare(
                "SELECT id, thesis_id, action, confidence, horizon,
                        reasoning, made_at, rule_passed, requires_review, review_reason
                 FROM decisions
                 ORDER BY made_at DESC",
            )?
            .query_map([], row_to_decision)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn row_to_decision(row: &rusqlite::Row) -> rusqlite::Result<contract::Decision> {
    let action_str: String = row.get(2)?;
    let horizon_str: String = row.get(4)?;

    Ok(contract::Decision {
        id: row.get(0)?,
        thesis_id: row.get(1)?,
        action: parse_decision_action(&action_str),
        confidence: row.get(3)?,
        horizon: parse_decision_horizon(&horizon_str),
        reasoning: row.get(5)?,
        made_at: row.get(6)?,
        rule_passed: row.get::<_, i32>(7)? != 0,
        requires_review: row.get::<_, i32>(8)? != 0,
        review_reason: row.get(9)?,
    })
}

fn parse_decision_action(s: &str) -> contract::DecisionType {
    match s {
        "Build" => contract::DecisionType::Build,
        "Invest" => contract::DecisionType::Invest,
        "Monitor" => contract::DecisionType::Monitor,
        "Learn" => contract::DecisionType::Learn,
        "Ignore" => contract::DecisionType::Ignore,
        "Exit" => contract::DecisionType::Exit,
        _ => {
            log::warn!("未知 DecisionType: '{}', 默认为 Monitor", s);
            contract::DecisionType::Monitor
        }
    }
}

fn parse_decision_horizon(s: &str) -> contract::DecisionHorizon {
    match s {
        "Immediate" => contract::DecisionHorizon::Immediate,
        "Days30" => contract::DecisionHorizon::Days30,
        "Days90" => contract::DecisionHorizon::Days90,
        "Days180" => contract::DecisionHorizon::Days180,
        _ => {
            log::warn!("未知 DecisionHorizon: '{}', 默认为 Days30", s);
            contract::DecisionHorizon::Days30
        }
    }
}

// ===== Signal Repository =====

/// SQLite 实现的 Signal 仓储
pub struct SqliteSignalRepository<'a> {
    conn: &'a Connection,
}

impl SignalRepository for SqliteSignalRepository<'_> {
    fn save(&self, signal: &contract::Signal) -> Result<()> {
        let category_str = format!("{:?}", signal.category);
        // SignalCategory Debug format: "StructuralShift", "CompetitiveSignal", etc.
        // The serde uses snake_case but we store the Debug format for internal consistency
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        self.conn.execute(
            "INSERT INTO signals (id, observation_id, importance, domain, category, why, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                importance = excluded.importance,
                domain = excluded.domain,
                category = excluded.category,
                why = excluded.why",
            params![
                signal.id,
                signal.observation_id,
                signal.importance,
                signal.domain,
                category_str,
                signal.why,
                now,
            ],
        )?;
        Ok(())
    }

    fn save_many(&self, signals: &[contract::Signal]) -> Result<()> {
        for signal in signals {
            self.save(signal)?;
        }
        Ok(())
    }

    fn find_by_date(&self, date: &str) -> Result<Vec<contract::Signal>> {
        self.conn
            .prepare(
                "SELECT id, observation_id, importance, domain, category, why
                 FROM signals
                 WHERE created_at LIKE ?1
                 ORDER BY importance DESC",
            )?
            .query_map(params![format!("{}%", date)], row_to_signal)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn find_all(&self) -> Result<Vec<contract::Signal>> {
        self.conn
            .prepare(
                "SELECT id, observation_id, importance, domain, category, why
                 FROM signals
                 ORDER BY created_at DESC",
            )?
            .query_map([], row_to_signal)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn row_to_signal(row: &rusqlite::Row) -> rusqlite::Result<contract::Signal> {
    let category_str: String = row.get(4)?;
    Ok(contract::Signal {
        id: row.get(0)?,
        observation_id: row.get(1)?,
        importance: row.get(2)?,
        domain: row.get(3)?,
        category: parse_signal_category(&category_str),
        why: row.get(5)?,
    })
}

fn parse_signal_category(s: &str) -> contract::SignalCategory {
    match s {
        "StructuralShift" => contract::SignalCategory::StructuralShift,
        "CompetitiveSignal" => contract::SignalCategory::CompetitiveSignal,
        "ContextUpdate" => contract::SignalCategory::ContextUpdate,
        "Noise" => contract::SignalCategory::Noise,
        _ => {
            log::warn!("未知 SignalCategory: '{}', 默认为 ContextUpdate", s);
            contract::SignalCategory::ContextUpdate
        }
    }
}

// ===== 测试 =====

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_store() -> SqliteStore {
        let conn = Connection::open_in_memory().unwrap();
        SqliteStore::from_conn(conn).unwrap()
    }

    fn sample_thesis() -> contract::Thesis {
        contract::Thesis {
            id: "thesis_test_001".into(),
            claim: "AI Agent adoption will accelerate".into(),
            confidence: 0.72,
            evidence: vec!["sig_001".into(), "sig_002".into()],
            status: contract::ThesisStatus::Active,
            falsification_conditions: vec!["Adoption flat for 12mo".into()],
            time_horizon: "12_months".into(),
            theme: Some("AI Enterprise".into()),
            belief_statement: Some("Cost reduction drives adoption".into()),
            summary: Some("Enterprise AI agents are accelerating".into()),
        }
    }

    fn sample_decision(thesis_id: &str) -> contract::Decision {
        contract::Decision {
            id: format!("dec_{}", thesis_id),
            thesis_id: thesis_id.into(),
            action: contract::DecisionType::Invest,
            confidence: 0.7,
            horizon: contract::DecisionHorizon::Days90,
            reasoning: "Strong signals".into(),
            made_at: "2026-07-12".into(),
            rule_passed: true,
            requires_review: false,
            review_reason: None,
        }
    }

    fn sample_signal() -> contract::Signal {
        contract::Signal {
            id: "sig_test_001".into(),
            observation_id: "obs_test_001".into(),
            importance: 0.85,
            domain: "AI Infrastructure".into(),
            category: contract::SignalCategory::StructuralShift,
            why: "Major breakthrough".into(),
        }
    }

    #[test]
    fn test_thesis_save_and_find() {
        let store = memory_store();
        let repo = store.theses();
        let thesis = sample_thesis();

        repo.save(&thesis).unwrap();

        let found = repo.find_by_id("thesis_test_001").unwrap().unwrap();
        assert_eq!(found.claim, thesis.claim);
        assert_eq!(found.confidence, 0.72);
        assert!(matches!(found.status, contract::ThesisStatus::Active));
        assert_eq!(found.evidence.len(), 2);
        assert_eq!(found.falsification_conditions.len(), 1);
        assert_eq!(found.theme, Some("AI Enterprise".into()));
        assert_eq!(found.summary, Some("Enterprise AI agents are accelerating".into()));
    }

    #[test]
    fn test_thesis_save_many_and_find_active() {
        let store = memory_store();
        let repo = store.theses();

        let active = contract::Thesis {
            id: "t1".into(),
            claim: "Active thesis".into(),
            confidence: 0.6,
            evidence: vec![],
            status: contract::ThesisStatus::Active,
            falsification_conditions: vec![],
            time_horizon: "12_months".into(),
            theme: None,
            belief_statement: None,
            summary: None,
        };
        let dormant = contract::Thesis {
            id: "t2".into(),
            claim: "Dormant thesis".into(),
            confidence: 0.3,
            evidence: vec![],
            status: contract::ThesisStatus::Dormant,
            falsification_conditions: vec![],
            time_horizon: "12_months".into(),
            theme: None,
            belief_statement: None,
            summary: None,
        };

        repo.save_many(&[active, dormant]).unwrap();
        let active_list = repo.find_active().unwrap();
        assert_eq!(active_list.len(), 1);
        assert_eq!(active_list[0].id, "t1");
    }

    #[test]
    fn test_decision_save_and_find() {
        let store = memory_store();

        // FK requires thesis row first
        store.theses().save(&sample_thesis()).unwrap();

        let repo = store.decisions();
        let decision = sample_decision("thesis_test_001");

        repo.save(&decision).unwrap();

        let by_thesis = repo.find_by_thesis_id("thesis_test_001").unwrap();
        assert_eq!(by_thesis.len(), 1);
        assert_eq!(by_thesis[0].id, decision.id);
        assert!(matches!(by_thesis[0].action, contract::DecisionType::Invest));

        let latest = repo.find_latest(5).unwrap();
        assert_eq!(latest.len(), 1);
    }

    #[test]
    fn test_decision_save_many() {
        let store = memory_store();

        // FK requires thesis rows first
        store.theses().save(&contract::Thesis {
            id: "thesis_a".into(),
            claim: "Thesis A".into(),
            confidence: 0.5,
            evidence: vec![],
            status: contract::ThesisStatus::Active,
            falsification_conditions: vec![],
            time_horizon: "12_months".into(),
            theme: None,
            belief_statement: None,
            summary: None,
        }).unwrap();
        store.theses().save(&contract::Thesis {
            id: "thesis_b".into(),
            claim: "Thesis B".into(),
            confidence: 0.5,
            evidence: vec![],
            status: contract::ThesisStatus::Active,
            falsification_conditions: vec![],
            time_horizon: "12_months".into(),
            theme: None,
            belief_statement: None,
            summary: None,
        }).unwrap();

        let repo = store.decisions();
        let d1 = sample_decision("thesis_a");
        let d2 = sample_decision("thesis_b");
        repo.save_many(&[d1, d2]).unwrap();

        assert_eq!(repo.find_all().unwrap().len(), 2);
    }

    #[test]
    fn test_signal_save_and_find() {
        let store = memory_store();
        let repo = store.signals();
        let signal = sample_signal();

        repo.save(&signal).unwrap();
        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "sig_test_001");
        assert!(matches!(all[0].category, contract::SignalCategory::StructuralShift));
    }

    #[test]
    fn test_signal_update_preserves_id() {
        let store = memory_store();
        let repo = store.signals();

        let s1 = sample_signal();
        repo.save(&s1).unwrap();

        let s2 = contract::Signal {
            id: "sig_test_001".into(),
            observation_id: "obs_updated".into(),
            importance: 0.95,
            domain: "Updated".into(),
            category: contract::SignalCategory::CompetitiveSignal,
            why: "Updated reason".into(),
        };
        repo.save(&s2).unwrap();

        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].importance, 0.95);
        assert!(matches!(all[0].category, contract::SignalCategory::CompetitiveSignal));
    }

    #[test]
    fn test_thesis_roundtrip() {
        let store = memory_store();
        let repo = store.theses();

        let mut thesis = sample_thesis();
        thesis.status = contract::ThesisStatus::Strengthening;
        thesis.falsification_conditions = vec![
            "Condition A".into(),
            "Condition B with \"quotes\"".into(),
        ];

        repo.save(&thesis).unwrap();
        let found = repo.find_by_id("thesis_test_001").unwrap().unwrap();

        assert!(matches!(found.status, contract::ThesisStatus::Strengthening));
        assert_eq!(found.falsification_conditions.len(), 2);
        assert_eq!(found.falsification_conditions[0], "Condition A");

        // Update
        thesis.confidence = 0.85;
        thesis.status = contract::ThesisStatus::Confirmed;
        repo.save(&thesis).unwrap();

        let updated = repo.find_by_id("thesis_test_001").unwrap().unwrap();
        assert!((updated.confidence - 0.85).abs() < 0.01);
        assert!(matches!(updated.status, contract::ThesisStatus::Confirmed));
    }
}
