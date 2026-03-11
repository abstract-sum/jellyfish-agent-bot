# Jellyfish Architecture

## Overview

Jellyfish is a Rust workspace organized around four primary concerns:

- assistant runtime and providers
- local tools and memory
- user-facing CLI workflows
- channel integration through a gateway and plugin model

The architecture is designed to keep the personal assistant core independent from any one transport or channel while still making provider- and channel-specific behavior explicit.

## Workspace Layout

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

## Crate Responsibilities

### `crates/core`

Shared domain types and stable cross-crate contracts.

Responsibilities:

- application configuration
- provider and transport enums
- shared error model
- session model
- event model
- memory and profile structures

Key files:

- `config.rs`
- `types.rs`
- `error.rs`
- `event.rs`
- `session.rs`
- `memory.rs`

### `crates/agent`

Assistant runtime and provider-specific execution logic.

Responsibilities:

- runtime selection via `build_runtime(...)`
- prompt construction
- tool loop orchestration
- native Codex auth and transport handling
- OpenAI-compatible and mock provider paths

Key files:

- `runtime.rs`
- `prompt.rs`
- `codex_auth.rs`
- `codex_runtime.rs`
- `codex_cli.rs`

### `crates/tools`

Local tool abstractions and implementations.

Responsibilities:

- tool trait and registry
- assistant-first tools like `notes` and `todos`
- optional repo tools like `read`, `glob`, `grep`, and `apply_patch`

### `crates/cli`

User-facing command layer.

Responsibilities:

- command parsing
- session and retrieval wiring
- user-facing output
- Feishu channel commands

Important commands today:

- `chat`
- `repl`
- `doctor`
- `session show/reset`
- `recall`
- `channel feishu-probe`
- `channel feishu-doctor`
- `channel feishu-start`

### `crates/schema`

Channel-agnostic message schema for IM and future transport integrations.

Responsibilities:

- `InboundMessage`
- `OutboundMessage`
- `ChannelKind`
- `PeerKind`
- `SessionLocator`

This crate is the shared boundary between channel adapters and the assistant gateway.

### `crates/gateway`

Bridges channel messages into the Jellyfish runtime.

Responsibilities:

- convert `InboundMessage` into a routed assistant request
- derive channel-scoped session keys
- load and persist channel-scoped sessions
- invoke the assistant runtime and package replies into `OutboundMessage`

### `crates/feishu-plugin`

Feishu/Lark channel integration crate.

Responsibilities:

- Feishu/Lark config parsing
- websocket startup
- event parsing
- mention gating
- message sending
- probe/doctor support

Current status:

- Milestone 1 implemented
- private-message loop validated end to end
- group behavior still limited to mention gating
- webhook, media, and richer policy layers are not yet implemented

## Dependency Boundaries

The intended dependency direction is:

```text
cli -> agent -> core
cli -> gateway -> agent/core/schema
cli -> feishu-plugin -> gateway/schema/core
tools -> core
agent -> tools -> core
gateway -> schema
feishu-plugin -> schema
```

Constraints:

- `core` should not depend on higher-level crates
- `schema` should remain channel/runtime neutral
- `gateway` should not own provider-specific logic
- channel plugins should not own assistant memory or provider behavior
- `cli` remains the orchestration shell, not the business-logic center

## Runtime Shape

The assistant runtime flow is:

1. receive user or channel input
2. load session and user context
3. construct prompt and retrieval context
4. invoke the selected provider
5. execute tools if the model requests them
6. update session and event history
7. render or route the final response

For native Codex, the provider-specific transport flow is:

1. load credentials from `~/.codex/auth.json`
2. extract `chatgpt_account_id`
3. refresh the token if needed
4. select `auto`, `sse`, or `websocket`
5. send request to `chatgpt.com/backend-api/codex/responses`
6. parse streamed events into assistant text

## Memory And Retrieval

Jellyfish currently uses a local lightweight memory model.

### Session Memory

Stored in `./.jellyfish/session.json` and related channel-scoped files.

Contains:

- user profile
- memories
- message history
- event history

### Retrieval Snapshot

Built from:

- user profile
- memory entries
- notes
- todos
- recent messages

The snapshot is used to create `retrieval_context` for assistant requests.

## Channel Model

The current channel architecture is intentionally lightweight but already follows a plugin-friendly shape.

Current layers:

- `schema`: stable message DTOs
- `gateway`: routing and session bridge
- `feishu-plugin`: first concrete channel implementation

This makes it possible to add more channels later without pushing channel-specific logic into the assistant runtime.

## Feishu / Lark Milestone 1 Scope

Currently implemented:

- websocket mode
- private-message text handling
- mention-gated group handling
- dry-run mode
- channel probe and doctor commands
- duplicate inbound message suppression by `message_id`

Not yet implemented:

- webhook mode
- pairing and allowlist
- group policy variations
- media handling
- rich cards and streaming cards
- multi-account channel config
- Feishu platform tool packs

## Current Follow-Up Work

- improve native Codex tool-calling stability across multi-turn loops
- refine native Codex recovery and provider-specific errors
- complete Feishu Milestone 2 policy layers
- decide when to formalize plugin metadata and discovery
