# OpenClaw Vision

## Project Goal

OpenClaw is a Rig-based coding agent project implemented in phases.

The goal is to build a practical AI coding assistant on a stable Rust foundation:

- start from the smallest useful workflow
- validate the core agent loop first
- expand gradually into repository understanding, tool execution, memory, and orchestration

## Product Direction

The current product direction is:

- CLI-first
- single-agent first
- tool-enabled coding workflow
- incremental delivery by phase

The first useful workflow is:

user request -> agent reasoning -> tool usage -> code modification -> validation -> result summary

## Product Principles

### Build the core loop first

The project should focus on the minimum workflow that solves a real problem before adding platform breadth.

### Keep module boundaries stable

The CLI should not own core business logic. Shared domain models should live in common crates and be reused by runtime and tool layers.

### Grow in layers

The system should evolve in this order:

1. compileable skeleton
2. real single-agent runtime
3. real tools
4. stable execution and session handling
5. retrieval and memory
6. multi-agent orchestration
7. API and UI

### Avoid premature complexity

Complex capabilities should only be added when the previous layer is already reliable.

## Current Scope Decisions

The following decisions are already confirmed:

- product type: Rig-based AI coding agent platform
- development order: core functionality first, then gradual expansion
- interaction model: no Plan Mode / Execute Mode split for now
- interface priority: CLI before API or web UI
- runtime strategy: single-agent first

## Deferred Items

The following items are intentionally deferred:

- Plan Mode / Execute Mode
- full permission sandbox
- long-term memory store
- PR automation
- browser UI
- distributed worker model

## Open Decisions For Later

The following decisions can be made in later phases:

- first production provider beyond the initial default
- persistence backend
- retrieval backend
- public API shape
- multi-agent orchestration pattern
