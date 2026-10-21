# Runtime Observability

**Version:** 1.0.1
**Status:** Stable
**Layer:** concept

## Overview

How an agent observes and drives a running game, not only the editor: the game's log output and error stack traces, its live node tree and properties, input injection, screenshots, and, for a game the server started itself, breakpoints and stepping. The tools reach the game through Godot's own remote debugger and a small runtime component the game loads only while a debugger is attached.

## Related Specifications

- [l1-tool-catalog.md](l1-tool-catalog.md) - The runtime tools join the catalog under its invariants.
- [l1-mcp-server.md](l1-mcp-server.md) - MCP-1: captured game output must never reach the protocol channel.
- [l1-editor-bridge.md](l1-editor-bridge.md) - Loopback-only access applies to the game channel too.
- [l2-game-runtime.md](l2-game-runtime.md) - Where this concept is realized.
- [l1-controlled-time.md](l1-controlled-time.md) - Holding a running game still and stepping it by exact ticks; extends this concept.

## 1. Motivation

After `godot_run_project` an agent could tell whether a scene plays, but not why it misbehaves: script errors, warnings and `print()` output were invisible, the editor viewport shows the scene as authored rather than as it runs, and nothing could press a key or click. Observation and control of the running game close that loop: run, look, act, look again.

## 2. Constraints & Assumptions

- Two kinds of run exist. A **managed process run** is a game this server starts with the remote debugger pointed at itself; it can be observed, driven and debugged. **Editor Play** is started by the editor; it can be observed and driven through a relay in the plugin, but breakpoints and stepping stay with the editor's own debugger.
- The runtime component is a script loaded as an autoload through a temporary `override.cfg`, never through `project.godot`. It does nothing unless a debugger is attached, so a stale override cannot affect a normal or exported run.
- Game output captured by the server goes to a bounded buffer, never to the server's standard output.
- A game without a window (headless) has nothing to capture as an image.
- The console wrapper setting (`GODOT_CONSOLE_PATH`) is not used by the server; log capture comes from the debugger channel.

## 3. Core Invariants

- **RO-1** Observation does not change the game. Reading the tree, properties, logs and pictures has no side effect; everything that does change a running game (setting a property, calling a method, injecting input, setting the time scale) is an explicit tool call, and nothing it changes is saved to disk.
- **RO-2** Captured output is bounded in memory and readable on demand; the oldest entries are dropped first.
- **RO-3** Runtime tools follow the catalog's live-first and error-hint rules: with no game running they say so and how to start one, and a game that disconnected or exited fails the next call with that reason.
- **RO-4** The runtime component never ships. It is loaded only while a debugger is attached, through a temporary override that is removed when the run ends (also after a crash, on the next run), and the addon is excluded from exports.
- **RO-5** Access to a game is local. The debugger listener binds to loopback, accepts one connection and then stops listening.
- **RO-6** No code the caller wrote runs in the game. A method call needs the method named in an explicit permit list and is refused for any method that can quit, evaluate code, or call another method; property writes refuse `script` and private names; there is no expression evaluator.
- **RO-7** Input the server holds is released. A failed step, an explicit release, and the end of the game each clear every key, button, axis and touch that was pressed.
- **RO-8** A picture is a real picture: a capture returns a validated PNG or an error, never a placeholder or an empty image. A comparison between captures of different sizes says it was not made.
- **RO-9** Waits are bounded. Every wait for frames, a node, a signal or a stop has a limit and reports when it ran out.
- **RO-10** Breakpoints and stepping work only on a game the server started, where the server is the debugger; the plugin relay carries only the server's own messages and leaves every other debugger message to the editor.

## 7. Drawbacks & Alternatives

- Parsing the editor's Output panel: fragile and editor-version dependent.
- Implementing a debugger of our own: the server is a client of Godot's protocol instead, so a new engine version changes little.
- A runtime component that stays in the project: richer access at any time, but a second artifact to ship and keep out of exports; the temporary autoload avoids that.

## Canonical References

| Alias | Path | Purpose |
| --- | --- | --- |
| `[GAME]` | `src/tools/game.rs` | Tool catalog and handlers |
| `[RUNS]` | `src/godot/runs.rs` | Managed process runs |
| `[RUNTIME]` | `addons/godot_mcp/runtime/mcp_runtime.gd` | In-game component |
| `[E2E]` | `tests/runtime_e2e_test.rs` | Real-game tests |

## Document History

| Version | Date | Author | Description |
| --- | --- | --- | --- |
| 0.1.0 | 2026-10-16 | Agent (magic.analyze first-time) | Draft collected from the README roadmap, the agent guide's "not implemented" note and the reserved console-wrapper setting. Ownership left open. |
| 1.0.0 | 2026-10-21 | Agent (magic.spec) | Implemented by pull requests 3 to 5, 9 and 13: ownership resolved (the server as debugger client plus a temporary in-game component, a plugin relay for editor Play). Scope widened from logs and a camera image to observation and control. Invariants rewritten. Draft → Stable. |
| 1.0.1 | 2026-10-21 | Agent (magic.spec) | Linked the controlled-time concept that extends this one. No requirement changed. |
