# Tool Catalog

**Version:** 1.0.0
**Status:** Stable
**Layer:** concept

## Overview

Defines the set of MCP tools godot-mcp offers, how each one chooses between the live editor and the headless engine, how a project directory is found when the caller does not name one, and the safety rules every tool that touches files or launches the engine must follow.

## Related Specifications

- [l1-mcp-server.md](l1-mcp-server.md) - Session, error contract and cancellation the tools run under.
- [l1-editor-bridge.md](l1-editor-bridge.md) - Commands behind the editor-backed tools.
- [l2-tool-handlers.md](l2-tool-handlers.md) - Implementation of this concept.
- [l1-content-authoring.md](l1-content-authoring.md) - Planned tools for creating scenes and editing scripts (Draft).
- [l1-runtime-observability.md](l1-runtime-observability.md) - Planned tools for runtime logs and in-game capture (Draft).

## 1. Motivation

Agents call tools without seeing the implementation; a tool's contract has to be predictable from its schema and description alone. Tools that write files or start processes are where an agent can do real damage — overwrite a user's script, write outside the project, inject an engine option, or start a game that dies with the agent's terminal. This specification fixes those boundaries for every current and future tool.

## 2. Constraints & Assumptions

- A tool is "editor-backed" when it needs a running editor with the bridge enabled, "host-side" when it runs without one, and "live-first" when it prefers the editor and falls back to the engine binary.
- The headless engine is only invoked for: version lookup, script validation, editor launch, and running the project.
- Tool names are stable public API (`godot_*`); renaming one is a breaking change.

## 3. Core Invariants

- **TL-1 Schema-first validation.** Every tool publishes an input schema. Required parameters, primitive types, array item types and enumerations are checked before the editor or the engine is contacted; a violation names the parameter, the expected type and the received type.
- **TL-2 Live-first, fall back only on absence.** A live-first tool uses the engine binary only when the editor is unreachable. An editor that answered with an error is reported as such and never masked by a fallback.
- **TL-3 Project resolution order.** When a tool needs a project directory: an explicit `project_path` wins; otherwise the project open in the connected editor (for tools that may use the editor); otherwise the nearest directory at or above the server's working directory that contains `project.godot`; otherwise the single direct subdirectory that contains one. Several candidates are an error that lists them and asks for `project_path`. A given `project_path` must contain `project.godot`.
- **TL-4 Confined, non-destructive writes.** Host-side writes resolve inside the project only: parent-directory segments are rejected, absolute paths must land inside the project, and the existing part of the path may not leave the project through a link. An existing file is never replaced unless the caller explicitly asks for it. A write is flushed before success is reported.
- **TL-5 No injection.** No caller-supplied string reaches the engine command line where it could be read as an option, and none is interpolated into generated source code. Class and node identifiers must match `^[A-Za-z_][A-Za-z0-9_]*$`; scene arguments must be `res://` paths without parent segments.
- **TL-6 Detached launches.** Processes started for the caller (editor, game) are detached: they receive no handle to the server's standard streams, do not inherit the server's console or process group, and keep running when the server or the client terminal is interrupted.
- **TL-7 Hints by failure class.** Every tool error carries recovery steps matched to what failed: editor unreachable, connection broken or busy, editor-reported failure (unauthorized, no active scene, not found), invalid arguments, engine binary failure.
- **TL-8 Bounded engine calls.** Engine invocations expected to finish (version lookup, script validation) have a time limit and report it when exceeded; their output is stripped of the engine banner.
- **TL-9 Editor stays in sync.** After a host-side write into the project the connected editor has open, the editor is asked to refresh that file so it is picked up (and assigned a UID) without a manual rescan.

## 5. Detailed Design

### 5.1 Catalog

| Tool | Mode | Mutates | Summary |
| --- | --- | --- | --- |
| `godot_get_version` | live-first | no | Engine version from the editor, else the binary |
| `godot_get_project_info` | editor | no | Name, main scene, features, directory |
| `godot_launch_editor` | host | process | Starts the editor for the resolved project |
| `godot_run_project` | live-first | process / play state | Editor Play mode; detached game process only without an editor (not stoppable via MCP) |
| `godot_stop_project` | editor | play state | Stops editor Play mode |
| `godot_get_run_status` | editor | no | Whether a scene plays, and which |
| `godot_get_open_scenes` | editor | no | Open tabs, active tab |
| `godot_open_scene` | editor | editor state | Opens a scene tab |
| `godot_save_scene` | editor | disk | Saves the active scene |
| `godot_get_scene_tree` | editor | no | Node hierarchy, scene-relative paths |
| `godot_add_node` | editor | scene (undoable) | Any instantiable Node class or global script class |
| `godot_remove_node` | editor | scene (undoable) | Root is protected |
| `godot_get_node_properties` | editor | no | `{type, value}` per editor-visible property |
| `godot_set_node_properties` | editor | scene (undoable) | All-or-nothing, read-back |
| `godot_capture_viewport` | editor (windowed) | editor state | PNG of the 2D/3D viewport with dimensions |
| `godot_create_script` | host | disk | `.gd` from a fixed template; no overwrite by default |
| `godot_validate_script` | host | no | Headless syntax check |
| `godot_resave_resources` | editor | editor file system | Re-import / refresh after external edits |

### 5.2 Live-First Decision

```mermaid
graph TD
    A[Tool call] --> B{Arguments valid?}
    B -->|no| E1[Error: parameter + expected type]
    B -->|yes| C[Send to editor]
    C -->|success| R1[Return editor result]
    C -->|editor reported error| E2[Return editor error + class hints]
    C -->|connection broken / timeout| E3[Return connection error + retry hints]
    C -->|unreachable| D{Tool has a headless path?}
    D -->|no| E4[Error: launch editor / enable plugin / check port]
    D -->|yes| F[Resolve project, run engine binary]
```

## 7. Drawbacks & Alternatives

- **Headless `--headless` for the fallback game run**: rejected — a visible game window is what a caller of "run project" expects.
- **Free-form script content in `godot_create_script`**: rejected for now; the fixed template keeps generated code free of caller text. Richer script editing is planned in [l1-content-authoring.md](l1-content-authoring.md).

## Canonical References

| Alias | Path | Purpose |
| --- | --- | --- |
| `[TOOLS]` | `src/tools.rs` | Catalog, dispatch, fallback and hint logic |
| `[SCHEMA]` | `src/validation.rs` | Argument checks against the input schema |
| `[PATHS]` | `src/paths.rs` | Project discovery, confinement, identifier pattern |
| `[ENGINE]` | `src/cli_runner.rs` | Engine invocations and detached launches |

## Document History

| Version | Date | Author | Description |
| --- | --- | --- | --- |
| 1.0.0 | 2026-10-16 | Agent (magic.analyze first-time) | Reverse-engineered from the code at 94e4540; rationale from the 2026-10-14 simulation QA report (script writes outside the project, engine option injection, fallback masking editor errors, Ctrl+C killing launched editors, inherited stdio handles on Windows). |
