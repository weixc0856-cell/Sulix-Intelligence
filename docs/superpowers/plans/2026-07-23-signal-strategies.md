# Signal Strategies Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Signal Strategies management UI — a product layer on top of the existing rules engine — with template-based strategy creation, preview, and full CRUD.

**Architecture:** Backend keeps `/api/rules` path for CRUD, adds `POST /api/strategies/preview` for impact preview. Frontend routes `/strategies`, `/strategies/new`, `/strategies/:id` mapped to those APIs. The `rules` crate stays untouched — all changes are in store, api, and frontend.

**IMPORTANT — rule_json stored format:** The scoring pipeline (`active_rule_jsons` → `rules::score`) expects full Rule JSON: `{name, audience_tag, condition, score_delta}`. So `rule_json` continues to store full Rule JSON. The frontend edits condition-only, but the API handler reconstructs the full Rule before storing. This keeps the scoring pipeline unchanged while giving the frontend a clean condition-only editing experience.

**Tech Stack:** Rust (store + api crates), D1/SQLite, Astro 5 SSR, Tailwind CSS

---

## File Map

### New Files
| File | Purpose |
|------|---------|
| `migrations/0002_signal_strategies.sql` | Add signal_type + updated_at columns |
| `crates/api/src/strategies.rs` | Preview endpoint handler |
| `src/pages/strategies/index.astro` | Strategy list page (Page 1) |
| `src/pages/strategies/new.astro` | Create strategy page (Page 2) |
| `src/pages/strategies/[id].astro` | Edit strategy page (reuses create pattern) |

### Modified Files
| File | Changes |
|------|---------|
| `crates/store/src/models.rs` | RuleEntry → SignalStrategy, add signal_type + updated_at fields |
| `crates/store/src/lib.rs` | Update CRUD for signal_type, add recent_articles_for_preview, change delete to soft-delete |
| `crates/api/src/lib.rs` | Register `/api/strategies/preview` route, add `signal_type` to create/update bodies |
| `src/lib/api.ts` | Add StrategyEntry type, listStrategies, createStrategy, updateStrategy, deleteStrategy, previewStrategy |
| `src/components/Sidebar.astro` | Add "Strategies" nav item |
| `src/components/Header.astro` | Add "Strategies" to mobile bottom nav |
| `src/pages/intelligence.astro` | Add "Strategies" to mobile sidebar nav |

---

## Task 1: Database Migration

**Files:**
- Create: `migrations/0002_signal_strategies.sql`

- [ ] **Step 1: Write the migration SQL**

```sql
-- 0002_signal_strategies.sql
-- Adds metadata columns for the Signal Strategies product feature.
-- signal_type: aggregation dimension for the future Signal Dashboard
-- updated_at: tracks when a strategy was last modified

ALTER TABLE filter_rules ADD COLUMN signal_type TEXT;
ALTER TABLE filter_rules ADD COLUMN updated_at INTEGER DEFAULT 0;

-- Backfill updated_at for existing rows
UPDATE filter_rules SET updated_at = created_at WHERE updated_at = 0;
```

- [ ] **Step 2: Commit**

```bash
git add migrations/0002_signal_strategies.sql
git commit -m "feat: add signal_type + updated_at columns to filter_rules"
```

---

## Task 2: Update Store Models — RuleEntry → SignalStrategy

**Files:**
- Modify: `crates/store/src/models.rs`

- [ ] **Step 1: Replace `RuleEntry` with `SignalStrategy`**

```rust
// Replace the existing RuleEntry struct entirely
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalStrategy {
    pub id: i64,
    pub name: String,
    pub signal_type: Option<String>,
    pub rule_json: String,
    pub audience_tag: String,
    pub score_delta: f64,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}
```

