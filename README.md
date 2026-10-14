# Godot MCP Server

A high-performance, cross-platform Model Context Protocol (MCP) server written in Rust that provides bidirectional control over the Godot Engine (4.x+) for AI coding assistants.

Designed for fast execution, zero runtime dependencies, and seamless operation across **Windows**, **Linux**, and **macOS**.

## Key Features

- **Native Rust Core**: Built on Tokio async I/O with JSON-RPC handling over stdio. No Node.js or Python runtime required.
- **Cross-Platform Compatibility**: Prebuilt standalone binaries for Windows (`godot-mcp.exe`), Linux (`godot-mcp`), and macOS (`godot-mcp`, Intel and Apple Silicon).
- **Live Editor Bridge**: Connects to a running Godot Editor session over WebSocket to inspect and mutate the currently open scene in real time.
- **Headless CLI Fallback**: Falls back to invoking the Godot binary directly (`--headless`, `--check-only`, etc.) for engine version queries, script validation, and launching the editor/project when the live bridge is unavailable.
- **Scene & Node Control**: Query the scene tree, add/remove nodes, and inspect or modify exported node properties.
- **Viewport Capture**: Capture the active Godot editor viewport as a base64-encoded PNG for multimodal inspection.
- **Scripting Helpers**: Generate boilerplate GDScript files and run headless syntax validation via the Godot CLI checker.
- **Safe Editor Integration**: Node mutations (add/remove/set properties) route through `EditorUndoRedoManager`, so human developers keep full undo/redo control (`Ctrl+Z`) over AI-driven changes.

## Architecture

The system consists of two tightly coupled components:

1. **Godot Editor Bridge Plugin (`addons/godot_mcp/`)**:
   A lightweight, zero-dependency GDScript plugin running inside the Godot Editor. It opens a local WebSocket server (default port `9333`) providing scene tree inspection, node mutation, and viewport capture commands.

2. **Core MCP Server (Rust)**:
   A binary implementing the Model Context Protocol over stdio. It forwards tool calls to the Godot Editor bridge when connected, or invokes the Godot binary directly in headless mode for CLI-only operations (version lookup, script validation, launching the editor/project).

## Prerequisites

- Godot Engine 4.3+ (tested against 4.8 dev builds). Version 4.3+ is required because the bundled `.uid` resource identifiers depend on it.

No Rust toolchain or other build tooling is required — install from the prebuilt release archives below.

## Configuration

