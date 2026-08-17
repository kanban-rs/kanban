use kanban_backend::KanbanBackend;
use kanban_domain::{Board, Card, Column, DataStore, KanbanResult, Sprint};
use kanban_persistence_sqlite::SqliteBackend;
use tempfile::TempDir;

async fn open(path: &std::path::Path) -> SqliteBackend {
    SqliteBackend::open(path.to_str().unwrap()).await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_with_transaction_commits_full_graph_via_real_db_transaction() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let backend = open(&path).await;

    let board = Board::new("B", None::<String>);
    let board_id = board.id;
    let column = Column::new(board.id, "Todo", 0);
    let column_id = column.id;
    let card = Card::new(board.id, column.id, "Card", 0);
    let card_id = card.id;
    let sprint = Sprint::new(board.id, 1, None, Some("SP"));
    let sprint_id = sprint.id;
    let other = uuid::Uuid::new_v4();

    let result: KanbanResult<()> = backend.with_transaction(Box::new(|| {
        backend.upsert_board(board.clone())?;
        backend.upsert_column(column.clone())?;
        backend.upsert_card(card.clone())?;
        backend.upsert_sprint(sprint.clone())?;
        backend.modify_graph(Box::new(move |graph| graph.set_block(other, card_id)))?;
        Ok(())
    }));
    result.expect("with_transaction should commit a valid batch");

    let fresh = open(&path).await;
    assert!(
        fresh.get_board(board_id).unwrap().is_some(),
        "board present"
    );
    assert!(
        fresh.get_column(column_id).unwrap().is_some(),
        "column present"
    );
    assert!(fresh.get_card(card_id).unwrap().is_some(), "card present");
    assert!(
        fresh.get_sprint(sprint_id).unwrap().is_some(),
        "sprint present"
    );
    assert!(
        fresh.get_graph().unwrap().contains(other, card_id),
        "edge present"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_with_transaction_rolls_back_full_graph_via_db_not_snapshot_restore() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let backend = open(&path).await;

    let board = Board::new("B", None::<String>);
    let board_id = board.id;
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(board.id, column.id, "Card", 0);
    let card_id = card.id;
    let sprint = Sprint::new(board.id, 1, None, Some("SP"));
    let sprint_id = sprint.id;
    backend.upsert_board(board.clone()).unwrap();
    backend.upsert_column(column.clone()).unwrap();
    backend.upsert_card(card.clone()).unwrap();
    backend.upsert_sprint(sprint.clone()).unwrap();
    let other = uuid::Uuid::new_v4();
    backend
        .modify_graph(Box::new(move |graph| graph.set_block(other, card_id)))
        .unwrap();

    let second_card_id = uuid::Uuid::new_v4();
    let second_sprint_id = uuid::Uuid::new_v4();

    let result: KanbanResult<()> = backend.with_transaction(Box::new(|| {
        let mut new_card = Card::new(board.id, column.id, "Second", 1);
        new_card.id = second_card_id;
        backend.upsert_card(new_card)?;
        let mut new_sprint = Sprint::new(board.id, 2, None, Some("SP"));
        new_sprint.id = second_sprint_id;
        backend.upsert_sprint(new_sprint)?;
        backend.insert_archived_card(kanban_domain::ArchivedCard::new(card_id, board_id))?;
        Err(kanban_domain::KanbanError::Internal("boom".into()))
    }));
    assert!(result.is_err());

    let fresh = open(&path).await;
    assert!(
        fresh.get_board(board_id).unwrap().is_some(),
        "original board intact"
    );
    assert!(
        fresh.get_card(card_id).unwrap().is_some(),
        "original card intact"
    );
    assert!(
        fresh.get_sprint(sprint_id).unwrap().is_some(),
        "original sprint intact"
    );
    assert!(
        fresh.get_card(second_card_id).unwrap().is_none(),
        "second card must not have landed"
    );
    assert!(
        fresh.get_sprint(second_sprint_id).unwrap().is_none(),
        "second sprint must not have landed"
    );
    assert!(
        fresh.get_archived_card(card_id).unwrap().is_none(),
        "archive marker must not have landed"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_with_transaction_delete_path_rolls_back() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let backend = open(&path).await;

    let board = Board::new("B", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(board.id, column.id, "Card", 0);
    let card_id = card.id;
    backend.upsert_board(board).unwrap();
    backend.upsert_column(column).unwrap();
    backend.upsert_card(card).unwrap();

    let result: KanbanResult<()> = backend.with_transaction(Box::new(|| {
        backend.delete_card(card_id)?;
        Err(kanban_domain::KanbanError::Internal("boom".into()))
    }));
    assert!(result.is_err());

    let fresh = open(&path).await;
    assert!(
        fresh.get_card(card_id).unwrap().is_some(),
        "card deletion must roll back on failure"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_with_transaction_propagates_inner_error_and_leaves_pre_state_on_disk() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let backend = open(&path).await;

    let board = Board::new("B", None::<String>);
    let board_id = board.id;
    backend.upsert_board(board).unwrap();

    let result: KanbanResult<()> = backend.with_transaction(Box::new(|| {
        Err(kanban_domain::KanbanError::Internal(
            "no writes attempted".into(),
        ))
    }));
    let err = result.expect_err("closure error must propagate");
    assert!(err.to_string().contains("no writes attempted"));

    let fresh = open(&path).await;
    assert_eq!(fresh.list_boards().unwrap().len(), 1);
    assert!(fresh.get_board(board_id).unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_capture_inverse_reads_uncommitted_sibling_write() {
    use kanban_domain::commands::{CardCommand, Command, CreateCard, MoveCard};
    use kanban_domain::CreateCardOptions;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let backend: std::sync::Arc<dyn KanbanBackend> = std::sync::Arc::new(open(&path).await);
    let mut ctx = kanban_service::KanbanContext::open(backend, kanban_core::AppConfig::default())
        .await
        .unwrap();

    use kanban_domain::KanbanOperations;
    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col_a = ctx.create_column(board.id, "A".into(), None).unwrap();
    let col_b = ctx.create_column(board.id, "B".into(), None).unwrap();

    let card_id = uuid::Uuid::new_v4();
    ctx.execute(vec![Command::Card(CardCommand::Create(CreateCard {
        id: card_id,
        card_number: 1,
        board_id: board.id,
        column_id: col_a.id,
        title: "Card".into(),
        position: 0,
        options: CreateCardOptions::default(),
        timestamp: chrono::Utc::now(),
        default_card_prefix: "task".to_string(),
    }))])
    .unwrap();

    // Second batch: MoveCard's WIP check / capture_inverse reads state the
    // first command in this same batch just wrote. Reuse Create+Move in one
    // batch so capture_inverse for the second command reads a write made by
    // the first command inside the same with_transaction call.
    let card_id2 = uuid::Uuid::new_v4();
    let result = ctx.execute(vec![
        Command::Card(CardCommand::Create(CreateCard {
            id: card_id2,
            card_number: 2,
            board_id: board.id,
            column_id: col_a.id,
            title: "Card2".into(),
            position: 1,
            options: CreateCardOptions::default(),
            timestamp: chrono::Utc::now(),
            default_card_prefix: "task".to_string(),
        })),
        Command::Card(CardCommand::Move(MoveCard {
            card_id: card_id2,
            new_column_id: col_b.id,
            new_position: 0,
        })),
    ]);
    result.expect("batch must succeed: MoveCard's capture_inverse must see the sibling create");

    assert_eq!(
        ctx.data_store()
            .list_cards_by_column(col_b.id)
            .unwrap()
            .len(),
        1
    );
}
