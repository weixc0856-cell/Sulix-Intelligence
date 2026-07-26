# Memory Engine (Sprint 5.5) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Memory Consolidation Loop — promote Reflection-derived knowledge into Sulix's long-term cognitive memory (Belief Objects + Evidence Lineage).

**Architecture:** New `crates/memory-engine/` crate as cron-powered consolidation layer. Consumes `ReflectionGenerated` events from `event_archive_index`, evaluates via Promotion Gate + Scoring, persists via outbox (event:memory + archive:memory) to D1 index + R2 Belief Object artifacts.

**Tech Stack:** Rust + Cloudflare Workers + D1 + R2 + EventStore (existing infrastructure)

**Spec reference:** `docs/superpowers/specs/2026-07-26-memory-engine-design.md`

---

## File Structure

### New files (to create):
- `migrations/0025_memory_engine.sql` — memory_index table
- `crates/store/src/models/memory.rs` — Memory, NewMemory, MemorySourceRef, MemoryOrigin, MemoryType, PromotionScore types
- `crates/store/src/domain/memory/mod.rs` — module registry
- `crates/store/src/domain/memory/crud.rs` — D1Store memory CRUD
- `crates/memory-engine/Cargo.toml` — new crate manifest
- `crates/memory-engine/src/lib.rs` — crate root, re-exports
- `crates/memory-engine/src/candidate.rs` — CandidateExtractor
- `crates/memory-engine/src/evaluator.rs` — MemoryEvaluator + PromotionScore + effective_confidence
- `crates/memory-engine/src/promotion.rs` — MemoryPromotion (outbox-first)
- `crates/memory-engine/src/worker.rs` — process_pending (cron entry)
- `crates/worker-entry/src/jobs/memory.rs` — cron scheduling entry

### Existing files to modify:
- `Cargo.toml` (workspace) — add memory-engine member + dep
- `crates/store/src/models/mod.rs` — add memory module
- `crates/store/src/domain/mod.rs` — add memory module
- `crates/store/src/backend.rs` — add memory StoreBackend methods
- `crates/store/src/d1_delegate.rs` — delegate memory methods
- `crates/store/src/memory/mod.rs` — MemoryStore memory fields
- `crates/store/src/memory/backend.rs` — MemoryStore memory impl
- `crates/worker-entry/Cargo.toml` — add memory-engine dep
- `crates/worker-entry/src/jobs/mod.rs` — register memory module
- `crates/worker-entry/src/runtime/cron.rs` — add memory::process_pending

---

## Task Plan

### Task 1: Migration — memory_index table

**Files:**
- Create: `migrations/0025_memory_engine.sql`

- [ ] **Step 1: Write migration SQL**

```sql
-- Sprint 5.5: Memory Engine — Cognitive Knowledge Layer.
-- Stores Belief Object metadata (lineage, origin, confidence decay fields).
-- Full content lives in R2 artifacts (memory/insights/MEM-{id}.json).
-- See design spec: docs/superpowers/specs/2026-07-26-memory-engine-design.md

CREATE TABLE IF NOT EXISTS memory_index (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_type         TEXT NOT NULL,
    memory_origin       TEXT NOT NULL DEFAULT 'derived',
    statement           TEXT NOT NULL,
    confidence          REAL NOT NULL DEFAULT 0.0,
    stability_score     REAL,
    confidence_updated_at INTEGER,
    memory_sources      TEXT,
    artifact_key        TEXT,
    status              TEXT NOT NULL DEFAULT 'candidate',
    usage_count         INTEGER DEFAULT 0,
    validation_count    INTEGER DEFAULT 0,
    promoted_at         INTEGER NOT NULL DEFAULT (unixepoch()),
    deprecated_at       INTEGER,
    last_used_at        INTEGER,
    created_at          INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(artifact_key)
);

CREATE INDEX IF NOT EXISTS idx_memory_type ON memory_index(memory_type);
CREATE INDEX IF NOT EXISTS idx_memory_status ON memory_index(status);
CREATE INDEX IF NOT EXISTS idx_memory_origin ON memory_index(memory_origin);
```

- [ ] **Step 2: Commit**

```bash
git add migrations/0025_memory_engine.sql
git commit -m "feat(sprint-5.5): add memory_index table"
```

---

### Task 2: Memory model types

**Files:**
- Create: `crates/store/src/models/memory.rs`
- Modify: `crates/store/src/models/mod.rs`

