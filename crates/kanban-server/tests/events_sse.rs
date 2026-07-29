#![cfg(feature = "test-helpers")]

use kanban_server::test_helpers::TestServer;
use std::time::Duration;

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
async fn test_sse_stream_emits_frame_on_mutation() {
    let server = TestServer::start().await;

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

    let _board_response = server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .json(&serde_json::json!({"name": "Test Board", "card_prefix": "TB"}))
        .send()
        .await
        .unwrap();

    let frame = read_one_sse_frame(&mut events_response).await;

    assert!(frame.get("writer_instance_id").is_some());
    assert!(frame.get("detected_at").is_some());
    assert!(frame.get("correlation_id").is_some());
    assert!(frame.get("issued_by").is_some());

    drop(events_response);
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sse_frame_carries_writer_instance_id() {
    let server = TestServer::start().await;

    let health_response = server
        .client()
        .get(format!("{}/health", server.base_url()))
        .send()
        .await
        .unwrap();
    let health_json: serde_json::Value = health_response.json().await.unwrap();
    let expected_instance_id = health_json["instance_id"].as_str().unwrap();

    let mut events_response = server
        .client()
        .get(format!("{}/v1/events", server.base_url()))
        .send()
        .await
        .unwrap();

    let _board_response = server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .json(&serde_json::json!({"name": "Test Board 2", "card_prefix": "TB2"}))
        .send()
        .await
        .unwrap();

    let frame = read_one_sse_frame(&mut events_response).await;

    assert_eq!(
        frame["writer_instance_id"].as_str().unwrap(),
        expected_instance_id
    );

    drop(events_response);
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sse_two_subscribers_both_receive_frame() {
    let server = TestServer::start().await;

    let mut events_response_1 = server
        .client()
        .get(format!("{}/v1/events", server.base_url()))
        .send()
        .await
        .unwrap();

    let mut events_response_2 = server
        .client()
        .get(format!("{}/v1/events", server.base_url()))
        .send()
        .await
        .unwrap();

    let _board_response = server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .json(&serde_json::json!({"name": "Test Board 3", "card_prefix": "TB3"}))
        .send()
        .await
        .unwrap();

    let frame1 = read_one_sse_frame(&mut events_response_1).await;
    let frame2 = read_one_sse_frame(&mut events_response_2).await;

    assert_eq!(
        frame1["correlation_id"].as_str().unwrap(),
        frame2["correlation_id"].as_str().unwrap()
    );

    drop(events_response_1);
    drop(events_response_2);
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sse_keep_alive_holds_connection_open() {
    let server = TestServer::start().await;

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

    let timeout_result =
        tokio::time::timeout(Duration::from_millis(200), events_response.chunk()).await;

    match timeout_result {
        Err(_timeout) => {
            // Timeout is fine - connection held open with no data yet
        }
        Ok(Ok(Some(_chunk))) => {
            // Keep-alive comment line received
        }
        Ok(_) => panic!("chunk() returned unexpected result"),
    }

    drop(events_response);
    server.shutdown().await;
}
