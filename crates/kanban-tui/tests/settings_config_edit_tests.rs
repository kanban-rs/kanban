mod helpers;

use kanban_tui::app::{ExportDialogState, ExportFormat, ExportStep, MigrationState};
use kanban_tui::App;

#[test]
fn test_apply_config_edit_save_failure_does_not_update_in_memory_config() {
    let mut app = App::test_default();
    let original_prefix = app.app_config.effective_default_card_prefix().to_string();

    // Force save failure portably: plant a regular file where save_to would
    // need a directory. create_dir_all refuses to turn a file into a dir, so
    // the save fails on both POSIX and Windows.
    let dir = tempfile::tempdir().unwrap();
    let file_blocking_parent = dir.path().join("blocker");
    std::fs::write(&file_blocking_parent, b"").unwrap();
    let config_path = file_blocking_parent.join("config.toml");
    let format = kanban_tui::edit_format::EditFormat::Json;
    let json = serde_json::json!({
        "default_card_prefix": "changed",
        "default_sprint_prefix": "sprint",
        "editing_format": "json",
        "configuration_format": "toml",
        "configuration_location": config_path.to_string_lossy(),
    })
    .to_string();

    let result = app.apply_config_edit(&json, &format);

    assert!(result.is_err(), "expected Err when save fails, got Ok");
    assert_eq!(
        app.app_config.effective_default_card_prefix(),
        original_prefix,
        "in-memory config must not change when save fails"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migration_state_is_idle_with_no_pending_receiver_after_completion() {
    use kanban_tui::app::MigrationState;
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let old_config = app.app_config.clone();
    let old_storage = kanban_service::config::resolve_storage_location(&app.app_config);
    let sqlite_path = dir.path().join("migrated.sqlite");
    app.app_config.storage_location = Some(sqlite_path.to_str().unwrap().to_string());

    app.apply_storage_location_change(old_config, &old_storage);
    assert!(
        matches!(app.migration_state, MigrationState::Migrating { .. }),
        "should be Migrating after apply"
    );

    app.await_migration().await;
    assert!(
        matches!(app.migration_state, MigrationState::Idle),
        "should be Idle after completion"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migration_edit_rejected_while_migrating() {
    use kanban_tui::app::MigrationState;
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    let old_config = app.app_config.clone();
    let old_storage = kanban_service::config::resolve_storage_location(&app.app_config);
    let sqlite_path = dir.path().join("migrated.sqlite");
    app.app_config.storage_location = Some(sqlite_path.to_str().unwrap().to_string());

    app.apply_storage_location_change(old_config, &old_storage);
    assert!(matches!(
        app.migration_state,
        MigrationState::Migrating { .. }
    ));

    let format = kanban_tui::edit_format::EditFormat::Json;
    let json = r#"{"default_card_prefix":"feat","default_sprint_prefix":"sprint","editing_format":"json","configuration_format":"toml"}"#;
    let result = app.apply_config_edit(json, &format);

    assert!(result.is_err(), "expected Err during migration");
    assert!(
        result.unwrap_err().contains("Migration in progress"),
        "error should mention migration"
    );

    app.await_migration().await;
}

#[test]
fn test_export_filename_rejects_path_separator_forward_slash() {
    let mut app = App::test_default();
    app.export_dialog = Some(ExportDialogState {
        board_selections: vec![true],
        cursor: 0,
        step: ExportStep::ExportOptions,
        format: ExportFormat::Json,
        filename: "export.json".to_string(),
    });

    app.handle_export_boards_dialog(crossterm::event::KeyCode::Char('/'));

    let dialog = app.export_dialog.as_ref().unwrap();
    assert_eq!(
        dialog.filename, "export.json",
        "forward slash must be rejected"
    );
}

#[test]
fn test_export_filename_rejects_path_separator_backslash() {
    let mut app = App::test_default();
    app.export_dialog = Some(ExportDialogState {
        board_selections: vec![true],
        cursor: 0,
        step: ExportStep::ExportOptions,
        format: ExportFormat::Json,
        filename: "export.json".to_string(),
    });

    app.handle_export_boards_dialog(crossterm::event::KeyCode::Char('\\'));

    let dialog = app.export_dialog.as_ref().unwrap();
    assert_eq!(dialog.filename, "export.json", "backslash must be rejected");
}

#[test]
fn test_export_filename_rejects_null_byte() {
    let mut app = App::test_default();
    app.export_dialog = Some(ExportDialogState {
        board_selections: vec![true],
        cursor: 0,
        step: ExportStep::ExportOptions,
        format: ExportFormat::Json,
        filename: "export.json".to_string(),
    });

    app.handle_export_boards_dialog(crossterm::event::KeyCode::Char('\0'));

    let dialog = app.export_dialog.as_ref().unwrap();
    assert_eq!(dialog.filename, "export.json", "null byte must be rejected");
}

#[test]
fn test_apply_config_edit_valid_json_updates_config() {
    let mut app = App::test_default();
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let format = kanban_tui::edit_format::EditFormat::Json;
    let json = serde_json::json!({
        "default_card_prefix": "feat",
        "default_sprint_prefix": "sprint",
        "editing_format": "json",
        "configuration_format": "toml",
        "configuration_location": config_path.to_string_lossy(),
    })
    .to_string();
    let result = app.apply_config_edit(&json, &format);
    assert!(result.is_ok());
    assert_eq!(app.app_config.effective_default_card_prefix(), "feat");
}

#[test]
fn test_apply_config_edit_invalid_json_returns_error() {
    let mut app = App::test_default();
    let format = kanban_tui::edit_format::EditFormat::Json;
    let result = app.apply_config_edit("{not valid json", &format);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("parse"), "error: {}", err);
}

#[test]
fn test_apply_config_edit_invalid_backend_returns_error() {
    let mut app = App::test_default();
    let format = kanban_tui::edit_format::EditFormat::Json;
    let json = r#"{"default_card_prefix":"task","default_sprint_prefix":"sprint","editing_format":"json","configuration_format":"toml","storage_backend":"yaml"}"#;
    let result = app.apply_config_edit(json, &format);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("storage_backend"), "error: {}", err);
}

#[test]
fn test_apply_config_edit_unchanged_storage_not_written_to_config() {
    // Red: from_config resolves storage_location to an absolute path; when the
    // user only changes card prefix and saves, that absolute path comes back in
    // the DTO and is written to config because strip_defaults compares against
    // the relative default ("kanban.json"), not the absolute.
    let mut app = App::test_default();
    // Reset to a known fresh-install state so the test is not affected by
    // any config file that may exist on the developer's machine.
    app.app_config = kanban_core::AppConfig::default();
    app.original_storage_backend = None;
    app.original_storage_location = None;
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let active_storage = kanban_service::config::resolve_storage_location(&app.app_config);
    let format = kanban_tui::edit_format::EditFormat::Json;
    // Simulate what the editor sends back: card prefix changed, storage fields
    // present and unchanged (as from_config would populate them).
    let json = serde_json::json!({
        "default_card_prefix": "feat",
        "default_sprint_prefix": "sprint",
        "editing_format": "json",
        "configuration_format": "toml",
        "storage_backend": "json",
        "storage_location": active_storage,
        "configuration_location": config_path.to_string_lossy(),
    })
    .to_string();
    let result = app.apply_config_edit(&json, &format);
    assert!(result.is_ok());
    assert!(
        app.app_config.storage_location.is_none(),
        "storage_location must not be written to config when it was not changed by the user"
    );
}

#[test]
fn test_apply_config_edit_with_startup_absolute_path_not_written_to_config() {
    // Simulates `kanban kanban.json` where the CLI path resolves to the same
    // location as the default. App::new sets app_config.storage_location to
    // the absolute canonical path and storage_backend via sync_backend_with_file.
    // Editing only card prefix must NOT write storage_location to the config file.
    let mut app = App::test_default();
    // Reset to a known fresh-install state so the test is not affected by
    // any config file that may exist on the developer's machine.
    app.app_config = kanban_core::AppConfig::default();
    app.original_storage_backend = None;
    app.original_storage_location = None;
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let active_storage = kanban_service::config::resolve_storage_location(&app.app_config);
    // Reproduce the startup state: absolute path + detected backend, but
    // original_storage_* remain None (no prior config on disk).
    app.app_config.storage_location = Some(active_storage.clone());
    app.app_config.storage_backend = Some("json".into());

    let format = kanban_tui::edit_format::EditFormat::Json;
    let json = serde_json::json!({
        "default_card_prefix": "feat",
        "default_sprint_prefix": "sprint",
        "editing_format": "json",
        "configuration_format": "toml",
        "storage_backend": "json",
        "storage_location": active_storage,
        "configuration_location": config_path.to_string_lossy(),
    })
    .to_string();
    let result = app.apply_config_edit(&json, &format);
    assert!(result.is_ok());
    assert!(
        app.app_config.storage_location.is_none(),
        "storage_location must not be written to config when startup injected the absolute path"
    );
}

#[test]
fn test_apply_config_edit_with_cli_override_preserves_session_storage_location() {
    // Red: currently self.app_config = config clears storage_location to None,
    // then apply_storage_location_change triggers a spurious migration that
    // eventually overwrites the config with the default path.
    let mut app = App::test_default();
    let cli_path = "/tmp/cli_supplied.json".to_string();
    app.cli_file_override = true;
    app.app_config.storage_location = Some(cli_path.clone());
    app.app_config.storage_backend = Some("json".into());

    let format = kanban_tui::edit_format::EditFormat::Json;
    let json = r#"{"default_card_prefix":"feat","default_sprint_prefix":"sprint","editing_format":"json","configuration_format":"toml"}"#;
    let _ = app.apply_config_edit(json, &format);

    assert_eq!(
        app.app_config.storage_location.as_deref(),
        Some(cli_path.as_str()),
        "session storage_location must remain the CLI-supplied path after a non-storage config edit"
    );
}

#[test]
fn test_apply_config_edit_with_cli_override_does_not_trigger_migration() {
    // Red: currently apply_storage_location_change is called with old=cli_path,
    // new=cwd/kanban.json (default after storage is cleared), triggering a migration.
    let mut app = App::test_default();
    app.cli_file_override = true;
    app.app_config.storage_location = Some("/tmp/cli_supplied.json".into());
    app.app_config.storage_backend = Some("json".into());

    let format = kanban_tui::edit_format::EditFormat::Json;
    let json = r#"{"default_card_prefix":"feat","default_sprint_prefix":"sprint","editing_format":"json","configuration_format":"toml"}"#;
    let _ = app.apply_config_edit(json, &format);

    assert!(
        matches!(app.migration_state, MigrationState::Idle),
        "no migration must be triggered when cli_file_override is active and only non-storage fields changed"
    );
}

#[test]
fn test_apply_config_edit_with_cli_override_unloads_when_storage_explicitly_provided() {
    // When cli_file_override is active, storage lines are commented out in the
    // editor. If the user uncomments them (DTO has storage fields), that is an
    // intentional request to persist those storage settings and drop the override.
    let mut app = App::test_default();
    app.cli_file_override = true;
    app.app_config.storage_location = Some("/tmp/cli_supplied.json".into());
    app.app_config.storage_backend = Some("json".into());
    app.has_data_file = false; // skip migration in this unit test

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let new_storage_path = dir.path().join("new_storage.json");
    let format = kanban_tui::edit_format::EditFormat::Json;
    // User explicitly uncomments storage fields (both present in DTO)
    let json = serde_json::json!({
        "default_card_prefix": "feat",
        "default_sprint_prefix": "sprint",
        "editing_format": "json",
        "configuration_format": "toml",
        "storage_backend": "json",
        "storage_location": new_storage_path.to_string_lossy(),
        "configuration_location": config_path.to_string_lossy(),
    })
    .to_string();
    let result = app.apply_config_edit(&json, &format);

    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    assert!(
        !app.cli_file_override,
        "cli_file_override must be cleared when user explicitly provides storage fields"
    );
}

#[test]
fn test_quit_while_migrating_shows_warning_and_does_not_quit() {
    use kanban_core::AppConfig;
    let mut app = App::test_default();
    let (_tx, rx) = tokio::sync::oneshot::channel();
    app.migration_state = MigrationState::Migrating {
        old_config: Box::new(AppConfig::default()),
        old_storage_location: "old.json".to_string(),
        result_rx: rx,
    };

    app.handle_quit_key();

    assert!(
        !app.should_quit,
        "should not quit on first q during migration"
    );
    assert!(
        app.quit_with_migration,
        "quit_with_migration must be set on first q"
    );
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("banner must be set with migration warning");
    assert!(
        banner.message.contains("Migration") || banner.message.contains("migration"),
        "banner must mention migration, got: {}",
        banner.message
    );
}

#[test]
fn test_quit_twice_while_migrating_quits() {
    use kanban_core::AppConfig;
    let mut app = App::test_default();
    let (_tx, rx) = tokio::sync::oneshot::channel();
    app.migration_state = MigrationState::Migrating {
        old_config: Box::new(AppConfig::default()),
        old_storage_location: "old.json".to_string(),
        result_rx: rx,
    };
    app.quit_with_migration = true;

    app.handle_quit_key();

    assert!(app.should_quit, "second q during migration must quit");
}

#[tokio::test]
async fn test_migration_complete_resets_quit_with_migration_flag() {
    let mut app = App::test_default();
    app.quit_with_migration = true;

    app.handle_migration_complete(
        kanban_core::AppConfig::default(),
        Err("simulated error".to_string()),
    )
    .await;

    assert!(
        !app.quit_with_migration,
        "quit_with_migration must be reset when migration completes"
    );
}

#[test]
fn test_quit_with_both_pending_saves_and_migration_sets_both_flags_on_first_press() {
    use kanban_core::AppConfig;
    let mut app = App::test_default();
    app.ctx.save_coordinator.set_pending_for_test(1);
    let (_tx, rx) = tokio::sync::oneshot::channel();
    app.migration_state = MigrationState::Migrating {
        old_config: Box::new(AppConfig::default()),
        old_storage_location: "old.json".to_string(),
        result_rx: rx,
    };

    app.handle_quit_key();

    assert!(!app.should_quit, "should not quit on first q");
    assert!(app.quit_with_pending, "quit_with_pending must be set");
    assert!(app.quit_with_migration, "quit_with_migration must be set");
    let banner = app.ui_state.banner.as_ref().expect("banner must be set");
    assert!(
        banner.message.contains("pending") || banner.message.contains("migration"),
        "banner must mention pending saves or migration, got: {}",
        banner.message
    );
}

#[test]
fn test_quit_twice_with_both_pending_saves_and_migration_quits() {
    use kanban_core::AppConfig;
    let mut app = App::test_default();
    app.ctx.save_coordinator.set_pending_for_test(1);
    let (_tx, rx) = tokio::sync::oneshot::channel();
    app.migration_state = MigrationState::Migrating {
        old_config: Box::new(AppConfig::default()),
        old_storage_location: "old.json".to_string(),
        result_rx: rx,
    };

    app.handle_quit_key();
    assert!(!app.should_quit, "should not quit after first q");

    app.handle_quit_key();
    assert!(app.should_quit, "should quit after second q");
}

#[test]
fn test_apply_config_edit_syncs_prefixes() {
    let mut app = App::test_default();
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let format = kanban_tui::edit_format::EditFormat::Json;
    let json = serde_json::json!({
        "default_card_prefix": "myprefix",
        "default_sprint_prefix": "mysprint",
        "editing_format": "json",
        "configuration_format": "toml",
        "configuration_location": config_path.to_string_lossy(),
    })
    .to_string();
    let result = app.apply_config_edit(&json, &format);
    assert!(result.is_ok());
    assert_eq!(app.app_config.effective_default_card_prefix(), "myprefix");
    assert_eq!(app.app_config.effective_default_sprint_prefix(), "mysprint");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_migration_complete_syncs_file_watcher_instance_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;

    // Set up a file watcher with an initial instance_id
    let old_watcher = kanban_persistence::FileWatcher::new();
    let old_id = uuid::Uuid::new_v4();
    old_watcher.set_own_instance_id(old_id);
    app.persistence.file_watcher = Some(old_watcher.clone());
    assert_eq!(
        app.persistence
            .file_watcher
            .as_ref()
            .unwrap()
            .own_instance_id(),
        Some(old_id),
        "watcher should have the old instance_id before migration"
    );

    // Prepare migration from JSON to SQLite
    let old_config = app.app_config.clone();
    let old_storage = kanban_service::config::resolve_storage_location(&app.app_config);
    let sqlite_path = dir.path().join("migrated.sqlite");
    app.app_config.storage_location = Some(sqlite_path.to_str().unwrap().to_string());

    app.apply_storage_location_change(old_config.clone(), &old_storage);
    app.await_migration().await;

    // After migration completes, the watcher's instance_id should be synced to
    // the new backend's instance_id (which is different from the old id since
    // JsonFileStore::new() generates a fresh one).
    let new_backend_id = app.ctx.backend().instance_id();
    assert_ne!(
        old_id, new_backend_id,
        "migration should create a new backend with a different instance_id"
    );
    assert_eq!(
        app.persistence
            .file_watcher
            .as_ref()
            .unwrap()
            .own_instance_id(),
        Some(new_backend_id),
        "watcher's instance_id must be synced to the new backend's instance_id after migration"
    );
}
