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
use kanban_domain::{Invalidation, LoadState, Model, NoProjections, Sprint};
use kanban_service::api::{ChangeKind, EntityType, Page, PageParams, SprintResponse};
use kanban_service::{resolve_sprint_name, KanbanError, KanbanOperations, SprintUpdate};
use uuid::Uuid;

async fn list_sprints(
    State(state): State<AppState>,
    Path(board_id): Path<Uuid>,
    Query(params): Query<PageParams>,
) -> Result<Json<Page<SprintResponse>>, AppError> {
    let guard = state.lock_session().await;
    let mut model = Model::default();
    guard.sync(
        &RouteScope::BoardSprints(board_id),
        &mut model,
        &mut NoProjections,
    );
    let board = require_loaded_entity(model.board_id_status(board_id), "Board", board_id)?;
    let sprints = require_loaded(model.board_sprints_state(board_id), "sprints of board")?;
    let responses: Vec<SprintResponse> = sprints
        .iter()
        .map(|s| SprintResponse::new(s, s.get_name(board).map(str::to_string)))
        .collect();
    paginate_response(responses, &params)
}

async fn get_sprint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    let guard = state.lock_session().await;
    let mut model = Model::default();
    guard.sync(
        &RouteScope::Sprint {
            board_id: Some(board_id),
            sprint_id: id,
        },
        &mut model,
        &mut NoProjections,
    );
    let sprint = require_loaded_entity(model.sprint_by_id_state(id), "Sprint", id)?;
    if sprint.board_id != board_id {
        return Err(AppError::from(&KanbanError::not_found("Sprint", id)));
    }
    let board = require_loaded_entity(model.board_id_status(board_id), "Board", board_id)?;
    etag::json_with_etag(
        &headers,
        &SprintResponse::new(sprint, sprint.get_name(board).map(str::to_string)),
    )
}

pub fn read_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards/{board_id}/sprints", get(list_sprints))
        .route("/v1/boards/{board_id}/sprints/{id}", get(get_sprint))
}

fn created_status(created: bool) -> StatusCode {
    if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    }
}

fn do_get_sprint(ctx: &crate::state::Session, id: Uuid) -> Result<Sprint, AppError> {
    ctx.get_sprint(id)
        .map_err(|e| AppError::from(&e))?
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Sprint", id)))
}

fn do_update_sprint(
    ctx: &mut crate::state::Session,
    id: Uuid,
    updates: SprintUpdate,
) -> Result<(Sprint, Invalidation), AppError> {
    crate::state::mutate(ctx, |c| c.update_sprint_impl(id, updates)).map_err(|e| AppError::from(&e))
}

fn do_delete_sprint(ctx: &mut crate::state::Session, id: Uuid) -> Result<Invalidation, AppError> {
    crate::state::mutate_unit(ctx, |c| c.delete_sprint_impl(id)).map_err(|e| AppError::from(&e))
}

pub(crate) fn require_sprint_in_board(
    ctx: &crate::state::Session,
    board_id: Uuid,
    id: Uuid,
) -> Result<(), AppError> {
    ctx.get_sprint(id)
        .map_err(|e| AppError::from(&e))?
        .filter(|s| s.board_id == board_id)
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Sprint", id)))?;
    Ok(())
}

pub(crate) fn respond(
    ctx: &crate::state::Session,
    sprint: &Sprint,
) -> Result<SprintResponse, AppError> {
    let name = resolve_sprint_name(ctx, sprint).map_err(|e| AppError::from(&e))?;
    Ok(SprintResponse::new(sprint, name))
}

fn sprint_current(
    session: &crate::state::Session,
    id: Uuid,
) -> Result<Option<SprintResponse>, AppError> {
    let mut model = Model::default();
    session.sync(
        &RouteScope::Sprint {
            board_id: None,
            sprint_id: id,
        },
        &mut model,
        &mut NoProjections,
    );
    let sprint = match model.sprint_by_id_state(id) {
        LoadState::Missing => return Ok(None),
        status => require_loaded_entity(status, "Sprint", id)?.clone(),
    };
    session.sync(
        &RouteScope::Sprint {
            board_id: Some(sprint.board_id),
            sprint_id: id,
        },
        &mut model,
        &mut NoProjections,
    );
    let board = require_loaded_entity(
        model.board_id_status(sprint.board_id),
        "Board",
        sprint.board_id,
    )?;
    Ok(Some(SprintResponse::new(
        &sprint,
        sprint.get_name(board).map(str::to_string),
    )))
}

