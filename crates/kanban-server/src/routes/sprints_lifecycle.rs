use crate::client_ident::ClientIdent;
use crate::error::{AppError, AppJson};
use crate::routes::sprints::{require_sprint_in_board, respond};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};
use kanban_core::ClientId;
use kanban_domain::{Invalidation, MutationOperations, Sprint};
use kanban_service::api::{ActivateSprintRequest, CarryOverRequest, CarryOverResponse};
use kanban_service::api::{ChangeKind, EntityType, MutationResponse, SprintResponse};
use kanban_service::KanbanResult;
use uuid::Uuid;

async fn run_sprint_transition(
    state: &AppState,
    client: ClientId,
    board_id: Uuid,
    id: Uuid,
    op: impl FnOnce(&mut dyn MutationOperations) -> KanbanResult<(Sprint, Invalidation)>,
) -> Result<Json<MutationResponse<SprintResponse>>, AppError> {
    let (body, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        require_sprint_in_board(&ctx, board_id, id)?;
        let (sprint, invalidation) =
            crate::state::mutate(&mut ctx, op).map_err(|e| AppError::from(&e))?;
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
        (body, invalidation)
    };
    Ok(Json(MutationResponse::new(body, &invalidation)))
}

async fn run_carry_over(
    state: &AppState,
    client: ClientId,
    board_id: Uuid,
    id: Uuid,
    to_sprint_id: Uuid,
) -> Result<Json<MutationResponse<CarryOverResponse>>, AppError> {
    let (moved, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        require_sprint_in_board(&ctx, board_id, id)?;
        require_sprint_in_board(&ctx, board_id, to_sprint_id)?;
        let (moved, invalidation) = crate::state::mutate(&mut ctx, |c| {
            c.carry_over_sprint_cards_impl(id, to_sprint_id)
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
        (moved, invalidation)
    };
    Ok(Json(MutationResponse::new(
        CarryOverResponse { moved },
        &invalidation,
    )))
}

async fn activate_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<ActivateSprintRequest>,
) -> Result<Json<MutationResponse<SprintResponse>>, AppError> {
    let duration_days = req
        .validated_duration_days()
        .map_err(|e| AppError::from(&e))?;
    run_sprint_transition(&state, client, board_id, id, |c| {
        c.activate_sprint_impl(id, duration_days)
    })
    .await
}

async fn complete_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
) -> Result<Json<MutationResponse<SprintResponse>>, AppError> {
    run_sprint_transition(&state, client, board_id, id, |c| c.complete_sprint_impl(id)).await
}

async fn cancel_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
) -> Result<Json<MutationResponse<SprintResponse>>, AppError> {
    run_sprint_transition(&state, client, board_id, id, |c| c.cancel_sprint_impl(id)).await
}

async fn carry_over_sprint_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<CarryOverRequest>,
) -> Result<Json<MutationResponse<CarryOverResponse>>, AppError> {
    run_carry_over(&state, client, board_id, id, req.to_sprint_id).await
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

async fn run_sprint_transition_flat(
    state: &AppState,
    client: ClientId,
    id: Uuid,
    op: impl FnOnce(&mut dyn MutationOperations) -> KanbanResult<(Sprint, Invalidation)>,
) -> Result<Json<MutationResponse<SprintResponse>>, AppError> {
    let (body, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        let (sprint, invalidation) =
            crate::state::mutate(&mut ctx, op).map_err(|e| AppError::from(&e))?;
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
        (body, invalidation)
    };
    Ok(Json(MutationResponse::new(body, &invalidation)))
}

async fn run_carry_over_flat(
    state: &AppState,
    client: ClientId,
    id: Uuid,
    to_sprint_id: Uuid,
) -> Result<Json<MutationResponse<CarryOverResponse>>, AppError> {
    let (moved, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        let (moved, invalidation) = crate::state::mutate(&mut ctx, |c| {
            c.carry_over_sprint_cards_impl(id, to_sprint_id)
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
        (moved, invalidation)
    };
    Ok(Json(MutationResponse::new(
        CarryOverResponse { moved },
        &invalidation,
    )))
}

async fn activate_sprint_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<ActivateSprintRequest>,
) -> Result<Json<MutationResponse<SprintResponse>>, AppError> {
    let duration_days = req
        .validated_duration_days()
        .map_err(|e| AppError::from(&e))?;
    run_sprint_transition_flat(&state, client, id, |c| {
        c.activate_sprint_impl(id, duration_days)
    })
    .await
}

async fn complete_sprint_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
) -> Result<Json<MutationResponse<SprintResponse>>, AppError> {
    run_sprint_transition_flat(&state, client, id, |c| c.complete_sprint_impl(id)).await
}

async fn cancel_sprint_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
) -> Result<Json<MutationResponse<SprintResponse>>, AppError> {
    run_sprint_transition_flat(&state, client, id, |c| c.cancel_sprint_impl(id)).await
}

async fn carry_over_sprint_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<CarryOverRequest>,
) -> Result<Json<MutationResponse<CarryOverResponse>>, AppError> {
    run_carry_over_flat(&state, client, id, req.to_sprint_id).await
}

pub fn flat_write_router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/sprints/{id}/activate",
            post(activate_sprint_route_flat),
        )
        .route(
            "/v1/sprints/{id}/complete",
            post(complete_sprint_route_flat),
        )
        .route("/v1/sprints/{id}/cancel", post(cancel_sprint_route_flat))
        .route(
            "/v1/sprints/{id}/carry-over",
            post(carry_over_sprint_route_flat),
        )
}
