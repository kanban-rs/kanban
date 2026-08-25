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

    let response = send(&state, "GET", "/v1/prefixes?name=kan", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"], "kan");
    assert_eq!(json["last_card_number"], 7);
    assert_eq!(json["last_sprint_number"], 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_normalizes_name_before_lookup() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let ctx = state.ctx.lock().await;
        ctx.data_store().upsert_prefix(Prefix::new("kan")).unwrap();
    }

    let response = send(&state, "GET", "/v1/prefixes?name=KAN", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"], "kan");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_unknown_name_returns_404_not_found() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = send(&state, "GET", "/v1/prefixes?name=missing", None).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND_BY_NAME");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_unknown_name_populates_available_from_the_workspace() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let ctx = state.ctx.lock().await;
        ctx.data_store().upsert_prefix(Prefix::new("kan")).unwrap();
        ctx.data_store().upsert_prefix(Prefix::new("feat")).unwrap();
    }

    let response = send(&state, "GET", "/v1/prefixes?name=missing", None).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    let available: std::collections::HashSet<_> = json["message"]
        .as_str()
        .unwrap()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| *s == "kan" || *s == "feat")
        .collect();
    assert_eq!(
        available,
        std::collections::HashSet::from(["kan", "feat"]),
        "the 404 message should name the available prefixes, got: {}",
        json["message"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_empty_name_is_addressable() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let ctx = state.ctx.lock().await;
        let mut empty = Prefix::new("");
        empty.card_counter = 4;
        ctx.data_store().upsert_prefix(empty).unwrap();
    }

    let response = send(&state, "GET", "/v1/prefixes?name=", None).await;

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "the empty-name row must be addressable, not silently 404"
    );
    let json = json_of(response).await;
    assert_eq!(json["name"], "");
    assert_eq!(json["last_card_number"], 4);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_prefixes_follows_pagination_past_the_first_page() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let ctx = state.ctx.lock().await;
        for i in 0..75 {
            ctx.data_store()
                .upsert_prefix(Prefix::new(&format!("p{i}")))
                .unwrap();
        }
    }

    let page1 = send(&state, "GET", "/v1/prefixes?page=1&page_size=50", None).await;
    let page1 = json_of(page1).await;
    assert_eq!(page1["items"].as_array().unwrap().len(), 50);
    assert_eq!(page1["total_pages"], 2);

    let page2 = send(&state, "GET", "/v1/prefixes?page=2&page_size=50", None).await;
    let page2 = json_of(page2).await;
    assert_eq!(page2["items"].as_array().unwrap().len(), 25);
}
