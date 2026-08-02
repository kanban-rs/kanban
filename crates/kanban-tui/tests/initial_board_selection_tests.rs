mod helpers;

use kanban_domain::{Board, Card, Column, KanbanError, KanbanResult, Snapshot};
use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};

#[tokio::test]
async fn test_load_initial_state_with_boards_selects_first_board() -> KanbanResult<()> {
    let dir = tempfile::tempdir()?;
    let path = helpers::create_test_json_file(dir.path(), "test.json", &["Alpha", "Beta"]).await;
    let (mut app, _rx) = kanban_tui::App::new(Some(path)).await?;
    app.load_initial_state().await;

    assert_eq!(
        app.selection.board.get(),
        Some(0),
        "first board should be selected after startup"
    );
    Ok(())
}

#[tokio::test]
async fn test_load_initial_state_with_boards_refreshes_card_view() -> KanbanResult<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("with_cards.json");
    let path_str = path.to_str().unwrap().to_string();

    let mut board = Board::new("Alpha", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(&mut board, column.id, "Task One", 0);
    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board],
        columns: vec![column],
        cards: vec![card],
        archived_cards: vec![],
        sprints: vec![],
        graph: Default::default(),
    };
    let store = kanban_persistence_json::JsonFileStore::new(&path_str);
    let store_snapshot = StoreSnapshot {
        data: serde_json::to_vec(&snapshot)
            .map_err(|e| KanbanError::serialization(e.to_string()))?,
        metadata: PersistenceMetadata::new(store.instance_id()),
    };
    store.save(store_snapshot).await?;

    let (mut app, _rx) = kanban_tui::App::new(Some(path_str)).await?;
    app.load_initial_state().await;
    app.prepare_frame();

    let task_list = app.view.strategy.get_active_task_list();
    assert!(
        task_list.is_some_and(|l| !l.is_empty()),
        "card view should be populated after startup without user interaction"
    );
    Ok(())
}

#[tokio::test]
async fn test_load_initial_state_with_no_boards_leaves_selection_none() -> KanbanResult<()> {
    let dir = tempfile::tempdir()?;
    let path = helpers::create_test_json_file(dir.path(), "empty.json", &[]).await;
    let (mut app, _rx) = kanban_tui::App::new(Some(path)).await?;
    app.load_initial_state().await;

    assert_eq!(
        app.selection.board.get(),
        None,
        "selection should remain None when there are no boards"
    );
    Ok(())
}

#[tokio::test]
async fn test_load_initial_state_with_no_file_leaves_selection_none() -> KanbanResult<()> {
    let mut app = kanban_tui::App::test_default();
    app.load_initial_state().await;

    assert_eq!(
        app.selection.board.get(),
        None,
        "selection should remain None when no file is provided"
    );
    Ok(())
}

#[tokio::test]
async fn test_load_initial_state_does_not_clobber_existing_board_selection() -> KanbanResult<()> {
    let dir = tempfile::tempdir()?;
    let path = helpers::create_test_json_file(dir.path(), "test.json", &["Alpha", "Beta"]).await;
    let (mut app, _rx) = kanban_tui::App::new(Some(path)).await?;
    app.selection.board.set(Some(1));
    app.load_initial_state().await;

    assert_eq!(
        app.selection.board.get(),
        Some(1),
        "pre-existing board selection should not be overwritten by load_initial_state"
    );
    Ok(())
}
