mod helpers;

use helpers::{
    store_manager_with_fixed_backend, FailingBoardListBackend, FailingSnapshotBackend,
    SnapshotCountingBackend,
};
use kanban_domain::DataStore;
use kanban_tui::App;
use std::sync::atomic::Ordering;
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
    let failing_destination = FailingBoardListBackend::wrap(destination.clone());
    app.store_manager = Arc::new(store_manager_with_fixed_backend(
        locator.clone(),
        failing_destination,
    ));
    app.app_config.storage_location = Some(locator);

    let old_config = app.app_config.clone();
    let outgoing_instance = app.ctx.backend().instance_id();
    app.handle_migration_complete(old_config, Ok(true)).await;

    assert_eq!(
        app.ctx.backend().instance_id(),
        outgoing_instance,
        "the context must still serve the outgoing backend when the probe aborts"
    );
    assert_eq!(
        destination
            .list_boards()
            .unwrap()
            .iter()
            .find(|b| b.id == board_id)
            .map(|b| &b.id),
        Some(&board_id),
        "destination board must survive a failed post-swap board-list read"
    );
    assert_eq!(
        destination
            .list_columns_by_board(board_id)
            .unwrap()
            .iter()
            .find(|c| c.id == column_id)
            .map(|c| c.id),
        Some(column_id),
        "destination column must survive a failed post-swap board-list read"
    );
    assert_eq!(
        destination
            .list_cards_by_column(column_id)
            .unwrap()
            .iter()
            .find(|c| c.id == card_id)
            .map(|c| c.id),
        Some(card_id),
        "destination card must survive a failed post-swap board-list read"
    );

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("should have an error banner when the post-swap read fails");
    assert_eq!(banner.variant, kanban_tui::components::BannerVariant::Error);
    assert!(
        banner.message.contains("simulated transient read failure"),
        "the abort must be the injected list_boards failure, not an unrelated error: {}",
        banner.message
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_storage_swap_leaves_a_working_save_worker_when_the_post_swap_read_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file_and_save_worker(dir.path()).await;

    let old_config = app.app_config.clone();
    let old_save_file = app.persistence.save_file.clone();
    let outgoing_instance = app.ctx.backend().instance_id();

    let (destination, _board_id, _column_id, _card_id) = seeded_destination_backend();
    let locator = dir
        .path()
        .join("destination-that-fails-to-read-2.db")
        .display()
        .to_string();
    let failing_destination = FailingBoardListBackend::wrap(destination.clone());
    app.store_manager = Arc::new(store_manager_with_fixed_backend(
        locator.clone(),
        failing_destination,
    ));
    app.app_config.storage_location = Some(locator);

    app.handle_migration_complete(old_config.clone(), Ok(true))
        .await;

    assert_eq!(
        app.ctx.backend().instance_id(),
        outgoing_instance,
        "the context must still serve the outgoing backend when the probe aborts"
    );
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
        banner.message.contains("simulated transient read failure"),
        "the abort must be the injected list_boards failure, not an unrelated error: {}",
        banner.message
    );
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

