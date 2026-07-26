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
  "answer": "Based on your decision history, you have 3 AI-related investments...",
  "reasoning": {
    "confidence": 0.82,
    "evidence_refs": ["DEC-001", "MEM-003", "REF-002"],
    "assumptions": ["assuming current market trends continue"],
    "uncertainty": ["limited long-term market data"],
    "reasoning_version": "v1"
  },
  "context": {
    "decisions_count": 3,
    "reflections_count": 2,
    "memories_count": 4,
    "patterns_count": 1,
    "evidence_refs": ["DEC-001", "MEM-003", "REF-002"]
  },
  "context_id": "CTX-1710000000",
  "execution": {
    "mode": "decision_advisor",
    "model": "noop",
    "prompt_version": "decision_advisor.v1",
    "reasoning_version": "v1",
    "generated_at": 1710000000,
    "latency_ms": 1234,
    "stages": ["ContextBuilding", "PromptConstruction", "LLMInference", "ResponseValidation", "Completed"]
  },
  "session_id": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `answer` | string | Natural language answer |
| `reasoning` | object | ReasoningTrace with confidence + evidence |
| `reasoning.confidence` | float | 0.0–1.0 |
| `reasoning.evidence_refs` | string[] | Source IDs cited |
| `reasoning.assumptions` | string[] | Assumptions made during reasoning |
| `reasoning.uncertainty` | string[] | Known uncertainties |
| `context` | object | ContextSummary — counts for UI |
| `context.decisions_count` | int | Number of relevant decisions found |
| `context.reflections_count` | int | Number of relevant reflections found |
| `context.memories_count` | int | Number of relevant memories found |
| `context.patterns_count` | int | Number of detected patterns |
| `context.evidence_refs` | string[] | All source IDs in context |
| `context_id` | string | Snapshot ID for traceability |
| `execution` | object | ExecutionMetadata (model, latency, stages) |
| `session_id` | string or null | Echoed from request |

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
- `503` — D1 binding unavailable
- `500` — agent execution failure (LLM error, validation failure)

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
