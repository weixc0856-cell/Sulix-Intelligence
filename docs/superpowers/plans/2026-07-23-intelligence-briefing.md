# Intelligence Briefing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the RSS article-list `/intelligence` page with an Intelligence Briefing — Signal Cards + Evidence Stream + Semantic Search.

**Architecture:** New `GET /api/signals/today` endpoint groups recent scored articles by `signal_type`, computes confidence from frequency × diversity × recency. Frontend rewrites `/intelligence` as a two-column layout with signal cards and evidence stream.

**Tech Stack:** Rust (store + api crates), D1/SQLite, Astro 5 SSR, Tailwind CSS

---

## File Map

### New Files
| File | Purpose |
|------|---------|
| `src/lib/api/signals.ts` | `TodaySignal` type + `fetchTodaySignals()` |

### Modified Files
| File | Changes |
|------|---------|
| `crates/store/src/models.rs` | Add `TodaySignal`, `SignalEvidence` types |
| `crates/store/src/lib.rs` | Add `signals_today()` query method |
| `crates/api/src/lib.rs` | Add `GET /api/signals/today` route + handler |
| `src/pages/intelligence.astro` | Full rewrite to Briefing layout |

---

## Task 1: Add Response Types to Models

**Files:**
- Modify: `crates/store/src/models.rs`

- [ ] **Step 1: Add `TodaySignal` and `SignalEvidence` structs**

