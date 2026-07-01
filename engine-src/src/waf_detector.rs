//! Ranked WAF-product detection (M2, RFC 0001 §4.4).
//!
//! Scores each profile in `engine/waf_profiles.yaml` against the live response
//! (cookies / headers / server / body markers) and returns a ranked list of
//! `(profile_id, confidence)` — never a single verdict (a wrong single guess
//! cascades into a wrong plan).
//
// TODO(M2): load profiles (serde_yaml), detect() ranking, unknown_challenge net.
