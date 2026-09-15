use crate::client_ident::ClientIdent;
use crate::error::{AppError, AppJson};
use crate::etag;
use crate::model_read::{require_loaded, require_loaded_entity};
use crate::pagination::paginate_response;
use crate::scope::RouteScope;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use kanban_domain::{Column, Invalidation, LoadState, Model, NoProjections};
use kanban_service::api::{
    ChangeKind, ColumnResponse, DeleteResponse, EntityType, MutationResponse, Page, PageParams,
};
use kanban_service::{ColumnUpdate, KanbanError, KanbanOperations};
use uuid::Uuid;

async fn list_columns(
    State(state): State<AppState>,
    Path(board_id): Path<Uuid>,
    Query(params): Query<PageParams>,
) -> Result<Json<Page<ColumnResponse>>, AppError> {
    let session = state.lock_session().await;
    let scope = RouteScope::BoardColumns(board_id);
    let mut model = Model::default();
    session.sync(&scope, &mut model, &mut NoProjections);
    require_loaded_entity(model.board_id_status(board_id), "Board", board_id)?;
    let cols = require_loaded(model.board_columns_state(board_id), "columns")?;
    paginate_response(cols.iter().map(ColumnResponse::from).collect(), &params)
}

fn column_current(
    session: &crate::state::Session,
    id: Uuid,
) -> Result<Option<ColumnResponse>, AppError> {
    let scope = RouteScope::Column(id);
    let mut model = Model::default();
    session.sync(&scope, &mut model, &mut NoProjections);
    let column = match model.column_id_status(id) {
        LoadState::Missing => return Ok(None),
        status => require_loaded_entity(status, "Column", id)?,
    };
    Ok(Some(ColumnResponse::from(column)))
}

async fn get_column(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    let session = state.lock_session().await;
    let scope = RouteScope::Column(id);
    let mut model = Model::default();
    session.sync(&scope, &mut model, &mut NoProjections);
    let column = require_loaded_entity(model.column_id_status(id), "Column", id)?;
    if column.board_id != board_id {
        return Err(AppError::from(&KanbanError::not_found("Column", id)));
    }
    etag::json_with_etag(&headers, &ColumnResponse::from(column))
}

pub fn read_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards/{board_id}/columns", get(list_columns))
        .route("/v1/boards/{board_id}/columns/{id}", get(get_column))
}

fn created_status(created: bool) -> StatusCode {
    if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    }
}

fn do_update_column(
    ctx: &mut crate::state::Session,
    id: Uuid,
    updates: ColumnUpdate,
) -> Result<(Column, Invalidation), AppError> {
    crate::state::mutate(ctx, |c| c.update_column_impl(id, updates)).map_err(|e| AppError::from(&e))
}

fn do_delete_column(ctx: &mut crate::state::Session, id: Uuid) -> Result<Invalidation, AppError> {
    crate::state::mutate_unit(ctx, |c| c.delete_column_impl(id)).map_err(|e| AppError::from(&e))
}

/// Fetch a column and 404 unless it belongs to `board_id`, needed because
/// `KanbanOperations::{update_column, delete_column, reorder_column}` key on
/// the global column id with no board scoping of their own.
fn require_column_in_board(
    ctx: &crate::state::Session,
    board_id: Uuid,
    id: Uuid,
) -> Result<(), AppError> {
    ctx.get_column(id)
        .map_err(|e| AppError::from(&e))?
        .filter(|c| c.board_id == board_id)
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Column", id)))?;
    Ok(())
}

