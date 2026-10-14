# Agent Guidelines & Engineering Standards

This document establishes the architectural principles, development workflow, and operational rules for AI agents and human contributors developing or interacting with the Godot MCP Server.

## Mission & Scope

The primary objective of this project is to build an uncompromising, ultra-fast, cross-platform Model Context Protocol (MCP) server providing AI agents with full, reliable control over the Godot Game Engine (4.x+) across Windows, Linux, and macOS.

Agents must treat the Godot Editor as a live, stateful environment while ensuring complete safety, non-destructive mutations, and immediate visual/diagnostic feedback.

## Architecture Guidelines

### 1. Dual-Mode Operation

- **Live Mode (Primary)**: Connect to the Godot Editor bridge via WebSocket on `GODOT_DEBUG_PORT` (default `9333`). All scene, node, and property mutations must execute in real time.
- **Headless Mode (Fallback)**: When the editor GUI is not running, fall back to headless CLI invocation (`godot --headless --script ...`) for batch operations, scene scaffolding, and automated testing. Never fail silently if the editor socket is temporarily unreachable; provide clear diagnostic context and suggest launching the editor or falling back to CLI.

### 2. Safety & Undo/Redo Integration

- Every mutation executed within the Godot Editor (adding nodes, altering properties, removing branches) MUST be wrapped in `EditorUndoRedoManager`.
- This ensures human developers retain full visibility and the ability to undo any action performed by an AI agent using standard shortcuts (`Ctrl+Z`).
- Never overwrite scene files (`.tscn`) or resource files (`.tres`) directly on disk when the editor is actively holding those resources in memory without notifying the editor or triggering a resource re-import.

### 3. Cross-Platform Path Handling & Security

- **Path Traversal**: Strictly validate all file paths against directory traversal (`../`). Paths must resolve within the target project's `res://` directory or designated export folders.
- **Multi-OS Normalization**: Always handle differences in path separators across Windows (`\`), Linux (`/`), and macOS (`/`). Normalize paths before passing them to the engine or returning them via the protocol.
- **Identifier Sanitization**: Validate node names and class identifiers using strict alphanumeric patterns (`^[A-Za-z_][A-Za-z0-9_]*$`).
- **Resource Integrity**: Maintain Godot UID tracking (`.uid` files and `uid://` references) whenever modifying scenes or assets to prevent broken dependencies in Godot 4.x.

## Implementation Standards

### Core Server Standards (Rust)

- **Zero-Overhead Async I/O**: Use Tokio event loops for stdio JSON-RPC transport and WebSocket bridge communication.
- **Error Handling**: Never use unhandled panics (`unwrap()` / `expect()`) in production code paths. Use explicit `Result` propagation with informative error types.
- **Clean Stdio Separation**: All internal logging and diagnostics MUST be emitted to `stderr` using `tracing`. The `stdout` stream is strictly reserved for MCP JSON-RPC frames.
- **Standardized MCP Responses**: All errors must return an informative message and at least one actionable recovery solution.

### GDScript Standards (Editor Plugin)

- Target Godot 4.3+ modern GDScript features with strict typing (`var count: int = 0`, `func get_node_by_id(id: String) -> Node:`).
- Utilize standard signals for asynchronous event handling.
- Keep the plugin lightweight with zero external third-party dependencies outside the standard Godot API.

## Quality Assurance & Verification Protocols

Before submitting or committing changes, agents must execute and pass the complete verification pipeline:

1. **Static Analysis & Linting**:

   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

2. **Code Formatting**:

   ```bash
   cargo fmt --all -- --check
   ```

3. **Automated Testing**:

   ```bash
   cargo test --all-targets
   ```

4. **GDScript Syntax Check**:

   The repo root has its own minimal `project.godot` so the addon's `class_name` declarations resolve. `--path .` is required — without it the check fails with `Could not find type "GodotMcpBridgeServer"`.

   ```bash
   godot --headless --path . --import   # first run only, or after script changes
   godot --headless --check-only --path . --script addons/godot_mcp/plugin.gd
   ```

## Tool Invocation Protocols for AI Agents

When interacting with a Godot project, agents must adhere to the following workflow:

1. **Inspection Before Mutation**:
   - Always query `godot_get_scene_tree` or `godot_get_node_properties` before attempting to modify, add, or delete nodes.
   - Confirm parent node paths exist before instantiating child nodes.

2. **Validation After Modification**:
   - Run `godot_validate_script` immediately after creating or altering any `.gd` file.
   - Catch and resolve syntax errors before attempting to run scenes.

3. **Visual Verification**:
   - Use `godot_capture_viewport` or `godot_capture_camera` to visually inspect scene changes, layouts, and rendering quality when performing level design or UI assembly.

4. **Runtime Monitoring**:
   - Always monitor `godot_get_debug_log` during and after invoking `godot_run_project` to ensure no runtime errors or shader warnings occur.

## Completion Protocol (Mandatory Checklist)

Before concluding any development task, every agent MUST verify the following checklist:

- [ ] **Technical Language**: All code, identifiers, comments, and technical docs are in English.
- [ ] **Communication Language**: All conversational responses and planning are in Russian.
