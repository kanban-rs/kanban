use crate::client_ident::ClientIdent;
use crate::error::{AppError, AppJson};
use crate::etag;
use crate::handlers::boards::{create_board, create_or_replace_board};
use crate::model_read::{require_loaded, require_loaded_entity};
use crate::pagination::paginate_response;
use crate::scope::RouteScope;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use kanban_domain::{LoadState, Model, NoProjections};
use kanban_service::api::{
    ArchivedBoardResponse, BoardResponse, ChangeKind, CreateBoardRequest, DeleteResponse,
    EntityType, MutationResponse, Page, PageParams, ReplaceBoardRequest, UpdateBoardRequest,
};
use kanban_service::KanbanError;
use uuid::Uuid;

async fn list_boards(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
) -> Result<Json<Page<BoardResponse>>, AppError> {
    let guard = state.lock_session().await;
    let mut model = Model::default();
    guard.sync(&RouteScope::BoardList, &mut model, &mut NoProjections);
    let boards = require_loaded(model.live_boards_state(), "board list")?;
    paginate_response(
        boards.iter().map(|b| BoardResponse::from(*b)).collect(),
        &params,
    )
}

fn board_current(
    session: &crate::state::Session,
    id: Uuid,
) -> Result<Option<BoardResponse>, AppError> {
    let mut model = Model::default();
    session.sync(&RouteScope::Board(id), &mut model, &mut NoProjections);
    let board = match model.board_id_status(id) {
        LoadState::Missing => return Ok(None),
        status => require_loaded_entity(status, "Board", id)?,
    };
    let markers = require_loaded(model.archived_boards_state(), "archived board list")?;
    let archived_at = markers
        .iter()
        .find(|marker| marker.entity_id == id)
        .map(|marker| marker.metadata.archived_at);
    Ok(Some(BoardResponse::with_archived_at(board, archived_at)))
}

fn board_response(session: &crate::state::Session, id: Uuid) -> Result<BoardResponse, AppError> {
    board_current(session, id)?.ok_or_else(|| AppError::from(&KanbanError::not_found("Board", id)))
}

async fn get_board(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let guard = state.lock_session().await;
    etag::json_with_etag(&headers, &board_response(&guard, id)?)
}

async fn list_archived_boards(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
) -> Result<Json<Page<ArchivedBoardResponse>>, AppError> {
    let guard = state.lock_session().await;
    let mut model = Model::default();
    guard.sync(
        &RouteScope::ArchivedBoardList,
        &mut model,
        &mut NoProjections,
    );
    let markers = require_loaded(model.archived_boards_state(), "archived board list")?;
    paginate_response(
        markers.iter().map(ArchivedBoardResponse::from).collect(),
        &params,
    )
}

pub fn read_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards", get(list_boards))
        .route("/v1/boards/{id}", get(get_board))
        .route("/v1/archived-boards", get(list_archived_boards))
}

async fn post_board(
    State(state): State<AppState>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<CreateBoardRequest>,
) -> Result<(StatusCode, Json<MutationResponse<BoardResponse>>), AppError> {
    let (resp, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        let (resp, invalidation) = create_board(&mut ctx, req).map_err(AppError::from)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Board,
                resp.id,
                ChangeKind::Created,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        (resp, invalidation)
    };
    Ok((
        StatusCode::CREATED,
        Json(MutationResponse::new(resp, &invalidation)),
    ))
}

async fn put_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
    AppJson(req): AppJson<ReplaceBoardRequest>,
) -> Result<(StatusCode, Json<BoardResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.lock_for_write(client).await;
        etag::check_if_match(&headers, || board_current(&ctx, id))?;
        let (resp, created, invalidation) =
            create_or_replace_board(&mut ctx, id, req).map_err(AppError::from)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Board,
                id,
                ChangeKind::created_or_updated(created),
                &invalidation,
            )
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
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
    AppJson(req): AppJson<UpdateBoardRequest>,
) -> Result<Json<MutationResponse<BoardResponse>>, AppError> {
    let (board, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        etag::check_if_match(&headers, || board_current(&ctx, id))?;
        let (board, invalidation) =
            crate::state::mutate(&mut ctx, |c| c.update_board_impl(id, req.into()))
                .map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Board,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        (board, invalidation)
    };
    Ok(Json(MutationResponse::new(
        BoardResponse::from(&board),
        &invalidation,
    )))
}

async fn delete_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<DeleteResponse>), AppError> {
    let invalidation = {
        let mut ctx = state.lock_for_write(client).await;
        etag::check_if_match(&headers, || board_current(&ctx, id))?;
        let invalidation = crate::state::mutate_unit(&mut ctx, |c| c.delete_board_impl(id))
            .map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Board,
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

async fn archive_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
) -> Result<Json<BoardResponse>, AppError> {
    let mut guard = state.lock_for_write(client).await;
    let invalidation = crate::state::mutate_unit(&mut guard, |c| c.archive_board_impl(id))
        .map_err(|e| AppError::from(&e))?;
    let response = board_response(&guard, id)?;
    state
        .persist_and_broadcast(
            &guard,
            EntityType::Board,
            id,
            ChangeKind::Updated,
            &invalidation,
        )
        .await
        .map_err(|e| AppError::from(&e))?;
    Ok(Json(response))
}

async fn restore_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
) -> Result<Json<BoardResponse>, AppError> {
    let mut guard = state.lock_for_write(client).await;
    let invalidation = crate::state::mutate_unit(&mut guard, |c| c.restore_board_impl(id))
        .map_err(|e| AppError::from(&e))?;
    let response = board_response(&guard, id)?;
    state
        .persist_and_broadcast(
            &guard,
            EntityType::Board,
            id,
            ChangeKind::Updated,
            &invalidation,
        )
        .await
        .map_err(|e| AppError::from(&e))?;
    Ok(Json(response))
}

pub fn write_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards", post(post_board))
        .route(
            "/v1/boards/{id}",
            put(put_board).patch(patch_board).delete(delete_board),
        )
        .route("/v1/boards/{id}/archive", post(archive_board))
        .route("/v1/boards/{id}/restore", post(restore_board))
}
