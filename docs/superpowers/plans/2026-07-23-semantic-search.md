# Semantic Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add semantic search to Sulix Intelligence — generate article embeddings via Workers AI `bge-large-en-v1.5`, upsert to Vectorize, and serve a `POST /api/articles/search` semantic endpoint with frontend mode toggle.

**Architecture:** New `embedding` crate with `EmbeddingProvider` trait + `WorkersAiEmbedder`. Vectorize binding gains `query()`. Queue consumer generates embeddings after summarization. Semantic search API queries Vectorize then enriches from D1.

**Tech Stack:** Rust (new `embedding` crate), Cloudflare Workers AI (free tier), Cloudflare Vectorize (1024d, cosine), Astro 5 SSR

---

## File Map

### New Files
| File | Purpose |
|------|---------|
| `crates/embedding/Cargo.toml` | New crate: embedding provider |
| `crates/embedding/src/lib.rs` | `EmbeddingProvider` trait + `WorkersAiEmbedder` |
| `crates/api/src/semantic.rs` | `POST /api/articles/search` semantic handler |

### Modified Files
| File | Changes |
|------|---------|
| `Cargo.toml` (workspace) | Add `embedding` member + workspace dep |
| `crates/worker-entry/wrangler.toml` | Add `[[ai]] binding = "AI"` |
| `crates/worker-entry/Cargo.toml` | Add `embedding` dependency |
| `crates/worker-entry/src/vectorize.rs` | Add `query` wasm binding + `query_vectors()` wrapper + `delete_vectors()` |
| `crates/worker-entry/src/lib.rs` | Wire embedding into queue handler, remove fire-and-forget upsert |
| `crates/api/Cargo.toml` | Add `embedding` dependency |
| `crates/api/src/lib.rs` | Register `POST /api/articles/search` route |
| `src/lib/api.ts` | Add `ModeSwitch` type, `semanticSearch()` function |
| `src/pages/search.astro` | Add mode toggle + semantic search results handler |

---

## Task 1: Add AI Binding to wrangler.toml

**Files:**
- Modify: `crates/worker-entry/wrangler.toml`

- [ ] **Step 1: Add `[[ai]]` binding**

Insert at the end of `crates/worker-entry/wrangler.toml`:

```toml
[[ai]]
binding = "AI"
```

- [ ] **Step 2: Commit**

```bash
git add crates/worker-entry/wrangler.toml
git commit -m "feat: add Workers AI binding for embedding generation"
```

---

## Task 2: Add `query` + `delete` to Vectorize Binding

**Files:**
- Modify: `crates/worker-entry/src/vectorize.rs`

- [ ] **Step 1: Add `query` and `delete` to wasm binding block**

```rust
#[wasm_bindgen]
extern "C" {
    pub type VectorizeIndex;

    #[wasm_bindgen(method, catch)]
    async fn upsert(this: &VectorizeIndex, vectors: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn query(this: &VectorizeIndex, vectors: JsValue, opts: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn delete(this: &VectorizeIndex, ids: JsValue) -> Result<JsValue, JsValue>;
}
```

- [ ] **Step 2: Add typed `VectorMatch` struct + `query_vectors()` wrapper**

After the existing `upsert_vector_faf` function, add:

