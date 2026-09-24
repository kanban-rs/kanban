mod helpers;

use helpers::CountingBackend;
use kanban_domain::{ArchivedCard, Board, Card, Column, Snapshot};
use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};
use kanban_tui::app::mode::AppMode;
use kanban_tui::App;

async fn json_app_with_archived_card() -> (App, uuid::Uuid) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archived_card.json");
    let path_str = path.to_str().unwrap().to_string();

    let store = kanban_persistence_json::JsonFileStore::new(&path_str);
    let board = Board::new("Board", None::<String>);
    let board_id = board.id;
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(board.id, column.id, "Archived task", 0);
    let card_id = card.id;
    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board],
        columns: vec![column],
        cards: vec![card],
        archived_cards: vec![ArchivedCard::new(card_id, board_id)],
        sprints: vec![],
        graph: Default::default(),
        prefixes: Vec::new(),
    };
    let store_snapshot = StoreSnapshot {
        data: serde_json::to_vec(&snapshot).unwrap(),
        metadata: PersistenceMetadata::new(store.instance_id()),
    };
    store.save(store_snapshot).await.unwrap();

    let (mut app, _rx) = App::new(Some(path_str)).await.unwrap();
    app.load_initial_state().await;
    (app, card_id)
}

async fn json_app_with_archived_board() -> (App, uuid::Uuid) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archived_board.json");
    let path_str = path.to_str().unwrap().to_string();

    let store = kanban_persistence_json::JsonFileStore::new(&path_str);
    let board = Board::new("Archived Board", None::<String>);
    let board_id = board.id;
    let snapshot = Snapshot {
        archived_boards: vec![kanban_domain::Archived::now(board_id)],
        boards: vec![board],
        columns: vec![],
        cards: vec![],
        archived_cards: vec![],
        sprints: vec![],
        graph: Default::default(),
        prefixes: Vec::new(),
    };
    let store_snapshot = StoreSnapshot {
        data: serde_json::to_vec(&snapshot).unwrap(),
        metadata: PersistenceMetadata::new(store.instance_id()),
    };
    store.save(store_snapshot).await.unwrap();

    let (mut app, _rx) = App::new(Some(path_str)).await.unwrap();
    app.load_initial_state().await;
    (app, board_id)
}

#[tokio::test]
async fn test_entering_the_archived_cards_view_populates_the_bodies_without_a_snapshot() {
    let (mut app, card_id) = json_app_with_archived_card().await;

    let (backend, _reads, ops) = CountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);
    ops.lock().unwrap().clear();

    app.handle_toggle_archived_cards_view();

    let recorded = ops.lock().unwrap().clone();
    assert!(
        !recorded.iter().any(|op| op.method == "snapshot"),
        "expected no snapshot op, got {recorded:?}"
    );

    let displayed = app.displayed_cards();
    assert!(displayed.is_loaded(), "expected Loaded, got {displayed:?}");
    assert!(displayed.loaded().unwrap().iter().any(|c| c.id == card_id));
}

#[tokio::test]
async fn test_entering_the_archived_boards_view_populates_the_heads_without_a_snapshot() {
    let (mut app, board_id) = json_app_with_archived_board().await;

    let (backend, _reads, ops) = CountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);
    ops.lock().unwrap().clear();

    app.handle_toggle_archived_boards_view();

    let recorded = ops.lock().unwrap().clone();
    assert!(
        !recorded.iter().any(|op| op.method == "snapshot"),
        "expected no snapshot op, got {recorded:?}"
    );
    assert!(matches!(app.mode, AppMode::ArchivedBoardsView));

    let displayed = app.controller.archived_boards_view();
    assert!(displayed.is_loaded(), "expected Loaded, got {displayed:?}");
    assert!(displayed.loaded().unwrap().iter().any(|b| b.id == board_id));
}

#[tokio::test]
async fn test_reload_model_never_calls_snapshot() {
    let (mut app, _card_id) = json_app_with_archived_card().await;

    let (backend, _reads, ops) = CountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);
    ops.lock().unwrap().clear();

    app.reload_model();

    let recorded = ops.lock().unwrap().clone();
    assert!(
        !recorded.iter().any(|op| op.method == "snapshot"),
        "expected no snapshot op, got {recorded:?}"
    );
    assert!(app.model.boards_state().is_loaded());
    let board_id = app.model.boards_state().loaded_or_empty()[0].id;
    assert!(app.model.board_cards_state(board_id).is_loaded());
}

#[tokio::test]
async fn test_reload_model_surfaces_a_failed_flat_tier_as_an_error() {
    let (mut app, _card_id) = json_app_with_archived_card().await;

    let failing = CountingBackend::wrap_failing(app.ctx.backend(), "list_boards");
    app.ctx.replace_backend(failing);

    app.reload_model();

    assert!(
        app.ui_state.banner.is_some(),
        "expected reload_model to surface a load failure"
    );
}
