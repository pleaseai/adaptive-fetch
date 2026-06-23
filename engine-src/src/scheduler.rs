//! Diversity scheduler — the heart (M2, RFC 0001 §4.5, §4.6).
//!
//! Materializes the grid (url_transforms × tls_impersonate × referer) across the
//! top-3 detected profiles, orders it for diversity (vary TLS *family* fastest),
//! deprioritizes `avoid` targets without deleting them, and is exhaustive by
//! default (R6). On failure it emits the R6 failure gate: `untried_routes` +
//! `must_invoke_playwright_mcp`.
//
// TODO(M2): build_plan() diversity ordering, run loop, failure-gate reporting.
