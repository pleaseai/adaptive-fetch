//! `adaptive-fetch` CLI — the single entrypoint the Claude Code skill invokes.
//!
//! Usage:
//! ```text
//! adaptive-fetch "<URL>" [--selector "<CSS>"]... [--device auto|desktop|mobile] [--trace] [--json]
//! ```
//! Exit code: `0` = validated success, `1` = failure (with `untried_routes`).

use std::process::ExitCode;

use adaptive_fetch::{fetch, DeviceClass, FetchOptions};
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "adaptive-fetch",
    version,
    about = "Resilient site-agnostic page reader — auto-bypasses blocked sites."
)]
struct Cli {
    /// URL to fetch.
    url: String,

    /// CSS selector proving success (repeatable). Strongest positive proof.
    #[arg(long = "selector", value_name = "CSS")]
    selectors: Vec<String>,

    /// Device shaping for the impersonation grid.
    #[arg(long, value_enum, default_value_t = DeviceClass::Auto)]
    device: DeviceClass,

    /// Print the per-attempt trace to stderr.
    #[arg(long)]
    trace: bool,

    /// Emit the full result (metadata + trace) as JSON to stdout.
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let opts = FetchOptions {
        success_selectors: cli.selectors,
        device_class: cli.device,
        ..FetchOptions::default()
    };

    let result = fetch(&cli.url, &opts);

    if cli.json {
        // content is excluded from serialization (see FetchResult::content).
        match serde_json::to_string_pretty(&result) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("adaptive-fetch: failed to serialize result: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        if !result.content.is_empty() {
            print!("{}", result.content);
        }
        eprintln!(
            "verdict={:?} stop={} profile={} attempts={}/{} content={}B",
            result.verdict,
            result.stop_reason,
            result.profile_used.as_deref().unwrap_or("-"),
            result.executed_attempts,
            result.planned_attempts,
            result.content_length(),
        );
        if cli.trace {
            for (i, a) in result.trace.iter().enumerate() {
                eprintln!(
                    "  [{i}] {phase} {exec} {imp} {tf} referer:{rf} -> {verdict:?} ({status})",
                    phase = a.phase,
                    exec = a.executor,
                    imp = a.impersonate.as_deref().unwrap_or("-"),
                    tf = a.url_transform,
                    rf = a.referer,
                    verdict = a.verdict,
                    status = a.status,
                );
            }
        }
        if !result.ok && !result.untried_routes.is_empty() {
            // R6: give-up is never silent — the agent must continue these routes.
            eprintln!("\n\u{26d4} NOT EXHAUSTED — untried routes remain:");
            for route in &result.untried_routes {
                eprintln!("  - {route}");
            }
            if result.must_invoke_playwright_mcp {
                eprintln!("  ! Playwright MCP must be driven from the agent session.");
            }
        }
    }

    if result.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
