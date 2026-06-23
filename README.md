# adaptive-fetch

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

## Repository layout

```
engine-src/            Rust crate (the engine binary)
skills/adaptive-fetch/ Claude Code skill (SKILL.md, references, profiles, templates)
docs/rfcs/             design RFCs
.claude-plugin/        plugin manifest
```

## Development

```bash
mise install      # rust + bun toolchain (see mise.toml)
mise run check    # fmt-check + clippy + test (mirrors CI)
cargo run -p adaptive-fetch -- "https://example.com" --json
```

## License

MIT
