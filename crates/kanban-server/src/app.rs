use crate::state::AppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    instance_id: uuid::Uuid,
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        instance_id: state.instance_id,
    })
}

/// The single `Router` composition point. Entity route cards extend this via
/// `.merge`/`.nest` rather than building their own `Router`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(crate::routes::boards::read_router())
        .merge(crate::routes::boards::write_router())
        .merge(crate::routes::columns::read_router())
        .with_state(state)
}
