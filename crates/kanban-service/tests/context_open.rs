/// Integration tests for `KanbanContext::open_deferred` (Steps 3 + 6 of the
/// "Unified Backends via True Deferred Reads" architecture).
///
/// SQLite tests use `#[tokio::test(flavor = "multi_thread")]` because sqlx
/// connection pools spawn background tasks that deadlock on single-threaded
/// runtimes. JSON tests no longer require `multi_thread`.
use kanban_domain::DataStore;
use kanban_persistence::PersistenceStore;
use kanban_persistence_json::JsonFileStore;
use kanban_service::{
    json_backend::JsonDataStore, AppConfig, KanbanBackend, KanbanContext, KanbanOperations,
    KanbanResult,
};
use std::sync::Arc;
use tempfile::tempdir;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_json_backend(path: &std::path::Path) -> Arc<dyn KanbanBackend> {
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))))
}

fn make_json_data_store(path: &std::path::Path) -> JsonDataStore {
    JsonDataStore::new(Arc::new(JsonFileStore::new(path)))
}

// ─── Step 3: KanbanContext::open_deferred ────────────────────────────────────

/// A fresh `KanbanContext` reports `can_undo() == false` and
/// `can_redo() == false` — the per-session UndoStack starts empty
/// regardless of what the backing store contains.
#[tokio::test(flavor = "multi_thread")]
async fn test_can_undo_returns_false_on_fresh_context() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("deferred.json");
    let ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());
    assert!(!ctx.can_undo());
    assert!(!ctx.can_redo());
}

/// A future-format JSON file must surface as the typed
/// `KanbanError::UnsupportedFutureVersion` variant through the service-layer
/// stack — not as a stringified `Internal(...)`. Otherwise downstream surfaces
/// (CLI / MCP / TUI) can't discriminate the case and `is_unsupported_future_version()`
/// returns false.
#[tokio::test(flavor = "multi_thread")]
async fn test_context_open_returns_typed_unsupported_future_version_for_v99_json_file() {
    use serde_json::json;
    let dir = tempdir().unwrap();
    let path = dir.path().join("future.json");
    let v99 = json!({
        "version": 99,
        "metadata": {
            "instance_id": "550e8400-e29b-41d4-a716-446655440000",
            "saved_at": "2030-01-01T00:00:00Z"
        },
        "data": {}
    });
    std::fs::write(&path, v99.to_string()).unwrap();

    // KanbanContext::open is what every surface (CLI, MCP, TUI) calls.
    // The first batch_count() inside open() triggers ensure_loaded which
    // hits the refusal guard. Use boards() to force the same load path via
    // a non-deferred call too.
    let backend = make_json_backend(&path);
    let ctx = KanbanContext::open(backend, AppConfig::default()).await;

    match ctx {
        Err(e) => assert!(
            e.is_unsupported_future_version(),
            "expected typed UnsupportedFutureVersion variant, got: {e:?}"
        ),
        Ok(ctx) => {
            // KanbanContext::open may not trigger the read on a JSON backend
            // until the first DataStore call; try once and assert the typed
            // error then.
            let err = ctx
                .boards()
                .expect_err("listing boards on a v99 file must fail");
            assert!(
                err.is_unsupported_future_version(),
                "expected typed UnsupportedFutureVersion variant, got: {err:?}"
            );
        }
    }
}

/// SQLite parallel of the JSON future-version test above. A SQLite file
/// whose `metadata.schema_version` exceeds `SUPPORTED_SCHEMA_VERSION` must
/// surface as the typed `KanbanError::UnsupportedFutureVersion` through
/// `KanbanContext::open` — not as a stringified `Database(...)` or `Internal`.
#[tokio::test(flavor = "multi_thread")]
async fn test_context_open_returns_typed_unsupported_future_version_for_v99_sqlite_file() {
    use kanban_service::sqlite_backend::SqliteBackend;
    let dir = tempdir().unwrap();
    let path = dir.path().join("future.db");
    kanban_persistence_sqlite::write_test_metadata_with_schema_version(&path, 99)
        .await
        .unwrap();

    let err = match SqliteBackend::open(path.to_str().unwrap()).await {
        Ok(_) => panic!("v99 SQLite DB must refuse to open via SqliteBackend"),
        Err(e) => e,
    };
    assert!(
        err.is_unsupported_future_version(),
        "expected typed UnsupportedFutureVersion variant, got: {err:?}"
    );
}

