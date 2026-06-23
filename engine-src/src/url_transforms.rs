//! Generic URL transforms for the fetch grid (M2, RFC 0001 §4.5).
//!
//! Domain-agnostic *rules* (`original`, `mobile_subdomain`, `am_prefix`,
//! `drop_www`) — never a site name. Each transform applies or is skipped.
//
// TODO(M2): apply_transform(), iter_transformed() with dedup.
