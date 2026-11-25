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

/// Godot's `JSON.stringify` leaves U+0001..U+001F (other than \b \t \n \f \r) raw inside a
/// string and writes U+000B as `\v`; JSON allows neither. A reply that echoes such text must
/// still reach its caller instead of being dropped until the call times out.
#[tokio::test]
async fn replies_with_raw_control_characters_reach_the_caller() {
    let bridge = MockBridge::start(|_| {
        Reply::Raw(
            "{\"commandId\":\"$ID\",\"result\":{\"name\":\"a\u{1}b\\vc\\\\vd\u{1f}\"},\"status\":\"success\"}"
                .into(),
        )
    })
    .await;
    let client = BridgeClient::new(bridge.port).with_timeout(Duration::from_secs(5));

    let started = Instant::now();
    let value = client
        .send_command("get_node", json!({}))
        .await
        .expect("the reply is read");
    assert_eq!(value["name"], "a\u{1}b\u{b}c\\vd\u{1f}");
    assert!(started.elapsed() < Duration::from_secs(2));
}

/// A reply that cannot be read even after the repair fails its own call at once, with a
/// recovery hint, instead of leaving it to the timeout.
#[tokio::test]
async fn an_unreadable_reply_fails_its_command_at_once() {
    let bridge =
        MockBridge::start(|_| Reply::Raw("{\"commandId\":\"$ID\",\"status\":\"success\",".into()))
            .await;
    let client = BridgeClient::new(bridge.port).with_timeout(Duration::from_secs(10));

    let started = Instant::now();
    let err = client
        .send_command("ping", json!({}))
        .await
        .expect_err("the reply is not JSON");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "waited for the timeout"
    );
    match err {
        McpError::Bridge(message) => {
            assert!(message.contains("could not be read"), "{message}");
            assert!(
                message.contains("godot_update_addon") || message.contains("Retry"),
                "{message}"
            );
        }
        other => panic!("unexpected error class: {other:?}"),
    }
}
