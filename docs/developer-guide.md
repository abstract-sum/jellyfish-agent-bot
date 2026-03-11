# Jellyfish Developer Guide

## 1. Purpose

This guide explains how Jellyfish is organized internally and how to work on the assistant runtime, providers, tools, memory, and debugging flow.

Use this guide when you want to:

- understand crate boundaries
- add or modify a provider
- debug native Codex behavior
- add tools or assistant capabilities
- verify runtime and retrieval behavior during development

## 2. Workspace Layout

The repository is organized as a Rust workspace:

```text
.
├── crates/
│   ├── core/
│   ├── agent/
│   ├── tools/
│   └── cli/
└── docs/
```

### `crates/core`

Shared domain types and configuration.

Important modules:

- `config.rs`: application configuration from environment
- `types.rs`: provider and transport enums
- `event.rs`: assistant and tool lifecycle events
- `session.rs`: session state, messages, memories, profile helpers
- `memory.rs`: memory entry and profile models

### `crates/agent`

Assistant runtime, provider bootstrapping, prompt construction, and provider-specific request flows.

Important modules:

- `runtime.rs`: runtime selection and main assistant loop
- `prompt.rs`: system prompt templates
- `codex_auth.rs`: native Codex OAuth cache parsing and refresh
- `codex_runtime.rs`: native Codex SSE / WebSocket transport
- `codex_cli.rs`: shell-out fallback backend

### `crates/tools`

Local tool abstractions and tool implementations.

Important modules:

- `traits.rs`: tool trait and outputs
- `registry.rs`: tool registration and dispatch
- `builtin.rs`: read, glob, grep, apply_patch, notes, todos

### `crates/cli`

User-facing command layer.

Important modules:

- `main.rs`: command dispatch and runtime wiring
- `args.rs`: CLI command definitions
- `output.rs`: user-facing output formatting
- `memory.rs`: simple phrase-based memory capture
- `retrieval.rs`: retrieval snapshot and local ranking
- `session_store.rs`: session persistence to `./.jellyfish/session.json`

## 3. Runtime Modes

Jellyfish currently supports these provider modes:

- `codex`: native Codex backend
- `codex-cli`: fallback shell-out backend
- `openai`: OpenAI-compatible Rig backend
- `mock`: offline local backend

Runtime selection happens in `crates/agent/src/runtime.rs` via `build_runtime(...)`.

### Native Codex

Native Codex is the default provider.

Key pieces:

- credentials loaded from `~/.codex/auth.json`
- `account_id` extracted from the access token
- refresh via `https://auth.openai.com/oauth/token`
- model requests sent to `https://chatgpt.com/backend-api/codex/responses`
- transport modes:
  - `auto`
  - `sse`
  - `websocket`

### Codex CLI

Fallback path that shells out to the local `codex` executable.

This is useful when:

- you want to compare native vs CLI behavior
- native Codex transport is under investigation
- the local Codex CLI path is easier to verify on a machine

### OpenAI-Compatible Rig Runtime

This path remains useful for non-Codex models and for keeping the assistant abstraction aligned with Rig.

### Mock Runtime

Use this for offline development and quick behavior checks without real model traffic.

## 4. Native Codex Request Flow

The native Codex runtime currently follows this path:

1. load credentials from `~/.codex/auth.json`
2. extract `chatgpt_account_id`
3. refresh the token if close to expiry
4. choose transport according to `RIG_CODEX_TRANSPORT`
5. send request via WebSocket or SSE
6. parse incremental text events into the final assistant message

Relevant files:

- `crates/agent/src/codex_auth.rs`
- `crates/agent/src/codex_runtime.rs`

## 5. Assistant Loop

The main assistant loop lives in `crates/agent/src/runtime.rs`.

For `openai` and `codex`, the loop can:

- build a step prompt with memory, retrieval, and tool metadata
- ask the model for the next action in JSON
- execute local tools
- feed tool results back into the next turn
- return a final assistant message

The loop is bounded by `MAX_TOOL_TURNS`.

Current step contract:

- `respond`: final text answer
- `tool`: tool call request

## 6. Memory And Retrieval

Jellyfish memory is split across two layers.

### Session memory

Stored in `./.jellyfish/session.json`.

Contains:

- user profile
- memories
- message history
- events

### Retrieval snapshot

Built in `crates/cli/src/retrieval.rs`.

Sources:

- profile
- memory entries
- notes
- todos
- recent messages

The snapshot is used to produce `retrieval_context` for the runtime request.

## 7. Tools

Tool registration happens inside provider runtimes.

Common tool groups:

- assistant-first tools:
  - `notes`
  - `todos`
- optional repo tools:
  - `read`
  - `glob`
  - `grep`
  - `apply_patch`

Repo tools are controlled by:

- `RIG_ENABLE_REPO_TOOLS`
- `RIG_ALLOW_FILE_EDITS`

## 8. Important Environment Variables

- `RIG_PROVIDER`
- `RIG_MODEL`
- `RIG_WORKSPACE_ROOT`
- `RIG_LOG`
- `RIG_ENABLE_REPO_TOOLS`
- `RIG_ALLOW_FILE_EDITS`
- `RIG_TOOL_TIMEOUT_SECS`
- `RIG_TOOL_OUTPUT_MAX_CHARS`
- `RIG_CODEX_TRANSPORT`
- `OPENAI_API_KEY`

## 9. Development Commands

General checks:

```bash
cargo check
cargo test
```

Agent-focused tests:

```bash
cargo test -p jellyfish-agent
```

CLI checks:

```bash
cargo run -p jellyfish-cli -- doctor
cargo run -p jellyfish-cli -- chat "Hello"
```

Transport checks:

```bash
RIG_CODEX_TRANSPORT=sse cargo run -p jellyfish-cli -- chat "Hello from SSE"
RIG_CODEX_TRANSPORT=websocket cargo run -p jellyfish-cli -- chat "Hello from WebSocket"
```

Retrieval checks:

```bash
cargo run -p jellyfish-cli -- recall "周日 规划"
```

## 10. Debugging Tips

### If native Codex fails

Check:

- `~/.codex/auth.json` exists
- `doctor` reports `Codex native credentials ready: true`
- try `RIG_CODEX_TRANSPORT=sse`
- then try `RIG_CODEX_TRANSPORT=websocket`

### If tool calls behave strangely

Check:

- whether the provider runtime actually registered the tool
- whether repo tools are enabled
- whether file edits were explicitly allowed
- whether the model returned malformed JSON instead of the expected step format

### If retrieval seems weak

Check:

- whether memories were actually written into the session
- whether `recall` shows the expected hits
- whether the request contains enough matching terms for the local ranking logic

## 11. Current Known Follow-Up Work

- improve native Codex tool-calling stability across multi-turn loops
- refine failure recovery and provider-specific error messaging for native Codex requests
- continue Phase 5 multi-agent exploration after the single-assistant runtime is considered stable
