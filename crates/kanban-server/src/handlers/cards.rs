//! Typed card-create seam for the HTTP server.
//!
//! This is the funnel `POST /v1/columns/:column_id/cards` +
//! `PUT /v1/columns/:column_id/cards/:id` bind to. The shared
//! [`CreateCardRequest`] is split via `into_new_card(column_id)` (the column
//! FK is path-supplied on the nested route; the owning board is derived from
//! the column server-side), runs the idempotent PUT-create
//! (`create_or_replace_card`), and the resulting domain `Card` is projected
//! onto the wire [`CardResponse`]. The `created` flag lets the HTTP layer
//! answer 201 (created) vs 200 (replaced).

use kanban_service::api::{ApiError, CardResponse, CreateCardRequest};
use kanban_service::{KanbanContext, KanbanError, KanbanOperations};
use uuid::Uuid;

/// `POST /v1/columns/:column_id/cards`: append-create a card under the
/// path-supplied column. The body id is honoured when present (idempotent), else
/// minted. Returns the wire projection plus whether the card was created
/// (`true`) or replaced an existing id (`false`). If the body id already
/// exists under a *different* column, this 404s rather than silently
/// relocating it — see [`create_or_replace_card`].
pub fn create_card(
    ctx: &mut KanbanContext,
    column_id: Uuid,
    req: CreateCardRequest,
) -> Result<(CardResponse, bool), ApiError> {
    let (maybe_id, spec) = req
        .into_new_card(column_id)
        .map_err(|e| ApiError::from(&e))?;
    let id = maybe_id.unwrap_or_else(Uuid::new_v4);
    require_card_in_column_if_present(ctx, id, column_id)?;
    let outcome = ctx
        .create_or_replace_card(id, spec)
        .map_err(|e| ApiError::from(&e))?;
    Ok((CardResponse::from(&outcome.card), outcome.created))
}

/// `PUT /v1/columns/:column_id/cards/:id`: idempotent create-or-replace for a
/// card keyed on the path `id`. The column FK is path-supplied; the
/// server-managed `card_number`/`position` stay stable across the replace arm.
/// Returns the wire projection plus whether the card was created (`true`) or
/// replaced (`false`). If `id` already exists under a *different* column,
/// this 404s rather than silently relocating it — mirrors the column route's
/// cross-board guard (`handlers::columns::create_or_replace_column`) since
/// `create_or_replace_card`'s replace arm never checks the existing card's
/// column on its own.
pub fn create_or_replace_card(
    ctx: &mut KanbanContext,
    column_id: Uuid,
    id: Uuid,
    req: CreateCardRequest,
) -> Result<(CardResponse, bool), ApiError> {
    require_card_in_column_if_present(ctx, id, column_id)?;
    let (_body_id, spec) = req
        .into_new_card(column_id)
        .map_err(|e| ApiError::from(&e))?;
    let outcome = ctx
        .create_or_replace_card(id, spec)
        .map_err(|e| ApiError::from(&e))?;
    Ok((CardResponse::from(&outcome.card), outcome.created))
}

/// 404s when `id` already refers to a card outside `column_id`. A no-op when
/// `id` doesn't exist yet (the create arm) or already belongs to `column_id`.
fn require_card_in_column_if_present(
    ctx: &KanbanContext,
    id: Uuid,
    column_id: Uuid,
) -> Result<(), ApiError> {
    if let Some(existing) = ctx.get_card(id).map_err(|e| ApiError::from(&e))? {
        if existing.column_id != column_id {
            return Err(ApiError::from(&KanbanError::not_found("Card", id)));
        }
    }
    Ok(())
}
