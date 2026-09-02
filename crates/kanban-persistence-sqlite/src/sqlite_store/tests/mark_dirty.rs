use kanban_backend::KanbanBackend;
use kanban_domain::data_store::DataStore;
use tempfile::TempDir;

use crate::SqliteBackend;

#[tokio::test(flavor = "multi_thread")]
async fn test_mark_dirty_on_a_write_through_backend_does_not_disturb_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");

    let backend = SqliteBackend::open(path.to_str().unwrap()).await.unwrap();

    let board = kanban_domain::Board::new("B", None::<String>);
    let board_id = board.id;
    let column = kanban_domain::Column::new(board.id, "Col", 0);
    let column_id = column.id;
    let card = kanban_domain::Card::new(board.id, column.id, "Task", 0);
    let card_id = card.id;
    backend.upsert_board(board).unwrap();
    backend.upsert_column(column).unwrap();
    backend.upsert_card(card).unwrap();

    backend.mark_dirty();
    assert!(!backend.needs_flush());

    backend.flush().await.unwrap();
    drop(backend);

    let reopened = SqliteBackend::open(path.to_str().unwrap()).await.unwrap();
    assert_eq!(reopened.get_board(board_id).unwrap().unwrap().id, board_id);
    assert_eq!(
        reopened.get_column(column_id).unwrap().unwrap().id,
        column_id
    );
    assert_eq!(reopened.get_card(card_id).unwrap().unwrap().id, card_id);
}
