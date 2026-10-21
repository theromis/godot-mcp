# Content Authoring

**Version:** 1.0.0
**Status:** Stable
**Layer:** concept

## Overview

Tools that create and change project content beyond node edits in an open scene: new scene files, attaching a script to a node, connecting and listing signals, resources, and project settings. Reading and rewriting the text of an existing script is not part of this set yet.

## Related Specifications

- [l1-tool-catalog.md](l1-tool-catalog.md) - TL-4 (confined, non-destructive writes), TL-5 (no injection) and TL-9 (editor sync) apply to every tool here.
- [l1-editor-bridge.md](l1-editor-bridge.md) - BR-5: scene changes made through the editor must stay undoable.
- [l2-tool-handlers.md](l2-tool-handlers.md) - Where the host side of these tools is realized.

## 1. Motivation

Without these tools an agent can add nodes to an open scene and write a script from a fixed template, but cannot create a scene, attach the script, wire a signal, create a resource or change a setting. Agents work around the gap by writing files directly, which bypasses the editor (no undo, no UID tracking, conflicts with resources the editor holds in memory) and, for `project.godot`, can leave a file that no longer loads.

## 2. Constraints & Assumptions

- The set is `godot_create_scene`, `godot_attach_script`, `godot_connect_signal`, `godot_list_signals`, `godot_create_resource`, `godot_get_project_settings` and `godot_set_project_setting`.
- With the editor connected, every tool goes through it: changes to a scene that is open are undoable editor actions, new files are registered with the editor file system so they get a UID at once.
- Without an editor, creating a scene, creating a resource, and reading or setting a project setting fall back to the file itself. Attaching a script, connecting a signal and listing signals need the editor, because they act on an open scene.
- A project setting value is a string, a number or a boolean. Structured values (vectors, arrays, resources) are changed in the editor's Project Settings.
- <!-- TBD: reading and updating an existing script: whether updates accept full file content, line-range patches, or both, and how a conflict with unsaved editor changes is reported. Not implemented; `godot_create_script` remains the only script writer. -->

## 3. Core Invariants

- **CA-1** No authoring tool replaces an existing file. Creating a scene or a resource that already exists is refused, also when the file appears between the check and the write.
- **CA-2** Attaching or detaching a script and connecting a signal in an open scene are each a single undoable editor action.
- **CA-3** Caller text is only ever data. It is not executed and not interpolated into generated code; a project setting value is written as one escaped value on one line, and a setting key is a plain path of letters, digits and `_ / . -` that starts with a section, so no call can add an entry or a section to `project.godot`.
- **CA-4** A project setting that exists keeps its type: a value of another type is refused, in the editor and in the file fallback.
- **CA-5** A link is checked before it is made: the script exists in the project, the signal exists on the node, the target has the method, and the connection is not already there. Each failure names the fix.
- **CA-6** Paths stay inside the project: scenes end in `.tscn`, resources in `.tres` (or `.res` through the editor), and `res://` paths cannot leave the project.

## 7. Drawbacks & Alternatives

- **Free-form script editing** is deliberately left out until its conflict rules (see the open question above) are decided.
- **A structured-value setting editor** would need a value syntax a caller can get wrong in ways that break project loading; scalars cover the common cases safely.

## Canonical References

| Alias | Path | Purpose |
| --- | --- | --- |
| `[HOST]` | `src/tools/authoring.rs` | Catalog entries and the file fallbacks |
| `[EDITOR]` | `addons/godot_mcp/bridge_server.gd` | Editor-side handlers |
| `[E2E]` | `tests/editor_e2e_test.rs` | `authoring_tools_create_what_they_should_and_refuse_the_rest` |
| `[TOOLS]` | `tests/tools_test.rs` | Fallback tests: no overwrite, escaping, key and type checks |

## Document History

| Version | Date | Author | Description |
| --- | --- | --- | --- |
| 0.1.0 | 2026-10-16 | Agent (magic.analyze first-time) | Draft collected from the README roadmap. |
| 1.0.0 | 2026-10-21 | Agent (magic.spec) | Seven tools implemented (pull request 7, reviewed and hardened). Constraints resolved: scene creation works with or without the editor. Invariants rewritten around what the tools guarantee; script reading and updating stays an open question. Draft → Stable. |
