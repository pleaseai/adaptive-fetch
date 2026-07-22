//! Transport: plain feed/API GET (M3 Phase 0 slice, RFC 0001 §4.3 / §4.7).
//!
//! This slice provides a **single, plain** HTTP GET with manual, SSRF-checked
//! redirect following — the transport Phase 0's deterministic official endpoints
//! need. It deliberately does **not** impersonate a browser.
//!
//! **Why plain, not impersonated:** Phase 0 targets official no-auth endpoints
//! (feeds, JSON APIs) that are meant for *simple* clients — RSS readers, API
//! consumers. A full browser TLS fingerprint (JA3/JA4 + `sec-ch-ua` client hints)
//! hitting a feed/API endpoint is itself anomalous — real browser users load HTML
//! pages, not feeds — and trips anti-bot. Observed live on a major platform's feed
//! endpoint: a plain client gets a real feed (HTTP 200) while a Chrome-emulated one
//! gets a 403 challenge page from the same IP. Browser impersonation is a Phase 1–3
//! tool for scraping HTML that *expects* a browser; that multi-target diversity grid
//! (with the per-(host, profile) session pool and root warmup of §4.7) lands with M2.

use std::time::{Duration, Instant};

use url::Url;
use wreq::header::{ACCEPT, LOCATION, USER_AGENT};
use wreq::redirect::Policy;

use crate::safety;

/// Trace label for the plain Phase 0 client (recorded as `profile_used`).
const PROFILE: &str = "plain";

/// A mainstream browser User-Agent. Official endpoints gate on an empty/unknown
/// UA, and a common browser string is proven-accepted; the point is that only the
/// *TLS/H2 fingerprint* is plain, not that we hide being a normal client.
const FEED_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";

/// Feed/API-first Accept header (RSS/Atom preferred, then generic XML, then HTML).
const FEED_ACCEPT: &str =
    "application/atom+xml, application/rss+xml, application/xml;q=0.9, text/html;q=0.8, */*;q=0.7";

/// Redirect hop cap. Each hop is SSRF-checked before it is followed.
const MAX_REDIRECTS: usize = 10;

/// Body read cap. Validation only inspects a small prefix, so buffering an
/// unbounded response would only risk exhausting memory on a huge or hostile one.
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

/// Outcome of one GET, after manual redirect resolution.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub status: u16,
    pub final_url: String,
    pub body: String,
    pub elapsed_ms: u64,
    /// Client profile used (recorded in the trace).
    pub impersonate: String,
}

/// Perform one plain GET of `url`, following redirects manually so every hop is
/// re-checked by [`safety::classify_url`]. Synchronous wrapper over the async
/// `wreq` client so the public [`crate::fetch`] API stays sync.
pub fn get(url: &str, timeout: Duration) -> Result<Fetched, String> {
    // A nested current-thread runtime would panic if a library caller invokes the
    // sync fetch() from inside an existing Tokio runtime; return an error instead
    // so the process can't crash. (The CLI is fully sync, so it never hits this.)
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(
            "synchronous fetch() cannot run inside a Tokio runtime — call it from a blocking thread"
                .to_string(),
        );
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("tokio runtime: {error}"))?;
    runtime.block_on(get_async(url, timeout))
}

async fn get_async(url: &str, timeout: Duration) -> Result<Fetched, String> {
    // No `.emulation()`: a plain client (see module doc) is what official Phase 0
    // endpoints expect. The browser-impersonation grid lands with M2.
    let client = wreq::Client::builder()
        .redirect(Policy::none()) // follow manually so each hop is SSRF-checked
        .timeout(timeout)
        .build()
        .map_err(|error| format!("client build: {error}"))?;

    let started = Instant::now();
    let mut current = url.to_string();

    for _hop in 0..=MAX_REDIRECTS {
        // Resolve + SSRF-check every hop before connecting, so a redirect to a
        // host that *resolves* to a private/metadata IP is blocked, not just an
        // IP-literal one.
        safety::classify_url_resolved(&current).await?;

        let mut response = client
            .get(current.as_str())
            .header(USER_AGENT, FEED_UA)
            .header(ACCEPT, FEED_ACCEPT)
            .send()
            .await
            .map_err(|error| format!("request: {error}"))?;
        let status = response.status();

        if status.is_redirection() {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| format!("redirect {status} without a Location header"))?;
            let next = Url::parse(&current)
                .map_err(|error| format!("invalid current url: {error}"))?
                .join(location)
                .map_err(|error| format!("invalid redirect target: {error}"))?;
            current = next.to_string();
            continue;
        }

        let status_u16 = status.as_u16();
        let final_url = response.url().to_string();
        // Read the body with a hard cap so a huge/hostile response can't exhaust
        // memory. Feeds are UTF-8; a truncated tail never affects validation
        // (which inspects only the first few KB).
        let mut buf: Vec<u8> = Vec::new();
        while buf.len() < MAX_BODY_BYTES {
            match response
                .chunk()
                .await
                .map_err(|error| format!("read body: {error}"))?
            {
                Some(chunk) => {
                    let take = (MAX_BODY_BYTES - buf.len()).min(chunk.len());
                    buf.extend_from_slice(&chunk[..take]);
                }
                None => break,
            }
        }
        let body = String::from_utf8_lossy(&buf).into_owned();
        return Ok(Fetched {
            status: status_u16,
            final_url,
            body,
            elapsed_ms: started.elapsed().as_millis() as u64,
            impersonate: PROFILE.to_string(),
        });
    }

    Err(format!("too many redirects (> {MAX_REDIRECTS})"))
}
