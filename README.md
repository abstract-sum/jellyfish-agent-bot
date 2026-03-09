# OpenClaw

OpenClaw is a Rig-based AI coding agent project built in phases.

The current implementation focus is:

- CLI-first
- single-agent first
- core coding workflow before advanced platform features

## Current Status

Phase 0 is complete.

Implemented so far:

- Rust workspace scaffold
- `core`, `agent`, `tools`, and `cli` crate boundaries
- compileable CLI skeleton
- shared configuration, error, session, and event types

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

## Next Step

The next implementation target is Phase 1:

- integrate Rig runtime support
- add initial repository tools
- connect the CLI to a real agent execution path
