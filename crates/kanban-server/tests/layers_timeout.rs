#![cfg(feature = "test-helpers")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kanban_server::app;
use kanban_server::layers::LayerConfig;
use kanban_server::test_helpers::{make_state, TestServer};
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tower::ServiceExt;

async fn read_one_sse_frame(response: &mut reqwest::Response) -> serde_json::Value {
    let mut buf = Vec::new();
    loop {
        let chunk = response
            .chunk()
            .await
            .unwrap()
            .expect("stream ended before a full SSE frame arrived");
        buf.extend_from_slice(&chunk);
        let text = String::from_utf8_lossy(&buf);
        if let Some(idx) = text.find("\n\n") {
            let frame_text = text[..idx].to_string();
            let data_line = frame_text
                .lines()
                .find(|l| l.starts_with("data:"))
                .expect("frame must have a data: line");
            return serde_json::from_str(data_line.trim_start_matches("data:").trim()).unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_request_queued_behind_the_session_lock_returns_408() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let guard = state.lock_session().await;

    let router = app::router_with(
        state.clone(),
        LayerConfig {
            timeout: Some(Duration::from_millis(100)),
            ..Default::default()
        },
    );

    let started = Instant::now();
    let request = Request::builder()
        .method("GET")
        .uri("/v1/boards")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    assert!(started.elapsed() < Duration::from_secs(2));

    drop(guard);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_fast_request_under_the_timeout_still_succeeds() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let router = app::router_with(
        state,
        LayerConfig {
            timeout: Some(Duration::from_secs(5)),
            ..Default::default()
        },
    );

    let request = Request::builder()
        .method("GET")
        .uri("/v1/boards")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_timeout_disabled_leaves_a_queued_request_pending() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let guard = state.lock_session().await;

    let router = app::router_with(
        state.clone(),
        LayerConfig {
            timeout: None,
            ..Default::default()
        },
    );

    let request = Request::builder()
        .method("GET")
        .uri("/v1/boards")
        .body(Body::empty())
        .unwrap();

    let result = tokio::time::timeout(Duration::from_millis(300), router.oneshot(request)).await;

    assert!(result.is_err());

    drop(guard);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sse_stream_survives_past_the_timeout_window() {
    let server = TestServer::start_with_layers(LayerConfig {
        timeout: Some(Duration::from_millis(300)),
        ..Default::default()
    })
    .await;

    let mut events_response = server
        .client()
        .get(format!("{}/v1/events", server.base_url()))
        .send()
        .await
        .unwrap();

    assert_eq!(
        events_response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    tokio::time::sleep(Duration::from_millis(900)).await;

    let board_response = server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .json(&serde_json::json!({"name": "Late Board", "card_prefix": "LB"}))
        .send()
        .await
        .unwrap();
    assert_eq!(board_response.status(), StatusCode::CREATED);

    let frame = tokio::time::timeout(
        Duration::from_secs(5),
        read_one_sse_frame(&mut events_response),
    )
    .await
    .unwrap();

    assert_eq!(frame["entity_type"], "board");
    assert_eq!(frame["kind"], "created");

    drop(events_response);
    server.shutdown().await;
}
