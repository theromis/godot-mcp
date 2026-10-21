# Tool Handlers (Rust)

**Version:** 1.3.2
**Status:** Stable
**Layer:** implementation
**Implements:** l1-tool-catalog.md

## Overview

The Rust realization of the tool catalog: the static catalog with input schemas, argument validation, dispatch to the bridge or to host-side handlers, project discovery and path confinement, and the engine runner that validates scripts and launches detached editor and game processes.

## Related Specifications

- [l1-tool-catalog.md](l1-tool-catalog.md) - Parent concept.
- [l2-bridge-client-rust.md](l2-bridge-client-rust.md) - Transport for editor-backed tools.
- [l2-rust-stdio-server.md](l2-rust-stdio-server.md) - Session that calls `ToolManager::call_tool`.
- [l2-agent-feedback-rust.md](l2-agent-feedback-rust.md) - The feedback tools routed here and the outcome recording done after every call. [ADDED]
- [l2-game-runtime.md](l2-game-runtime.md) - The running-game and debugger tools routed to `tools/game.rs`. [ADDED]
- [l2-lsp-client.md](l2-lsp-client.md) - The language-server tools routed to `tools/code.rs`. [ADDED]
- [l1-content-authoring.md](l1-content-authoring.md) - CA-7 to CA-10: the script read and edit tools (§5.4).

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
| TL-4 Confined, non-destructive writes | `resolve_in_project` strips `res://`, normalizes `\` to `/`, rejects `..` components (names containing `..` are fine), requires the nearest existing ancestor to canonicalize inside the project (blocks symlink escapes). `handle_create_script` opens with `create_new` unless `overwrite: true`, and flushes the file before reporting success. `godot_read_script` and `godot_edit_script` use the same path check and write only through a temporary file and a rename (§5.4). |
| TL-5 No injection | `is_valid_identifier` gates `extends_class` and `class_name`; script content comes from a fixed template; `godot_run_project` requires `scene` to start with `res://` and contain no `..`; the bridge receives values only as JSON parameters. |
| TL-6 Detached launches | `spawn_detached`: null stdin/stdout/stderr; Windows `DETACHED_PROCESS \| CREATE_NEW_PROCESS_GROUP` and the server's own std handles marked non-inheritable before spawning; Unix `process_group(0)`. The child is not awaited. A managed process run (`GameHub::start`) takes the same process-group flags and a closed stdin, pipes its output into a bounded log instead of inheriting anything, and is killed when the hub is dropped. [MODIFIED] |
| TL-7 Hints by failure class | `bridge_error_result` maps `BridgeUnavailable`, `Bridge` and `ToolExecution`; `editor_error_hints` specializes on "unauthorized", "no active scene", and not-found messages; host-side errors carry their own hints (`PROJECT_PATH_HINTS`, overwrite, writable directory, `GODOT_PATH`). |
| TL-8 Bounded engine calls | `get_version` and `validate_script` wrap the process in `timeout`; `strip_banner` drops `Godot Engine v…` lines; validation fails if the exit code is non-zero or stderr is non-empty. |
| TL-9 Editor stays in sync | After `godot_create_script` writes into the editor's own project, it sends `resave_resources` for the new `res://` path (best effort). |

## 5. Detailed Design

### 5.1 Files

