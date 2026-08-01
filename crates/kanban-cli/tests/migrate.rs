use kanban_core::AppConfig;
use kanban_domain::{DataStore, KanbanOperations, KanbanResult};
use kanban_persistence::PersistenceStore;
use kanban_persistence_json::JsonFileStore;
use kanban_persistence_sqlite::SqliteStore;
use kanban_service::KanbanContext;
use std::sync::Arc;
use tempfile::TempDir;

async fn open_context(locator: &str, config: AppConfig) -> KanbanResult<KanbanContext> {
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

async fn create_populated_json_context(path: &std::path::Path) -> KanbanContext {
    let mut ctx = open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let board = ctx
        .create_board("Test Board".into(), Some("TB".into()))
        .unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    ctx.create_card(board.id, col.id, "Test Card".into(), Default::default())
        .unwrap();
    ctx.save().await.unwrap();
    ctx
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_json_to_sqlite_roundtrip() {
    let dir = TempDir::new().unwrap();
    let json_path = dir.path().join("source.json");
    let db_path = dir.path().join("dest.db");

    let json_store = Arc::new(JsonFileStore::new(&json_path));
    let original = create_populated_json_context(&json_path).await;

    // Migrate snapshot from JSON to SQLite
    let (snap, _) = json_store.load().await.unwrap();
    let snapshot: kanban_domain::Snapshot = serde_json::from_slice(&snap.data).unwrap();
    let sqlite = SqliteStore::open(db_path.to_str().unwrap()).await.unwrap();
    sqlite.apply_snapshot(snapshot).unwrap();
    drop(sqlite);

    let loaded = open_context(db_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();

    assert_eq!(
        original.list_boards().unwrap().len(),
        loaded.list_boards().unwrap().len()
    );
    let orig_board = &original.list_boards().unwrap()[0];
    let loaded_board = &loaded.list_boards().unwrap()[0];
    assert_eq!(orig_board.name, loaded_board.name);
    assert_eq!(orig_board.card_prefix, loaded_board.card_prefix);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_sqlite_to_json_roundtrip() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("source.db");
    let json_path = dir.path().join("dest.json");

    let mut original =
        open_context(db_path.to_str().unwrap(), AppConfig::default())
            .await
            .unwrap();
    let board = original
        .create_board("Test Board".into(), Some("TB".into()))
        .unwrap();
    let col = original
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    original
        .create_card(board.id, col.id, "Test Card".into(), Default::default())
        .unwrap();

    // Migrate snapshot from SQLite to JSON via context snapshot
    let snapshot = original.snapshot().unwrap();
    let data = serde_json::to_vec(&snapshot).unwrap();
    let json_store = Arc::new(JsonFileStore::new(&json_path));
    let store_snap = kanban_persistence::StoreSnapshot {
        data,
        metadata: kanban_persistence::PersistenceMetadata::new(uuid::Uuid::new_v4()),
    };
    json_store.save(store_snap).await.unwrap();

    let loaded = open_context(json_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();

    assert_eq!(
        original.list_boards().unwrap().len(),
        loaded.list_boards().unwrap().len()
    );
    let orig_board = &original.list_boards().unwrap()[0];
    let loaded_board = &loaded.list_boards().unwrap()[0];
    assert_eq!(orig_board.name, loaded_board.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_json_to_json_roundtrip() {
    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("source.json");
    let dst_path = dir.path().join("dest.json");

    let src_store = Arc::new(JsonFileStore::new(&src_path));
    create_populated_json_context(&src_path).await;

    let (snapshot, _) = src_store.load().await.unwrap();
    let dst_store = Arc::new(JsonFileStore::new(&dst_path));
    dst_store.save(snapshot).await.unwrap();

    let loaded = open_context(dst_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();

    assert_eq!(loaded.list_boards().unwrap().len(), 1);
    assert_eq!(loaded.list_boards().unwrap()[0].name, "Test Board");
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_sqlite_to_sqlite_roundtrip() {
    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("source.db");
    let dst_path = dir.path().join("dest.db");

    let mut original =
        open_context(src_path.to_str().unwrap(), AppConfig::default())
            .await
            .unwrap();
    let board = original
        .create_board("Test Board".into(), Some("TB".into()))
        .unwrap();
    let col = original
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    original
        .create_card(board.id, col.id, "Test Card".into(), Default::default())
        .unwrap();

    // Copy snapshot to destination
    let snapshot = original.snapshot().unwrap();
    let dst = SqliteStore::open(dst_path.to_str().unwrap()).await.unwrap();
    dst.apply_snapshot(snapshot).unwrap();
    drop(dst);

    let loaded = open_context(dst_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();

    assert_eq!(loaded.list_boards().unwrap().len(), 1);
    assert_eq!(loaded.list_boards().unwrap()[0].name, "Test Board");
}

#[tokio::test]
async fn test_migrate_rejects_missing_source() {
    use assert_cmd::cargo_bin_cmd;

    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("nonexistent.json");

    let output = cargo_bin_cmd!("kanban")
        .args([
            "migrate",
            missing.to_str().unwrap(),
            "sqlite",
            "--output",
            dir.path().join("dest.sqlite").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found") || stderr.contains("does not exist"),
        "stderr: {stderr}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_rejects_existing_target() {
    use assert_cmd::cargo_bin_cmd;

    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("source.json");
    let dst_path = dir.path().join("dest.db");

    create_populated_json_context(&src_path).await;

    std::fs::write(&dst_path, "existing").unwrap();

    let output = cargo_bin_cmd!("kanban")
        .args([
            "migrate",
            src_path.to_str().unwrap(),
            "sqlite",
            "--output",
            dst_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_cli_with_explicit_output() {
    use assert_cmd::cargo_bin_cmd;

    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("source.json");
    let dst_path = dir.path().join("custom_output.db");

    create_populated_json_context(&src_path).await;

    let output = cargo_bin_cmd!("kanban")
        .args([
            "migrate",
            src_path.to_str().unwrap(),
            "sqlite",
            "--output",
            dst_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dst_path.exists());

    let loaded = open_context(dst_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    assert_eq!(loaded.list_boards().unwrap().len(), 1);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_cli_explicit_output_path() {
    use assert_cmd::cargo_bin_cmd;

    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("myboard.json");
    let dst_path = dir.path().join("myboard.db");

    create_populated_json_context(&src_path).await;

    let output = cargo_bin_cmd!("kanban")
        .args([
            "migrate",
            src_path.to_str().unwrap(),
            "sqlite",
            "--output",
            dst_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        dst_path.exists(),
        "Expected output at {}",
        dst_path.display()
    );

    let loaded = open_context(dst_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    assert_eq!(loaded.list_boards().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_cli_default_output_path() {
    use assert_cmd::cargo_bin_cmd;

    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("myboard.json");
    let expected_output = dir.path().join("myboard.sqlite");

    create_populated_json_context(&src_path).await;

    let output = cargo_bin_cmd!("kanban")
        .args(["migrate", src_path.to_str().unwrap(), "sqlite"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        expected_output.exists(),
        "Expected default output at {}",
        expected_output.display()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_rejects_unknown_backend() {
    use assert_cmd::cargo_bin_cmd;

    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("source.json");

    create_populated_json_context(&src_path).await;

    let output = cargo_bin_cmd!("kanban")
        .args([
            "migrate",
            src_path.to_str().unwrap(),
            "postgres",
            "--output",
            dir.path().join("dest.postgres").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // clap rejects unrecognised backends via PossibleValuesParser before the
    // service layer is reached; both rejection sites are acceptable.
    assert!(
        stderr.contains("No backend registered for")
            || stderr.contains("Unknown backend")
            || stderr.contains("invalid value"),
        "stderr: {stderr}"
    );
}
