//! `BridgeClient` against a fake editor: error classes, multiplexing, reconnects.

mod common;

use common::{MockBridge, Reply, free_port};
use godot_mcp::bridge::BridgeClient;
use godot_mcp::error::McpError;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

#[tokio::test]
async fn reports_unavailable_when_nothing_listens() {
    let client = BridgeClient::new(free_port());
    let err = client
        .send_command("ping", json!({}))
        .await
        .expect_err("no editor is running");
    assert!(matches!(err, McpError::BridgeUnavailable(_)), "{err:?}");
}

#[tokio::test]
async fn editor_reported_errors_keep_their_message() {
    let bridge = MockBridge::start(|_| Reply::Err("Node not found at path: X".into())).await;
    let client = BridgeClient::new(bridge.port);

    let err = client
        .send_command("remove_node", json!({"node_path": "X"}))
        .await
        .expect_err("editor reports an error");

    match err {
        McpError::ToolExecution(message) => assert_eq!(message, "Node not found at path: X"),
        other => panic!("unexpected error class: {other:?}"),
    }
}

#[tokio::test]
async fn concurrent_commands_are_matched_by_command_id() {
    let bridge = MockBridge::start(|cmd| match cmd["type"].as_str() {
        Some("slow") => Reply::After(Duration::from_millis(600), json!({"which": "slow"})),
        _ => Reply::Ok(json!({"which": "fast"})),
    })
    .await;
    let client = BridgeClient::new(bridge.port);

    let started = Instant::now();
    let (slow, fast) = tokio::join!(
        async {
            let value = client.send_command("slow", json!({})).await;
            (value, started.elapsed())
        },
        async {
            let value = client.send_command("fast", json!({})).await;
            (value, started.elapsed())
        }
    );

    assert_eq!(slow.0.expect("slow ok")["which"], "slow");
    assert_eq!(fast.0.expect("fast ok")["which"], "fast");
    assert!(
        fast.1 < Duration::from_millis(400),
        "fast reply was held back by the slow one: {:?}",
        fast.1
    );
}

#[tokio::test]
async fn reconnects_after_the_editor_restarts() {
    let first = MockBridge::start(|_| Reply::Ok(json!({"gen": 1}))).await;
    let port = first.port;
    let client = BridgeClient::new(port);
    assert_eq!(
        client.send_command("ping", json!({})).await.expect("first")["gen"],
        1
    );

    first.shutdown();
    drop(first);
    let _second = MockBridge::start_on(port, |_| Reply::Ok(json!({"gen": 2}))).await;

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last: Option<Value> = None;
    while Instant::now() < deadline {
        if let Ok(value) = client.send_command("ping", json!({})).await {
            last = Some(value);
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert_eq!(last.expect("client must reconnect")["gen"], 2);
}

#[tokio::test]
async fn unanswered_commands_time_out_with_a_bridge_error() {
    let bridge = MockBridge::start(|_| Reply::Never).await;
    let client = BridgeClient::new(bridge.port).with_timeout(Duration::from_millis(300));

    let err = client
        .send_command("ping", json!({}))
        .await
        .expect_err("nobody answers");
    assert!(matches!(err, McpError::Bridge(_)), "{err:?}");
}

/// A fake addon of bridge protocol 2 that lists `open_scene` with one parameter.
fn listing_addon(command: &Value) -> Reply {
    match command["type"].as_str() {
        Some("get_version") => Reply::Ok(json!({
            "version_string": "4.5.stable",
            "protocol": 2,
            "commands": {"open_scene": ["scene_path"], "ping": []}
        })),
        _ => Reply::Ok(json!({"done": true})),
    }
}

fn sent_types(bridge: &MockBridge) -> Vec<String> {
    bridge
        .received()
        .iter()
        .filter_map(|command| command["type"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn a_parameter_the_addon_does_not_list_is_refused_before_it_is_sent() {
    let bridge = MockBridge::start(listing_addon).await;
    let client = BridgeClient::new(bridge.port);

    let err = client
        .send_command(
            "open_scene",
            json!({"scene_path": "res://a.tscn", "reload": true}),
        )
        .await
        .expect_err("the addon has no reload");
    match err {
        McpError::ToolExecution(message) => assert!(
            message.starts_with("Unknown parameter 'reload' for command 'open_scene'"),
            "{message}"
        ),
        other => panic!("unexpected error class: {other:?}"),
    }
    assert_eq!(sent_types(&bridge), ["get_version"]);

    // A listed parameter, a null one and a command the addon does not list are sent; the
    // commands are read once per connection.
    client
        .send_command(
            "open_scene",
            json!({"scene_path": "res://a.tscn", "reload": null}),
        )
        .await
        .expect("listed parameters");
    client
        .send_command("__test_hook", json!({"anything": 1}))
        .await
        .expect("an unlisted command is left to the addon");
    assert_eq!(
        sent_types(&bridge),
        ["get_version", "open_scene", "__test_hook"]
    );
}

#[tokio::test]
async fn an_addon_without_a_protocol_is_sent_every_parameter() {
    let bridge = MockBridge::start(|command| match command["type"].as_str() {
        Some("get_version") => Reply::Ok(json!({"version_string": "4.5.stable"})),
        _ => Reply::Ok(json!({"done": true})),
    })
    .await;
    let client = BridgeClient::new(bridge.port);

    client
        .send_command(
            "open_scene",
            json!({"scene_path": "res://a.tscn", "reload": true}),
        )
        .await
        .expect("an old addon gets the call as before");
    let received = bridge.received();
    let sent = received
        .iter()
        .find(|command| command["type"] == "open_scene")
        .expect("open_scene was sent");
    assert_eq!(sent["params"]["reload"], true);
}

#[tokio::test]
async fn an_addon_that_cannot_list_its_commands_still_gets_the_call() {
    let bridge = MockBridge::start(|command| match command["type"].as_str() {
        Some("get_version") => Reply::Never,
        _ => Reply::Ok(json!({"done": true})),
    })
    .await;
    let client = BridgeClient::new(bridge.port).with_timeout(Duration::from_millis(400));

    let answer = client
        .send_command("open_scene", json!({"scene_path": "res://a.tscn"}))
        .await
        .expect("the call goes out after the list times out");
    assert_eq!(answer["done"], true);
}
