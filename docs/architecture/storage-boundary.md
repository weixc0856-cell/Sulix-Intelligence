# Storage Boundary Contract v1

**Status:** Frozen at Sprint 5.9

**Purpose:** Define permanent boundaries between D1, R2, EventStore, and VectorStore in the Sulix Intelligence architecture. Every new feature must pass these rules before data hits storage.

---

## 1. Data Classification

| Data Type | Storage | Examples |
|-----------|---------|---------|
| **Current State** | D1 | Decision status, Reflection status, Memory metadata |
| **Event Metadata** | D1 | event_archive_index (type, aggregate_id, object_key, timestamp) |
| **Event Payload** | R2 | Full DecisionCreated/ReflectionGenerated event JSON |
| **Large Artifact** | R2 | Briefing JSON, Reflection draft, Context snapshot, Article content |
| **Semantic Index** | Vectorize | Embedding vectors for article/signal similarity |
| **Immutable History** | EventStore (D1 index + R2) | DecisionCreated, OutcomeObserved, ReflectionGenerated |

---

## 2. Size Boundaries

### D1

| Limit | Value | Rationale |
|-------|-------|-----------|
| Single TEXT field | < 8 KB | Keeps D1 fast for queries; larger = R2 |
| Full row | < 32 KB | D1 is a query layer, not a document store |
| Payload in core tables | < 8 KB | signal_events.payload, outcome_events.observation |

### R2

| Limit | Value | Rationale |
|-------|-------|-----------|
| Min artifact size | > 10 KB | Below this, D1 is acceptable |
| Max artifact size | < 100 MB | R2 free tier limit, single worker response |
| Typical artifact | 10 KB - 5 MB | Briefings, events, context snapshots |

### Decision Tree

```
Is the data > 8 KB per field?
  ├── Yes → Is it a payload or artifact?
  │   ├── Payload → R2 + event_archive_index (pointer in D1)
  │   └── Artifact → R2 + artifacts table (pointer in D1)
  └── No → Is it query-critical current state?
      ├── Yes → D1
      └── No → Question if it needs to exist
```

---

## 3. Table Audit

### Compliant Patterns

| Table | Compliance | Notes |
|-------|-----------|-------|
| `articles` | ✅ | Full text in R2 (`raw_content_r2_key`) |
| `articles` (vector_id) | ✅ | Embedding in Vectorize, only pointer in D1 |
| `event_archive_index` | ✅ | Metadata only, payload in R2 |
| `memory_index` | ✅ | `artifact_key` → R2, `statement` < 300 chars |
| `memory_artifacts` | ✅ | Pointer + metadata, no payload |
| `object_outbox` | ✅ | Transient staging, drained to R2 |

### Fixed in Sprint 5.9

| Table | Violation | Fix |
|-------|-----------|-----|
| `context_snapshots.context_json` | Full AgentContext JSON (10-50 KB) | R2 artifact + `object_key` pointer |
| `intelligence_briefs.content` | Full briefing JSON (legacy dual write) | R2-only via `memory_artifacts` |
| `reflections.result` | May hold full reflection output | Short summary only, full content via `artifact_key` |
| `signal_events.payload` | No size cap or R2 path | 8 KB limit, payload_ref → R2 for large |

---

## 4. Architecture Diagram

```
                    Intelligence Runtime
                           |
            +--------------+--------------+
            |                             |
        EventStore                     Projection
        (D1 index + R2 payload)     (D1 query state)
            |                             |
            +--------------+--------------+
                           |
                          D1
                           |
                    (Current State)
                           |
            +--------------+--------------+
            |                             |
           R2                        Vectorize
     (Artifacts + Events)          (Semantic Index)
```

---

## 5. Rules for New Features

1. **No TEXT column > 8 KB** in a core D1 table. Use R2 + artifact_key.
2. **No embedding vector in D1.** Use Vectorize; store only vector_id.
3. **No event payload in D1.** Put in R2; index in event_archive_index.
4. **No LLM prompt/response in D1.** Log to R2 if needed.
5. **Artifacts always have a registry entry.** If it goes to R2, it gets an `artifacts` row.
6. **Prefer R2 for anything that's written once and read rarely.** Briefings, context snapshots, reflection drafts.
