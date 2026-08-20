use crate::error::{AppError, AppJson};
use crate::pagination::paginate_response;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use kanban_domain::Sprint;
use kanban_service::api::{ChangeKind, EntityType, Page, PageParams, SprintResponse};
use kanban_service::{
    resolve_sprint_name, resolve_sprint_names, KanbanError, KanbanOperations, SprintUpdate,
};
use uuid::Uuid;

async fn list_sprints(
    State(state): State<AppState>,
    Path(board_id): Path<Uuid>,
    Query(params): Query<PageParams>,
) -> Result<Json<Page<SprintResponse>>, AppError> {
    let ctx = state.ctx.lock().await;
    let sprints = ctx.list_sprints(board_id).map_err(|e| AppError::from(&e))?;
    let names = resolve_sprint_names(&*ctx, board_id, &sprints).map_err(|e| AppError::from(&e))?;
    let responses: Vec<SprintResponse> = sprints
        .iter()
        .zip(names)
        .map(|(s, name)| SprintResponse::new(s, name))
        .collect();
    paginate_response(responses, &params)
}

async fn get_sprint(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SprintResponse>, AppError> {
    let ctx = state.ctx.lock().await;
    require_sprint_in_board(&ctx, board_id, id)?;
    let sprint = do_get_sprint(&ctx, id)?;
    Ok(Json(respond(&ctx, &sprint)?))
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

fn do_get_sprint(ctx: &kanban_service::KanbanContext, id: Uuid) -> Result<Sprint, AppError> {
    ctx.get_sprint(id)
        .map_err(|e| AppError::from(&e))?
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Sprint", id)))
}

fn do_update_sprint(
    ctx: &mut kanban_service::KanbanContext,
    id: Uuid,
    updates: SprintUpdate,
) -> Result<Sprint, AppError> {
    ctx.update_sprint(id, updates)
        .map_err(|e| AppError::from(&e))
}

fn do_delete_sprint(ctx: &mut kanban_service::KanbanContext, id: Uuid) -> Result<(), AppError> {
    ctx.delete_sprint(id).map_err(|e| AppError::from(&e))
}

fn require_sprint_in_board(
    ctx: &kanban_service::KanbanContext,
    board_id: Uuid,
    id: Uuid,
) -> Result<(), AppError> {
    ctx.get_sprint(id)
        .map_err(|e| AppError::from(&e))?
        .filter(|s| s.board_id == board_id)
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Sprint", id)))?;
    Ok(())
}

fn respond(
    ctx: &kanban_service::KanbanContext,
    sprint: &Sprint,
) -> Result<SprintResponse, AppError> {
    let name = resolve_sprint_name(ctx, sprint).map_err(|e| AppError::from(&e))?;
    Ok(SprintResponse::new(sprint, name))
}

async fn create_sprint_route(
    State(state): State<AppState>,
    Path(board_id): Path<Uuid>,
    AppJson(req): AppJson<kanban_service::api::CreateSprintRequest>,
) -> Result<(StatusCode, Json<SprintResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.ctx.lock().await;
        let result = crate::handlers::sprints::create_sprint(&mut ctx, board_id, req)
            .map_err(AppError::from)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Sprint,
                result.0.id,
                ChangeKind::created_or_updated(result.1),
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        result
    };
    Ok((created_status(created), Json(resp)))
}

async fn put_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    AppJson(req): AppJson<kanban_service::api::ReplaceSprintRequest>,
) -> Result<(StatusCode, Json<SprintResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.ctx.lock().await;
        let result =
            crate::handlers::sprints::create_or_replace_sprint(&mut ctx, board_id, id, req)
                .map_err(AppError::from)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Sprint,
                id,
                ChangeKind::created_or_updated(result.1),
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        result
    };
    Ok((created_status(created), Json(resp)))
}

async fn update_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    AppJson(req): AppJson<kanban_service::api::UpdateSprintRequest>,
) -> Result<Json<SprintResponse>, AppError> {
    let updates = SprintUpdate::from(req);
    let body = {
        let mut ctx = state.ctx.lock().await;
        require_sprint_in_board(&ctx, board_id, id)?;
        let sprint = do_update_sprint(&mut ctx, id, updates)?;
        let body = respond(&ctx, &sprint)?;
        state
            .persist_and_broadcast(&ctx, EntityType::Sprint, id, ChangeKind::Updated)
            .await
            .map_err(|e| AppError::from(&e))?;
        body
    };
    Ok(Json(body))
}

async fn delete_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.ctx.lock().await;
        require_sprint_in_board(&ctx, board_id, id)?;
        do_delete_sprint(&mut ctx, id)?;
        state
            .persist_and_broadcast(&ctx, EntityType::Sprint, id, ChangeKind::Deleted)
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
    Path(id): Path<Uuid>,
) -> Result<Json<SprintResponse>, AppError> {
    let ctx = state.ctx.lock().await;
    let sprint = do_get_sprint(&ctx, id)?;
    Ok(Json(respond(&ctx, &sprint)?))
}

async fn update_sprint_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    AppJson(req): AppJson<kanban_service::api::UpdateSprintRequest>,
) -> Result<Json<SprintResponse>, AppError> {
    let updates = SprintUpdate::from(req);
    let body = {
        let mut ctx = state.ctx.lock().await;
        do_get_sprint(&ctx, id)?;
        let sprint = do_update_sprint(&mut ctx, id, updates)?;
        let body = respond(&ctx, &sprint)?;
        state
            .persist_and_broadcast(&ctx, EntityType::Sprint, id, ChangeKind::Updated)
            .await
            .map_err(|e| AppError::from(&e))?;
        body
    };
    Ok(Json(body))
}

async fn delete_sprint_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.ctx.lock().await;
        do_get_sprint(&ctx, id)?;
        do_delete_sprint(&mut ctx, id)?;
        state
            .persist_and_broadcast(&ctx, EntityType::Sprint, id, ChangeKind::Deleted)
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
