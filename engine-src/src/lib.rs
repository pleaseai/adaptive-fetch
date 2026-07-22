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
pub use phase0::can_route;
pub use presets::{PresetFile, UrlPreset};
pub use result::{Attempt, FetchResult, Verdict};

/// Fetch `url`, bypassing blocks site-agnostically.
///
/// **Phase 0 slice (M3):** a recognized platform host with a deterministic
/// official-endpoint route (see [`phase0`], the sole site-aware module) is
/// fetched directly — [`phase0::route`] rewrites the URL, [`transport::get`] retrieves it with a
/// plain feed/API client (SSRF-checked redirects; no browser impersonation — a
/// browser fingerprint would itself trip anti-bot on an official endpoint), and
/// [`validators::validate_feed`] proves it. A validated feed returns `ok = true`
/// with the body; a challenged/blocked feed returns `ok = false` and hands off to
/// the not-yet-built grid via `untried_routes` (R6 — a deterministic route is
/// still validated, never trusted blindly).
///
/// Every **other** host still returns the honest "not yet implemented"
/// [`FetchResult`] — the generic probe → grid → fallback stages land in M1/M2/M4.
pub fn fetch(url: &str, opts: &FetchOptions) -> FetchResult {
    if opts.enable_phase0 {
        if let Some(route) = phase0::route(url) {
            return run_phase0(url, &route, opts);
        }
    }
    unimplemented_result(url)
}

/// Execute a deterministic Phase 0 route: fetch → validate → build the trace.
fn run_phase0(url: &str, route: &phase0::Route, opts: &FetchOptions) -> FetchResult {
    let timeout = std::time::Duration::from_secs(opts.timeout_secs);

    let fetched = match transport::get(&route.url, timeout) {
        Ok(fetched) => fetched,
        Err(error) => {
            let attempt = Attempt {
                phase: "phase0".to_string(),
                executor: format!("phase0:{}", route.name),
                url: route.url.clone(),
                url_transform: "phase0_route".to_string(),
                impersonate: None,
                referer: "-".to_string(),
                status: 0,
                body_size: 0,
                verdict: Verdict::Unknown,
                reasons: vec![error.clone()],
                elapsed_ms: 0,
                error: Some(error.clone()),
            };
            return FetchResult {
                ok: false,
                content: String::new(),
                final_url: url.to_string(),
                verdict: Verdict::Unknown,
                profile_used: None,
                trace: vec![attempt],
                summary: format!("phase0 {} transport error: {error}", route.name),
                planned_attempts: 1,
                executed_attempts: 1,
                grid_exhausted: false,
                stop_reason: "error".to_string(),
                untried_routes: grid_fallback_routes(),
                must_invoke_playwright_mcp: false,
            };
        }
    };

    let (verdict, reasons) = validators::validate_feed(fetched.status, &fetched.body);
    let attempt = Attempt {
        phase: "phase0".to_string(),
        executor: format!("phase0:{}", route.name),
        url: route.url.clone(),
        url_transform: "phase0_route".to_string(),
        impersonate: Some(fetched.impersonate.clone()),
        referer: "-".to_string(),
        status: fetched.status,
        body_size: fetched.body.len(),
        verdict,
        reasons: reasons.clone(),
        elapsed_ms: fetched.elapsed_ms,
        error: None,
    };

    if verdict.is_ok() {
        FetchResult {
            ok: true,
            content: fetched.body,
            final_url: fetched.final_url,
            verdict,
            profile_used: Some(fetched.impersonate),
            trace: vec![attempt],
            summary: format!("phase0 {} → {verdict:?}", route.name),
            planned_attempts: 1,
            executed_attempts: 1,
            grid_exhausted: false,
            stop_reason: "success".to_string(),
            untried_routes: Vec::new(),
            must_invoke_playwright_mcp: false,
        }
    } else {
        // R6: the deterministic route did not validate — hand off to the grid,
        // don't silently give up on a recognized host.
        FetchResult {
            ok: false,
            content: String::new(),
            final_url: fetched.final_url,
            verdict,
            profile_used: Some(fetched.impersonate),
            trace: vec![attempt],
            summary: format!(
                "phase0 {} did not validate ({verdict:?}): {}",
                route.name,
                reasons.join("; ")
            ),
            planned_attempts: 1,
            executed_attempts: 1,
            grid_exhausted: false,
            stop_reason: format!("{verdict:?}").to_ascii_lowercase(),
            untried_routes: grid_fallback_routes(),
            must_invoke_playwright_mcp: false,
        }
    }
}

/// The not-yet-built routes a failed Phase 0 fetch would escalate to (R6).
fn grid_fallback_routes() -> Vec<String> {
    vec![
        "M2: diversity grid scheduler + WAF detection".to_string(),
        "M4: Playwright fallback".to_string(),
    ]
}

/// Honest "not yet implemented" result for a host Phase 0 does not recognize.
fn unimplemented_result(url: &str) -> FetchResult {
    FetchResult {
        ok: false,
        content: String::new(),
        final_url: url.to_string(),
        verdict: Verdict::Unknown,
        profile_used: None,
        trace: Vec::new(),
        summary: "no Phase 0 route for this host; generic grid not implemented yet".to_string(),
        planned_attempts: 0,
        executed_attempts: 0,
        grid_exhausted: false,
        stop_reason: "unimplemented".to_string(),
        untried_routes: vec![
            "M1: generic probe + 4-layer validation + SSRF transport".to_string(),
            "M2: diversity grid scheduler + WAF detection".to_string(),
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
