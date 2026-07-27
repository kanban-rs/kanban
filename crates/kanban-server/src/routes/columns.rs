use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use kanban_service::api::ColumnResponse;
use kanban_service::{KanbanError, KanbanOperations};
use uuid::Uuid;

async fn list_columns(
    State(state): State<AppState>,
    Path(board_id): Path<Uuid>,
) -> Result<Json<Vec<ColumnResponse>>, AppError> {
    let ctx = state.ctx.lock().await;
    let cols = ctx.list_columns(board_id).map_err(|e| AppError::from(&e))?;
    Ok(Json(cols.iter().map(ColumnResponse::from).collect()))
}

async fn get_column(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ColumnResponse>, AppError> {
    let ctx = state.ctx.lock().await;
    let column = ctx
        .get_column(id)
        .map_err(|e| AppError::from(&e))?
        .filter(|c| c.board_id == board_id)
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Column", id)))?;
    Ok(Json(ColumnResponse::from(&column)))
}

pub fn read_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards/{board_id}/columns", get(list_columns))
        .route("/v1/boards/{board_id}/columns/{id}", get(get_column))
}