The server reads environment variables from a local `.env` file (in the server's working directory) or from the host system:

```env
# Path to the primary Godot executable
# Windows: "C:\path\to\Godot\Godot.exe"
# Linux:   "/path/to/godot"
# macOS:   "/path/to/Godot.app/Contents/MacOS/Godot"
GODOT_PATH="C:\path\to\Godot\Godot.exe"

# Communication port between the MCP server and the Godot Editor bridge
GODOT_DEBUG_PORT="9333"
```

If `GODOT_PATH` is unset or points to a missing file, the server searches a short list of common install locations for your platform, then falls back to `godot` on `PATH`.

> **Reserved, not yet active:** `GODOT_CONSOLE_PATH` and `GODOT_MCP_URL` are accepted as environment variables/CLI flags but are not consumed by any code path yet (no synchronous console log capture, no HTTP/SSE transport). They are placeholders for planned features — see [Roadmap](#roadmap).

## Installation

### 1. Download the MCP server binary

Grab the archive matching your OS/architecture from the [Releases page](https://github.com/zigenzoog/godot-mcp/releases/latest) and extract it anywhere on disk:

| Platform | Asset | Contents |
| --- | ---- | ---- |
| Windows x86_64 | `godot-mcp-x86_64-pc-windows-msvc.zip` | `godot-mcp.exe` |
| Linux x86_64 | `godot-mcp-x86_64-unknown-linux-gnu.tar.gz` | `godot-mcp` |
| macOS Apple Silicon | `godot-mcp-aarch64-apple-darwin.tar.gz` | `godot-mcp` |
| macOS Intel | `godot-mcp-x86_64-apple-darwin.tar.gz` | `godot-mcp` |
| Any platform | `godot-mcp-addon.zip` | `addons/godot_mcp/` (Godot Editor plugin — needed regardless of which binary above you use) |

### 2. Godot Editor plugin setup

1. Extract `addons/godot_mcp` from `godot-mcp-addon.zip` (downloaded in step 1) into your target Godot project's `addons/` folder:

   ```
   your_godot_project/
   └── addons/
       └── godot_mcp/
           ├── plugin.cfg
           ├── plugin.gd
           └── bridge_server.gd
   ```

2. Open your project in the Godot Editor.
3. Go to **Project → Project Settings → Plugins** and enable **Godot MCP Bridge**.
4. The plugin automatically begins listening on the port configured for your project (default: `9333`).

### 3. MCP client configuration

Point your MCP client at the binary from step 1, and set `GODOT_PATH` to your actual Godot executable. **Do not guess this path or assume a default install location** — official Windows builds ship as a version-named, portable executable (e.g. `Godot_v4.8.2-stable_win64.exe`) that the user extracts wherever they like, so the exact filename and folder vary per machine.

#### Claude Code (project-scoped `.mcp.json`)

Both the server binary's location and `GODOT_PATH` are machine-specific — two developers on the same repo will almost never have identical values. **Never commit a `.mcp.json` containing real absolute paths.** Instead:

1. Commit a `.mcp.json.example` with placeholder paths, and add this to the project's `.gitignore`:

   ```gitignore
   .mcp.json
   !.mcp.json.example
   ```

2. Each developer copies it locally and fills in their own paths:

   ```bash
   cp .mcp.json.example .mcp.json
   ```

`.mcp.json.example`:

```json
{
  "mcpServers": {
    "godot": {
      "command": "/path/to/godot-mcp/godot-mcp",
      "args": [],
      "env": {
        "GODOT_PATH": "/path/to/Godot/Godot_v4.x-stable_platform.exe",
        "GODOT_DEBUG_PORT": "9333"
      }
    }
  }
}
```

#### Claude Desktop

Add the server to your `claude_desktop_config.json`:

Windows:

```json
{
  "mcpServers": {
    "godot": {
      "command": "C:\\path\\to\\godot-mcp\\godot-mcp.exe",
      "args": [],
      "env": {
        "GODOT_PATH": "C:\\path\\to\\Godot\\Godot_v4.x-stable_win64.exe",
        "GODOT_DEBUG_PORT": "9333"
      }
    }
  }
}
```

Linux / macOS:

```json
{
  "mcpServers": {
    "godot": {
      "command": "/path/to/godot-mcp/godot-mcp",
      "args": [],
      "env": {
        "GODOT_PATH": "/path/to/godot",
        "GODOT_DEBUG_PORT": "9333"
      }
    }
  }
}
```

#### Cursor / Antigravity / VS Code

Configure the server in your MCP settings (`.cursor/mcp.json` or equivalent):

```json
{
  "mcpServers": {
    "godot": {
      "command": "/path/to/godot-mcp/godot-mcp",
      "args": []
    }
  }
}
```

## Autonomous AI Setup Prompt

Copy and paste this prompt to allow an AI assistant to detect your environment and install the Godot MCP server automatically:

```text
You are an expert systems engineer tasked with installing the Godot MCP server (https://github.com/zigenzoog/godot-mcp) for this user. You are working in an arbitrary Godot project directory, not a clone of that repository — do not assume any of its files are already present locally.

Follow this exact execution plan:
1. Environment Detection:
   - Identify the host operating system and architecture (Windows, Linux, or macOS; x86_64 or arm64).
   - **Ask the user for the exact path to their Godot executable — never guess or assume a default install location.** Official Windows builds are portable, version-named files (e.g. `Godot_v4.8.2-stable_win64.exe`) extracted wherever the user chose; there is no reliable standard path to infer this from.
   - Once given the path, run `<GODOT_PATH> --version` to confirm it is a valid, working Godot executable before proceeding.
2. Obtain the Server Binary and Addon from the remote repository (https://github.com/zigenzoog/godot-mcp) — do not look for a local checkout:
   - Preferred: download the release matching this OS/architecture, e.g. via
     `gh release download --repo zigenzoog/godot-mcp --pattern "godot-mcp-<target-triple>.*"`
     (or the direct asset URL from https://github.com/zigenzoog/godot-mcp/releases/latest if `gh` is unavailable), plus
     `gh release download --repo zigenzoog/godot-mcp --pattern "godot-mcp-addon.zip"`.
   - Extract both into a persistent location outside the target Godot project (e.g. `%LOCALAPPDATA%\godot-mcp\` on Windows, `~/.local/share/godot-mcp/` on Linux, `~/Library/Application Support/godot-mcp/` on macOS).
3. Install the Godot Addon:
   - Identify the user's active Godot project directory.
   - Copy the `addons/godot_mcp` folder obtained in step 2 into `<target_project>/addons/godot_mcp`.
   - Ensure the plugin is enabled in `<target_project>/project.godot` under `[editor_plugins]` with `enabled=PackedStringArray("res://addons/godot_mcp/plugin.cfg")`.
4. Register the MCP Client:
   - Locate the active MCP client configuration file (e.g., Claude Code's `.mcp.json`, Claude Desktop, Cursor, or Antigravity).
   - Register the server under the key "godot" with the full path to the binary from step 2 and the `GODOT_PATH` the user provided in step 1.
   - If the target is a project-scoped `.mcp.json` inside a shared repository, **never commit it with real absolute paths**: write a `.mcp.json.example` with placeholders instead, add `.mcp.json` (but not `.mcp.json.example`) to `.gitignore`, and have the user copy the example to `.mcp.json` locally.
5. End-to-End Verification:
   - Trigger a basic verification call (e.g., `godot_get_version` or `godot_get_scene_tree`).
   - Confirm healthy communication and report the detected version and tool list.
```

## Quality Assurance & Development

> This section is for contributing to godot-mcp itself. If you only want to use the addon in your own Godot project, see [Installation](#installation) above — no local build is needed.

The codebase enforces strict testing, formatting, and linting standards:

### Running Tests

```bash
# Run all unit and integration tests
cargo test

# Run tests with output logging
cargo test -- --nocapture
```

### Code Formatting

```bash
# Check formatting
cargo fmt --all -- --check

# Apply formatting
cargo fmt --all
```

### Static Analysis & Lints

```bash
# Run Clippy with zero warnings allowed
cargo clippy --all-targets --all-features -- -D warnings
```

### GDScript Addon Validation

The repository root is itself a minimal Godot project (`project.godot`) whose only purpose is to give the addon's `class_name` declarations somewhere to resolve during headless checks. On a fresh checkout, run the import once so Godot builds its script class cache, then validate:

```bash
# One-time (or after adding/removing scripts): build the script class cache
godot --headless --path . --import

# Validate GDScript syntax using the Godot CLI
godot --headless --check-only --path . --script addons/godot_mcp/plugin.gd
```

Omitting `--path .` will fail with `Could not find type "GodotMcpBridgeServer"`, since class name resolution requires a real project context.

### Cutting a release

Pushing a tag matching `v*.*.*` (e.g. `v0.1.0`) runs `.github/workflows/release.yml`, which builds the server for Windows, Linux, macOS (Intel), and macOS (Apple Silicon), and attaches the archives to a GitHub Release.

## Available MCP Tools

### Project & Engine Control

- `godot_get_version`: Returns the Godot Engine version. Uses the live editor bridge if connected, otherwise falls back to `godot --version`.
- `godot_get_project_info`: Inspects the project currently open in the live editor (name, main scene, enabled features). Requires the editor bridge to be connected.
- `godot_launch_editor`: Launches the Godot Editor for a given project directory as a detached process.
- `godot_run_project`: Runs the project (or a specific scene). If the editor bridge is connected, this plays it via the editor's own Play mode (`EditorInterface.play_main_scene` / `play_custom_scene`) so it can be stopped and queried afterwards; otherwise it falls back to spawning a detached headless CLI process (requires `project_path`), which cannot be stopped or queried via MCP.
- `godot_stop_project`: Stops a scene started by `godot_run_project` via the live editor. Requires the editor bridge.
- `godot_get_run_status`: Reports whether a scene is currently playing via the live editor, and which one. Requires the editor bridge.

### Scene Management

- `godot_get_scene_tree`: Retrieves the node hierarchy of a scene open in the editor. Defaults to the active tab; pass `scene_path` to target a different open scene tab.
- `godot_get_open_scenes`: Lists the `res://` paths of all scene tabs currently open in the editor, and which one is active.
- `godot_open_scene`: Opens a scene file in the editor by its `res://` path.
- `godot_save_scene`: Saves the current open scene in the editor.

### Node Operations

- `godot_add_node`: Adds a new child node to a parent path in the active scene, with undo/redo support.
- `godot_remove_node`: Removes a node by its node path, with undo/redo support.
- `godot_get_node_properties`: Inspects all properties and export variables of a target node.
- `godot_set_node_properties`: Modifies exported properties on a target node, with undo/redo support.

### Scripting

- `godot_create_script`: Writes a new GDScript file to disk with a standard `extends`/`class_name`/`_ready()` template.
- `godot_validate_script`: Runs `godot --headless --check-only` against a `.gd` file and reports syntax errors.
- `godot_resave_resources`: Forces re-import of specific `res://` assets (synchronous, e.g. after editing them externally), or triggers a full project filesystem rescan if no `paths` are given (runs in the background — may still be in progress when the call returns). Requires the editor bridge.

### Visual Inspection

- `godot_capture_viewport`: Captures the active editor viewport as a base64 PNG. Requires the editor bridge to be connected with a scene open.

## Roadmap

The following ideas are referenced in older design notes or reserved config fields but are **not implemented yet**:

- Runtime log streaming and stack trace capture (`godot_get_debug_log`, `godot_get_stack_trace`)
- In-scene camera capture (`godot_capture_camera`), as opposed to the editor viewport
- Scene creation tool (`godot_create_scene`)
- Script attach/read/update tools beyond `godot_create_script` / `godot_validate_script`
- Remote HTTP/SSE transport (`GODOT_MCP_URL`) and synchronous console log capture via a console wrapper (`GODOT_CONSOLE_PATH`)

## License

MIT License. See `LICENSE` for details.
