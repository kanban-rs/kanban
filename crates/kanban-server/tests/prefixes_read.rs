#![cfg(feature = "test-helpers")]

use axum::http::StatusCode;
use kanban_domain::Prefix;
use kanban_server::test_helpers::{json_of, make_state, send};
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_prefixes_empty_returns_200_empty_page() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = send(&state, "GET", "/v1/prefixes", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["items"], serde_json::json!([]));
    assert_eq!(json["total"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_prefixes_returns_seeded_rows() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let ctx = state.ctx.lock().await;
        let mut kan = Prefix::new("kan");
        kan.card_counter = 5;
        ctx.data_store().upsert_prefix(kan).unwrap();
        let mut feat = Prefix::new("feat");
        feat.sprint_counter = 2;
        ctx.data_store().upsert_prefix(feat).unwrap();
    }

    let response = send(&state, "GET", "/v1/prefixes", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let arr = json["items"].as_array().expect("items should be an array");
    assert_eq!(arr.len(), 2);
    let names: std::collections::HashSet<_> =
        arr.iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, std::collections::HashSet::from(["kan", "feat"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_returns_row_for_existing_name() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let ctx = state.ctx.lock().await;
        let mut kan = Prefix::new("kan");
        kan.card_counter = 7;
        kan.sprint_counter = 3;
        ctx.data_store().upsert_prefix(kan).unwrap();
    }

    let response = send(&state, "GET", "/v1/prefixes/kan", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"], "kan");
    assert_eq!(json["card_counter"], 7);
    assert_eq!(json["sprint_counter"], 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_normalizes_name_before_lookup() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let ctx = state.ctx.lock().await;
        ctx.data_store().upsert_prefix(Prefix::new("kan")).unwrap();
    }

    let response = send(&state, "GET", "/v1/prefixes/KAN", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"], "kan");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_unknown_name_returns_404_not_found() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = send(&state, "GET", "/v1/prefixes/missing", None).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND_BY_NAME");
}
