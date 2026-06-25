//! Typed sprint-create seam for the HTTP server.
//!
//! The server itself is still a stub (no router/transport yet), but the
//! create-from-spec wiring is real and tested here: this is the single funnel a
//! future `POST /v1/boards/:board_id/sprints` + `PUT /v1/sprints/:id` handler
//! binds to. The board FK is path-supplied on the nested route. The shared
//! [`CreateSprintRequest`]/[`ReplaceSprintRequest`] carry the client-settable
//! create/replace fields; the service mints `sprint_number` + `name_index`
//! against the owning board (the create DTO carries a `name` STRING, not the
//! internal `name_index`). The idempotent PUT-create
//! (`create_or_replace_sprint`) reports whether it created (`true`, 201) or
//! replaced (`false`, 200). A re-PUT of a client-supplied id that already exists
//! on the POST path is a conflict -> 409 (mapped from `AlreadyExists` by
//! [`ApiError`]). The resulting domain `Sprint` is projected onto the wire
//! [`SprintResponse`], whose `name` is resolved against the owning board.

use kanban_service::api::{ApiError, CreateSprintRequest, ReplaceSprintRequest, SprintResponse};
use kanban_service::{KanbanContext, KanbanOperations};
use uuid::Uuid;

/// `POST /v1/boards/:board_id/sprints`: create a sprint under the path-supplied
/// board. The body id is honoured when present (idempotent create with that
/// exact id); a present id that already exists is a conflict (409). Returns the
/// wire projection plus whether the sprint was created (`true`) or replaced an
/// existing id (`false`).
pub fn create_sprint(
    ctx: &mut KanbanContext,
    board_id: Uuid,
    req: CreateSprintRequest,
) -> Result<(SprintResponse, bool), ApiError> {
    // POST is a true create (not create-or-replace): a present client id that
    // already exists is a conflict (`AlreadyExists` -> 409), not a silent
    // replace. `create_sprint_from_spec` mints the id when absent and rejects a
    // collision before any side effect.
    let sprint = ctx
        .create_sprint_from_spec(board_id, req.id, req.name, req.prefix, false)
        .map_err(|e| ApiError::from(&e))?;
    project(ctx, sprint, true)
}

/// `PUT /v1/boards/:board_id/sprints/:id`: idempotent create-or-replace for a
/// sprint keyed on the path `id`. The board FK is path-supplied; the
/// server-managed `sprint_number`/status/dates stay stable across the replace
/// arm. The body is a true full replace ([`ReplaceSprintRequest`]); the absent
/// id maps to the create arm with that exact path id, a present id maps to a
/// content replace of the client-settable `name`/`prefix`. Returns the wire
/// projection plus whether the sprint was created (`true`) or replaced
/// (`false`).
pub fn create_or_replace_sprint(
    ctx: &mut KanbanContext,
    board_id: Uuid,
    id: Uuid,
    req: ReplaceSprintRequest,
) -> Result<(SprintResponse, bool), ApiError> {
    let ReplaceSprintRequest {
        name,
        prefix,
        card_prefix: _,
    } = req;
    let outcome = ctx
        .create_or_replace_sprint(board_id, id, name, prefix, false)
        .map_err(|e| ApiError::from(&e))?;
    project(ctx, outcome.sprint, outcome.created)
}

/// Project the created/replaced domain sprint onto its wire response, resolving
/// the sprint `name` against the owning board's name pool.
fn project(
    ctx: &KanbanContext,
    sprint: kanban_service::Sprint,
    created: bool,
) -> Result<(SprintResponse, bool), ApiError> {
    let board = ctx
        .get_board(sprint.board_id)
        .map_err(|e| ApiError::from(&e))?
        .ok_or_else(|| {
            ApiError::from(&kanban_service::KanbanError::not_found(
                "Board",
                sprint.board_id,
            ))
        })?;
    Ok((SprintResponse::from_sprint(&sprint, &board), created))
}
