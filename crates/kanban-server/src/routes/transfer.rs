use crate::client_ident::ClientIdent;
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use kanban_domain::{KanbanError, KanbanOperations, Snapshot};
use kanban_service::api::{ApiError, BoardResponse, ChangeKind, EntityType, ErrorCode};
use uuid::Uuid;

async fn export_board_route(
    State(state): State<AppState>,
    Path(board_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let guard = state.lock_session().await;
    guard
        .get_board(board_id)
        .map_err(|e| AppError::from(&e))?
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Board", board_id)))?;
    let json = guard
        .export_board(Some(board_id))
        .map_err(|e| AppError::from(&e))?;
    Ok(([(header::CONTENT_TYPE, "application/json")], json))
}

async fn export_all_route(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let guard = state.lock_session().await;
    let json = guard.export_board(None).map_err(|e| AppError::from(&e))?;
    Ok(([(header::CONTENT_TYPE, "application/json")], json))
}

pub fn read_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards/{board_id}/export", get(export_board_route))
        .route("/v1/export", get(export_all_route))
}

async fn import_route(
    State(state): State<AppState>,
    ClientIdent(client): ClientIdent,
    body: String,
) -> Result<(StatusCode, Json<BoardResponse>), AppError> {
    serde_json::from_str::<Snapshot>(&body).map_err(|e| {
        AppError(ApiError::new(
            ErrorCode::ValidationFailed,
            format!("request body is not a valid snapshot: {e}"),
        ))
    })?;

    let mut guard = state.lock_for_write(client).await;
    let (board, invalidation) = crate::state::mutate(&mut guard, |c| c.import_board_impl(&body))
        .map_err(|e| AppError::from(&e))?;
    state
        .persist_and_broadcast(
            &guard,
            EntityType::Board,
            board.id,
            ChangeKind::Created,
            &invalidation,
        )
        .await
        .map_err(|e| AppError::from(&e))?;
    Ok((StatusCode::CREATED, Json(BoardResponse::from(&board))))
}

pub fn write_router() -> Router<AppState> {
    Router::new().route("/v1/import", post(import_route))
}