/// The counter never observes `apply_snapshot`: both `SnapshotCountingBackend`
/// and `FailingSnapshotBackend` delegate it verbatim to `inner`. It only pins
/// that the migration probe itself stops reading a whole `Snapshot`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migration_complete_does_not_call_the_whole_store_trait_methods() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let (destination, _board_id, _column_id, _card_id) = seeded_destination_backend();
    let locator = dir
        .path()
        .join("destination-counted.db")
        .display()
        .to_string();
    let (counted_destination, snapshot_reads) = SnapshotCountingBackend::wrap(destination);
    app.store_manager = Arc::new(store_manager_with_fixed_backend(
        locator.clone(),
        counted_destination,
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
        "the migration must succeed"
    );
    assert_eq!(
        snapshot_reads.load(Ordering::SeqCst),
        1,
        "the only whole-Snapshot read must be reload_model's post-swap sync, \
         not the migration probe"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migration_complete_tolerates_a_failing_snapshot_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let (destination, _board_id, _column_id, _card_id) = seeded_destination_backend();
    let locator = dir
        .path()
        .join("destination-snapshot-fails.db")
        .display()
        .to_string();
    let failing_destination = FailingSnapshotBackend::wrap(destination);
    app.store_manager = Arc::new(store_manager_with_fixed_backend(
        locator.clone(),
        failing_destination,
    ));
    app.app_config.storage_location = Some(locator.clone());

    let old_config = app.app_config.clone();
    app.handle_migration_complete(old_config, Ok(true)).await;

    assert_eq!(
        app.app_config.effective_storage_location(),
        locator,
        "a failed snapshot() read on the new backend must not abort the migration \
         now that the probe no longer calls it"
    );
    assert_eq!(app.persistence.save_file.as_deref(), Some(locator.as_str()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migration_complete_preserves_active_board_sort_field() {
    use kanban_domain::{SortField, SortOrder};

    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let inner = kanban_backend_memory::InMemoryStore::new();
    let mut board = kanban_domain::Board::new("Destination", None::<String>);
    board.update_task_sort(SortField::Position, SortOrder::Descending);
    let board_id = board.id;
    inner.upsert_board(board).unwrap();
    let destination: Arc<dyn kanban_backend::KanbanBackend> = Arc::new(inner);

    let locator = dir
        .path()
        .join("destination-sort-field.db")
        .display()
        .to_string();
    app.store_manager = Arc::new(store_manager_with_fixed_backend(
        locator.clone(),
        destination,
    ));
    app.app_config.storage_location = Some(locator);
    app.selection.active_board_id = Some(board_id);
    app.filter.current_sort_field = Some(SortField::Default);
    app.filter.current_sort_order = Some(SortOrder::Ascending);

    let old_config = app.app_config.clone();
    app.handle_migration_complete(old_config, Ok(true)).await;

    assert_eq!(
        app.filter.current_sort_field,
        Some(SortField::Position),
        "the sort field must be synced from the new backend's active board"
    );
    assert_eq!(
        app.filter.current_sort_order,
        Some(SortOrder::Descending),
        "the sort order must be synced from the new backend's active board"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migration_complete_preserves_active_board_sort_field_when_board_is_archived() {
    use kanban_domain::{SortField, SortOrder};

    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let inner = kanban_backend_memory::InMemoryStore::new();
    let mut board = kanban_domain::Board::new("Archived destination", None::<String>);
    board.update_task_sort(SortField::DueDate, SortOrder::Descending);
    let board_id = board.id;
    inner.upsert_board(board).unwrap();
    inner
        .insert_archived_board(kanban_domain::ArchivedBoard::at(
            board_id,
            chrono::Utc::now(),
        ))
        .unwrap();
    let destination: Arc<dyn kanban_backend::KanbanBackend> = Arc::new(inner);

    let locator = dir
        .path()
        .join("destination-sort-field-archived.db")
        .display()
        .to_string();
    app.store_manager = Arc::new(store_manager_with_fixed_backend(
        locator.clone(),
        destination,
    ));
    app.app_config.storage_location = Some(locator);
    app.selection.active_board_id = Some(board_id);
    app.filter.current_sort_field = Some(SortField::Default);
    app.filter.current_sort_order = Some(SortOrder::Ascending);

    let old_config = app.app_config.clone();
    app.handle_migration_complete(old_config, Ok(true)).await;

    assert_eq!(
        app.filter.current_sort_field,
        Some(SortField::DueDate),
        "the sort sync must resolve an archived active board too"
    );
    assert_eq!(
        app.filter.current_sort_order,
        Some(SortOrder::Descending),
        "the sort sync must resolve an archived active board too"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migration_complete_populates_the_model_from_the_new_backend() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;
    assert_eq!(
        app.model.boards_state().loaded_or_empty()[0].name,
        "OriginalBoard"
    );

    let (destination, board_id, _column_id, _card_id) = seeded_destination_backend();
    let locator = dir
        .path()
        .join("destination-populates-model.db")
        .display()
        .to_string();
    app.store_manager = Arc::new(store_manager_with_fixed_backend(
        locator.clone(),
        destination,
    ));
    app.app_config.storage_location = Some(locator);

    let old_config = app.app_config.clone();
    app.handle_migration_complete(old_config, Ok(true)).await;

    let boards = app
        .model
        .live_boards_state()
        .loaded()
        .cloned()
        .expect("boards must be loaded after migration completes");
    assert_eq!(
        boards.len(),
        1,
        "the model must reflect the new file's boards"
    );
    assert_eq!(boards[0].id, board_id);
    assert!(
        boards.iter().all(|b| b.name != "OriginalBoard"),
        "the model must no longer carry the outgoing backend's boards"
    );
}

struct FullGraphIds {
    board_id: Uuid,
    live_column_id: Uuid,
    live_card_id: Uuid,
    child_card_id: Uuid,
    sprint_id: Uuid,
    archived_card_id: Uuid,
    archived_board_id: Uuid,
    archived_board_column_id: Uuid,
    archived_board_card_id: Uuid,
}

fn seed_full_graph(store: &dyn DataStore) -> FullGraphIds {
    use kanban_domain::{ArchivedBoard, ArchivedCard, Board, Card, Column, Prefix, Sprint};

    store.upsert_prefix(Prefix::new("FUL")).unwrap();

    let board = Board::new("Full graph board", Some("FUL"));
    let board_id = board.id;
    store.upsert_board(board).unwrap();

    let mut column = Column::new(board_id, "Todo", 0);
    column.wip_limit = Some(3);
    let live_column_id = column.id;
    store.upsert_column(column).unwrap();

    let mut parent_card = Card::new(board_id, live_column_id, "Parent", 0);
    parent_card.prefix = "FUL".to_string();
    let live_card_id = parent_card.id;
    store.upsert_card(parent_card).unwrap();

    let mut child_card = Card::new(board_id, live_column_id, "Child", 1);
    child_card.prefix = "FUL".to_string();
    let child_card_id = child_card.id;
    store.upsert_card(child_card).unwrap();

    let mut graph = store.get_graph().unwrap();
    graph.set_parent(child_card_id, live_card_id).unwrap();
    store.set_graph(graph).unwrap();

    let sprint = Sprint::new(board_id, 1, None, Some("FUL"));
    let sprint_id = sprint.id;
    store.upsert_sprint(sprint).unwrap();

    let mut archived_card = Card::new(board_id, live_column_id, "Archived", 2);
    archived_card.prefix = "FUL".to_string();
    let archived_card_id = archived_card.id;
    store.upsert_card(archived_card).unwrap();
    store
        .insert_archived_card(ArchivedCard::new(archived_card_id, board_id))
        .unwrap();

    let archived_board = Board::new("Archived board", Some("ABD"));
    let archived_board_id = archived_board.id;
    store.upsert_prefix(Prefix::new("ABD")).unwrap();
    store.upsert_board(archived_board).unwrap();
    let archived_board_column = Column::new(archived_board_id, "Todo", 0);
    let archived_board_column_id = archived_board_column.id;
    store.upsert_column(archived_board_column).unwrap();
    let mut archived_board_card = Card::new(archived_board_id, archived_board_column_id, "Sub", 0);
    archived_board_card.prefix = "ABD".to_string();
    let archived_board_card_id = archived_board_card.id;
    store.upsert_card(archived_board_card).unwrap();
    store
        .insert_archived_board(ArchivedBoard::at(archived_board_id, chrono::Utc::now()))
        .unwrap();

    FullGraphIds {
        board_id,
        live_column_id,
        live_card_id,
        child_card_id,
        sprint_id,
        archived_card_id,
        archived_board_id,
        archived_board_column_id,
        archived_board_card_id,
    }
}

fn assert_full_graph_intact(store: &dyn DataStore, ids: &FullGraphIds) {
    let board = store
        .get_board(ids.board_id)
        .unwrap()
        .expect("board must survive migration");
    assert_eq!(board.card_prefix.as_deref(), Some("FUL"));

    let column = store
        .get_column(ids.live_column_id)
        .unwrap()
        .expect("column must survive migration");
    assert_eq!(column.wip_limit, Some(3));

    let live_card = store
        .get_card(ids.live_card_id)
        .unwrap()
        .expect("parent card must survive migration");
    assert_eq!(live_card.board_id, ids.board_id);

    let child_card = store
        .get_card(ids.child_card_id)
        .unwrap()
        .expect("child card must survive migration");
    assert_eq!(child_card.board_id, ids.board_id);

    let graph = store.get_graph().unwrap();
    assert_eq!(
        graph.parents(ids.child_card_id),
        vec![ids.live_card_id],
        "the spawns dependency edge must survive migration"
    );

    let sprint = store
        .get_sprint(ids.sprint_id)
        .unwrap()
        .expect("sprint must survive migration");
    assert_eq!(sprint.board_id, ids.board_id);

    let archived_cards = store.list_archived_cards().unwrap();
    assert!(
        archived_cards
            .iter()
            .any(|ac| ac.entity_id == ids.archived_card_id),
        "archived card marker must survive migration"
    );
    assert!(
        store.get_card(ids.archived_card_id).unwrap().is_some(),
        "the archived card's live row must survive migration"
    );

    let archived_boards = store.list_archived_boards().unwrap();
    assert!(
        archived_boards
            .iter()
            .any(|ab| ab.entity_id == ids.archived_board_id),
        "archived board marker must survive migration"
    );
    assert!(
        store.get_board(ids.archived_board_id).unwrap().is_some(),
        "the archived board's live head must survive migration"
    );
    assert!(
        store
            .get_column(ids.archived_board_column_id)
            .unwrap()
            .is_some(),
        "the archived board's subtree column must survive migration"
    );
    assert!(
        store
            .get_card(ids.archived_board_card_id)
            .unwrap()
            .is_some(),
        "the archived board's subtree card must survive migration"
    );

    assert!(
        store.get_prefix("FUL").unwrap().is_some(),
        "the prefix row must survive migration"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migration_complete_preserves_the_whole_graph_of_the_incoming_file_json() {
    use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};

    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let seed_store = kanban_backend_memory::InMemoryStore::new();
    let ids = seed_full_graph(&seed_store);
    let snapshot = seed_store.snapshot().unwrap();

    let json_path = dir.path().join("destination-full-graph.json");
    let json_path_str = json_path.to_str().unwrap().to_string();
    let file_store = kanban_persistence_json::JsonFileStore::new(&json_path_str);
    let store_snapshot = StoreSnapshot {
        data: serde_json::to_vec(&snapshot).unwrap(),
        metadata: PersistenceMetadata::new(file_store.instance_id()),
    };
    file_store.save(store_snapshot).await.unwrap();

    app.app_config.storage_location = Some(json_path_str.clone());
    let old_config = app.app_config.clone();
    app.handle_migration_complete(old_config, Ok(true)).await;

    let reopened = kanban_persistence_json::JsonDataStore::new(Arc::new(
        kanban_persistence_json::JsonFileStore::new(&json_path_str),
    ));
    assert_full_graph_intact(&reopened, &ids);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migration_complete_preserves_the_whole_graph_of_the_incoming_file_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let seed_store = kanban_backend_memory::InMemoryStore::new();
    let ids = seed_full_graph(&seed_store);
    let snapshot = seed_store.snapshot().unwrap();

    let sqlite_path = dir.path().join("destination-full-graph.sqlite3");
    let sqlite_path_str = sqlite_path.to_str().unwrap().to_string();
    let sqlite_store = kanban_persistence_sqlite::SqliteStore::open(&sqlite_path_str)
        .await
        .unwrap();
    sqlite_store.apply_snapshot(snapshot).unwrap();
    drop(sqlite_store);

    app.app_config.storage_location = Some(sqlite_path_str.clone());
    let old_config = app.app_config.clone();
    app.handle_migration_complete(old_config, Ok(true)).await;

    let reopened = kanban_persistence_sqlite::SqliteStore::open(&sqlite_path_str)
        .await
        .unwrap();
    assert_full_graph_intact(&reopened, &ids);
}
