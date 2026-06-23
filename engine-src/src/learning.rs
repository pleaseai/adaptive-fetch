//! Self-learning: per-host winning-route store (M5, RFC 0001 §4.9).
//!
//! A bounded, self-pruning JSON store mapping (host, device_class) → last winning
//! route {transform, impersonate, referer}. Promoted to the front of the next
//! grid; evicted after two consecutive *real* failures. Any store error is
//! swallowed — learning can never break a fetch (ADAPTIVE_FETCH_LEARN=0 disables).
//
// TODO(M5): lookup(), record_success(), record_failure(), eviction, store I/O.
