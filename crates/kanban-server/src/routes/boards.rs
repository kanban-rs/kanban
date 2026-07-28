use crate::error::{AppError, AppJson};
use crate::handlers::boards::{create_board, create_or_replace_board};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use kanban_service::api::{
    BoardResponse, CreateBoardRequest, ReplaceBoardRequest, UpdateBoardRequest,
};
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

async fn post_board(
    State(state): State<AppState>,
    AppJson(req): AppJson<CreateBoardRequest>,
) -> Result<(StatusCode, Json<BoardResponse>), AppError> {
    let resp = {
        let mut ctx = state.ctx.lock().await;
        let resp = create_board(&mut ctx, req).map_err(AppError::from)?;
        state
            .persist_and_broadcast(&ctx)
            .await
            .map_err(|e| AppError::from(&e))?;
        resp
    };
    Ok((StatusCode::CREATED, Json(resp)))
}

async fn put_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    AppJson(req): AppJson<ReplaceBoardRequest>,
) -> Result<(StatusCode, Json<BoardResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.ctx.lock().await;
        let (resp, created) = create_or_replace_board(&mut ctx, id, req).map_err(AppError::from)?;
        state
            .persist_and_broadcast(&ctx)
            .await
            .map_err(|e| AppError::from(&e))?;
        (resp, created)
    };
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(resp)))
}

async fn patch_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    AppJson(req): AppJson<UpdateBoardRequest>,
) -> Result<Json<BoardResponse>, AppError> {
    let board = {
        let mut ctx = state.ctx.lock().await;
        let board = ctx
            .update_board(id, req.into())
            .map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(&ctx)
            .await
            .map_err(|e| AppError::from(&e))?;
        board
    };
    Ok(Json(BoardResponse::from(&board)))
}

async fn delete_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.ctx.lock().await;
        ctx.delete_board(id).map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(&ctx)
            .await
            .map_err(|e| AppError::from(&e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn write_router() -> Router<AppState> {
    Router::new().route("/v1/boards", post(post_board)).route(
        "/v1/boards/{id}",
        put(put_board).patch(patch_board).delete(delete_board),
    )
}
