//! HTTP routes for the global label registry.
//!
//! Labels are identified by name (no UUID), so the name travels in the path:
//!
//! - `GET    /v1/labels`              list all labels
//! - `PUT    /v1/labels/{name}`       upsert a label (201 created / 200 updated)
//! - `DELETE /v1/labels/{name}`       delete a label by name
//! - `POST   /v1/labels/{name}/cards` assign the label to a batch of cards
//! - `DELETE /v1/labels/{name}/cards` remove the label from a batch of cards
//!
//! Batch routes apply all-or-nothing: a single missing card id fails the
//! whole request, so `failed` is empty in every successful response.

use crate::client_ident::ClientIdent;
use crate::error::{AppError, AppJson};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use kanban_service::api::{
    BatchOperationResponse, ChangeKind, DeleteResponse, LabelCardsRequest, LabelResponse,
    MutationResponse, UpsertLabelRequest,
};
use kanban_service::KanbanOperations;

pub fn read_router() -> Router<AppState> {
    Router::new().route("/v1/labels", get(list_labels))
}

pub fn write_router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/labels/{name}",
            put(upsert_label_route).delete(delete_label_route),
        )
        .route(
            "/v1/labels/{name}/cards",
            post(assign_label_cards_route).delete(remove_label_cards_route),
        )
}

/// `GET /v1/labels` — every label in the global registry, keyed by name.
async fn list_labels(State(state): State<AppState>) -> Result<Json<Vec<LabelResponse>>, AppError> {
    let session = state.lock_session().await;
    let labels = session.list_labels().map_err(|e| AppError::from(&e))?;
    Ok(Json(labels.iter().map(LabelResponse::from).collect()))
}

/// `PUT /v1/labels/{name}` — create the label (201) or replace it in full
/// (200). The name travels in the path and the body replaces the metadata;
/// an omitted `color` or `description` is cleared.
async fn upsert_label_route(
    State(state): State<AppState>,
    Path(name): Path<String>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<UpsertLabelRequest>,
) -> Result<(StatusCode, Json<MutationResponse<LabelResponse>>), AppError> {
    let (resp, created, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        let label = req.into_label(name);
        let created = ctx
            .get_label(&label.name)
            .map_err(|e| AppError::from(&e))?
            .is_none();
        let (label, invalidation) = crate::state::mutate(&mut ctx, |c| c.upsert_label_impl(label))
            .map_err(|e| AppError::from(&e))?;
        state
            .persist_unscoped_change(&ctx, ChangeKind::created_or_updated(created), &invalidation)
            .await
            .map_err(|e| AppError::from(&e))?;
        (LabelResponse::from(&label), created, invalidation)
    };
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(MutationResponse::new(resp, &invalidation))))
}

/// `DELETE /v1/labels/{name}` — remove the label from the registry. Cards
/// that still carry it keep their labels; only the registry entry goes.
async fn delete_label_route(
    State(state): State<AppState>,
    Path(name): Path<String>,
    ClientIdent(client): ClientIdent,
) -> Result<(StatusCode, Json<DeleteResponse>), AppError> {
    let invalidation = {
        let mut ctx = state.lock_for_write(client).await;
        let invalidation = crate::state::mutate_unit(&mut ctx, |c| c.delete_label_impl(name))
            .map_err(|e| AppError::from(&e))?;
        state
            .persist_unscoped_change(&ctx, ChangeKind::Deleted, &invalidation)
            .await
            .map_err(|e| AppError::from(&e))?;
        invalidation
    };
    Ok((StatusCode::OK, Json(DeleteResponse::new(&invalidation))))
}

/// `POST /v1/labels/{name}/cards` — apply `name` to `card_ids`. The label is
/// auto-registered if (and only if) it is not already in the registry, so an
/// existing label's `color`/`description` is never touched.
async fn assign_label_cards_route(
    State(state): State<AppState>,
    Path(name): Path<String>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<LabelCardsRequest>,
) -> Result<Json<BatchOperationResponse>, AppError> {
    let (succeeded, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        let ids = req.card_ids.clone();
        let (_count, invalidation) = crate::state::mutate(&mut ctx, |c| {
            c.assign_label_to_cards_impl(name, req.card_ids)
        })
        .map_err(|e| AppError::from(&e))?;
        state
            .persist_unscoped_change(&ctx, ChangeKind::Updated, &invalidation)
            .await
            .map_err(|e| AppError::from(&e))?;
        (ids, invalidation)
    };
    Ok(Json(BatchOperationResponse::new(
        succeeded,
        Vec::new(),
        &invalidation,
    )))
}

/// `DELETE /v1/labels/{name}/cards` — remove `name` from `card_ids`. The
/// registry entry is left intact and a card that does not carry the label
/// comes back untouched.
async fn remove_label_cards_route(
    State(state): State<AppState>,
    Path(name): Path<String>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<LabelCardsRequest>,
) -> Result<Json<BatchOperationResponse>, AppError> {
    let (succeeded, invalidation) = {
        let mut ctx = state.lock_for_write(client).await;
        let ids = req.card_ids.clone();
        let (_count, invalidation) = crate::state::mutate(&mut ctx, |c| {
            c.remove_label_from_cards_impl(name, req.card_ids)
        })
        .map_err(|e| AppError::from(&e))?;
        state
            .persist_unscoped_change(&ctx, ChangeKind::Updated, &invalidation)
            .await
            .map_err(|e| AppError::from(&e))?;
        (ids, invalidation)
    };
    Ok(Json(BatchOperationResponse::new(
        succeeded,
        Vec::new(),
        &invalidation,
    )))
}
