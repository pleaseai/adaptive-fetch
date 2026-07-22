---
name: adaptive-fetch
description: >
  Auto-bypass for blocked websites — tries every site-agnostic strategy until one
  works. Use when WebFetch returns 402/403/blocked or hits a WAF/CAPTCHA. EARLY
  ACCESS: the Phase 0 router fetches Reddit (`.rss`) end-to-end today; the generic
  engine (probe → grid → fallback) is still landing across M1–M4, so most hosts
  still return an honest "not implemented" result. See docs/rfcs/0001-adaptive-fetch.md.
---

# adaptive-fetch (early access)

> ⚠️ **Partial engine.** The Phase 0 router (`engine-src/`) fetches Reddit
> (`.rss`) end-to-end today. Every other host still returns an honest "not
> implemented" result — the generic probe → grid → fallback stages are being
> ported from the design across M1–M4. The harness rules (R1–R7) and usage
> contract below activate as each milestone lands.

The full design — architecture, invariants, and the milestone plan — lives in
[`docs/rfcs/0001-adaptive-fetch.md`](../../docs/rfcs/0001-adaptive-fetch.md).

## Engine contract (target)

```bash
adaptive-fetch "<URL>" [--selector "<CSS>"]... [--device auto|desktop|mobile] [--trace] [--json]
# exit 0 = validated success, exit 1 = failure (with untried_routes in --json)
```

For a host with no Phase 0 route, a non-zero exit and `stop_reason="unimplemented"`
means "engine not ready for this host yet" — not "site is unreachable". A Phase 0
host (Reddit) returns real content on success, or an honest non-success verdict
(e.g. `blocked`, `ratelimited`) with grid-fallback routes in `untried_routes`.

## PreToolUse WebFetch hook + URL presets

The plugin registers a `PreToolUse` hook for `WebFetch`: `hooks.json` invokes
`hooks/webfetch-guard.sh`, which runs `adaptive-fetch check-url` against
`skills/adaptive-fetch/url_presets.toml`. When the first preset whose glob matches
the request hostname is found, the hook denies `WebFetch` and tells the agent to
run the suggested `adaptive-fetch "<url>" …` command instead.

The presets file is user-editable runtime configuration. Site knowledge stays
there and never enters the engine, preserving the site-agnostic invariant. The
hook is fail-open: if the binary, `jq`, input, presets, or output is unavailable
or invalid, `WebFetch` proceeds normally.

The hook needs the compiled engine binary. It looks first for
`skills/adaptive-fetch/engine/bin/adaptive-fetch`, then for `adaptive-fetch` on
`PATH`. Build it with `cargo build --release` in `engine-src/` and copy
`target/release/adaptive-fetch` into `skills/adaptive-fetch/engine/bin/` (or
install it on `PATH`); until then the hook simply finds no binary and fails open.

`check-url` reports `engine_ready` **per URL**: it is `true` only when the engine
has a working route for that exact host (Phase 0 recognizes it — Reddit today),
and `false` otherwise. The hook denies + redirects only when `engine_ready` is
true; every other preset match stays fail-open and `WebFetch` runs unchanged. The
routing never strands a request on an engine that cannot yet retrieve it.
