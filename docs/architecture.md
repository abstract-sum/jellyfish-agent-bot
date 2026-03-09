# OpenClaw Architecture

## Overview

OpenClaw is organized as a Rust workspace with clear module boundaries.

The architecture is designed to support gradual delivery:

- common domain types live in a shared crate
- runtime logic lives in a dedicated agent crate
- tool contracts and implementations live in a tools crate
- local execution starts from a CLI crate

This keeps the codebase extensible without forcing early coupling between user interface, runtime, and execution details.

## Workspace Layout

```text
.
├── Cargo.toml
├── rust-toolchain.toml
├── .env.example
├── crates/
│   ├── core/
│   ├── agent/
│   ├── tools/
│   └── cli/
└── docs/
```

## Crate Responsibilities

### `crates/core`

Purpose:

- host shared domain types
- define reusable configuration and error models
- centralize session and event structures

Responsibilities:

- application configuration
- shared result and error types
- session state model
- event model
- shared identifiers and enums

Current files:

- `config.rs`
- `error.rs`
- `event.rs`
- `session.rs`
- `types.rs`

Why it exists:

This crate prevents CLI, agent, and tool layers from redefining the same concepts in incompatible ways.

### `crates/agent`

Purpose:

- host the agent runtime abstraction
- own prompt construction and later Rig integration
- normalize request/response handling

Responsibilities:

- agent runtime trait
- runtime request and response types
- prompt templates
- provider bootstrap in later phases
- execution loop orchestration in later phases

Current status:

- contains a stub runtime only
- does not yet call a real model provider

Why it exists:

This crate isolates model-specific logic from both the CLI and the tool layer.

### `crates/tools`

Purpose:

- define the tool contract used by the runtime
- provide registry and discovery for tools

Responsibilities:

- tool trait definition
- tool metadata and schema
- tool output shape
- registry for future tool lookup and dispatch

Current status:

- abstractions only
- no real file or command tools yet

Why it exists:

This crate allows tool capability to grow independently from runtime and interface concerns.

### `crates/cli`

Purpose:

- provide the local executable entrypoint
- support developer validation during early phases

Responsibilities:

- argument parsing
- command dispatch
- output formatting
- bootstrap logging and config loading

Current status:

- supports a minimal scaffold with `chat` and `doctor`

Why it exists:

This crate keeps terminal interaction separate from agent runtime implementation.

## Dependency Boundaries

The intended dependency direction is:

```text
cli -> agent -> core
cli -> core
agent -> tools -> core
```

Constraints:

- `core` should not depend on `agent`, `tools`, or `cli`
- `tools` should depend on `core`, but not on `cli`
- `agent` may depend on `tools` and `core`, but should not depend on `cli`
- `cli` may depend on all runtime-facing crates, but should remain thin

## Runtime Shape

The target runtime flow is:

1. receive user input
2. load session context
3. construct prompt and runtime request
4. invoke model
5. process tool calls if needed
6. update session and event stream
7. render result to the user

Phase 0 only implements a stub version of this flow. Real model execution and real tool invocation begin in Phase 1.

## Session And Event Model

The shared data model should support future extension without changing the outer structure.

### Session

Session is responsible for:

- stable session identity
- ordered message history
- ordered event history

This gives later phases a place to attach persistence, replay, and observability without redesigning the basic state model.

### Event

The event layer is intended to represent:

- user messages
- agent messages
- tool calls
- tool results
- system events

This is important because later phases need clearer progress output and better execution tracing.

## Configuration Strategy

Configuration currently starts from simple defaults and environment variables.

The early config model should remain small:

- provider kind
- model name
- workspace root
- log filter

This is enough for Phase 1 while still leaving room for later expansion into timeout settings, tool permissions, and persistence configuration.

## Architectural Non-Goals For Early Phases

The architecture intentionally avoids the following in the early stages:

- plugin over-engineering
- distributed runtime coordination
- complex policy engines
- heavy persistence integration before the core loop is stable
- UI-specific data modeling in shared crates

## Phase 0 Outcome

Phase 0 establishes the structural foundation for the project:

- compileable workspace
- stable crate boundaries
- shared domain model
- stub runtime
- tool registry abstraction
- CLI scaffold

This foundation is considered successful because future phases can now add real capability without reorganizing the repository.
