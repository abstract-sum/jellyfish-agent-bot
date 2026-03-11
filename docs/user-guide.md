# Jellyfish User Guide

## 1. What Jellyfish Can Do Today

Jellyfish is a CLI-first personal assistant with local session memory and native Codex support.

Current capabilities:

- chat in a single command
- stay in an interactive REPL session
- remember simple user preferences and notes
- persist local session state in `./.jellyfish/session.json`
- retrieve relevant context from profile, memories, notes, todos, and recent conversation
- manage local `notes` and `todos` through the assistant runtime
- use native Codex transport with `auto`, `sse`, or `websocket`
- fall back to `codex-cli` or `mock` providers when needed

## 2. Requirements

You need:

- Rust toolchain
- a valid workspace directory
- one usable provider configuration

Supported provider modes:

- `codex`: native Codex backend using `~/.codex/auth.json`
- `codex-cli`: local `codex` CLI backend
- `openai`: OpenAI-compatible API key backend
- `mock`: local offline verification mode

## 3. Configuration

Copy `.env.example` values into your environment as needed.

Important variables:

- `RIG_PROVIDER`
- `RIG_MODEL`
- `RIG_WORKSPACE_ROOT`
- `RIG_LOG`
- `RIG_ENABLE_REPO_TOOLS`
- `RIG_ALLOW_FILE_EDITS`
- `RIG_TOOL_TIMEOUT_SECS`
- `RIG_TOOL_OUTPUT_MAX_CHARS`
- `RIG_CODEX_TRANSPORT`
- `OPENAI_API_KEY` for `openai`

Default native Codex profile:

```bash
RIG_PROVIDER=codex
RIG_MODEL=gpt-5.4
RIG_CODEX_TRANSPORT=auto
```

## 4. Startup And Health Checks

Check current runtime status:

```bash
cargo run -p jellyfish-cli -- doctor
```

The doctor command reports:

- active provider
- active model
- credential readiness
- retrieval entry count
- Codex transport mode
- session file path

## 5. Basic Chat

Run one assistant turn:

```bash
cargo run -p jellyfish-cli -- chat "你好，简单介绍一下你自己"
```

Run a multi-turn interactive session:

```bash
cargo run -p jellyfish-cli -- repl
```

Exit REPL with:

- `exit`
- `quit`

## 6. Session And Memory

Jellyfish stores local session state in:

```text
./.jellyfish/session.json
```

Show current session:

```bash
cargo run -p jellyfish-cli -- session show
```

Reset current session:

```bash
cargo run -p jellyfish-cli -- session reset
```

Simple memory phrases supported today:

- `记住：我每周日晚上做下周规划`
- `我叫 Yvonne`
- `我的时区是 Asia/Shanghai`
- `我的语言是 zh-CN`
- `我的偏好是 tone=concise`
- `我的任务是 整理本周回顾`

## 7. Retrieval

Jellyfish builds a local retrieval snapshot from:

- profile fields
- saved memories
- notes
- todos
- recent conversation messages

Inspect retrieval hits manually:

```bash
cargo run -p jellyfish-cli -- recall "周日 规划"
```

Use retrieval in conversation:

```bash
cargo run -p jellyfish-cli -- chat "结合我之前的规划习惯给我建议"
```

## 8. Providers

### Native Codex

Default mode.

Requirements:

- `~/.codex/auth.json` present
- valid Codex OAuth credentials

Example:

```bash
cargo run -p jellyfish-cli -- chat "Hello from native Codex"
```

Transport options:

```bash
RIG_CODEX_TRANSPORT=auto cargo run -p jellyfish-cli -- chat "Hello"
RIG_CODEX_TRANSPORT=sse cargo run -p jellyfish-cli -- chat "Hello"
RIG_CODEX_TRANSPORT=websocket cargo run -p jellyfish-cli -- chat "Hello"
```

### Codex CLI Fallback

Use this if you want Jellyfish to delegate requests to the installed `codex` binary.

```bash
RIG_PROVIDER=codex-cli cargo run -p jellyfish-cli -- chat "Hello from codex-cli"
```

### OpenAI-Compatible API

```bash
OPENAI_API_KEY=... RIG_PROVIDER=openai cargo run -p jellyfish-cli -- chat "Hello from OpenAI"
```

### Mock Mode

Useful for local checks without network calls.

```bash
RIG_PROVIDER=mock cargo run -p jellyfish-cli -- chat "帮我整理今天的重点"
```

## 9. Repository Tools And File Editing

By default, Jellyfish runs as a personal assistant first.

Repository tools are optional:

```bash
RIG_ENABLE_REPO_TOOLS=true cargo run -p jellyfish-cli -- doctor
```

Dangerous file edits are blocked unless explicitly enabled:

```bash
cargo run -p jellyfish-cli -- chat --yes "请修改某个文件"
```

Or:

```bash
RIG_ALLOW_FILE_EDITS=true cargo run -p jellyfish-cli -- chat "请修改某个文件"
```

## 10. Test Commands

Recommended checks:

```bash
cargo check
cargo test
cargo test -p jellyfish-agent
```

Native Codex parsing tests:

```bash
cargo test -p jellyfish-agent extracts_account_id_from_access_token_claim
cargo test -p jellyfish-agent parses_output_text_deltas_into_final_message
cargo test -p jellyfish-agent parses_websocket_json_events_into_final_message
```

## 11. Troubleshooting

If `doctor` says credentials are missing:

- check `~/.codex/auth.json`
- verify provider selection with `RIG_PROVIDER`
- verify `OPENAI_API_KEY` for `openai`

If transport issues happen on Codex:

- try `RIG_CODEX_TRANSPORT=sse`
- then try `RIG_CODEX_TRANSPORT=websocket`
- use `doctor` to confirm current mode

If you want a safe local dry run:

- switch to `RIG_PROVIDER=mock`
