use crate::client_ident::ClientIdent;
use crate::error::{AppError, AppJson};
use crate::routes::sprints::{require_sprint_in_board, respond};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};
use kanban_service::api::{ActivateSprintRequest, CarryOverRequest, CarryOverResponse};
use kanban_service::api::{ChangeKind, EntityType, SprintResponse};
use uuid::Uuid;

async fn activate_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<ActivateSprintRequest>,
) -> Result<Json<SprintResponse>, AppError> {
    let duration_days = req
        .validated_duration_days()
        .map_err(|e| AppError::from(&e))?;
    let body = {
        let mut ctx = state.lock_for_write(client).await;
        require_sprint_in_board(&ctx, board_id, id)?;
        let (sprint, invalidation) =
            crate::state::mutate(&mut ctx, |c| c.activate_sprint_impl(id, duration_days))
                .map_err(|e| AppError::from(&e))?;
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

async fn complete_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
) -> Result<Json<SprintResponse>, AppError> {
    let body = {
        let mut ctx = state.lock_for_write(client).await;
        require_sprint_in_board(&ctx, board_id, id)?;
        let (sprint, invalidation) = crate::state::mutate(&mut ctx, |c| c.complete_sprint_impl(id))
            .map_err(|e| AppError::from(&e))?;
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

async fn cancel_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
) -> Result<Json<SprintResponse>, AppError> {
    let body = {
        let mut ctx = state.lock_for_write(client).await;
        require_sprint_in_board(&ctx, board_id, id)?;
        let (sprint, invalidation) = crate::state::mutate(&mut ctx, |c| c.cancel_sprint_impl(id))
            .map_err(|e| AppError::from(&e))?;
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

async fn carry_over_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<CarryOverRequest>,
) -> Result<Json<CarryOverResponse>, AppError> {
    let moved = {
        let mut ctx = state.lock_for_write(client).await;
        require_sprint_in_board(&ctx, board_id, id)?;
        require_sprint_in_board(&ctx, board_id, req.to_sprint_id)?;
        let (moved, invalidation) = crate::state::mutate(&mut ctx, |c| {
            c.carry_over_sprint_cards_impl(id, req.to_sprint_id)
        })
        .map_err(|e| AppError::from(&e))?;
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
        moved
    };
    Ok(Json(CarryOverResponse { moved }))
}

pub fn write_router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/boards/{board_id}/sprints/{id}/activate",
            post(activate_sprint_route),
        )
        .route(
            "/v1/boards/{board_id}/sprints/{id}/complete",
            post(complete_sprint_route),
        )
        .route(
            "/v1/boards/{board_id}/sprints/{id}/cancel",
            post(cancel_sprint_route),
        )
        .route(
            "/v1/boards/{board_id}/sprints/{id}/carry-over",
            post(carry_over_sprint_route),
        )
}