/// `KanbanContext::persistence_metadata` delegates to the backend and surfaces
/// the writer-stamp recorded on the most recent save.
#[tokio::test(flavor = "multi_thread")]
async fn test_context_persistence_metadata_returns_writer_stamp_after_save() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("md.json");
    let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;
    ctx.create_board("Stamped".into(), None)?;
    ctx.save().await?;

    let meta = ctx
        .persistence_metadata()
        .expect("metadata must be exposed after save");
    assert_eq!(
        meta.writer_version.as_deref(),
        Some(kanban_core::KANBAN_VERSION),
    );
    assert_eq!(
        meta.writer_commit.as_deref(),
        Some(kanban_core::KANBAN_COMMIT),
    );
    Ok(())
}

/// `KanbanContext::open_deferred` with a JSON backend must not touch the filesystem.
#[tokio::test(flavor = "multi_thread")]
async fn test_open_context_json_does_no_io_at_construction() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nonexistent.json");
    assert!(!path.exists(), "file should not exist before construction");

    let _ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());

    assert!(
        !path.exists(),
        "KanbanContext::open_deferred must not create the file (zero-I/O construction)"
    );
}

/// Reading boards on a lazily-loaded JSON context triggers the first file load.
#[tokio::test(flavor = "multi_thread")]
async fn test_open_context_first_list_boards_triggers_load() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("pre.json");

    // Pre-populate the JSON file with one board.
    {
        use kanban_domain::{Board, Snapshot};
        use kanban_persistence::{snapshot_to_json_bytes, PersistenceMetadata, StoreSnapshot};

        let snap = Snapshot {
            boards: vec![Board::new("Alpha", None::<String>)],
            ..Snapshot::new()
        };
        let data = snapshot_to_json_bytes(&snap).unwrap();
        let meta = PersistenceMetadata::new(uuid::Uuid::new_v4());
        let jfs = JsonFileStore::new(&path);
        jfs.save(StoreSnapshot {
            data,
            metadata: meta,
        })
        .await
        .unwrap();
    }

    let ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());
    let boards = ctx.boards()?;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "Alpha");
    Ok(())
}

/// Calling `undo()` before any `execute()` must return `Ok(false)` — no panic.
#[tokio::test(flavor = "multi_thread")]
async fn test_open_context_undo_before_any_execute_is_noop() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.json");
    let mut ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());

    assert!(
        !ctx.undo()?,
        "undo before any execute must return false (nothing to undo)"
    );
    Ok(())
}

/// `undo()` after `execute()` reverts the mutation.
#[tokio::test(flavor = "multi_thread")]
async fn test_open_context_undo_works_after_execute() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("undo.json");
    let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;

    ctx.create_board("B".into(), None)?;
    assert_eq!(ctx.boards()?.len(), 1);

    assert!(ctx.undo()?);
    assert!(
        ctx.boards()?.is_empty(),
        "undo must revert the board creation"
    );
    Ok(())
}

/// `save()` flushes the JSON backend's in-memory cache to disk; a second
/// independent store at the same path sees the persisted data.
#[tokio::test(flavor = "multi_thread")]
async fn test_open_context_save_flushes_json_backend() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("save.json");

    {
        let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;
        ctx.create_board("Saved".into(), None)?;
        ctx.save().await?;
    }

    // Independent store: verify the board was written to disk.
    let jds = make_json_data_store(&path);
    let boards = jds.list_boards()?;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "Saved");
    Ok(())
}

/// `reload()` clears the JSON backend's cache so the next read re-loads from
/// disk, picking up externally-written changes.
#[tokio::test(flavor = "multi_thread")]
async fn test_open_context_reload_delegates_to_backend() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("reload.json");

    let mut ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());
    // Trigger initial load (empty file → empty in-memory cache).
    assert!(ctx.boards()?.is_empty());

    // External writer adds a board.
    let external = make_json_data_store(&path);
    external.upsert_board(kanban_domain::Board::new("External", None::<String>))?;
    external.flush().await?;

    // Before reload the context cache is stale.
    assert!(ctx.boards()?.is_empty(), "must be stale before reload");

    ctx.reload().await?;
    let boards = ctx.boards()?;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "External");
    Ok(())
}

/// Undo and redo work correctly on a context opened against a JSON
/// backend that loads lazily on first read.
#[tokio::test(flavor = "multi_thread")]
async fn test_undo_redo_work_with_lazy_json_backend() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lazy.json");
    let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;

    ctx.create_board("A".into(), None)?;
    ctx.create_board("B".into(), None)?;
    assert_eq!(ctx.boards()?.len(), 2);

    assert!(ctx.undo()?);
    let boards = ctx.boards()?;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "A");

    assert!(ctx.redo()?);
    assert_eq!(ctx.boards()?.len(), 2);
    Ok(())
}

