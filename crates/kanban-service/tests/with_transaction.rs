//! `KanbanBackend::with_transaction` atomicity contract: every
//! mutation in the closure commits together or rolls back together.

use kanban_backend_memory::InMemoryStore;
use kanban_domain::data_store::DataStore;
use kanban_domain::{Board, KanbanError, KanbanResult};
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
