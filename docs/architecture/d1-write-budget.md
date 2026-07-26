# D1 Write Budget Contract v1

**Status:** Frozen at Sprint 5.10

**Purpose:** Define per-module write budgets and Write Amplification Ratio (WAR) targets to prevent D1 write quota exhaustion.

## Budget per 30-min Cron Cycle

| Module | Budget | Current (est.) | Target | Notes |
|--------|--------|---------------|--------|-------|
| RSS Ingestion | < 150 | ~265 | < 150 | Articles, entities, relations |
| Entity Extraction | < 80 | ~180 | < 80 | Sprint 5.10: Top-5 cap + UPSERT |
| Signal Engine | < 50 | ~94 | < 50 | Sprint 5.10: fingerprint skip |
| Outbox + Event Index | < 30 | ~40 | ~30 | Legacy signal_events removed |
| Reflection + Memory | < 20 | ~18 | ~18 | KV-guarded, low frequency |
| **Total** | **< 300/cycle** | **~446** | **< 300** | Daily: < 14k (within 100k limit) |

## Write Amplification Ratio (WAR)

```
WAR = D1 writes / logical operations
Target: WAR < 3.0
```

Measurement points:
- **Ingestion**: 1 logical article → WAR counts articles + entities + relations
- **Signal**: 1 signal candidate → WAR counts upsert + instance + event
- **Decision**: 1 decision → WAR counts create + outcome + evaluation

## Enforcement

- Cron cycle outputs `[D1 Budget] total=NNN/WAR=X.X` report
- WAR > 3.0 triggers warning log
- Budget regression test in CI

## History

| Sprint | Change | WAR |
|--------|--------|-----|
| 4.8 | Initial amplification control | ~8.0 |
| 5.10 | Entity UPSERT + Top-5 + signal fingerprint | ~3.5 |
| Target | — | < 3.0 |
