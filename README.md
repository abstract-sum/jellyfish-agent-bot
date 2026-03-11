# Jellyfish

Jellyfish is a Rig-based AI personal assistant project built in phases.

The default local profile is tuned for Codex-compatible OpenAI models.

The current implementation focus is:

- CLI-first
- single-agent first
- core assistant workflow before advanced platform features

## Current Status

Phase 0 through Phase 4 are complete.

Implemented so far:

- Rust workspace scaffold
- `core`, `agent`, `tools`, and `cli` crate boundaries
- shared configuration, error, session, and event types
- persistent local session storage in `./.jellyfish/session.json`
- personal-assistant memory/profile model with relevance-based recall
- assistant-first `notes` and `todos` tools
- REPL mode for ongoing local conversations
- clearer progress and summary output for each assistant turn
- confirmation gates for dangerous file-edit tools
- retrieval across profile, memories, notes, todos, and conversation history
- `recall` command for inspecting retrieved context

## Repository Layout

```text
.
├── crates/
│   ├── core/
│   ├── agent/
│   ├── tools/
│   └── cli/
└── docs/
```

## Documentation

- `docs/vision.md`: product goals, principles, and deferred items
- `docs/architecture.md`: workspace structure and crate responsibilities
- `docs/roadmap.md`: phased implementation plan and milestones
- `docs/README.md`: documentation index

## Product Direction

Jellyfish is being positioned as a general personal assistant rather than a code agent.

That means the long-term priorities are:

- conversation and task assistance first
- memory and user context before code execution
- optional tools for utility tasks, not repo automation as the primary value

## Codex Compatibility

- default provider profile: `codex`
- default model: `gpt-5.4`
- Jellyfish natively calls `https://chatgpt.com/backend-api/codex/responses`
- Codex transport supports `auto`, `sse`, and `websocket` via `RIG_CODEX_TRANSPORT`
- you can switch back to `openai` or `mock` with `RIG_PROVIDER`
- Jellyfish reads OAuth credentials from `~/.codex/auth.json`
- `codex-cli` remains available as a fallback provider if you want to delegate requests to the local CLI

## Next Step

The next implementation target is Phase 1:

- integrate Rig runtime support
- add initial assistant-facing tools
- connect the CLI to a real agent execution path
