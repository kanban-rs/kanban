use crate::cli::{BoardAction, BoardUpdateArgs};
use crate::context::CliContext;
use crate::output;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{BoardUpdate, FieldUpdate, KanbanOperations};
use kanban_service::api::BoardResponse;

pub async fn handle(ctx: &mut CliContext, action: BoardAction) -> anyhow::Result<()> {
    match action {
        BoardAction::Create { name, card_prefix } => {
            // Funnels through the Board factory via the name/card_prefix shim
            // (KAN-792); the JSON edge projects the domain Board via BoardResponse.
            let board = ctx.create_board(name, card_prefix)?;
            ctx.save().await?;
            output::output_success(BoardResponse::from(&board));
        }
        BoardAction::List {
            archived,
            include_archived,
            page,
            page_size,
        } => {
            // I4 (KAN-886): ONE list command, three states. Boards have no domain
            // filter struct, so the selector is composed here from the service ops.
            // Live board payloads stay byte-identical (BoardResponse omits
            // archived_at when None — D2 skip_serializing_if).
            let responses = match build_board_list(ctx, archived, include_archived) {
                Ok(r) => r,
                Err(e) => return output::output_error(&e),
            };
            let (page, page_size) = resolve_page_params(page, page_size)?;
            output::output_success(PaginatedList::paginate(responses, page, page_size)?);
        }
        BoardAction::Get { board } => {
            let uuid = match ctx.resolve_board_id(&board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            match ctx.get_board(uuid)? {
                Some(b) => output::output_success(BoardResponse::from(&b)),
                None => return output::output_error(&format!("Board not found: {}", board)),
            }
        }
        BoardAction::Update(args) => {
            let board = handle_update(ctx, args).await?;
            output::output_success(BoardResponse::from(&board));
        }
        BoardAction::Delete { board } => {
            let uuid = match ctx.resolve_board_id(&board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            ctx.delete_board(uuid)?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"deleted": uuid.to_string()}));
        }
        BoardAction::Archive { board } => {
            // Live board (still in the live list) — the standard resolver suffices.
            let uuid = match ctx.resolve_board_id(&board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            ctx.archive_board(uuid)?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"archived": uuid.to_string()}));
        }
        BoardAction::Restore { board } => {
            let uuid = match resolve_archived_board_id(ctx, &board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e),
            };
            ctx.restore_board(uuid)?;
            ctx.save().await?;
            match ctx.get_board(uuid)? {
                Some(b) => output::output_success(BoardResponse::from(&b)),
                None => return output::output_error(&format!("Board not found: {}", board)),
            }
        }
        BoardAction::DeleteArchived { board } => {
            // Resolve against the ARCHIVED collection only: a same-named live
            // board must never be hit by `delete-archived` (delete_board cascades).
            let uuid = match resolve_archived_board_id(ctx, &board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e),
            };
            ctx.delete_board(uuid)?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"deleted": uuid.to_string()}));
        }
    }
    Ok(())
}

/// Compose the three-state board list from the service ops.
/// - default: live boards (`archived_at` None).
/// - `--archived`: only archived boards, each stamped `archived_at` (the live
///   head resolved by the marker's `entity_id`).
/// - `--include-archived`: live boards followed by the archived ones.
fn build_board_list(
    ctx: &CliContext,
    archived: bool,
    include_archived: bool,
) -> Result<Vec<BoardResponse>, String> {
    let mut out: Vec<BoardResponse> = Vec::new();

    if !archived {
        out.extend(
            ctx.list_boards()
                .map_err(|e| e.to_string())?
                .iter()
                .map(BoardResponse::from),
        );
    }

    if archived || include_archived {
        for marker in ctx.list_archived_boards().map_err(|e| e.to_string())? {
            // `get_board` is unfiltered: it resolves the still-live head.
            if let Some(board) = ctx.get_board(marker.entity_id).map_err(|e| e.to_string())? {
                out.push(BoardResponse::archived(&board, marker.metadata.archived_at));
            }
        }
    }

    Ok(out)
}

/// Resolve a board id from EITHER the live or the archived view. UUIDs resolve
/// immediately; a live-board name resolves via the standard resolver; otherwise
/// fall back to matching an archived board by name (its head is unfiltered via
/// `get_board`). Keeps the domain resolver (live-only) unchanged.
/// Resolve a board id from the ARCHIVED collection ONLY. A UUID passes through
/// (the caller's op validates existence); a name matches an archived board's
/// head exactly. This never matches a LIVE board, so an `-archived` command can
/// never accidentally hit a same-named live board (REGR-4 / KAN-894 data-loss).
fn resolve_archived_board_id(ctx: &CliContext, raw: &str) -> Result<uuid::Uuid, String> {
    if let Ok(uuid) = uuid::Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let mut matches: Vec<uuid::Uuid> = Vec::new();
    for marker in ctx.list_archived_boards().map_err(|e| e.to_string())? {
        if let Some(board) = ctx.get_board(marker.entity_id).map_err(|e| e.to_string())? {
            if board.name == raw {
                matches.push(board.id);
            }
        }
    }
    match matches.as_slice() {
        [id] => Ok(*id),
        [] => Err(format!("No archived board named: {}", raw)),
        _ => Err(format!("Ambiguous archived board name: {}", raw)),
    }
}

async fn handle_update(
    ctx: &mut CliContext,
    args: BoardUpdateArgs,
) -> anyhow::Result<kanban_domain::Board> {
    let uuid = ctx
        .resolve_board_id(&args.board)
        .map_err(anyhow::Error::from)?;
    let updates = BoardUpdate {
        name: args.name,
        description: args
            .description
            .map(FieldUpdate::Set)
            .unwrap_or(FieldUpdate::NoChange),
        sprint_prefix: args
            .sprint_prefix
            .map(FieldUpdate::Set)
            .unwrap_or(FieldUpdate::NoChange),
        card_prefix: args
            .card_prefix
            .map(FieldUpdate::Set)
            .unwrap_or(FieldUpdate::NoChange),
        task_sort_field: args.sort_field.map(|s| s.to_sort_field()),
        task_sort_order: args.sort_order.map(|o| o.to_sort_order()),
        ..Default::default()
    };
    let board = ctx.update_board(uuid, updates)?;
    ctx.save().await?;
    Ok(board)
}
