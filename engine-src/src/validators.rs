//! 4-layer response validation (M1, RFC 0001 §4.2).
//!
//! Layered AND check producing a [`crate::Verdict`]: status semantics → hard
//! challenge markers → size fingerprint → JSON awareness → caller positive
//! proof (CSS selectors) → no-proof heuristics (soft markers, tiny body,
//! unresolved sensor cookie).
//
// TODO(M1): validate(resp, success_selectors, known_bad_sizes) -> Verdict.
