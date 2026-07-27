use crate::error::{AppError, AppJson};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use kanban_service::api::ColumnResponse;
use kanban_service::{ColumnUpdate, KanbanError, KanbanOperations};
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

async fn create_column_route(
    State(state): State<AppState>,
    Path(board_id): Path<Uuid>,
    AppJson(req): AppJson<kanban_service::api::CreateColumnRequest>,
) -> Result<(StatusCode, Json<ColumnResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.ctx.lock().await;
        crate::handlers::columns::create_column(&mut ctx, board_id, req).map_err(AppError::from)?
    };
    state.broadcast_change();
    Ok((
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(resp),
    ))
}

async fn put_column_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    AppJson(req): AppJson<kanban_service::api::ReplaceColumnRequest>,
) -> Result<(StatusCode, Json<ColumnResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.ctx.lock().await;
        crate::handlers::columns::create_or_replace_column(&mut ctx, board_id, id, req)
            .map_err(AppError::from)?
    };
    state.broadcast_change();
    Ok((
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(resp),
    ))
}

async fn update_column_route(
    State(state): State<AppState>,
    Path((_board_id, id)): Path<(Uuid, Uuid)>,
    AppJson(req): AppJson<kanban_service::api::UpdateColumnRequest>,
) -> Result<Json<ColumnResponse>, AppError> {
    let updates = ColumnUpdate::try_from(req).map_err(|e| AppError::from(&e))?;
    let col = {
        let mut ctx = state.ctx.lock().await;
        ctx.update_column(id, updates)
            .map_err(|e| AppError::from(&e))?
    };
    state.broadcast_change();
    Ok(Json(ColumnResponse::from(&col)))
}

async fn delete_column_route(
    State(state): State<AppState>,
    Path((_board_id, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.ctx.lock().await;
        ctx.delete_column(id).map_err(|e| AppError::from(&e))?;
    }
    state.broadcast_change();
    Ok(StatusCode::NO_CONTENT)
}

async fn reorder_column_route(
    State(state): State<AppState>,
    Path((_board_id, id)): Path<(Uuid, Uuid)>,
    AppJson(req): AppJson<kanban_service::api::ReorderColumnRequest>,
) -> Result<Json<ColumnResponse>, AppError> {
    if req.position < 0 {
        return Err(AppError::from(&kanban_domain::KanbanError::validation(
            format!("column position must be >= 0, got {}", req.position),
        )));
    }
    let col = {
        let mut ctx = state.ctx.lock().await;
        ctx.reorder_column(id, req.position)
            .map_err(|e| AppError::from(&e))?
    };
    state.broadcast_change();
    Ok(Json(ColumnResponse::from(&col)))
}

pub fn write_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards/{board_id}/columns", post(create_column_route))
        .route(
            "/v1/boards/{board_id}/columns/{id}",
            put(put_column_route)
                .patch(update_column_route)
                .delete(delete_column_route),
        )
        .route(
            "/v1/boards/{board_id}/columns/{id}/reorder",
            post(reorder_column_route),
        )
}