- [ ] **Step 2: Add composite preview response types**

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreviewRequest {
    pub condition: serde_json::Value,
    pub score_delta: f64,
    pub signal_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreviewMatch {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub published_at: Option<i64>,
    pub feed_name: Option<String>,
    pub score_change: f64,
    pub matched_reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreviewResult {
    pub total: i64,
    pub matched: i64,
    pub signal_type: Option<String>,
    pub items: Vec<PreviewMatch>,
}
```

- [ ] **Step 3: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p store
```
Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/store/src/models.rs
git commit -m "feat: add SignalStrategy domain type with signal_type + preview types"
```

---

## Task 3: Update Store CRUD for signal_type + soft-delete + preview

**Files:**
- Modify: `crates/store/src/lib.rs`

- [ ] **Step 1: Update `list_rules` to select new columns and return `SignalStrategy`**

Change the SQL + return type of `list_rules`:

```rust
pub async fn list_rules(&self) -> Result<Vec<SignalStrategy>, StoreError> {
    Ok(self.db.prepare(
        "SELECT id, name, signal_type, rule_json, audience_tag, score_delta, enabled, created_at, updated_at FROM filter_rules ORDER BY created_at DESC",
    ).all().await?.results()?)
}
```

- [ ] **Step 2: Update `get_rule` similarly**

```rust
pub async fn get_rule(&self, id: i64) -> Result<Option<SignalStrategy>, StoreError> {
    Ok(self.db.prepare(
        "SELECT id, name, signal_type, rule_json, audience_tag, score_delta, enabled, created_at, updated_at FROM filter_rules WHERE id = ?1",
    ).bind(&[JsValue::from_f64(id as f64)])?.first::<SignalStrategy>(None).await?)
}
```

- [ ] **Step 3: Update `insert_rule` to accept and store signal_type + updated_at**

```rust
pub async fn insert_rule(&self, name: &str, rule_json: &str, audience_tag: &str, signal_type: Option<&str>, score_delta: f64) -> Result<Option<i64>, StoreError> {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    self.db.prepare(
        "INSERT INTO filter_rules (name, rule_json, audience_tag, signal_type, score_delta, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    ).bind(&[
        name.into(),
        rule_json.into(),
        audience_tag.into(),
        signal_type.map_or(JsValue::null(), |v| v.into()),
        JsValue::from_f64(score_delta),
        JsValue::from_f64(now as f64),
    ])?.run().await?;
    let q = self.db.prepare("SELECT id FROM filter_rules WHERE name = ?1 ORDER BY created_at DESC LIMIT 1").bind(&[name.into()])?;
    let row = q.first::<serde_json::Value>(None).await?;
    Ok(row.and_then(|v| v.get("id").and_then(|id| id.as_i64())))
}
```

- [ ] **Step 4: Update `update_rule` to handle signal_type and set updated_at**

```rust
pub async fn update_rule(&self, id: i64, name: Option<&str>, rule_json: Option<&str>, enabled: Option<bool>, signal_type: Option<Option<&str>>) -> Result<(), StoreError> {
    let mut parts: Vec<String> = Vec::new();
    let mut vals: Vec<JsValue> = Vec::new();
    if let Some(v) = name       { parts.push("name = ?".into()); vals.push(v.into()); }
    if let Some(v) = rule_json  { parts.push("rule_json = ?".into()); vals.push(v.into()); }
    if let Some(v) = enabled    { parts.push("enabled = ?".into()); vals.push(JsValue::from_f64(if v { 1.0 } else { 0.0 })); }
    if let Some(ref v) = signal_type {
        parts.push("signal_type = ?".into());
        vals.push(v.map_or(JsValue::null(), |s| s.into()));
    }
    if parts.is_empty() { return Ok(()); }
    // Always update updated_at when any field changes
    parts.push("updated_at = ?".into());
    vals.push(JsValue::from_f64((js_sys::Date::now() / 1000.0) as f64));
    vals.push(JsValue::from_f64(id as f64));
    self.db.prepare(format!("UPDATE filter_rules SET {} WHERE id = ?", parts.join(", "))).bind(&vals)?.run().await?;
    Ok(())
}
```

- [ ] **Step 5: Change `delete_rule` from hard delete to soft-delete (set enabled=0)**

```rust
pub async fn delete_rule(&self, id: i64) -> Result<(), StoreError> {
    self.db.prepare("UPDATE filter_rules SET enabled = 0, updated_at = ?1 WHERE id = ?2")
        .bind(&[JsValue::from_f64((js_sys::Date::now() / 1000.0) as f64), JsValue::from_f64(id as f64)])?.run().await?;
    Ok(())
}
```

- [ ] **Step 6: Add `recent_articles_for_preview` method**

```rust
/// Fetch recent articles for preview evaluation, up to `limit` rows.
/// Joins with feeds to get feed_name.
pub async fn recent_articles_for_preview(&self, limit: u32) -> Result<Vec<ArticleDetail>, StoreError> {
    Ok(self.db.prepare(
        "SELECT a.id, a.feed_id, f.title AS feed_name, a.guid, a.title, a.url, a.published_at, a.ai_summary, a.ai_tags, a.score
         FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id
         WHERE a.title != ''
         ORDER BY a.published_at DESC LIMIT ?1",
    ).bind(&[JsValue::from_f64(limit as f64)])?.all().await?.results()?)
}
```

- [ ] **Step 7: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p store
```
Expected: Compiles successfully.

- [ ] **Step 8: Commit**

```bash
git add crates/store/src/lib.rs
git commit -m "feat: update store CRUD for signal_type, soft-delete, preview query"
```

---

## Task 4: Add Preview API Endpoint

**Files:**
- Create: `crates/api/src/strategies.rs`
- Modify: `crates/api/src/lib.rs`

- [ ] **Step 1: Create `crates/api/src/strategies.rs`**

```rust
//! Signal Strategies preview endpoint.
//! Evaluates a proposed strategy against recent articles and returns
//! matched results so users can see impact before saving.

use crate::{json_err, json_ok};
use rules::{score, ArticleInput, Condition};
use store::{PreviewMatch, PreviewRequest, PreviewResult, Store};
use worker::*;

/// POST /api/strategies/preview
///
/// Accepts a strategy condition + score_delta, evaluates against recent
/// articles, and returns matched items with human-readable match reasons.
pub async fn preview(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);

    let body: PreviewRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return json_err(400, "invalid JSON body"),
    };

    // Parse condition from the incoming JSON
    let condition: Condition = match serde_json::from_value(body.condition.clone()) {
        Ok(c) => c,
        Err(e) => return json_err(400, &format!("invalid condition: {e}")),
    };

    // Build a temporary rule for scoring
    let rule = rules::Rule {
        name: "preview".into(),
        audience_tag: "default".into(),
        condition: condition.clone(),
        score_delta: body.score_delta,
    };

    // Fetch recent articles (max 500, default 100)
    let articles = match store.recent_articles_for_preview(100).await {
        Ok(a) => a,
        Err(e) => return json_err(500, &e.to_string()),
    };

    let total = articles.len() as i64;

    // Build a human-readable match reason from the condition
    let match_reason = describe_condition(&condition);

    let mut matched_items: Vec<PreviewMatch> = Vec::new();
    let mut matched_count: i64 = 0;

    for article in &articles {
        let input = ArticleInput {
            title: &article.title,
            summary: &article.ai_summary,
            feed_url: "", // preview doesn't need feed_url matching
        };
        let result = score(&input, &[rule.clone()], "default");
        if result != 0.0 {
            matched_count += 1;
            matched_items.push(PreviewMatch {
                id: article.id,
                title: article.title.clone(),
                url: article.url.clone(),
                published_at: article.published_at,
                feed_name: article.feed_name.clone(),
                score_change: result,
                matched_reason: match_reason.clone(),
            });
        }
    }

    json_ok(serde_json::json!(PreviewResult {
        total,
        matched: matched_count,
        signal_type: body.signal_type,
        items: matched_items,
    }))
}

/// Produce a human-readable description of the condition.
fn describe_condition(condition: &Condition) -> String {
    match condition {
        Condition::KeywordIncludes { field, keyword } => {
            format!("{} contains \"{}\"", field_name(*field), keyword)
        }
        Condition::KeywordExcludes { field, keyword } => {
            format!("{} excludes \"{}\"", field_name(*field), keyword)
        }
        Condition::SourceIn { feed_urls } => {
            if feed_urls.len() == 1 {
                format!("source is {}", feed_urls[0])
            } else {
                format!("source is one of {} feeds", feed_urls.len())
            }
        }
        Condition::All { .. } => "all conditions match".into(),
        Condition::Any { .. } => "any condition matches".into(),
    }
}

fn field_name(f: rules::Field) -> &'static str {
    match f {
        rules::Field::Title => "Title",
        rules::Field::Summary => "Summary",
    }
}
```

- [ ] **Step 2: Register the new route in `crates/api/src/lib.rs`**

Add import and route registration:

```rust
// Near the top, add the module declaration
mod strategies;

// In the router() function, add the preview route after the health routes
. post_async("/api/strategies/preview", strategies::preview)
```

Place it right after `get_async("/api/debug/feeds-due", debug_feeds_due)` so it's grouped with utility/strategy routes.

- [ ] **Step 3: Update API handlers for signal_type**

Update `CreateRuleBody` and `rules_create` to accept `signal_type` and `score_delta`, and **construct full Rule JSON** for the scoring pipeline:

```rust
#[derive(Deserialize)]
struct CreateRuleBody {
    name: String,
    rule_json: String,       // from frontend: condition-only JSON, e.g. {"type":"keyword_includes",...}
    audience_tag: Option<String>,
    signal_type: Option<String>,
    score_delta: Option<f64>,
}
```

In `rules_create`, parse the condition, wrap it into full Rule JSON, then store:

```rust
async fn rules_create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let body: CreateRuleBody = match req.json().await { Ok(b) => b, Err(_) => return json_err(400, "invalid JSON body") };
    if body.name.is_empty() { return json_err(400, "name is required"); }
    if body.rule_json.is_empty() { return json_err(400, "rule_json is required"); }

    // Validate that rule_json is valid Condition JSON
    if let Err(e) = serde_json::from_str::<serde_json::Value>(&body.rule_json) {
        return json_err(400, &format!("invalid condition JSON: {e}"));
    }

    // Reconstruct full Rule JSON for the scoring pipeline (active_rule_jsons → rules::score
    // expects {name, audience_tag, condition, score_delta}).
    let full_rule = serde_json::json!({
        "name": body.name,
        "audience_tag": body.audience_tag.clone().unwrap_or_else(|| "default".into()),
        "condition": serde_json::from_str::<serde_json::Value>(&body.rule_json).unwrap(),
        "score_delta": body.score_delta.unwrap_or(0.0),
    });
    let full_rule_str = serde_json::to_string(&full_rule).map_err(|e| json_err(500, &e.to_string()))?;

    match store.insert_rule(
        &body.name,
        &full_rule_str,
        &body.audience_tag.unwrap_or_else(|| "default".into()),
        body.signal_type.as_deref(),
        body.score_delta.unwrap_or(0.0),
    ).await {
        Ok(Some(id)) => match store.get_rule(id).await { Ok(Some(rule)) => json_ok(json!({"rule": rule})), _ => json_ok(json!({"id": id})) },
        Ok(None) => json_err(500, "rule creation returned no id"),
        Err(e) => json_err(500, &e.to_string()),
    }
}
```

Update `UpdateRuleBody` and `rules_update` to pass `signal_type` and reconstruct full Rule JSON on condition change:

```rust
#[derive(Deserialize)]
struct UpdateRuleBody {
    name: Option<String>,
    rule_json: Option<String>,    // from frontend: condition-only JSON
    enabled: Option<bool>,
    signal_type: Option<Option<String>>, // None = not sent, Some(None) = set to null, Some(Some(v)) = set value
}
```

In `rules_update`, reconstruct full Rule JSON when `rule_json` changes:

```rust
async fn rules_update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "invalid id") };
    let body: UpdateRuleBody = match req.json().await { Ok(b) => b, Err(_) => return json_err(400, "invalid JSON body") };

    // If rule_json is being updated, it's condition-only JSON from the frontend —
    // wrap it in full Rule JSON for the scoring pipeline.
    let mut rule_json_for_store: Option<String> = None;
    if let Some(ref cond_json) = body.rule_json {
        // We need the existing name + score_delta to reconstruct. Fetch current rule.
        if let Ok(Some(existing)) = store.get_rule(id).await {
            let full_rule = serde_json::json!({
                "name": body.name.as_deref().unwrap_or(&existing.name),
                "audience_tag": existing.audience_tag,
                "condition": serde_json::from_str::<serde_json::Value>(cond_json).unwrap_or_default(),
                "score_delta": existing.score_delta,
            });
            rule_json_for_store = Some(serde_json::to_string(&full_rule).unwrap_or_else(|_| cond_json.clone()));
        } else {
            return json_err(404, "rule not found for update");
        }
    }

    if let Err(e) = store.update_rule(
        id,
        body.name.as_deref(),
        rule_json_for_store.as_deref().or(body.rule_json.as_deref()),
        body.enabled,
        body.signal_type.as_ref().map(|opt| opt.as_deref()),
    ).await {
        return json_err(500, &e.to_string());
    }
    match store.get_rule(id).await { Ok(Some(rule)) => json_ok(json!({"rule": rule})), Ok(None) => json_err(404, "rule not found"), Err(e) => json_err(500, &e.to_string()) }
}
```

- [ ] **Step 4: Update rules_delete handler to return enabled=false status**

```rust
async fn rules_delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let id = match param_i64(&ctx, "id") { Some(v) => v, None => return json_err(400, "invalid id") };
    match store.delete_rule(id).await {
        Ok(()) => json_ok(json!({"status": "disabled", "id": id})),
        Err(e) => json_err(500, &e.to_string()),
    }
}
```

- [ ] **Step 5: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p api -p store
```
Expected: Compiles successfully.

- [ ] **Step 6: Commit**

```bash
git add crates/api/src/strategies.rs crates/api/src/lib.rs
git commit -m "feat: add preview endpoint, update rules CRUD for signal_type"
```

---

## Task 5: Full Backend Verification

- [ ] **Step 1: Run cargo check across all crates**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p store -p rules -p fetcher -p ai-pipeline -p search -p api -p worker-entry
```
Expected: No errors.

- [ ] **Step 2: Run existing tests**

```bash
cargo test -p rules
```
Expected: 13 tests pass.

- [ ] **Step 3: Commit**

```bash
git commit -m "chore: verify all crate compilation and tests"
```

---

## Task 6: Frontend API Client — Strategy Types + Functions

**Files:**
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add StrategyEntry type and CRUD functions**

```typescript
// After the existing CategoryEntry section
export interface StrategyEntry {
  id: number;
  name: string;
  signal_type: string | null;
  rule_json: string;
  audience_tag: string;
  score_delta: number;
  enabled: boolean;
  created_at: number;
  updated_at: number;
}

export async function fetchStrategies(env: ApiEnv): Promise<StrategyEntry[]> {
  const resp = await apiFetch(env, '/api/rules');
  if (!resp.ok) throw new Error(`strategies fetch failed: ${resp.status}`);
  const data = (await resp.json()) as { rules: StrategyEntry[] };
  return data.rules;
}

export async function createStrategy(
  env: ApiEnv,
  name: string,
  rule_json: string,
  audience_tag?: string,
  signal_type?: string,
  score_delta?: number,
): Promise<StrategyEntry> {
  const resp = await apiFetch(env, '/api/rules', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      name,
      rule_json,
      audience_tag: audience_tag || 'default',
      signal_type: signal_type || undefined,
      score_delta: score_delta ?? 0,
    }),
  });
  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`create strategy failed (${resp.status}): ${text}`);
  }
  const data = (await resp.json()) as { rule: StrategyEntry };
  return data.rule;
}

export async function updateStrategy(
  env: ApiEnv,
  id: number,
  updates: { name?: string; rule_json?: string; enabled?: boolean; signal_type?: string | null },
): Promise<StrategyEntry> {
  const body: Record<string, unknown> = {};
  if (updates.name !== undefined) body.name = updates.name;
  if (updates.rule_json !== undefined) body.rule_json = updates.rule_json;
  if (updates.enabled !== undefined) body.enabled = updates.enabled;
  if (updates.signal_type !== undefined) body.signal_type = updates.signal_type;
  const resp = await apiFetch(env, `/api/rules/${id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw new Error(`update strategy failed: ${resp.status}`);
  const data = (await resp.json()) as { rule: StrategyEntry };
  return data.rule;
}

export async function deleteStrategy(env: ApiEnv, id: number): Promise<void> {
  const resp = await apiFetch(env, `/api/rules/${id}`, { method: 'DELETE' });
  if (!resp.ok) throw new Error(`delete strategy failed: ${resp.status}`);
}

export interface PreviewMatchItem {
  id: number;
  title: string;
  url: string | null;
  published_at: number | null;
  feed_name: string | null;
  score_change: number;
  matched_reason: string;
}

export interface PreviewData {
  total: number;
  matched: number;
  signal_type: string | null;
  items: PreviewMatchItem[];
}

export async function previewStrategy(
  env: ApiEnv,
  condition: Record<string, unknown>,
  score_delta: number,
  signal_type?: string,
): Promise<PreviewData> {
  const resp = await apiFetch(env, '/api/strategies/preview', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ condition, score_delta, signal_type }),
  });
  if (!resp.ok) throw new Error(`preview failed: ${resp.status}`);
  const data = (await resp.json()) as PreviewData;
  return data;
}
```

- [ ] **Step 2: Verify frontend type-check**

```bash
cd "d:/Project/intel-web"
npx tsc --noEmit 2>&1 | head -20
```
Expected: No type errors (possibly existing warnings about other files).

- [ ] **Step 3: Commit**

```bash
git add src/lib/api.ts
git commit -m "feat: add strategy API client functions and types"
```

---

## Task 7: Frontend — Strategies List Page

**Files:**
- Create: `src/pages/strategies/index.astro`

- [ ] **Step 1: Create directory and list page**

```astro
---
import ReaderLayout from '../../layouts/ReaderLayout.astro';
import ErrorState from '../../components/ErrorState.astro';
import { fetchStrategies, updateStrategy, deleteStrategy } from '../../lib/api';
import type { StrategyEntry } from '../../lib/api';

