use kanban_backend_memory::InMemoryStore;
use kanban_domain::data_store::DataStore;
use kanban_domain::{Board, KanbanResult};

#[test]
fn test_read_full_snapshot_is_reachable_from_an_integration_test() -> KanbanResult<()> {
    let store = InMemoryStore::new();
    let board = Board::new("B", None::<String>);
    let board_id = board.id;
    store.upsert_board(board)?;

    let snap = kanban_service::read_full_snapshot(&store)?;

    assert_eq!(snap.boards.len(), 1);
    assert_eq!(snap.boards[0].id, board_id);
    Ok(())
}
