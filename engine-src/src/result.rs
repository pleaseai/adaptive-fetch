//! Public result + verdict types — the stable engine contract.
//!
//! The field set mirrors insane-search's `FetchResult` so the `SKILL.md`
//! harness rules (notably the R6 failure gate) port verbatim. See
//! `docs/rfcs/0001-adaptive-fetch.md` §3, §4.2, §4.6.

use serde::{Deserialize, Serialize};

/// Classification of a fetched response.
///
/// HTTP 200 is the *start* of inspection, not success (RFC 0001 §4.2): a
/// response is only a success once it clears the layered validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Caller-supplied positive proof matched → terminal success.
    StrongOk,
    /// Clean response, no negative signal → terminal success.
    WeakOk,
    /// Ambiguous (unresolved sensor cookie / soft marker) → NON-terminal:
    /// kept as best-effort while the grid keeps searching for real proof.
    SuspectOk,
    /// WAF challenge (negative proof).
    Challenge,
    /// Generic non-2xx block.
    Blocked,
    /// 429 — back off, do not hammer. Transient, NOT a terminal wall.
    RateLimited,
    /// 401/407 — terminal; retrying TLS cannot help.
    AuthRequired,
    /// 404/410 — terminal.
    NotFound,
    /// Exception / dependency missing / unscored.
    Unknown,
}

impl Verdict {
    /// Terminal success only. `SuspectOk` is intentionally excluded.
    pub fn is_ok(self) -> bool {
        matches!(self, Verdict::StrongOk | Verdict::WeakOk)
    }

    /// Truly terminal — no bypass route can recover this resource, so give up.
    /// `RateLimited` is deliberately **excluded**: a 429 is transient (back off
    /// and retry), not a wall. To decide whether to stop the current TLS grid,
    /// use [`Verdict::is_grid_stop`] instead.
    pub fn is_terminal_nonsuccess(self) -> bool {
        matches!(self, Verdict::AuthRequired | Verdict::NotFound)
    }

    /// Stop the current TLS grid — more handshakes won't help *right now*. This
    /// is the terminal set plus `RateLimited`: a 429 halts the grid so we don't
    /// hammer, but the failure gate still surfaces it as a transient
    /// back-off-and-retry route, not a give-up (RFC 0001 §4.6).
    pub fn is_grid_stop(self) -> bool {
        self.is_terminal_nonsuccess() || matches!(self, Verdict::RateLimited)
    }
}

/// One attempt in the trace: a (transform × impersonate × referer × executor)
/// combination and how the response was judged. Exposed so callers can diagnose
/// a failure without re-running.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    /// `probe` | `grid` | `fallback` | `phase0`.
    pub phase: String,
    /// `rquest` | `playwright_real_chrome` | `phase0:<route>` | ...
    pub executor: String,
    pub url: String,
    /// `original` | `mobile_subdomain` | `am_prefix` | `drop_www` | `-`.
    pub url_transform: String,
    /// TLS impersonation target (None for non-curl executors).
    pub impersonate: Option<String>,
    pub referer: String,
    pub status: u16,
    pub body_size: usize,
    pub verdict: Verdict,
    pub reasons: Vec<String>,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

/// The single value [`crate::fetch`] returns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResult {
    pub ok: bool,
    /// Fetched body. Excluded from `--json` via `#[serde(skip)]`.
    /// TODO(M1): add a serialized `content_length` field so JSON consumers
    /// can inspect body size without the full text bloating the trace output.
    #[serde(skip)]
    pub content: String,
    pub final_url: String,
    pub verdict: Verdict,
    pub profile_used: Option<String>,
    pub trace: Vec<Attempt>,
    pub summary: String,
    // --- scheduler diagnostics (RFC 0001 §4.5) ---
    pub planned_attempts: u32,
    pub executed_attempts: u32,
    pub grid_exhausted: bool,
    /// `success` | `exhausted` | `budget` | `<terminal verdict>` | `error`.
    pub stop_reason: String,
    // --- failure gate (R6, RFC 0001 §4.6) ---
    /// When `ok == false`, the escalation routes the engine could not perform
    /// itself — so the caller never mistakes give-up for "everything was tried".
    pub untried_routes: Vec<String>,
    /// Playwright MCP can only be driven from the agent session, so it is, by
    /// construction, an untried route the engine cannot perform.
    pub must_invoke_playwright_mcp: bool,
}

impl FetchResult {
    /// Length of the fetched body in bytes (content itself is excluded from
    /// serialization — see the `content` field).
    pub fn content_length(&self) -> usize {
        self.content.len()
    }
}
