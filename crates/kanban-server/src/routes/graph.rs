use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use kanban_service::api::CardGraphResponse;
use uuid::Uuid;

async fn get_card_graph(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CardGraphResponse>, AppError> {
    let ctx = state.ctx.lock().await;
    crate::routes::cards::do_get_card(&ctx, id)?;
    let graph = ctx.graph().map_err(|e| AppError::from(&e))?;
    Ok(Json(CardGraphResponse::from_graph(id, &graph)))
}

pub fn read_router() -> Router<AppState> {
    Router::new().route("/v1/cards/{id}/graph", get(get_card_graph))
}
