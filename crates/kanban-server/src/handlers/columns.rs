//! Typed column-create/replace seam for the HTTP server.
//!
//! The server itself is still a stub (no router/transport yet), but the
//! create-from-spec and replace-from-spec wiring is real and tested here: this
//! is the funnel `POST /v1/boards/:id/columns` + `PUT /v1/boards/:id/columns/:id`
//! handlers bind to. The shared DTOs are split via `into_new_column` (board id
//! is path-supplied on the nested route), runs the idempotent PUT-create
//! (`create_or_replace_column`), and the resulting domain `Column` is projected
//! onto the wire [`ColumnResponse`]. The `created` flag lets the HTTP layer
//! answer 201 (created) vs 200 (replaced).

use kanban_service::api::{ApiError, ColumnResponse, CreateColumnRequest, ReplaceColumnRequest};
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
        .create_or_replace_column(id, spec, None)
        .map_err(|e| ApiError::from(&e))?;
    Ok((ColumnResponse::from(&outcome.column), outcome.created))
}

/// `PUT /v1/boards/:board_id/columns/:id`: idempotent create-or-replace for a
/// column keyed on the path `id`. The board FK is path-supplied; `position` is
/// set from the request (full-replace semantics of PUT). Returns the wire
/// projection plus whether the column was created (`true`) or replaced
/// (`false`).
pub fn create_or_replace_column(
    ctx: &mut KanbanContext,
    board_id: Uuid,
    id: Uuid,
    req: ReplaceColumnRequest,
) -> Result<(ColumnResponse, bool), ApiError> {
    let (spec, position) = req
        .into_new_column(board_id)
        .map_err(|e| ApiError::from(&e))?;
    let outcome = ctx
        .create_or_replace_column(id, spec, Some(position))
        .map_err(|e| ApiError::from(&e))?;
    Ok((ColumnResponse::from(&outcome.column), outcome.created))
}
