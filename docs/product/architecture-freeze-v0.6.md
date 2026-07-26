# Architecture Freeze — Sulix Intelligence v0.6

**Status:** Release checkpoint  
**Date:** 2026-07-27  
**Previous:** `docs/product/source-governance.md`, `docs/product/judgment-os.md`

---

## Completed Capabilities

### Ingestion Layer
- RSS/Atom fetch with conditional HTTP + SSRF guard — `crates/fetcher`
- Per-feed full-text extraction (opt-in) — `crates/fetcher`
- Article dedup via `UNIQUE(feed_id, guid)` — `crates/store`

### Source Governance (Sprint 5.6)
- Source Registry — tier, policy, license, trust_score — `crates/store/src/models/source.rs`
- Content Policy Enforcer (4-tier matrix) — `crates/content-governance`
- Provenance Chain — `ArticleProvenance`, `ProvenanceSummary`
- Compliance — Takedown workflow + audit events

### Signal Engine
- Signal discovery, clustering, scoring — `crates/intelligence/signal-engine`
- Radar projection (batch queries, N+1 eliminated) — `crates/application/src/radar.rs`
- Signal health + trend + evidence

### Claim Engine (Sprint 5.9C)
- Claim extraction via LLM (falsifiable, typed) — `crates/claim-engine`
- ClaimType: Fact, Trend, Prediction, Causal, Opinion
- Confidence evaluated by ConfidenceEngine v2 (not LLM)

### Reasoning Runtime (Sprint 5.9B)
- `ModelProvider` trait + `RealDeepSeek` + `NoopProvider` — `crates/model-runtime`
- Agent adapter: `ModelProviderLLM` → Agent
- Reflection adapter: `RealReflectionGenerator` → Reflection
- Shared `IntelligenceRuntime` initialization

### Evaluation & Calibration (Sprint 5.9D)
- `reasoning_runs` — every model invocation logged
- `reasoning_evaluations` — quality scoring
- `confidence_calibrations` — predicted vs actual tracking
- Trust Dashboard — Model Reliability + Calibration + Decision Accuracy

### Decision Intelligence (Sprint 6.0)
- Decision Records — action-feedback-learning entity
- Decision Outcomes — multiple metrics per decision
- Decision-Claim linking (4 relationship types)
- Decision Memo generator (12-section consulting format)
- Decision Proposal builder (Signal → Proposal → Human)
- Trust Dashboard decision accuracy metrics

### AI Pipeline
- `HttpSummarizer` delegating to `ModelProvider`
- Briefing generation (daily intelligence brief)

### Frontend — 27+ pages
- Intelligence Radar, Signal Detail, Decision Detail, Confidence Evolution
- Trust Center, Source Card, Provenance Panel
- Decision Graph (Cytoscape.js, 3 layout modes)
- Navigation state resolution (centralized `resolveActiveNav`)
- Cognitive UX components (ConfidenceEvolution, WhySulixThinksThis)

---

## Architecture Diagram

```
Sources → Source Governance → Observation
                                       ↓
                                  Evidence
                                       ↓
                               Claim Engine (5.9C)
                                       ↓
                              Signal Engine (existing)
                                       ↓
                              Reasoning Runtime (5.9B)
                              ┌──────────────────┐
                              │  ModelProvider    │
                              │  RealDeepSeek     │
                              │  NoopProvider     │
                              └────────┬─────────┘
                                       ↓
                  ┌────────────────────┼────────────────────┐
                  │                    │                    │
            Agent Engine         Reflection Engine     Claim Extraction
                  │                    │                    │
                  └────────────────────┼────────────────────┘
                                       ↓
                            Decision Intelligence (6.0)
                                       ↓
                              Outcome Tracking
                                       ↓
                                 Reflection
                                       ↓
                              Evaluation & Calibration (5.9D)
                                       ↓
                              Memory Engine
                                       ↓
                              Context Engine
```

---

## Test Statistics

| Suite | Count |
|-------|-------|
| Backend `cargo test --workspace` | 154 |
| Frontend `vitest` | 42 |
| **Total** | **196** |

---

## Deployment

| Worker | Version | Platform |
|--------|---------|----------|
| `sulix-feed-worker` | `486fb4c7` | Cloudflare Workers |
| `sulix-feed-frontend` | `94b1d300` | Cloudflare Workers |

---

## Repository

| Repo | Branch | Tag |
|------|--------|-----|
| `weixc0856-cell/Sulix-Intelligence` | `master` | `v0.6.0` |
| `weixc0856-cell/Intel-Web` | `main` | `v0.6.0` |

---

## Next: Stabilization Phase

Not Sprint 6.1. Instead, accumulate:

- 50+ Claims
- 20+ Decisions
- 10+ Outcomes

Then analyze calibration data before proceeding to Judgment OS.
