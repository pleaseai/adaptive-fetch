//! SSRF guard (M1, RFC 0001 §4.7).
//!
//! Rejects a URL whose host is private / loopback / link-local / cloud-metadata
//! before any request is made, and is re-run on every redirect hop by the
//! transport (redirects are followed manually so each hop is re-checked).
//!
//! Two layers:
//! - [`classify_url`] — sync, offline: scheme + IP-literal + well-known-name
//!   checks. Fast and unit-testable with no network.
//! - [`classify_url_resolved`] — async: runs [`classify_url`], then resolves a
//!   hostname and rejects if ANY resolved address is blocked, closing the
//!   name-only gap where a host (e.g. a redirect target) points at a private /
//!   metadata IP. Pinning the resolved IP into the actual connection (fully
//!   closing the DNS-rebinding TOCTOU window, since `wreq` re-resolves) is a
//!   later hardening for the generic transport.

use std::net::{IpAddr, Ipv6Addr};
use url::Url;

/// Reject `url` if its scheme is not http(s) or its host resolves to a blocked
/// address literal / name. Returns the reason on rejection.
pub fn classify_url(url: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|error| format!("invalid url: {error}"))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("unsupported scheme: {other}")),
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| "url has no host".to_string())?;
    let host = host.strip_suffix('.').unwrap_or(host);

    // IP literal (strip brackets around IPv6).
    let literal = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = literal.parse::<IpAddr>() {
        return if is_blocked_ip(ip) {
            Err(format!("blocked address: {ip}"))
        } else {
            Ok(())
        };
    }

    // Well-known private names (full DNS resolution is deferred; see module doc).
    let lowered = host.to_ascii_lowercase();
    if lowered == "localhost"
        || lowered.ends_with(".localhost")
        || lowered == "metadata"
        || lowered == "metadata.google.internal"
    {
        return Err(format!("blocked host name: {host}"));
    }

    Ok(())
}

/// Async companion to [`classify_url`]: runs the sync checks, then — for a real
/// hostname (not an IP literal) — resolves it and rejects if any resolved address
/// is blocked. This closes the name-only gap where a hostname resolves to a
/// private / loopback / metadata IP (e.g. a redirect target under attacker
/// influence). A residual DNS-rebinding TOCTOU window remains because `wreq`
/// re-resolves at connect time; pinning the checked IP is a later hardening.
pub async fn classify_url_resolved(url: &str) -> Result<(), String> {
    classify_url(url)?;

    let parsed = Url::parse(url).map_err(|error| format!("invalid url: {error}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "url has no host".to_string())?;
    let host = host.strip_suffix('.').unwrap_or(host);

    // IP literals were already fully vetted by classify_url — only resolve names.
    let literal = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host);
    if literal.parse::<IpAddr>().is_ok() {
        return Ok(());
    }

    let port = parsed.port_or_known_default().unwrap_or(443);
    let resolved = tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| format!("dns resolution failed for {host}: {error}"))?;
    for addr in resolved {
        if is_blocked_ip(addr.ip()) {
            return Err(format!("blocked address: {host} resolves to {}", addr.ip()));
        }
    }
    Ok(())
}

/// Whether `ip` is in a private / loopback / link-local / metadata / reserved
/// range that must never be fetched.
fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local() // covers 169.254.169.254 cloud metadata
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.octets()[0] == 0 // 0.0.0.0/8
        }
        IpAddr::V6(v6) => {
            // An IPv4-mapped address (`::ffff:a.b.c.d`) reaches IPv4 space — check
            // the embedded v4 against the v4 rules so `[::ffff:10.0.0.1]` is blocked.
            v6.to_ipv4_mapped()
                .is_some_and(|v4| is_blocked_ip(IpAddr::V4(v4)))
                || v6.is_loopback()
                || v6.is_unspecified()
                || is_unique_local(v6) // fc00::/7
                || is_link_local(v6) // fe80::/10
        }
    }
}

fn is_unique_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_public_hosts() {
        assert!(classify_url("https://www.example.org/feed.rss").is_ok());
        assert!(classify_url("https://93.184.216.34/").is_ok()); // example.com public IP
    }

    #[test]
    fn blocks_loopback_and_private_and_metadata() {
        assert!(classify_url("http://127.0.0.1/").is_err());
        assert!(classify_url("http://localhost/").is_err());
        assert!(classify_url("http://10.0.0.5/").is_err());
        assert!(classify_url("http://192.168.1.1/").is_err());
        assert!(classify_url("http://169.254.169.254/latest/meta-data/").is_err());
        assert!(classify_url("http://[::1]/").is_err());
        assert!(classify_url("http://[fe80::1]/").is_err());
        assert!(classify_url("http://[fc00::1]/").is_err());
    }

    #[test]
    fn blocks_ipv4_mapped_ipv6_private_targets() {
        // `::ffff:10.0.0.1` and `::ffff:169.254.169.254` reach private/metadata
        // IPv4 space through the v6 branch — must be blocked, not followed.
        assert!(classify_url("http://[::ffff:10.0.0.1]/").is_err());
        assert!(classify_url("http://[::ffff:169.254.169.254]/").is_err());
        assert!(classify_url("http://[::ffff:127.0.0.1]/").is_err());
        // A mapped *public* address is still allowed.
        assert!(classify_url("http://[::ffff:93.184.216.34]/").is_ok());
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert!(classify_url("file:///etc/passwd").is_err());
        assert!(classify_url("gopher://x/").is_err());
    }

    #[test]
    fn trailing_dot_host_is_still_classified() {
        assert!(classify_url("http://127.0.0.1./").is_err());
    }

    #[test]
    fn resolved_delegates_sync_checks_and_skips_literals() {
        // Offline: literals are vetted synchronously (no DNS lookup), and the
        // well-known-name check still fires. The real A/AAAA-resolution branch is
        // exercised by the live Phase 0 fetch (a public host resolves + passes).
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(rt
            .block_on(classify_url_resolved("https://93.184.216.34/"))
            .is_ok());
        assert!(rt
            .block_on(classify_url_resolved("http://10.0.0.5/"))
            .is_err());
        assert!(rt
            .block_on(classify_url_resolved("http://[::ffff:10.0.0.1]/"))
            .is_err());
        assert!(rt
            .block_on(classify_url_resolved("http://localhost/"))
            .is_err());
    }
}
