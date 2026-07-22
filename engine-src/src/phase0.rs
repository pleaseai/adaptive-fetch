//! Phase 0 — official public-API router (M3, RFC 0001 §4.3).
//!
//! The SANCTIONED exception to the no-site-name rule: the ONLY module allowed to
//! name platform hosts, and the only one exempt from the bias linter. Tries an
//! official no-auth endpoint BEFORE the generic grid (Reddit→.rss,
//! X→tweet-result/oEmbed/syndication, YouTube→yt-dlp).
//!
//! First landed route: **Reddit → `.rss`** (the unauth `.json` is WAF-gated; the
//! RSS feed survives). Route *selection* is deterministic — no probe/detect/grid
//! is needed to discover it — so Phase 0 short-circuits the adaptive loop. The
//! fetch itself is still validated, and a challenged feed falls back to the grid
//! (R6), so "deterministic route" never means "trust blindly".

use url::Url;

/// A recognized platform with an official no-auth endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Reddit,
}

/// A deterministic Phase 0 route: the rewritten official endpoint plus a trace
/// label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub platform: Platform,
    /// Trace label, e.g. `reddit:rss`.
    pub name: String,
    /// The official-endpoint URL to fetch.
    pub url: String,
}

/// Recognize the platform for `url`, if Phase 0 knows it.
pub fn detect(url: &str) -> Option<Platform> {
    let host = host_of(url)?;
    if host == "reddit.com" || host.ends_with(".reddit.com") {
        return Some(Platform::Reddit);
    }
    None
}

/// Build the deterministic Phase 0 route for `url`, or `None` if unrecognized.
pub fn route(url: &str) -> Option<Route> {
    match detect(url)? {
        Platform::Reddit => reddit_rss(url),
    }
}

/// Whether Phase 0 can service `url`. The WebFetch hook reads this (via
/// `check-url`'s route-aware `engine_ready`) so it only denies hosts the engine
/// can actually fetch.
pub fn can_route(url: &str) -> bool {
    route(url).is_some()
}

/// Lowercase hostname of `url` with any trailing DNS root dot removed.
fn host_of(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    Some(host.strip_suffix('.').unwrap_or(&host).to_string())
}

/// Reddit → `.rss` feed. Appends `.rss` to the path (leaving an already-`.rss`
/// URL intact) and normalizes the bare apex to `www` so the endpoint does not
/// depend on a redirect.
fn reddit_rss(url: &str) -> Option<Route> {
    let mut parsed = Url::parse(url).ok()?;
    if parsed.host_str().map(str::to_ascii_lowercase).as_deref() == Some("reddit.com") {
        parsed.set_host(Some("www.reddit.com")).ok()?;
    }

    let path = parsed.path().trim_end_matches('/');
    if !path.ends_with(".rss") {
        let new_path = if path.is_empty() {
            "/.rss".to_string()
        } else {
            format!("{path}/.rss")
        };
        parsed.set_path(&new_path);
    }
    parsed.set_fragment(None);

    Some(Route {
        platform: Platform::Reddit,
        name: "reddit:rss".to_string(),
        url: parsed.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_reddit_apex_and_subdomains() {
        assert_eq!(detect("https://reddit.com/r/rust"), Some(Platform::Reddit));
        assert_eq!(
            detect("https://www.reddit.com/r/rust"),
            Some(Platform::Reddit)
        );
        assert_eq!(
            detect("https://old.reddit.com/r/rust"),
            Some(Platform::Reddit)
        );
        // trailing DNS root dot resolves to the same host
        assert_eq!(detect("https://reddit.com./r/rust"), Some(Platform::Reddit));
    }

    #[test]
    fn does_not_match_lookalike_hosts() {
        assert_eq!(detect("https://notreddit.com/r/rust"), None);
        assert_eq!(detect("https://reddit.com.evil.com/r/rust"), None);
        assert_eq!(detect("https://example.com/path/reddit.com/x"), None);
    }

    #[test]
    fn reddit_route_appends_rss_and_normalizes_apex() {
        let r = route("https://reddit.com/r/rust").expect("reddit route");
        assert_eq!(r.name, "reddit:rss");
        assert_eq!(r.url, "https://www.reddit.com/r/rust/.rss");
    }

    #[test]
    fn reddit_route_preserves_subdomain_and_query() {
        let r = route("https://old.reddit.com/r/rust?sort=new").expect("reddit route");
        assert_eq!(r.url, "https://old.reddit.com/r/rust/.rss?sort=new");
    }

    #[test]
    fn reddit_route_frontpage_and_already_rss() {
        assert_eq!(
            route("https://www.reddit.com/").unwrap().url,
            "https://www.reddit.com/.rss"
        );
        // an already-.rss URL is left intact (no double suffix)
        assert_eq!(
            route("https://www.reddit.com/r/rust/.rss").unwrap().url,
            "https://www.reddit.com/r/rust/.rss"
        );
    }

    #[test]
    fn can_route_reflects_recognition() {
        assert!(can_route("https://www.reddit.com/r/rust"));
        assert!(!can_route("https://example.com/"));
    }
}
