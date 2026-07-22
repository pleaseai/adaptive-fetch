//! SSRF guard (M1, RFC 0001 §4.7).
//!
//! Rejects a URL whose host is private / loopback / link-local / cloud-metadata
//! before any request is made, and is re-run on every redirect hop by the
//! transport (redirects are followed manually so each hop is re-checked).
//!
//! M1 slice: IP-literal + well-known-name checks. Full DNS-resolution SSRF
//! (resolving a hostname to its A/AAAA records and checking each, closing the
//! DNS-rebinding gap) lands with the generic transport — tracked as a known gap.

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
            v6.is_loopback()
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
    fn rejects_non_http_schemes() {
        assert!(classify_url("file:///etc/passwd").is_err());
        assert!(classify_url("gopher://x/").is_err());
    }

    #[test]
    fn trailing_dot_host_is_still_classified() {
        assert!(classify_url("http://127.0.0.1./").is_err());
    }
}
