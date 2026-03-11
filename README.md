# Jellyfish

Jellyfish is a Rust-based AI personal assistant with native Codex support, local memory, retrieval, and early IM channel integration.

## Current Status

The project has completed the original Phase 0 through Phase 4 assistant milestones and now includes additional native Codex and Feishu/Lark integration work.

Implemented so far:

- Rust workspace scaffold with clear crate boundaries
- native Codex runtime using `~/.codex/auth.json`
- Codex transport modes: `auto`, `sse`, `websocket`
- `codex-cli` fallback provider
- persistent local session storage in `./.jellyfish/session.json`
- memory/profile model with lightweight retrieval
- assistant-first `notes` and `todos` tools
- REPL mode, recall, session inspection, and doctor commands
- progress/summary output and dangerous file-edit confirmation gates
- Feishu/Lark Milestone 1 channel integration through schema + gateway + plugin crates

## Repository Layout

```text
.
├── crates/
│   ├── core/
│   ├── agent/
│   ├── tools/
│   ├── cli/
│   ├── schema/
│   ├── gateway/
│   └── feishu-plugin/
└── docs/
```

## Quick Start

Check runtime status:

```bash
cargo run -p jellyfish-cli -- doctor
```

Run one native Codex-backed assistant turn:

```bash
cargo run -p jellyfish-cli -- chat "你好，简单介绍一下你自己"
```

Start an interactive session:

```bash
cargo run -p jellyfish-cli -- repl
```

Inspect retrieval hits:

```bash
cargo run -p jellyfish-cli -- recall "周日 规划"
```

## Codex Runtime

- default provider profile: `codex`
- default model: `gpt-5.4`
- native endpoint: `https://chatgpt.com/backend-api/codex/responses`
- transport selection via `RIG_CODEX_TRANSPORT=auto|sse|websocket`
- credentials loaded from `~/.codex/auth.json`
- `codex-cli` remains available as a shell-out fallback

## Feishu / Lark Milestone 1

Current Feishu/Lark scope:

- single account
- websocket mode
- text message parsing
- private-message loop working end to end
- group messages gated by `@bot`
- dry-run, probe, and doctor commands

Set environment variables:

```bash
export FEISHU_APP_ID=cli_xxx
export FEISHU_APP_SECRET=xxx
export FEISHU_DOMAIN=feishu   # or lark
export FEISHU_CONNECTION_MODE=websocket
export FEISHU_REQUIRE_MENTION=true
```

Probe and inspect the channel config:

```bash
cargo run -p jellyfish-cli -- channel feishu-doctor
cargo run -p jellyfish-cli -- channel feishu-probe
```

Start the websocket listener:

```bash
cargo run -p jellyfish-cli -- channel feishu-start --bot-open-id ou_xxx
```

Start in dry-run mode without sending replies:

```bash
cargo run -p jellyfish-cli -- channel feishu-start --bot-open-id ou_xxx --dry-run
```

## Documentation

- `docs/vision.md`: product goals and design principles
- `docs/architecture.md`: current crate structure, runtime layers, and channel model
- `docs/roadmap.md`: phased delivery plan and current follow-up work
- `docs/user-guide.md`: user-facing setup and command guide
- `docs/developer-guide.md`: developer-facing runtime, provider, and debugging notes
- `docs/README.md`: docs index

## Next Step

The next implementation targets are:

- native Codex stability and recovery polish
- Feishu/Lark Milestone 2: pairing, allowlist, and group policy
- Phase 5 orchestration work after the single-assistant runtime is considered stable
