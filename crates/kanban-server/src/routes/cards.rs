use crate::error::{AppError, AppJson};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use kanban_domain::{CardListFilter, CardSummary};
use kanban_service::api::ArchivedFilterDto;
use kanban_service::api::CardResponse;
use kanban_service::api::{CreateCardRequest, UpdateCardRequest};
use kanban_service::{CardUpdate, KanbanError, KanbanOperations};
use serde::Deserialize;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CardQuery {
    pub column_id: Option<Uuid>,
    pub sprint_id: Option<Uuid>,
    #[serde(default)]
    pub archived: ArchivedFilterDto,
}

async fn list_cards(
    State(state): State<AppState>,
    Path(board_id): Path<Uuid>,
    Query(q): Query<CardQuery>,
) -> Result<Json<Vec<CardSummary>>, AppError> {
    let filter = CardListFilter {
        board_id: Some(board_id),
        column_id: q.column_id,
        sprint_ids: q.sprint_id.map(|id| HashSet::from([id])),
        archived: q.archived.into(),
        ..Default::default()
    };
    let ctx = state.ctx.lock().await;
    let cards = ctx.list_cards(filter).map_err(|e| AppError::from(&e))?;
    Ok(Json(cards))
}

async fn get_card(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CardResponse>, AppError> {
    let ctx = state.ctx.lock().await;
    let card = ctx
        .get_card(id)
        .map_err(|e| AppError::from(&e))?
        .filter(|c| c.board_id == board_id)
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Card", id)))?;
    Ok(Json(CardResponse::from(&card)))
}

pub fn read_router() -> Router<AppState> {
    Router::new()
        .route("/v1/boards/{board_id}/cards", get(list_cards))
        .route("/v1/boards/{board_id}/cards/{id}", get(get_card))
}

fn created_status(created: bool) -> StatusCode {
    if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    }
}

/// Fetch a card and 404 unless it belongs to `board_id` — the same
/// cross-board guard `get_card` (read route, above) applies, needed here
/// too since `KanbanOperations::{update_card, delete_card}` key on the
/// global card id alone with no board scoping of their own.
fn require_card_in_board(
    ctx: &kanban_service::KanbanContext,
    board_id: Uuid,
    id: Uuid,
) -> Result<(), AppError> {
    ctx.get_card(id)
        .map_err(|e| AppError::from(&e))?
        .filter(|c| c.board_id == board_id)
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Card", id)))?;
    Ok(())
}

async fn create_card_route(
    State(state): State<AppState>,
    Path(column_id): Path<Uuid>,
    AppJson(req): AppJson<CreateCardRequest>,
) -> Result<(StatusCode, Json<CardResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.ctx.lock().await;
        let result = crate::handlers::cards::create_card(&mut ctx, column_id, req)
            .map_err(AppError::from)?;
        state
            .persist_and_broadcast(&ctx)
            .await
            .map_err(|e| AppError::from(&e))?;
        result
    };
    Ok((created_status(created), Json(resp)))
}

async fn put_card_route(
    State(state): State<AppState>,
    Path((column_id, id)): Path<(Uuid, Uuid)>,
    AppJson(req): AppJson<CreateCardRequest>,
) -> Result<(StatusCode, Json<CardResponse>), AppError> {
    let (resp, created) = {
        let mut ctx = state.ctx.lock().await;
        let result = crate::handlers::cards::create_or_replace_card(&mut ctx, column_id, id, req)
            .map_err(AppError::from)?;
        state
            .persist_and_broadcast(&ctx)
            .await
            .map_err(|e| AppError::from(&e))?;
        result
    };
    Ok((created_status(created), Json(resp)))
}

async fn update_card_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
    AppJson(req): AppJson<UpdateCardRequest>,
) -> Result<Json<CardResponse>, AppError> {
    let updates = CardUpdate::try_from(req).map_err(|e| AppError::from(&e))?;
    let card = {
        let mut ctx = state.ctx.lock().await;
        require_card_in_board(&ctx, board_id, id)?;
        let card = ctx
            .update_card(id, updates)
            .map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(&ctx)
            .await
            .map_err(|e| AppError::from(&e))?;
        card
    };
    Ok(Json(CardResponse::from(&card)))
}

async fn delete_card_route(
    State(state): State<AppState>,
    Path((board_id, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.ctx.lock().await;
        require_card_in_board(&ctx, board_id, id)?;
        ctx.delete_card(id).map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(&ctx)
            .await
            .map_err(|e| AppError::from(&e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn write_router() -> Router<AppState> {
    Router::new()
        .route("/v1/columns/{column_id}/cards", post(create_card_route))
        .route("/v1/columns/{column_id}/cards/{id}", put(put_card_route))
        .route(
            "/v1/boards/{board_id}/cards/{id}",
            patch(update_card_route).delete(delete_card_route),
        )
}

async fn get_card_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CardResponse>, AppError> {
    let ctx = state.ctx.lock().await;
    let card = ctx
        .get_card(id)
        .map_err(|e| AppError::from(&e))?
        .ok_or_else(|| AppError::from(&KanbanError::not_found("Card", id)))?;
    Ok(Json(CardResponse::from(&card)))
}

async fn update_card_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    AppJson(req): AppJson<UpdateCardRequest>,
) -> Result<Json<CardResponse>, AppError> {
    let updates = CardUpdate::try_from(req).map_err(|e| AppError::from(&e))?;
    let card = {
        let mut ctx = state.ctx.lock().await;
        let card = ctx
            .update_card(id, updates)
            .map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(&ctx)
            .await
            .map_err(|e| AppError::from(&e))?;
        card
    };
    Ok(Json(CardResponse::from(&card)))
}

async fn delete_card_route_flat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.ctx.lock().await;
        ctx.delete_card(id).map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(&ctx)
            .await
            .map_err(|e| AppError::from(&e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn flat_read_router() -> Router<AppState> {
    Router::new().route("/v1/cards/{id}", get(get_card_flat))
}

pub fn flat_write_router() -> Router<AppState> {
    Router::new().route(
        "/v1/cards/{id}",
        patch(update_card_route_flat).delete(delete_card_route_flat),
    )
}