async fn create_column_route(
    State(state): State<AppState>,
    Path(board_id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<kanban_service::api::CreateColumnRequest>,
) -> Result<(StatusCode, Json<MutationResponse<ColumnResponse>>), AppError> {
    let (resp, created, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        let (resp, created, invalidation) =
            crate::handlers::columns::create_column(&mut ctx, board_id, req)
                .map_err(AppError::from)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Column,
                resp.id,
                ChangeKind::created_or_updated(created),
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        (resp, created, invalidation)
    };
    Ok((
        created_status(created),
        Json(MutationResponse::new(resp, &invalidation)),
    ))
}

async fn put_column_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
    AppJson(req): AppJson<kanban_service::api::ReplaceColumnRequest>,
) -> Result<(StatusCode, Json<ColumnResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.lock_for_write(client).await;
        etag::check_if_match(&headers, || column_current(&ctx, id))?;
        let (resp, created, invalidation) =
            crate::handlers::columns::create_or_replace_column(&mut ctx, board_id, id, req)
                .map_err(AppError::from)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Column,
                id,
                ChangeKind::created_or_updated(created),
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        (resp, created)
    };
    Ok((created_status(created), Json(resp)))
}

async fn update_column_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
    AppJson(req): AppJson<kanban_service::api::UpdateColumnRequest>,
) -> Result<Json<ColumnResponse>, AppError> {
    let updates = ColumnUpdate::try_from(req).map_err(|e| AppError::from(&e))?;
    let col = {
        let mut ctx = state.lock_for_write(client).await;
        require_column_in_board(&ctx, board_id, id)?;
        etag::check_if_match(&headers, || column_current(&ctx, id))?;
        let (col, invalidation) = do_update_column(&mut ctx, id, updates)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Column,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        col
    };
    Ok(Json(ColumnResponse::from(&col)))
}

async fn delete_column_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.lock_for_write(client).await;
        require_column_in_board(&ctx, board_id, id)?;
        etag::check_if_match(&headers, || column_current(&ctx, id))?;
        let invalidation = do_delete_column(&mut ctx, id)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Column,
                id,
                ChangeKind::Deleted,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn reorder_column_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<kanban_service::api::ReorderColumnRequest>,
) -> Result<Json<ColumnResponse>, AppError> {
    let position = req.validated_position().map_err(|e| AppError::from(&e))?;
    let col = {
        let mut ctx = state.lock_for_write(client).await;
        require_column_in_board(&ctx, board_id, id)?;
        let (col, invalidation) =
            crate::state::mutate(&mut ctx, |c| c.reorder_column_impl(id, position))
                .map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Column,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        col
    };
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

async fn get_column_flat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let session = state.lock_session().await;
    let scope = RouteScope::Column(id);
    let mut model = Model::default();
    session.sync(&scope, &mut model, &mut NoProjections);
    let column = require_loaded_entity(model.column_id_status(id), "Column", id)?;
    etag::json_with_etag(&headers, &ColumnResponse::from(column))
}

async fn update_column_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
    AppJson(req): AppJson<kanban_service::api::UpdateColumnRequest>,
) -> Result<Json<MutationResponse<ColumnResponse>>, AppError> {
    let updates = ColumnUpdate::try_from(req).map_err(|e| AppError::from(&e))?;
    let (col, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        etag::check_if_match(&headers, || column_current(&ctx, id))?;
        let (col, invalidation) = do_update_column(&mut ctx, id, updates)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Column,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        (col, invalidation)
    };
    Ok(Json(MutationResponse::new(
        ColumnResponse::from(&col),
        &invalidation,
    )))
}

async fn delete_column_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<DeleteResponse>), AppError> {
    let invalidation = {
        let mut ctx = state.lock_for_write(client).await;
        etag::check_if_match(&headers, || column_current(&ctx, id))?;
        let invalidation = do_delete_column(&mut ctx, id)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Column,
                id,
                ChangeKind::Deleted,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        invalidation
    };
    Ok((StatusCode::OK, Json(DeleteResponse::new(&invalidation))))
}

pub fn flat_read_router() -> Router<AppState> {
    Router::new().route("/v1/columns/{id}", get(get_column_flat))
}

pub fn flat_write_router() -> Router<AppState> {
    Router::new().route(
        "/v1/columns/{id}",
        patch(update_column_route_flat).delete(delete_column_route_flat),
    )
}