async fn create_sprint_route(
    State(state): State<AppState>,
    Path(board_id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<kanban_service::api::CreateSprintRequest>,
) -> Result<(StatusCode, Json<SprintResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.lock_for_write(client).await;
        let (resp, created, invalidation) =
            crate::handlers::sprints::create_sprint(&mut ctx, board_id, req)
                .map_err(AppError::from)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Sprint,
                resp.id,
                ChangeKind::created_or_updated(created),
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        (resp, created)
    };
    Ok((created_status(created), Json(resp)))
}

async fn put_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
    AppJson(req): AppJson<kanban_service::api::ReplaceSprintRequest>,
) -> Result<(StatusCode, Json<SprintResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.lock_for_write(client).await;
        etag::check_if_match(&headers, || sprint_current(&ctx, id))?;
        let (resp, created, invalidation) =
            crate::handlers::sprints::create_or_replace_sprint(&mut ctx, board_id, id, req)
                .map_err(AppError::from)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Sprint,
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

async fn update_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
    AppJson(req): AppJson<kanban_service::api::UpdateSprintRequest>,
) -> Result<Json<SprintResponse>, AppError> {
    let updates = SprintUpdate::from(req);
    let body = {
        let mut ctx = state.lock_for_write(client).await;
        require_sprint_in_board(&ctx, board_id, id)?;
        etag::check_if_match(&headers, || sprint_current(&ctx, id))?;
        let (sprint, invalidation) = do_update_sprint(&mut ctx, id, updates)?;
        let body = respond(&ctx, &sprint)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Sprint,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        body
    };
    Ok(Json(body))
}

async fn delete_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.lock_for_write(client).await;
        require_sprint_in_board(&ctx, board_id, id)?;
        etag::check_if_match(&headers, || sprint_current(&ctx, id))?;
        let invalidation = do_delete_sprint(&mut ctx, id)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Sprint,
                id,
                ChangeKind::Deleted,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn write_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards/{board_id}/sprints", post(create_sprint_route))
        .route(
            "/v1/boards/{board_id}/sprints/{id}",
            put(put_sprint_route)
                .patch(update_sprint_route)
                .delete(delete_sprint_route),
        )
}

async fn get_sprint_flat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let guard = state.lock_session().await;
    let mut model = Model::default();
    guard.sync(
        &RouteScope::Sprint {
            board_id: None,
            sprint_id: id,
        },
        &mut model,
        &mut NoProjections,
    );
    let sprint = require_loaded_entity(model.sprint_by_id_state(id), "Sprint", id)?.clone();
    guard.sync(
        &RouteScope::Sprint {
            board_id: Some(sprint.board_id),
            sprint_id: id,
        },
        &mut model,
        &mut NoProjections,
    );
    let board = require_loaded_entity(
        model.board_id_status(sprint.board_id),
        "Board",
        sprint.board_id,
    )?;
    etag::json_with_etag(
        &headers,
        &SprintResponse::new(&sprint, sprint.get_name(board).map(str::to_string)),
    )
}

async fn update_sprint_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
    AppJson(req): AppJson<kanban_service::api::UpdateSprintRequest>,
) -> Result<Json<SprintResponse>, AppError> {
    let updates = SprintUpdate::from(req);
    let body = {
        let mut ctx = state.lock_for_write(client).await;
        do_get_sprint(&ctx, id)?;
        etag::check_if_match(&headers, || sprint_current(&ctx, id))?;
        let (sprint, invalidation) = do_update_sprint(&mut ctx, id, updates)?;
        let body = respond(&ctx, &sprint)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Sprint,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        body
    };
    Ok(Json(body))
}

async fn delete_sprint_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.lock_for_write(client).await;
        do_get_sprint(&ctx, id)?;
        etag::check_if_match(&headers, || sprint_current(&ctx, id))?;
        let invalidation = do_delete_sprint(&mut ctx, id)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Sprint,
                id,
                ChangeKind::Deleted,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn flat_read_router() -> Router<AppState> {
    Router::new().route("/v1/sprints/{id}", get(get_sprint_flat))
}

pub fn flat_write_router() -> Router<AppState> {
    Router::new().route(
        "/v1/sprints/{id}",
        patch(update_sprint_route_flat).delete(delete_sprint_route_flat),
    )
}
