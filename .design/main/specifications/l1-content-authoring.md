# Content Authoring

**Version:** 0.1.0
**Status:** Draft
**Layer:** concept

## Overview

Planned tools for creating and changing project content beyond node edits in an open scene: creating new scene files, and reading, updating and attaching GDScript files. Collected from the README roadmap; not implemented.

## Related Specifications

- [l1-tool-catalog.md](l1-tool-catalog.md) - TL-4 (confined, non-destructive writes), TL-5 (no injection) and TL-9 (editor sync) apply to every tool here.
- [l1-editor-bridge.md](l1-editor-bridge.md) - BR-5: scene changes made through the editor must stay undoable.

## 1. Motivation

Today an agent can add nodes to an already open scene and create a script from a fixed template, but cannot create a scene, read an existing script, change it, or attach it to a node. Agents work around this by writing files directly, which bypasses the editor (no undo, no UID tracking, possible conflicts with resources the editor holds in memory).

## 2. Constraints & Assumptions

- Candidate tool names already announced publicly: `godot_create_scene`, plus script read/update/attach tools (names not fixed).
- Files the editor holds open must be changed through the editor, not overwritten on disk behind its back.
- New files must receive Godot UIDs so references to them stay stable.
- <!-- TBD: whether script updates accept full file content, line-range patches, or both; and how a conflict with unsaved editor changes is reported. -->
- <!-- TBD: whether scene creation needs the editor (undoable, UID assigned immediately) or also works headless. -->

## 3. Core Invariants

- **CA-1** No authoring tool overwrites an existing file unless the caller explicitly asks for it.
- **CA-2** Attaching or changing a script on a node is a single undoable editor action.
- **CA-3** Script text supplied by the caller is written verbatim as file content and is never executed or interpolated into other generated code.
- **CA-4** After a write, the tool reports how to validate the result (script check) and the editor is synchronized.

## Document History

| Version | Date | Author | Description |
| --- | --- | --- | --- |
| 0.1.0 | 2026-10-16 | Agent (magic.analyze first-time) | Draft collected from the README roadmap. |