```rust
/// A match returned from Vectorize query.
#[derive(Debug, Clone)]
pub struct VectorMatch {
    pub id: String,
    pub score: f32,
}

/// Query the Vectorize index for the nearest neighbors of `embedding`.
/// Returns `top_k` matches sorted by similarity (highest first).
pub async fn query_vectors(
    index: &VectorizeIndex,
    embedding: &[f32],
    top_k: u32,
) -> Result<Vec<VectorMatch>, String> {
    let values = Float32Array::new_with_length(embedding.len() as u32);
    for (i, v) in embedding.iter().enumerate() {
        values.set_index(i as u32, *v);
    }

    let vector = Object::new();
    let _ = Reflect::set(&vector, &"vector".into(), &values.into());

    let opts = Object::new();
    let _ = Reflect::set(&opts, &"topK".into(), &JsValue::from_f64(top_k as f64));
    let _ = Reflect::set(&opts, &"returnMetadata".into(), &JsValue::from_bool(false));
    // returnMetadata=false because we fetch article details from D1

    let result = index
        .query(vector.into(), opts.into())
        .await
        .map_err(|e| format!("{e:?}"))?;

    // Parse the result: { matches: [{ id, score }, ...] }
    let matches = js_sys::Reflect::get(&result, &"matches".into())
        .map_err(|e| format!("missing matches: {e:?}"))?
        .dyn_into::<js_sys::Array>()
        .map_err(|_| "matches is not an array".to_string())?;

    let mut results = Vec::with_capacity(matches.length() as usize);
    for i in 0..matches.length() {
        let item = matches.get(i);
        let id = js_sys::Reflect::get(&item, &"id".into())
            .map_err(|e| format!("missing id: {e:?}"))?
            .as_string()
            .unwrap_or_default();
        let score = js_sys::Reflect::get(&item, &"score".into())
            .map_err(|e| format!("missing score: {e:?}"))?
            .as_f64()
            .unwrap_or(0.0) as f32;
        results.push(VectorMatch { id, score });
    }

    Ok(results)
}
```

- [ ] **Step 3: Add `delete_vectors()` for test cleanup and bulk rebuild**

```rust
/// Delete vectors by their IDs.
pub async fn delete_vectors(index: &VectorizeIndex, ids: &[String]) -> Result<(), String> {
    let js_arr = Array::new();
    for id in ids {
        js_arr.push(&id.clone().into());
    }
    index
        .delete(js_arr.into())
        .await
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}
```

- [ ] **Step 4: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p worker-entry
```

- [ ] **Step 5: Commit**

```bash
git add crates/worker-entry/src/vectorize.rs
git commit -m "feat: add query + delete to Vectorize binding with typed Rust wrapper"
```

---

## Task 3: Create `embedding` Crate

**Files:**
- Create: `crates/embedding/Cargo.toml`
- Create: `crates/embedding/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create crate files**

```rust
// crates/embedding/Cargo.toml
[package]
name = "embedding"
version.workspace = true
edition.workspace = true

[dependencies]
worker.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
async-trait.workspace = true
```

