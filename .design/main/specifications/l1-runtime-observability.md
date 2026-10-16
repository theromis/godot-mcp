# Runtime Observability

**Version:** 0.1.0
**Status:** Draft
**Layer:** concept

## Overview

Planned capability for agents to observe a running game, not only the editor: the game's log output, error stack traces, and an image of what the in-game camera renders. Collected from the README roadmap and the reserved console-wrapper setting; not implemented.

## Related Specifications

- [l1-tool-catalog.md](l1-tool-catalog.md) - New tools would join the catalog under its invariants.
- [l1-mcp-server.md](l1-mcp-server.md) - MCP-1: captured game output must never reach the protocol channel.

## 1. Motivation

After `godot_run_project` an agent can tell whether a scene plays, but not why it misbehaves: script errors, warnings and `print()` output of the game are invisible to it, and the editor viewport shows the scene as authored rather than as the camera sees it at run time. Agent guidance currently has to tell agents not to call the planned tool names, which is a recurring source of confusion.

## 2. Constraints & Assumptions

- Candidate tool names already announced publicly: `godot_get_debug_log`, `godot_get_stack_trace`, `godot_capture_camera`.
- The console wrapper path setting (`GODOT_CONSOLE_PATH`) is reserved for log capture and currently unused by the server.
- Game output captured by the server must go to a bounded buffer, never to the server's standard output.
- <!-- TBD: which process owns runtime access — this server through the editor's debugger session, or a separate runtime-side component inside the game. The choice decides whether these tools belong in this catalog at all. -->
- <!-- TBD: retention and size limits of the log buffer, and whether logs from a CLI-fallback game process are captured too. -->

## 3. Core Invariants

- **RO-1** Capturing runtime output never alters what the game prints or how it runs.
- **RO-2** Captured output is bounded in memory and readable on demand; the oldest data is dropped first.
- **RO-3** Runtime tools follow the catalog's live-first and error-hint rules; when no game is running they say so and how to start one.

## 7. Drawbacks & Alternatives

- Parsing the editor's Output panel: fragile and editor-version dependent.
- A runtime-side component injected into the game: richer access (frame stepping, input) but a second artifact to ship and keep out of exports.

## Document History

| Version | Date | Author | Description |
| --- | --- | --- | --- |
| 0.1.0 | 2026-10-16 | Agent (magic.analyze first-time) | Draft collected from the README roadmap, the agent guide's "not implemented" note and the reserved console-wrapper setting. Ownership left open. |
