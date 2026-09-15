use crate::client_ident::ClientIdent;
use crate::error::{AppError, AppJson};
use crate::state::{AppState, Session};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use kanban_core::ClientId;
use kanban_domain::Invalidation;
use kanban_service::api::{
    BatchArchiveRequest, BatchAssignSprintRequest, BatchFailure, BatchMoveRequest,
    BatchOperationResponse, BatchUpdateRequest, ChangeKind, EntityType,
};
use kanban_service::{BatchOperationResult, CardUpdate};
use uuid::Uuid;

fn to_wire(result: &BatchOperationResult) -> BatchOperationResponse {
    BatchOperationResponse::new(
        result.succeeded.clone(),
        result
            .failed
            .iter()
            .map(|f| BatchFailure::new(f.id, f.error.clone()))
            .collect(),
    )
}

async fn run_detailed(
    state: &AppState,
    client: ClientId,
    op: impl FnOnce(&mut Session) -> (BatchOperationResult, Invalidation),
) -> Result<BatchOperationResponse, AppError> {
    let mut ctx = state.lock_for_write(client).await;
    let (result, invalidation) = op(&mut ctx);
    ctx.save().await.map_err(|e| AppError::from(&e))?;
    for id in &result.succeeded {
        state.broadcast_change(
            ctx.issued_by(),
            EntityType::Card,
            *id,
            ChangeKind::Updated,
            &invalidation,
        );
    }
    Ok(to_wire(&result))
}

async fn batch_archive_route(
    State(state): State<AppState>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<BatchArchiveRequest>,
) -> Result<Json<BatchOperationResponse>, AppError> {
    Ok(Json(
        run_detailed(&state, client, |c| c.archive_cards_detailed(req.ids)).await?,
    ))
}

async fn batch_move_route(
    State(state): State<AppState>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<BatchMoveRequest>,
) -> Result<Json<BatchOperationResponse>, AppError> {
    Ok(Json(
        run_detailed(&state, client, |c| {
            c.move_cards_detailed(req.ids, req.column_id)
        })
        .await?,
    ))
}

async fn batch_assign_sprint_route(
    State(state): State<AppState>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<BatchAssignSprintRequest>,
) -> Result<Json<BatchOperationResponse>, AppError> {
    Ok(Json(
        run_detailed(&state, client, |c| {
            c.assign_cards_to_sprint_detailed(req.ids, req.sprint_id)
        })
        .await?,
    ))
}

async fn batch_update_route(
    State(state): State<AppState>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<BatchUpdateRequest>,
) -> Result<Json<BatchOperationResponse>, AppError> {
    let ids: Vec<Uuid> = req.updates.iter().map(|i| i.id).collect();
    let pairs: Vec<(Uuid, CardUpdate)> = req
        .updates
        .into_iter()
        .map(|i| CardUpdate::try_from(i.update).map(|u| (i.id, u)))
        .collect::<Result<_, _>>()
        .map_err(|e| AppError::from(&e))?;

    let mut ctx = state.lock_for_write(client).await;
    let (_count, invalidation) = crate::state::mutate(&mut ctx, |c| c.update_cards_impl(pairs))
        .map_err(|e| AppError::from(&e))?;
    ctx.save().await.map_err(|e| AppError::from(&e))?;
    for id in &ids {
        state.broadcast_change(
            ctx.issued_by(),
            EntityType::Card,
            *id,
            ChangeKind::Updated,
            &invalidation,
        );
    }
    Ok(Json(BatchOperationResponse::new(ids, vec![])))
}

/// Invalid ids are rejected individually; the remaining ids are applied in a
/// single transaction, so on any execution error no card is modified and
/// every id is reported failed.
pub fn write_router() -> Router<AppState> {
    Router::new()
        .route("/v1/cards/batch/archive", post(batch_archive_route))
        .route("/v1/cards/batch/move", post(batch_move_route))
        .route(
            "/v1/cards/batch/assign-sprint",
            post(batch_assign_sprint_route),
        )
        .route("/v1/cards/batch/update", post(batch_update_route))
}
