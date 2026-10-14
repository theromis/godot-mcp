//! Drives the real `godot-mcp` binary over stdio against a fake editor bridge.

mod common;

use common::{MockBridge, Reply, Session, fake_godot, free_port, is_tool_error, project_dir};
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Generous upper bound for waiting on a response; speed assertions use their own thresholds.
const FAST: Duration = Duration::from_secs(10);

fn no_godot() -> PathBuf {
    PathBuf::from("godot-binary-that-does-not-exist")
}

#[tokio::test]
async fn initialize_echoes_a_supported_protocol_version() {
    let mut session = Session::start(free_port(), &no_godot()).await;

    let reply = session
        .request("initialize", json!({"protocolVersion": "2025-06-18"}), FAST)
        .await;
    assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");

    let reply = session
        .request("initialize", json!({"protocolVersion": "1999-01-01"}), FAST)
        .await;
    assert_eq!(
        reply["result"]["protocolVersion"], "2025-06-18",
        "unsupported versions get the newest one the server speaks"
    );
}

#[tokio::test]
async fn malformed_json_is_a_parse_error() {
    let mut session = Session::start(free_port(), &no_godot()).await;
    session.send_raw("{this is not json").await;
    let reply = session.recv(FAST).await.expect("parse error response");
    assert_eq!(reply["error"]["code"], -32700);
}

