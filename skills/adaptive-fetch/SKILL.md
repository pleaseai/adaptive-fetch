---
name: adaptive-fetch
description: >
  Auto-bypass for blocked websites — tries every site-agnostic strategy until one
  works. Use when WebFetch returns 402/403/blocked or hits a WAF/CAPTCHA. UNDER
  CONSTRUCTION (M0 scaffold): the Rust engine is not wired up yet, so do not rely
  on this skill for real fetches. See docs/rfcs/0001-adaptive-fetch.md.
---

# adaptive-fetch (under construction)

> ⚠️ **M0 scaffold.** The engine (`engine-src/`) currently returns an honest
> "not implemented" result. The harness rules (R1–R7), Phase 0 index, and usage
> contract below are being ported from the design and will activate as the
> milestones land.

The full design — architecture, invariants, and the milestone plan — lives in
[`docs/rfcs/0001-adaptive-fetch.md`](../../docs/rfcs/0001-adaptive-fetch.md).

## Engine contract (target)

```bash
adaptive-fetch "<URL>" [--selector "<CSS>"]... [--device auto|desktop|mobile] [--trace] [--json]
# exit 0 = validated success, exit 1 = failure (with untried_routes in --json)
```

Until M1+ ships, treat a non-zero exit and `stop_reason="unimplemented"` as
"engine not ready" rather than "site is unreachable".

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

In the M0 scaffold the hook and preset-matching layer are wired and tested, but
the deny is intentionally held back: `check-url` reports `engine_ready = false`
(the engine's fetch is still a stub), and the hook only denies once that flips to
`true` in a later milestone. Until then a preset match is a no-op and `WebFetch`
runs unchanged — the routing never strands a request on an engine that cannot yet
retrieve it.
