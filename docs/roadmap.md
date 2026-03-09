# Jellyfish Roadmap

## Delivery Strategy

Jellyfish is delivered in phases.

The guiding strategy is:

- build the smallest useful capability first
- keep each phase independently valuable
- avoid adding advanced features before the core loop is reliable

## Phase 0 - Workspace Scaffold

### Goal

Create a compileable and extensible repository structure that supports future phases without major reshaping.

### Included

- Rust workspace
- crate boundaries
- shared domain model
- stub agent runtime
- tool registry abstraction
- CLI scaffold
- basic documentation

### Excluded

- real Rig integration
- real tool execution
- persistence
- retrieval
- multi-agent workflows
- web/API layer

### Acceptance Criteria

- `cargo check` passes
- CLI binary runs
- core abstractions are stable enough for Phase 1
- future runtime and tool logic can be added without moving crate boundaries

### Status

Completed.

## Phase 1 - Core MVP

### Goal

Implement the smallest useful Jellyfish workflow: a real Rig-backed assistant that can hold a conversation, answer questions, and use a minimal toolset when necessary.

### Scope

- integrate `rig-core`
- configure one provider first
- create a real runtime in `crates/agent`
- implement the first utility tools
- wire `chat` command to real model execution

### Initial Tool Set

Recommended order:

1. conversation-only runtime
2. lightweight local knowledge tools
3. memory/session helpers
4. optional file tools
5. optional command tools

### Key Work Items

#### Agent runtime

- add Rig dependency
- create provider client bootstrap
- define a system prompt for personal assistant behavior
- transform session messages into Rig-compatible requests
- return a normalized `AgentResponse`

#### Tool system

- keep the existing `Tool` abstraction
- align tool definitions with model-callable schema
- add registry lookup and invocation plumbing
- surface tool events back into the session/event model
- keep code-oriented tools optional rather than foundational

#### CLI integration

- replace the stub runtime path with a real runtime
- support loading config from environment
- print model responses and tool events clearly

### Acceptance Criteria

- user can ask a general question in the CLI
- runtime reaches a real model provider
- assistant can use at least one basic tool when needed
- responses are visible in the terminal and mapped into event records

## Phase 2 - Stable Execution Layer

### Goal

Make the single-agent workflow robust enough for recurring personal assistant tasks.

### Scope

- session persistence
- better runtime error handling
- tool timeout and output truncation
- tool execution event stream
- structured tracing and observability

### Key Work Items

- store session state in memory first, then prepare for SQLite
- normalize tool call lifecycle:
  - requested
  - started
  - completed
  - failed
- define command execution constraints
- integrate `tracing`
- prepare Langfuse/OpenTelemetry compatibility later

### Acceptance Criteria

- repeated CLI interactions can preserve state
- tool failures are recoverable and visible
- long-running or noisy tool outputs are controlled
- runtime behavior is observable through logs and events

## Phase 3 - Interaction Experience Enhancements

### Goal

Improve usability without introducing Plan Mode / Execute Mode.

### Scope

- better progress feedback
- streaming output
- clearer summaries
- confirmations for dangerous operations

### Included Examples

- show "analyzing repository"
- show "editing file"
- show "running validation command"
- summarize changed files and validation results at the end

### Explicit Non-Goal

Do not split the product into separate planning and execution modes in this phase.

## Phase 4 - Retrieval And Memory

### Goal

Enable user-aware and history-aware reasoning.

### Scope

- user profile and preference storage
- embeddings
- vector search
- reusable memory for prior tasks and prior conversations

### Notes

Suggested progression:

- start with simple local memory
- evaluate `rig-sqlite` or a lightweight vector backend
- keep retrieval optional at runtime

## Phase 5 - Multi-Agent Orchestration

### Goal

Expand beyond one runtime loop only after the single-assistant path is stable.

### Scope

Potential roles:

- planner
- organizer
- researcher
- action executor

### Constraint

Start with simple orchestration, not a complex autonomous swarm.

## Phase 6 - API And Web Layer

### Goal

Expose the runtime beyond CLI consumers.

### Scope

- `axum` API
- streaming transport
- session endpoints
- frontend integration later

## Milestones

### M1

Real model call from CLI.

### M2

Basic assistant tool usage available.

### M3

Persistent session and memory workflow available.

### M4

User-oriented utility actions available.

### M5

Single-assistant task loop is stable.

## Phase 1 Recommended Implementation Order

1. add `rig-core` to `crates/agent`
2. create provider bootstrap from environment config
3. replace the stub runtime with a real runtime adapter
4. simplify the prompt for assistant-first behavior
5. wire tool registration into the runtime
6. add the first non-code-centric tool
7. update CLI `chat` to use the real runtime
8. add basic runtime tests
9. polish logging and event output
10. begin memory/session persistence planning
