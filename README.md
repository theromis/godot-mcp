# Godot MCP Server

A fast, cross-platform [Model Context Protocol](https://modelcontextprotocol.io) (MCP) server, written in Rust, that lets AI assistants inspect and edit a live Godot 4 editor.

- **One native binary** for Windows, Linux, and macOS (Intel and Apple Silicon). No Node.js or Python.
- **Live editor bridge.** A small GDScript plugin exposes the open scenes over a local WebSocket (loopback only, token-protected). Node edits go through `EditorUndoRedoManager`, so anything the AI changes can be undone with `Ctrl+Z`.
- **Headless fallback.** Without a running editor, the server calls the Godot binary directly to report the engine version, validate scripts, create scripts, and launch the editor or the project.

## Installation

Requirements: **Godot 4.5+** (tested with 4.8-dev) and an MCP client such as Claude Code, Claude Desktop, Cursor, VS Code, or Antigravity. No Rust toolchain needed.

### Option A: let your AI assistant install it

Open your Godot project in the assistant and paste:

```text
Install the Godot MCP server (https://github.com/zigenzoog/godot-mcp) for this Godot project. Use the GitHub release, not a local clone of that repository.

1. Detect the OS and CPU architecture (Windows, Linux, or macOS; x86_64 or arm64).
2. Ask me for the full path to my Godot executable. Never guess it: Windows builds are portable, version-named files (e.g. Godot_v4.5-stable_win64.exe) stored wherever I put them. Run `<path> --version` to confirm it works.
3. Download the server archive for this platform and godot-mcp-addon.zip from https://github.com/zigenzoog/godot-mcp/releases/latest, e.g.:
   gh release download --repo zigenzoog/godot-mcp --pattern "godot-mcp-<target-triple>.*"
   gh release download --repo zigenzoog/godot-mcp --pattern "godot-mcp-addon.zip"
   Extract the server to a permanent folder outside the project: %LOCALAPPDATA%\godot-mcp\ (Windows), ~/.local/share/godot-mcp/ (Linux), ~/Library/Application Support/godot-mcp/ (macOS).
4. Copy addons/godot_mcp from the addon archive into <project>/addons/godot_mcp and enable it in project.godot:
   [editor_plugins]
   enabled=PackedStringArray("res://addons/godot_mcp/plugin.cfg")
5. Register the server as "godot" in my MCP client config, with the binary from step 3 as the command and GODOT_PATH from step 2 in env. If the config is a project-scoped .mcp.json in a shared repository, do not commit real paths: write a .mcp.json.example with placeholders and add .mcp.json to .gitignore.
6. Ask me to restart the MCP client and (re)open the project in the Godot editor, then call godot_get_version and report the result.
```

### Option B: install manually

**1. Download** the server for your platform and the addon from the [latest release](https://github.com/zigenzoog/godot-mcp/releases/latest):

| Platform | Asset |
| --- | --- |
| Windows x86_64 | `godot-mcp-x86_64-pc-windows-msvc.zip` |
| Linux x86_64 | `godot-mcp-x86_64-unknown-linux-gnu.tar.gz` |
| macOS Apple Silicon | `godot-mcp-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `godot-mcp-x86_64-apple-darwin.tar.gz` |
| Godot addon (required, any platform) | `godot-mcp-addon.zip` |

**2. Enable the plugin.** Copy `addons/godot_mcp` from the addon archive into your project's `addons/` folder, open the project, and enable **Godot MCP Bridge** in **Project → Project Settings → Plugins**. The bridge listens on port `9333`.

**3. Register the server** in your MCP client, pointing `command` at the extracted binary and `GODOT_PATH` at your Godot executable:

```json
{
  "mcpServers": {
    "godot": {
      "command": "/path/to/godot-mcp",
      "args": [],
      "env": {
        "GODOT_PATH": "/path/to/Godot_v4.x-stable",
        "GODOT_DEBUG_PORT": "9333"
      }
    }
  }
}
```

| Client | Config location |
| --- | --- |
| Claude Code | `.mcp.json` in the project root, or `claude mcp add godot -e GODOT_PATH=/path/to/godot -- /path/to/godot-mcp` |
| Claude Desktop | `claude_desktop_config.json` |
| Cursor | `.cursor/mcp.json` |
| VS Code | `.vscode/mcp.json` (uses a top-level `servers` key instead of `mcpServers`) |
| Antigravity / others | the client's MCP settings file |

On Windows, escape backslashes in JSON: `"C:\\Tools\\godot-mcp\\godot-mcp.exe"`.

Both paths are machine-specific. Don't commit a `.mcp.json` with real paths: commit a `.mcp.json.example` with placeholders, add `.mcp.json` to `.gitignore`, and let each developer copy it locally.

## Configuration

The server reads its settings from CLI flags, the environment (e.g. the MCP client's `env` block), or a `.env` file in its working directory or a parent directory, in that order of precedence.

| Variable | Flag | Default | Purpose |
| --- | --- | --- | --- |
| `GODOT_PATH` | `--godot-path` | auto-detect | Godot executable. If unset or missing, the server tries a few common install locations, then `godot` on `PATH`. Set it explicitly. |
| `GODOT_DEBUG_PORT` | `--debug-port` | `9333` | Editor bridge port. |
| `GODOT_MCP_TOKEN` | | generated | Shared secret for the bridge. Normally unnecessary, see [Security](#security). |
| `GODOT_CONSOLE_PATH`, `GODOT_MCP_URL` | | | Reserved for planned features; currently unused. |

An empty value counts as "not set", so placeholders in a copied `.env.example` or an MCP client's `env` block are harmless.

In a `.env` file, put **Windows paths in single quotes** (`GODOT_PATH='C:\Tools\Godot\Godot.exe'`). In double quotes a backslash starts an escape sequence and the whole file fails to load; the server logs a warning when that happens. See [.env.example](.env.example).

The plugin picks its port from the project setting `godot_mcp/bridge_port` if you add one, otherwise from the editor's own `GODOT_DEBUG_PORT` environment variable, otherwise `9333`. Keep it equal to the server's port. Add the project setting `godot_mcp/verbose` = `true` to log every client connection in the editor's Output panel.

## Security

- The bridge listens on `127.0.0.1` only, so other machines cannot reach it.
- Every command must carry a random token. The plugin writes it when the editor starts to `<user data dir>/godot-mcp/bridge-<port>.token` (readable by your user only), and the server reads it from there. This also keeps web pages in your browser from driving the editor. Both processes must therefore run as the same OS user. If they can't (containers, Flatpak, different users), set the same `GODOT_MCP_TOKEN` for both instead.
- `godot_create_script` only writes `.gd` files inside the project, never overwrites without `overwrite: true`, and accepts only plain identifiers for `class_name` and `extends_class`.
- Node paths are resolved inside the edited scene only; the scene root can't be removed.

## Tools

"Editor" means the tool needs a running Godot editor with the plugin enabled.

| Tool | Needs | What it does |
| --- | --- | --- |
| `godot_get_version` | — | Engine version, from the editor or the Godot binary. |
| `godot_get_project_info` | Editor | Name, main scene, and features of the open project. |
| `godot_launch_editor` | — | Launches the editor for a project directory. |
| `godot_run_project` | — | Plays the main scene or a given `res://` scene. With the editor, uses its Play mode (stoppable). Without it, starts a separate game process (requires `project_path`, can't be stopped via MCP). |
| `godot_stop_project` | Editor | Stops the editor's Play mode. |
| `godot_get_run_status` | Editor | Whether a scene is playing, and which one. |
| `godot_get_open_scenes` | Editor | Open scene tabs and the active one. |
| `godot_open_scene` | Editor | Opens a scene by `res://` path. |
| `godot_save_scene` | Editor | Saves the active scene. |
| `godot_get_scene_tree` | Editor | Node hierarchy of the active scene, or of another open tab via `scene_path`. Paths are relative to the scene root. |
| `godot_add_node` | Editor | Adds a node of any instantiable `Node` class or global script class (undoable). |
| `godot_remove_node` | Editor | Removes a node, restoring order and ownership on undo. |
| `godot_get_node_properties` | Editor | Reads a node's editor-visible properties as `{type, value}` in Godot's text format. |
| `godot_set_node_properties` | Editor | Sets properties (undoable), converting values to the property's type: `[10, 20]` or `{"x": 10, "y": 20}` for vectors, `"#ff0000"` for colors, `res://` paths for resources. The whole call is rejected if any property is unknown or unconvertible. |
| `godot_capture_viewport` | Editor with a window | Screenshot of the 2D or 3D editor viewport as PNG (`view`, `max_dimension` options). Switches the editor to that main screen; doesn't work with `--headless`. |
| `godot_create_script` | — | Writes a `.gd` file from an `extends` / `class_name` / `_ready()` template into the project (`res://` path, plus `project_path` when no editor is connected). Refuses to overwrite unless `overwrite: true`. |
| `godot_validate_script` | — | Syntax-checks a `.gd` file with `godot --headless --check-only`. |
| `godot_resave_resources` | Editor | Refreshes the given `res://` files after external edits: assets are re-imported (synchronous), other files re-scanned. Without paths, rescans the whole project (background). |

## How it works

```
AI client <-stdio JSON-RPC-> godot-mcp <-WebSocket :9333-> addons/godot_mcp (inside the Godot editor)
                                 |
                                 +-> Godot binary (headless fallback)
```

## Roadmap

Not implemented yet:

- Runtime log streaming and stack traces (`godot_get_debug_log`, `godot_get_stack_trace`)
- In-game camera capture (`godot_capture_camera`)
- Scene creation (`godot_create_scene`) and script read/update/attach tools
- HTTP/SSE transport (`GODOT_MCP_URL`) and console log capture (`GODOT_CONSOLE_PATH`)

## Development

For contributors to godot-mcp itself:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets

# GDScript check. The repo root is a minimal Godot project that gives the addon's
# class_name declarations somewhere to resolve.
godot --headless --path . --import    # first run, or after adding scripts
godot --headless --check-only --path . --script addons/godot_mcp/plugin.gd
godot --headless --check-only --path . --script addons/godot_mcp/bridge_server.gd

# End-to-end tests: start a real headless editor with the addon (needs Godot 4.5+).
# On Windows, put the path in GODOT_PATH; GODOT_CONSOLE_PATH adds editor logs to failures.
GODOT_PATH=/path/to/godot cargo test --test editor_e2e_test -- --ignored
```

CI runs all of the above on every push and pull request.

**Releasing:** bump the version in `Cargo.toml` and `addons/godot_mcp/plugin.cfg` together (`tests/version_sync_test.rs` fails if they differ), commit, and push a `vX.Y.Z` tag. The release workflow builds all four platforms and attaches the archives plus `godot-mcp-addon.zip` to a GitHub Release.

## License

MIT. See [LICENSE](LICENSE).
