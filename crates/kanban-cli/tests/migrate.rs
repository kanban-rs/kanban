use kanban_core::AppConfig;
use kanban_domain::{DataStore, KanbanOperations, KanbanResult};
use kanban_persistence::PersistenceStore;
use kanban_persistence_json::JsonFileStore;
use kanban_persistence_sqlite::SqliteStore;
use kanban_service::KanbanContext;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

async fn open_context(locator: &str, config: AppConfig) -> KanbanResult<KanbanContext> {
    let mut config = config;
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
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

    let mut original = open_context(db_path.to_str().unwrap(), AppConfig::default())
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

    let mut original = open_context(src_path.to_str().unwrap(), AppConfig::default())
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

fn make_store_manager() -> kanban_service::StoreManager {
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    kanban_service::StoreManager::new(stores, backends)
}

struct FullGraph {
    board: Uuid,
    other_board: Uuid,
    col_lowest: Uuid,
    col_other: Uuid,
    blocker: Uuid,
    blocked: Uuid,
    sprint: Uuid,
    archived_card: Uuid,
    archived_board: Uuid,
}

/// Seeds a non-trivial entity graph through the real `KanbanContext` API: two
/// boards (one archived), two columns, a live card that blocks another, a
/// sprint with a bound card, and an archived card. `path`'s extension picks
/// the source backend the same way the CLI's own backend detection does.
async fn seed_full_graph(path: &std::path::Path) -> FullGraph {
    use kanban_domain::{GraphOperations, Severity};

    let mut ctx = open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board One".into(), Some("B1".into()))
        .unwrap();
    let col_lowest = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let col_other = ctx.create_column(board.id, "Done".into(), None).unwrap();

    let blocker = ctx
        .create_card(
            board.id,
            col_lowest.id,
            "Blocker".into(),
            Default::default(),
        )
        .unwrap();
    let blocked = ctx
        .create_card(
            board.id,
            col_lowest.id,
            "Blocked".into(),
            Default::default(),
        )
        .unwrap();
    ctx.block(blocker.id, blocked.id, Severity::High).unwrap();

    let sprint = ctx
        .create_sprint(board.id, Some("SPR".into()), Some("Sprint 1".into()))
        .unwrap();
    ctx.assign_card_to_sprint(blocker.id, sprint.id).unwrap();

    let to_archive = ctx
        .create_card(
            board.id,
            col_other.id,
            "Will archive".into(),
            Default::default(),
        )
        .unwrap();
    ctx.archive_card(to_archive.id).unwrap();

    let other_board = ctx
        .create_board("Board Two".into(), Some("B2".into()))
        .unwrap();
    ctx.archive_board(other_board.id).unwrap();

    ctx.save().await.unwrap();

    FullGraph {
        board: board.id,
        other_board: other_board.id,
        col_lowest: col_lowest.id,
        col_other: col_other.id,
        blocker: blocker.id,
        blocked: blocked.id,
        sprint: sprint.id,
        archived_card: to_archive.id,
        archived_board: other_board.id,
    }
}

fn assert_full_graph_present(ctx: &KanbanContext, g: &FullGraph) {
    use kanban_domain::GraphOperations;

    let boards = ctx.list_boards().unwrap();
    assert!(
        boards.iter().any(|b| b.id == g.board),
        "live board must survive"
    );
    let archived_boards = ctx.list_archived_boards().unwrap();
    assert!(
        archived_boards
            .iter()
            .any(|ab| ab.entity_id == g.archived_board),
        "archived board marker must survive"
    );
    assert!(
        ctx.get_board(g.other_board).unwrap().is_some(),
        "archived board head must survive"
    );

    let columns = ctx.list_all_columns().unwrap();
    assert!(
        columns.iter().any(|c| c.id == g.col_lowest),
        "lowest-position column must survive"
    );
    assert!(
        columns.iter().any(|c| c.id == g.col_other),
        "second column must survive"
    );

    assert!(ctx.get_card(g.blocker).unwrap().is_some(), "blocker card");
    assert!(ctx.get_card(g.blocked).unwrap().is_some(), "blocked card");
    assert!(
        ctx.get_card(g.archived_card).unwrap().is_some(),
        "archived card's live row must survive"
    );

    let archived_cards = ctx.list_archived_cards().unwrap();
    assert!(
        archived_cards
            .iter()
            .any(|ac| ac.entity_id == g.archived_card),
        "archived-card marker must survive"
    );

    let sprints = ctx.list_all_sprints().unwrap();
    assert!(
        sprints.iter().any(|s| s.id == g.sprint),
        "sprint must survive"
    );

    let blocker_card = ctx.get_card(g.blocker).unwrap().unwrap();
    assert_eq!(
        blocker_card.sprint_id,
        Some(g.sprint),
        "sprint binding on the card must survive"
    );

    assert_eq!(
        ctx.list_blockers_of(g.blocked).unwrap(),
        vec![g.blocker],
        "the dependency edge must survive"
    );
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_json_to_sqlite_migration_preserves_full_graph() {
    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("source.json");
    let dst_path = dir.path().join("dest.db");

    let graph = seed_full_graph(&src_path).await;

    let sm = make_store_manager();
    sm.migrate_store(
        "json",
        src_path.to_str().unwrap(),
        "sqlite",
        dst_path.to_str().unwrap(),
    )
    .await
    .unwrap();

    let loaded = open_context(dst_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    assert_full_graph_present(&loaded, &graph);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_to_json_migration_preserves_full_graph() {
    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("source.db");
    let dst_path = dir.path().join("dest.json");

    let graph = seed_full_graph(&src_path).await;

    let sm = make_store_manager();
    sm.migrate_store(
        "sqlite",
        src_path.to_str().unwrap(),
        "json",
        dst_path.to_str().unwrap(),
    )
    .await
    .unwrap();

    let loaded = open_context(dst_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    assert_full_graph_present(&loaded, &graph);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migration_round_trip_is_identity_over_the_entity_graph() {
    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("source.json");
    let mid_path = dir.path().join("mid.db");
    let dst_path = dir.path().join("dest.json");

    let graph = seed_full_graph(&src_path).await;

    let sm = make_store_manager();
    sm.migrate_store(
        "json",
        src_path.to_str().unwrap(),
        "sqlite",
        mid_path.to_str().unwrap(),
    )
    .await
    .unwrap();
    sm.migrate_store(
        "sqlite",
        mid_path.to_str().unwrap(),
        "json",
        dst_path.to_str().unwrap(),
    )
    .await
    .unwrap();

    let loaded = open_context(dst_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    assert_full_graph_present(&loaded, &graph);

    let blocker_card = loaded.get_card(graph.blocker).unwrap().unwrap();
    let blocked_card = loaded.get_card(graph.blocked).unwrap().unwrap();
    assert_eq!(blocker_card.column_id, graph.col_lowest);
    assert_eq!(blocked_card.column_id, graph.col_lowest);

    let columns = loaded.list_all_columns().unwrap();
    let lowest = columns.iter().find(|c| c.id == graph.col_lowest).unwrap();
    let other = columns.iter().find(|c| c.id == graph.col_other).unwrap();
    assert_eq!(
        lowest.position, 0,
        "column ordering must survive the round trip"
    );
    assert!(other.position > lowest.position);
}

/// A live card whose column has since disappeared cannot be reconstructed
/// through the normal API (deleting a non-empty column is refused), so the
/// dangling reference is planted directly in the JSON snapshot after seeding
/// through the real context.
#[tokio::test(flavor = "multi_thread")]
async fn test_orphaned_card_survives_json_leg_and_is_rehomed_to_the_lowest_column() {
    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("source.json");
    let dst_path = dir.path().join("dest.db");

    let mut ctx = open_context(src_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col_lowest = ctx.create_column(board.id, "Backlog".into(), None).unwrap();
    let col_other = ctx.create_column(board.id, "Doing".into(), None).unwrap();
    let orphan = ctx
        .create_card(board.id, col_other.id, "Orphan".into(), Default::default())
        .unwrap();
    ctx.save().await.unwrap();
    drop(ctx);

    let json_store = Arc::new(JsonFileStore::new(&src_path));
    let (snap, _) = json_store.load().await.unwrap();
    let mut snapshot: kanban_domain::Snapshot = serde_json::from_slice(&snap.data).unwrap();
    for card in snapshot.cards.iter_mut() {
        if card.id == orphan.id {
            card.column_id = uuid::Uuid::new_v4();
        }
    }
    let data = serde_json::to_vec(&snapshot).unwrap();
    json_store
        .save(kanban_persistence::StoreSnapshot {
            data,
            metadata: kanban_persistence::PersistenceMetadata::new(uuid::Uuid::new_v4()),
        })
        .await
        .unwrap();

    let sm = make_store_manager();
    sm.migrate_store(
        "json",
        src_path.to_str().unwrap(),
        "sqlite",
        dst_path.to_str().unwrap(),
    )
    .await
    .unwrap();

    let loaded = open_context(dst_path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let migrated = loaded
        .get_card(orphan.id)
        .unwrap()
        .expect("orphaned card must survive migration");
    assert_eq!(
        migrated.column_id, col_lowest.id,
        "orphaned card must be rehomed to the lowest-position column"
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