/// After `reload()`, undo history is invalidated — `can_undo()` must return `false`.
#[tokio::test(flavor = "multi_thread")]
async fn test_can_undo_returns_false_after_reload() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("reload_undo.json");
    let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;

    ctx.create_board("B".into(), None)?;
    assert!(ctx.can_undo(), "must be undoable before reload");

    ctx.save().await?;
    ctx.reload().await?;

    assert!(!ctx.can_undo(), "undo history must be invalid after reload");
    Ok(())
}

// ─── Step 6: Reload behaviour (service layer) ────────────────────────────────

/// Writing to the JSON file externally (bypassing the context) and then calling
/// `reload()` makes the updated data visible on the next read.
#[tokio::test(flavor = "multi_thread")]
async fn test_reload_after_external_json_change_returns_updated_data() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("ext_change.json");

    let mut ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());

    // External writer adds a board, bypassing ctx.
    let external = make_json_data_store(&path);
    external.upsert_board(kanban_domain::Board::new("External", None::<String>))?;
    external.flush().await?;

    // reload() clears the lazy cache; next read loads the updated file.
    ctx.reload().await?;
    let boards = ctx.boards()?;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "External");
    Ok(())
}

// ─── SQLite-specific tests (Step 3 + Step 6) ─────────────────────────────────

#[cfg(feature = "sqlite")]
mod sqlite_tests {
    use super::*;
    use kanban_domain::DataStore;
    use kanban_service::sqlite_backend::SqliteBackend;

    /// `open_deferred` with a SQLite backend issues no DB queries at
    /// construction; a fresh `KanbanContext` reports no undo history.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_open_context_sqlite_open_deferred_has_no_undo_history() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.sqlite3");
        let backend = SqliteBackend::open(path.to_str().unwrap()).await.unwrap();
        let ctx = KanbanContext::open_deferred(Arc::new(backend), AppConfig::default());
        assert!(!ctx.can_undo());
    }

    /// `save()` on a SQLite-backed context runs a WAL checkpoint and returns
    /// `Ok(())` — it succeeds even on a freshly-opened empty database.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_open_context_sqlite_save_succeeds() -> KanbanResult<()> {
        let dir = tempdir().unwrap();
        let path = dir.path().join("noop.sqlite3");
        let backend = SqliteBackend::open(path.to_str().unwrap()).await?;
        let ctx = KanbanContext::open_deferred(Arc::new(backend), AppConfig::default());
        ctx.save().await?;
        Ok(())
    }

    /// SQLite reads are always live: a board written by a second `SqliteBackend`
    /// instance is immediately visible to the first context — no `reload()` needed.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_reload_on_sqlite_ctx_is_transparent() -> KanbanResult<()> {
        let dir = tempdir().unwrap();
        let path = dir.path().join("live.sqlite3");

        let backend1 = SqliteBackend::open(path.to_str().unwrap()).await?;
        let ctx = KanbanContext::open_deferred(Arc::new(backend1), AppConfig::default());

        // Second instance writes a board directly to the DB.
        let backend2 = SqliteBackend::open(path.to_str().unwrap()).await?;
        backend2.upsert_board(kanban_domain::Board::new("Via2nd", None::<String>))?;

        // First context sees the write immediately — SQLite reads are live.
        let boards = ctx.boards()?;
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].name, "Via2nd");
        Ok(())
    }
}

// ─── replace_backend ─────────────────────────────────────────────────────────

/// `replace_backend` resets undo history — `can_undo()` returns `false` after the swap.
#[tokio::test(flavor = "multi_thread")]
async fn test_replace_backend_resets_undo_history() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let mut ctx = KanbanContext::open(
        make_json_backend(&dir.path().join("a.json")),
        AppConfig::default(),
    )
    .await?;
    ctx.create_board("A".into(), None)?;
    assert!(ctx.can_undo());

    ctx.replace_backend(make_json_backend(&dir.path().join("b.json")));
    assert!(!ctx.can_undo(), "replace_backend must reset undo history");
    Ok(())
}

/// `replace_backend` resets redo history — `can_redo()` returns `false` after the swap.
#[tokio::test(flavor = "multi_thread")]
async fn test_replace_backend_resets_redo_history() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let mut ctx = KanbanContext::open(
        make_json_backend(&dir.path().join("a.json")),
        AppConfig::default(),
    )
    .await?;
    ctx.create_board("A".into(), None)?;
    ctx.undo()?;
    assert!(ctx.can_redo());

    ctx.replace_backend(make_json_backend(&dir.path().join("b.json")));
    assert!(!ctx.can_redo(), "replace_backend must reset redo history");
    Ok(())
}

