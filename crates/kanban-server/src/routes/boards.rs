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
    // get_board is unfiltered, so an archived board comes back looking live
    // unless stamped with the marker's archived_at (mirrors kanban-cli/-mcp).
    let archived_at = ctx.board_archived_at(id).map_err(|e| AppError::from(&e))?;
    Ok(Json(BoardResponse::with_archived_at(&board, archived_at)))
}

pub fn read_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards", get(list_boards))
        .route("/v1/boards/{id}", get(get_board))
}
