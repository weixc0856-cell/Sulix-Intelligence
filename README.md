<p align="center">
  <a href="README.md">🇬🇧 English</a>
</p>

# Sulix Intelligence

> **Fully Automated Cognitive Engine — Personal Strategy OS for solo entrepreneurs.**

Sulix Intelligence transforms raw signals into structured **strategic memory** — Signal → Assessment → Decision → Outcome. Not "what happened", but "does this change my decision for the next 6 months."

```
Observation (RSS/USPTO/Reddit)
    ↓
SignalClassificationStep — Fast Path (rule) | Slow Path (LLM)
    ↓
ThesisGenerationStep — match existing | LLM generate new
    ↓
DecisionMappingStep — RuleEngine + optional LlmJudge
    ↓
PipelineStats + MDX Output
```

## Architecture

```
crates/                    cargo run -p sulix-cli
├── sulix-contract/       ← Layer-boundary types (Observation/Signal/Thesis/Decision)
├── sulix-config/         ← TOML config loading
├── sulix-llm/            ← LLM provider (client/retry/api/parser/dispatch/audit)
├── sulix-intelligence/   ← Cognitive pipeline (PipelineStep trait + 3 steps)
├── sulix-observation/    ← Source adapters (RSS/Reddit/USPTO)
└── sulix-cli/            ← CLI entry points (sulix-cli + intel-pipeline)
```

## Three-Repository Architecture

| Repo | Responsibility | Tech Stack |
|------|--------------|------------|
| **sulix-engine** ← this repo | Data Acquisition, Analysis, Strategic Memory | Rust + DeepSeek API |
| [sulix-web](https://github.com/weixc0856-cell/Intel-Web) | UI Shell, Navigation, UX | Astro + Tailwind |
| **sulix-docs** | Product Decisions, Architecture, ADR | Obsidian Markdown |

## Products

| Product | Purpose | Price |
|---------|---------|-------|
| **News Layer** | User acquisition | $0 |
| **Research Layer** | Revenue | $99-$4999 |
| **Memory Layer** | Moat | Private |

## Quick Start

```bash
# 1. Build
git clone https://github.com/weixc0856-cell/Sulix-Intelligence.git
cd Sulix-Intelligence
cp config.example.toml config.toml
cargo build --release -p sulix-cli

# 2. Run (requires DEEPSEEK_API_KEY)
export DEEPSEEK_API_KEY="sk-..."
cargo run -p sulix-cli --release

# 3. Pipeline debug (standalone, with mock input)
cargo run --bin intel-pipeline -- --input tests/fixtures/observation.json
```

## Pipeline

```
crates/sulix-observation/     — Source Acquisition (RSS / USPTO / Reddit)
    ↓  RawSignal → Article → Observation
    ↓
crates/sulix-cli/src/agent/  — Signal Agent: fetch → SQLite dedup → enrich
    ↓  Vec<Observation>
    ↓
crates/sulix-intelligence/
    │
    ├── SignalClassificationStep  — Fast Path (rule) | Slow Path (LLM batch)
    │   observation → Signal { importance, domain, category, why }
    │
    ├── ThesisGenerationStep     — match existing | LLM generate new
    │   signal → Thesis { claim, confidence, evidence, status }
    │
    └── DecisionMappingStep      — RuleEngine + optional LlmJudge
        thesis → Decision { action, horizon, confidence, reasoning }
    ↓
    Post-processing: calibration (LLM question) + summary (rule-based)
    ↓
    MDX Output: thesis/{slug}.md + decision/{slug}.md
```

## Code Structure

```
crates/
├── sulix-contract/     — Layer-boundary types (7 models: Observation/Signal/Thesis/Decision/Theme/Belief/Reflection)
├── sulix-config/       — TOML config loading (LlmConfig, SourceConfig, OutputConfig, IntelligenceConfig)
├── sulix-llm/          — LLM provider (client/retry/api/parser/dispatch/audit — 7 sub-modules)
├── sulix-intelligence/ — Cognitive pipeline (PipelineStep<Observation, Signal, Thesis, Decision>)
│   ├── step.rs          — PipelineStep trait + PipelineStats
│   ├── signal_classification.rs — Fast/Slow dual path
│   ├── thesis_generation.rs    — Title overlap + LLM generation
│   ├── decision_mapping.rs     — RuleEngine + smoothing + stability
│   ├── loader.rs        — Memory DB bridge + load_last_decisions()
│   └── output.rs        — MDX rendering
├── sulix-observation/  — Source adapters (RSS/Reddit/USPTO) + fetcher + client cache
└── sulix-cli/          — Entry points: `cargo run -p sulix-cli` | `cargo run --bin intel-pipeline`
    ├── src/main.rs              — Production pipeline entry
    ├── src/bin/intel_pipeline.rs — Standalone pipeline debugger
    ├── src/agent/signal.rs       — Signal Agent (fetch → dedup)
    ├── src/db.rs                — SQLite dedup database
    └── src/entity.rs            — EntitySanctionDb (OpenCTI-like)
```

## Key Architecture Patterns

- **PipelineStep trait**: Unified abstract interface for all 3 steps (analogous to ripgrep's `Matcher` trait)
- **Builder pattern**: Each step has `XxxStepBuilder` with `build()` (analogous to ripgrep's `SearcherBuilder`)
- **Fast/Slow dual path**: Rule-based (zero LLM) or LLM-based classification (analogous to ripgrep's `is_line_by_line_fast`)
- **PipelineStats**: Run timing + item counts per step + LLM audit counters

## Configuration

| Section | Purpose |
|---------|---------|
| `[llm]` | API key, model, endpoint |
| `[[sources]]` | Data sources (name, URL, category, layer, score) |
| `[prompts]` | System prompts for each analysis stage |
| `[output]` | Output paths (vault_path, mdx_dir, frontend_public_dir) |
| `[storage]` | data_dir for persistent state |
| `[r2]` | Cloudflare R2 config (bucket, endpoint, public_url) |

## Deployment

### CI Pipeline (GitHub Actions)

```yaml
# .github/workflows/cron_brief.yml
# Daily: cargo run -p sulix-cli --release → R2 → sulix-web build → CF Pages
```

Secrets required:
- `DEEPSEEK_API_KEY` — LLM provider
- `R2_ACCESS_KEY_ID` / `R2_SECRET_ACCESS_KEY` / `R2_ENDPOINT` — R2 storage
- `CLOUDFLARE_API_TOKEN` / `CLOUDFLARE_ACCOUNT_ID` — Pages deploy

## License

MIT