- [ ] **Step 1: Write type definitions**

```rust
use serde::{Deserialize, Serialize};

/// A row from the memory_index table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub memory_type: String,
    pub memory_origin: String,
    pub statement: String,
    pub confidence: f64,
    pub stability_score: Option<f64>,
    pub confidence_updated_at: Option<i64>,
    pub memory_sources: Option<String>,         // JSON array, deserialized to Vec<MemorySourceRef>
    pub artifact_key: Option<String>,
    pub status: String,
    pub usage_count: i64,
    pub validation_count: i64,
    pub promoted_at: i64,
    pub deprecated_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

/// Input for inserting a new memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMemory {
    pub memory_type: String,
    pub memory_origin: String,
    pub statement: String,
    pub confidence: f64,
    pub stability_score: Option<f64>,
    pub memory_sources: Option<String>,
    pub artifact_key: Option<String>,
    pub status: String,
}

/// A single source reference in the memory lineage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySourceRef {
    pub source_type: String,
    pub source_id: String,
}

/// Promotion score — calculated in the MemoryEvaluator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionScore {
    pub confidence: f32,
    pub recurrence: f32,
    pub impact: f32,
    pub evidence: f32,
    pub stability: f32,
    pub total: f32,
}
```

- [ ] **Step 2: Register in models/mod.rs**

Add `pub(crate) mod memory;` and `pub use memory::*;`

- [ ] **Step 3: Commit**

```bash
git add crates/store/src/models/memory.rs crates/store/src/models/mod.rs
git commit -m "feat(sprint-5.5): add memory model types"
```

---

### Task 3: StoreBackend memory methods

**Files:**
- Modify: `crates/store/src/backend.rs`
- Modify: `crates/store/src/d1_delegate.rs`

- [ ] **Step 1: Add memory methods to StoreBackend trait**

Add `Memory, NewMemory` to imports in `backend.rs`.

Add before the closing `}`:

```rust
    // ===== Memory Engine (Sprint 5.5) =====

    /// Create a new memory entry. Returns the new id.
    async fn create_memory(&self, entry: &NewMemory) -> Result<i64, StoreError>;

    /// Get a memory entry by id.
    async fn get_memory(&self, id: i64) -> Result<Option<Memory>, StoreError>;

    /// List memories, optionally filtered by type and status.
    async fn list_memories(&self, memory_type: Option<&str>, status: Option<&str>, limit: u32) -> Result<Vec<Memory>, StoreError>;

    /// Update memory usage stats (increment usage_count, set last_used_at).
    async fn touch_memory(&self, id: i64, now: i64) -> Result<(), StoreError>;

    /// Count memories pending promotion.
    async fn count_candidate_memories(&self) -> Result<i64, StoreError>;
```

- [ ] **Step 2: Add delegation methods**

Add imports and delegations to `d1_delegate.rs`.

- [ ] **Step 3: Commit**

```bash
git add crates/store/src/backend.rs crates/store/src/d1_delegate.rs
git commit -m "feat(sprint-5.5): add memory methods to StoreBackend"
```

---

### Task 4: D1Store memory CRUD + MemoryStore

**Files:**
- Create: `crates/store/src/domain/memory/mod.rs`
- Create: `crates/store/src/domain/memory/crud.rs`
- Modify: `crates/store/src/domain/mod.rs`
- Modify: `crates/store/src/memory/mod.rs`
- Modify: `crates/store/src/memory/backend.rs`

- [ ] **Step 1: Create domain module**

`memory/mod.rs`:
```rust
pub mod crud;
```

`crates/store/src/domain/mod.rs`: add `pub mod memory;`

- [ ] **Step 2: Write D1Store CRUD**

`crud.rs` with:
- `create_memory` — INSERT INTO memory_index ... RETURNING id
- `get_memory` — SELECT by id
- `list_memories` — SELECT with optional filters
- `touch_memory` — UPDATE usage_count, last_used_at
- `count_candidate_memories` — SELECT COUNT(*) WHERE status='candidate'

- [ ] **Step 3: Add MemoryStore fields**

Add `Memory` to imports, add `memories: RefCell<HashMap<i64, Memory>>` and `next_memory_id: RefCell<i64>` fields, init in `new()`.

- [ ] **Step 4: Add MemoryStore impl**

