# adaptive-fetch

[![CI](https://github.com/pleaseai/adaptive-fetch/actions/workflows/ci.yml/badge.svg)](https://github.com/pleaseai/adaptive-fetch/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

A resilient, **site-agnostic** public-page reader for Claude Code. When a fetch
is blocked (402 / 403 / WAF / CAPTCHA), `adaptive-fetch` automatically tries every
bypass strategy until one works — no API keys, no proxy setup.

Inspired by [`fivetaku/insane-search`](https://github.com/fivetaku/insane-search),
with the engine written in **Rust** so the core capability — browser TLS
fingerprint impersonation — runs natively, in-process, as a single static binary.

> 🚧 **Status: M0 scaffold.** The CLI, plugin packaging, and toolchain are in
> place; the engine returns an honest "not implemented" result for now. The
> network stages land across milestones M1–M6. See the design RFC.

## Design

[`docs/rfcs/0001-adaptive-fetch.md`](docs/rfcs/0001-adaptive-fetch.md) — architecture,
invariants, and the milestone plan.

## Platforms

[`PLATFORMS.md`](PLATFORMS.md) — the planned coverage map: which platforms and
routes each milestone (M1–M6) unlocks. Most sites need no explicit entry; only the
Phase 0 official-API endpoints (Reddit, X, YouTube, …) are indexed.

## Repository layout

```
engine-src/            Rust crate (the engine binary)
skills/adaptive-fetch/ Claude Code skill (SKILL.md, references, profiles, templates)
docs/rfcs/             design RFCs
.claude-plugin/        plugin manifest
```

## Development

The engine links `wreq` → BoringSSL (`boring-sys2`), which **compiles native code
at build time**. You need a C/C++ toolchain plus **CMake** and **libclang**
(bindgen) on `PATH`; without them the build fails before compiling the crate.

```bash
# macOS:        xcode-select --install && brew install cmake llvm
# Debian/Ubuntu: apt-get install -y build-essential cmake clang libclang-dev
```

```bash
mise install      # rust + bun toolchain (see mise.toml)
mise run check    # fmt-check + clippy + test (mirrors CI)
cargo run -p adaptive-fetch -- "https://example.com" --json
```

## License

MIT