const env = Astro.locals.runtime.env;

// Handle POST actions: toggle enable/disable, delete (soft)
if (Astro.request.method === 'POST') {
  const formData = await Astro.request.formData();
  const action = formData.get('action')?.toString() ?? '';

  if (action === 'toggle') {
    const id = parseInt(formData.get('id')?.toString() ?? '');
    const enabled = formData.get('enabled')?.toString() === '1';
    if (!isNaN(id)) {
      try { await updateStrategy(env, id, { enabled }); } catch { /* ignore */ }
    }
  } else if (action === 'delete') {
    const id = parseInt(formData.get('id')?.toString() ?? '');
    if (!isNaN(id)) {
      try { await deleteStrategy(env, id); } catch { /* ignore */ }
    }
  }
}

let strategies: StrategyEntry[] = [];
let loadError: string | null = null;
try {
  strategies = await fetchStrategies(env);
} catch (e) {
  loadError = e instanceof Error ? e.message : 'failed to load strategies';
}

const activeCount = strategies.filter((s) => s.enabled).length;

function signalTypeClass(type: string | null): string {
  switch (type) {
    case 'Technology': return 'bg-teal-100 dark:bg-teal-900/30 text-teal-800 dark:text-teal-300';
    case 'Industry': return 'bg-orange-100 dark:bg-orange-900/30 text-orange-800 dark:text-orange-300';
    case 'Macro': return 'bg-blue-100 dark:bg-blue-900/30 text-blue-800 dark:text-blue-300';
    case 'Noise': return 'bg-red-100 dark:bg-red-900/30 text-red-800 dark:text-red-300';
    case 'Multi-factor': return 'bg-indigo-100 dark:bg-indigo-900/30 text-indigo-800 dark:text-indigo-300';
    default: return 'bg-surface-container dark:bg-dark-surface text-on-surface-variant';
  }
}

function impactClass(delta: number): string {
  if (delta > 0) return 'text-secondary dark:text-dark-primary font-semibold';
  if (delta < 0) return 'text-tertiary dark:text-red-400 font-semibold';
  return 'text-on-surface-variant';
}
---

