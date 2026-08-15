mod helpers;

use kanban_tui::App;

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
    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "OriginalBoard");

    let old_config = app.app_config.clone();
    let old_storage_location = app.app_config.effective_storage_location();
    let sqlite_path = dir.path().join("migrated.sqlite");
    app.app_config.storage_location = Some(sqlite_path.to_str().unwrap().to_string());

    app.apply_storage_location_change(old_config, &old_storage_location);
    app.await_migration().await;
    app.reload_model();
    app.prepare_frame();
    assert!(sqlite_path.exists(), "SQLite file should be created");
    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "OriginalBoard");
    assert_eq!(
        app.persistence.save_file.as_deref(),
        Some(sqlite_path.to_str().unwrap())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_switch_to_existing_sqlite_reloads_data() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;
    assert_eq!(app.model.boards()[0].name, "OriginalBoard");

    let sqlite_path =
        helpers::create_test_sqlite_file(dir.path(), "other.db", &["SqliteBoard"]).await;

    let old_config = app.app_config.clone();
    let old_storage_location = app.app_config.effective_storage_location();
    app.app_config.storage_location = Some(sqlite_path.clone());

    app.apply_storage_location_change(old_config, &old_storage_location);
    app.await_migration().await;
    app.reload_model();
    app.prepare_frame();
    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "SqliteBoard");
    assert_eq!(
        app.persistence.save_file.as_deref(),
        Some(sqlite_path.as_str())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_switch_to_existing_json_reloads_data() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;
    assert_eq!(app.model.boards()[0].name, "OriginalBoard");

    let second_json =
        helpers::create_test_json_file(dir.path(), "other.json", &["SecondBoard"]).await;

    let old_config = app.app_config.clone();
    let old_storage_location = app.app_config.effective_storage_location();
    app.app_config.storage_location = Some(second_json.clone());

    app.apply_storage_location_change(old_config, &old_storage_location);
    app.await_migration().await;
    app.reload_model();
    app.prepare_frame();
    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "SecondBoard");
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
