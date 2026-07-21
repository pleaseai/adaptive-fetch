//! Transport: rquest session pool + warmup + cookie bridge (M1, RFC 0001 §4.7).
//!
//! Per-(host, impersonate) session reuse so WAF sensor cookies mature across
//! attempts; root warmup for deep URLs; browser→curl cookie bridge so one
//! expensive Playwright pass converts into cheap impersonated-HTTP throughput.
//
// TODO(M1): rquest client, SessionPool, warmup(), inject_cookies(), request().
