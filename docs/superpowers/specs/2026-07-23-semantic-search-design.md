# Semantic Search — Sulix Intelligence Knowledge Retrieval Layer

> **Status:** Approved  
> **Date:** 2026-07-23  
> **Author:** Brainstorming → Design Review  

---

## Context

Sulix Intelligence currently has only keyword-based search (D1 FTS5). The AI pipeline generates summaries and tags but discards the embedding step (`AI_EMBEDDING_MODEL = ""` → `Vec::new()` → no-op upsert). The Vectorize index (`sulix-article-embeddings`) exists with **1024 dimensions, cosine metric, 0 vectors** — a clean slate.

Semantic search closes the pipeline's last gap: article → AI Summary → **Embedding** → Vectorize, enabling concept-level retrieval that keyword matching cannot provide.

---

## Architecture

```
RSS Fetch → AI Summary + Tags
                ↓
      Embedding Service (new, independent)
                ↓
  Workers AI bge-large-en-v1.5 (1024d)
                ↓
        Vectorize upsert (await)


User Query → Workers AI → Query Embedding
                ↓
        Vectorize.query() → top-N
                ↓
        D1 batch fetch → Results
```

## Design Principles

1. **Rust type safety** — Vectorize query is wrapped in typed functions. No `JsValue` leaks into business logic.
2. **Metadata for future filtering** — Each vector stores `article_id`, `feed_id`, `published_at`, `embedding_version`, `tags`.
3. **Search mode coexistence** — Keyword (FTS5) is default. Semantic is opt-in. Future hybrid is API-compatible.
4. **Embedding is part of pipeline, not HTTP** — Runs in queue consumer. Bulk rebuild uses Queue, not CLI as primary.

---

## Infrastructure

### wrangler.toml — AI binding

```toml
[[ai]]
binding = "AI"
```

Enables `env.ai()` calling `@cf/baai/bge-large-en-v1.5` (1024d, free, zero cost).

### Existing index

```
sulix-article-embeddings:
  dimensions: 1024  ✅ bge-large-en-v1.5
  metric: cosine    ✅ ideal for semantic
  vectors: 0        ✅ clean slate
```

---

## Embedding Service

### Separate abstraction (not deep in ai-pipeline)

The embedding logic lives as a focused module, not mixed into `ai-pipeline`'s summarizer. A dedicated `EmbeddingProvider` trait in a distinct crate boundary keeps it swappable:

```
crates/
  ai-pipeline/    ← summarization + tagging
  embedding/      ← vector generation (new, focused)
  search/         ← FTS5 + semantic query
```

`EmbeddingProvider` trait:

```rust
#[async_trait(?Send)]
pub trait EmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}
```

One implementation: `WorkersAiEmbedder` (Cloudflare AI binding). Future: BGE-M3, OpenAI.

### What gets embedded

```text
Title:
{article.title}

Summary:
{article.ai_summary}

Topics:
{tags comma-separated}

Source:
{feed_name}
```

Field labels help the model understand semantics — NOT raw `Tags: AI,GPU`.

### Metadata stored with each vector

```json
{
  "article_id": 1234,
  "feed_id": 3,
  "published_at": 1784779000,
  "embedding_model": "bge-large-en-v1.5",
  "embedding_version": 1,
  "language": "en",
  "tags": ["AI", "GPU"]
}
```

The `embedding_version` field is critical for model migrations. When the model changes (e.g. to BGE-M3 for multilingual support), old and new vectors coexist with different version tags.

### Embedding status tracking

A new `embedding_status` column on `articles`:

```
pending   → article ready for embedding
completed → embedding upserted successfully
failed    → embedding errored (non-fatal to article pipeline)
```

### Where embedding runs

In the queue consumer, after summarization:

1. AI Summary + Tags generated (existing)
2. Embedding text constructed
3. `embedding.embed(text)` → 1024-d vector
4. Vectorize upsert (await, not fire-and-forget)
5. On failure: set `embedding_status = failed`, log warning, continue

---

## Vectorize Binding

### Add `query` to wasm binding

```rust
#[wasm_bindgen]
extern "C" {
    pub type VectorizeIndex;

    #[wasm_bindgen(method, catch)]
    async fn upsert(this: &VectorizeIndex, vectors: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn query(this: &VectorizeIndex, vectors: JsValue, opts: JsValue) -> Result<JsValue, JsValue>;
}
```

### Typed Rust wrapper

```rust
pub struct VectorMatch {
    pub id: String,
    pub score: f32,
    pub metadata: Option<serde_json::Value>,
}

pub async fn query_vectors(
    index: &VectorizeIndex,
    embedding: &[f32],
    top_k: u32,
) -> Result<Vec<VectorMatch>, String>
```