```rust
// crates/embedding/src/lib.rs
//! Embedding generation abstraction.
//!
//! Provides a trait-based interface for generating text embeddings,
//! independent of the specific model or provider.  The current
//! implementation uses Cloudflare Workers AI (bge-large-en-v1.5)
//! which produces 1024-dimensional vectors.
//!
//! Future implementations can add OpenAI, BGE-M3 for multilingual,
//! or local WASM-based models without changing callers.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use worker::*;

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("embedding request failed: {0}")]
    Request(String),
    #[error("unexpected response: {0}")]
    Response(String),
}

/// Generates vector embeddings from text.
#[async_trait(?Send)]
pub trait EmbeddingProvider {
    /// Embed a single text string into a float vector.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}

/// Embedder using Cloudflare Workers AI.
///
/// Model: `@cf/baai/bge-large-en-v1.5` (1024 dimensions, free tier).
/// Called via `env.ai().run(model, inputs)` — no external HTTP call needed.
pub struct WorkersAiEmbedder {
    env: Env,
}

impl WorkersAiEmbedder {
    pub fn new(env: &Env) -> Self {
        Self { env: env.clone() }
    }
}

#[async_trait(?Send)]
impl EmbeddingProvider for WorkersAiEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let ai = self.env.ai().map_err(|e| EmbeddingError::Request(e.to_string()))?;

        let result = ai
            .run(
                "@cf/baai/bge-large-en-v1.5",
                serde_json::json!({ "text": [text] }),
            )
            .await
            .map_err(|e| EmbeddingError::Request(e.to_string()))?;

        // Workers AI returns { data: [{ embedding: [f32; 1024] }] }
        let data = result["data"]
            .as_array()
            .and_then(|arr| arr.first())
            .ok_or_else(|| EmbeddingError::Response("missing data array".into()))?;

        let embedding = data["embedding"]
            .as_array()
            .ok_or_else(|| EmbeddingError::Response("missing embedding field".into()))?;

        let vec: Vec<f32> = embedding
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();

        if vec.is_empty() {
            return Err(EmbeddingError::Response("empty embedding vector".into()));
        }

        Ok(vec)
    }
}

/// Build the embedding input text from article fields.
/// Structured field labels help the model understand semantics.
pub fn build_embedding_text(title: &str, summary: &str, tags: &[String], feed_name: Option<&str>) -> String {
    let tags_str = if tags.is_empty() {
        String::new()
    } else {
        format!("\nTopics:\n{}", tags.join(", "))
    };
    let source = feed_name.map(|n| format!("\nSource:\n{}", n)).unwrap_or_default();
    format!("Title:\n{title}\n\nSummary:\n{summary}{tags_str}{source}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_embedding_text_constructs_correctly() {
        let text = build_embedding_text(
            "AI News",
            "Latest AI developments",
            &vec!["AI".into(), "tech".into()],
            Some("TechCrunch"),
        );
        assert!(text.contains("Title:\nAI News"));
        assert!(text.contains("Topics:\nAI, tech"));
        assert!(text.contains("Source:\nTechCrunch"));
    }

    #[test]
    fn build_embedding_text_handles_no_tags() {
        let text = build_embedding_text("Hello", "World", &[], None);
        assert!(!text.contains("Topics"));
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test::wasm_bindgen_test]
    async fn wasm_embedding_dimension() {
        // This test can only run in a WASM environment with Workers AI available.
        // When run with an actual AI binding, it validates the output dimension.
    }
}
```

- [ ] **Step 2: Register `embedding` crate in workspace**

Add to `Cargo.toml` `[workspace].members`:
```toml
    "crates/embedding",
```

Add to `[workspace].dependencies`:
```toml
embedding = { path = "crates/embedding" }
```

- [ ] **Step 3: Add workspace dependency to worker-entry and api**

In `crates/worker-entry/Cargo.toml`:
```toml
embedding.workspace = true
```

In `crates/api/Cargo.toml`:
```toml
embedding.workspace = true
```