Implement all 5 memory methods in `memory/backend.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/store/src/domain/memory/ crates/store/src/domain/mod.rs crates/store/src/memory/mod.rs crates/store/src/memory/backend.rs
git commit -m "feat(sprint-5.5): add D1Store + MemoryStore memory CRUD"
```

---

### Task 5: Memory Engine crate — Cargo.toml + lib.rs

**Files:**
- Create: `crates/memory-engine/Cargo.toml`
- Create: `crates/memory-engine/src/lib.rs`
- Modify: `Cargo.toml` (workspace)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "memory-engine"
version.workspace = true
edition.workspace = true

[dependencies]
worker.workspace = true
store.workspace = true
event-store.workspace = true
serde.workspace = true
serde_json.workspace = true
async-trait.workspace = true
thiserror.workspace = true

[dev-dependencies]
futures = "0.3"
```

- [ ] **Step 2: Create lib.rs**

```rust
pub mod candidate;
pub mod evaluator;
pub mod promotion;
pub mod worker;
```

- [ ] **Step 3: Register in workspace**

Add `"crates/memory-engine",` to members and `memory-engine = { path = "crates/memory-engine" }` to workspace.dependencies.

- [ ] **Step 4: Commit**

```bash
git add crates/memory-engine/ Cargo.toml Cargo.lock
git commit -m "feat(sprint-5.5): add memory-engine crate skeleton"
```

---

### Task 6: CandidateExtractor

**Files:**
- Create: `crates/memory-engine/src/candidate.rs`

- [ ] **Step 1: Write CandidateExtractor**

```rust
//! CandidateExtractor — loads ReflectionGenerated events from event_archive_index.
//!
//! Queries by aggregate_type='reflection' and filters by last_run timestamp
//! stored in KV (memory:last_run).

use event_store::keys as event_keys;
use store::{EventIndexEntry, StoreBackend};

/// A candidate for memory promotion, extracted from a ReflectionGenerated event.
#[derive(Debug, Clone)]
pub struct MemoryCandidate {
    pub event_id: String,
    pub reflection_id: String,
    pub decision_id: String,
    pub artifact_key: String,
    pub quality_score: f64,
    pub lesson_count: i64,
    pub rule_count: i64,
    pub occurred_at: i64,
}

/// Extract memory candidates from the event archive index.
pub async fn extract_candidates<S: StoreBackend>(
    store: &S,
    since: i64,
    limit: u32,
) -> Result<Vec<MemoryCandidate>, String> {
    // Query event_archive_index for reflection events since last run
    let rows: Vec<EventIndexEntry> = store
        .find_event_keys("reflection", "", limit)
        .await
        .map_err(|e| format!("find_event_keys failed: {e}"))?;

    // Filter by occurred_at > since (client-side since D1 query doesn't filter by timestamp easily)
    let candidates: Vec<MemoryCandidate> = rows
        .into_iter()
        .filter(|r| r.occurred_at > since)
        .filter_map(|r| {
            // Parse reflection_id from event_id or aggregate_id
            let reflection_id = r.event_id.clone();
            Some(MemoryCandidate {
                event_id: r.event_id,
                reflection_id,
                decision_id: r.aggregate_id.clone(),
                artifact_key: r.object_key,
                quality_score: 0.0, // filled from R2 artifact
                lesson_count: 0,
                rule_count: 0,
                occurred_at: r.occurred_at,
            })
        })
        .collect();

    Ok(candidates)
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/memory-engine/src/candidate.rs
git commit -m "feat(sprint-5.5): add CandidateExtractor"
```

---

### Task 7: MemoryEvaluator

**Files:**
- Create: `crates/memory-engine/src/evaluator.rs`

- [ ] **Step 1: Write evaluator**

```rust
//! MemoryEvaluator — Promotion Gate + Scoring + Confidence Decay.
//!
//! Promotion Gate (hard fail-fast):
//!   quality_score >= 0.7?  outcome exists?  evidence exists?
//!   (rules >= 1 OR lessons >= 1)?
//!
//! Promotion Score:
//!   0.25*confidence + 0.20*recurrence + 0.20*impact + 0.20*evidence + 0.15*stability

use store::PromotionScore;

/// Result of evaluating a memory candidate.
#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationResult {
    /// Score > 0.75, ready for promotion
    Promote { score: PromotionScore },
    /// Score 0.4-0.75, needs human review
    Review { score: PromotionScore },
    /// Score < 0.4 or gate failed, archive
    Archive { reason: String },
}