/// After `replace_backend`, reads are served from the new backend.
#[tokio::test(flavor = "multi_thread")]
async fn test_replace_backend_reads_go_to_new_backend() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path_b = dir.path().join("b.json");

    // Pre-populate backend B with one board.
    let writer = make_json_data_store(&path_b);
    writer.upsert_board(kanban_domain::Board::new("B", None::<String>))?;
    writer.flush().await?;

    let mut ctx = KanbanContext::open(
        make_json_backend(&dir.path().join("a.json")),
        AppConfig::default(),
    )
    .await?;
    ctx.create_board("A".into(), None)?;
    assert_eq!(ctx.boards()?.len(), 1);
    assert_eq!(ctx.boards()?[0].name, "A");

    ctx.replace_backend(make_json_backend(&path_b));
    let boards = ctx.boards()?;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "B", "reads must come from the new backend");
    Ok(())
}

/// After `replace_backend`, `is_dirty()` returns `false`.
#[tokio::test(flavor = "multi_thread")]
async fn test_replace_backend_clears_dirty_flag() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let mut ctx = KanbanContext::open(
        make_json_backend(&dir.path().join("a.json")),
        AppConfig::default(),
    )
    .await?;
    ctx.create_board("A".into(), None)?;
    assert!(ctx.is_dirty());

    ctx.replace_backend(make_json_backend(&dir.path().join("b.json")));
    assert!(!ctx.is_dirty(), "replace_backend must clear the dirty flag");
    Ok(())
}

/// Opening a session against a pre-populated file must leave the
/// existing state visible and undoable. Undo of a single new
/// CreateBoard must leave the pre-existing boards intact.
#[tokio::test(flavor = "multi_thread")]
async fn test_open_session_undo_preserves_preexisting_boards() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("restart.json");

    // Session 0: pre-populate the file with B0.
    {
        let jds = make_json_data_store(&path);
        jds.upsert_board(kanban_domain::Board::new("B0", None::<String>))?;
        jds.flush().await?;
    }

    // Session 1: open, create B1, save.
    {
        let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;
        assert_eq!(ctx.boards()?.len(), 1);
        ctx.create_board("B1".into(), None)?;
        ctx.save().await?;
    }

    // Session 2: open, create B2, undo. B0 and B1 must remain.
    let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;
    ctx.create_board("B2".into(), None)?;
    assert_eq!(ctx.boards()?.len(), 3);

    assert!(ctx.undo()?);
    let boards = ctx.boards()?;
    assert_eq!(boards.len(), 2);
    let names: Vec<&str> = boards.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"B0"));
    assert!(names.contains(&"B1"));
    Ok(())
}

/// After `replace_backend`, `execute()` must work directly against the
/// new backend. The UndoStack lives on `KanbanContext` and is reset by
/// `replace_backend`; no separate init hook is needed.
#[tokio::test(flavor = "multi_thread")]
async fn test_replace_backend_then_execute_succeeds() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let mut ctx = KanbanContext::open(
        make_json_backend(&dir.path().join("a.json")),
        AppConfig::default(),
    )
    .await?;

    ctx.replace_backend(make_json_backend(&dir.path().join("b.json")));

    ctx.create_board("B".into(), None)?;
    assert_eq!(ctx.boards()?.len(), 1);
    Ok(())
}

// ─── Gap F: open_context with corrupt file ────────────────────────────────────

/// Opening a context backed by a corrupt JSON file must return an error on the
/// first read — not silently produce an empty board list.
#[tokio::test(flavor = "multi_thread")]
async fn test_open_context_with_corrupt_json_returns_error_on_first_read() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("corrupt.json");
    std::fs::write(&path, b"NOT VALID JSON {{{").unwrap();

    let ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());
    let result = ctx.boards();
    assert!(
        result.is_err(),
        "reading a corrupt JSON file must return an error"
    );
}

/// After `reload()`, the backend must not be marked dirty. A dirty
/// reload would cause a spurious save after every external-change
/// reload.
#[tokio::test(flavor = "multi_thread")]
async fn test_reload_does_not_mark_backend_dirty() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("reload.json");
    let backend = make_json_backend(&path);
    let mut ctx = KanbanContext::open(backend.clone(), AppConfig::default())
        .await
        .unwrap();

    ctx.create_board("B".into(), None).unwrap();
    ctx.save().await.unwrap();

    ctx.reload().await.unwrap();

    assert!(
        !backend.needs_flush(),
        "backend must not be dirty after reload()"
    );
}
