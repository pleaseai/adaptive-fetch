//! Phase 0 — official public-API router (M3, RFC 0001 §4.3).
//!
//! The SANCTIONED exception to the no-site-name rule: the ONLY module allowed to
//! name platform hosts, and the only one exempt from the bias linter. Tries an
//! official no-auth endpoint BEFORE the generic grid (Reddit→.rss,
//! X→tweet-result/oEmbed/syndication, YouTube→yt-dlp).
//
// TODO(M3): detect(url) -> platform, per-platform routers, route() entrypoint.