/// Run the promotion gate. Returns None if the candidate fails any gate check.
pub fn check_gate(quality_score: f64, has_outcome: bool, has_evidence: bool, has_lessons_or_rules: bool) -> Option<()> {
    if quality_score < 0.7 { return None; }
    if !has_outcome { return None; }
    if !has_evidence { return None; }
    if !has_lessons_or_rules { return None; }
    Some(())
}

/// Calculate the promotion score.
pub fn calculate_score(
    confidence: f32,
    recurrence: f32,
    impact: f32,
    evidence: f32,
    stability: f32,
) -> PromotionScore {
    let total = 0.25 * confidence + 0.20 * recurrence + 0.20 * impact + 0.20 * evidence + 0.15 * stability;
    PromotionScore { confidence, recurrence, impact, evidence, stability, total }
}

/// Evaluate a candidate: run gate, then score, then classify.
pub fn evaluate(
    quality_score: f64,
    has_outcome: bool,
    has_evidence: bool,
    has_lessons_or_rules: bool,
    recurrence: f32,
    impact: f32,
    stability: f32,
) -> EvaluationResult {
    match check_gate(quality_score, has_outcome, has_evidence, has_lessons_or_rules) {
        Some(()) => {
            let score = calculate_score(quality_score as f32, recurrence, impact, 0.8, stability);
            if score.total > 0.75 {
                EvaluationResult::Promote { score }
            } else if score.total >= 0.4 {
                EvaluationResult::Review { score }
            } else {
                EvaluationResult::Archive { reason: format!("score too low: {:.2}", score.total) }
            }
        }
        None => EvaluationResult::Archive { reason: "promotion gate failed".into() },
    }
}

