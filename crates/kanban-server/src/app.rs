use crate::layers::LayerConfig;
use crate::state::AppState;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

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
    router_with(state, LayerConfig::default())
}

/// Like [`router`], but with an explicit [`LayerConfig`].
///
/// `.layer()` wraps outside-in on the LAST call, so calling body-limit then
/// timeout then cors then trace here makes Trace the outermost layer and
/// BodyLimit the innermost. The import route is merged in AFTER the global
/// body-limit layer, so `.layer()` never wraps it; it carries its own,
/// larger `RequestBodyLimitLayer` instead.
pub fn router_with(state: AppState, config: LayerConfig) -> Router {
    let router = Router::new()
        .route("/health", get(health))
        .merge(crate::routes::boards::read_router())
        .merge(crate::routes::boards::write_router())
        .merge(crate::routes::cards::read_router())
        .merge(crate::routes::cards::write_router())
        .merge(crate::routes::cards::flat_read_router())
        .merge(crate::routes::cards::flat_write_router())
        .merge(crate::routes::cards_batch::write_router())
        .merge(crate::routes::columns::read_router())
        .merge(crate::routes::columns::write_router())
        .merge(crate::routes::columns::flat_read_router())
        .merge(crate::routes::columns::flat_write_router())
        .merge(crate::routes::graph::read_router())
        .merge(crate::routes::graph::write_router())
        .merge(crate::routes::prefixes::read_router())
        .merge(crate::routes::sprints::read_router())
        .merge(crate::routes::sprints::write_router())
        .merge(crate::routes::sprints::flat_read_router())
        .merge(crate::routes::sprints::flat_write_router())
        .merge(crate::routes::sprints_lifecycle::write_router())
        .merge(crate::routes::transfer::read_router())
        .merge(crate::routes::events::router());

    let mut router = router
        .layer(RequestBodyLimitLayer::new(config.body_limit_bytes))
        .merge(
            crate::routes::transfer::write_router()
                .layer(DefaultBodyLimit::disable())
                .layer(RequestBodyLimitLayer::new(config.import_body_limit_bytes)),
        );
    if let Some(timeout) = config.timeout {
        router = router.layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            timeout,
        ));
    }
    if let Some(cors) = config.cors.into_layer() {
        router = router.layer(cors);
    }
    router.layer(TraceLayer::new_for_http()).with_state(state)
}
