//! `KanbanBackend::with_transaction` atomicity contract: every
//! mutation in the closure commits together or rolls back together.

use kanban_backend_memory::InMemoryStore;
use kanban_domain::data_store::DataStore;
use kanban_domain::{Board, Card, Column, KanbanError, KanbanResult, Sprint};
use kanban_service::backend::KanbanBackend;
use std::sync::Arc;

#[test]
fn test_with_transaction_commits_on_success() -> KanbanResult<()> {
    let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());
    let board = Board::new("Committed", None::<String>);
    let board_id = board.id;

    let backend_for_closure = Arc::clone(&backend);
    backend.with_transaction(&mut || {
        let store: &dyn DataStore = backend_for_closure.as_data_store();
        store.upsert_board(board.clone())
    })?;

    let boards = backend.list_boards()?;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, board_id);
    Ok(())
}

#[test]
fn test_with_transaction_rolls_back_on_failure() -> KanbanResult<()> {
    let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());
    // Pre-state: one board already exists; the transaction below tries to add
    // a second but fails. After rollback only the pre-state survives.
    backend.upsert_board(Board::new("Original", None::<String>))?;
    let pre_count = backend.list_boards()?.len();

    let backend_for_closure = Arc::clone(&backend);
    let result = backend.with_transaction(&mut || {
        let store: &dyn DataStore = backend_for_closure.as_data_store();
        store.upsert_board(Board::new("Will be rolled back", None::<String>))?;
        Err(KanbanError::Internal("simulated failure".into()))
    });

    assert!(
        result.is_err(),
        "transaction must propagate the inner error"
    );
    let post_count = backend.list_boards()?.len();
    assert_eq!(
        post_count, pre_count,
        "rollback must restore the entity count to its pre-transaction value"
    );
    Ok(())
}

#[test]
fn test_with_transaction_propagates_inner_error() -> KanbanResult<()> {
    let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());

    let backend_for_closure = Arc::clone(&backend);
    let err = backend
        .with_transaction(&mut || {
            let _store: &dyn DataStore = backend_for_closure.as_data_store();
            Err(KanbanError::Internal("inner error message".into()))
        })
        .unwrap_err();

    let msg = format!("{err}");
    assert!(
        msg.contains("inner error message"),
        "the original error message must be preserved (got: {msg:?})"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_execute_partial_batch_failure_rolls_back_via_transaction() -> KanbanResult<()> {
    // End-to-end check: KanbanContext::execute uses with_transaction, so a
    // batch that fails partway through leaves no trace of the successful
    // commands.
    use kanban_core::AppConfig;
    use kanban_domain::commands::{BoardCommand, Command, CreateBoard, UpdateBoard};
    use kanban_domain::BoardUpdate;
    use kanban_service::KanbanContext;
    use uuid::Uuid;

    let mut ctx = KanbanContext::open(Arc::new(InMemoryStore::new()), AppConfig::default()).await?;

    let valid = Command::Board(BoardCommand::Create(CreateBoard {
        id: Uuid::new_v4(),
        name: "First".into(),
        card_prefix: None,
        position: 0,
    }));
    // Force a failure by issuing a board update against a non-existent
    // board_id. UpdateBoard::execute returns a NotFound error in that case.
    let failing = Command::Board(BoardCommand::Update(UpdateBoard {
        board_id: Uuid::new_v4(),
        updates: BoardUpdate {
            name: Some("renamed-but-target-missing".into()),
            ..Default::default()
        },
    }));

    let result = ctx.execute(vec![valid, failing]);
    assert!(result.is_err(), "batch must surface the inner failure");

    assert_eq!(
        ctx.boards()?.len(),
        0,
        "rollback must remove the board that was created before the failure"
    );
    Ok(())
}

// NOTE: this test already passes against the pre-KAN-1110 default
// `with_transaction` body, because that default's snapshot/restore already
// covers the whole `InMemoryStore` state (via `snapshot_impl`/
// `apply_snapshot_impl`). It is regression cover pinning the override's
// full-graph behaviour, not a Red test for this card; the card's genuine Red
// surface is the compile-level gate (every `KanbanBackend` impl must supply
// `with_transaction` once the default is deleted).
#[test]
fn test_in_memory_with_transaction_rolls_back_full_graph() -> KanbanResult<()> {
    let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());

    let mut board = Board::new("Board", None::<String>);
    let col = Column::new(board.id, "Col", 0);
    let card_a = Card::new(&mut board, col.id, "A", 0);
    let card_b = Card::new(&mut board, col.id, "B", 1);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);

    backend.upsert_board(board.clone())?;
    backend.upsert_column(col.clone())?;
    backend.upsert_card(card_a.clone())?;
    backend.upsert_card(card_b.clone())?;
    backend.upsert_sprint(sprint.clone())?;

    let mut graph = backend.get_graph()?;
    graph.set_block(card_a.id, card_b.id)?;
    backend.set_graph(graph)?;

    let before = backend.snapshot()?;

    let backend_for_closure = Arc::clone(&backend);
    let result = backend.with_transaction(&mut || {
        let store: &dyn DataStore = backend_for_closure.as_data_store();
        store.upsert_board(Board::new("Injected", None::<String>))?;
        store.delete_card(card_a.id)?;
        store.delete_sprint(sprint.id)?;
        Err(KanbanError::validation("forced batch failure"))
    });

    assert!(result.is_err(), "the batch's own error must propagate");

    let after = backend.snapshot()?;
    assert_eq!(
        after, before,
        "entire graph must be restored byte-identical after a failed batch"
    );

    assert!(
        !backend.list_boards()?.iter().any(|b| b.name == "Injected"),
        "the injected board from the failed batch must not survive"
    );
    assert!(
        backend.get_card(card_a.id)?.is_some(),
        "deleted card must be restored"
    );
    assert!(
        backend.get_sprint(sprint.id)?.is_some(),
        "deleted sprint must be restored"
    );
    let restored_graph = backend.get_graph()?;
    assert!(
        restored_graph
            .blocks_edges()
            .iter()
            .any(|e| e.base.source == card_a.id && e.base.target == card_b.id),
        "the block edge must survive rollback"
    );

    Ok(())
}
