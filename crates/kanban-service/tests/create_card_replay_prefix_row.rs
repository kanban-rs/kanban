//! `InMemoryStore`, used by `command_replay.rs`, never checks a card's prefix
//! against a backing row, so `JsonDataStore` and `SqliteBackend` are used here
//! instead: both reject an unbacked prefix.

use kanban_domain::commands::CommandContext;
use kanban_domain::data_store::DataStore;
use kanban_domain::{CreateCardOptions, KanbanOperations, KanbanResult, Snapshot};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

enum Backend {
    Json,
    Sqlite,
}

async fn open(backend: &Backend, path: &std::path::Path) -> KanbanContext {
    let inner: Arc<dyn KanbanBackend> = match backend {
        Backend::Json => Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path)))),
        Backend::Sqlite => Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap()),
    };
    KanbanContext::open(inner, AppConfig::default())
        .await
        .unwrap()
}

async fn fresh_destination(backend: &Backend, path: &std::path::Path) -> Arc<dyn DataStore> {
    let store: Arc<dyn DataStore> = match backend {
        Backend::Json => Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path)))),
        Backend::Sqlite => Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap()),
    };
    store.apply_snapshot(Snapshot::new()).unwrap();
    store
}

fn path_for(backend: &Backend, dir: &std::path::Path, stem: &str) -> std::path::PathBuf {
    match backend {
        Backend::Json => dir.join(format!("{stem}.json")),
        Backend::Sqlite => dir.join(format!("{stem}.sqlite")),
    }
}

async fn assert_replay_reconstructs_prefix_row(backend: Backend) {
    let dir = tempdir().unwrap();
    let mut ctx = open(&backend, &path_for(&backend, dir.path(), "src")).await;

    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let column = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "card 1".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    assert_eq!(card.card_number, 1);

    let (batches, count) = ctx.backend().load_all_batches().unwrap();
    assert!(count > 0, "should have recorded at least one command batch");

    let replay_store = fresh_destination(&backend, &path_for(&backend, dir.path(), "replay")).await;
    let cmd_ctx = CommandContext {
        store: replay_store.as_ref(),
    };
    for batch in &batches {
        for cmd in &batch.commands {
            cmd.execute(&cmd_ctx).unwrap();
        }
    }

    let row = replay_store
        .get_prefix("kan")
        .unwrap()
        .unwrap_or_else(|| panic!("replay must reconstruct the 'kan' prefix row"));
    assert_eq!(
        row.card_counter, 1,
        "the reconstructed row must cover the card number it names"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_replay_from_baseline_reconstructs_prefix_row() -> KanbanResult<()> {
    assert_replay_reconstructs_prefix_row(Backend::Json).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_replay_from_baseline_reconstructs_prefix_row() -> KanbanResult<()> {
    assert_replay_reconstructs_prefix_row(Backend::Sqlite).await;
    Ok(())
}
