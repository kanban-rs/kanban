use crate::error::AppError;
use crate::pagination::paginate_response;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use kanban_domain::{KanbanError, Prefix};
use kanban_service::api::{Page, PageParams, PrefixResponse};

async fn list_prefixes(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
) -> Result<Json<Page<PrefixResponse>>, AppError> {
    let ctx = state.ctx.lock().await;
    let prefixes = ctx
        .data_store()
        .list_prefixes()
        .map_err(|e| AppError::from(&e))?;
    paginate_response(prefixes.iter().map(PrefixResponse::from).collect(), &params)
}

async fn get_prefix(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<PrefixResponse>, AppError> {
    let ctx = state.ctx.lock().await;
    let normalized = Prefix::normalize(&name);
    let prefix = ctx
        .data_store()
        .get_prefix(&normalized)
        .map_err(|e| AppError::from(&e))?
        .ok_or_else(|| {
            AppError::from(&KanbanError::not_found_by_name(
                "Prefix",
                &normalized,
                Vec::new(),
            ))
        })?;
    Ok(Json(PrefixResponse::from(&prefix)))
}

pub fn read_router() -> Router<AppState> {
    Router::new()
        .route("/v1/prefixes", get(list_prefixes))
        .route("/v1/prefixes/{name}", get(get_prefix))
}
