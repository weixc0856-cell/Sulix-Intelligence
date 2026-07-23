# Article Reader V1 — Knowledge Consumption Layer

> **Status:** Approved  
> **Date:** 2026-07-23  

---

## Context

Article detail page currently strips HTML to plain text, losing code blocks, images, and tables. No reading progress tracking. This spec adds three enhancements: safe rich content rendering, reading progress persistence, and UX polish.

---

## Components

### 1. Rich Content Render

Replace current `stripHtml()` with a safe HTML sanitizer (whitelist-based):

| Allowed | Removed |
|---------|---------|
| `p, h1-h6, pre, code, blockquote` | `script, style` |
| `ul, ol, li, table, thead, tbody, tr, td` | `iframe, object, embed` |
| `img (src/alt only), a (target=_blank)` | `form, input, svg` |
| `strong, em, br, hr` | `onclick, onerror, style` |

Images: max-width 100%, remove event handlers.  
Code blocks: dark background, monospace, horizontal scroll.  
Links: `target="_blank" rel="noopener noreferrer"`.  
Tables: wrapper with overflow-x: auto.

### 2. Reading Progress

- Fixed top progress bar (3px, gradient, z-50)
- Updates via scroll `IntersectionObserver` on article container
- Stores to localStorage: `{ "article:{id}": { progress, completed, updated_at } }`
- Auto-completed when scroll > 90%
- ArticleCard shows `✓ Read` badge for completed articles

### 3. UX Polish

- Container: `max-w-2xl mx-auto`
- Typography: `font-body-reading`, `line-height: 1.8`, `font-size: 1.0625rem`
- Code blocks: dark background, rounded, padding
- Reading time: accurate based on R2 content length (220 wpm)
- Scroll-to-top: existing component already available

---

## Implementation Order

| Step | Description | Files | Effort |
|------|-------------|-------|--------|
| 1 | Safe HTML sanitizer + article content render | `article/[id].astro` | Medium |
| 2 | Reading progress bar + localStorage persistence | `article/[id].astro` + client script | Medium |
| 3 | UX polish refresh | CSS tweaks in `article/[id].astro` | Small |
| 4 | Verify | `npm run build` | Small |
