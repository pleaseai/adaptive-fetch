//! `adaptive-fetch` CLI — the single entrypoint the Claude Code skill invokes.
//!
//! Usage:
//! ```text
//! adaptive-fetch "<URL>" [--selector "<CSS>"]... [--device auto|desktop|mobile] [--trace] [--json]
//! adaptive-fetch check-url "<URL>" [--presets P] [--json]
//! ```
//! Fetch exit code: `0` = validated success, `1` = failure (with `untried_routes`).
//! `check-url` exit code: `10` = preset matched, `0` = no match or fail-soft error.

use std::path::PathBuf;
use std::process::ExitCode;

use adaptive_fetch::{fetch, presets, DeviceClass, FetchOptions};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "adaptive-fetch",
    version,
    about = "Resilient site-agnostic page reader — auto-bypasses blocked sites.",
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
struct Cli {
    /// URL to fetch.
    #[arg(value_name = "URL")]
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

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Check a URL against url_presets.toml (used by the WebFetch PreToolUse hook).
    CheckUrl {
        /// URL to check.
        url: String,
        /// Path to url_presets.toml (falls back to $ADAPTIVE_FETCH_PRESETS).
        #[arg(long)]
        presets: Option<PathBuf>,
        /// Emit the match result as JSON.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let cli = parse_cli();

    match cli.command {
        Some(Command::CheckUrl { url, presets, json }) => run_check_url(&url, presets, json),
        None => run_fetch(cli),
    }
}

fn parse_cli() -> Cli {
    let mut matches = Cli::command().get_matches();

    if matches.subcommand_name().is_some() {
        let command =
            Command::from_arg_matches_mut(&mut matches).unwrap_or_else(|error| error.exit());
        return Cli {
            url: String::new(),
            selectors: Vec::new(),
            device: DeviceClass::Auto,
            trace: false,
            json: false,
            command: Some(command),
        };
    }

    Cli::from_arg_matches_mut(&mut matches).unwrap_or_else(|error| error.exit())
}

fn run_fetch(cli: Cli) -> ExitCode {
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
            // RFC 0001 §4.6 separates (a) grid not exhausted → re-run exhaustive,
            // from (b) grid exhausted → re-running won't help; the listed routes
            // (e.g. Playwright MCP) are the only remaining escalation.
            if result.grid_exhausted {
                eprintln!("\n\u{26d4} GRID EXHAUSTED — escalation routes remain:");
            } else {
                eprintln!("\n\u{26d4} NOT EXHAUSTED — untried routes remain:");
            }
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

fn run_check_url(url: &str, presets: Option<PathBuf>, json: bool) -> ExitCode {
    let presets_path = presets.or_else(|| {
        std::env::var("ADAPTIVE_FETCH_PRESETS")
            .ok()
            .map(PathBuf::from)
    });
    let matched = presets_path.and_then(|path| match presets::load(&path) {
        Ok(file) => file.match_url(url).cloned(),
        Err(error) => {
            eprintln!("adaptive-fetch check-url: {error}");
            None
        }
    });

    let Some(preset) = matched else {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({ "matched": false }))
                    .expect("static match result should serialize")
            );
        }
        return ExitCode::SUCCESS;
    };

    let suggested_command = preset.suggested_command(url);
    if json {
        let output = serde_json::json!({
            "matched": true,
            "engine_ready": adaptive_fetch::ENGINE_READY,
            "reason": preset.reason,
            "device": preset.device,
            "selectors": preset.selectors,
            "impersonate_first": preset.impersonate_first,
            "referer_strategy": preset.referer_strategy,
            "suggested_command": suggested_command,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&output).expect("preset match result should serialize")
        );
    } else {
        println!("{suggested_command}");
        if let Some(reason) = &preset.reason {
            eprintln!("{reason}");
        }
    }

    ExitCode::from(10)
}
