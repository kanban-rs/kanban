use crate::cli::{BoardAction, BoardUpdateArgs};
use crate::context::CliContext;
use crate::output;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::commands::{BoardCommand, ColumnCommand, Command, CreateBoard, CreateColumn};
use kanban_domain::{ArchivedFilter, BoardListFilter, BoardUpdate, FieldUpdate, KanbanOperations};
use kanban_service::api::BoardResponse;

pub async fn handle(ctx: &mut CliContext, action: BoardAction) -> anyhow::Result<()> {
    match action {
        BoardAction::Create {
            name,
            card_prefix,
            with_default_columns,
        } => {
            let board = if with_default_columns {
                let board_id = uuid::Uuid::new_v4();
                let position = ctx.list_boards()?.len() as i32;
                ctx.execute_commands(build_create_board_batch(
                    board_id,
                    name,
                    card_prefix,
                    position,
                ))?;
                ctx.get_board(board_id)?.ok_or_else(|| {
                    anyhow::anyhow!("board not found after creating it with default columns")
                })?
            } else {
                ctx.mutate(|c| c.create_board_impl(name, card_prefix))?
            };
            ctx.save().await?;
            output::output_success(BoardResponse::from(&board));
        }
        BoardAction::List {
            archived,
            include_archived,
            sort,
            order,
            page,
            page_size,
        } => {
            // B4a (KAN-927): ONE list command, three states, routed through the
            // service filter (B2). The archival selector is built from the flags
            // exactly as cards do (handlers/card.rs); `archived_at` is stamped
            // per board from its archive marker (the filtered `Board`s carry no
            // timestamp). Live payloads stay byte-identical (BoardResponse omits
            // archived_at when None — D2 skip_serializing_if).
            let filter = BoardListFilter {
                archived: if archived {
                    ArchivedFilter::ArchivedOnly
                } else if include_archived {
                    ArchivedFilter::Include
                } else {
                    ArchivedFilter::LiveOnly
                },
                sort: sort.map(|s| s.to_board_sort_field()),
                sort_order: order.map(|o| o.to_sort_order()),
                search: None,
            };
            let responses = match project_board_list(ctx, filter) {
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
                // Stamp the marker's `archived_at` so an archived board is not
                // returned looking live.
                Some(b) => output::output_success(BoardResponse::with_archived_at(
                    &b,
                    ctx.board_archived_at(uuid)?,
                )),
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
            ctx.mutate_unit(|c| c.delete_board_impl(uuid))?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"deleted": uuid.to_string()}));
        }
        BoardAction::Archive { board } => {
            // Live board (still in the live list) — the standard resolver suffices.
            let uuid = match ctx.resolve_board_id(&board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e.to_string()),
            };
            ctx.mutate_unit(|c| c.archive_board_impl(uuid))?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"archived": uuid.to_string()}));
        }
        BoardAction::Restore { board } => {
            // Resolve against the ARCHIVED collection only (ArchivedOnly filter):
            // a same-named live board must never be hit (KAN-894).
            let uuid = match resolve_archived_board(ctx, &board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e),
            };
            ctx.mutate_unit(|c| c.restore_board_impl(uuid))?;
            ctx.save().await?;
            match ctx.get_board(uuid)? {
                Some(b) => output::output_success(BoardResponse::from(&b)),
                None => return output::output_error(&format!("Board not found: {}", board)),
            }
        }
        BoardAction::DeleteArchived { board } => {
            // Resolve against the ARCHIVED collection only: a same-named live
            // board must never be hit by `delete-archived` (delete_board cascades).
            let uuid = match resolve_archived_board(ctx, &board) {
                Ok(u) => u,
                Err(e) => return output::output_error(&e),
            };
            ctx.mutate_unit(|c| c.delete_board_impl(uuid))?;
            ctx.save().await?;
            output::output_success(serde_json::json!({"deleted": uuid.to_string()}));
        }
        BoardAction::SetSort { .. } => {
            // Config-only: intercepted in `CliApp::run_with_args` before a data
            // file is opened, so it never reaches the data-file dispatch path.
            unreachable!("board set-sort is handled before context load");
        }
    }
    Ok(())
}

