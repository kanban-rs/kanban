mod helpers;

use helpers::{store_manager_with_fixed_backend, FailingSnapshotBackend};
use kanban_domain::DataStore;
use kanban_tui::App;
use std::sync::Arc;
use uuid::Uuid;

// --- File arg overrides config backend tests ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_file_arg_detects_backend_from_content_ignoring_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = helpers::create_test_json_file(dir.path(), "board.json", &["TestBoard"]).await;

    let (mut app, _rx) = App::new(Some(path)).await.unwrap();
    app.load_initial_state().await;

    assert_eq!(app.app_config.effective_storage_backend(), "json");
    assert!(app
        .app_config
        .storage_location
        .as_ref()
        .unwrap()
        .contains("board.json"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_file_arg_new_file_defaults_to_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("brand_new.myext");
    assert!(!path.exists());

    let (app, _rx) = App::new(Some(path.to_str().unwrap().to_string()))
        .await
        .unwrap();
    assert_eq!(app.app_config.effective_storage_backend(), "json");
}

// --- Storage location switching tests ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migrate_json_to_sqlite_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;
    assert_eq!(app.model.boards_state().loaded_or_empty().len(), 1);
    assert_eq!(
        app.model.boards_state().loaded_or_empty()[0].name,
        "OriginalBoard"
    );

    let old_config = app.app_config.clone();
    let old_storage_location = app.app_config.effective_storage_location();
    let sqlite_path = dir.path().join("migrated.sqlite");
    app.app_config.storage_location = Some(sqlite_path.to_str().unwrap().to_string());

    app.apply_storage_location_change(old_config, &old_storage_location);
    app.await_migration().await;
    app.reload_model();
    app.prepare_frame();
    assert!(sqlite_path.exists(), "SQLite file should be created");
    assert_eq!(app.model.boards_state().loaded_or_empty().len(), 1);
    assert_eq!(
        app.model.boards_state().loaded_or_empty()[0].name,
        "OriginalBoard"
    );
    assert_eq!(
        app.persistence.save_file.as_deref(),
        Some(sqlite_path.to_str().unwrap())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_switch_to_existing_sqlite_reloads_data() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;
    assert_eq!(
        app.model.boards_state().loaded_or_empty()[0].name,
        "OriginalBoard"
    );

    let sqlite_path =
        helpers::create_test_sqlite_file(dir.path(), "other.db", &["SqliteBoard"]).await;

    let old_config = app.app_config.clone();
    let old_storage_location = app.app_config.effective_storage_location();
    app.app_config.storage_location = Some(sqlite_path.clone());

    app.apply_storage_location_change(old_config, &old_storage_location);
    app.await_migration().await;
    app.reload_model();
    app.prepare_frame();
    assert_eq!(app.model.boards_state().loaded_or_empty().len(), 1);
    assert_eq!(
        app.model.boards_state().loaded_or_empty()[0].name,
        "SqliteBoard"
    );
    assert_eq!(
        app.persistence.save_file.as_deref(),
        Some(sqlite_path.as_str())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_switch_to_existing_json_reloads_data() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;
    assert_eq!(
        app.model.boards_state().loaded_or_empty()[0].name,
        "OriginalBoard"
    );

    let second_json =
        helpers::create_test_json_file(dir.path(), "other.json", &["SecondBoard"]).await;

    let old_config = app.app_config.clone();
    let old_storage_location = app.app_config.effective_storage_location();
    app.app_config.storage_location = Some(second_json.clone());

    app.apply_storage_location_change(old_config, &old_storage_location);
    app.await_migration().await;
    app.reload_model();
    app.prepare_frame();
    assert_eq!(app.model.boards_state().loaded_or_empty().len(), 1);
    assert_eq!(
        app.model.boards_state().loaded_or_empty()[0].name,
        "SecondBoard"
    );
    assert_eq!(
        app.persistence.save_file.as_deref(),
        Some(second_json.as_str())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_backend_mismatch_auto_corrected_with_warning() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let old_config = app.app_config.clone();
    let old_storage_location = app.app_config.effective_storage_location();

    app.app_config.storage_backend = Some("sqlite".into());

    app.apply_storage_location_change(old_config, &old_storage_location);

    assert_eq!(app.app_config.effective_storage_backend(), "json");

    let banner = app.ui_state.banner.as_ref().expect("should have banner");
    assert!(
        banner.message.contains("json"),
        "banner should mention the detected backend: {}",
        banner.message
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_switch_storage_location_nonexistent_parent_shows_error() {
    use kanban_tui::components::BannerVariant;

    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let old_config = app.app_config.clone();
    let old_storage_location = app.app_config.effective_storage_location();
    app.app_config.storage_location = Some("/nonexistent/dir/board.json".to_string());

    app.apply_storage_location_change(old_config.clone(), &old_storage_location);
    app.await_migration().await;

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("should have error banner");
    assert_eq!(banner.variant, BannerVariant::Error);

    assert_eq!(
        app.app_config.effective_storage_location(),
        old_config.effective_storage_location(),
        "config should be reverted on error"
    );
}

fn seeded_destination_backend() -> (Arc<dyn kanban_backend::KanbanBackend>, Uuid, Uuid, Uuid) {
    use kanban_domain::{Board, Card, Column};

    let inner = kanban_backend_memory::InMemoryStore::new();
    let board = Board::new("Destination", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let card = Card::new(board.id, column.id, "Existing card", 0);
    let (board_id, column_id, card_id) = (board.id, column.id, card.id);
    inner.upsert_board(board).unwrap();
    inner.upsert_column(column).unwrap();
    inner.upsert_card(card).unwrap();
    (Arc::new(inner), board_id, column_id, card_id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_storage_swap_does_not_apply_an_empty_snapshot_when_the_read_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let (destination, board_id, column_id, card_id) = seeded_destination_backend();
    let locator = dir
        .path()
        .join("destination-that-fails-to-read.db")
        .display()
        .to_string();
    let failing_destination = FailingSnapshotBackend::wrap(destination.clone());
    app.store_manager = Arc::new(store_manager_with_fixed_backend(
        locator.clone(),
        failing_destination,
    ));
    app.app_config.storage_location = Some(locator);

    let old_config = app.app_config.clone();
    app.handle_migration_complete(old_config, Ok(true)).await;

    assert_eq!(
        destination
            .list_boards()
            .unwrap()
            .iter()
            .find(|b| b.id == board_id)
            .map(|b| &b.id),
        Some(&board_id),
        "destination board must survive a failed post-swap snapshot read"
    );
    assert_eq!(
        destination
            .list_columns_by_board(board_id)
            .unwrap()
            .iter()
            .find(|c| c.id == column_id)
            .map(|c| c.id),
        Some(column_id),
        "destination column must survive a failed post-swap snapshot read"
    );
    assert_eq!(
        destination
            .list_cards_by_column(column_id)
            .unwrap()
            .iter()
            .find(|c| c.id == card_id)
            .map(|c| c.id),
        Some(card_id),
        "destination card must survive a failed post-swap snapshot read"
    );

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("should have an error banner when the post-swap read fails");
    assert_eq!(banner.variant, kanban_tui::components::BannerVariant::Error);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_storage_swap_leaves_a_working_save_worker_when_the_post_swap_read_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file_and_save_worker(dir.path()).await;

    let old_config = app.app_config.clone();
    let old_save_file = app.persistence.save_file.clone();

    let (destination, _board_id, _column_id, _card_id) = seeded_destination_backend();
    let locator = dir
        .path()
        .join("destination-that-fails-to-read-2.db")
        .display()
        .to_string();
    let failing_destination = FailingSnapshotBackend::wrap(destination.clone());
    app.store_manager = Arc::new(store_manager_with_fixed_backend(
        locator.clone(),
        failing_destination,
    ));
    app.app_config.storage_location = Some(locator);

    app.handle_migration_complete(old_config.clone(), Ok(true))
        .await;

    assert_eq!(
        app.app_config.effective_storage_location(),
        old_config.effective_storage_location(),
        "config should be reverted when the post-swap read fails"
    );
    assert_eq!(
        app.persistence.save_file, old_save_file,
        "save_file should not point at the unreadable destination"
    );

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("should have an error banner when the post-swap read fails");
    assert_eq!(banner.variant, kanban_tui::components::BannerVariant::Error);
    assert!(
        !banner.message.starts_with("Loaded from") && !banner.message.starts_with("Migrated to"),
        "a success banner must not overwrite the swap-failure error: {}",
        banner.message
    );

    app.ctx.save_coordinator.queue_flush();
    let ack = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        app.persistence
            .save_completion_rx
            .as_mut()
            .expect("a save worker should still be wired up after a failed post-swap read")
            .recv(),
    )
    .await
    .expect("save worker did not acknowledge the flush within the timeout");

    assert_eq!(
        ack,
        Some(()),
        "a queued flush should be acknowledged by a live save worker"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_storage_swap_still_syncs_the_view_when_the_read_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let (destination, board_id, _column_id, _card_id) = seeded_destination_backend();
    let locator = dir
        .path()
        .join("destination-that-reads-fine.db")
        .display()
        .to_string();
    app.store_manager = Arc::new(store_manager_with_fixed_backend(
        locator.clone(),
        destination,
    ));
    app.app_config.storage_location = Some(locator);

    let old_config = app.app_config.clone();
    app.handle_migration_complete(old_config, Ok(true)).await;

    assert!(
        app.ui_state
            .banner
            .as_ref()
            .map(|b| b.variant != kanban_tui::components::BannerVariant::Error)
            .unwrap_or(true),
        "a successful read must not surface an error banner"
    );
    assert_eq!(
        app.model.boards_state().loaded_or_empty().len(),
        1,
        "the view must be synced from the destination on a successful read"
    );
    assert_eq!(app.model.boards_state().loaded_or_empty()[0].id, board_id);
}
