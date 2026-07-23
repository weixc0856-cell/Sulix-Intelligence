# ADR-003: Worker-Queue-Fetch Pipeline

## Status

Accepted (2026-07)

## Context

The system needs to fetch RSS/Atom feeds on a recurring schedule. The naive
approach — a synchronous cron loop that fetches all feeds sequentially — risks
hitting the Workers CPU limit as feed count grows.

Alternative approaches:
- **Sequential cron loop**: simple but each 1-feed failure delays all others; total time = sum of all fetch times
- **Promise.all in cron**: races all feeds but a single slow/blocked feed ties up the entire invocation
- **Queue-based**: cron enqueues one message per feed, consumer processes them independently with retry

## Decision

Use Cloudflare Queues as an async buffer between the cron producer and feed consumer.

Architecture:
1. **Cron trigger** (`*/30 * * * *`): reads active feeds from D1, sends one `FetchJob` per feed to `FETCH_QUEUE`
2. **Queue consumer**: processes messages in batches (max 10), each with independent ack/retry
3. **Dead letter queue**: messages that fail after 3 retries go to a DLQ for manual inspection

The cron handler stays intentionally cheap — it only reads feeds and enqueues,
so it will never time out regardless of feed count.

## Consequences

Positive:
- Per-feed isolation: one slow/broken feed doesn't delay others
- Built-in retry with exponential backoff (no custom code needed)
- Dead letter queue captures persistent failures
- Scales to hundreds of feeds without changing the architecture

Negative:
- Queue consumer doesn't run the rules engine or AI pipeline yet (TBD)
- Additional latency: each fetch takes at least one queue round-trip
- Queue consumer must be idempotent (re-delivery is possible)
