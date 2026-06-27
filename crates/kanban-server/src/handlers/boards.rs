//! Typed board-create seam for the HTTP server.
//!
//! The server itself is still a stub (no router/transport yet), but the
//! create-from-spec wiring is real and tested here: this is the single funnel a
//! future `PUT /v1/boards/:id` handler binds to. It takes the shared
//! [`CreateBoardRequest`], splits it via `into_new_board`, runs the idempotent
//! PUT-create (`create_or_replace_board`), and projects the resulting domain
//! `Board` onto the wire [`BoardResponse`]. The `created` flag lets the eventual
//! HTTP layer answer 201 (created) vs 200 (replaced).

use kanban_service::api::{ApiError, BoardResponse, CreateBoardRequest};
use kanban_service::KanbanContext;

/// Idempotent create-or-replace for a board, returning the wire projection plus
/// whether the board was created (`true`) or replaced (`false`).
///
/// The `id` is taken from the request body (the PUT-create contract). When the
/// request omits an id, one is minted by the create arm; the replace arm only
/// runs for an id that already exists.
pub fn create_or_replace_board(
    ctx: &mut KanbanContext,
    req: CreateBoardRequest,
) -> Result<(BoardResponse, bool), ApiError> {
    let (maybe_id, spec) = req.into_new_board();
    let id = maybe_id.unwrap_or_else(uuid::Uuid::new_v4);
    let outcome = ctx
        .create_or_replace_board(id, spec)
        .map_err(|e| ApiError::from(&e))?;
    Ok((BoardResponse::from(&outcome.board), outcome.created))
}
