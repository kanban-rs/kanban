use kanban_backend_memory::InMemoryStore;
use kanban_domain::commands::{
    BoardCommand, CardCommand, Command, CompactColumnPositions, CreateBoard, ImportEntities,
    UpdateBoard,
};
use kanban_domain::{BoardUpdate, CardUpdate, KanbanOperations, KanbanResult, Snapshot};
use kanban_service::KanbanContext;
use std::sync::Arc;

async fn open_context(
    locator: &str,
    config: kanban_core::AppConfig,
) -> KanbanResult<KanbanContext> {
    let mut config = config;
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    stores.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    let sm = kanban_service::StoreManager::new(stores, backends);
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    KanbanContext::open(backend, config).await
}

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(
        Arc::new(InMemoryStore::new()),
        kanban_core::AppConfig::default(),
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_snapshot_roundtrip_preserves_all_fields() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.create_board("B".into(), None)?;
    let board_id = ctx.boards()?[0].id;
    ctx.create_column(board_id, "C".into(), None)?;
    let col_id = ctx.columns()?[0].id;
    ctx.create_card(board_id, col_id, "Card".into(), Default::default())?;

    let snap = ctx.snapshot()?;
    ctx.apply_snapshot(Snapshot::new())?;
    assert!(ctx.boards()?.is_empty());

    ctx.apply_snapshot(snap.clone())?;
    assert_eq!(ctx.snapshot()?, snap);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_execute_enables_undo() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }));
    ctx.execute(vec![cmd])?;
    assert!(ctx.can_undo());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_restores_previous_state() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }));
    ctx.execute(vec![cmd])?;
    assert_eq!(ctx.boards()?.len(), 1);

    assert!(ctx.undo()?);
    assert!(ctx.boards()?.is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redo_restores_undone_state() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }));
    ctx.execute(vec![cmd])?;
    ctx.undo()?;
    assert!(ctx.boards()?.is_empty());

    assert!(ctx.redo()?);
    assert_eq!(ctx.boards()?.len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_on_empty_returns_false() {
    let ctx = make_ctx().await;
    assert!(!ctx.can_undo());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_new_action_after_undo_clears_redo() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "A".into(),
        card_prefix: None,
        position: 0,
    }))])?;
    ctx.undo()?;
    assert!(ctx.can_redo());

    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }))])?;
    assert!(!ctx.can_redo());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reload_clears_undo_history() -> KanbanResult<()> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.json");
    let mut ctx = open_context(path.to_str().unwrap(), kanban_core::AppConfig::default()).await?;

    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }))])?;
    ctx.save().await?;

    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B2".into(),
        card_prefix: None,
        position: 0,
    }))])?;
    assert!(ctx.can_undo());

    ctx.reload().await?;
    assert!(!ctx.can_undo(), "reload must reset undo history");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_dirty_flag_lifecycle() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    assert!(!ctx.is_dirty());

    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }))])?;
    assert!(ctx.is_dirty());

    ctx.mark_clean();
    assert!(!ctx.is_dirty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_operations_create_board_captures_history() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.create_board("B".into(), None)?;
    assert!(
        ctx.can_undo(),
        "create_board via KanbanOperations should capture history"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_operations_update_card_is_undoable() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "Original".into(), Default::default())?;

    ctx.update_card(
        card.id,
        CardUpdate {
            title: Some("Updated".into()),
            ..Default::default()
        },
    )?;
    assert_eq!(ctx.get_card(card.id)?.unwrap().title, "Updated");

    ctx.undo()?;
    assert_eq!(ctx.get_card(card.id)?.unwrap().title, "Original");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_cards_creates_single_undo_entry() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let c1 = ctx.create_card(board.id, col.id, "Card 1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, col.id, "Card 2".into(), Default::default())?;
    let c3 = ctx.create_card(board.id, col.id, "Card 3".into(), Default::default())?;

    ctx.clear_history()?;

    ctx.archive_cards(vec![c1.id, c2.id, c3.id])?;
    assert_eq!(ctx.cards()?.len(), 0);
    assert_eq!(ctx.archived_cards()?.len(), 3);

    assert!(ctx.undo()?);
    assert_eq!(ctx.cards()?.len(), 3);
    assert_eq!(ctx.archived_cards()?.len(), 0);
    assert!(!ctx.can_undo());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_undo_restores_board_counters() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    ctx.clear_history()?;

    let _sprint = ctx.create_sprint(board.id, None, None)?;
    assert_eq!(ctx.sprints()?.len(), 1);

    ctx.undo()?;
    assert_eq!(ctx.sprints()?.len(), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_board_clears_history() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("Export".into(), None)?;
    let _col = ctx.create_column(board.id, "C".into(), None)?;
    let json = ctx.export_board(Some(board.id))?;

    let mut ctx2 = make_ctx().await;
    ctx2.import_board(&json)?;
    assert_eq!(ctx2.boards()?.len(), 1);
    assert_eq!(ctx2.boards()?[0].name, "Export");

    assert!(!ctx2.can_undo(), "import should clear history");
    assert_eq!(ctx2.undo_depth(), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_board_includes_archived_cards() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("Export".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "Card".into(), Default::default())?;
    ctx.archive_card(card.id)?;
    assert_eq!(ctx.archived_cards()?.len(), 1);

    let json = ctx.export_board(None)?;

    let mut ctx2 = make_ctx().await;
    ctx2.import_board(&json)?;
    assert_eq!(
        ctx2.archived_cards()?.len(),
        1,
        "imported archived cards should appear"
    );
    assert!(!ctx2.can_undo(), "import should clear history");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_conflict_flag_lifecycle() {
    let mut ctx = make_ctx().await;
    assert!(!ctx.has_conflict());
    ctx.set_conflict();
    assert!(ctx.has_conflict());
    ctx.clear_conflict();
    assert!(!ctx.has_conflict());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reload_resets_undo_history() -> KanbanResult<()> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.json");
    let mut ctx = open_context(path.to_str().unwrap(), kanban_core::AppConfig::default()).await?;

    ctx.create_board("B".into(), None)?;
    ctx.save().await?;

    ctx.create_board("B2".into(), None)?;
    assert!(ctx.can_undo());

    ctx.reload().await?;
    assert!(!ctx.can_undo(), "reload must reset undo history");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_pops_before_pushing_to_redo() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }))])?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B2".into(),
        card_prefix: None,
        position: 0,
    }))])?;

    assert!(ctx.undo()?);
    assert_eq!(ctx.redo_depth(), 1);
    assert_eq!(ctx.undo_depth(), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redo_pops_before_pushing_to_undo() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }))])?;
    ctx.undo()?;

    assert!(ctx.redo()?);
    assert_eq!(ctx.undo_depth(), 1);
    assert_eq!(ctx.redo_depth(), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_on_empty_history_does_not_corrupt() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    assert!(!ctx.undo()?);
    assert_eq!(ctx.undo_depth(), 0);
    assert_eq!(ctx.redo_depth(), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_execute_batch_partial_failure_rollback_leaves_clean_undo_stack() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.create_board("B1".into(), None)?;
    ctx.clear_history()?;

    let batch: Vec<Command> = vec![
        Command::Board(BoardCommand::Create(CreateBoard {
            id: uuid::Uuid::new_v4(),
            name: "B2".into(),
            card_prefix: None,
            position: 0,
        })),
        Command::Board(BoardCommand::Update(UpdateBoard {
            board_id: uuid::Uuid::nil(),
            updates: BoardUpdate {
                name: Some("Nonexistent".into()),
                ..Default::default()
            },
        })),
    ];
    assert!(ctx.execute(batch).is_err());
    assert_eq!(ctx.boards()?.len(), 1);
    assert!(!ctx.can_undo(), "undo stack should be empty after rollback");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_cards_returns_actual_moved_count() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col_a = ctx.create_column(board.id, "A".into(), None)?;
    let col_b = ctx.create_column(board.id, "B".into(), None)?;
    let c1 = ctx.create_card(board.id, col_a.id, "Card 1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, col_a.id, "Card 2".into(), Default::default())?;
    let c3 = ctx.create_card(board.id, col_b.id, "Card 3".into(), Default::default())?;

    let count = ctx.move_cards(vec![c1.id, c2.id, c3.id], col_b.id)?;
    assert_eq!(count, 2, "only 2 cards should have actually moved");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_cards_to_sprint_returns_actual_assigned_count() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let c1 = ctx.create_card(board.id, col.id, "Card 1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, col.id, "Card 2".into(), Default::default())?;
    let sprint = ctx.create_sprint(board.id, None, None)?;

    ctx.assign_card_to_sprint(c1.id, sprint.id)?;

    let count = ctx.assign_cards_to_sprint(vec![c1.id, c2.id], sprint.id)?;
    assert_eq!(count, 1, "only 1 card should have been newly assigned");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_null_store_context_loads_empty() -> KanbanResult<()> {
    let ctx = make_ctx().await;
    assert!(ctx.boards()?.is_empty());
    assert!(ctx.columns()?.is_empty());
    assert!(ctx.cards()?.is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_cards_single_undo_entry() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col1 = ctx.create_column(board.id, "From".into(), None)?;
    let col2 = ctx.create_column(board.id, "To".into(), None)?;
    let c1 = ctx.create_card(board.id, col1.id, "Card 1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, col1.id, "Card 2".into(), Default::default())?;
    ctx.clear_history()?;

    ctx.move_cards(vec![c1.id, c2.id], col2.id)?;
    assert!(ctx.cards()?.iter().all(|c| c.column_id == col2.id));

    assert!(ctx.undo()?);
    assert!(ctx.cards()?.iter().all(|c| c.column_id == col1.id));
    assert!(!ctx.can_undo());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_cards_to_sprint_renamed_trait_method() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "Card".into(), Default::default())?;
    let sprint = ctx.create_sprint(board.id, None, None)?;

    let count = ctx.assign_cards_to_sprint(vec![card.id], sprint.id)?;
    assert_eq!(count, 1);
    assert_eq!(ctx.get_card(card.id)?.unwrap().sprint_id, Some(sprint.id));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_cards_to_sprint_creates_single_undo_entry() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let c1 = ctx.create_card(board.id, col.id, "Card 1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, col.id, "Card 2".into(), Default::default())?;
    let c3 = ctx.create_card(board.id, col.id, "Card 3".into(), Default::default())?;
    let sprint = ctx.create_sprint(board.id, None, None)?;

    ctx.clear_history()?;

    ctx.assign_cards_to_sprint(vec![c1.id, c2.id, c3.id], sprint.id)?;
    assert_eq!(ctx.get_card(c1.id)?.unwrap().sprint_id, Some(sprint.id));
    assert_eq!(ctx.get_card(c2.id)?.unwrap().sprint_id, Some(sprint.id));
    assert_eq!(ctx.get_card(c3.id)?.unwrap().sprint_id, Some(sprint.id));

    assert!(ctx.undo()?);
    assert_eq!(ctx.get_card(c1.id)?.unwrap().sprint_id, None);
    assert_eq!(ctx.get_card(c2.id)?.unwrap().sprint_id, None);
    assert_eq!(ctx.get_card(c3.id)?.unwrap().sprint_id, None);
    assert!(!ctx.can_undo());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_execute_batch_partial_failure_rolls_back() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.create_board("B1".into(), None)?;
    ctx.clear_history()?;

    let batch: Vec<Command> = vec![
        Command::Board(BoardCommand::Create(CreateBoard {
            id: uuid::Uuid::new_v4(),
            name: "B2".into(),
            card_prefix: None,
            position: 0,
        })),
        Command::Board(BoardCommand::Update(UpdateBoard {
            board_id: uuid::Uuid::nil(),
            updates: BoardUpdate {
                name: Some("Nonexistent".into()),
                ..Default::default()
            },
        })),
    ];
    let result = ctx.execute(batch);
    assert!(result.is_err());

    assert_eq!(ctx.boards()?.len(), 1);
    assert_eq!(ctx.boards()?[0].name, "B1");
    assert!(!ctx.can_undo());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_execute_batch_success_is_undoable() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.create_board("B1".into(), None)?;
    ctx.clear_history()?;

    let batch: Vec<Command> = vec![
        Command::Board(BoardCommand::Create(CreateBoard {
            id: uuid::Uuid::new_v4(),
            name: "B2".into(),
            card_prefix: None,
            position: 0,
        })),
        Command::Board(BoardCommand::Create(CreateBoard {
            id: uuid::Uuid::new_v4(),
            name: "B3".into(),
            card_prefix: None,
            position: 0,
        })),
    ];
    ctx.execute(batch)?;
    assert_eq!(ctx.boards()?.len(), 3);

    assert!(ctx.undo()?);
    assert_eq!(ctx.boards()?.len(), 1);
    assert!(!ctx.can_undo());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_entities_is_undoable() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.create_board("B1".into(), None)?;
    ctx.clear_history()?;

    let b2 = kanban_domain::Board::new("B2", None::<String>);
    let col = kanban_domain::Column::new(b2.id, "Todo", 0);
    let cmd = Command::Board(BoardCommand::Import(ImportEntities {
        boards: vec![b2],
        columns: vec![col],
        cards: vec![],
        archived_cards: vec![],
        archived_boards: vec![],
        sprints: vec![],
        graph: None,
    }));
    ctx.execute(vec![cmd])?;
    assert_eq!(ctx.boards()?.len(), 2);

    assert!(ctx.undo()?);
    assert_eq!(ctx.boards()?.len(), 1);
    assert_eq!(ctx.boards()?[0].name, "B1");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_card_not_found_returns_error() {
    let mut ctx = make_ctx().await;
    let result = ctx.archive_card(uuid::Uuid::new_v4());
    assert!(result.is_err());
    assert!(result.unwrap_err().is_not_found());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_cards_detailed_all_valid_creates_single_undo_entry() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let c1 = ctx.create_card(board.id, col.id, "Card 1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, col.id, "Card 2".into(), Default::default())?;
    ctx.clear_history()?;
    ctx.mark_clean();

    let result = ctx.archive_cards_detailed(vec![c1.id, c2.id]);
    assert_eq!(result.succeeded.len(), 2);
    assert!(result.failed.is_empty());
    assert!(ctx.is_dirty());
    assert_eq!(ctx.archived_cards()?.len(), 2);

    assert!(ctx.undo()?);
    assert_eq!(ctx.cards()?.len(), 2);
    assert_eq!(ctx.archived_cards()?.len(), 0);
    assert!(
        !ctx.can_undo(),
        "should be a single undo entry for the whole batch"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_cards_detailed_all_fail_does_not_set_dirty() {
    let mut ctx = make_ctx().await;
    ctx.mark_clean();
    let result = ctx.archive_cards_detailed(vec![uuid::Uuid::new_v4(), uuid::Uuid::new_v4()]);
    assert!(result.succeeded.is_empty());
    assert_eq!(result.failed.len(), 2);
    assert!(
        !ctx.is_dirty(),
        "dirty flag should not be set when all ops fail"
    );
    assert!(
        !ctx.can_undo(),
        "undo stack should be clean when all ops fail"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_detailed_archive_partial_success_is_undoable() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "Card".into(), Default::default())?;
    ctx.clear_history()?;
    ctx.mark_clean();

    let result = ctx.archive_cards_detailed(vec![card.id, uuid::Uuid::new_v4()]);
    assert_eq!(result.succeeded.len(), 1);
    assert_eq!(result.failed.len(), 1);
    assert!(ctx.is_dirty());
    assert_eq!(ctx.archived_cards()?.len(), 1);

    assert!(ctx.undo()?);
    assert_eq!(ctx.cards()?.len(), 1);
    assert_eq!(ctx.archived_cards()?.len(), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_cards_detailed_all_valid_creates_single_undo_entry() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col_a = ctx.create_column(board.id, "A".into(), None)?;
    let col_b = ctx.create_column(board.id, "B".into(), None)?;
    let c1 = ctx.create_card(board.id, col_a.id, "Card 1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, col_a.id, "Card 2".into(), Default::default())?;
    ctx.clear_history()?;
    ctx.mark_clean();

    let result = ctx.move_cards_detailed(vec![c1.id, c2.id], col_b.id);
    assert_eq!(result.succeeded.len(), 2);
    assert!(result.failed.is_empty());
    assert!(ctx.is_dirty());
    assert!(ctx.cards()?.iter().all(|c| c.column_id == col_b.id));

    assert!(ctx.undo()?);
    assert!(ctx.cards()?.iter().all(|c| c.column_id == col_a.id));
    assert!(
        !ctx.can_undo(),
        "should be a single undo entry for the whole batch"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_cards_detailed_all_fail_does_not_set_dirty() {
    let mut ctx = make_ctx().await;
    ctx.mark_clean();
    let result = ctx.move_cards_detailed(
        vec![uuid::Uuid::new_v4(), uuid::Uuid::new_v4()],
        uuid::Uuid::new_v4(),
    );
    assert!(result.succeeded.is_empty());
    assert_eq!(result.failed.len(), 2);
    assert!(
        !ctx.is_dirty(),
        "dirty flag should not be set when all ops fail"
    );
    assert!(
        !ctx.can_undo(),
        "undo stack should be clean when all ops fail"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_cards_detailed_partial_success_is_undoable() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col_a = ctx.create_column(board.id, "A".into(), None)?;
    let col_b = ctx.create_column(board.id, "B".into(), None)?;
    let card = ctx.create_card(board.id, col_a.id, "Card".into(), Default::default())?;
    ctx.clear_history()?;
    ctx.mark_clean();

    let result = ctx.move_cards_detailed(vec![card.id, uuid::Uuid::new_v4()], col_b.id);
    assert_eq!(result.succeeded.len(), 1);
    assert_eq!(result.failed.len(), 1);
    assert!(ctx.is_dirty());
    assert_eq!(ctx.cards()?.first().unwrap().column_id, col_b.id);

    assert!(ctx.undo()?);
    assert_eq!(ctx.cards()?.first().unwrap().column_id, col_a.id);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_cards_to_sprint_detailed_all_valid_creates_single_undo_entry(
) -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let c1 = ctx.create_card(board.id, col.id, "Card 1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, col.id, "Card 2".into(), Default::default())?;
    let sprint = ctx.create_sprint(board.id, None, None)?;
    ctx.clear_history()?;
    ctx.mark_clean();

    let result = ctx.assign_cards_to_sprint_detailed(vec![c1.id, c2.id], sprint.id);
    assert_eq!(result.succeeded.len(), 2);
    assert!(result.failed.is_empty());
    assert!(ctx.is_dirty());
    assert_eq!(ctx.get_card(c1.id)?.unwrap().sprint_id, Some(sprint.id));
    assert_eq!(ctx.get_card(c2.id)?.unwrap().sprint_id, Some(sprint.id));

    assert!(ctx.undo()?);
    assert_eq!(ctx.get_card(c1.id)?.unwrap().sprint_id, None);
    assert_eq!(ctx.get_card(c2.id)?.unwrap().sprint_id, None);
    assert!(
        !ctx.can_undo(),
        "should be a single undo entry for the whole batch"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_cards_to_sprint_detailed_all_fail_does_not_set_dirty() {
    let mut ctx = make_ctx().await;
    ctx.mark_clean();
    let result = ctx.assign_cards_to_sprint_detailed(
        vec![uuid::Uuid::new_v4(), uuid::Uuid::new_v4()],
        uuid::Uuid::new_v4(),
    );
    assert!(result.succeeded.is_empty());
    assert_eq!(result.failed.len(), 2);
    assert!(
        !ctx.is_dirty(),
        "dirty flag should not be set when all ops fail"
    );
    assert!(
        !ctx.can_undo(),
        "undo stack should be clean when all ops fail"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_cards_to_sprint_detailed_partial_success_is_undoable() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "Card".into(), Default::default())?;
    let sprint = ctx.create_sprint(board.id, None, None)?;
    ctx.clear_history()?;
    ctx.mark_clean();

    let result =
        ctx.assign_cards_to_sprint_detailed(vec![card.id, uuid::Uuid::new_v4()], sprint.id);
    assert_eq!(result.succeeded.len(), 1);
    assert_eq!(result.failed.len(), 1);
    assert!(ctx.is_dirty());
    assert_eq!(ctx.get_card(card.id)?.unwrap().sprint_id, Some(sprint.id));

    assert!(ctx.undo()?);
    assert_eq!(ctx.get_card(card.id)?.unwrap().sprint_id, None);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_compact_column_positions_is_undoable() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), Some("TST".into()))?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let c1 = ctx.create_card(board.id, col.id, "Card1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, col.id, "Card2".into(), Default::default())?;

    ctx.update_card(
        c1.id,
        CardUpdate {
            position: Some(0),
            ..Default::default()
        },
    )?;
    ctx.update_card(
        c2.id,
        CardUpdate {
            position: Some(5),
            ..Default::default()
        },
    )?;
    ctx.clear_history()?;

    let cmd = Command::Card(CardCommand::CompactPositions(CompactColumnPositions {
        column_id: col.id,
    }));
    ctx.execute(vec![cmd])?;
    assert_eq!(
        ctx.cards()?
            .iter()
            .find(|c| c.id == c1.id)
            .unwrap()
            .position,
        0
    );
    assert_eq!(
        ctx.cards()?
            .iter()
            .find(|c| c.id == c2.id)
            .unwrap()
            .position,
        1
    );

    assert!(ctx.undo()?);
    assert_eq!(
        ctx.cards()?
            .iter()
            .find(|c| c.id == c1.id)
            .unwrap()
            .position,
        0
    );
    assert_eq!(
        ctx.cards()?
            .iter()
            .find(|c| c.id == c2.id)
            .unwrap()
            .position,
        5
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_cursor_tracks_command_count() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    assert_eq!(ctx.undo_depth(), 0);
    assert_eq!(ctx.redo_depth(), 0);

    ctx.create_board("B1".into(), None)?;
    assert_eq!(ctx.undo_depth(), 1);
    assert_eq!(ctx.redo_depth(), 0);

    ctx.create_board("B2".into(), None)?;
    assert_eq!(ctx.undo_depth(), 2);
    assert_eq!(ctx.redo_depth(), 0);

    ctx.undo()?;
    assert_eq!(ctx.undo_depth(), 1);
    assert_eq!(ctx.redo_depth(), 1);

    ctx.undo()?;
    assert_eq!(ctx.undo_depth(), 0);
    assert_eq!(ctx.redo_depth(), 2);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redo_uses_stored_commands() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let board_id = board.id;

    ctx.undo()?;
    assert!(ctx.boards()?.is_empty());

    ctx.redo()?;
    let boards = ctx.boards()?;
    assert_eq!(boards.len(), 1);
    assert_eq!(
        boards[0].id, board_id,
        "redo must replay the original command, producing the same id"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_clear_history_resets_baseline() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.create_board("B".into(), None)?;
    assert!(ctx.can_undo());

    ctx.clear_history()?;
    assert!(
        !ctx.can_undo(),
        "clear_history makes current state the new baseline"
    );
    assert!(!ctx.can_redo(), "clear_history drops all redo entries too");
    assert_eq!(
        ctx.boards()?.len(),
        1,
        "clear_history preserves current state"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_multi_step_undo_redo_full_cycle() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;

    ctx.create_board("B1".into(), None)?;
    ctx.create_board("B2".into(), None)?;
    ctx.create_board("B3".into(), None)?;
    assert_eq!(ctx.boards()?.len(), 3);
    assert_eq!(ctx.undo_depth(), 3);

    assert!(ctx.undo()?);
    assert_eq!(ctx.boards()?.len(), 2);
    assert!(ctx.undo()?);
    assert_eq!(ctx.boards()?.len(), 1);
    assert!(ctx.undo()?);
    assert_eq!(ctx.boards()?.len(), 0);
    assert!(!ctx.can_undo());
    assert_eq!(ctx.redo_depth(), 3);

    assert!(ctx.redo()?);
    assert_eq!(ctx.boards()?.len(), 1);
    assert!(ctx.redo()?);
    assert_eq!(ctx.boards()?.len(), 2);
    assert!(ctx.redo()?);
    assert_eq!(ctx.boards()?.len(), 3);
    assert!(!ctx.can_redo());
    assert_eq!(ctx.undo_depth(), 3);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redo_past_end_returns_false() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    assert!(!ctx.redo()?);

    ctx.create_board("B".into(), None)?;
    assert!(!ctx.redo()?);

    ctx.undo()?;
    assert!(ctx.redo()?);
    assert!(!ctx.redo()?);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_history_is_not_preserved_across_sessions() -> KanbanResult<()> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.json");
    let mut ctx = open_context(path.to_str().unwrap(), kanban_core::AppConfig::default()).await?;

    ctx.create_board("B1".into(), None)?;
    ctx.create_board("B2".into(), None)?;
    assert_eq!(ctx.boards()?.len(), 2);
    assert_eq!(ctx.undo_depth(), 2);

    ctx.save().await?;

    let ctx2 = open_context(path.to_str().unwrap(), kanban_core::AppConfig::default()).await?;
    assert_eq!(
        ctx2.boards()?.len(),
        2,
        "board data survives the session boundary"
    );
    assert!(
        !ctx2.can_undo(),
        "undo is in-session only; history must not carry over across sessions"
    );
    assert_eq!(ctx2.undo_depth(), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_extends_past_old_200_step_cap() -> KanbanResult<()> {
    // KAN-191 dropped the MAX_UNDO_DEPTH=200 cap. With pure command-replay
    // every undo step is `Vec<Command>` (a few hundred bytes), so the cap
    // that protected RAM from 200 full Snapshot clones is no longer needed.
    let mut ctx = make_ctx().await;

    let total = 250usize;
    for i in 0..total {
        ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
            id: uuid::Uuid::new_v4(),
            name: format!("B{i}"),
            card_prefix: None,
            position: i as i32,
        }))])?;
    }

    assert_eq!(
        ctx.undo_depth(),
        total,
        "undo depth must equal total commands executed (no cap)"
    );
    assert_eq!(ctx.boards()?.len(), total);

    for _ in 0..total {
        assert!(ctx.undo()?);
    }
    assert!(
        !ctx.can_undo(),
        "after undoing every step, can_undo is false"
    );
    assert_eq!(
        ctx.boards()?.len(),
        0,
        "after undoing every step, state is the initial baseline"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_can_redo_uses_cached_count() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }))])?;
    assert!(!ctx.can_redo());

    ctx.undo()?;
    assert!(ctx.can_redo());
    assert_eq!(ctx.redo_depth(), 1);

    ctx.redo()?;
    assert!(!ctx.can_redo());
    assert_eq!(ctx.redo_depth(), 0);
    Ok(())
}
