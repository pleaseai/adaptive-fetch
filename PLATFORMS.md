# Supported platforms (planned)

> 🚧 **Early access.** The Phase 0 router fetches **Reddit** (`.rss`) end-to-end
> today; every other host still returns an honest "not implemented" result. This
> page is the coverage map: which platforms each milestone (M1–M6) unlocks, ported
> from [`fivetaku/insane-search`](https://github.com/fivetaku/insane-search)'s
> `PLATFORMS.md`. Each row's status flips from 🚧 planned to ✅ live as the owning
> milestone lands. Track progress in the
> [design RFC §8](docs/rfcs/0001-adaptive-fetch.md#8-implementation-milestones-proposed-after-this-design-is-approved).

Most sites need **no explicit entry** — the generic Phase 0→3 adaptive engine
handles them once M1–M2 land. This page lists the *special* endpoints (the Phase 0
official-API router, M3) and the milestone that unlocks each route.

**Site knowledge lives only in docs and runtime config** — this file,
[`skills/adaptive-fetch/url_presets.toml`](skills/adaptive-fetch/url_presets.toml),
and the sanctioned `phase0` module. The engine core stays site-agnostic; no other
module names a platform (RFC [§4.3](docs/rfcs/0001-adaptive-fetch.md) / §7).

## Status legend

| Marker | Meaning |
|--------|---------|
| ✅ | Live on `main` |
| 🚧 M`x` | Planned — lands in milestone M`x` |
| 🚧 M3+ | Phase 0 router is extensible; planned after the M3 core routers (Reddit / X / YouTube) |

## Generic bypass — no per-site entry — 🚧 M1–M2

The adaptive engine reaches most blocked sites with no platform knowledge at all.
Coupang, LinkedIn, Medium, Substack, most forums, and any site exposing `/rss` or
`/feed` flow through here.

| Capability | Planned mechanism | Milestone |
|------------|-------------------|-----------|
| Probe + 4-layer validation | `transport.rs` + `validators.rs` (HTTP 200 is the *start* of inspection) | 🚧 M1 |
| SSRF-safe transport | `safety.rs` block-list (private / loopback / link-local / metadata) | 🚧 M1 |
| Browser TLS-fingerprint impersonation | **`rquest`, in-process** (no `curl_cffi`, no Python) | 🚧 M2 |
| WAF detection → ranked priors | `waf_detector.rs` + `waf_profiles.yaml` | 🚧 M2 |
| Diversity grid + exhaustive failure gate (R6) | `scheduler.rs` (session pool, warmup, jitter) | 🚧 M2 |
| Generic URL rewrites | `url_transforms.rs` | 🚧 M2 |

## Platform-specific APIs — 🚧 M3 (Phase 0 router)

`phase0.rs` is the **only** engine module allowed to name hosts. It tries the
official no-auth endpoint *before* the generic grid.

| Platform | Planned route | Reference | Status |
|----------|---------------|-----------|--------|
| Reddit | `.rss` feed via a **plain** client (browser impersonation trips its anti-bot; the unauth `.json` is WAF-gated) | `json-api.md` | ✅ |
| X/Twitter | single tweet → `cdn.syndication.twimg.com/tweet-result` + oEmbed · timeline → `syndication.twitter.com` · keyword → WebSearch → tweet-result | `twitter.md` | 🚧 M3 |
| Bluesky | AT Protocol (`public.api.bsky.app/xrpc/…`) | `public-api.md` | 🚧 M3+ |
| Mastodon | Per-instance public API | `public-api.md` | 🚧 M3+ |
| Hacker News | Firebase API + Algolia Search (`hn.algolia.com/api/v1/search`) | `json-api.md` | 🚧 M3+ |
| Stack Overflow | Stack Exchange API v2.3 | `public-api.md` | 🚧 M3+ |
| Lobste.rs / V2EX / dev.to | Public JSON APIs | `json-api.md` | 🚧 M3+ |

## Media (CLI tool required) — 🚧 M3

| Platform | Planned route | Reference | Status |
|----------|---------------|-----------|--------|
| YouTube / Vimeo / Twitch / TikTok / SoundCloud + 1,853 others | `yt-dlp --dump-json` subprocess | `media.md` | 🚧 M3 |

## Academic & registry — 🚧 M3+

| Platform | Planned route | Reference | Status |
|----------|---------------|-----------|--------|
| arXiv | Atom API | `public-api.md` | 🚧 M3+ |
| CrossRef | REST API | `public-api.md` | 🚧 M3+ |
| Wikipedia | REST API | `json-api.md` | 🚧 M3+ |
| OpenLibrary | JSON API | `public-api.md` | 🚧 M3+ |
| GitHub | `gh` CLI / REST API | `public-api.md` | 🚧 M3+ |
| npm / PyPI | Registry API | `json-api.md` | 🚧 M3+ |
| Wayback Machine | CDX API | `cache-archive.md` | 🚧 M3+ |

## Korea-specific — 🚧 M2–M3

| Platform | Planned route | Reference | Status |
|----------|---------------|-----------|--------|
| Naver Search | impersonation + `search.naver.com` (통합 / 블로그 / 뉴스) | `naver.md` | 🚧 M3 |
| Naver Finance (stock prices) | `api.finance.naver.com/siseJson.naver` (unofficial, no auth) | `naver.md` | 🚧 M3+ |

## Fallback & self-learning

| Capability | Planned mechanism | Milestone |
|------------|-------------------|-----------|
| Playwright fallback (capability-matched) | `executor.rs` + Node templates + `must_invoke_playwright_mcp` flag + browser→curl cookie bridge | 🚧 M4 |
| Per-host winning-route store | `learning.rs` (promote / strike / evict) | 🚧 M5 |

## Reference files (planned)

The skill will ship a set of reference files under
`skills/adaptive-fetch/references/`, each covering one class of techniques —
ported from insane-search and landing with M6.

| File | Covers | Milestone |
|------|--------|-----------|
| `fallback.md` | Phase 0→3 adaptive scheduler, escalation signals, response validation | 🚧 M6 |
| `json-api.md` | Public JSON APIs (Reddit, HN, dev.to, Wikipedia, npm, PyPI, …) | 🚧 M6 |
| `public-api.md` | Bluesky, Mastodon, Stack Exchange, arXiv, CrossRef, OpenLibrary, GitHub | 🚧 M6 |
| `media.md` | `yt-dlp` usage for media sites | 🚧 M6 |
| `twitter.md` | X/Twitter tweet-result + oEmbed + syndication + WebSearch keyword search | 🚧 M6 |
| `naver.md` | Naver Search (impersonation), blog mobile URLs, Finance JSON API | 🚧 M6 |
| `rss.md` | RSS / Atom feeds, Google News RSS | 🚧 M6 |
| `tls-impersonate.md` | `rquest` multi-target impersonation, cookie warming, referrer chains, challenge detection | 🚧 M6 |
| `playwright.md` | Playwright MCP toolkit (snapshot, evaluate, network_requests) | 🚧 M6 |
| `cache-archive.md` | Google AMP cache, archive.today, Wayback Machine | 🚧 M6 |
| `metadata.md` | OGP, JSON-LD, Schema.org, Next.js RSC payload extraction | 🚧 M6 |

## Dependencies (planned)

The key departure from insane-search: **TLS impersonation is native**. The engine
links `rquest` and impersonates browser fingerprints in-process, so there is no
`pip install curl_cffi` step and no Python runtime.

**Required:** Claude Code + the `adaptive-fetch` binary (single static binary; see
`setup/setup.sh`, M6).

**Auto-invoked when needed** (external CLI, not linked into the engine):

```bash
yt-dlp        # media sites (Phase 0 YouTube route + media.md), M3
```

**Optional, improves coverage:**

```bash
gh                                                       # GitHub (faster than REST)
claude mcp add playwright -- npx @playwright/mcp@latest  # JS-rendered sites, M4
```

## What adaptive-fetch is not

- **Not a scraper** — a method-selection layer over public APIs and standard access techniques.
- **Not API-key based** — everything uses no-auth public endpoints or URL transformations.
- **Not a hand-maintained answer key** — the Phase 0 index is minimal; everything else is discovered by the adaptive scheduler.
- **Not bias-forming** — there is no "access denied" list, and no site name lives in the engine core (only `phase0`, docs, and runtime presets). If a site can be reached, the chain finds the way.

## Example prompts (planned behaviour)

No commands — just talk normally. The skill triggers when a URL is blocked or when
a platform needs special handling. These illustrate the intended M3+ behaviour:

```
"What's on the front page of Hacker News right now?"
→ Firebase API → top stories with scores and comments

"Find AI papers published this week on arXiv"
→ arXiv Atom API with date filter

"Scrape Coupang for laptop deals under $1000"
→ generic grid: rquest impersonation → JSON-LD ItemList

"Check what people are saying about Claude Code on Reddit"
→ Reddit .rss feed → posts

"Search X for adaptive-fetch"
→ intent routing: keyword → WebSearch(site:x.com) → tweet-result → full tweets

"네이버에서 클로드코드 뉴스 찾아줘"
→ Naver Search (impersonation) → news tab → article URLs
```