No `JsValue` exposure outside this module.

---

## API Endpoint

```http
POST /api/articles/search
```

Request:
```json
{
  "q": "AI infrastructure investment",
  "mode": "semantic",
  "limit": 30
}
```

Response:
```json
{
  "mode": "semantic",
  "query": "AI infrastructure investment",
  "results": [
    {
      "id": 1234,
      "feed_id": 3,
      "feed_name": "TechCrunch",
      "title": "OpenAI releases GPT-6",
      "url": "https://...",
      "published_at": 1784779000,
      "ai_summary": "OpenAI announced...",
      "similarity": 0.87
    }
  ]
}
```

**Note:** Only `similarity` is returned — not both `score` and `similarity`. Vectorize's match `score` IS the similarity score. Unified field avoids frontend confusion.

**Backend flow:**
1. Generate query embedding via Workers AI (same model)
2. `Vectorize.query(query_embedding, top_k=limit*2)` → top-N matches
3. Extract `article_id` from each match's metadata
4. D1 batch fetch articles by IDs (preserving Vectorize order)
5. Return enriched results with `similarity` from Vectorize

### Compatibility

Existing: `GET /api/articles/search?q=...` (FTS5, unchanged)  
New: `POST /api/articles/search { mode: "semantic" }`  
Future: `POST /api/articles/search { mode: "hybrid" }`

---

## Frontend

### search.astro

Toggle next to search bar:

```
[🔍 Keyword | 🧠 Semantic]
```

- Default: Keyword (existing FTS5 UX)
- When Semantic selected: POST JSON to `/api/articles/search` with `mode: semantic`
- URL param: `?mode=semantic` for shareability
- Results show similarity percentage instead of FTS5 rank

---

## Bulk Rebuild

After deployment, ~533 existing articles need embeddings.

**MVP:** One-shot queue trigger — enqueue all article IDs to a dedicated queue, consumer generates and upserts embeddings in batches of 10.

**Future path:** Dedicated embedding queue consumer (decoupled from RSS fetch queue). This scales to 100k+ articles without blocking feed ingestion.

---

## Multilingual Future

`@cf/baai/bge-large-en-v1.5` is English-optimized. When Chinese or other language feeds are added, swap to `bge-m3` (multilingual, same 1024d support) via the `EmbeddingProvider` trait — no pipeline changes needed.

---

## Edge Cases

- **Empty query:** 400
- **Zero results:** Empty `results` array, `mode: "semantic"`
- **Vectorize timeout:** 503 with retry suggestion
- **Embedding failure:** Continue article processing, mark `embedding_status = failed`
- **Workers AI rate limit:** At current scale (~500 articles, batches of 10), no issues
- **Dimension mismatch:** Already verified ✅

---

## Testing

### Embedding Contract Test

```rust
#[test]
fn embedding_dimension() {
    let text = "AI infrastructure investment";
    let embedding = generate_embedding(text).await;
    assert_eq!(embedding.len(), 1024);
}
```

### Vectorize Roundtrip Test

```rust
#[test]
async fn vectorize_query_roundtrip() {
    let v = vec![0.1_f32; 1024];
    upsert_vectors(&index, &[VectorEntry { id: "test-001".into(), values: v.clone(), metadata: None }]).await.unwrap();
    let results = query_vectors(&index, &v, 5).await.unwrap();
    assert!(results.iter().any(|m| m.id == "test-001"));
    delete_vectors(&index, &["test-001"]).await.unwrap();
}
```

---

## Implementation Order

| Step | Description | Files | Effort |
|------|-------------|-------|--------|
| 1 | Add AI binding to wrangler.toml | `crates/worker-entry/wrangler.toml` | Small |
| 2 | Add `query` to Vectorize binding + typed wrapper | `crates/worker-entry/src/vectorize.rs` | Small |
| 3 | New `embedding` crate with `EmbeddingProvider` trait + Workers AI impl | `crates/embedding/` (new) | Medium |
| 4 | Wire embedding into queue handler | `crates/worker-entry/src/lib.rs` | Medium |
| 5 | Add `POST /api/articles/search` semantic endpoint | `crates/api/src/semantic.rs` (new) | Medium |
| 6 | Frontend: mode toggle + semantic fetch | `src/pages/search.astro`, `src/lib/api.ts` | Medium |
| 7 | Bulk rebuild mechanism | Queue-based one-shot trigger | Medium |
| 8 | Verify | cargo check + npm build + wrangler tail | Small |
