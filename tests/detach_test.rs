//! A process launched for the user (editor, game) must not stay tied to the MCP server.

mod common;

use common::{free_port, project_dir, sleeping_godot, tool_text};
use serde_json::{Value, json};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

/// An MCP client reads the server's stderr until it closes. A launched editor that inherited
/// the pipe keeps it open for as long as the editor runs, so the client would wait forever
/// after the server is gone.
#[tokio::test]
async fn launched_process_does_not_hold_the_servers_stderr_open() {
    let project = project_dir();
    let godot = sleeping_godot(project.path());
    let home = tempfile::tempdir().expect("hermetic home");

    let mut server = Command::new(env!("CARGO_BIN_EXE_godot-mcp"))
        .env("GODOT_PATH", &godot)
        .env("GODOT_DEBUG_PORT", free_port().to_string())
        .env_remove("GODOT_MCP_TOKEN")
        .env("APPDATA", home.path())
        .env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path())
        .current_dir(home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn godot-mcp");

    let mut stdin = server.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(server.stdout.take().expect("stdout")).lines();
    let mut stderr = server.stderr.take().expect("stderr");

    let request = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "godot_launch_editor",
                   "arguments": {"project_path": project.path()}}});
    stdin
        .write_all(format!("{request}\n").as_bytes())
        .await
        .expect("send request");
    stdin.flush().await.expect("flush request");

    let line = timeout(Duration::from_secs(60), stdout.next_line())
        .await
        .expect("a response in time")
        .expect("read response")
        .expect("a response line");
    let response: Value = serde_json::from_str(&line).expect("JSON response");
    let reply = tool_text(&json!({ "result": response["result"] }));
    assert!(reply.contains("launched"), "{reply}");

    // The launched process now runs for several seconds; the server goes away first.
    server.kill().await.expect("kill server");

    let mut output = Vec::new();
    let closed = timeout(Duration::from_secs(5), stderr.read_to_end(&mut output)).await;
    assert!(
        closed.is_ok(),
        "the launched process still holds the server's stderr open"
    );
}
