//! Typed card-create seam for the HTTP server.
//!
//! The server itself is still a stub (no router/transport yet), but the
//! create-from-spec wiring is real and tested here: this is the single funnel a
//! future `POST /v1/columns/:column_id/cards` + `PUT /v1/cards/:id` handler
//! binds to. The shared [`CreateCardRequest`] is split via
//! `into_new_card(column_id)` (the column FK is path-supplied on the nested
//! route; the owning board is derived from the column server-side), runs the
//! idempotent PUT-create (`create_or_replace_card`), and the resulting domain
//! `Card` is projected onto the wire [`CardResponse`]. The `created` flag lets
//! the eventual HTTP layer answer 201 (created) vs 200 (replaced).

use kanban_service::api::{ApiError, CardResponse, CreateCardRequest};
use kanban_service::KanbanContext;
use uuid::Uuid;

/// `POST /v1/columns/:column_id/cards`: append-create a card under the
/// path-supplied column. The body id is honoured when present (idempotent), else
/// minted. Returns the wire projection plus whether the card was created
/// (`true`) or replaced an existing id (`false`).
pub fn create_card(
    ctx: &mut KanbanContext,
    column_id: Uuid,
    req: CreateCardRequest,
) -> Result<(CardResponse, bool), ApiError> {
    let (maybe_id, spec) = req
        .into_new_card(column_id)
        .map_err(|e| ApiError::from(&e))?;
    let id = maybe_id.unwrap_or_else(Uuid::new_v4);
    let outcome = ctx
        .create_or_replace_card(id, spec)
        .map_err(|e| ApiError::from(&e))?;
    Ok((CardResponse::from(&outcome.card), outcome.created))
}

/// `PUT /v1/columns/:column_id/cards/:id`: idempotent create-or-replace for a
/// card keyed on the path `id`. The column FK is path-supplied; the
/// server-managed `card_number`/`position` stay stable across the replace arm.
/// Returns the wire projection plus whether the card was created (`true`) or
/// replaced (`false`).
pub fn create_or_replace_card(
    ctx: &mut KanbanContext,
    column_id: Uuid,
    id: Uuid,
    req: CreateCardRequest,
) -> Result<(CardResponse, bool), ApiError> {
    let (_body_id, spec) = req
        .into_new_card(column_id)
        .map_err(|e| ApiError::from(&e))?;
    let outcome = ctx
        .create_or_replace_card(id, spec)
        .map_err(|e| ApiError::from(&e))?;
    Ok((CardResponse::from(&outcome.card), outcome.created))
}