/// Project a service-filtered board list, stamping `archived_at` per board from
/// its archive marker. The filtered `Board`s carry no timestamp (an archived
/// head is a still-live board plus a marker), so the marker's `archived_at` is
/// looked up by board id; a board with no marker is live and stamps `None`
/// (BoardResponse then omits the wire key).
fn project_board_list(
    ctx: &CliContext,
    filter: BoardListFilter,
) -> Result<Vec<BoardResponse>, String> {
    let archived_at: std::collections::HashMap<uuid::Uuid, chrono::DateTime<chrono::Utc>> = ctx
        .list_archived_boards()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|m| (m.entity_id, m.metadata.archived_at))
        .collect();

    Ok(ctx
        .list_boards_filtered(filter)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|board| BoardResponse::with_archived_at(board, archived_at.get(&board.id).copied()))
        .collect())
}

/// Resolve a board id from the ARCHIVED collection ONLY, for `-archived`
/// commands. A UUID passes straight through (the caller's op validates
/// existence); a name is matched against the `ArchivedOnly` service filter.
/// Because the candidate set is archived boards only, an `-archived` command can
/// never match a same-named LIVE board (REGR-4 / KAN-894 data-loss guard).
fn resolve_archived_board(ctx: &CliContext, raw: &str) -> Result<uuid::Uuid, String> {
    if let Ok(uuid) = uuid::Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let archived = ctx
        .list_boards_filtered(BoardListFilter {
            archived: ArchivedFilter::ArchivedOnly,
            ..Default::default()
        })
        .map_err(|e| e.to_string())?;
    let matches: Vec<uuid::Uuid> = kanban_domain::find_boards_by_name(raw, &archived)
        .iter()
        .map(|b| b.id)
        .collect();
    match matches.as_slice() {
        [id] => Ok(*id),
        [] => Err(format!("No archived board named: {}", raw)),
        _ => Err(format!("Ambiguous archived board name: {}", raw)),
    }
}

fn build_create_board_batch(
    board_id: uuid::Uuid,
    name: String,
    card_prefix: Option<String>,
    position: i32,
) -> Vec<Command> {
    let mut commands = vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: board_id,
        name,
        card_prefix,
        position,
    }))];
    for (position, (name, default_status)) in kanban_domain::DEFAULT_TEMPLATE_COLUMNS
        .into_iter()
        .enumerate()
    {
        commands.push(Command::Column(ColumnCommand::Create(CreateColumn {
            id: uuid::Uuid::new_v4(),
            board_id,
            name: name.to_string(),
            position: position as i32,
            default_status,
        })));
    }
    commands
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
    let board = ctx.mutate(|c| c.update_board_impl(uuid, updates))?;
    ctx.save().await?;
    Ok(board)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::commands::{BoardCommand, ColumnCommand, Command};
    use kanban_domain::CardStatus;

    #[test]
    fn test_build_create_board_batch_builds_board_then_template_columns_in_one_batch() {
        let board_id = uuid::Uuid::new_v4();
        let commands =
            build_create_board_batch(board_id, "Board".to_string(), Some("KAN".to_string()), 2);

        assert_eq!(commands.len(), 4);
        match &commands[0] {
            Command::Board(BoardCommand::Create(create)) => {
                assert_eq!(create.id, board_id);
                assert_eq!(create.name, "Board");
                assert_eq!(create.card_prefix.as_deref(), Some("KAN"));
                assert_eq!(create.position, 2);
            }
            other => panic!("expected board create first, got {:?}", other),
        }

        let expected = [
            ("TODO", 0, Some(CardStatus::Todo)),
            ("Doing", 1, Some(CardStatus::InProgress)),
            ("Complete", 2, Some(CardStatus::Done)),
        ];
        for (command, (name, position, default_status)) in commands[1..].iter().zip(expected) {
            match command {
                Command::Column(ColumnCommand::Create(create)) => {
                    assert_eq!(create.board_id, board_id);
                    assert_eq!(create.name, name);
                    assert_eq!(create.position, position);
                    assert_eq!(create.default_status, default_status);
                }
                other => panic!("expected column create, got {:?}", other),
            }
        }
    }
}
