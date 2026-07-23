# Signal Strategies — Sulix Intelligence Information Prioritization Engine

> **Status:** Approved  
> **Date:** 2026-07-23  
> **Author:** Brainstorming → Design Review  
> **Prerequisites:** rules crate (existing), filter_rules table (existing), rules CRUD API (existing)

---

## Context

Sulix Intelligence is an RSS Feed + AI Digest product. The pipeline ingests articles → AI tags them → applies scoring rules → produces a curated digest. The existing "rules" system (D1 `filter_rules` table + Rust `rules` crate) was built as a pure scoring engine but had no management UI. The product naming ("Rules", "Score") positioned it as an RSS filter, not as the signal intelligence layer it should be.

This spec renames and repositions the feature as **Signal Strategies** — the control layer between raw Observation and curated Digest. The UX communicates "I define what information matters" rather than "I filter what I don't want."

---

## Design Principles

1. **Product semantics ≠ engineering paths** — The frontend uses `Signal Strategies`; the API stays at `/api/rules`; the database table stays `filter_rules`. Layers evolve independently.
2. **Preview is product value, not UI decoration** — Every strategy edit shows real impact on recent articles before saving.
3. **Condition is logic, everything else is metadata** — `rule_json` stores only the condition tree. Name, score_delta, signal_type belong on the entity.
4. **Delete is soft** — Strategies are disabled, never hard-deleted. Past strategies create Decision Memory.

---

## Data Model

### Migration

```sql
-- 0002_signal_strategies.sql
ALTER TABLE filter_rules ADD COLUMN signal_type TEXT;
ALTER TABLE filter_rules ADD COLUMN updated_at INTEGER DEFAULT 0;

-- Backfill updated_at for existing rows
UPDATE filter_rules SET updated_at = created_at WHERE updated_at = 0;
```

### Final Schema

```
filter_rules
━━━━━━━━━━━━━━━━━━━━━━━━━
id            INTEGER PRIMARY KEY
name          TEXT NOT NULL
signal_type   TEXT                ← metadata for Dashboard aggregation
rule_json     TEXT NOT NULL       ← CONTAINS ONLY Condition tree
audience_tag  TEXT NOT NULL DEFAULT 'default'
score_delta   REAL NOT NULL
enabled       INTEGER NOT NULL DEFAULT 1
created_at    INTEGER NOT NULL DEFAULT (unixepoch())
updated_at    INTEGER NOT NULL DEFAULT 0
```

### Rust Domain Type

```rust
// crates/store/src/models.rs
pub struct SignalStrategy {
    pub id: i64,
    pub name: String,
    pub signal_type: Option<String>,
    pub audience_tag: String,
    pub condition: Condition,       // deserialized from rule_json
    pub score_delta: f64,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}
```

### rule_json — condition only

```json
{
  "type": "keyword_includes",
  "field": "title",
  "keyword": "AI"
}
```

NOT:

```json
{
  "name": "AI boost",
  "audience": "default",
  "score_delta": 5,
  "condition": { ... }
}
```

---

## API Design

### Existing CRUD — unchanged, body updated for signal_type

| Method | Path | Description |
|--------|------|-------------|
| GET    | /api/rules       | List all strategies |
| POST   | /api/rules       | Create strategy (body includes signal_type) |
| GET    | /api/rules/:id   | Get one strategy |
| PUT    | /api/rules/:id   | Update strategy (supports signal_type) |
| DELETE | /api/rules/:id   | Soft-delete (set enabled=0) |

### New: Preview

```
POST /api/strategies/preview
```

**Purpose:** Evaluate what a strategy would match against recent articles before saving.

**Request:**
```json
{
  "condition": {
    "type": "keyword_includes",
    "field": "title",
    "keyword": "AI"
  },
  "score_delta": 5.0,
  "signal_type": "Technology"
}
```

**Response:**
```json
{
  "total": 120,
  "matched": 15,
  "signal_type": "Technology",
  "items": [
    {
      "id": 1234,
      "title": "OpenAI releases GPT-6 with 10x inference speed",
      "url": "https://techcrunch.com/...",
      "published_at": 1784779000,
      "feed_name": "TechCrunch",
      "score_change": 5.0,
      "matched_reason": "title contains AI"
    }
  ]
}
```

**Implementation:**
- Backend: Scan recent 100 articles (max 500) from D1 via `store.recent_articles_for_preview(limit)`
- Reuse `rules::score()` + `ArticleInput` — no new matching logic
- Condition parsing failure → 400 error
- `matched_reason` is a human-readable string explaining which condition triggered

**Data window:**
- Default: 100 most recent articles (by `published_at`)
- Maximum: 500 articles (hard-coded, no API param)
- Source: D1 `articles` table, no R2 fetch needed

---

## Frontend Pages

### Route Map (Astro)

