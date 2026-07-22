# RFC 0001 — adaptive-fetch engine

| | |
|---|---|
| **Status** | Accepted (engine language) — implementation pending |
| **Author** | Minsu Lee (@amondnet) |
| **Supersedes** | — |

> **Decision log:** engine language = **Rust**, confirmed. The Bun path (§9) is
> retained only as a documented alternative, not a live option.

> A resilient public-page reader for Claude Code. When a fetch is blocked
> (402 / 403 / WAF / CAPTCHA), `adaptive-fetch` automatically tries every
> site-agnostic bypass strategy until one works — no API keys, no proxy setup.
>
> Modeled on [`fivetaku/insane-search`](https://github.com/fivetaku/insane-search),
> but with the engine rewritten in **Rust** so the core capability (browser TLS
> fingerprint impersonation) runs natively, in-process, as a single static binary.

This RFC proposes the architecture and an implementation plan to review and
iterate on **before** any engine code is written. Follow-up RFCs (`0002-…`) can
refine individual milestones as the design evolves.

---

## 1. Goal & scope

`adaptive-fetch` is a **Claude Code plugin** (a `SKILL.md` + reference docs +
a compiled engine binary). When `WebFetch` returns a block, the skill kicks in
and routes the URL through an adaptive scheduler that escalates through cheap →
expensive bypass strategies and stops as soon as a *validated* success is found.

It is **site-agnostic by construction**: the engine never hard-codes a domain,
selector, or brand name. Site knowledge enters only at runtime (caller-supplied
selectors / hints) or through generic WAF-product profiles. A small, sanctioned
exception exists for official public APIs (Phase 0 — see §4.3).

### Non-goals
- Defeating authentication / paywalls (terminal failures, reported honestly).
- Bypassing rate limits by hammering (429 is transient → back off, not brute-force).
- Any per-site scraping logic baked into the engine (CI lint forbids it).

---

## 2. Why Rust for the engine (the decisive trade-off)

The entire value of insane-search is **TLS/JA3/JA4 fingerprint impersonation** —
making an HTTP client's TLS handshake look like a real Safari/Chrome so a WAF
that fingerprints the client doesn't reject it. insane-search gets this from
`curl_cffi` (a Python binding over `curl-impersonate`).

A Bun/Node engine **cannot do this natively** — Node's TLS stack is itself a
recognizable fingerprint. So whichever language we pick, the impersonation layer
needs a native component. The comparison is therefore about *where* that native
code lives:

| Concern | **Rust (recommended)** | Bun / TypeScript |
|---|---|---|
| TLS impersonation | `rquest` / `wreq` (BoringSSL) — native, in-process, Chrome/Safari/Firefox/Edge JA3+JA4+HTTP2 emulation. The Rust analogue of curl_cffi. | No native option. Must `bun:ffi`-load `tls-client` (Go `.so`/`.dylib`/`.dll`) or shell out to a `curl-impersonate` binary. Native dep either way. |
| Distribution | One self-contained static binary per platform (`ripgrep` model). | TS source + `bun install`, **plus** a per-platform native TLS lib to FFI-load or a sidecar process. |
| HTML/CSS selectors | `scraper` (servo's html5ever + selectors). | `cheerio` / `linkedom` — easy. |
| Startup latency | Cold-start ~ms; matters for a per-request CLI. | Bun cold-start fine, but FFI lib load adds setup. |
| pleaseai standard | Diverges (standard is bun/TS), but justified by capability. | Matches the standard toolchain. |

**Decision: Rust (confirmed).** The one hard requirement (native TLS impersonation)
is in-process and first-class in Rust via `rquest`/`wreq`, and the output is a
single static binary that the skill invokes exactly like insane-search invokes
`python3 -m engine`. Bun would still need a per-platform native blob for the only
thing that actually matters, giving up its main advantage (pure-TS distribution)
while keeping a worse impersonation story.

> The Bun path remains viable if avoiding per-platform binary distribution is the
> top priority — see §9 for the fallback design. This document assumes Rust.

> Source-driven note: `rquest` was renamed to `wreq`; confirm the current crate
> name, version, and the exact `Emulation`/`Impersonate` profile API at
> implementation time rather than trusting this doc's snapshot.

---

## 3. Deliverable shape (plugin layout)

Mirrors insane-search's plugin structure; engine is a Rust binary instead of a
Python package.

```
adaptive-fetch/
├─ .claude-plugin/
│  └─ plugin.json                  # plugin manifest
├─ hooks.json                      # PreToolUse(WebFetch) hook registration
├─ hooks/
│  └─ webfetch-guard.sh            # fail-open guard: check-url → deny + redirect
├─ skills/
│  └─ adaptive-fetch/
│     ├─ SKILL.md                  # harness rules R1–R7 + Phase 0 index + usage
│     ├─ url_presets.toml          # caller-editable per-host routing presets (runtime)
│     ├─ references/               # on-demand deep docs (tls, playwright, apis…)
│     │  ├─ tls-impersonate.md
│     │  ├─ playwright.md
│     │  ├─ public-api.md
│     │  ├─ json-api.md
│     │  ├─ media.md
│     │  └─ … (rss, metadata, cache-archive, naver, twitter, fallback)
│     └─ engine/
│        ├─ bin/                   # downloaded prebuilt binary lands here
│        ├─ waf_profiles.yaml      # WAF-product priors (shipped, editable)
│        └─ templates/             # Playwright node templates (real-chrome, mobile)
├─ engine-src/                     # Rust crate (the source of the binary)
│  ├─ Cargo.toml
│  └─ src/
│     ├─ main.rs                   # CLI entrypoint + JSON output
│     ├─ lib.rs                    # `fetch(url, opts) -> FetchResult`
│     ├─ presets.rs                # host-glob URL presets + `check-url` matching
│     ├─ scheduler.rs              # diversity planner + grid + failure gate (R6)
│     ├─ transport.rs              # rquest session pool, warmup, cookie bridge
│     ├─ validators.rs             # 4-layer validation, Verdict enum
│     ├─ waf_detector.rs           # ranked WAF-product detection
│     ├─ url_transforms.rs         # generic URL rewrite rules
│     ├─ phase0.rs                 # official public-API router (sanctioned)
│     ├─ executor.rs               # Playwright fallback routing (node subprocess / MCP flag)
│     ├─ learning.rs               # per-host winning-route store
│     └─ safety.rs                 # SSRF guard (private/loopback/metadata block-list)
├─ setup/
│  └─ setup.sh                     # first-run: download correct binary, deps check
├─ orca.yaml                       # pleaseai worktree/dev-tooling (rust+mise variant)
├─ .worktreeinclude
└─ DESIGN.md
```

### Engine public contract (unchanged from insane-search, so SKILL.md rules port verbatim)

```bash
adaptive-fetch "<URL>" [--selector "<CSS>"] [--device auto|desktop|mobile] [--trace] [--json]
# exit 0 = ok (validated success), exit 1 = fail (with untried_routes in --json)
```

Library form (`fetch`) returns a `FetchResult` with the same fields insane-search
exposes: `ok`, `content`, `final_url`, `verdict`, `profile_used`, `trace[]`,
`planned_attempts`, `executed_attempts`, `grid_exhausted`, `stop_reason`,
`untried_routes[]`, `must_invoke_playwright_mcp`.

### 3.1 WebFetch hook + URL presets

A `PreToolUse` hook (`hooks.json` → `hooks/webfetch-guard.sh`) intercepts
`WebFetch` calls *before* they run and, for hosts a user has flagged as hard,
steers them through the engine instead of letting `WebFetch` fail first. The hook
shells out to a site-agnostic `adaptive-fetch check-url "<URL>" --presets <file>
--json` subcommand, which matches the URL's **hostname** against
`skills/adaptive-fetch/url_presets.toml` (first host-glob match wins) and, on a
match, tells the hook to **deny** the built-in `WebFetch` with a
`permissionDecision` reason carrying the suggested `adaptive-fetch …` command.
Host-scoped matching (not full-URL) keeps a glob from crossing path boundaries and
lets bare origins match without a trailing slash.

Invariant fit: the `check-url` code (`engine-src/src/presets.rs`) names no site —
domains live only in `url_presets.toml`, a caller-supplied **runtime** config
(the same sanctioned channel as `success_selectors` / `user_hint`, §7). The hook
is **fail-open**: any error (missing binary, `jq`, or presets file; parse
failure; no match) lets `WebFetch` proceed unchanged. It also stays fail-open
unless the engine can actually service **that** url: `check-url` reports a
**route-aware** `engine_ready` (`phase0::can_route(url)` — true only when the
engine has a working route for the host, Reddit today), and the hook only denies
when it is true, so a preset host with no working route (or on a milestone before
its route lands) is never stranded. `check-url` exits `10` on a match, `0` otherwise.

---

## 4. Engine architecture (port of the insane-search invariants)

The whole point of insane-search's engine is a set of **hard invariants** that
stop an agent from bailing out early. We carry these over exactly; only the
language changes.

### 4.1 Single entrypoint + explicit phases

`fetch()` is the only public API, but internally it is staged so `trace[]` can
attribute every attempt:

```
probe → validate → detect → plan → execute (grid) → fallback → report
```

### 4.2 Validation: HTTP 200 is the *start* of inspection, not success

Port `validators.rs` as a layered AND check (insane-search's `validators.py`):

1. **Status semantics** — 429 → `RateLimited` (transient), 401/407 → `AuthRequired`
   (terminal), 404/410 → `NotFound` (terminal), 5xx → `Blocked`, 0 → `Unknown`.
2. **Hard challenge markers** — structural WAF containers (`Just a moment...`,
   `sec-if-cpt-container`, Incapsula/Akamai strings) → `Challenge`, decisive.
3. **Size fingerprint** — body byte-size near a known-bad WAF stub size → `Challenge`.
4. **JSON awareness** — small non-empty parseable JSON is a *success* (`WeakOk`),
   not a challenge (this is what unlocks the R7 API-first route).
5. **Caller positive proof** — `success_selectors` match → `StrongOk`
   (`scraper` crate for CSS); requested-but-unmatched → `Challenge`.
6. **Heuristics (no proof)** — soft markers, tiny incomplete body, unresolved
   Akamai `_abck=~-1~` sensor cookie → `SuspectOk` (non-terminal: keep searching).

Verdict enum: `StrongOk`, `WeakOk`, `SuspectOk`(non-terminal), `Challenge`,
`Blocked`, `RateLimited`, `AuthRequired`, `NotFound`, `Unknown`. Terminal-nonsuccess
= {`AuthRequired`, `NotFound`} (give up — no route recovers the resource).
Grid-stop = terminal-nonsuccess ∪ {`RateLimited`}: a 429 also halts the TLS grid
(more handshakes won't help — don't hammer), but stays a transient
back-off-and-retry route via the failure gate, not a give-up.

### 4.3 Phase 0 — official public-API router (the only site-aware module)

`phase0.rs` is the **sole** engine file allowed to name platform hosts (exempt
from the bias linter). For recognised platforms it tries the official no-auth
endpoint *before* the generic grid:

- **Reddit** → `.rss` (the JSON API is WAF-gated; RSS survives).
- **X/Twitter** → `cdn.syndication.twimg.com/tweet-result` + oEmbed (single
  tweet), `syndication.twitter.com` timeline (profile, retry-once).
- **YouTube** → `yt-dlp --dump-json` subprocess.

Extensible to HN/Bluesky/Mastodon/arXiv/etc. as documented in `references/`.

**Phase 0 fetches with a plain client, not browser impersonation** (found live
during the M3 Reddit slice). Official endpoints are built for *simple* consumers
(RSS readers, API clients); a full browser TLS/JA3 fingerprint plus `sec-ch-ua`
client hints on a `.rss`/API URL is itself anomalous — real browser users load
HTML, not feeds — and trips anti-bot. Observed: Reddit's `/r/<sub>/.rss` returns a
real Atom feed (HTTP 200) to a plain client but a 403 challenge page to a
Chrome-emulated one from the same IP. So the impersonation grid (§4.5–4.7) is a
Phase 1–3 tool for scraping HTML that *expects* a browser; Phase 0 stays plain.
This is orthogonal to R6: a plain Phase 0 fetch is still validated, and a
challenged/blocked/rate-limited feed falls back to the grid on the original URL.

### 4.4 WAF detection → ranked priors

`waf_detector.rs` scores each profile in `waf_profiles.yaml` against the live
response (cookies / headers / server / body markers) and returns a **ranked
list** of `(profile_id, confidence)` — never a single verdict (a wrong single
guess cascades into a wrong plan). Profiles cover Akamai Bot Manager, Cloudflare
Turnstile, F5 BIG-IP, AWS WAF, DataDome, PerimeterX/HUMAN, plus an
`unknown_challenge` safety net. Each profile carries: detectors, capabilities
needed (`needs_real_tls_stack`, `needs_js_exec`, `needs_mobile_context`), TLS
candidate families, referer strategies, URL-transform order, and fallback chain.

### 4.5 The diversity scheduler (the heart)

`scheduler.rs` ports the v2 planner from `fetch_chain.py`:

- Materialize the full grid = `url_transforms × tls_impersonate × referer`
  across the top-3 detected profiles (round-robin interleaved so a confident #1
  can't starve #2/#3).
- **Order for diversity:** vary TLS *family* fastest, then transform, then
  version depth, then referer — so a small attempt budget still touches every
  family/transform instead of burning out on one family's old versions.
- `tls_impersonate_avoid` targets are **deprioritized, never deleted** (still
  tried in exhaustive mode).
- `device_class` shapes the grid (`mobile` → mobile TLS + `m.` subdomain
  transforms; `desktop` → desktop TLS only).
- Default `max_attempts = None` = **exhaustive** (honours R6). A numeric cap is
  a *budget*; budget vs exhaustion vs early-terminal is reported via
  `stop_reason` / `grid_exhausted` so a truncated run is never mistaken for a
  true exhaustive failure.
- Jitter sleep only on a *continuing* (failed) attempt, never before returning success.

### 4.6 Failure gate (R6) — give-up is never silent

When `ok=false`, the engine reports what it could *not* itself do:
`untried_routes[]` and `must_invoke_playwright_mcp`. A non-terminal failure
always surfaces: (a) re-run exhaustive if grid not exhausted, (b) Playwright MCP
from the agent session (engine can only spawn local Node Chrome, so MCP is
structurally the agent's job), (c) `user_hint` retry. The CLI prints a
`⛔ NOT EXHAUSTED` block to stderr so the agent knows it must continue. 429 is
explicitly **not** terminal (back off + retry / different TLS / MCP).

### 4.7 Transport: session pool + warmup + cookie bridge

`transport.rs` ports `transport.py` onto `rquest`:

- **Per-(host, impersonate) session pool** — reuse cookies (WAF sensors like
  `_abck`, `cf_clearance` need to *mature* across requests) and the warm
  connection across attempts and across pages of the same host.
- **Root warmup** — for deep URLs, hit the site root once so a sensor sets a
  resolved cookie before the deep request (classic first-hit-rejection fix).
- **Browser→curl cookie bridge** — when a Playwright pass clears a JS challenge,
  inject its cookies + UA into the pooled session so one expensive browser pass
  converts into cheap impersonated-HTTP throughput (the FlareSolverr pattern).
- **SSRF guard** — `safety.rs` validates the initial URL *and every redirect hop*
  against a private/loopback/link-local/cloud-metadata block-list; redirects are
  followed manually so each hop is checked.

### 4.8 Playwright fallback (capability-matched)

`executor.rs` reads the profile's `capabilities_needed` and routes:

| Capability | Executor | In the Rust engine |
|---|---|---|
| `needs_real_tls_stack` + `needs_js_exec` | local Node `playwright_real_chrome.js` | engine spawns `node` subprocess (templates shipped in `engine/templates/`) |
| `needs_js_exec` only | Playwright **MCP** | engine **cannot** drive MCP → sets `must_invoke_playwright_mcp=true`; the *agent* runs it (per R6) |
| `needs_mobile_context` | `playwright_mobile_chrome.js` | node subprocess, mobile device emulation |

The Node templates are reused as-is from insane-search (no Rust browser needed);
the engine only orchestrates the subprocess and bridges cookies back.

### 4.9 Self-learning (per-host winning route)

`learning.rs` ports `learning.py`: a bounded, self-pruning JSON store mapping a
host (+ device class) to its last winning route `{transform, impersonate,
referer}`. On the next fetch the winning route is promoted to the probe identity
and the front of the grid; two consecutive *real* failures evict it. Any error in
the store is swallowed — learning can never break a fetch
(`ADAPTIVE_FETCH_LEARN=0` to disable).

---

## 5. Crate / dependency choices (Rust)

| Need | Crate | Note |
|---|---|---|
| TLS impersonation HTTP client | `rquest` / `wreq` | BoringSSL JA3/JA4/HTTP2 browser emulation — the core. Verify current name/API. |
| Async runtime | `tokio` | required by the client. |
| CSS selectors / HTML | `scraper` | for `success_selectors` validation. |
| WAF profiles (YAML) | `serde_yaml` (+ `serde`) | embed via `include_str!` with a ship-alongside override path. |
| JSON I/O (`--json`, learning store) | `serde_json` | result + trace serialization. |
| IP / CIDR classification (SSRF) | `ipnet`, `std::net` | block private/loopback/link-local/metadata. |
| CLI args | `clap` | `--selector/--device/--trace/--json`. |
| URL parsing | `url` | transforms + redirect resolution. |

Map insane-search's impersonate target names (`safari`, `safari_ios`,
`chrome131`, `chrome_android`, `firefox`, `edge99`…) onto the chosen crate's
emulation profiles in `transport.rs`. Where a target has no 1:1 emulation, pick
the nearest and record the mapping in `references/tls-impersonate.md`.

---

## 6. Distribution & install (the real cost of the Rust choice)

The skill needs the right binary on the user's machine with zero friction.

- **Build:** `cargo-dist` (or a `cross` matrix in CI) produces binaries for
  `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, published to GitHub
  Releases with checksums.
- **Install:** `setup/setup.sh` (run once by SKILL.md Step 0, idempotent)
  detects OS/arch, downloads the matching binary into `engine/bin/`, verifies the
  checksum, `chmod +x`. Falls back to `cargo install` only if a Rust toolchain
  is present and no prebuilt asset matches.
- **Alternative:** npm package with `optionalDependencies` per-platform (the
  `esbuild`/`@biomejs` model) so `bunx`/`npx` resolves the right binary —
  attractive because it matches the pleaseai bun toolchain for the *wrapper*
  even though the engine is Rust.
- **Runtime deps the binary can't bundle:** `node` + `playwright` (only when the
  Playwright fallback fires), `yt-dlp` (only for Phase 0 media). The skill checks
  and installs these lazily, exactly like insane-search.

CI must pin third-party GitHub Actions to full commit SHAs (pleaseai
`github-actions` rule).

---

## 7. The "no site names" invariant + CI gate

Port `bias_check.py` as a Rust test or a small CI script that fails the build if
any file under `engine-src/src/**` (except `phase0.rs`) or `waf_profiles.yaml`
contains a site domain, brand name, or `if url.contains("<site>")` branch.
Allowed: descriptive prose in `SKILL.md`/`references/*.md`, the Phase 0 official
endpoints, runtime `success_selectors`/`user_hint`, and append-only observation logs.

Boundary rule: *"Would this entry be valid on any other site running the same
WAF?"* → yes ⇒ `waf_profiles.yaml`; no ⇒ runtime hint.

---

## 8. Implementation milestones (proposed, after this design is approved)

| Milestone | Deliverable | Verifies |
|---|---|---|
| **M0 Scaffolding** | Cargo crate, `clap` CLI skeleton, `orca.yaml` (rust+mise), `.worktreeinclude`, CI (build + SHA-pinned actions), `plugin.json` | `cargo build`, plugin loads |
| **M1 Probe + validate** | `transport.rs` (single rquest GET + SSRF), `validators.rs` (all layers), `safety.rs`; CLI returns verdict + `--json` | unit tests on canned responses; example.com, a JSON API |
| **M2 Grid scheduler** | `waf_detector.rs`, `waf_profiles.yaml`, `url_transforms.rs`, `scheduler.rs` (diversity plan, exhaustive grid, failure gate, jitter), session pool + warmup | grid ordering tests; `untried_routes`/`grid_exhausted` correctness |
| **M3 Phase 0** | `phase0.rs` (reddit/x/youtube routers) | per-platform route tests, trace records |
| **M4 Playwright fallback** | `executor.rs` capability matching, Node templates, `must_invoke_playwright_mcp` flag, cookie bridge | mock subprocess; flag set correctly for MCP-only profiles |
| **M5 Learning** | `learning.rs` per-host store, eviction | promote/strike/evict tests |
| **M6 Skill + ship** | `SKILL.md` (R1–R7, Phase 0 index, intent table), `references/*.md`, `setup.sh`, release pipeline, bias-check CI, coverage battery | end-to-end against a real blocked site; bias gate green |

Each milestone keeps files ≤500 LOC (engineering standard) — `scheduler.rs` is
the risk; split planner / executor-loop / result-builder if it grows.

---

## 9. Bun fallback design (if per-platform binaries are rejected)

If the team prefers to stay on the standard bun/TS toolchain and accept a native
TLS sidecar:

- **TLS layer:** `bun:ffi` loading bogdanfinn's `tls-client` shared library
  (Go, exposes JA3 impersonation), or a long-lived `curl-impersonate` /
  `tls-client` HTTP sidecar the TS engine talks to. Per-platform native lib still
  downloaded by `setup.sh`.
- **Everything else** maps cleanly to TS: `cheerio` (selectors), `yaml`
  (profiles), native `fetch`/`undici` only for the non-impersonated Phase 0/JSON
  routes, `node:net`/`ipaddr.js` (SSRF), Playwright node templates reused directly.
- **Cost:** the only hard requirement (impersonation) is the clunkiest part, and
  we still ship a native blob — i.e. we pay Rust's distribution cost without
  Rust's clean in-process impersonation. This is why §2 recommends Rust.

---

## 10. Open decisions for review

1. ~~**Engine language:** Rust vs Bun fallback (§9).~~ **RESOLVED → Rust.**
2. **Distribution:** GitHub Releases + `setup.sh` download, vs npm
   `optionalDependencies` per-platform.
3. **Phase 0 breadth at v1:** ship reddit/x/youtube only, or also HN / Bluesky /
   arXiv / Naver from the start.
4. **Playwright in v1:** include the local-Node fallback in the first release, or
   ship curl-grid-only first and add browser fallback in a follow-up (relying on
   the `must_invoke_playwright_mcp` agent route until then).
5. **Repo standards:** confirm `orca.yaml` should use the `mise` rust toolchain
   variant (vs the default bun template).
