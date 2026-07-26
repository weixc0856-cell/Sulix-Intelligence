# Product Decision Record: Source Governance & Intelligence Positioning

**Status:** Accepted — product architecture freeze record  
**Date:** 2026-07-26  
**Author:** Sulix-Bot  
**Scope:** Product boundary, source governance, domain model, pricing principle

---

## Decision

Sulix is **not an RSS Reader**.

RSS is an internal acquisition protocol, not a user-facing product surface.

Sulix provides:

> Curated, analyzed, provenance-backed intelligence streams that improve decisions over time.

The product value lies in:

```
Observation → Evidence → Claim → Signal → Decision → Outcome → Memory
```

not:

```
Feed → Article → Read
```

---

## Product Boundary

### Infrastructure Layer

Internal system components, not exposed to normal users:

| Component | Description |
|---|---|
| RSS Feed ingestion | Feed fetching, parsing, deduplication |
| Article extraction | HTML-to-text, R2 archiving |
| Embedding pipeline | Vector generation, Vectorize indexing |
| LLM summarization | AI summary, tag extraction |
| Pipeline metrics | KV-based per-cycle observability |

Governed by Source Registry — optimized for ingestion reliability.

### Intelligence Layer

Core product capability — user visible, monetizable, differentiated:

| Capability | Status |
|---|---|
| Source Governance (tier, policy, trust) | ✅ Sprint 5.6 |
| Content Policy Enforcement | ✅ Sprint 5.6 |
| Provenance Chain | ✅ Sprint 5.7 |
| Confidence Engine (factor-based) | ✅ Sprint 5.8A |
| Decision Explanation | ✅ Sprint 5.8B |
| Trust Dashboard | ✅ Sprint 5.8C |
| Real LLM Provider | ❌ Sprint 5.9 |
| Strategy OS | ❌ Sprint 6.0+ |

### User Surface Layer

Future product entry — Intelligence Domains:

```
AI Agent Intelligence

42 verified sources
12 observations today
3 emerging signals
78% confidence
5 previous decisions
```

---

## Current Completed Foundation

### Source Registry ✅

Purpose: Control information provenance.

- source tier (Tier0–Tier3)
- content policy (MetadataOnly / SummaryAllowed / FullTextAllowed / UserOwned)
- license metadata
- attribution text
- trust score (0.0–1.0)

### Content Governance ✅

Purpose: Control what Sulix can ingest and expose.

| Policy | Storage | Serving | Embedding | AI Summary |
|--------|---------|---------|-----------|------------|
| MetadataOnly | Denied | Denied | Denied | TitleOnly |
| SummaryAllowed | Denied | Denied | Limited | Allowed |
| FullTextAllowed | Allowed | Allowed | Allowed | Allowed |
| UserOwned | Allowed | Allowed | Allowed | Allowed |

### Provenance Chain ✅

Every intelligence artifact carries lineage:

```
Source → Observation → Evidence → Signal → Claim → Decision → Memory
```

Each artifact answers:
- Where did this come from?
- Why was it selected?
- How confident is Sulix?

### Confidence Engine v2 ✅

Moved from:

```
LLM says confidence = 0.85
```

to:

```
Confidence = (evidence_strength × source_trust × freshness × calibration)^(1/4)
```

Fully interpretable, per-factor explanation for every confidence value.

---

## Source Tiers

### System Curated

Sulix maintained. Core intelligence graph.

- Research institutions (arXiv, MIT, Stanford)
- Official engineering blogs (OpenAI, Anthropic, Cloudflare, Meta)
- Trusted publications (Reuters, TechCrunch, Ars Technica, Wired)
- Industry analysis (Simon Willison, Latent Space, Stratechery)

Users cannot edit. Managed via `sources` table (`tier = 'Tier0'` or `'Tier1'`).

### Community Sources (Future)

```
User submission → Review Queue → Trust Scoring → Promotion to Global
```

Similar to GitHub PR workflow. Not immediately entered into the main pipeline.

### Private Sources (Future)

User-owned:
- Private RSS feeds
- Internal newsletters
- Personal knowledge bases

Isolated: **does not affect** the global intelligence graph.

---

## Future Domain Model

**Not now.** After:

```
Sprint 5.8 Trust Layer
Sprint 5.9 Real Intelligence Engine
Sprint 6.0 Strategy OS
```

Then introduce:

```
Intelligence Domain

Example: "AI Research"

Sources:      42
Observations: 12,000
Signals:      350
Decisions:    28
Confidence:   continuously updated
```

A Domain is **not a folder**. A Domain is a **living intelligence model**:

- owns a curated source list
- accumulates observations over time
- tracks signal evolution
- stores decision history
- maintains confidence profile
- preserves memory

---

## Pricing Principle

**Do not price by RSS count, feed count, or storage amount.** These are infrastructure metrics.

**Price by intelligence depth:**

| Tier | Features | Target |
|------|----------|--------|
| **Free** | Limited domains, daily briefing, basic signals | Individual exploration |
| **Pro** (¥99/mo) | Unlimited domains, decision memory, confidence evolution, graph | Knowledge workers |
| **Team** (¥499/mo) | Shared intelligence, private sources, compliance, API | Research / Enterprise |

---

## Not Doing Now

### ❌ Feed Management as Product

Reason: Commodity market. Competitors: Feedly, Inoreader, NewsBlur, Miniflux.

The current `/feeds` page stays as an **internal admin tool**, not a user-facing product surface. It will be moved to `/admin/sources` when the Domain model ships.

### ❌ Domain Subscription Layer

Reason: Needs Trust Layer first.

Without trust, a Domain is just a category page. With trust, a Domain is a **curated intelligence asset** that users pay for.

### ❌ Pricing Finalization

Reason: Depends on final Intelligence Domain model.

---

## Roadmap

```
Sprint 5.8 Trust Layer         ← current
Sprint 5.9 Real LLM            ← next
Sprint 6.0 Strategy OS         ← future
Product Surface Refactor       ← after Strategy OS
Intelligence Domain Subscription ← final
```

---

## Product Principle

> Users do not subscribe to information sources.
> Users subscribe to continuously improving intelligence.

Sulix's moat is built on:

```
Trust + Memory + Decision Intelligence
```

not on:

```
Feed count + Storage volume + Reader features
```

---

## References

- `crates/store/src/models/source.rs` — Source model with tier, policy, license, trust_score
- `crates/content-governance/` — Policy evaluation engine (pure logic)
- `crates/store/src/models/provenance.rs` — Provenance chain model
- `crates/store/src/domain/confidence/` — Confidence factors + calculator
- `crates/api/src/routes/article.rs` — Provenance response in article detail
- `src/features/intelligence/provenance/SourceCard.astro` — Source provenance UI
- `src/features/intelligence/provenance/ProvenancePanel.astro` — Evidence origin UI
