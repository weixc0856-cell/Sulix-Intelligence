# ADR-004: Astro Server Mode + Service Binding

## Status

Accepted (2026-07)

## Context

The frontend needs to display articles from the D1 database. The previous V1
frontend used static content collections (MDX files committed to git), which
required a rebuild every time new articles arrived.

For V2, every page view should show fresh data without a rebuild. Options:
- **Static site with rebuild trigger**: rebuild on cron (slow, complex CI)
- **Client-side fetch from API**: works but loses server-side rendering (SEO, performance)
- **Astro server mode + service binding**: pages render server-side, calling the API Worker directly

## Decision

Use Astro 5 in server mode (`output: 'server'`) with `@astrojs/cloudflare`
adapter, talking to the API Worker via a Cloudflare `[[services]]` binding.

Data flow:
```
Browser → Astro Worker (server-renders page)
  → env.API.fetch("/api/articles/latest")  (service binding, no public HTTP)
    → API Worker → D1 query → JSON response
      → Astro page renders HTML → Browser
```

## Consequences

Positive:
- Fresh data on every page load — no rebuild needed for new articles
- Server-side rendering (no client-side fetch waterfall)
- Service binding is private (no public API exposure needed)
- Fallback to `API_BASE_URL` for local dev

Negative:
- Server mode has higher per-request cost than static (negligible at MVP scale)
- Service binding requires both Workers under the same Cloudflare account
- `@astrojs/cloudflare` Runtime types are adapter-version-specific
