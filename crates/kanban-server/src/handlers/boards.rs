//! Typed board create/replace seams for the HTTP server.
//!
//! `create_board` is the `POST /v1/boards` seam (pure create, always mints or
//! accepts a client id, 409 on collision). `create_or_replace_board` is the
//! `PUT /v1/boards/:id` seam (idempotent create-or-replace keyed on the path
//! id). Both project the resulting domain `Board` onto the wire
//! [`BoardResponse`].

use kanban_service::api::{ApiError, BoardResponse, CreateBoardRequest, ReplaceBoardRequest};
use kanban_service::KanbanContext;
use uuid::Uuid;

/// `POST /v1/boards`: pure create. The body id is honoured when present
/// (idempotent create with that exact id); a present id that already exists
/// is a conflict (`AlreadyExists` -> 409), not a silent replace.
pub fn create_board(
    ctx: &mut KanbanContext,
    req: CreateBoardRequest,
) -> Result<BoardResponse, ApiError> {
    let (id, spec) = req.into_new_board();
    let board = ctx
        .create_board_from_spec(id, spec)
        .map_err(|e| ApiError::from(&e))?;
    Ok(BoardResponse::from(&board))
}

/// `PUT /v1/boards/:id`: idempotent create-or-replace for a board keyed on
/// the path `id`. Returns the wire projection plus whether the board was
/// created (`true`, 201) or replaced (`false`, 200).
pub fn create_or_replace_board(
    ctx: &mut KanbanContext,
    id: Uuid,
    req: ReplaceBoardRequest,
) -> Result<(BoardResponse, bool), ApiError> {
    let spec = req.into_new_board();
    let outcome = ctx
        .create_or_replace_board(id, spec)
        .map_err(|e| ApiError::from(&e))?;
    Ok((BoardResponse::from(&outcome.board), outcome.created))
}
