# Jellyfish

Jellyfish is a Rig-based AI personal assistant project built in phases.

The current implementation focus is:

- CLI-first
- single-agent first
- core assistant workflow before advanced platform features

## Current Status

Phase 0 and Phase 1 are complete. Phase 2 is now complete as the stable execution baseline for Jellyfish.

Implemented so far:

- Rust workspace scaffold
- `core`, `agent`, `tools`, and `cli` crate boundaries
- shared configuration, error, session, and event types
- persistent local session storage in `./.jellyfish/session.json`
- personal-assistant memory/profile model with relevance-based recall
- assistant-first `notes` and `todos` tools
- REPL mode for ongoing local conversations

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

## Next Step

The next implementation target is Phase 1:

- integrate Rig runtime support
- add initial assistant-facing tools
- connect the CLI to a real agent execution path