#[tokio::test]
async fn request_without_jsonrpc_field_is_an_invalid_request_keeping_its_id() {
    let mut session = Session::start(free_port(), &no_godot()).await;
    session.send_raw(r#"{"id":77,"method":"ping"}"#).await;
    let reply = session.recv(FAST).await.expect("invalid request response");
    assert_eq!(reply["error"]["code"], -32600);
    assert_eq!(reply["id"], 77);
}

#[tokio::test]
async fn batch_requests_are_rejected_as_invalid_requests() {
    let mut session = Session::start(free_port(), &no_godot()).await;
    session
        .send_raw(r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#)
        .await;
    let reply = session.recv(FAST).await.expect("batch rejection");
    assert_eq!(reply["error"]["code"], -32600);
}

#[tokio::test]
async fn unknown_tool_is_an_invalid_params_error() {
    let mut session = Session::start(free_port(), &no_godot()).await;
    let reply = session
        .call_tool("godot_does_not_exist", json!({}), FAST)
        .await;
    assert_eq!(reply["error"]["code"], -32602);
}

#[tokio::test]
async fn tool_call_without_a_name_is_an_invalid_params_error() {
    let mut session = Session::start(free_port(), &no_godot()).await;
    let reply = session.request("tools/call", json!({}), FAST).await;
    assert_eq!(reply["error"]["code"], -32602);
}

#[tokio::test]
async fn wrong_argument_type_fails_fast_without_reaching_the_editor() {
    let bridge = MockBridge::start(|_| Reply::Never).await;
    let mut session = Session::start(bridge.port, &no_godot()).await;

    let reply = session
        .call_tool("godot_get_node_properties", json!({"node_path": 5}), FAST)
        .await;

    assert!(is_tool_error(&reply));
    let text = common::tool_text(&reply);
    assert!(
        text.contains("node_path") && text.contains("string"),
        "{text}"
    );
    assert!(
        bridge.received().is_empty(),
        "invalid call must not be forwarded"
    );
}

#[tokio::test]
async fn missing_required_argument_comes_with_a_recovery_hint() {
    let mut session = Session::start(free_port(), &no_godot()).await;
    let reply = session
        .call_tool("godot_create_script", json!({}), FAST)
        .await;

    assert!(is_tool_error(&reply));
    let text = common::tool_text(&reply);
    assert!(text.contains("file_path"), "{text}");
    assert!(text.contains("Actionable recovery suggestions"), "{text}");
}

#[tokio::test]
async fn bridge_reply_is_passed_through_to_the_client() {
    let bridge = MockBridge::start(|_| {
        Reply::Ok(json!({"open_scenes": ["res://a.tscn"], "active_scene": "res://a.tscn"}))
    })
    .await;
    let mut session = Session::start(bridge.port, &no_godot()).await;

    let reply = session
        .call_tool("godot_get_open_scenes", json!({}), FAST)
        .await;

    assert!(!is_tool_error(&reply));
    assert!(common::tool_text(&reply).contains("res://a.tscn"));
}

#[tokio::test]
async fn ping_is_answered_while_a_tool_call_is_pending() {
    let bridge = MockBridge::start(|_| Reply::Never).await;
    let mut session = Session::start(bridge.port, &no_godot()).await;

    session
        .send(&json!({"jsonrpc":"2.0","id":100,"method":"tools/call",
            "params":{"name":"godot_get_open_scenes","arguments":{}}}))
        .await;
    let started = Instant::now();
    let reply = session.request("ping", json!({}), FAST).await;

    assert!(reply["result"].is_object());
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "ping must not queue behind the hanging call"
    );
}

#[tokio::test]
async fn cancelled_request_never_gets_a_response() {
    let bridge = MockBridge::start(|_| Reply::Never).await;
    let mut session = Session::start(bridge.port, &no_godot()).await;

    session
        .send(&json!({"jsonrpc":"2.0","id":100,"method":"tools/call",
            "params":{"name":"godot_get_open_scenes","arguments":{}}}))
        .await;
    session
        .send(&json!({"jsonrpc":"2.0","method":"notifications/cancelled",
            "params":{"requestId":100,"reason":"user abort"}}))
        .await;
    session
        .send(&json!({"jsonrpc":"2.0","id":101,"method":"ping"}))
        .await;

    let mut ids = Vec::new();
    while let Some(frame) = session.recv(Duration::from_millis(1500)).await {
        ids.push(frame["id"].clone());
    }
    assert_eq!(ids, vec![json!(101)], "only the ping may be answered");
}

#[tokio::test]
async fn offline_bridge_fails_fast_with_an_actionable_error() {
    let mut session = Session::start(free_port(), &no_godot()).await;

    let started = Instant::now();
    let reply = session
        .call_tool("godot_get_scene_tree", json!({}), Duration::from_secs(10))
        .await;

    assert!(is_tool_error(&reply));
    assert!(common::tool_text(&reply).contains("Launch Godot Editor"));
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn editor_errors_are_reported_as_is_without_cli_fallback() {
    let bridge =
        MockBridge::start(|_| Reply::Err("Scene file does not exist: res://nope.tscn".into()))
            .await;
    let project = project_dir();
    let fake = fake_godot(project.path());
    let mut session = Session::start(bridge.port, &fake).await;

    let reply = session
        .call_tool(
            "godot_run_project",
            json!({"scene": "res://nope.tscn", "project_path": project.path()}),
            Duration::from_secs(10),
        )
        .await;

    assert!(is_tool_error(&reply));
    let text = common::tool_text(&reply);
    assert!(text.contains("Scene file does not exist"), "{text}");
    assert!(
        !text.contains("Spawned"),
        "must not fall back to the CLI: {text}"
    );
    assert!(
        !text.contains("Launch Godot Editor"),
        "an editor-reported error is not a connectivity problem: {text}"
    );
}

#[tokio::test]
async fn cli_fallback_keeps_stdout_clean() {
    let project = project_dir();
    let fake = fake_godot(project.path());
    let mut session = Session::start(free_port(), &fake).await;

    let reply = session
        .call_tool(
            "godot_run_project",
            json!({"project_path": project.path()}),
            Duration::from_secs(10),
        )
        .await;
    assert!(!is_tool_error(&reply), "{reply}");

    // `Session::recv` panics on any line that is not JSON, so draining proves stdout stayed clean.
    session
        .send(&json!({"jsonrpc":"2.0","id":900,"method":"ping"}))
        .await;
    let mut frames = 0;
    while session.recv(Duration::from_millis(1200)).await.is_some() {
        frames += 1;
    }
    assert_eq!(frames, 1, "only the ping response may appear on stdout");
}

#[tokio::test]
async fn bridge_commands_carry_the_shared_token() {
    let bridge = MockBridge::start(|_| Reply::Ok(json!({}))).await;
    let mut session =
        Session::start_with_env(bridge.port, &no_godot(), &[("GODOT_MCP_TOKEN", "s3cret")]).await;

    let reply = session
        .call_tool("godot_get_open_scenes", json!({}), FAST)
        .await;
    assert!(!is_tool_error(&reply));

    let received = bridge.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0]["token"], "s3cret");
    assert_eq!(received[0]["type"], "get_open_scenes");
}

#[tokio::test]
async fn empty_settings_are_treated_as_unset() {
    // MCP client configs and copied templates often contain empty placeholders.
    let mut session = Session::start_with_env(
        free_port(),
        std::path::Path::new(""),
        &[
            ("GODOT_CONSOLE_PATH", ""),
            ("GODOT_MCP_URL", ""),
            ("GODOT_DEBUG_PORT", ""),
        ],
    )
    .await;

    let reply = session.request("tools/list", json!({}), FAST).await;
    assert!(reply["result"]["tools"].is_array());
}

#[tokio::test]
async fn env_example_can_be_copied_as_is() {
    let mut session = Session::start_with_dotenv(include_str!("../.env.example")).await;

    let reply = session.request("tools/list", json!({}), FAST).await;
    assert!(reply["result"]["tools"].is_array());
}

#[tokio::test]
async fn unparsable_dotenv_is_ignored_instead_of_stopping_the_server() {
    // Double-quoted Windows paths are not valid in a .env file.
    let mut session = Session::start_with_dotenv(
        r#"GODOT_PATH="C:\Program Files\Godot\Godot.exe"
"#,
    )
    .await;

    let reply = session.request("ping", json!({}), FAST).await;
    assert!(reply["result"].is_object());
}
