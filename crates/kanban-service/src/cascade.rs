use kanban_domain::commands::cascade_commands::{
    CascadeCommand, DeleteArchivedCards, DeleteCardEdges, DeleteCardsByColumns,
    DeleteColumnsByBoard, DeleteSprintsByBoard,
};
use kanban_domain::commands::{BoardCommand, Command, DeleteBoard};
use kanban_domain::data_store::DataStore;
use kanban_domain::KanbanResult;
use uuid::Uuid;

pub(crate) fn delete_board(store: &dyn DataStore, board_id: Uuid) -> KanbanResult<Vec<Command>> {
    let column_ids: Vec<Uuid> = store
        .list_columns_by_board(board_id)?
        .iter()
        .map(|c| c.id)
        .collect();

    // Gather archived cards by the first-class `board_id` field, not by (historical)
    // column membership. A card whose column was deleted after archival — now
    // possible since the DeleteColumn archived-cards guard is gone — has a dangling
    // `original_column_id` and is missed by a by-columns gather, leaking its archived
    // record plus its graph edges. Every backend populates `board_id` (B4), so the
    // board-scoped query catches all of them.
    let archived_card_ids: Vec<Uuid> = store
        .list_archived_cards_by_board(board_id)?
        .into_iter()
        .map(|ac| ac.entity_id)
        .collect();

    // Widened early-return guard: the only remaining board may hold nothing but
    // archived cards whose column is already gone (column_ids empty) — do not skip
    // the cascade in that case, or those records leak. Likewise a board with only
    // sprints (all columns emptied and deleted) must still emit
    // `DeleteSprintsByBoard`, else its sprints leak (JSON/in-memory) or FK-cascade
    // without undo capture (SQLite), so undo would restore the board without them.
    let has_sprints = !store.list_sprints_by_board(board_id)?.is_empty();
    if column_ids.is_empty() && archived_card_ids.is_empty() && !has_sprints {
        return Ok(vec![Command::Board(BoardCommand::Delete(DeleteBoard {
            board_id,
        }))]);
    }

    let mut card_ids: Vec<Uuid> = store
        .list_cards_by_columns(&column_ids)?
        .iter()
        .map(|c| c.id)
        .collect();
    card_ids.extend(archived_card_ids.iter().copied());

    let mut commands = Vec::new();
    commands.extend([
        Command::Cascade(CascadeCommand::DeleteCardEdges(DeleteCardEdges {
            ids: card_ids,
        })),
        Command::Cascade(CascadeCommand::DeleteCardsByColumns(DeleteCardsByColumns {
            column_ids: column_ids.clone(),
        })),
        Command::Cascade(CascadeCommand::DeleteArchivedCards(DeleteArchivedCards {
            card_ids: archived_card_ids,
        })),
        Command::Cascade(CascadeCommand::DeleteColumnsByBoard(DeleteColumnsByBoard {
            board_id,
        })),
        Command::Cascade(CascadeCommand::DeleteSprintsByBoard(DeleteSprintsByBoard {
            board_id,
        })),
        Command::Board(BoardCommand::Delete(DeleteBoard { board_id })),
    ]);
    Ok(commands)
}
