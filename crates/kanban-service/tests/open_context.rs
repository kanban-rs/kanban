/// End-to-end tests for the `open_context()` free function (Step 5 of the
/// "Unified Backends via True Deferred Reads" architecture).
///
/// All tests call `kanban_service::open_context(locator, cfg)` and exercise
/// the full detection + backend-creation pipeline with real TempDir files.
use kanban_service::{open_context, AppConfig, KanbanOperations, KanbanResult};
use tempfile::tempdir;

/// JSON round-trip: create a board, save, reopen, board is still there.
#[tokio::test(flavor = "multi_thread")]
async fn test_open_context_json_end_to_end() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");

    {
        let mut ctx = open_context(path.to_str().unwrap(), AppConfig::default()).await?;
        ctx.create_board("Board1".into(), None)?;
        ctx.save().await?;
    }

    let ctx = open_context(path.to_str().unwrap(), AppConfig::default()).await?;
    let boards = ctx.boards()?;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "Board1");
    Ok(())
}

/// SQLite round-trip: create a board (write-through), reopen, board persists.
#[cfg(feature = "sqlite")]
mod sqlite_tests {
    use super::*;
    use kanban_persistence_sqlite::SqliteStore;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_open_context_sqlite_end_to_end() -> KanbanResult<()> {
        let dir = tempdir().unwrap();
        let path = dir.path().join("board.sqlite");

        {
            let mut ctx = open_context(path.to_str().unwrap(), AppConfig::default()).await?;
            ctx.create_board("Board1".into(), None)?;
            // SQLite is write-through — no explicit save() needed.
        }

        let ctx = open_context(path.to_str().unwrap(), AppConfig::default()).await?;
        let boards = ctx.boards()?;
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].name, "Board1");
        Ok(())
    }

    /// `open_context` detects SQLite from magic bytes when the file has no
    /// recognised extension.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_open_context_auto_detects_backend_from_magic_bytes() -> KanbanResult<()> {
        let dir = tempdir().unwrap();
        let path = dir.path().join("noext");

        // Create a SQLite file with no extension so magic-byte detection kicks in.
        SqliteStore::open(path.to_str().unwrap()).await.unwrap();

        let mut ctx = open_context(path.to_str().unwrap(), AppConfig::default()).await?;
        ctx.create_board("B".into(), None)?;
        let boards = ctx.boards()?;
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].name, "B");
        Ok(())
    }
}

/// A context from `open_deferred` is immediately ready to execute
/// against. The UndoStack starts empty; no extra setup step needed.
#[tokio::test(flavor = "multi_thread")]
async fn test_open_deferred_context_executes_immediately() {
    use kanban_domain::commands::{BoardCommand, Command, CreateBoard};
    use kanban_domain::InMemoryStore;
    use std::sync::Arc;

    let mut ctx = kanban_service::KanbanContext::open_deferred(
        Arc::new(InMemoryStore::new()),
        kanban_service::AppConfig::default(),
    );
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "Test".into(),
        card_prefix: None,
        position: 0,
    }));
    ctx.execute(vec![cmd]).expect("execute should succeed");
    assert_eq!(ctx.boards().unwrap().len(), 1);
}

/// `KanbanContext::execute` records exactly one `CommandBatch` per
/// transaction, holding the full `Vec<Command>` plus shared provenance.
/// This test verifies the round-trip: execute -> store -> `load_all_batches`
/// returns one batch with the original commands and populated provenance.
#[test]
fn test_execute_records_one_command_batch_with_provenance() {
    use kanban_domain::command_store::CommandStore;
    use kanban_domain::commands::{BoardCommand, Command, CreateBoard};
    use kanban_domain::InMemoryStore;
    use kanban_service::KanbanBackend;
    use std::sync::Arc;

    let store = Arc::new(InMemoryStore::new());
    let mut ctx = kanban_service::KanbanContext::open_deferred(
        Arc::clone(&store) as Arc<dyn KanbanBackend>,
        kanban_service::AppConfig::default(),
    );
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "Batch Test".into(),
        card_prefix: None,
        position: 0,
    }));

    ctx.execute(vec![cmd.clone()])
        .expect("execute should succeed");

    let (batches, batch_count) = store.load_all_batches().unwrap();
    assert_eq!(batch_count, 1, "one execute call = one batch");
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];
    assert_eq!(
        batch.commands,
        vec![cmd],
        "the batch must hold the original commands"
    );
    assert_ne!(
        batch.session_id,
        uuid::Uuid::nil(),
        "provenance: session_id must be populated"
    );
    assert_eq!(
        batch.app_version,
        kanban_core::KANBAN_VERSION,
        "provenance: app_version must be the current kanban version"
    );
    assert_eq!(
        batch.app_type,
        kanban_core::AppType::Unknown,
        "provenance: app_type defaults to Unknown for open_deferred"
    );
}

/// Setting an app type via `with_app_type` propagates onto every recorded
/// batch, proving the attribution mechanism end-to-end.
#[test]
fn test_execute_with_app_type_records_that_app_type() {
    use kanban_domain::command_store::CommandStore;
    use kanban_domain::commands::{BoardCommand, Command, CreateBoard};
    use kanban_domain::InMemoryStore;
    use kanban_service::KanbanBackend;
    use std::sync::Arc;

    let store = Arc::new(InMemoryStore::new());
    let mut ctx = kanban_service::KanbanContext::open_deferred(
        Arc::clone(&store) as Arc<dyn KanbanBackend>,
        kanban_service::AppConfig::default(),
    )
    .with_app_type(kanban_core::AppType::Cli);
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "App Type Test".into(),
        card_prefix: None,
        position: 0,
    }));

    ctx.execute(vec![cmd]).expect("execute should succeed");

    let (batches, _) = store.load_all_batches().unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(
        batches[0].app_type,
        kanban_core::AppType::Cli,
        "with_app_type(Cli) must attribute the recorded batch to the CLI surface"
    );
}

/// A non-existent path produces an empty context (no boards).
#[tokio::test(flavor = "multi_thread")]
async fn test_open_context_new_file_starts_empty() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("new.json");

    let ctx = open_context(path.to_str().unwrap(), AppConfig::default()).await?;
    assert!(ctx.boards()?.is_empty());
    Ok(())
}
