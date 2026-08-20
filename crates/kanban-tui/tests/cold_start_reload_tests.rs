mod helpers;

use helpers::{create_test_json_file, SnapshotCountingBackend};
use kanban_domain::{Column, Snapshot, Sprint};
use kanban_tui::App;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn test_cold_start_reads_the_whole_workspace_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = create_test_json_file(dir.path(), "source.json", &["Board"]).await;
    let (mut app, _rx) = App::new(Some(path)).await.unwrap();

    let (backend, snapshot_reads) = SnapshotCountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);

    app.load_initial_state().await;

    assert_eq!(
        snapshot_reads.load(Ordering::SeqCst),
        1,
        "cold start must read the whole workspace exactly once"
    );
    let board_present = app
        .model
        .boards_state()
        .loaded_or_empty()
        .iter()
        .any(|b| b.name == "Board");
    assert!(board_present, "the model must reflect the seeded board");
}

#[tokio::test]
async fn test_cold_start_after_a_sprint_log_migration_loads_the_migrated_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("migrate.json");
    let path_str = path.to_str().unwrap().to_string();

    let store = kanban_persistence_json::JsonFileStore::new(&path_str);
    let board = kanban_domain::Board::new("Board".to_string(), None::<String>);
    let column = Column::new(board.id, "Todo".to_string(), 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    let mut card = kanban_domain::Card::new(board.id, column.id, "Card".to_string(), 0);
    card.sprint_id = Some(sprint.id);
    assert!(
        card.sprint_logs.is_empty(),
        "precondition: seeded card has no sprint log yet"
    );

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board],
        columns: vec![column],
        cards: vec![card.clone()],
        archived_cards: vec![],
        sprints: vec![sprint],
        graph: Default::default(),
        prefixes: Vec::new(),
    };

    use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};
    let store_snapshot = StoreSnapshot {
        data: serde_json::to_vec(&snapshot).unwrap(),
        metadata: PersistenceMetadata::new(store.instance_id()),
    };
    store.save(store_snapshot).await.unwrap();

    let (mut app, _rx) = App::new(Some(path_str)).await.unwrap();
    app.load_initial_state().await;

    let migrated_log_present = app
        .model
        .all_cards()
        .iter()
        .find(|c| c.id == card.id)
        .map(|c| !c.sprint_logs.is_empty())
        .unwrap_or(false);
    assert!(
        migrated_log_present,
        "cold start must serve the migrated sprint log, not the stale pre-migration probe snapshot"
    );
}