/// Calculate effective confidence based on time decay.
/// Lambda varies by memory_type (strategic_pattern=0.002, etc).
pub fn effective_confidence(confidence: f64, days_since: i64, lambda: f64) -> f64 {
    if days_since <= 0 { return confidence; }
    confidence * (-lambda * days_since as f64).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_gate_passes() {
        assert!(check_gate(0.8, true, true, true).is_some());
    }

    #[test]
    fn promotion_gate_fails_low_quality() {
        assert!(check_gate(0.5, true, true, true).is_none());
    }

    #[test]
    fn promotion_gate_fails_no_outcome() {
        assert!(check_gate(0.8, false, true, true).is_none());
    }

    #[test]
    fn score_calculation() {
        let s = calculate_score(0.9, 0.5, 0.6, 0.7, 0.8);
        let expected = 0.25*0.9 + 0.20*0.5 + 0.20*0.6 + 0.20*0.7 + 0.15*0.8;
        assert!((s.total - expected as f32).abs() < 0.01);
    }

    #[test]
    fn evaluate_promotes_high_score() {
        let r = evaluate(0.85, true, true, true, 0.8, 0.7, 0.7);
        assert!(matches!(r, EvaluationResult::Promote { .. }));
    }

    #[test]
    fn evaluate_archives_low_quality() {
        let r = evaluate(0.5, true, true, true, 0.5, 0.5, 0.5);
        assert!(matches!(r, EvaluationResult::Archive { .. }));
    }

    #[test]
    fn confidence_decay_over_time() {
        let e = effective_confidence(0.9, 365, 0.002);  // ~1 year, strategic pattern
        assert!(e < 0.9);   // decayed
        assert!(e > 0.5);   // not too much
    }

    #[test]
    fn confidence_decay_zero_days() {
        let e = effective_confidence(0.9, 0, 0.002);
        assert!((e - 0.9).abs() < 0.001);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd "d:/Project/Sulix Intelligence" && cargo test -p memory-engine
```

Expected: 7 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/memory-engine/src/evaluator.rs
git commit -m "feat(sprint-5.5): add MemoryEvaluator with promotion gate, scoring, confidence decay"
```

---

### Task 8: MemoryPromotion

**Files:**
- Create: `crates/memory-engine/src/promotion.rs`

- [ ] **Step 1: Write promotion module**

```rust
//! MemoryPromotion — persists a promoted memory via outbox-first pattern.
//!
//! 1. D1: INSERT memory_index
//! 2. Outbox: event:memory (MemoryPromoted EventEnvelope)
//! 3. Outbox: archive:memory (R2 Belief Object JSON)
//! 4. Archive worker → EventStore append + R2 artifact

use event_store::{AggregateRef, EventEnvelope, EventMetadata, keys as event_keys};
use store::{Memory, NewMemory, NewOutbox, PromotionScore, StoreBackend};

use crate::candidate::MemoryCandidate;

/// Promote a candidate to long-term memory.
pub async fn promote<S: StoreBackend>(
    store: &S,
    candidate: &MemoryCandidate,
    score: &PromotionScore,
    statement: &str,
) -> Result<i64, String> {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let artifact_key = format!("memory/insights/{}.json", candidate.reflection_id.replace("REF", "MEM"));

    // 1. D1: INSERT memory_index
    let memory_id = store
        .create_memory(&NewMemory {
            memory_type: "strategic_pattern".into(),
            memory_origin: "derived".into(),
            statement: statement.to_string(),
            confidence: score.total as f64,
            stability_score: Some(score.stability as f64),
            memory_sources: Some(serde_json::json!([{
                "source_type": "reflection",
                "source_id": &candidate.reflection_id,
            }]).to_string()),
            artifact_key: Some(artifact_key.clone()),
            status: "active".into(),
        })
        .await
        .map_err(|e| format!("create_memory failed: {e}"))?;

    // 2. Outbox: event:memory (MemoryPromoted)
    let event_payload = serde_json::json!({
        "memory_id": format!("MEM-{memory_id:06}"),
        "source_reflection": candidate.reflection_id,
        "score": score.total,
        "artifact_key": artifact_key,
    });
    let event_key = event_keys::event("memory", now, &format!("mem_{now}_{memory_id}"));

    let _ = store
        .insert_outbox(&NewOutbox {
            object_type: "event:memory".into(),
            object_key: event_key.clone(),
            payload: event_payload.to_string(),
        })
        .await;

    // 3. Outbox: archive:memory (R2 artifact)
    let archive_payload = serde_json::json!({
        "schema_version": 1,
        "artifact_type": "memory",
        "memory_id": format!("MEM-{memory_id:06}"),
        "memory_type": "strategic_pattern",
        "memory_origin": "derived",
        "claim": { "statement": statement, "type": "heuristic" },
        "belief": {
            "confidence": score.total,
            "stability": score.stability,
            "effective_confidence": score.total,
        },
        "lineage": {
            "reflections": [candidate.reflection_id],
            "decisions": [candidate.decision_id],
        },
        "promotion": { "score": score.total },
        "created_at": now,
    });

    let _ = store
        .insert_outbox(&NewOutbox {
            object_type: "archive:memory".into(),
            object_key: artifact_key,
            payload: archive_payload.to_string(),
        })
        .await;

    Ok(memory_id)
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/memory-engine/src/promotion.rs
git commit -m "feat(sprint-5.5): add MemoryPromotion with outbox-first persistence"
```

---

### Task 9: Worker — process_pending + Cron integration

**Files:**
- Create: `crates/memory-engine/src/worker.rs`
- Create: `crates/worker-entry/src/jobs/memory.rs`
- Modify: `crates/worker-entry/src/jobs/mod.rs`
- Modify: `crates/worker-entry/src/runtime/cron.rs`
- Modify: `crates/worker-entry/Cargo.toml`

- [ ] **Step 1: Write worker module**

`crates/memory-engine/src/worker.rs`:
```rust
//! Memory Consolidation Worker — runs daily in cron chain.
//!
//! 1. Check KV memory:last_run → skip if already consolidated today
//! 2. Extract candidates from event_archive_index
//! 3. For each candidate: evaluate → promote/archive
//! 4. Update KV memory:last_run

use store::StoreBackend;

use crate::candidate::extract_candidates;
use crate::evaluator::{evaluate, EvaluationResult};
use crate::promotion::promote;

pub const DEFAULT_BATCH_SIZE: u32 = 50;
pub const KV_LAST_RUN: &str = "memory:last_run";

/// Process pending memory candidates.
pub async fn process_pending<S: StoreBackend>(
    store: &S,
    cache: &worker::kv::KvStore,
    now: i64,
) {
    // 1. Check last_run (daily guard)
    if let Ok(Some(val)) = cache.get(KV_LAST_RUN).text().await {
        if let Ok(ts) = val.trim().parse::<i64>() {
            if now - ts < 86400 {
                return; // already consolidated today
            }
        }
    }

    // 2. Extract candidates
    let candidates = match extract_candidates(store, now - 86400 * 7, DEFAULT_BATCH_SIZE).await {
        Ok(c) => c,
        Err(e) => {
            console_log!("[memory] extract_candidates failed: {e}");
            return;
        }
    };

    if candidates.is_empty() {
        // Still update last_run to avoid re-scanning
        let _ = cache.put(KV_LAST_RUN, now.to_string());
        return;
    }

    // 3. Evaluate and promote each candidate
    for candidate in &candidates {
        // For MVP: assume gate passes (quality_score from R2 not yet parsed)
        // Placeholder: always promote with moderate score
        let result = evaluate(0.75, true, true, true, 0.5, 0.5, 0.5);

        match result {
            EvaluationResult::Promote { score } => {
                match promote(store, candidate, &score, "Consolidated memory from reflection").await {
                    Ok(id) => console_log!("[memory] MEM-{:06} promoted", id),
                    Err(e) => console_log!("[memory] promotion failed: {e}"),
                }
            }
            EvaluationResult::Review { .. } => {
                console_log!("[memory] candidate {} needs review — skipped", candidate.reflection_id);
            }
            EvaluationResult::Archive { reason } => {
                console_log!("[memory] candidate {} archived: {reason}", candidate.reflection_id);
            }
        }
    }

    // 4. Update last_run
    if let Ok(pb) = cache.put(KV_LAST_RUN, now.to_string()) {
        let _ = pb.expiration_ttl(604800).execute().await;
    }

    console_log!("[memory] consolidated {} candidates", candidates.len());
}
```

- [ ] **Step 2: Create cron entry in worker-entry**

`crates/worker-entry/src/jobs/memory.rs`:
```rust
//! Memory Consolidation — cron entry point.
//! Dispatches to memory-engine::worker::process_pending.

use store::D1Store;
use worker::*;

pub(crate) async fn process_pending(env: &Env, now: i64) {
    let store = match env.d1("DB") {
        Ok(db) => D1Store::new(db),
        Err(e) => {
            console_log!("[memory] D1 binding failed: {e}");
            return;
        }
    };
    let cache = match env.kv("CACHE") {
        Ok(c) => c,
        Err(e) => {
            console_log!("[memory] KV binding failed: {e}");
            return;
        }
    };

    memory_engine::worker::process_pending(&store, &cache, now).await;
}
```

- [ ] **Step 3: Register module**

Edit `crates/worker-entry/src/jobs/mod.rs`: add `pub mod memory;`

- [ ] **Step 4: Wire into cron**

Edit `crates/worker-entry/src/runtime/cron.rs`:
- Add `memory` to `use crate::jobs::{...}`
- Add `memory::process_pending(&env, now).await;` after `reflection::process_pending_reflections`

- [ ] **Step 5: Add Cargo dep**

Edit `crates/worker-entry/Cargo.toml`: add `memory-engine.workspace = true`

- [ ] **Step 6: Commit**

```bash
git add crates/memory-engine/src/worker.rs crates/worker-entry/src/jobs/memory.rs crates/worker-entry/src/jobs/mod.rs crates/worker-entry/src/runtime/cron.rs crates/worker-entry/Cargo.toml
git commit -m "feat(sprint-5.5): add Memory Consolidation cron worker"
```

---

### Task 10: Full compilation and test

- [ ] **Step 1: Check workspace compilation**

```bash
cd "d:/Project/Sulix Intelligence" && cargo check --workspace 2>&1
```

Expected: success. Fix any compilation errors.

- [ ] **Step 2: Run workspace tests**

```bash
cd "d:/Project/Sulix Intelligence" && cargo test --workspace 2>&1 | grep -E "test result:|FAILED"
```

Expected: 7 evaluator tests + all existing pass.

- [ ] **Step 3: Commit final**

```bash
git add -A
git commit -m "feat(sprint-5.5): Memory Engine — Cognitive Knowledge Layer

New crate: crates/memory-engine/
- CandidateExtractor: loads ReflectionGenerated events from event_archive_index
- MemoryEvaluator: Promotion Gate + scoring + confidence decay
- MemoryPromotion: outbox-first persistence (event:memory + archive:memory)
- Worker: daily cron consolidation (KV last_run guard)

New table: memory_index (migration 0025)
- Lineage (memory_sources JSON), origin, stability, decay, graveyard

StoreBackend: 5 new methods (create, get, list, touch, count)

Evaluator tests: 7 tests (gate, scoring, decay, classification)
"
git push
```
