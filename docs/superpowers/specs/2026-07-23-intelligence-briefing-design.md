# Intelligence Briefing — Signal Discovery Interface

> **Status:** Approved  
> **Date:** 2026-07-23  
> **Author:** Brainstorming → Design Review  

---

## Context

Sulix Intelligence currently presents as an RSS reader: `/intelligence` shows a list of articles, `/dashboard` shows operational stats. The core pipeline (RSS → AI Summary + Tags → Signal Strategy → Scoring → Vectorize) runs invisibly in the background. Users never see the **intelligence layer**.

This spec repositions `/intelligence` as the **Intelligence Briefing** — a Signal Discovery Interface exposing the pipeline's value.

---

## Design Principles

1. **Signal-first, not article-first** — The page answers "what's happening" not "what's new"
2. **Evidence builds trust** — Every signal links to its source articles
3. **Discovery over search** — Surface what users don't know to look for
4. **Read-only projection** — Signals are pipeline output, not user-configurable

---

## Page Structure

```
SULIX INTELLIGENCE · {date} · {N} active strategies
─────────────────────────────────────────────────────

🔥 Today's Signals                          │  Evidence Stream
                                             │
┌──────────────────────┐  ┌─────────────┐   │  ┌──────────────────┐
│ AI Infrastructure    │  │ Open Source  │   │  │ NVIDIA announces  │
│ Expansion           │  │ AI Models    │   │  │ Blackwell Ultra   │
│ 87% confidence      │  │ 74% conf    │   │  │ 5h ago · TC       │
│ 23 sources          │  │ 15 sources  │   │  ├──────────────────┤
│ ↑ Rising            │  │ → Stable    │   │  │ OpenAI GPT-6      │
│ AI infrastructure   │  │             │   │  │ 2h ago · Blog     │
│ spending...         │  │             │   │  ├──────────────────┤
│ [Explore]           │  │ [Explore]   │   │  │ Meta Llama model  │
└──────────────────────┘  └─────────────┘   │  │ 8h ago · Meta    │
                                             │  └──────────────────┘
────────────────────────────────────────────
🧠 Semantic Search — [Ask about any topic...]
```

### Three Components

**1. Signal Cards** — Title, confidence %, evidence count, trend, summary, "Explore" link (→ `/search?q={full signal title}&mode=semantic`)

**2. Evidence Stream** — Latest matched articles across all signals, sorted by signal relevance × recency. Not pure chronological.

**3. Semantic Search** — Text input → `/search?q=...&mode=semantic`

---

## API Design

### GET /api/signals/today

**Response:**
```json
{
  "date": "2026-07-23",
  "generated_at": "2026-07-23T08:30:00Z",
  "signals": [
    {
      "id": "ai_infrastructure_expansion",
      "title": "AI Infrastructure Expansion",
      "summary": "AI infrastructure spending accelerating across hyperscalers with record capex commitments.",
      "confidence": 0.87,
      "evidence_count": 23,
      "trend": "rising",
      "articles": [
        { "id": 1234, "title": "NVIDIA announces Blackwell Ultra", "source": "TechCrunch", "published_at": 1784779000 },
        { "id": 1235, "title": "OpenAI infrastructure investment", "source": "OpenAI Blog", "published_at": 1784775000 }
      ]
    }
  ]
}
```

**Backend logic:**
1. Fetch articles from last 7 days where `score != 0` (matched enabled strategies)
2. Filter: only articles with `score >= 0.6` (avoid noise)
3. Group by `signal_type` from the matched strategy
4. Confidence formula: `0.4 × frequency_score + 0.3 × source_diversity + 0.3 × recency`
   - frequency_score: normalized article count within group
   - source_diversity: unique feed URLs in group / total feeds
   - recency: decay weight (newer articles score higher)
5. ID: projection derived from group name (snake_case), e.g. `ai_infrastructure_expansion`
6. Trend: compare article count in last 3 days vs 3 days before that
7. Evidence Stream: articles sorted by `(signal_confidence × 0.6) + (recency_normalized × 0.4)`

**No pagination** — V1 returns all signals (expected < 20).

### Future compatibility

```json
{
  "date": "2026-07-23",
  "generated_at": "2026-07-23T08:30:00Z",
  "signals": [ ... ],
  "themes": [],     // V2
  "decisions": []   // V2
}
```

---

## Frontend Changes

| File | Change |
|------|--------|
| `src/pages/intelligence.astro` | Rewrite to Briefing layout (Signal Cards + Evidence Stream + Semantic Search) |
| `src/lib/api/signals.ts` | New: `fetchTodaySignals()` |
| Sidebar nav | Unchanged — `/intelligence` same URL |

Existing `/intelligence?article=N` detail panel → removed. Article navigation goes to `/article/N`.

---

## Backend Changes

| File | Change |
|------|--------|
| `crates/store/src/lib.rs` | New `signals_today()` method |
| `crates/store/src/models.rs` | New `TodaySignal` / `SignalEvidence` types |
| `crates/api/src/lib.rs` | New `GET /api/signals/today` route + handler |

---

## Constraints

- Signals are read-only (projection, not entity)
- Article is primary evidence — signals link back
- No charts (V1 text-only)
- No personalization
- No WebSocket

---

## Edge Cases

- **No signals yet:** "Analyzing feeds... Signals will appear once enough articles are processed."
- **0 matched articles:** Empty `signals` array
- **Single article per signal:** Still shown, confidence may be low
- **All articles score = 0:** "No significant signals detected today."

---

## Implementation Order

| Step | Description | Files | Effort |
|------|-------------|-------|--------|
| 1 | Store: `signals_today()` query + models | `crates/store/src/lib.rs`, `models.rs` | Medium |
| 2 | API: `GET /api/signals/today` handler + debug preview via cli | `crates/api/src/lib.rs` | Medium |
| 3 | Frontend: `fetchTodaySignals()` + types | `src/lib/api/signals.ts` (new) | Small |
| 4 | Frontend: Rewrite `/intelligence` as Briefing | `src/pages/intelligence.astro` | Large |
| 5 | Build + deploy + verify | — | Small |
