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
        .export_to_sqlite(AllBoardsExport::empty(), &output)
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
        .export_to_sqlite(AllBoardsExport::empty(), &output)
        .await
        .expect("export_to_sqlite must succeed");

    let ctx = open_context(&output, AppConfig::default())
        .await
        .expect("open_context must succeed on exported file");
    assert_eq!(ctx.boards().unwrap().len(), 0);
}
