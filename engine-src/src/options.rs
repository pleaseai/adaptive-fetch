//! Caller-facing fetch options (RFC 0001 §3, §4.5).
//!
//! Site knowledge enters the engine only through these options
//! (`success_selectors`, `user_hint`) — never through hard-coded site logic.

use clap::ValueEnum;

/// Device shaping for the impersonation grid (RFC 0001 §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
#[value(rename_all = "lower")]
pub enum DeviceClass {
    /// Follow the detected profile's strategy.
    #[default]
    Auto,
    /// Desktop TLS only; disable `m.` subdomain transforms.
    Desktop,
    /// Mobile TLS only; enable `m.` subdomain transforms.
    Mobile,
}

/// Optional runtime hints. Applied to the current call only — never persisted
/// by the engine (the self-learning store in `learning.rs` is separate).
#[derive(Debug, Clone, Default)]
pub struct UserHint {
    /// TLS impersonation target to try first (e.g. `safari_ios`, `chrome`).
    pub impersonate_first: Option<String>,
    /// Referer strategy override (`self_root` | `google_search` | `none`).
    pub referer_strategy: Option<String>,
}

/// Inputs to [`crate::fetch`].
#[derive(Debug, Clone)]
pub struct FetchOptions {
    /// CSS selectors proving a real page rendered (strongest positive proof).
    pub success_selectors: Vec<String>,
    pub device_class: DeviceClass,
    pub user_hint: Option<UserHint>,
    pub timeout_secs: u64,
    /// `None` = exhaustive (honours R6); `Some(n)` = total attempt budget.
    pub max_attempts: Option<u32>,
    pub enable_playwright: bool,
    pub enable_phase0: bool,
    pub enable_learning: bool,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            success_selectors: Vec::new(),
            device_class: DeviceClass::Auto,
            user_hint: None,
            timeout_secs: 25,
            max_attempts: None, // exhaustive by default (R6)
            enable_playwright: true,
            enable_phase0: true,
            enable_learning: true,
        }
    }
}