<ReaderLayout title="Signal Strategies" activePath="/strategies" maxWidth="max-w-6xl">
  <h1 class="font-headline-lg text-headline-lg text-on-surface dark:text-dark-on-surface mb-2">Signal Strategies</h1>
  <p class="font-body-ui text-body-ui text-on-surface-variant dark:text-dark-on-surface-variant mb-8">
    Define what information matters. Active strategies influence AI article scoring and your daily Signal brief.
  </p>

  {loadError && <ErrorState message={loadError} />}

  {!loadError && (
    <div class="flex justify-between items-center mb-6">
      <p class="text-label-sm font-label-sm text-on-surface-variant dark:text-dark-on-surface-variant">
        {activeCount} active / {strategies.length} total
      </p>
      <a
        href="/strategies/new"
        class="px-4 py-2 rounded-lg bg-primary dark:bg-dark-primary text-on-primary font-body-ui text-body-ui font-medium hover:opacity-90 transition-opacity"
      >+ New Strategy</a>
    </div>
  )}

  {!loadError && strategies.length === 0 && (
    <div class="text-center py-density-comfortable">
      <p class="font-body-ui text-body-ui text-on-surface-variant dark:text-dark-on-surface-variant mb-4">
        No strategies yet. Create your first signal strategy to start prioritizing what matters.
      </p>
      <a
        href="/strategies/new"
        class="inline-block px-5 py-2.5 rounded-lg bg-primary dark:bg-dark-primary text-on-primary font-body-ui text-body-ui font-medium hover:opacity-90 transition-opacity"
      >Create Your First Strategy</a>
    </div>
  )}

  {strategies.length > 0 && (
    <div class="overflow-x-auto rounded-xl bg-surface-container dark:bg-dark-surface">
      <table class="w-full text-body-ui text-body-ui">
        <thead>
          <tr class="border-b border-outline-variant dark:border-dark-border text-label-sm font-label-sm text-on-surface-variant uppercase tracking-wide">
            <th class="text-left py-3 px-4 w-10"></th>
            <th class="text-left py-3 pr-2">Strategy</th>
            <th class="text-left py-2 px-3 hidden sm:table-cell">Signal Type</th>
            <th class="text-left py-2 px-3 hidden md:table-cell">Impact</th>
            <th class="text-left py-2 px-3 hidden sm:table-cell">Context</th>
            <th class="text-right py-3 pl-4">Actions</th>
          </tr>
        </thead>
        <tbody>
          {strategies.map((s) => (
            <tr class:list={[
              'border-b border-outline-variant/50 dark:border-dark-border/50 hover:bg-surface-dim dark:hover:bg-dark-surface/50 transition-colors',
              !s.enabled ? 'opacity-40' : '',
            ]}>
              <!-- Status indicator -->
              <td class="py-3 px-4">
                <span class:list={[
                  'inline-block w-2.5 h-2.5 rounded-full',
                  s.enabled ? 'bg-secondary' : 'bg-outline',
                ]} title={s.enabled ? 'Active' : 'Disabled'}></span>
              </td>
              <!-- Name + keyword preview -->
              <td class="py-3 pr-2 min-w-0">
                <div class="font-medium text-on-surface dark:text-dark-on-surface truncate max-w-[240px]">
                  {s.name}
                </div>
                <div class="text-label-sm font-label-sm text-on-surface-variant dark:text-dark-on-surface-variant truncate max-w-[240px] mt-0.5">
                  {(() => {
                    try {
                      const cond = JSON.parse(s.rule_json);
                      return cond.keyword ? `Keyword: "${cond.keyword}"` : cond.type ?? '';
                    } catch { return ''; }
                  })()}
                </div>
              </td>
              <!-- Signal Type pill -->
              <td class="py-2 px-3 hidden sm:table-cell">
                {s.signal_type && (
                  <span class:list={['px-2 py-0.5 rounded-full text-label-sm font-label-sm', signalTypeClass(s.signal_type)]}>
                    {s.signal_type}
                  </span>
                )}
              </td>
              <!-- Impact -->
              <td class="py-2 px-3 hidden md:table-cell">
                <span class={impactClass(s.score_delta)}>
                  {s.score_delta > 0 ? '+' : ''}{s.score_delta.toFixed(1)}
                </span>
              </td>
              <!-- Context (audience) -->
              <td class="py-2 px-3 hidden sm:table-cell">
                <span class="px-2 py-0.5 rounded-full bg-surface-dim dark:bg-dark-surface text-on-surface-variant text-label-sm font-label-sm">
                  {s.audience_tag}
                </span>
              </td>
              <!-- Actions -->
              <td class="py-3 pl-4 text-right whitespace-nowrap">
                <a
                  href={`/strategies/${s.id}`}
                  class="text-label-sm font-label-sm text-primary dark:text-dark-primary hover:underline mr-3"
                >Edit</a>
                <form method="post" action="/strategies" class="inline">
                  <input type="hidden" name="action" value="toggle" />
                  <input type="hidden" name="id" value={s.id} />
                  <input type="hidden" name="enabled" value={s.enabled ? '0' : '1'} />
                  <button
                    type="submit"
                    class:list={[
                      'text-label-sm font-label-sm hover:underline cursor-pointer border-0 bg-transparent mr-3',
                      s.enabled ? 'text-tertiary' : 'text-secondary',
                    ]}
                  >{s.enabled ? 'Disable' : 'Enable'}</button>
                </form>
                <form method="post" action="/strategies" class="inline" onsubmit="return confirm('Disable this strategy?')">
                  <input type="hidden" name="action" value="delete" />
                  <input type="hidden" name="id" value={s.id} />
                  <button type="submit" class="text-label-sm font-label-sm text-error hover:underline cursor-pointer border-0 bg-transparent">
                    Delete
                  </button>
                </form>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )}
</ReaderLayout>
```

- [ ] **Step 2: Build to verify**

```bash
cd "d:/Project/intel-web"
npm run build
```
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add src/pages/strategies/index.astro
git commit -m "feat: add strategies list page"
```

---

## Task 8: Frontend — Strategy Create/Edit Page

**Files:**
- Create: `src/pages/strategies/new.astro`
- Create: `src/pages/strategies/[id].astro`

- [ ] **Step 1: Create the template definitions as a shared data module**

Create `src/lib/strategy-templates.ts`:

```typescript
export interface StrategyTemplate {
  id: string;
  group: string;
  groupLabel: string;
  label: string;
  description: string;
  nameHint: string;
  condition: Record<string, unknown>;
  defaultScore: number;
  signalType: string;
}

export const STRATEGY_TEMPLATES: StrategyTemplate[] = [
  // Content Signals
  {
    id: 'keyword-boost',
    group: 'content',
    groupLabel: 'Content Signals',
    label: '📈 Keyword Boost',
    description: 'Boost articles when title contains a specific keyword',
    nameHint: 'Keyword Watch',
    condition: { type: 'keyword_includes', field: 'title', keyword: '' },
    defaultScore: 3,
    signalType: 'Technology',
  },
  {
    id: 'topic-match',
    group: 'content',
    groupLabel: 'Content Signals',
    label: '🏷️ Topic Match',
    description: 'Match articles where summary mentions a topic',
    nameHint: 'Topic Focus',
    condition: { type: 'keyword_includes', field: 'summary', keyword: '' },
    defaultScore: 2,
    signalType: 'Industry',
  },
  // Source Signals
  {
    id: 'trusted-source',
    group: 'source',
    groupLabel: 'Source Signals',
    label: '📡 Trusted Source',
    description: 'Boost articles from specific feed URLs',
    nameHint: 'Trusted Source',
    condition: { type: 'source_in', feed_urls: [''] },
    defaultScore: 2,
    signalType: 'Industry',
  },
  // Noise Control
  {
    id: 'keyword-exclude',
    group: 'noise',
    groupLabel: 'Noise Control',
    label: '🚫 Keyword Exclude',
    description: 'Downrank articles when title excludes a keyword',
    nameHint: 'Noise Filter',
    condition: { type: 'keyword_excludes', field: 'title', keyword: '' },
    defaultScore: -5,
    signalType: 'Noise',
  },
  // Advanced
  {
    id: 'multi-condition',
    group: 'advanced',
    groupLabel: 'Advanced',
    label: '🎯 Multi-condition',
    description: 'Combine multiple conditions with ALL/ANY logic',
    nameHint: 'Combined Strategy',
    condition: {
      type: 'all',
      conditions: [
        { type: 'keyword_includes', field: 'title', keyword: '' },
      ],
    },
    defaultScore: 8,
    signalType: 'Multi-factor',
  },
];
```

- [ ] **Step 2: Create the new/edit strategy page**

Create `src/pages/strategies/new.astro`:

```astro
---
import ReaderLayout from '../../layouts/ReaderLayout.astro';
import ErrorState from '../../components/ErrorState.astro';
import { createStrategy, previewStrategy } from '../../lib/api';
import type { PreviewMatchItem } from '../../lib/api';
import { STRATEGY_TEMPLATES } from '../../lib/strategy-templates';
import type { StrategyTemplate } from '../../lib/strategy-templates';

const env = Astro.locals.runtime.env;

// Serialize templates for client-side use
const templatesJson = JSON.stringify(STRATEGY_TEMPLATES);

let successId: number | null = null;
let errorMsg: string | null = null;

if (Astro.request.method === 'POST') {
  const formData = await Astro.request.formData();
  const name = formData.get('name')?.toString() ?? '';
  const audienceTag = formData.get('audience_tag')?.toString() ?? 'default';
  const scoreDelta = parseFloat(formData.get('score_delta')?.toString() ?? '0');
  const signalType = formData.get('signal_type')?.toString() ?? '';
  const ruleJson = formData.get('rule_json')?.toString() ?? '';

  if (!name) {
    errorMsg = 'Strategy name is required.';
  } else if (!ruleJson) {
    errorMsg = 'Condition JSON is required.';
  } else {
    try {
      // Validate JSON
      JSON.parse(ruleJson);
      const result = await createStrategy(env, name, ruleJson, audienceTag, signalType || undefined, scoreDelta);
      successId = result.id;
    } catch (e) {
      errorMsg = e instanceof Error ? e.message : 'Failed to create strategy.';
    }
  }
}
---

<ReaderLayout title={successId ? 'Strategy Created' : 'New Strategy'} activePath="/strategies" maxWidth="max-w-7xl">
  {successId ? (
    <div class="text-center py-density-comfortable">
      <p class="font-headline-md text-headline-md text-on-surface dark:text-dark-on-surface mb-4">Strategy Created ✓</p>
      <div class="flex justify-center gap-4">
        <a href="/strategies" class="px-5 py-2.5 rounded-lg bg-primary dark:bg-dark-primary text-on-primary font-body-ui font-medium hover:opacity-90 transition-opacity">
          Back to Strategies
        </a>
        <a href={`/strategies/${successId}`} class="px-5 py-2.5 rounded-lg border border-outline-variant dark:border-dark-border text-on-surface font-body-ui font-medium hover:bg-surface-container transition-colors">
          Edit Strategy
        </a>
      </div>
    </div>
  ) : (
    <>
      <div class="flex items-center gap-3 mb-6">
        <a href="/strategies" class="text-label-sm font-label-sm text-primary dark:text-dark-primary hover:underline">&larr; Back</a>
        <h1 class="font-headline-lg text-headline-lg text-on-surface dark:text-dark-on-surface">New Strategy</h1>
      </div>

      {errorMsg && <ErrorState message={errorMsg} />}

      <div class="flex gap-0 rounded-xl overflow-hidden border border-outline-variant dark:border-dark-border" style="min-height: 70vh">
        <!-- Left: Templates Panel (320px) -->
        <div class="w-[320px] min-w-[320px] bg-surface-container-low dark:bg-dark-surface/50 p-4 border-r border-outline-variant dark:border-dark-border overflow-y-auto">
          <p class="font-label-sm font-label-sm text-on-surface-variant uppercase tracking-wide mb-1">Templates</p>
          <p class="text-label-sm font-label-sm text-on-surface-variant mb-4">Select a template to pre-fill the strategy.</p>

          {['content', 'source', 'noise', 'advanced'].map((group) => {
            const groupTemplates = STRATEGY_TEMPLATES.filter((t) => t.group === group);
            if (groupTemplates.length === 0) return null;
            const groupColors: Record<string, string> = {
              content: 'text-secondary',
              source: 'text-orange-600',
              noise: 'text-tertiary',
              advanced: 'text-indigo-600',
            };
            return (
              <div class="mb-4">
                <p class:list={['text-label-sm font-label-sm font-semibold uppercase tracking-wide mb-2', groupColors[group] ?? 'text-on-surface-variant']}>
                  {groupTemplates[0].groupLabel}
                </p>
                <div class="space-y-1.5">
                  {groupTemplates.map((t) => (
                    <button
                      type="button"
                      class="w-full text-left px-3 py-2.5 rounded-lg bg-background dark:bg-dark-bg border border-outline-variant/50 dark:border-dark-border hover:border-primary dark:hover:border-dark-primary transition-colors cursor-pointer"
                      data-template-id={t.id}
                      onclick="applyTemplate(this.dataset.templateId)"
                    >
                      <div class="font-body-ui text-body-ui text-on-surface dark:text-dark-on-surface">{t.label}</div>
                      <div class="text-label-sm font-label-sm text-on-surface-variant">{t.description}</div>
                    </button>
                  ))}
                </div>
              </div>
            );
          })}
        </div>

        <!-- Right: Editor -->
        <div class="flex-1 p-6 bg-background dark:bg-dark-bg overflow-y-auto">
          <form method="post" action="/strategies/new" id="strategy-form" class="space-y-5">
            <!-- Hidden field for rule_json -->
            <input type="hidden" name="rule_json" id="input-rule-json" value="" />

            <!-- Row: Name + Context -->
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label class="block text-label-sm font-label-sm text-on-surface-variant mb-1">Strategy Name</label>
                <input type="text" name="name" id="input-name" required
                  class="w-full px-3 py-2 rounded-lg bg-surface-container-low dark:bg-dark-surface border border-outline-variant dark:border-dark-outline text-body-ui text-on-surface dark:text-dark-on-surface focus:border-primary focus:outline-none"
                  placeholder="e.g. AI Infrastructure Watch" />
              </div>
              <div>
                <label class="block text-label-sm font-label-sm text-on-surface-variant mb-1">Context</label>
                <select name="audience_tag"
                  class="w-full px-3 py-2 rounded-lg bg-surface-container-low dark:bg-dark-surface border border-outline-variant dark:border-dark-outline text-body-ui text-on-surface dark:text-dark-on-surface focus:border-primary focus:outline-none">
                  <option value="default">Default</option>
                  <option value="investor">Investor</option>
                  <option value="developer">Developer</option>
                  <option value="personal">Personal</option>
                </select>
              </div>
            </div>

            <!-- Row: Weight + Signal Type -->
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label class="block text-label-sm font-label-sm text-on-surface-variant mb-1">
                  Signal Weight
                  <span class="text-on-surface-variant font-normal"> (positive = boost, negative = downrank)</span>
                </label>
                <input type="number" name="score_delta" id="input-score" step="0.5"
                  class="w-full px-3 py-2 rounded-lg bg-surface-container-low dark:bg-dark-surface border border-outline-variant dark:border-dark-outline text-body-ui text-on-surface dark:text-dark-on-surface focus:border-primary focus:outline-none"
                  value="3" />
              </div>
              <div>
                <label class="block text-label-sm font-label-sm text-on-surface-variant mb-1">Signal Type</label>
                <select name="signal_type" id="input-signal-type"
                  class="w-full px-3 py-2 rounded-lg bg-surface-container-low dark:bg-dark-surface border border-outline-variant dark:border-dark-outline text-body-ui text-on-surface dark:text-dark-on-surface focus:border-primary focus:outline-none">
                  <option value="">—</option>
                  <option value="Technology">Technology</option>
                  <option value="Industry">Industry</option>
                  <option value="Macro">Macro</option>
                  <option value="Noise">Noise</option>
                  <option value="Multi-factor">Multi-factor</option>
                </select>
              </div>
            </div>

            <!-- Visual Condition Builder -->
            <div>
              <label class="block text-label-sm font-label-sm text-on-surface-variant mb-2">Condition</label>
              <div class="bg-surface-container-low dark:bg-dark-surface border border-outline-variant dark:border-dark-border rounded-lg p-4">
                <div class="flex flex-wrap gap-2 items-center">
                  <span class="text-body-ui text-on-surface">Where</span>
                  <select id="cond-field"
                    class="px-2 py-1.5 rounded border border-outline-variant bg-background dark:bg-dark-bg text-body-ui text-on-surface">
                    <option value="title">Title</option>
                    <option value="summary">Summary</option>
                  </select>
                  <select id="cond-operator"
                    class="px-2 py-1.5 rounded border border-outline-variant bg-background dark:bg-dark-bg text-body-ui text-on-surface">
                    <option value="keyword_includes">contains</option>
                    <option value="keyword_excludes">excludes</option>
                  </select>
                  <input type="text" id="cond-keyword" placeholder="keyword"
                    class="px-2 py-1.5 rounded border border-outline-variant bg-background dark:bg-dark-bg text-body-ui text-on-surface focus:border-primary focus:outline-none" />
                </div>
                <p class="text-label-sm font-label-sm text-on-surface-variant mt-2">
                  Multiple keywords are OR'd within the same field.
                </p>
              </div>
            </div>

            <!-- Advanced JSON (collapsed by default) -->
            <details class="border border-outline-variant dark:border-dark-border rounded-lg">
              <summary class="px-4 py-2.5 cursor-pointer text-label-sm font-label-sm text-primary dark:text-dark-primary hover:bg-surface-container-low transition-colors">
                ▸ Advanced: Edit Raw JSON
              </summary>
              <div class="p-4 border-t border-outline-variant dark:border-dark-border">
                <textarea id="input-json-editor" rows="8"
                  class="w-full px-3 py-2 rounded-lg bg-surface-dim dark:bg-dark-surface border border-outline-variant font-mono text-sm text-on-surface dark:text-dark-on-surface focus:border-primary focus:outline-none"
                  placeholder='{"type":"keyword_includes","field":"title","keyword":""}'></textarea>
                <div class="flex items-center gap-3 mt-2">
                  <button type="button" id="btn-validate-json"
                    class="px-3 py-1.5 rounded text-label-sm font-label-sm bg-surface-container dark:bg-dark-surface border border-outline-variant hover:bg-surface-container-high transition-colors">
                    Validate JSON
                  </button>
                  <span id="json-validity" class="text-label-sm font-label-sm"></span>
                </div>
              </div>
            </details>

            <!-- Preview Impact -->
            <div class="border border-outline-variant dark:border-dark-border rounded-lg overflow-hidden">
              <div class="bg-surface-container-low dark:bg-dark-surface/50 px-4 py-2.5 border-b border-outline-variant dark:border-dark-border flex justify-between items-center">
                <span class="text-label-sm font-label-sm text-on-surface font-semibold">🔍 Preview Impact</span>
                <span id="preview-summary" class="text-label-sm font-label-sm text-on-surface-variant">Enter a keyword to preview</span>
              </div>
              <div id="preview-body" class="p-4 max-h-64 overflow-y-auto">
                <p class="text-label-sm font-label-sm text-on-surface-variant text-center py-4">
                  Condition and weight changes will auto-preview.
                </p>
              </div>
            </div>

            <!-- Actions -->
            <div class="flex justify-end gap-3 pt-2">
              <a href="/strategies"
                class="px-5 py-2.5 rounded-lg border border-outline-variant dark:border-dark-border text-on-surface font-body-ui font-medium hover:bg-surface-container transition-colors">
                Cancel
              </a>
              <button type="submit"
                class="px-5 py-2.5 rounded-lg bg-primary dark:bg-dark-primary text-on-primary font-body-ui font-medium hover:opacity-90 transition-opacity">
                Save Strategy
              </button>
            </div>
          </form>
        </div>
      </div>
    </>
  )}
</ReaderLayout>

<script>
(function() {
  const TEMPLATES = {templatesJson};
  const form = document.getElementById('strategy-form');
  const nameInput = document.getElementById('input-name');
  const fieldSelect = document.getElementById('cond-field');
  const operatorSelect = document.getElementById('cond-operator');
  const keywordInput = document.getElementById('cond-keyword');
  const jsonEditor = document.getElementById('input-json-editor');
  const ruleJsonInput = document.getElementById('input-rule-json');
  const scoreInput = document.getElementById('input-score');
  const signalTypeSelect = document.getElementById('input-signal-type');
  const validateBtn = document.getElementById('btn-validate-json');
  const jsonValidity = document.getElementById('json-validity');
  const previewSummary = document.getElementById('preview-summary');
  const previewBody = document.getElementById('preview-body');
  let previewTimer = null;

  // Template application
  window.applyTemplate = function(templateId) {
    const tpl = TEMPLATES.find(function(t) { return t.id === templateId; });
    if (!tpl) return;

    nameInput.value = tpl.nameHint;
    scoreInput.value = String(tpl.defaultScore);
    signalTypeSelect.value = tpl.signalType;

    // Set condition from template
    if (tpl.condition.type === 'keyword_includes' || tpl.condition.type === 'keyword_excludes') {
      fieldSelect.value = tpl.condition.field || 'title';
      operatorSelect.value = tpl.condition.type;
      keywordInput.value = '';
    } else {
      fieldSelect.value = 'title';
      operatorSelect.value = 'keyword_includes';
      keywordInput.value = '';
    }

    // Update JSON editor
    updateJsonFromVisual();
    triggerPreview();
  };

  // Visual builder → JSON
  function updateJsonFromVisual() {
    const condition = {
      type: operatorSelect.value,
      field: fieldSelect.value,
      keyword: keywordInput.value || ''
    };
    const json = JSON.stringify(condition, null, 2);
    jsonEditor.value = json;
    ruleJsonInput.value = json;
    validateJson(json);
  }

  // JSON editor → visual (best-effort)
  function updateVisualFromJson() {
    try {
      const cond = JSON.parse(jsonEditor.value);
      if (cond.type === 'keyword_includes' || cond.type === 'keyword_excludes') {
        fieldSelect.value = cond.field || 'title';
        operatorSelect.value = cond.type;
        keywordInput.value = cond.keyword || '';
      }
    } catch {}
  }

  // JSON validation
  function validateJson(json) {
    try {
      JSON.parse(json);
      jsonValidity.textContent = '✓ Valid';
      jsonValidity.className = 'text-label-sm font-label-sm text-secondary';
      return true;
    } catch (e) {
      jsonValidity.textContent = '✗ Invalid: ' + e.message;
      jsonValidity.className = 'text-label-sm font-label-sm text-tertiary';
      return false;
    }
  }

  // Preview
  function triggerPreview() {
    clearTimeout(previewTimer);
    previewTimer = setTimeout(function() {
      const json = jsonEditor.value;
      const score = parseFloat(scoreInput.value) || 0;
      if (!json) return;

      try {
        JSON.parse(json);
      } catch {
        previewSummary.textContent = 'Invalid JSON — cannot preview';
        return;
      }

      previewSummary.textContent = 'Loading preview…';
      previewBody.innerHTML = '<p class="text-label-sm text-on-surface-variant text-center py-2">Evaluating…</p>';

      fetch('/api/strategies/preview', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          condition: JSON.parse(json),
          score_delta: score,
          signal_type: signalTypeSelect.value || undefined,
        }),
      })
        .then(function(r) { return r.json(); })
        .then(function(data) {
          if (data.error) { throw new Error(data.error); }
          previewSummary.textContent = data.matched + ' matched / ' + data.total + ' total';
          var items = data.items || [];
          if (items.length === 0) {
            previewBody.innerHTML = '<p class="text-label-sm text-on-surface-variant text-center py-2">No articles match this strategy.</p>';
            return;
          }
          var html = '';
          for (var i = 0; i < Math.min(items.length, 5); i++) {
            var item = items[i];
            html += '<div class="flex justify-between items-start py-1.5 border-b border-outline-variant/30">';
            html += '<div class="min-w-0 flex-1">';
            html += '<div class="text-body-ui text-on-surface truncate">' + escapeHtml(item.title) + '</div>';
            html += '<div class="text-label-sm text-on-surface-variant">' + escapeHtml(item.feed_name || '') + ' · ' + escapeHtml(item.matched_reason) + '</div>';
            html += '</div>';
            html += '<span class="text-label-sm font-semibold shrink-0 ml-2 ' + (item.score_change > 0 ? 'text-secondary' : 'text-tertiary') + '">' + (item.score_change > 0 ? '+' : '') + item.score_change.toFixed(1) + '</span>';
            html += '</div>';
          }
          if (items.length > 5) {
            html += '<p class="text-label-sm text-on-surface-variant text-center pt-2">+' + (items.length - 5) + ' more matches</p>';
          }
          previewBody.innerHTML = html;
        })
        ['catch'](function(err) {
          previewSummary.textContent = 'Preview unavailable';
          previewBody.innerHTML = '<p class="text-label-sm text-tertiary text-center py-2">' + escapeHtml(err.message) + '</p>';
        });
    }, 500);
  }

  function escapeHtml(str) {
    if (typeof str !== 'string') return '';
    var div = document.createElement('div');
    div.appendChild(document.createTextNode(str));
    return div.innerHTML;
  }

  // Event listeners
  keywordInput.addEventListener('input', function() { updateJsonFromVisual(); triggerPreview(); });
  fieldSelect.addEventListener('change', function() { updateJsonFromVisual(); triggerPreview(); });
  operatorSelect.addEventListener('change', function() { updateJsonFromVisual(); triggerPreview(); });
  scoreInput.addEventListener('input', triggerPreview);
  signalTypeSelect.addEventListener('change', triggerPreview);
  jsonEditor.addEventListener('input', function() {
    ruleJsonInput.value = jsonEditor.value;
    updateVisualFromJson();
    validateJson(jsonEditor.value);
  });
  validateBtn.addEventListener('click', function() { validateJson(jsonEditor.value); });

  // Form submit: ensure rule_json is set from editor
  form.addEventListener('submit', function() {
    ruleJsonInput.value = jsonEditor.value;
  });

  // Init with empty condition
  updateJsonFromVisual();
})();
</script>
```

- [ ] **Step 3: Create edit page that reuses new page with pre-population**

Create `src/pages/strategies/[id].astro`:

```astro
---
import ReaderLayout from '../../layouts/ReaderLayout.astro';
import ErrorState from '../../components/ErrorState.astro';
import { fetchStrategies, updateStrategy } from '../../lib/api';
import { STRATEGY_TEMPLATES } from '../../lib/strategy-templates';

const env = Astro.locals.runtime.env;
const { id } = Astro.params;

let strategy = null;
let loadError = null;
let successMsg = null;

// Handle POST: update strategy
if (Astro.request.method === 'POST' && id) {
  const formData = await Astro.request.formData();
  const name = formData.get('name')?.toString() ?? '';
  const audienceTag = formData.get('audience_tag')?.toString() ?? 'default';
  const scoreDelta = parseFloat(formData.get('score_delta')?.toString() ?? '0');
  const signalType = formData.get('signal_type')?.toString() ?? '';
  const ruleJson = formData.get('rule_json')?.toString() ?? '';

  try {
    const updates = { name, rule_json: ruleJson, signal_type: signalType || null };
    await updateStrategy(env, parseInt(id), updates);
    successMsg = 'Strategy updated.';
  } catch (e) {
    loadError = e instanceof Error ? e.message : 'Update failed.';
  }
}

// Fetch current strategy data
if (id) {
  try {
    const all = await fetchStrategies(env);
    strategy = all.find(function(s) { return s.id === parseInt(id); }) ?? null;
  } catch (e) {
    loadError = e instanceof Error ? e.message : 'Failed to load strategy.';
  }
}

const templatesJson = JSON.stringify(STRATEGY_TEMPLATES);

// Parse condition for pre-fill
let conditionKeyword = '';
let conditionField = 'title';
let conditionType = 'keyword_includes';
if (strategy) {
  try {
    const cond = JSON.parse(strategy.rule_json);
    conditionKeyword = cond.keyword || '';
    conditionField = cond.field || 'title';
    conditionType = cond.type || 'keyword_includes';
  } catch {}
}
---

<ReaderLayout title={strategy ? `Edit: ${strategy.name}` : 'Edit Strategy'} activePath="/strategies" maxWidth="max-w-7xl">
  {loadError && <ErrorState message={loadError} />}

  {!strategy && !loadError && (
    <div class="text-center py-density-comfortable">
      <p class="text-on-surface-variant font-body-ui mb-4">Strategy not found.</p>
      <a href="/strategies" class="text-primary hover:underline">&larr; Back to strategies</a>
    </div>
  )}

  {successMsg && (
    <div class="mb-6 px-4 py-3 rounded-lg bg-secondary-container dark:bg-dark-surface text-on-secondary-container font-body-ui">
      {successMsg}
      <a href="/strategies" class="ml-3 text-primary hover:underline">Back to strategies &rarr;</a>
    </div>
  )}

  {strategy && (
    <>
      <div class="flex items-center gap-3 mb-6">
        <a href="/strategies" class="text-label-sm font-label-sm text-primary dark:text-dark-primary hover:underline">&larr; Back</a>
        <h1 class="font-headline-lg text-headline-lg text-on-surface dark:text-dark-on-surface">Edit: {strategy.name}</h1>
      </div>

      <div class="flex gap-0 rounded-xl overflow-hidden border border-outline-variant dark:border-dark-border" style="min-height: 70vh">
        <!-- Left Panel -->
        <div class="w-[320px] min-w-[320px] bg-surface-container-low dark:bg-dark-surface/50 p-4 border-r border-outline-variant dark:border-dark-border overflow-y-auto">
          <p class="font-label-sm font-label-sm text-on-surface-variant uppercase tracking-wide mb-1">Templates</p>
          <p class="text-label-sm font-label-sm text-on-surface-variant mb-4">Select a template to replace the current condition.</p>
          {['content', 'source', 'noise', 'advanced'].map((group) => {
            const groupTemplates = STRATEGY_TEMPLATES.filter(function(t) { return t.group === group; });
            if (groupTemplates.length === 0) return null;
            return (
              <div class="mb-4">
                <p class="text-label-sm font-label-sm font-semibold uppercase tracking-wide text-on-surface-variant mb-2">{groupTemplates[0].groupLabel}</p>
                <div class="space-y-1.5">
                  {groupTemplates.map(function(t) {
                    return (
                      <button type="button"
                        class="w-full text-left px-3 py-2.5 rounded-lg bg-background dark:bg-dark-bg border border-outline-variant/50 hover:border-primary transition-colors cursor-pointer"
                        onclick="applyTemplate('{t.id}')">
                        <div class="font-body-ui text-body-ui text-on-surface">{t.label}</div>
                        <div class="text-label-sm font-label-sm text-on-surface-variant">{t.description}</div>
                      </button>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>

        <!-- Right Panel -->
        <div class="flex-1 p-6 bg-background dark:bg-dark-bg overflow-y-auto">
          <form method="post" action={`/strategies/${strategy.id}`} id="strategy-form" class="space-y-5">
            <input type="hidden" name="rule_json" id="input-rule-json" value={strategy.rule_json} />

            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label class="block text-label-sm font-label-sm text-on-surface-variant mb-1">Strategy Name</label>
                <input type="text" name="name" id="input-name" required value={strategy.name}
                  class="w-full px-3 py-2 rounded-lg bg-surface-container-low dark:bg-dark-surface border border-outline-variant text-body-ui text-on-surface focus:border-primary focus:outline-none" />
              </div>
              <div>
                <label class="block text-label-sm font-label-sm text-on-surface-variant mb-1">Context</label>
                <select name="audience_tag"
                  class="w-full px-3 py-2 rounded-lg bg-surface-container-low dark:bg-dark-surface border border-outline-variant text-body-ui text-on-surface focus:border-primary focus:outline-none">
                  <option value="default" selected={strategy.audience_tag === 'default'}>Default</option>
                  <option value="investor" selected={strategy.audience_tag === 'investor'}>Investor</option>
                  <option value="developer" selected={strategy.audience_tag === 'developer'}>Developer</option>
                  <option value="personal" selected={strategy.audience_tag === 'personal'}>Personal</option>
                </select>
              </div>
            </div>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label class="block text-label-sm font-label-sm text-on-surface-variant mb-1">Signal Weight</label>
                <input type="number" name="score_delta" id="input-score" step="0.5" value={strategy.score_delta}
                  class="w-full px-3 py-2 rounded-lg bg-surface-container-low dark:bg-dark-surface border border-outline-variant text-body-ui text-on-surface focus:border-primary focus:outline-none" />
              </div>
              <div>
                <label class="block text-label-sm font-label-sm text-on-surface-variant mb-1">Signal Type</label>
                <select name="signal_type" id="input-signal-type"
                  class="w-full px-3 py-2 rounded-lg bg-surface-container-low dark:bg-dark-surface border border-outline-variant text-body-ui text-on-surface focus:border-primary focus:outline-none">
                  <option value="">—</option>
                  <option value="Technology" selected={strategy.signal_type === 'Technology'}>Technology</option>
                  <option value="Industry" selected={strategy.signal_type === 'Industry'}>Industry</option>
                  <option value="Macro" selected={strategy.signal_type === 'Macro'}>Macro</option>
                  <option value="Noise" selected={strategy.signal_type === 'Noise'}>Noise</option>
                  <option value="Multi-factor" selected={strategy.signal_type === 'Multi-factor'}>Multi-factor</option>
                </select>
              </div>
            </div>

            <!-- Condition Builder -->
            <div>
              <label class="block text-label-sm font-label-sm text-on-surface-variant mb-2">Condition</label>
              <div class="bg-surface-container-low dark:bg-dark-surface border border-outline-variant rounded-lg p-4">
                <div class="flex flex-wrap gap-2 items-center">
                  <span class="text-body-ui text-on-surface">Where</span>
                  <select id="cond-field"
                    class="px-2 py-1.5 rounded border border-outline-variant bg-background dark:bg-dark-bg text-body-ui text-on-surface">
                    <option value="title">Title</option>
                    <option value="summary">Summary</option>
                  </select>
                  <select id="cond-operator"
                    class="px-2 py-1.5 rounded border border-outline-variant bg-background dark:bg-dark-bg text-body-ui text-on-surface">
                    <option value="keyword_includes">contains</option>
                    <option value="keyword_excludes">excludes</option>
                  </select>
                  <input type="text" id="cond-keyword" placeholder="keyword" value={conditionKeyword}
                    class="px-2 py-1.5 rounded border border-outline-variant bg-background dark:bg-dark-bg text-body-ui text-on-surface focus:border-primary focus:outline-none" />
                </div>
              </div>
            </div>

            <!-- Advanced JSON -->
            <details class="border border-outline-variant rounded-lg">
              <summary class="px-4 py-2.5 cursor-pointer text-label-sm font-label-sm text-primary hover:bg-surface-container-low transition-colors">
                ▸ Advanced: Edit Raw JSON
              </summary>
              <div class="p-4 border-t border-outline-variant">
                <textarea id="input-json-editor" rows="8"
                  class="w-full px-3 py-2 rounded-lg bg-surface-dim dark:bg-dark-surface border border-outline-variant font-mono text-sm text-on-surface focus:border-primary focus:outline-none">{strategy.rule_json}</textarea>
                <div class="flex items-center gap-3 mt-2">
                  <button type="button" id="btn-validate-json"
                    class="px-3 py-1.5 rounded text-label-sm font-label-sm bg-surface-container dark:bg-dark-surface border border-outline-variant hover:bg-surface-container-high transition-colors">
                    Validate JSON
                  </button>
                  <span id="json-validity" class="text-label-sm font-label-sm"></span>
                </div>
              </div>
            </details>

            <!-- Preview Impact -->
            <div class="border border-outline-variant rounded-lg overflow-hidden">
              <div class="bg-surface-container-low dark:bg-dark-surface/50 px-4 py-2.5 border-b border-outline-variant flex justify-between items-center">
                <span class="text-label-sm font-label-sm text-on-surface font-semibold">🔍 Preview Impact</span>
                <span id="preview-summary" class="text-label-sm font-label-sm text-on-surface-variant">Enter a keyword to preview</span>
              </div>
              <div id="preview-body" class="p-4 max-h-64 overflow-y-auto">
                <p class="text-label-sm font-label-sm text-on-surface-variant text-center py-4">Evaluating…</p>
              </div>
            </div>

            <div class="flex justify-end gap-3 pt-2">
              <a href="/strategies"
                class="px-5 py-2.5 rounded-lg border border-outline-variant text-on-surface font-body-ui font-medium hover:bg-surface-container transition-colors">
                Cancel
              </a>
              <button type="submit"
                class="px-5 py-2.5 rounded-lg bg-primary dark:bg-dark-primary text-on-primary font-body-ui font-medium hover:opacity-90 transition-opacity">
                Save Changes
              </button>
            </div>
          </form>
        </div>
      </div>
    </>
  )}
</ReaderLayout>

<!-- Re-use the same client script as new page -->
<script>
(function() {
  const TEMPLATES = {templatesJson};
  const form = document.getElementById('strategy-form');
  const nameInput = document.getElementById('input-name');
  const fieldSelect = document.getElementById('cond-field');
  const operatorSelect = document.getElementById('cond-operator');
  const keywordInput = document.getElementById('cond-keyword');
  const jsonEditor = document.getElementById('input-json-editor');
  const ruleJsonInput = document.getElementById('input-rule-json');
  const scoreInput = document.getElementById('input-score');
  const signalTypeSelect = document.getElementById('input-signal-type');
  const validateBtn = document.getElementById('btn-validate-json');
  const jsonValidity = document.getElementById('json-validity');
  const previewSummary = document.getElementById('preview-summary');
  const previewBody = document.getElementById('preview-body');
  let previewTimer = null;

  window.applyTemplate = function(templateId) {
    const tpl = TEMPLATES.find(function(t) { return t.id === templateId; });
    if (!tpl) return;
    nameInput.value = tpl.nameHint;
    scoreInput.value = String(tpl.defaultScore);
    signalTypeSelect.value = tpl.signalType;
    if (tpl.condition.type === 'keyword_includes' || tpl.condition.type === 'keyword_excludes') {
      fieldSelect.value = tpl.condition.field || 'title';
      operatorSelect.value = tpl.condition.type;
      keywordInput.value = '';
    }
    updateJsonFromVisual();
    triggerPreview();
  };

  function updateJsonFromVisual() {
    const condition = {
      type: operatorSelect.value,
      field: fieldSelect.value,
      keyword: keywordInput.value || ''
    };
    const json = JSON.stringify(condition, null, 2);
    jsonEditor.value = json;
    ruleJsonInput.value = json;
    validateJson(json);
  }

  function updateVisualFromJson() {
    try {
      const cond = JSON.parse(jsonEditor.value);
      if (cond.type === 'keyword_includes' || cond.type === 'keyword_excludes') {
        fieldSelect.value = cond.field || 'title';
        operatorSelect.value = cond.type;
        keywordInput.value = cond.keyword || '';
      }
    } catch {}
  }

  function validateJson(json) {
    try {
      JSON.parse(json);
      jsonValidity.textContent = '✓ Valid';
      jsonValidity.className = 'text-label-sm font-label-sm text-secondary';
      return true;
    } catch (e) {
      jsonValidity.textContent = '✗ Invalid: ' + e.message;
      jsonValidity.className = 'text-label-sm font-label-sm text-tertiary';
      return false;
    }
  }

  function triggerPreview() {
    clearTimeout(previewTimer);
    previewTimer = setTimeout(function() {
      const json = jsonEditor.value;
      const score = parseFloat(scoreInput.value) || 0;
      if (!json) return;
      try { JSON.parse(json); } catch { return; }
      previewSummary.textContent = 'Loading preview…';
      fetch('/api/strategies/preview', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ condition: JSON.parse(json), score_delta: score, signal_type: signalTypeSelect.value || undefined }),
      })
        .then(function(r) { return r.json(); })
        .then(function(data) {
          previewSummary.textContent = data.matched + ' matched / ' + data.total + ' total';
          var items = data.items || [];
          if (items.length === 0) {
            previewBody.innerHTML = '<p class="text-label-sm text-on-surface-variant text-center py-2">No articles match.</p>';
            return;
          }
          var html = '';
          for (var i = 0; i < Math.min(items.length, 5); i++) {
            var item = items[i];
            html += '<div class="flex justify-between items-start py-1.5 border-b border-outline-variant/30">';
            html += '<div class="min-w-0 flex-1"><div class="text-body-ui text-on-surface truncate">' + escapeHtml(item.title) + '</div><div class="text-label-sm text-on-surface-variant">' + escapeHtml(item.feed_name || '') + ' · ' + escapeHtml(item.matched_reason) + '</div></div>';
            html += '<span class="text-label-sm font-semibold shrink-0 ml-2 ' + (item.score_change > 0 ? 'text-secondary' : 'text-tertiary') + '">' + (item.score_change > 0 ? '+' : '') + item.score_change.toFixed(1) + '</span>';
            html += '</div>';
          }
          if (items.length > 5) html += '<p class="text-label-sm text-on-surface-variant text-center pt-2">+' + (items.length - 5) + ' more</p>';
          previewBody.innerHTML = html;
        })['catch'](function(err) {
          previewSummary.textContent = 'Preview unavailable';
          previewBody.innerHTML = '<p class="text-label-sm text-tertiary text-center py-2">' + escapeHtml(err.message) + '</p>';
        });
    }, 500);
  }

  function escapeHtml(str) { if (typeof str !== 'string') return ''; var d = document.createElement('div'); d.appendChild(document.createTextNode(str)); return d.innerHTML; }

  keywordInput.addEventListener('input', function() { updateJsonFromVisual(); triggerPreview(); });
  fieldSelect.addEventListener('change', function() { updateJsonFromVisual(); triggerPreview(); });
  operatorSelect.addEventListener('change', function() { updateJsonFromVisual(); triggerPreview(); });
  scoreInput.addEventListener('input', triggerPreview);
  signalTypeSelect.addEventListener('change', triggerPreview);
  jsonEditor.addEventListener('input', function() { ruleJsonInput.value = jsonEditor.value; updateVisualFromJson(); validateJson(jsonEditor.value); });
  validateBtn?.addEventListener('click', function() { validateJson(jsonEditor.value); });
  form?.addEventListener('submit', function() { ruleJsonInput.value = jsonEditor.value; });

  // Init trigger preview on load
  setTimeout(triggerPreview, 300);
})();
</script>
```

- [ ] **Step 4: Build to verify**

```bash
cd "d:/Project/intel-web"
npm run build
```
Expected: Build succeeds.

- [ ] **Step 5: Commit**

```bash
git add src/lib/strategy-templates.ts src/pages/strategies/new.astro src/pages/strategies/[id].astro
git commit -m "feat: add strategy create and edit pages with templates + preview"
```

---

## Task 9: Navigation Integration

**Files:**
- Modify: `src/components/Sidebar.astro`
- Modify: `src/components/Header.astro`
- Modify: `src/pages/intelligence.astro`

- [ ] **Step 1: Add "Strategies" to sidebar nav**

In `src/components/Sidebar.astro`, insert `{ label: 'Strategies', href: '/strategies', icon: 'tune' }` after the Tags entry:

```typescript
const navItems: NavItem[] = [
  { label: 'Latest', href: '/intelligence', icon: 'rss_feed' },
  { label: 'Trending', href: '/trending', icon: 'trending_up' },
  { label: 'Categories', href: '/categories', icon: 'category' },
  { label: 'Tags', href: '/tags', icon: 'sell' },
  { label: 'Search', href: '/search', icon: 'search' },
  { label: 'Bookmarks', href: '/bookmarks', icon: 'bookmark' },
  { label: 'Strategies', href: '/strategies', icon: 'tune' },   // <-- add this
  { label: 'Feeds', href: '/feeds', icon: 'source' },
  { label: 'Dashboard', href: '/dashboard', icon: 'dashboard' },
  { label: 'About', href: '/about', icon: 'info' },
];
```

- [ ] **Step 2: Add to mobile bottom nav in `Header.astro`**

Replace the last item or insert before Dashboard:

```html
<a href="/strategies" ...>
  <span class="material-symbols-outlined text-xl">tune</span>
  <span class="text-[10px] font-label-sm">Strategies</span>
</a>
```

- [ ] **Step 3: Add to intelligence mobile sidebar**

In `src/pages/intelligence.astro`, add `{ label: 'Strategies', href: '/strategies', icon: 'tune' }` after Bookmarks in the mobile nav array.

- [ ] **Step 4: Commit**

```bash
git add src/components/Sidebar.astro src/components/Header.astro src/pages/intelligence.astro
git commit -m "feat: add Strategies to navigation (sidebar, mobile nav)"
```

---

## Task 10: Full Verification

**Files:** N/A

- [ ] **Step 1: Full backend compilation check**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p store -p rules -p fetcher -p ai-pipeline -p search -p api -p worker-entry
cargo test -p rules
```
Expected: All crates compile, 13 tests pass.

- [ ] **Step 2: Full frontend build**

```bash
cd "d:/Project/intel-web"
npm run build
```
Expected: Build succeeds.

- [ ] **Step 3: Summary commit**

```bash
git add -A
git commit -m "chore: signal strategies feature complete"
```
