//! SSRF guard (M1, RFC 0001 §4.7).
//!
//! Validates the initial URL **and every redirect hop** against a
//! private/loopback/link-local/cloud-metadata block-list before any request is
//! made. Redirects are followed manually so each hop is re-checked.
//
// TODO(M1): classify_url(), redirect resolution, IP/CIDR block-list (ipnet).
