//! # adaptive-fetch engine
//!
//! A resilient, **site-agnostic** public-page reader. When a fetch is blocked
//! (402 / 403 / WAF / CAPTCHA) it escalates through cheap → expensive bypass
//! strategies and stops as soon as a *validated* success is found.
//!
//! Single public entrypoint: [`fetch`]. Internally staged so [`FetchResult`]'s
//! `trace` can attribute every attempt:
//!
//! ```text
//! probe → validate → detect → plan → execute (grid) → fallback → report
//! ```
//!
//! Full design: `docs/rfcs/0001-adaptive-fetch.md`.
//!
//! **No-site-name invariant:** no module here (except `phase0`) may name a site
//! domain, selector, or brand. Site knowledge enters only via [`FetchOptions`].

mod options;
pub mod presets;
mod result;

// Engine stages (RFC 0001 §4). Stubs now; filled in across milestones M1–M5.
mod executor; // M4: Playwright fallback routing (node subprocess / MCP flag)
mod learning; // M5: per-host winning-route store
mod phase0; // M3: official public-API router (the sole site-aware module)
mod safety; // M1: SSRF guard (private/loopback/link-local/metadata block-list)
mod scheduler; // M2: diversity planner + grid + failure gate (R6)
mod transport; // M1: rquest session pool, root warmup, browser→curl cookie bridge
mod url_transforms; // M2: generic URL rewrite rules
mod validators; // M1: 4-layer validation, Verdict classification
mod waf_detector; // M2: ranked WAF-product detection

pub use options::{DeviceClass, FetchOptions, UserHint};
pub use presets::{PresetFile, UrlPreset};
pub use result::{Attempt, FetchResult, Verdict};

/// Whether the engine can service a real fetch yet.
///
/// `false` in the M0 scaffold — [`fetch`] returns `stop_reason = "unimplemented"`.
/// The WebFetch PreToolUse hook reads this (via `check-url`'s `engine_ready` field)
/// and stays **fail-open** while it is `false`, so a preset host is never denied
/// while the redirect target cannot actually retrieve it. Flip to `true` when M1
/// lands a working fetch route.
pub const ENGINE_READY: bool = false;

/// Fetch `url`, bypassing blocks site-agnostically.
///
/// **M0 scaffold:** the engine stages are not implemented yet, so this returns
/// an honest "not yet implemented" [`FetchResult`] — `ok = false`,
/// `stop_reason = "unimplemented"` — with the remaining milestones surfaced in
/// `untried_routes`. This lets the CLI and plugin wiring be exercised before the
/// network stages land in M1+.
pub fn fetch(url: &str, _opts: &FetchOptions) -> FetchResult {
    FetchResult {
        ok: false,
        content: String::new(),
        final_url: url.to_string(),
        verdict: Verdict::Unknown,
        profile_used: None,
        trace: Vec::new(),
        summary: "engine not implemented yet (M0 scaffold)".to_string(),
        planned_attempts: 0,
        executed_attempts: 0,
        grid_exhausted: false,
        stop_reason: "unimplemented".to_string(),
        untried_routes: vec![
            "M1: probe + 4-layer validation + SSRF transport".to_string(),
            "M2: diversity grid scheduler + WAF detection".to_string(),
            "M3: Phase 0 official-API router".to_string(),
            "M4: Playwright fallback".to_string(),
        ],
        must_invoke_playwright_mcp: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_returns_honest_unimplemented_result() {
        let r = fetch("https://example.com", &FetchOptions::default());
        assert!(!r.ok);
        assert_eq!(r.stop_reason, "unimplemented");
        assert_eq!(r.final_url, "https://example.com");
        assert!(!r.untried_routes.is_empty());
    }

    #[test]
    fn verdict_terminality() {
        assert!(Verdict::StrongOk.is_ok());
        assert!(!Verdict::SuspectOk.is_ok());
        assert!(Verdict::NotFound.is_terminal_nonsuccess());
        assert!(!Verdict::Challenge.is_terminal_nonsuccess());
        // 429 is transient, not terminal — but it still stops the TLS grid
        // (don't hammer). The two predicates are deliberately distinct.
        assert!(!Verdict::RateLimited.is_terminal_nonsuccess());
        assert!(Verdict::RateLimited.is_grid_stop());
        assert!(Verdict::NotFound.is_grid_stop());
        assert!(!Verdict::Challenge.is_grid_stop());
    }

    #[test]
    fn defaults_are_exhaustive() {
        // None == exhaustive (R6): the default must never silently cap attempts.
        assert_eq!(FetchOptions::default().max_attempts, None);
    }
}