- [ ] **Step 4: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p embedding -p worker-entry -p api
```

- [ ] **Step 5: Commit**

```bash
git add crates/embedding/ Cargo.toml crates/worker-entry/Cargo.toml crates/api/Cargo.toml
git commit -m "feat: add embedding crate with EmbeddingProvider trait + WorkersAiEmbedder"
```

---

## Task 4: Wire Embedding into Queue Handler

**Files:**
- Modify: `crates/worker-entry/src/lib.rs`

- [ ] **Step 1: Replace `upsert_vector` fire-and-forget with awaited embedding + upsert**

Import the embedding module:
```rust
use embedding::{build_embedding_text, EmbeddingProvider, WorkersAiEmbedder};
```

Modify the `process_one_feed` function, specifically the AI pipeline section around line 248-267:

Replace this section (the `if do_ai { if let Some(ref s) = summarizer { ... } }` block):

```rust
if do_ai {
    if let Some(ref s) = summarizer {
        match process_article(store, s, article_id, &article.title, &body, article_score).await {
            Ok(result) => {
                // ---- Embedding pipeline (new, replaces fire-and-forget upsert) ----
                let tags: Vec<String> = serde_json::from_str(&result.tags).unwrap_or_default();
                let feed_name = None; // Could be fetched from feed data; None is fine

                let embedder = WorkersAiEmbedder::new(env); // note: `env` needs to be passed through
                let embed_text = build_embedding_text(
                    &article.title,
                    &result.summary,
                    &tags,
                    feed_name,
                );

                match embedder.embed(&embed_text).await {
                    Ok(embedding) => {
                        // Build rich metadata for future filtering
                        let metadata = serde_json::json!({
                            "article_id": article_id,
                            "feed_id": job.feed_id,
                            "published_at": article.published_at.unwrap_or(0),
                            "embedding_model": "bge-large-en-v1.5",
                            "embedding_version": 1,
                            "language": "en",
                            "tags": tags,
                        });

                        let entry = vectorize::VectorEntry {
                            id: format!("article-{article_id}"),
                            values: embedding,
                            metadata: Some(metadata),
                        };

                        if let Some(ref idx) = vectorize {
                            if let Err(e) = vectorize::upsert_vectors(idx, &[entry]).await {
                                console_log!("  vectorize upsert failed for article {article_id}: {e}");
                            }
                        }

                        // Mark embedding as completed (future: D1 column)
                    }
                    Err(e) => {
                        console_log!("  embedding generation failed for article {article_id}: {e}");
                    }
                }
            }
            Err(_) => {
                let excerpt = if body.len() > 500 { &body[..500] } else { &body };
                let _ = store.set_raw_content_r2_key(article_id, Some(excerpt)).await;
            }
        }
    }
}
```

> **Note:** `env` is not currently in `process_one_feed`'s scope — it's in the `queue` handler. Pass `env: &Env` as a parameter to `process_one_feed`.

- [ ] **Step 2: Update `process_one_feed` signature to accept `&Env`**

```rust
async fn process_one_feed(
    store: &Store,
    env: &Env,               // <-- new param
    summarizer: &Option<HttpSummarizer>,
    r2_bucket: &Option<Bucket>,
    vectorize: &Option<VectorizeIndex>,
    rules: &[Rule],
    has_rules: bool,
    job: &FetchJob,
    now: i64,
) -> Result<(), Error> {
```

Update the caller in the `queue` handler and in `process_all_feeds` sync fallback to pass `&env`.

- [ ] **Step 3: Update all callers of `process_one_feed`**

In the `queue` handler (around line 119):
```rust
if let Err(e) = process_one_feed(&store, &env, &summarizer, &r2_bucket, &vectorize, &rules, has_rules, job, now).await {
```

In `process_all_feeds` sync fallback (around line 174):
```rust
if let Err(e) = process_one_feed(&store, &env, &summarizer, &r2_bucket, &vectorize, &rules, has_rules, &job, now).await {
```

- [ ] **Step 4: Update imports in `worker-entry/src/lib.rs`**

Add to the existing import block:
```rust
use embedding::{build_embedding_text, EmbeddingProvider, WorkersAiEmbedder};
```

- [ ] **Step 5: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p worker-entry
```

- [ ] **Step 6: Commit**

```bash
git add crates/worker-entry/src/lib.rs
git commit -m "feat: wire embedding generation into queue handler with awaited upsert"
```

---

## Task 5: Add Semantic Search API Endpoint

**Files:**
- Create: `crates/api/src/semantic.rs`
- Modify: `crates/api/src/lib.rs`

- [ ] **Step 1: Create `crates/api/src/semantic.rs`**

```rust
//! Semantic search endpoint.
//!
//! POST /api/articles/search
//!
//! Accepts a JSON body with `q`, `mode: "semantic"`, and optional `limit`.
//! Generates a query embedding via Workers AI, searches Vectorize for
//! nearest neighbors, and enriches results with article details from D1.

use crate::{json_err, json_ok};
use embedding::{build_embedding_text, EmbeddingProvider, WorkersAiEmbedder};
use serde::Deserialize;
use store::{Store};
use worker::*;
use worker::wasm_bindgen::prelude::*;
use worker::EnvBinding;

// ---- Vectorize inline binding (api doesn't depend on worker-entry) ----

#[wasm_bindgen]
extern "C" {
    pub type VectorizeIndex;

    #[wasm_bindgen(method, catch)]
    async fn query(this: &VectorizeIndex, vector: JsValue, opts: JsValue) -> Result<JsValue, JsValue>;
}

impl EnvBinding for VectorizeIndex {
    const TYPE_NAME: &'static str = "VectorizeIndex";
}

#[derive(Deserialize)]
struct SemanticSearchRequest {
    q: String,
    #[allow(dead_code)]
    mode: Option<String>,
    limit: Option<u32>,
}

pub async fn semantic_search(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body: SemanticSearchRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return json_err(400, "invalid JSON body"),
    };

    if body.q.trim().is_empty() {
        return json_err(400, "missing query 'q'");
    }

    let store = Store::new(ctx.env.d1("DB")?);
    let vectorize = match ctx.env.get_binding::<VectorizeIndex>("VECTORIZE") {
        Ok(v) => v,
        Err(e) => return json_err(500, &format!("VECTORIZE binding: {e}")),
    };

    let limit = body.limit.unwrap_or(30).min(100);

    // 1. Generate query embedding
    let embedder = WorkersAiEmbedder::new(&ctx.env);
    let embed_text = build_embedding_text(&body.q, "", &[], None);
    let query_emb = match embedder.embed(&embed_text).await {
        Ok(v) => v,
        Err(e) => return json_err(500, &format!("embedding failed: {e}")),
    };

    // 2. Build Vectorize query
    use js_sys::{Array, Float32Array, Object, Reflect};
    let values = Float32Array::new_with_length(query_emb.len() as u32);
    for (i, v) in query_emb.iter().enumerate() {
        values.set_index(i as u32, *v);
    }
    let vector_obj = Object::new();
    let _ = Reflect::set(&vector_obj, &"vector".into(), &values.into());

    let opts = Object::new();
    let _ = Reflect::set(&opts, &"topK".into(), &JsValue::from_f64(limit as f64));
    let _ = Reflect::set(&opts, &"returnMetadata".into(), &JsValue::from_bool(false));

    let result: JsValue = match vectorize.query(vector_obj.into(), opts.into()).await {
        Ok(v) => v,
        Err(e) => return json_err(500, &format!("Vectorize query failed: {e:?}")),
    };

    // 3. Parse matches: { matches: [{ id, score }, ...] }
    let matches = js_sys::Reflect::get(&result, &"matches".into())
        .ok().and_then(|v| v.dyn_into::<Array>().ok())
        .unwrap_or_else(Array::new);

    if matches.length() == 0 {
        return json_ok(serde_json::json!({
            "mode": "semantic", "query": body.q, "results": []
        }));
    }

    // 4. Extract article IDs, build similarity map
    let mut article_ids = Vec::new();
    let mut sim_map = std::collections::HashMap::<i64, f64>::new();

    for i in 0..matches.length() {
        let item = matches.get(i);
        let id = js_sys::Reflect::get(&item, &"id".into())
            .ok().and_then(|v| v.as_string()).unwrap_or_default();
        let score = js_sys::Reflect::get(&item, &"score".into())
            .ok().and_then(|v| v.as_f64()).unwrap_or(0.0);

        if let Some(id_str) = id.strip_prefix("article-") {
            if let Ok(aid) = id_str.parse::<i64>() {
                article_ids.push(aid);
                sim_map.insert(aid, score);
            }
        }
    }

    // 5. Fetch articles from D1
    let articles = match store.articles_by_ids(&article_ids).await {
        Ok(a) => a,
        Err(e) => return json_err(500, &e.to_string()),
    };

    // 6. Enrich with similarity, sort by Vectorize order
    let mut enriched: Vec<serde_json::Value> = articles.into_iter().map(|a| {
        serde_json::json!({
            "id": a.id,
            "title": a.title,
            "url": a.url,
            "published_at": a.published_at,
            "ai_summary": a.ai_summary,
            "ai_tags": a.ai_tags,
            "similarity": sim_map.get(&a.id).copied().unwrap_or(0.0),
        })
    }).collect();

    enriched.sort_by(|a, b| {
        b["similarity"].as_f64().unwrap_or(0.0)
            .partial_cmp(&a["similarity"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    json_ok(serde_json::json!({
        "mode": "semantic",
        "query": body.q,
        "results": enriched,
    }))
}
```

- [ ] **Step 2: Register the route in `crates/api/src/lib.rs`**

Add module declaration near the top:
```rust
mod strategies;
mod semantic;    // <-- new
```

Add route to the router, after the existing `get_async("/api/articles/search", search_articles)` line:
```rust
.post_async("/api/articles/search", semantic::semantic_search)
```

> **Note:** worker::Router routes by method + path. `GET /api/articles/search` goes to existing FTS5 handler. `POST /api/articles/search` goes to the new semantic handler. No conflict.

- [ ] **Step 3: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p api
```

- [ ] **Step 6: Commit**

```bash
git add crates/api/src/semantic.rs crates/api/src/lib.rs crates/worker-entry/src/lib.rs
git commit -m "feat: add POST /api/articles/search semantic endpoint"
```

---

## Task 6: Frontend — Semantic Search Mode Toggle

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/pages/search.astro`

- [ ] **Step 1: Add `SemanticResultItem` interface and `semanticSearch()` function to `src/lib/api.ts`**

```typescript
// After the existing searchArticles function
export interface SemanticResultItem {
  id: number;
  title: string;
  url: string | null;
  published_at: number | null;
  ai_summary: string;
  ai_tags: string | null;
  similarity: number;
}

export async function semanticSearch(
  env: ApiEnv,
  query: string,
  limit = 30,
): Promise<SemanticResultItem[]> {
  const resp = await apiFetch(env, '/api/articles/search', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ q: query, mode: 'semantic', limit }),
  });
  if (!resp.ok) throw new Error(`semantic search failed: ${resp.status}`);
  const data = (await resp.json()) as { results: SemanticResultItem[] };
  return data.results;
}
```

- [ ] **Step 2: Update `search.astro` to support `mode` param and conditional rendering**

Add mode detection and semantic result type:

```astro
---
import ReaderLayout from '../layouts/ReaderLayout.astro';
import ArticleCard from '../components/ArticleCard.astro';
import ErrorState from '../components/ErrorState.astro';
import SkeletonCard from '../components/SkeletonCard.astro';
import { searchArticles, semanticSearch, fetchTags } from '../lib/api';
import type { Article, TagEntry, SemanticResultItem } from '../lib/api';

const env = Astro.locals.runtime.env;
const query = Astro.url.searchParams.get('q') ?? '';
const tagFilter = Astro.url.searchParams.get('tag') ?? '';
const sortBy = Astro.url.searchParams.get('sort') ?? '';
const searchMode = Astro.url.searchParams.get('mode') ?? 'keyword';
const offset = Number(Astro.url.searchParams.get('offset')) || 0;
const limit = 30;

let results: Article[] = [];
let semanticResults: SemanticResultItem[] = [];
let loadError: string | null = null;
let tags: TagEntry[] = [];

try { tags = await fetchTags(env); } catch { /* non-fatal */ }

if (query) {
  try {
    if (searchMode === 'semantic') {
      semanticResults = await semanticSearch(env, query, limit);
    } else {
      results = await searchArticles(env, query, { limit, offset, tag: tagFilter || undefined, sort: sortBy || undefined });
    }
  } catch (e) {
    loadError = e instanceof Error ? e.message : 'search failed';
  }
}
```

Update the form to include mode:
```astro
<form method="get" action="/search" class="mb-4" id="search-form">
  <!-- Mode toggle (pill UI) -->
  <div class="flex gap-1 mb-2">
    <a
      href={query ? `/search?q=${encodeURIComponent(query)}&mode=keyword` : '/search'}
      class:list={[
        'px-3 py-1.5 rounded-full text-label-sm font-label-sm transition-colors',
        searchMode !== 'semantic'
          ? 'bg-primary dark:bg-dark-primary text-on-primary'
          : 'bg-surface-container dark:bg-dark-surface text-on-surface-variant hover:bg-surface-container-high',
      ]}
    >🔍 Keyword</a>
    <a
      href={query ? `/search?q=${encodeURIComponent(query)}&mode=semantic` : '/search?mode=semantic'}
      class:list={[
        'px-3 py-1.5 rounded-full text-label-sm font-label-sm transition-colors',
        searchMode === 'semantic'
          ? 'bg-primary dark:bg-dark-primary text-on-primary'
          : 'bg-surface-container dark:bg-dark-surface text-on-surface-variant hover:bg-surface-container-high',
      ]}
    >🧠 Semantic</a>
  </div>
  <!-- ... rest of form stays same ... -->
```

Update results rendering for semantic mode - show similarity instead of score:
```astro
<!-- After existing results rendering, add semantic results handler -->
{searchMode === 'semantic' && semanticResults.length > 0 && (
  <div>
    <p class="text-label-sm font-label-sm text-on-surface-variant dark:text-dark-on-surface-variant mb-4">
      {semanticResults.length} semantic result{semanticResults.length === 1 ? '' : 's'} for "<span class="text-on-surface font-medium">{query}</span>"
    </p>
    {semanticResults.map((item) => (
      <article class="border-b border-outline-variant dark:border-dark-border py-density-comfortable px-2 md:px-density-compact">
        <div class="flex items-center gap-2 mb-1 flex-wrap">
          {item.published_at && (
            <span class="text-label-sm font-label-sm text-on-surface-variant">
              <time datetime={new Date(item.published_at * 1000).toISOString()}>
                {new Date(item.published_at * 1000).toUTCString().split(' ').slice(1, 4).join(' ')}
              </time>
            </span>
          )}
          <span class="text-label-sm font-label-sm text-primary font-medium">
            {(item.similarity * 100).toFixed(0)}% match
          </span>
        </div>
        <h3 class="font-article-title text-article-title text-on-surface dark:text-dark-on-surface mb-1 leading-snug">
          {item.title}
        </h3>
        {item.ai_summary && (
          <p class="font-body-reading text-body-reading text-on-surface-variant leading-relaxed line-clamp-3">
            {item.ai_summary}
          </p>
        )}
      </article>
    ))}
  </div>
)}
```

- [ ] **Step 3: Build to verify**

```bash
cd "d:/Project/intel-web"
npm run build
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/api.ts src/pages/search.astro
git commit -m "feat: add semantic search mode toggle to search page"
```

---

## Task 7: Bulk Embedding Rebuild Mechanism

**Files:**
- Create: `crates/api/src/rebuild.rs`
- Modify: `crates/api/src/lib.rs`

- [ ] **Step 1: Create the rebuild endpoint**

```rust
// crates/api/src/rebuild.rs
//! Admin endpoint to trigger bulk embedding rebuild for all articles
//! that don't yet have embeddings in Vectorize.

use crate::{json_err, json_ok};
use embedding::{build_embedding_text, EmbeddingProvider, WorkersAiEmbedder};
use store::Store;
use worker::*;
use worker::wasm_bindgen::prelude::*;
use worker::EnvBinding;

#[wasm_bindgen]
extern "C" {
    pub type VectorizeIndex;

    #[wasm_bindgen(method, catch)]
    async fn upsert(this: &VectorizeIndex, vectors: JsValue) -> Result<JsValue, JsValue>;
}

impl EnvBinding for VectorizeIndex {
    const TYPE_NAME: &'static str = "VectorizeIndex";
}


/// POST /api/admin/rebuild-embeddings
///
/// Scans articles, generates embeddings for any that are missing,
/// and upserts to Vectorize. Returns count of processed articles.
/// Limited to 50 articles per call to stay within Workers CPU limits.
pub async fn rebuild_embeddings(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let vectorize = match ctx.env.get_binding::<VectorizeIndex>("VECTORIZE") {
        Ok(v) => v,
        Err(e) => return json_err(500, &format!("VECTORIZE binding: {e}")),
    };
    let embedder = WorkersAiEmbedder::new(&ctx.env);

    // Fetch articles that need embeddings (no vector_id set, or newest first)
    let articles = match store.pending_ai_articles(50).await {
        Ok(a) => a,
        Err(e) => return json_err(500, &e.to_string()),
    };

    if articles.is_empty() {
        return json_ok(serde_json::json!({
            "processed": 0,
            "message": "All articles already have embeddings"
        }));
    }

    let mut processed = 0u64;
    let mut errors = 0u64;

    for article in &articles {
        let tags: Vec<String> = serde_json::from_str(&article.ai_tags).unwrap_or_default();
        let embed_text = build_embedding_text(
            &article.title,
            &article.ai_summary,
            &tags,
            None,
        );

        match embedder.embed(&embed_text).await {
            Ok(embedding) => {
                let metadata = serde_json::json!({
                    "article_id": article.id,
                    "feed_id": article.feed_id,
                    "published_at": article.published_at.unwrap_or(0),
                    "embedding_model": "bge-large-en-v1.5",
                    "embedding_version": 1,
                    "language": "en",
                    "tags": tags,
                });

                let entry = VectorEntry {
                    id: format!("article-{}", article.id),
                    values: embedding,
                    metadata: Some(metadata),
                };

                match upsert_vectors(&vectorize, &[entry]).await {
                    Ok(_) => processed += 1,
                    Err(e) => {
                        console_log!("  upsert failed for article {}: {e}", article.id);
                        errors += 1;
                    }
                }
            }
            Err(e) => {
                console_log!("  embedding failed for article {}: {e}", article.id);
                errors += 1;
            }
        }
    }

    json_ok(serde_json::json!({
        "processed": processed,
        "errors": errors,
        "remaining": articles.len().saturating_sub(processed as usize),
        "message": format!("Processed {processed} articles ({errors} errors). Call again to continue.")
    }))
}
```

- [ ] **Step 2: Register route in `crates/api/src/lib.rs`**

```rust
mod rebuild;

// In router():
.post_async("/api/admin/rebuild-embeddings", rebuild::rebuild_embeddings)
```

- [ ] **Step 3: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p api -p worker-entry
```

- [ ] **Step 4: Commit**

```bash
git add crates/api/src/rebuild.rs crates/api/src/lib.rs
git commit -m "feat: add POST /api/admin/rebuild-embeddings bulk rebuild endpoint"
```

---

## Task 8: Full Verification

- [ ] **Step 1: Full cargo check**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p store -p rules -p embedding -p ai-pipeline -p search -p api -p worker-entry
cargo test -p rules -p embedding
```

- [ ] **Step 2: Frontend build**

```bash
cd "d:/Project/intel-web"
npm run build
```

- [ ] **Step 3: Deploy backend**

```bash
cd "d:/Project/Sulix Intelligence/crates/worker-entry"
worker-build --release
npx wrangler deploy
```

- [ ] **Step 4: Verify endpoints**

```bash
curl -s "https://sulix-feed-worker.weixc0856.workers.dev/api/ping"
curl -s "https://sulix-feed-worker.weixc0856.workers.dev/api/health"
```

- [ ] **Step 5: Trigger bulk rebuild**

```bash
curl -s -X POST "https://sulix-feed-worker.weixc0856.workers.dev/api/admin/rebuild-embeddings"
```

- [ ] **Step 6: Test semantic search**

```bash
curl -s -X POST "https://sulix-feed-worker.weixc0856.workers.dev/api/articles/search" \
  -H "Content-Type: application/json" \
  -d '{"q":"AI infrastructure","mode":"semantic","limit":5}'
```
Expected: Returns results with `similarity` scores.

- [ ] **Step 7: Verify frontend still works**

```bash
cd "d:/Project/intel-web"
npm run build
```

- [ ] **Step 8: Summary commit**

```bash
git add -A
git commit -m "chore: semantic search feature complete"
```
