use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use kanban_domain::{CardListFilter, CardSummary};
use kanban_service::api::ArchivedFilterDto;
use kanban_service::api::CardResponse;
use kanban_service::{KanbanError, KanbanOperations};
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