```plaintext
src/
├── tools.rs        ToolManager, catalog, dispatch, fallback, hints
├── tools/          authoring.rs (scene, script, signal, resource and setting tools and their file fallbacks), scripts.rs (reading and editing existing scripts), game.rs (running game and debugger tools), code.rs (language-server tools), feedback.rs
├── godot/          managed runs, debugger session, breakpoint control, Variant codec
├── lsp/            language-server client and framing
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

### 5.3 Feedback Hooks [ADDED]

- `call_tool` hands every outcome to the feedback journal after the handler has returned: a failed argument check as `invalid_arguments`, an error result as `error`, and a per-tool call and failure count. Calls to the three feedback tools are not recorded. Recording never alters the result.
- The three feedback tools are routed like host tools and appear in the catalog according to the configured feedback mode.
- Details, limits and the order of work are in [l2-agent-feedback-rust.md](l2-agent-feedback-rust.md) (5.2).

### 5.4 Script Tools

`godot_read_script` and `godot_edit_script` realize CA-7 to CA-10 of [l1-content-authoring.md](l1-content-authoring.md). Both are live-first: the editor commands `read_script` and `edit_script` (described in [l2-godot-editor-addon.md](l2-godot-editor-addon.md) §5.3) are tried first, and only `BridgeUnavailable` falls back to the file in the resolved project (`host_project`), as `create_scene` does; any other bridge error is returned as it is. They live in `tools/scripts.rs`.

| Tool | Parameters | Answer |
| --- | --- | --- |
| `godot_read_script` | `script_path` (required), `project_path` | `{path, text, source, dirty, version}`; in the file fallback `source` is `file`, `dirty` is false, and `editor_consulted` is false |
| `godot_edit_script` | `script_path`, `old_text`, `new_text` (required), `version`, `project_path` | `{path, replaced: 1, version, source}`; in the file fallback `source` is `file` and `editor_consulted` is false |

Shared rules, in the editor and in the file fallback:

- The path goes through `resolve_in_project` (TL-4), ends in `.gd`, names an existing file and the file is at most 1 MiB; a file that is not valid UTF-8 is refused, never converted. The edit tool never creates a file: a missing file is an error that points to `godot_create_script`.
- Version label: the 64-bit FNV-1a hash of the UTF-8 bytes of the text, with `\r\n` read as `\n`, written as 16 lowercase hex digits. No dependency is added on either side. Test vectors, one per side: the empty text gives `cbf29ce484222325`; `extends Node` followed by a newline gives `688cd8553db5c904`; `a` gives `af63dc4c8601ec8c`.
- Matching: `old_text` and the file text are compared with `\r\n` read as `\n`; `old_text` is not empty and differs from `new_text`; it must occur exactly once, otherwise the edit is refused with the count. A supplied `version` must equal the label of the current text, otherwise the edit is refused and the caller reads again. A refused edit leaves the file and the editor untouched.
- The caller's strings are data: they are matched and inserted as text, never executed, never interpolated into a command or into generated script text (TL-5).

File fallback only:

- The host cannot know that an editor holds unsaved typing when no bridge is connected, so CA-9 is about the editor only; the answer says `editor_consulted: false`.
- A file that uses `\r\n` keeps it: the result is written with `\r\n` for every line, and a leading UTF-8 byte order mark is kept.
- The new text is written to a temporary file in the same directory and renamed over the script, so no half-written script is ever seen (CA-10). The temporary file has an unpredictable name and is created exclusively (`create_new`), so a file or link planted at that name is never followed and the write cannot leave the project (TL-4). A script that is itself a symbolic link is not replaced: the edit is refused with the fix. When the write or the rename fails, the temporary file is removed and the script is unchanged.
- No lock is held across the bridge round-trip (TL-2); the fallback reads and writes through `tokio::fs`.

Refusals are `ToolResult` errors with at least one fix (the TL-7 rule): the count of matches, the stale label, the unsaved changes, the size limit, the extension.

## 7. Drawbacks & Alternatives

- **A full JSON-Schema validator crate**: more complete, but the catalog uses a small subset; the hand-written checker gives clearer messages and no extra dependency.

## Canonical References

| Alias | Path | Purpose |
| --- | --- | --- |
| `[TOOLS]` | `src/tools.rs` | Catalog and dispatch |
| `[SCHEMA]` | `src/validation.rs` | Argument validation |
| `[PATHS]` | `src/paths.rs` | Project discovery and confinement |
| `[ENGINE]` | `src/cli_runner.rs` | Engine runner and detached spawning |
| `[SCRIPTS]` | `src/tools/scripts.rs` | Script read and edit tools and their file fallback |
| `[DOCS]` | `README.md` | User-facing tool table |
| `[PARITY]` | `docs/feature-parity.md` | Comparison of the tool set with other Godot MCP servers |

## Document History

| Version | Date | Author | Description |
| --- | --- | --- | --- |
| 1.0.0 | 2026-10-16 | Agent (magic.analyze first-time) | Reverse-engineered from the code at 94e4540. |
| 1.1.0 | 2026-10-16 | Agent (magic.spec) | [ADDED] Section 5.3 on the feedback hooks in dispatch and the catalog; link to the feedback implementation. |
| 1.2.0 | 2026-10-21 | Agent (magic.spec) | [ADDED] The tool modules and the engine-protocol modules in the file map, links to their specifications; [MODIFIED] TL-6 for managed process runs. |
| 1.2.1 | 2026-10-21 | Agent (magic.spec) | Added the feature parity document to Canonical References. No requirement changed. |
| 1.3.0 | 2026-10-21 | Agent (magic.run) | [ADDED] §5.4: the `godot_read_script` and `godot_edit_script` tools, their parameters and answers, the version label and its test vectors, the matching and line-ending rules, the file fallback and its atomic write. Stable → RFC → Stable after Post-Update Review. |
| 1.3.1 | 2026-10-21 | Agent (magic.run) | The script tools live in `tools/scripts.rs`, not `authoring.rs`, which would have passed 700 lines; the file is a canonical reference. No requirement changed. |
| 1.3.2 | 2026-10-21 | Agent (magic.run) | After the automated security review: the temporary file of the edit fallback is created exclusively under an unpredictable name, and a script that is a symbolic link is refused. No other requirement changed. |
