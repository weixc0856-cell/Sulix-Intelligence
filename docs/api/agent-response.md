# Agent API Contract

**Endpoint:** `POST /api/internal/agent/run`

**Version:** v1 (frozen at Sprint 5.8)

**Purpose:** Internal endpoint for Agent Reasoning Engine. Not exposed to external clients directly — consumed by the Astro frontend via service binding.

---

## Request

```json
{
  "query": "Should I invest in AI startups?",
  "mode": "decision_advisor",
  "session_id": null,
  "options": null
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | yes | Natural language question |
| `mode` | string | yes | Only `decision_advisor` in v1 |
| `session_id` | string or null | no | Request correlation (not persisted) |
| `options` | object or null | no | `{ include_evidence?, max_context_items? }` |

---

## Response (200)

```json
{
  "answer": "Based on 5 past decisions in your history, the balance of evidence favors a staged approach...",
  "reasoning": {
    "confidence": 0.38,
    "evidence_refs": ["DEC-000001", "DEC-000002", "DEC-000003", "DEC-000004", "DEC-000005"],
    "assumptions": [],
    "uncertainty": [],
    "reasoning_version": "v1"
  },
  "context": {
    "decisions_count": 5,
    "reflections_count": 0,
    "memories_count": 0,
    "patterns_count": 0,
    "evidence_refs": ["DEC-000001", "DEC-000002", "DEC-000003", "DEC-000004", "DEC-000005"]
  },
  "context_id": "CTX-1710000000",
  "execution": {
    "mode": "decision_advisor",
    "model": "noop",
    "prompt_version": "decision_advisor.v1",
    "reasoning_version": "v1",
    "generated_at": 1710000000,
    "latency_ms": 1234,
    "stages": ["ContextBuilding", "PromptConstruction", "LLMInference", "Completed"]
  },
  "session_id": null,
  "insufficient_evidence": false,
  "disclaimer": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `answer` | string | Natural language answer |
| `reasoning` | object | ReasoningTrace with confidence + evidence |
| `reasoning.confidence` | float | 0.0–1.0 |
| `reasoning.evidence_refs` | string[] | Source IDs cited (system-derived from context evidence — all `DEC-*`) |
| `reasoning.assumptions` | string[] | Assumptions made during reasoning |
| `reasoning.uncertainty` | string[] | Known uncertainties |
| `context` | object | ContextSummary — counts for UI |
| `context.decisions_count` | int | Number of eligible decisions found |
| `context.reflections_count` | int | Number of reflections found (feed currently empty — reserved seam) |
| `context.memories_count` | int | Number of memories found (feed currently empty — reserved seam) |
| `context.patterns_count` | int | Number of detected patterns |
| `context.evidence_refs` | string[] | All source IDs in context |
| `context_id` | string | Snapshot ID for traceability |
| `execution` | object | ExecutionMetadata (model, latency, stages) |
| `session_id` | string or null | Echoed from request |
| `insufficient_evidence` | bool | `true` when fewer than 5 eligible decisions were in context |
| `disclaimer` | string or null | Set when `insufficient_evidence` is true (else `null`) |

## Evidence & the insufficiency gate

- **Eligible evidence** = decisions with status `active` (executing) or `completed`.
  Superseded / draft / proposed / approved decisions never enter Advisor context.
- **Insufficient evidence is a valid 200**, not an error. When fewer than 5 eligible
  decisions are in context, the response returns `insufficient_evidence: true` plus a
  `disclaimer` and the prompt requires the model to say evidence is sparse rather than
  inventing decisions. The gate is computed from the system-side context (pre-LLM) —
  model output cannot inflate it.

---

## Error Response (4xx/5xx)

```json
{
  "error": "agent execution failed",
  "status": 500
}
```

Common errors:
- `400` — invalid request body (JSON parse failure)
- `503` — `AI_API_KEY` missing/empty (fail-closed: the Advisor never fakes a 200
  via Noop when no provider is configured; set `AI_PROVIDER=noop` explicitly to
  allow the local-dev Noop escape hatch)
- `500` — agent execution failure (LLM/provider error)

---

## Frontend Consumption

The Astro frontend should NOT consume `AgentResponse` directly. Use the adapter layer:

```
src/lib/agent/types.ts     ← AgentViewModel (UI-facing)
src/lib/agent/api.ts       ← runAgent() → AgentResponse
src/lib/agent/mapper.ts    ← AgentResponse → AgentViewModel
```

The `context` field is specifically designed for UI rendering:
- `context.decisions_count` — show "Past Decisions: 3" badge
- `context.evidence_refs` — link to individual Decision/Reflection/Memory pages