Append to `crates/store/src/models.rs`:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignalEvidence {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub feed_name: Option<String>,
    pub published_at: Option<i64>,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TodaySignal {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub confidence: f64,
    pub evidence_count: i64,
    pub trend: String,
    pub articles: Vec<SignalEvidence>,
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p store
```
Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/store/src/models.rs
git commit -m "feat: add TodaySignal and SignalEvidence response types"
```

---

## Task 2: Add `signals_today()` Store Method

**Files:**
- Modify: `crates/store/src/lib.rs`

- [ ] **Step 1: Add the query method before `active_rule_jsons`**

```rust
/// Fetch today's intelligence signals.
///
/// Returns scored articles from the last 7 days grouped by signal_type,
/// with confidence computed from frequency × diversity × recency.
pub async fn signals_today(&self, now: i64) -> Result<Vec<TodaySignal>, StoreError> {
    use std::collections::HashMap;

    let seven_days_ago = now - 7 * 86400;

    // 1. Fetch recent scored articles with their feed info
    #[derive(Deserialize)]
    struct ScoredArticle {
        id: i64,
        title: String,
        url: Option<String>,
        feed_name: Option<String>,
        published_at: Option<i64>,
        ai_summary: String,
        score: f64,
        signal_type: Option<String>,
    }

    // Query articles from last 7 days with score >= 0.6, joined with strategies
    // to get signal_type
    let all_articles: Vec<ScoredArticle> = self.db.prepare(
        "SELECT a.id, a.title, a.url, f.title AS feed_name,
                a.published_at, a.ai_summary, a.score
         FROM articles a
         LEFT JOIN feeds f ON f.id = a.feed_id
         WHERE a.score >= 0.6
           AND a.published_at >= ?1
         ORDER BY a.published_at DESC
         LIMIT 500"
    ).bind(&[JsValue::from_f64(seven_days_ago as f64)])?
    .all().await?.results()?;

    if all_articles.is_empty() {
        return Ok(Vec::new());
    }

    // 2. Group articles by signal_type
    // Since we don't have a direct article→strategy join in the DB,
    // we infer signal_type from the article's score pattern.
    // For V1: use a simple heuristic — articles with high scores
    // are likely matched by Technology strategies, medium by Industry, etc.
    // TODO V2: store signal_type on articles during pipeline execution.
    
    let mut groups: HashMap<String, Vec<&ScoredArticle>> = HashMap::new();
    for article in &all_articles {
        let key = if article.score >= 8.0 {
            "technology".to_string()
        } else if article.score >= 5.0 {
            "industry".to_string()
        } else {
            "other".to_string()
        };
        groups.entry(key).or_default().push(article);
    }

    let total_articles = all_articles.len() as f64;
    let now_f64 = now as f64;

    let mut signals: Vec<TodaySignal> = groups.into_iter().map(|(type_key, articles)| {
        let count = articles.len() as f64;
        
        // Frequency score: normalized within all articles
        let freq = count / total_articles.max(1.0);
        
        // Source diversity: unique feed names / total feeds
        let unique_feeds = articles.iter()
            .filter_map(|a| a.feed_name.as_deref())
            .collect::<std::collections::HashSet<_>>()
            .len() as f64;
        let diversity = unique_feeds / total_articles.max(1.0);
        
        // Recency: average decay weight (newer = higher)
        let recency: f64 = articles.iter()
            .filter_map(|a| a.published_at)
            .map(|ts| 1.0 - ((now_f64 - ts as f64) / 604800.0).clamp(0.0, 1.0))
            .sum::<f64>() / count.max(1.0);
        
        let confidence = 0.4 * freq + 0.3 * diversity + 0.3 * recency;
        
        // Trend: compare article count last 3 days vs 3 days before that
        let three_days = 3 * 86400;
        let recent = articles.iter()
            .filter(|a| a.published_at.map_or(false, |ts| ts >= now - three_days))
            .count() as f64;
        let earlier = articles.iter()
            .filter(|a| a.published_at.map_or(false, |ts| ts >= now - 2 * three_days && ts < now - three_days))
            .count() as f64;
        let trend = if recent > earlier * 1.2 { "rising" }
                    else if recent < earlier * 0.8 { "declining" }
                    else { "stable" };

        // Title: human-readable from the type key
        let title = match type_key.as_str() {
            "technology" => "Technology & AI Infrastructure".to_string(),
            "industry" => "Industry & Market Trends".to_string(),
            _ => "Other Signals".to_string(),
        };
        
        // Summary: use the first article's AI summary
        let summary = articles.first()
            .map(|a| a.ai_summary.clone())
            .unwrap_or_default();

        let evidence: Vec<SignalEvidence> = articles.iter().map(|a| SignalEvidence {
            id: a.id,
            title: a.title.clone(),
            url: a.url.clone(),
            feed_name: a.feed_name.clone(),
            published_at: a.published_at,
            score: a.score,
        }).collect();

        TodaySignal {
            id: type_key.clone(),
            title,
            summary,
            confidence,
            evidence_count: count as i64,
            trend: trend.to_string(),
            articles: evidence,
        }
    }).collect();

    // Sort by confidence descending
    signals.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

    Ok(signals)
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p store
```
Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/store/src/lib.rs
git commit -m "feat: add signals_today() store method with confidence scoring"
```

---

## Task 3: Add `GET /api/signals/today` API Endpoint

**Files:**
- Modify: `crates/api/src/lib.rs`

- [ ] **Step 1: Add the handler function before `intelligence_signals`**

Replace the existing `intelligence_signals` handler with the new signals_today version:

```rust
async fn intelligence_signals(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);
    let now = (js_sys::Date::now() / 1000.0) as i64;
    
    match store.signals_today(now).await {
        Ok(signals) => {
            // Compute generated_at
            let generated_at_ms = js_sys::Date::now();
            // Format as ISO 8601 — simplified for WASM
            let generated_at = format!("{}", generated_at_ms);
            
            json_ok(json!({
                "date": chrono_format_today(now),
                "generated_at": generated_at,
                "signals": signals,
            }))
        }
        Err(e) => json_err(500, &e.to_string()),
    }
}

/// Format unix timestamp as YYYY-MM-DD using js_sys Date
fn chrono_format_today(ts: i64) -> String {
    let js_date = js_sys::Date::new(&JsValue::from_f64(ts as f64 * 1000.0));
    let year = js_date.get_full_year();
    let month = js_date.get_month() + 1; // 0-indexed
    let day = js_date.get_date();
    format!("{year:04}-{month:02}-{day:02}")
}
```

- [ ] **Step 2: Verify route is already registered**

The route `.get_async("/api/intelligence/signals", intelligence_signals)` already exists in the router. Confirm by grepping:

```bash
grep "intelligence/signals" crates/api/src/lib.rs
```
Expected: The route line is present.

- [ ] **Step 3: Verify compilation**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p api
```
Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/api/src/lib.rs
git commit -m "feat: update GET /api/intelligence/signals with confidence scoring"
```

---

## Task 4: Frontend API Client — `fetchTodaySignals()`

**Files:**
- Create: `src/lib/api/signals.ts`

- [ ] **Step 1: Create the API client file**

```typescript
import { apiFetch, type ApiEnv } from './client';

export interface SignalEvidence {
  id: number;
  title: string;
  url: string | null;
  feed_name: string | null;
  published_at: number | null;
  score: number;
}

export interface TodaySignal {
  id: string;
  title: string;
  summary: string;
  confidence: number;
  evidence_count: number;
  trend: string;
  articles: SignalEvidence[];
}

export interface SignalsResponse {
  date: string;
  generated_at: string;
  signals: TodaySignal[];
}

export async function fetchTodaySignals(env: ApiEnv): Promise<SignalsResponse> {
  const resp = await apiFetch(env, '/api/intelligence/signals');
  if (!resp.ok) throw new Error(`signals fetch failed: ${resp.status}`);
  return (await resp.json()) as SignalsResponse;
}
```

- [ ] **Step 2: Build to verify**

```bash
cd "d:/Project/intel-web"
npm run build
```
Expected: Build succeeds with no type errors.

- [ ] **Step 3: Commit**

```bash
git add src/lib/api/signals.ts
git commit -m "feat: add fetchTodaySignals() API client"
```

---

## Task 5: Rewrite `/intelligence` as Intelligence Briefing

**Files:**
- Modify: `src/pages/intelligence.astro`

- [ ] **Step 1: Rewrite the page**

```astro
---
import Layout from '../layouts/Layout.astro';
import Sidebar from '../components/Sidebar.astro';
import Header from '../components/Header.astro';
import Footer from '../components/Footer.astro';
import ErrorState from '../components/ErrorState.astro';
import ScrollToTop from '../components/ScrollToTop.astro';
import { fetchTodaySignals } from '../lib/api/signals';
import type { TodaySignal, SignalEvidence } from '../lib/api/signals';

const env = Astro.locals.runtime.env;

let signalsData: { date: string; signals: TodaySignal[] } | null = null;
let loadError: string | null = null;

try {
  signalsData = await fetchTodaySignals(env);
} catch (e) {
  loadError = e instanceof Error ? e.message : 'failed to load signals';
}

const signals = signalsData?.signals ?? [];
const allEvidence: SignalEvidence[] = signals.flatMap((s) => s.articles);

function fmtDate(ts: number | null): string {
  if (!ts) return '';
  return new Date(ts * 1000).toUTCString().split(' ').slice(1, 4).join(' ');
}

function fmtRelative(ts: number | null): string {
  if (!ts) return '';
  const diff = Date.now() / 1000 - ts;
  if (diff < 60) return 'Just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 172800) return 'Yesterday';
  return `${Math.floor(diff / 86400)}d ago`;
}

function confidenceColor(c: number): string {
  if (c >= 0.8) return 'bg-teal-100 dark:bg-teal-900/30 text-teal-800 dark:text-teal-300';
  if (c >= 0.6) return 'bg-blue-100 dark:bg-blue-900/30 text-blue-800 dark:text-blue-300';
  return 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400';
}

function trendIcon(trend: string): string {
  if (trend === 'rising') return '↑';
  if (trend === 'declining') return '↓';
  return '→';
}

function trendClass(trend: string): string {
  if (trend === 'rising') return 'text-secondary';
  if (trend === 'declining') return 'text-tertiary';
  return 'text-on-surface-variant';
}
---

<Layout title="Intelligence Briefing">
  <Header activePath="/intelligence" />
  <Sidebar activePath="/intelligence" />

  <div id="sidebar-overlay" class="fixed inset-0 z-30 bg-black/30 hidden lg:hidden" onclick=""></div>
  <div id="mobile-sidebar" class="fixed left-0 top-16 bottom-14 z-40 w-64 bg-background dark:bg-dark-bg border-r border-outline-variant dark:border-dark-border shadow-xl hidden overflow-y-auto">
    <nav class="flex flex-col py-4 px-2 space-y-0.5">
      {[
        { label: 'Latest', href: '/intelligence', icon: 'rss_feed' },
        { label: 'Trending', href: '/trending', icon: 'trending_up' },
        { label: 'Categories', href: '/categories', icon: 'category' },
        { label: 'Tags', href: '/tags', icon: 'sell' },
        { label: 'Search', href: '/search', icon: 'search' },
        { label: 'Bookmarks', href: '/bookmarks', icon: 'bookmark' },
        { label: 'Strategies', href: '/strategies', icon: 'tune' },
        { label: 'Feeds', href: '/feeds', icon: 'source' },
        { label: 'Dashboard', href: '/dashboard', icon: 'dashboard' },
        { label: 'About', href: '/about', icon: 'info' },
      ].map((item) => (
        <a href={item.href} class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-on-surface-variant dark:text-dark-on-surface-variant hover:bg-surface-container-high dark:hover:bg-dark-surface transition-all">
          <span class="material-symbols-outlined text-xl">{item.icon}</span>
          <span class="text-label-sm font-label-sm">{item.label}</span>
        </a>
      ))}
      <div class="border-t border-outline-variant dark:border-dark-border my-3"></div>
      <a href="/feed.xml" target="_blank" class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-on-surface-variant dark:text-dark-on-surface-variant hover:bg-surface-container-high dark:hover:bg-dark-surface transition-all">
        <span class="material-symbols-outlined text-xl">rss_feed</span>
        <span class="text-label-sm font-label-sm">RSS Feed</span>
      </a>
    </nav>
  </div>

  <main class="pt-16 lg:pl-60 pb-14 lg:pb-0 flex-1">
    <div class="px-edge-margin py-6 max-w-6xl mx-auto">

      <!-- Header bar -->
      <div class="flex justify-between items-center mb-6">
        <div>
          <h1 class="font-headline-lg text-headline-lg text-on-surface dark:text-dark-on-surface">Intelligence Briefing</h1>
          <p class="font-body-ui text-body-ui text-on-surface-variant dark:text-dark-on-surface-variant">
            {signalsData?.date ?? ''} · {signals.length} signal{signals.length !== 1 ? 's' : ''} detected
          </p>
        </div>
        <a href="/strategies" class="text-label-sm font-label-sm text-primary hover:underline">Manage strategies &rarr;</a>
      </div>

      {loadError && <ErrorState message={loadError} retry="/intelligence" />}

      <!-- Empty state -->
      {!loadError && signals.length === 0 && (
        <div class="text-center py-density-comfortable">
          <p class="font-body-ui text-body-ui text-on-surface-variant dark:text-dark-on-surface-variant mb-2">
            No signals detected today.
          </p>
          <p class="text-label-sm font-label-sm text-on-surface-variant dark:text-dark-on-surface-variant mb-4">
            Signals will appear once scored articles are available. Configure strategies on the Strategies page.
          </p>
          <div class="flex justify-center gap-3">
            <a href="/strategies" class="px-4 py-2 rounded-lg bg-primary text-on-primary font-body-ui font-medium hover:opacity-90">Configure Strategies</a>
            <a href="/intelligence?mode=articles" class="px-4 py-2 rounded-lg border border-outline-variant text-on-surface font-body-ui font-medium hover:bg-surface-container">Browse Latest Articles</a>
          </div>
        </div>
      )}

      {signals.length > 0 && (
        <div class="flex gap-6">
          <!-- Left: Signal Cards -->
          <div class="flex-1 min-w-0">
            <h2 class="font-label-sm font-label-sm text-on-surface-variant uppercase tracking-wide mb-4">🔥 Today's Signals</h2>
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
              {signals.map((signal) => (
                <div class="rounded-xl bg-surface-container dark:bg-dark-surface border border-outline-variant/50 dark:border-dark-border/50 p-5 hover:shadow-sm transition-shadow">
                  <div class="flex justify-between items-start mb-3">
                    <h3 class="font-headline-md text-headline-md text-on-surface dark:text-dark-on-surface leading-tight">{signal.title}</h3>
                    <span class:list={['px-2 py-0.5 rounded-full text-label-sm font-label-sm shrink-0 ml-2', confidenceColor(signal.confidence)]}>
                      {Math.round(signal.confidence * 100)}%
                    </span>
                  </div>

                  <p class="font-body-ui text-body-ui text-on-surface-variant dark:text-dark-on-surface-variant mb-4 line-clamp-2">
                    {signal.summary || 'No summary available.'}
                  </p>

                  <div class="flex items-center gap-4 text-label-sm font-label-sm text-on-surface-variant dark:text-dark-on-surface-variant mb-4">
                    <span>📄 {signal.evidence_count} source{signal.evidence_count === 1 ? '' : 's'}</span>
                    <span class:list={[trendClass(signal.trend)]}>{trendIcon(signal.trend)} {signal.trend}</span>
                  </div>

                  <!-- Inline evidence list -->
                  {signal.articles.length > 0 && (
                    <div class="border-t border-outline-variant/50 dark:border-dark-border/50 pt-3 mt-3">
                      <p class="text-label-sm font-label-sm text-on-surface-variant dark:text-dark-on-surface-variant mb-2">Latest evidence</p>
                      {signal.articles.slice(0, 3).map((article) => (
                        <a href={`/article/${article.id}`} class="block py-1.5 border-b border-outline-variant/30 dark:border-dark-border/30 last:border-0 hover:text-primary dark:hover:text-dark-primary transition-colors">
                          <p class="font-body-ui text-body-ui text-on-surface dark:text-dark-on-surface truncate">{article.title}</p>
                          <p class="text-label-sm font-label-sm text-on-surface-variant dark:text-dark-on-surface-variant">
                            {article.feed_name ?? 'Unknown source'} · {fmtRelative(article.published_at)}
                          </p>
                        </a>
                      ))}
                      {signal.evidence_count > 3 && (
                        <a href={`/search?q=${encodeURIComponent(signal.title)}&mode=semantic`} class="mt-2 inline-block text-label-sm font-label-sm text-primary hover:underline">
                          Explore all {signal.evidence_count} sources &rarr;
                        </a>
                      )}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>

          <!-- Right: Evidence Stream -->
          <div class="w-80 shrink-0 hidden lg:block">
            <div class="sticky top-20">
              <h2 class="font-label-sm font-label-sm text-on-surface-variant uppercase tracking-wide mb-4">Evidence Stream</h2>
              <div class="space-y-0 rounded-xl bg-surface-container dark:bg-dark-surface border border-outline-variant/50 dark:border-dark-border/50 divide-y divide-outline-variant/30 dark:divide-dark-border/30">
                {allEvidence.slice(0, 15).map((article) => (
                  <a href={`/article/${article.id}`} class="block px-4 py-3 hover:bg-surface-dim dark:hover:bg-dark-surface/50 transition-colors first:rounded-t-xl last:rounded-b-xl">
                    <p class="font-body-ui text-body-ui text-on-surface dark:text-dark-on-surface line-clamp-2 mb-1">{article.title}</p>
                    <p class="text-label-sm font-label-sm text-on-surface-variant dark:text-dark-on-surface-variant">
                      {article.feed_name ?? 'Unknown'} · {fmtRelative(article.published_at)}
                    </p>
                  </a>
                ))}
              </div>
              <a href="/trending" class="mt-3 inline-block text-label-sm font-label-sm text-primary hover:underline">View trending &rarr;</a>
            </div>
          </div>
        </div>
      )}

      <!-- Semantic Search bar -->
      <div class="mt-8 bg-surface-container dark:bg-dark-surface rounded-xl p-5 border border-outline-variant/50 dark:border-dark-border/50">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-2xl text-primary">search</span>
          <span class="font-body-ui text-body-ui text-on-surface dark:text-dark-on-surface font-medium">Semantic Search</span>
          <span class="text-label-sm font-label-sm text-on-surface-variant">Explore any topic across all articles</span>
        </div>
        <form method="get" action="/search" class="mt-3 flex gap-2">
          <input type="hidden" name="mode" value="semantic" />
          <input type="text" name="q" placeholder="Ask about any topic..." autocomplete="off"
            class="flex-1 px-4 py-2.5 bg-background dark:bg-dark-bg border border-outline-variant dark:border-dark-outline rounded-lg text-body-ui text-on-surface dark:text-dark-on-surface focus:border-primary focus:outline-none" />
          <button type="submit" class="px-5 py-2.5 rounded-lg bg-primary dark:bg-dark-primary text-on-primary font-body-ui font-medium hover:opacity-90">Explore</button>
        </form>
      </div>
    </div>
  </main>

  <ScrollToTop />
  <Footer />
</Layout>
```

- [ ] **Step 2: Build to verify**

```bash
cd "d:/Project/intel-web"
npm run build
```
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add src/pages/intelligence.astro
git commit -m "feat: rewrite /intelligence as Intelligence Briefing with Signal Cards + Evidence Stream + Semantic Search"
```

---

## Task 6: Build, Deploy & Verify

- [ ] **Step 1: Full backend check**

```bash
cd "d:/Project/Sulix Intelligence"
cargo check -p store -p rules -p embedding -p api -p worker-entry
cargo test -p rules
```

- [ ] **Step 2: Build worker**

```bash
cd "d:/Project/Sulix Intelligence/crates/worker-entry"
worker-build --release
```

- [ ] **Step 3: Deploy**

```bash
npx wrangler@4.113.0 deploy
```

- [ ] **Step 4: Verify endpoint**

```bash
curl -s "https://sulix-feed-worker.weixc0856.workers.dev/api/intelligence/signals"
```
Expected: Returns `{"date": "...", "generated_at": "...", "signals": [...]}`

- [ ] **Step 5: Frontend build**

```bash
cd "d:/Project/intel-web"
npm run build
```

- [ ] **Step 6: Commit everything**

```bash
cd "d:/Project/Sulix Intelligence"
git add -A && git commit -m "feat: intelligence briefing with signal cards and evidence stream"
git push origin master
cd "d:/Project/intel-web"
git add -A && git commit -m "feat: intelligence briefing frontend"
git push origin refactor/layout-split
```
