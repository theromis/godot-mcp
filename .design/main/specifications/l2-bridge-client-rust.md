# Bridge Client (Rust)

**Version:** 1.0.0
**Status:** Stable
**Layer:** implementation
**Implements:** l1-editor-bridge.md

## Overview

The server side of the live editor bridge: a cloneable client that keeps one WebSocket connection to the editor, multiplexes concurrent commands over it by id, attaches the shared secret, classifies failures, and reconnects after an editor restart.

## Related Specifications

- [l1-editor-bridge.md](l1-editor-bridge.md) - Parent concept.
- [l2-godot-editor-addon.md](l2-godot-editor-addon.md) - The editor-side peer.
- [l2-tool-handlers.md](l2-tool-handlers.md) - Callers of `send_command`.

## 1. Motivation

Captures the timing constants and the connection state machine, which are easy to change accidentally and directly determine how fast a tool fails when no editor is running.

## 2. Constraints & Assumptions

- Client library: `tokio-tungstenite` over a `TcpStream` connected to `127.0.0.1:{port}`.
- Timing: connect 300 ms, WebSocket handshake 2 s, command reply 15 s (overridable for tests), cooldown after a failed connect 500 ms.
- Secret source order: `GODOT_MCP_TOKEN` (trimmed, non-empty), else the token file at the OS user-data directory, mirroring the editor's `OS.get_data_dir()`: `%APPDATA%` on Windows, `~/Library/Application Support` on macOS, `$XDG_DATA_HOME` or `~/.local/share` elsewhere, then `godot-mcp/bridge-{port}.token`.
- The secret is read once per connection, so a restarted editor (new secret) is picked up on reconnect.

## 4. Invariant Compliance

| L1 Invariant | Implementation |
| --- | --- |
| BR-1 Loopback only | The client only ever dials `127.0.0.1`; the listening side is enforced by the addon. |
| BR-2 Shared secret | Every `BridgeCommand` carries `token` from `read_token(port)`; the editor's "Unauthorized" reply surfaces as `ToolExecution` and gets token-specific hints in the tool layer. |
| BR-3 Exactly one reply | A `oneshot` reply slot per command; a guard removes the slot when the caller stops waiting (timeout or cancellation); on connection loss every pending slot receives a connection-closed error. |
| BR-4 Correlated commands | `commandId` is a UUID v4; a reader task delivers each reply to the slot with the same id and ignores unknown ids and malformed frames (logged). |
| BR-5 Undoable mutations | Enforced editor-side; the client sends mutations as ordinary commands. |
| BR-6 Scene confinement | Enforced editor-side. |
| BR-7 All-or-nothing updates | Enforced editor-side; the reply's `applied` map is passed through unchanged. |
| BR-8 Fast, classified failures | Connect/handshake failures → `BridgeUnavailable` and a 500 ms cooldown that fails further attempts immediately; broken connection or reply timeout → `Bridge`; `status: error` → `ToolExecution`. A dead connection is detected via an `alive` flag and replaced on the next command. |
| BR-9 Version gate | Enforced editor-side. |
| BR-10 Editor-only | Not applicable to the server binary. |
| BR-11 Project-confined paths | Enforced editor-side; host-side paths are confined by the tool layer. |

## 5. Detailed Design

### 5.1 Concurrency Model

```mermaid
graph LR
    C1[Tool task A] -->|send_command| Q[(outbound queue)]
    C2[Tool task B] -->|send_command| Q
    Q --> W[Connection task]
    W -->|text frames| E[Editor]
    E -->|replies| W
    W -->|by commandId| P[(pending map)]
    P --> C1
    P --> C2
```

- The state lock is held only while obtaining or establishing the connection, never while waiting for a reply.
- WebSocket pings from the editor are answered with pongs by the connection task.

## 7. Drawbacks & Alternatives

- **Connection per command** was the original design; replaced because each call paid a connect and logged on the editor side.
- **Longer connect timeout**: unnecessary on loopback, where a closed port refuses immediately; the short timeout keeps offline tool calls around 300 ms instead of seconds.

## Canonical References

| Alias | Path | Purpose |
| --- | --- | --- |
| `[CLIENT]` | `src/bridge.rs` | Client, connection task, token path |
| `[TESTS]` | `tests/bridge_client_test.rs` | Multiplexing, reconnection, timeout and error-class tests |

## Document History

| Version | Date | Author | Description |
| --- | --- | --- | --- |
| 1.0.0 | 2026-10-16 | Agent (magic.analyze first-time) | Reverse-engineered from the code at 94e4540. |
