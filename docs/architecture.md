# OpenClaw Architecture

## Goal

This repository is a Rig-oriented coding agent project built in phases.

Phase 0 establishes a compileable workspace and clear module boundaries without implementing the full agent loop yet.

## Workspace Layout

- `crates/core`: shared domain types, configuration, session model, and error handling.
- `crates/agent`: agent runtime abstractions and the initial stub runtime.
- `crates/tools`: tool trait definitions and registry.
- `crates/cli`: command-line entrypoint for local development.

## Phase Boundaries

### Phase 0

- Build the Rust workspace.
- Define stable core data structures.
- Add a compileable CLI scaffold.

### Phase 1

- Integrate Rig-backed model runtime.
- Add real tool implementations.
- Support end-to-end coding tasks from the CLI.

## Notes

- Plan Mode and Execute Mode are intentionally deferred.
- Persistence, RAG, and multi-agent orchestration are also deferred.
