//! Typed column-create/replace seam for the HTTP server.
//!
//! This is the funnel `POST /v1/boards/:id/columns` +
//! `PUT /v1/boards/:id/columns/:id` handlers bind to. The shared DTOs are
//! split via `into_new_column` (board id is path-supplied on the nested
//! route), runs the idempotent PUT-create (`create_or_replace_column`), and
//! the resulting domain `Column` is projected onto the wire
//! [`ColumnResponse`]. The `created` flag lets the HTTP layer answer 201
//! (created) vs 200 (replaced).

use kanban_service::api::{ApiError, ColumnResponse, CreateColumnRequest, ReplaceColumnRequest};
use kanban_service::{KanbanContext, KanbanError, KanbanOperations};
use uuid::Uuid;

/// `POST /v1/boards/:board_id/columns`: append-create a column under the
/// path-supplied board. The body id is honoured when present (idempotent), else
/// minted. Returns the wire projection plus whether the column was created
/// (`true`) or replaced an existing id (`false`). If the body id already
/// exists under a *different* board, this 404s rather than silently
/// relocating it — see [`create_or_replace_column`].
pub fn create_column(
    ctx: &mut KanbanContext,
    board_id: Uuid,
    req: CreateColumnRequest,
) -> Result<(ColumnResponse, bool), ApiError> {
    let (maybe_id, spec) = req
        .into_new_column(board_id)
        .map_err(|e| ApiError::from(&e))?;
    let id = maybe_id.unwrap_or_else(Uuid::new_v4);
    require_column_in_board_if_present(ctx, id, board_id)?;
    let outcome = ctx
        .create_or_replace_column(id, spec, None)
        .map_err(|e| ApiError::from(&e))?;
    Ok((ColumnResponse::from(&outcome.column), outcome.created))
}

/// `PUT /v1/boards/:board_id/columns/:id`: idempotent create-or-replace for a
/// column keyed on the path `id`. The board FK is path-supplied; `position` is
/// set from the request (full-replace semantics of PUT). Returns the wire
/// projection plus whether the column was created (`true`) or replaced
/// (`false`). If `id` already exists under a *different* board, this 404s
/// rather than silently replacing it — mirrors the read route's cross-board
/// guard (`routes/columns.rs::get_column`) since `create_or_replace_column`'s
/// replace arm never checks the existing column's board on its own.
pub fn create_or_replace_column(
    ctx: &mut KanbanContext,
    board_id: Uuid,
    id: Uuid,
    req: ReplaceColumnRequest,
) -> Result<(ColumnResponse, bool), ApiError> {
    require_column_in_board_if_present(ctx, id, board_id)?;
    let (spec, position) = req
        .into_new_column(board_id)
        .map_err(|e| ApiError::from(&e))?;
    let outcome = ctx
        .create_or_replace_column(id, spec, Some(position))
        .map_err(|e| ApiError::from(&e))?;
    Ok((ColumnResponse::from(&outcome.column), outcome.created))
}

/// 404s when `id` already refers to a column outside `board_id`. A no-op
/// when `id` doesn't exist yet (the create arm) or already belongs to
/// `board_id`.
fn require_column_in_board_if_present(
    ctx: &KanbanContext,
    id: Uuid,
    board_id: Uuid,
) -> Result<(), ApiError> {
    if let Some(existing) = ctx.get_column(id).map_err(|e| ApiError::from(&e))? {
        if existing.board_id != board_id {
            return Err(ApiError::from(&KanbanError::not_found("Column", id)));
        }
    }
    Ok(())
}
