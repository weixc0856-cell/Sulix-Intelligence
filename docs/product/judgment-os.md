# Product Decision Record: Judgment OS v1.1

**Status:** Accepted — Strategic Architecture Direction  
**Date:** 2026-07-27  
**Target Release:** Post Sprint 6.0  
**Scope:** Product position, cognitive architecture, knowledge governance, implementation roadmap

---

## 1. Core Decision

Sulix will evolve from an Intelligence Pipeline into a **Judgment Intelligence Platform**.

Sulix is not designed to replace human decision makers or generate arbitrary opinions. Its purpose is:

> Apply established knowledge frameworks, evidence evaluation, uncertainty modeling, and outcome calibration to help humans make higher-quality decisions.

---

## 2. Strategic Positioning

### Current AI Market

Most AI systems optimize:

```
Information Retrieval → Generation → Automation → Execution
```

Examples: search agents, coding agents, workflow agents. These capabilities are increasingly commoditized.

### Sulix Differentiation

Sulix focuses on:

```
What matters? → What is likely true? → What should we believe? → What should we do? → Was the decision correct?
```

The core asset: **Judgment Quality Loop** — a closed feedback system where every judgment is tracked, evaluated, and calibrated.

---

## 3. Cognitive Architecture

```
                    Knowledge Layer
        ┌───────────┬───────────┬───────────┐
        │ Finance   │ Strategy  │ Psychology│
        │ Risk      │ Industry  │ Behavior  │
        ├───────────┼───────────┼───────────┤
        │ Philosophy│ Statistics│ Decision  │
        │ Reasoning │ Models    │ Science   │
        └───────────┴───────────┴───────────┘
                       ↓
              Judgment Framework Layer
                       ↓
              Judgment Engine
                       ↓
        Observation → Evidence → Claim → Signal
                                      ↓
                                  Decision
                                      ↓
                                 Outcome
                                      ↓
                              Calibration
                                      ↓
                                Memory
```

---

## 4. Knowledge Layer vs Judgment Engine

### Knowledge Layer — "What frameworks exist?"

A registry of established frameworks from mature disciplines. Not invented by Sulix.

| Domain | Frameworks | Source |
|--------|-----------|--------|
| Finance | Expected Value, Compound Effect, Risk/Reward, Capital Allocation | Academics / Practice |
| Strategy | Porter Five Forces, SWOT, TAM/SAM/SOM, BCG Matrix, Value Chain | Academics / Consulting |
| Psychology | Prospect Theory, Loss Aversion, Confirmation Bias, Status Quo Bias | Kahneman, Tversky |
| Philosophy | First Principles, Falsification (Popper), Occam's Razor | Academic Philosophy |
| Statistics | Bayes Theorem, Factor Models, Brier Score, Significance Testing | Mathematics |
| Decision Sci. | Decision Trees, Cost-Benefit, Multi-Criteria Analysis | Decision Science |

### Judgment Engine — "Which framework should be applied here?"

Not a knowledge encyclopedia. A selection and composition system that:

1. Accepts a Decision Context + Claims + Constraints
2. Selects the appropriate frameworks
3. Applies each framework to the evidence
4. Synthesizes across frameworks
5. Produces a structured recommendation

---

## 5. Judgment Framework Model

```rust
pub struct JudgmentFramework {
    pub id: String,
    pub name: String,
    pub domain: FrameworkDomain,
    pub purpose: String,
    pub assumptions: Vec<String>,
    pub inputs: Vec<String>,
    pub evaluation_method: String,
    pub limitations: Vec<String>,
}

pub enum FrameworkDomain { Finance, Strategy, Psychology, Philosophy, Statistics, DecisionScience }
```

Example:

```yaml
name: Expected Value
domain: Finance
purpose: Quantify decision quality under uncertainty
inputs:
  - probability of success
  - impact if successful
  - downside risk
output: expected_value
limitations:
  - probability estimates are inherently uncertain
  - assumes rational actors
```

---

## 6. Judgment Process

Not Claim → Decision directly. Instead:

```
Claim
  ↓
Framework Selection (context → matching)
  ↓
Analysis (apply frameworks to evidence)
  ↓
Trade-off Evaluation (option A/B/C)
  ↓
Decision Recommendation (with confidence + risks)
```

### Decision Memo Format

Sulix output follows consulting-grade structure, not ChatGPT-style bullet points:

```
1. Executive Summary
2. Situation Analysis (Market / Industry / Tech / Competition)
3. Evidence Review
4. Key Findings (each with evidence + confidence)
5. Strategic Options (A/B/C with risks + conditions)
6. Recommendation (with rationale + mitigation)
7. Risks & Mitigation
8. Action Plan (0-30 / 30-90 / 90-180 days)
9. Confidence & Calibration (unique to Sulix)
```

The **Confidence & Calibration** section is Sulix's differentiator:

```
Recommendation Confidence: 72%
Main uncertainty: Market adoption speed
Calibration history: Previous similar decisions had 68% accuracy
```

---

## 7. Claim Quality Gate

Every claim entering the Judgment Engine must pass a quality gate:

```
Claim Quality Score = EvidenceQuality + Falsifiability + CounterArgument + SourceReliability

Evidence: What supports this claim?
Falsification (Popper): What would invalidate this claim?
Bias Check: Confirmation / Availability / Survivorship / Status Quo
```

No claim enters judgment without a falsification condition.

---

## 8. Knowledge Governance

### Source Tiering

Every framework must declare its source tier:

| Tier | Source | Examples |
|------|--------|----------|
| Tier 1 | Academic / Classic | Kahneman, Popper, Porter, Bayes |
| Tier 2 | Industry Standard | McKinsey, BCG, Deloitte methodologies |
| Tier 3 | Practitioner / Opinion | Must be marked as opinion |

### Prohibited

Sulix will NOT:

- ❌ Invent its own philosophy system
- ❌ Act as an "AI life coach"
- ❌ Generate unbounded general advice
- ❌ Personify as "AI guru"

Sulix WILL:

- ✅ Apply evidence-based frameworks
- ✅ Track calibration of every prediction
- ✅ Clearly label opinion vs analysis
- ✅ Falsify its own claims when outcomes contradict

---

## 9. Implementation Roadmap

### Phase 1 — Foundation (Sprint 5.9 — Complete)

- ✅ Claim Engine (5.9C)
- ✅ Evaluation & Calibration (5.9D)
- ✅ Model Runtime (5.9A)
- ✅ Reasoning Runtime (5.9B)

### Phase 2 — Decision Intelligence (Sprint 6.x)

- Decision Record — formal artifact type
- Outcome Tracking — structured outcome capture
- Reflection Loop — closed feedback from outcome to future decisions
- Decision Memo Generator — consulting-grade output formatting

### Phase 3 — Judgment OS (Post 6.x)

- Framework Registry — knowledge layer with versioning
- Framework Selector — context-aware framework recommendation
- Judgment Calibration — per-framework accuracy tracking
- Decision Graph — full decision lineage visualization

---

## 10. What NOT to Build

| ❌ | Reason |
|----|--------|
| Proprietary philosophy | Professionals will not adopt it |
| AI life coach | Dilutes product focus, trust risk |
| Unbounded general advice | Uncalibratable, unverifiable |
| "AI Guru" persona | Undermines evidence-based positioning |

---

## 11. Competitive Positioning

Sulix's competition is NOT:

- ChatGPT / Perplexity / Claude (general AI)
- Feedly / NewsBlur (RSS readers)
- Copilot / Cursor (coding agents)

Sulix's competition is:

- **Investment research** (think Bloomberg Terminal thesis generation)
- **Strategic consulting** (think McKinsey analysis frameworks)
- **Decision support systems** (think Palantir Foundry for decisions)
- **Personal knowledge systems** (think Obsidian + AI for decision-making)

---

## 12. Product Principle

> Do not generate opinions.  
> Apply structured knowledge frameworks to evidence.  
> Track every judgment.  
> Calibrate against outcomes.  
> Improve over time.

Sulix's moat is built on:

```
Judgment Quality Loop
  = Claim Quality Gate
  × Framework Application
  × Outcome Calibration
  × Historical Accuracy
```

---

## 13. References

- `docs/product/source-governance.md` — Product positioning: RSS is infrastructure, not product
- `crates/claim-engine/` — Claim extraction infrastructure
- `crates/store/src/domain/confidence/calculator.rs` — Factor-based confidence engine
- `crates/store/src/domain/confidence/factors.rs` — ConfidenceFactors model
- `migrations/0041_intelligence_metrics.sql` — Calibration tracking tables
- `Sprint 5.9` — Complete reasoning infrastructure (Model → Reasoning → Claim → Evaluation)