| Route | Page | Notes |
|-------|------|-------|
| `/strategies` | List all strategies | Reuse ReaderLayout pattern from feeds/dashboard |
| `/strategies/new` | Create strategy | Templates panel (320px) + Editor + Preview |
| `/strategies/:id` | Edit strategy | Same layout as `/new`, pre-loaded from API |
| (API only) | `/api/rules` | Backend stays at this path; frontend maps |

### List Page (`/strategies`)

- **Layout:** ReaderLayout (sidebar + header + footer)
- **Header:** "Signal Strategies" + subtitle explaining purpose
- **Table columns:** Status indicator · Strategy Name (with keyword preview) · Signal Type (colored pill) · Impact (colored weight) · Context (audience tag) · Actions (Edit · Disable)
- **Toggle:** Click to enable/disable in-line (fires PUT /api/rules/:id with enabled)
- **Empty state:** "No strategies yet. Create your first signal strategy to start prioritizing what matters."
- **Bottom summary:** "N active strategies processing ~M articles/cycle"

### Create/Edit Page (`/strategies/new`, `/strategies/:id`)

**Two-column layout:**
- Left panel: 320px fixed width
- Right panel: Flex (remaining width)

**Left panel — Templates (grouped):**

```
Content Signals
├── 📈 Keyword Boost       — title contains "X" → +N
├── 🏷️ Topic Match         — summary contains "X" → +N

Source Signals
├── 📡 Trusted Source      — from specific feed → +N
├── ⭐ Expert Feed          — from specific URL → +N

Noise Control
├── 🚫 Keyword Exclude     — title excludes "X" → -N

Advanced
├── 🎯 Multi-condition     — ALL/ANY nested conditions → +N
```

Clicking a template pre-fills the editor with:
- Strategy name suggestion
- Appropriate condition structure
- Default weight
- Signal type

**Right panel — Editor:**

1. **Strategy Name** (text input)
2. **Context** (select: Default / Investor / Developer / Personal)
3. **Signal Weight** (number input, positive or negative)
4. **Signal Type** (select: Technology / Industry / Macro / Noise / Multi-factor / —)
5. **Visual Condition Builder** (default view):
   - "Where [Title|Summary] [contains|excludes] [keyword input] + [add keyword]"
   - Multiple keywords are OR'd within the same field
6. **Advanced JSON** (`<details>` collapsed by default):
   - Raw condition JSON in monospace editor (<textarea>)
   - "Validate JSON" button
   - Shows green "✓ Valid" or red error message
7. **Preview Impact** panel (bottom):
   - Fetches `POST /api/strategies/preview` on condition/weight change (debounced 500ms)
   - Shows: "N matched / M total"
   - Matched articles list (title + feed_name + score_change + matched_reason)
   - Non-matched count line
   - "Before: M articles · After strategy: N high-signal"
8. **Actions:** Cancel · Save Strategy

### State: Active vs Disabled

Strategies are never hard-deleted. `DELETE` sets `enabled = 0`. Disabled strategies:
- Do not affect article scoring (existing `active_rule_jsons` query filters `enabled = 1`)
- Show as grey/opaque in the list
- Can be re-enabled from the UI
- Remain in the database for Decision Memory

---

## Implementation Order

| Step | Description | Files | Effort |
|------|-------------|-------|--------|
| 1 | Migration: signal_type + updated_at | `migrations/0002_signal_strategies.sql` | Small |
| 2 | Store: update models + CRUD for signal_type/updated_at | `crates/store/src/models.rs` · `crates/store/src/lib.rs` | Small |
| 3 | Preview API: recent_articles query + preview endpoint | `crates/store/src/lib.rs` · `crates/api/src/strategies.rs` (new) · `crates/api/src/lib.rs` (register route) | Medium |
| 4 | Frontend: /strategies list page | `src/pages/strategies/` directory · `strategies/index.astro` | Medium |
| 5 | Frontend: /strategies/new + /:id editor with templates + preview | `strategies/new.astro` · `strategies/[id].astro` · template definitions | Large |
| 6 | Nav integration | Sidebar · Header · intelligence mobile nav | Small |
| 7 | Verify | cargo check · npm build · wrangler dev test | Small |

---

## Edge Cases & Constraints

- **No auth (MVP):** All strategies are global. `audience_tag` is a future extension point.
- **Preview timeout:** If D1 query + rules eval exceeds Workers CPU limit (30ms soft), the preview endpoint may return a timeout signal. Implement with a hard limit of 500 articles and no external HTTP calls.
- **Empty keyword:** If condition contains empty keyword, reject with validation error at both API and UI level.
- **score_delta = 0:** Technically valid but useless. UI shows a warning "Weight of 0 will not affect article scores."
- **D1 FTS5 export limitation:** The `articles_fts` virtual table prevents `wrangler d1 export`. Documented in existing schema; no change needed.
- **rule_json backward compatibility:** Existing records may contain full Rule JSON (name/audience/condition/score_delta). On read, the Store layer must handle both formats — parse condition from the `condition` key if present, otherwise treat the entire JSON as the condition. This is a transitional concern for existing data.
