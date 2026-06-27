//! Typed column-create seam for the HTTP server.
//!
//! The server itself is still a stub (no router/transport yet), but the
//! create-from-spec wiring is real and tested here: this is the single funnel a
//! future `POST /v1/boards/:id/columns` + `PUT /v1/columns/:id` handler binds
//! to. The shared [`CreateColumnRequest`] is split via `into_new_column`
//! (board id is path-supplied on the nested route), runs the idempotent
//! PUT-create (`create_or_replace_column`), and the resulting domain `Column`
//! is projected onto the wire [`ColumnResponse`]. The `created` flag lets the
//! eventual HTTP layer answer 201 (created) vs 200 (replaced).

use kanban_service::api::{ApiError, ColumnResponse, CreateColumnRequest};
use kanban_service::KanbanContext;
use uuid::Uuid;

/// `POST /v1/boards/:board_id/columns`: append-create a column under the
/// path-supplied board. The body id is honoured when present (idempotent), else
/// minted. Returns the wire projection plus whether the column was created
/// (`true`) or replaced an existing id (`false`).
pub fn create_column(
    ctx: &mut KanbanContext,
    board_id: Uuid,
    req: CreateColumnRequest,
) -> Result<(ColumnResponse, bool), ApiError> {
    let (maybe_id, spec) = req
        .into_new_column(board_id)
        .map_err(|e| ApiError::from(&e))?;
    let id = maybe_id.unwrap_or_else(Uuid::new_v4);
    let outcome = ctx
        .create_or_replace_column(id, spec)
        .map_err(|e| ApiError::from(&e))?;
    Ok((ColumnResponse::from(&outcome.column), outcome.created))
}

/// `PUT /v1/boards/:board_id/columns/:id`: idempotent create-or-replace for a
/// column keyed on the path `id`. The board FK is path-supplied; `position`
/// stays server-managed across the replace arm. Returns the wire projection
/// plus whether the column was created (`true`) or replaced (`false`).
pub fn create_or_replace_column(
    ctx: &mut KanbanContext,
    board_id: Uuid,
    id: Uuid,
    req: CreateColumnRequest,
) -> Result<(ColumnResponse, bool), ApiError> {
    let (_body_id, spec) = req
        .into_new_column(board_id)
        .map_err(|e| ApiError::from(&e))?;
    let outcome = ctx
        .create_or_replace_column(id, spec)
        .map_err(|e| ApiError::from(&e))?;
    Ok((ColumnResponse::from(&outcome.column), outcome.created))
}
