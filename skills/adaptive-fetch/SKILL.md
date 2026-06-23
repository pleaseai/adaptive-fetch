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
