use kanban_domain::KanbanOperations;
use kanban_service::{AppConfig, KanbanContext, StoreManager};
use kanban_tui::tui_context::TuiContext;
use tempfile::TempDir;

fn test_store_manager() -> StoreManager {
    let mut registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    registry.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    StoreManager::new(registry, backends)
}

async fn make_ctx_with_persistence() -> (TuiContext, tokio::sync::mpsc::Receiver<()>, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.json");
    let sm = test_store_manager();
    let backend = sm
        .make_backend(path.to_str().unwrap(), &AppConfig::default())
        .await
        .unwrap();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let (tui_ctx, save_rx, _) = TuiContext::new(ctx).unwrap();
    (tui_ctx, save_rx.unwrap(), dir)
}

#[tokio::test]
async fn test_undo_queues_flush_signal_to_save_coordinator() {
    let (mut ctx, mut save_rx, _dir) = make_ctx_with_persistence().await;

    ctx.create_board("Board".into(), None).unwrap();
    // drain the post-create flush signal
    save_rx.try_recv().ok();

    assert!(ctx.undo().unwrap());
    save_rx
        .try_recv()
        .expect("undo should queue a flush signal to the save coordinator");
    assert!(
        ctx.list_boards().unwrap().is_empty(),
        "state after undo should reflect the rolled-back board list"
    );
}

#[tokio::test]
async fn test_redo_queues_flush_signal_to_save_coordinator() {
    let (mut ctx, mut save_rx, _dir) = make_ctx_with_persistence().await;

    ctx.create_board("Board".into(), None).unwrap();
    assert!(ctx.undo().unwrap());
    // drain setup flush signals (create + undo)
    while save_rx.try_recv().is_ok() {}

    assert!(ctx.redo().unwrap());
    save_rx
        .try_recv()
        .expect("redo should queue a flush signal to the save coordinator");
    assert_eq!(
        ctx.list_boards().unwrap().len(),
        1,
        "state after redo should reflect the re-applied board"
    );
}

#[tokio::test]
async fn test_undo_when_nothing_to_undo_does_not_queue_flush_signal() {
    let (mut ctx, mut save_rx, _dir) = make_ctx_with_persistence().await;

    assert!(!ctx.undo().unwrap());
    assert!(
        save_rx.try_recv().is_err(),
        "failed undo should not queue a flush signal"
    );
}
