//! Playwright fallback routing (M4, RFC 0001 §4.8).
//!
//! Reads a profile's `capabilities_needed` and routes:
//!
//!   * needs_real_tls_stack + needs_js_exec → spawn local node `playwright_real_chrome.js`
//!   * needs_js_exec only                   → set `must_invoke_playwright_mcp` (agent drives MCP)
//!   * needs_mobile_context                 → `playwright_mobile_chrome.js`
//!
//! The engine never drives MCP itself — that is structurally the agent's job (R6).
//
// TODO(M4): capability matching, node subprocess orchestration, cookie bridge-back.
