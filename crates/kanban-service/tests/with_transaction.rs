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
    backend.with_transaction(Box::new(|| {
        let store: &dyn DataStore = backend_for_closure.as_data_store();
        store.upsert_board(board.clone())
    }))?;

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
    let result = backend.with_transaction(Box::new(|| {
        let store: &dyn DataStore = backend_for_closure.as_data_store();
        store.upsert_board(Board::new("Will be rolled back", None::<String>))?;
        Err(KanbanError::Internal("simulated failure".into()))
    }));

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
        .with_transaction(Box::new(|| {
            let _store: &dyn DataStore = backend_for_closure.as_data_store();
            Err(KanbanError::Internal("inner error message".into()))
        }))
        .unwrap_err();

    let msg = format!("{err}");
    assert!(
        msg.contains("inner error message"),
        "the original error message must be preserved (got: {msg:?})"
    );
    Ok(())
}

#[test]
fn test_in_memory_with_transaction_rolls_back_full_graph() -> KanbanResult<()> {
    // `test_with_transaction_rolls_back_on_failure` only checks a board count.
    // Rollback has to restore every entity the store owns plus the workspace
    // graph, so seed a non-trivial graph and assert all of it comes back.
    use kanban_domain::{Card, Column, Sprint};

    let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());

    let mut board = Board::new("Seeded", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let blocker = Card::new(&mut board, column.id, "Blocker", 0);
    let blocked = Card::new(&mut board, column.id, "Blocked", 1);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);

    let (board_id, column_id) = (board.id, column.id);
    let (blocker_id, blocked_id, sprint_id) = (blocker.id, blocked.id, sprint.id);

    backend.upsert_board(board)?;
    backend.upsert_column(column)?;
    backend.upsert_card(blocker)?;
    backend.upsert_card(blocked)?;
    backend.upsert_sprint(sprint)?;
    backend.modify_graph(Box::new(move |graph| {
        graph.set_block(blocker_id, blocked_id)
    }))?;

    let backend_for_closure = Arc::clone(&backend);
    let result = backend.with_transaction(Box::new(move || {
        let store: &dyn DataStore = backend_for_closure.as_data_store();
        store.delete_card(blocked_id)?;
        store.delete_sprint(sprint_id)?;
        store.modify_graph(Box::new(move |graph| {
            graph.unblock(blocker_id, blocked_id)?;
            Ok(())
        }))?;
        store.delete_column(column_id)?;
        store.delete_board(board_id)?;
        Err(KanbanError::Internal("simulated failure".into()))
    }));
    assert!(
        result.is_err(),
        "transaction must propagate the inner error"
    );

    assert!(
        backend.get_board(board_id)?.is_some(),
        "rollback must restore the board"
    );
    assert!(
        backend.get_column(column_id)?.is_some(),
        "rollback must restore the column"
    );
    assert!(
        backend.get_card(blocker_id)?.is_some() && backend.get_card(blocked_id)?.is_some(),
        "rollback must restore both cards"
    );
    assert!(
        backend.get_sprint(sprint_id)?.is_some(),
        "rollback must restore the sprint"
    );
    assert_eq!(
        backend.get_graph()?.blockers(blocked_id),
        vec![blocker_id],
        "rollback must restore the dependency edge, which lives in the \
         workspace-global graph rather than being owned by the board"
    );
    Ok(())
}

#[test]
fn test_with_transaction_accepts_a_closure_that_consumes_captured_state() -> KanbanResult<()> {
    // Pins the FnOnce contract: a closure may move an owned value out of its
    // captures. Under `&mut dyn FnMut` this does not compile and callers have
    // to launder the value through an `Option::take()`.
    let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());
    let board = Board::new("Moved in", None::<String>);
    let board_id = board.id;

    let backend_for_closure = Arc::clone(&backend);
    backend.with_transaction(Box::new(move || {
        backend_for_closure.as_data_store().upsert_board(board)
    }))?;

    assert!(
        backend.get_board(board_id)?.is_some(),
        "the moved-in board must have been committed"
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
