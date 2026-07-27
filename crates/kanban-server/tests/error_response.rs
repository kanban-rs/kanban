//! Tests for the AppError IntoResponse bridge. Verify that axum error responses
//! map correctly to HTTP status codes and flat JSON envelopes.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use kanban_domain::KanbanError;
use kanban_server::error::AppError;
use kanban_service::api::ApiError;
use kanban_service::api::ErrorCode;
use uuid::Uuid;

#[tokio::test]
async fn test_app_error_uses_service_http_status_for_each_code() {
    // Exhaustive table: all 19 ErrorCode variants mapped to their expected HTTP status.
    let cases = vec![
        (ErrorCode::NotFound, 404),
        (ErrorCode::NotFoundByName, 404),
        (ErrorCode::EdgeNotFound, 404),
        (ErrorCode::ValidationFailed, 422),
        (ErrorCode::SprintBoardMismatch, 422),
        (ErrorCode::SelfReference, 422),
        (ErrorCode::BatchResolutionFailed, 400),
        (ErrorCode::Ambiguous, 409),
        (ErrorCode::WipLimitExceeded, 409),
        (ErrorCode::ConflictDetected, 409),
        (ErrorCode::AlreadyExists, 409),
        (ErrorCode::UnsupportedVersion, 409),
        (ErrorCode::DependencyError, 409),
        (ErrorCode::CycleDetected, 409),
        (ErrorCode::DuplicateEdge, 409),
        (ErrorCode::IoError, 500),
        (ErrorCode::SerializationError, 500),
        (ErrorCode::DatabaseError, 500),
        (ErrorCode::InternalError, 500),
    ];

    for (code, expected_status) in cases {
        let app_error = AppError(ApiError::new(code, "test message"));
        let response = app_error.into_response();
        assert_eq!(
            response.status().as_u16(),
            expected_status,
            "ErrorCode {:?} should map to status {}",
            code,
            expected_status
        );
    }
}

#[tokio::test]
async fn test_app_error_body_is_flat_code_and_message() {
    let app_error = AppError(ApiError::new(ErrorCode::NotFound, "board not found"));
    let response = app_error.into_response();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify the response is a flat object with "code" and "message" at top level.
    assert!(json["code"].is_string(), "code should be a string");
    assert!(json["message"].is_string(), "message should be a string");
    assert_eq!(json["code"], "NOT_FOUND");
    assert_eq!(json["message"], "board not found");

    // Ensure there is no nested "error" key (the body must be flat, not nested).
    assert!(
        json["error"].is_null(),
        "body should be flat, not nested under 'error' key"
    );
}

#[tokio::test]
async fn test_app_error_not_found_maps_to_404() {
    let kanban_err = KanbanError::not_found("Board", Uuid::nil());
    let app_error = AppError::from(&kanban_err);
    let response = app_error.into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_app_error_conflict_maps_to_409() {
    let kanban_err = KanbanError::ConflictDetected {
        path: "test.json".into(),
        source: None,
    };
    let app_error = AppError::from(&kanban_err);
    let response = app_error.into_response();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_app_error_internal_maps_to_500() {
    let kanban_err = KanbanError::Internal("something went wrong".into());
    let app_error = AppError::from(&kanban_err);
    let response = app_error.into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
