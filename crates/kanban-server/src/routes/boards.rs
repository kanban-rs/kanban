use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use kanban_service::api::BoardResponse;
use kanban_service::{KanbanError, KanbanOperations};
use uuid::Uuid;

async fn list_boards(State(state): State<AppState>) -> Result<Json<Vec<BoardResponse>>, AppError> {
    let ctx = state.ctx.lock().await;
    let boards = ctx.list_boards().map_err(|e| AppError::from(&e))?;
    Ok(Json(boards.iter().map(BoardResponse::from).collect()))
}

async fn get_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<BoardResponse>, AppError> {
    let ctx = state.ctx.lock().await;
    let board = ctx
        .get_board(id)
        .map_err(|e| AppError::from(&e))?
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Board", id)))?;
    Ok(Json(BoardResponse::from(&board)))
}

pub fn read_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards", get(list_boards))
        .route("/v1/boards/{id}", get(get_board))
}
