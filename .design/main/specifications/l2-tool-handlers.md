# Tool Handlers (Rust)

**Version:** 1.0.0
**Status:** Stable
**Layer:** implementation
**Implements:** l1-tool-catalog.md

## Overview

The Rust realization of the tool catalog: the static catalog with input schemas, argument validation, dispatch to the bridge or to host-side handlers, project discovery and path confinement, and the engine runner that validates scripts and launches detached editor and game processes.

## Related Specifications

- [l1-tool-catalog.md](l1-tool-catalog.md) - Parent concept.
- [l2-bridge-client-rust.md](l2-bridge-client-rust.md) - Transport for editor-backed tools.
- [l2-rust-stdio-server.md](l2-rust-stdio-server.md) - Session that calls `ToolManager::call_tool`.

## 1. Motivation

Maps each safety rule of the catalog to the function that enforces it, so a new tool can reuse the same helpers instead of re-implementing path or identifier checks.

## 2. Constraints & Assumptions

- The catalog is built once (`OnceLock`) and shared by `tools/list` and dispatch.
- Engine time limits: `--version` 10 s, `--check-only` 30 s; both use `kill_on_drop`.
- Windows-only code uses `windows-sys` (`Win32_Foundation`, `Win32_System_Console`).
- Observed convention: every `unsafe` block carries a `SAFETY:` comment explaining why the call is sound.

## 4. Invariant Compliance

| L1 Invariant | Implementation |
| --- | --- |
| TL-1 Schema-first validation | `call_tool` runs `validate_arguments(tool.input_schema, args)` before dispatch: object-or-null arguments, required keys present and non-null, `string`/`boolean`/`object`/`array`/`number`/`integer` (whole floats accepted), array item types, enums; unknown keys are ignored. |
| TL-2 Live-first fallback | `handle_get_version` falls back unless the editor returned `ToolExecution`; `handle_run_project` and `resolve_project` fall back only on `BridgeUnavailable`. |
| TL-3 Project resolution | `resolve_project`: `project_path` → editor `get_project_info.project_dir` → `find_project(cwd)`; `host_project` (launch, CLI run) skips the editor step. `find_project` walks ancestors, then accepts exactly one child with `project.godot`, else lists candidates. `ensure_project_dir` requires `project.godot`. The working directory is captured at start-up and overridable for tests. |
| TL-4 Confined, non-destructive writes | `resolve_in_project` strips `res://`, normalizes `\` to `/`, rejects `..` components (names containing `..` are fine), requires the nearest existing ancestor to canonicalize inside the project (blocks symlink escapes). `handle_create_script` opens with `create_new` unless `overwrite: true`, and flushes the file before reporting success. |
| TL-5 No injection | `is_valid_identifier` gates `extends_class` and `class_name`; script content comes from a fixed template; `godot_run_project` requires `scene` to start with `res://` and contain no `..`; the bridge receives values only as JSON parameters. |
| TL-6 Detached launches | `spawn_detached`: null stdin/stdout/stderr; Windows `DETACHED_PROCESS \| CREATE_NEW_PROCESS_GROUP` and the server's own std handles marked non-inheritable before spawning; Unix `process_group(0)`. The child is not awaited. |
| TL-7 Hints by failure class | `bridge_error_result` maps `BridgeUnavailable`, `Bridge` and `ToolExecution`; `editor_error_hints` specializes on "unauthorized", "no active scene", and not-found messages; host-side errors carry their own hints (`PROJECT_PATH_HINTS`, overwrite, writable directory, `GODOT_PATH`). |
| TL-8 Bounded engine calls | `get_version` and `validate_script` wrap the process in `timeout`; `strip_banner` drops `Godot Engine v…` lines; validation fails if the exit code is non-zero or stderr is non-empty. |
| TL-9 Editor stays in sync | After `godot_create_script` writes into the editor's own project, it sends `resave_resources` for the new `res://` path (best effort). |

## 5. Detailed Design

### 5.1 Files

```plaintext
src/
├── tools.rs        ToolManager, catalog, dispatch, fallback, hints
├── validation.rs   JSON-schema subset checks
├── paths.rs        is_valid_identifier, ensure_project_dir, find_project, resolve_in_project
└── cli_runner.rs   CliRunner: get_version, validate_script, launch_editor, run_project, spawn_detached
```

### 5.2 Adding a Tool

1. Add the catalog entry with a complete schema (required keys, types, enums) and a description that states whether it needs the editor.
2. Write the failing test first in `tests/tools_test.rs` (host-side) or `tests/stdio_protocol_test.rs` / `tests/editor_e2e_test.rs` (bridge-backed).
3. Route the name in `call_tool`; for an editor command, add the handler and its `PARAM_TYPES` entry in the addon.
4. Reuse `resolve_in_project`, `is_valid_identifier` and `bridge_error_result`; never build engine arguments or script text from raw caller strings.
5. Update the README tool table.

## 7. Drawbacks & Alternatives

- **A full JSON-Schema validator crate**: more complete, but the catalog uses a small subset; the hand-written checker gives clearer messages and no extra dependency.

## Canonical References

| Alias | Path | Purpose |
| --- | --- | --- |
| `[TOOLS]` | `src/tools.rs` | Catalog and dispatch |
| `[SCHEMA]` | `src/validation.rs` | Argument validation |
| `[PATHS]` | `src/paths.rs` | Project discovery and confinement |
| `[ENGINE]` | `src/cli_runner.rs` | Engine runner and detached spawning |
| `[DOCS]` | `README.md` | User-facing tool table |

## Document History

| Version | Date | Author | Description |
| --- | --- | --- | --- |
| 1.0.0 | 2026-10-16 | Agent (magic.analyze first-time) | Reverse-engineered from the code at 94e4540. |
