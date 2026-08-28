use kanban_core::AppConfig;
use kanban_domain::export::AllBoardsExport;
use kanban_domain::KanbanResult;
use kanban_persistence::StoreRegistry;
use kanban_service::{KanbanContext, StoreManager};

fn manager() -> StoreManager {
    let mut registry = StoreRegistry::new();
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    StoreManager::new(registry, kanban_backend::KanbanBackendRegistry::new())
}

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

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_export_to_sqlite_succeeds_and_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.sqlite").to_string_lossy().to_string();

    manager()
        .export_to_sqlite(AllBoardsExport::empty(), &output, &AppConfig::default())
        .await
        .expect("export_to_sqlite must succeed");

    assert!(
        std::path::Path::new(&output).exists(),
        "exported sqlite file must be created on disk"
    );
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_export_to_sqlite_result_is_readable_via_open_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.sqlite").to_string_lossy().to_string();

    manager()
        .export_to_sqlite(AllBoardsExport::empty(), &output, &AppConfig::default())
        .await
        .expect("export_to_sqlite must succeed");

    let ctx = open_context(&output, AppConfig::default())
        .await
        .expect("open_context must succeed on exported file");
    assert_eq!(ctx.boards().unwrap().len(), 0);
}

// ─── KAN-1105: export writes through the store adapter ───────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_export_to_sqlite_preserves_full_archival_graph() {
    use kanban_domain::export::BoardExporter;
    use kanban_domain::{Archived, ArchivedCard, Board, Card, Column, Sprint};

    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.json").to_string_lossy().to_string();

    let mut stores = StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    stores.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    let sm = StoreManager::new(stores, backends);

    let mut config = AppConfig::default();
    sm.sync_backend_with_file(&source, &mut config);
    let src = sm.make_backend(&source, &config).await.unwrap();

    // A live board with a column, cards, a sprint and an archived card, plus a
    // separately archived board carrying its own subtree.
    let live = Board::new("Live", None::<String>);
    let live_col = Column::new(live.id, "Todo", 0);
    let card = Card::new(live.id, live_col.id, "Card", 0);
    let archived_card = Card::new(live.id, live_col.id, "Archived", 1);
    let sprint = Sprint::new(live.id, 1, None, None::<String>);
    let arch = Board::new("Archived board", None::<String>);
    let arch_col = Column::new(arch.id, "Done", 0);
    let arch_card = Card::new(arch.id, arch_col.id, "On archived board", 0);

    let (live_id, live_col_id, card_id, archived_card_id, sprint_id) =
        (live.id, live_col.id, card.id, archived_card.id, sprint.id);
    let (arch_id, arch_col_id, arch_card_id) = (arch.id, arch_col.id, arch_card.id);

    src.upsert_board(live).unwrap();
    src.upsert_column(live_col).unwrap();
    src.upsert_card(card).unwrap();
    src.upsert_card(archived_card).unwrap();
    src.upsert_sprint(sprint).unwrap();
    src.insert_archived_card(ArchivedCard::new(archived_card_id, live_id))
        .unwrap();
    src.upsert_board(arch).unwrap();
    src.upsert_column(arch_col).unwrap();
    src.upsert_card(arch_card).unwrap();
    src.insert_archived_board(Archived::now(arch_id)).unwrap();

    let export = BoardExporter::export_all_boards(
        &[
            src.get_board(live_id).unwrap().unwrap(),
            src.get_board(arch_id).unwrap().unwrap(),
        ],
        &src.list_all_columns().unwrap(),
        &[
            src.get_card(card_id).unwrap().unwrap(),
            src.get_card(archived_card_id).unwrap().unwrap(),
            src.get_card(arch_card_id).unwrap().unwrap(),
        ],
        &src.list_archived_cards().unwrap(),
        &src.list_archived_boards().unwrap(),
        &src.list_all_sprints().unwrap(),
    );

    let output = dir.path().join("out.sqlite").to_string_lossy().to_string();
    sm.export_to_sqlite(export, &output, &AppConfig::default())
        .await
        .expect("export must succeed");

    let mut out_config = AppConfig::default();
    sm.sync_backend_with_file(&output, &mut out_config);
    let dest = sm.make_backend(&output, &out_config).await.unwrap();

    assert!(dest.get_board(live_id).unwrap().is_some(), "live board");
    assert!(
        dest.get_board(arch_id).unwrap().is_some(),
        "archived board head"
    );
    assert!(
        dest.get_column(live_col_id).unwrap().is_some(),
        "live column"
    );
    assert!(
        dest.get_column(arch_col_id).unwrap().is_some(),
        "archived board's column"
    );
    assert!(dest.get_card(card_id).unwrap().is_some(), "live card");
    assert!(
        dest.get_card(archived_card_id).unwrap().is_some(),
        "archived card's live row"
    );
    assert!(
        dest.get_card(arch_card_id).unwrap().is_some(),
        "archived board's card"
    );
    assert!(dest.get_sprint(sprint_id).unwrap().is_some(), "sprint");
    assert!(
        dest.get_archived_card(archived_card_id).unwrap().is_some(),
        "archived-card marker"
    );
    assert!(
        dest.get_archived_board(arch_id).unwrap().is_some(),
        "archived-board marker"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_to_sqlite_rejects_an_existing_destination() {
    // The path this replaced wiped the destination's tables before inserting,
    // so an export onto an existing database silently replaced it. Writing per
    // entity would merge instead, leaving unrelated boards behind. Refusing the
    // write keeps the caller's data intact and makes them choose.
    use kanban_domain::export::BoardExporter;
    use kanban_domain::{Board, Column};

    let dir = tempfile::tempdir().unwrap();
    let target = dir
        .path()
        .join("existing.sqlite")
        .to_string_lossy()
        .to_string();

    let mut stores = StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    stores.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    let sm = StoreManager::new(stores, backends);

    let mut config = AppConfig::default();
    sm.sync_backend_with_file(&target, &mut config);
    let existing = sm.make_backend(&target, &config).await.unwrap();
    let keep = Board::new("Already here", None::<String>);
    let keep_id = keep.id;
    existing.upsert_board(keep).unwrap();
    drop(existing);

    let board = Board::new("Exported", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let export = BoardExporter::export_all_boards(&[board], &[column], &[], &[], &[], &[]);

    let err = sm
        .export_to_sqlite(export, &target, &AppConfig::default())
        .await
        .expect_err("exporting onto an existing database must be refused");
    assert!(
        err.to_string().contains("already exists"),
        "the error must say why (got: {err})"
    );

    let reopened = sm.make_backend(&target, &config).await.unwrap();
    assert!(
        reopened.get_board(keep_id).unwrap().is_some(),
        "the caller's existing data must be untouched"
    );
}
