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

async fn make_tui_ctx(path: &std::path::Path) -> TuiContext {
    let sm = test_store_manager();
    let backend = sm
        .make_backend(path.to_str().unwrap(), &AppConfig::default())
        .await
        .unwrap();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let (tui_ctx, _, _) = TuiContext::new(ctx).unwrap();
    tui_ctx
}

/// After `reload()`, the UndoStack is cleared and the context is
/// immediately ready for mutations.
#[tokio::test]
async fn test_tui_reload_clears_history_and_re_arms() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("reload.json");
    let mut tui_ctx = make_tui_ctx(&path).await;

    tui_ctx.create_board("Before".to_string(), None).unwrap();
    assert!(tui_ctx.can_undo(), "must be undoable before reload");

    tui_ctx.save().await.unwrap();
    tui_ctx.reload().await.unwrap();

    assert!(
        !tui_ctx.can_undo(),
        "undo history must be cleared after reload"
    );

    tui_ctx
        .create_board("After".to_string(), None)
        .expect("context must accept mutations immediately after reload");
    assert!(
        tui_ctx.can_undo(),
        "must be undoable after a post-reload mutation"
    );
}

/// `reload()` must expose the data that was present on disk at the time of
/// the reload.
#[tokio::test]
async fn test_tui_reload_then_save_preserves_data() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("preserve.json");
    let mut tui_ctx = make_tui_ctx(&path).await;

    tui_ctx.create_board("Persisted".to_string(), None).unwrap();
    tui_ctx.save().await.unwrap();

    tui_ctx.reload().await.unwrap();

    let boards = tui_ctx.list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "Persisted");
}
