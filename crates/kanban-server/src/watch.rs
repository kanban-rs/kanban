use crate::state::AppState;
use kanban_persistence::ChangeDetector;
use kanban_service::StoreManager;
use std::path::PathBuf;

/// Watch `locator` for external changes (writes from another process — TUI,
/// CLI, MCP) and reload `state.ctx` when they happen, broadcasting a change
/// event afterward so any connected SSE clients see it too.
///
/// No-op for a SQLite locator (queries hit the live DB on every call, no
/// in-memory cache to go stale) and safe to call with a locator whose file
/// doesn't exist yet — `FileWatcher::start_watching` resolves the parent
/// directory rather than the file itself, so it can watch for the file's
/// first creation as well as later external writes.
pub async fn watch_for_external_changes(
    state: AppState,
    locator: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let sm = StoreManager::new(kanban_service::default_registry());
    if sm.is_sqlite(locator) {
        return Ok(());
    }

    let watcher = kanban_persistence::FileWatcher::new();
    let mut rx = watcher.subscribe();
    watcher.start_watching(PathBuf::from(locator)).await?;

    tokio::spawn(async move {
        // Keep `watcher` alive for the life of this task — its background
        // notify task is tied to `watcher`'s own lifetime.
        let _watcher = watcher;
        while rx.recv().await.is_ok() {
            let mut ctx = state.ctx.lock().await;
            match ctx.reload().await {
                Ok(()) => {
                    drop(ctx);
                    state.broadcast_change();
                    tracing::info!("Reloaded state from external file change");
                }
                Err(e) => tracing::error!("Failed to reload from disk: {e}"),
            }
        }
    });

    Ok(())
}
