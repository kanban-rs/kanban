//! `InMemoryStore`, used by `command_replay.rs`, never checks a card's prefix
//! against a backing row, so `JsonDataStore` and `SqliteBackend` are used here
//! instead: both reject an unbacked prefix.

use kanban_domain::commands::{CardCommand, Command, CommandContext};
use kanban_domain::data_store::DataStore;
use kanban_domain::{
    CommandBatch, CreateCardOptions, KanbanOperations, KanbanResult, Prefix, Snapshot,
};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{write_full_snapshot, AppConfig, KanbanBackend, KanbanContext};
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
    write_full_snapshot(store.as_ref(), Snapshot::new()).unwrap();
    store
}

fn path_for(backend: &Backend, dir: &std::path::Path, stem: &str) -> std::path::PathBuf {
    match backend {
        Backend::Json => dir.join(format!("{stem}.json")),
        Backend::Sqlite => dir.join(format!("{stem}.sqlite")),
    }
}

async fn record_one_card_creation(
    backend: &Backend,
    dir: &std::path::Path,
    board_prefix: &str,
) -> Vec<CommandBatch> {
    let mut ctx = open(backend, &path_for(backend, dir, "src")).await;

    let board = ctx
        .create_board("B".into(), Some(board_prefix.into()))
        .unwrap();
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
    batches
}

fn replay_onto(store: &dyn DataStore, batches: &[CommandBatch]) {
    let cmd_ctx = CommandContext { store };
    for batch in batches {
        for cmd in &batch.commands {
            cmd.execute(&cmd_ctx).unwrap();
        }
    }
}

async fn assert_replay_reconstructs_prefix_row(backend: Backend) {
    let dir = tempdir().unwrap();
    let batches = record_one_card_creation(&backend, dir.path(), "KAN").await;

    let replay_store = fresh_destination(&backend, &path_for(&backend, dir.path(), "replay")).await;
    replay_onto(replay_store.as_ref(), &batches);

    let row = replay_store
        .get_prefix("kan")
        .unwrap()
        .unwrap_or_else(|| panic!("replay must reconstruct the 'kan' prefix row"));
    assert_eq!(
        row.card_counter, 1,
        "the reconstructed row must cover the card number it names"
    );
}

/// A replay must never lower a namespace's counter below what the
/// destination already recorded. A naive implementation that overwrites
/// `card_counter` with the replayed command's number, instead of taking the
/// max, would silently rewind it here and later reissue a number already
/// handed out.
async fn assert_replay_never_lowers_an_existing_counter(backend: Backend) {
    let dir = tempdir().unwrap();
    let batches = record_one_card_creation(&backend, dir.path(), "KAN").await;

    let replay_store = fresh_destination(&backend, &path_for(&backend, dir.path(), "replay")).await;
    replay_store
        .upsert_prefix(Prefix {
            name: "kan".into(),
            card_counter: 10,
            sprint_counter: 7,
        })
        .unwrap();

    replay_onto(replay_store.as_ref(), &batches);

    let row = replay_store
        .get_prefix("kan")
        .unwrap()
        .unwrap_or_else(|| panic!("the pre-seeded 'kan' prefix row must still be backed"));
    assert_eq!(
        row.sprint_counter, 7,
        "replaying a card create must not touch the sprint counter"
    );
    assert_eq!(
        row.card_counter, 10,
        "replaying a lower card number must not roll the counter back"
    );
}

/// End-to-end outcome only: every backend storage layer normalises on
/// write, so this cannot isolate `lifecycle.rs`'s own normalisation --
/// see `prefix_write_order.rs` for that.
async fn assert_replay_stores_one_normalised_row_for_a_mixed_case_prefix(backend: Backend) {
    let dir = tempdir().unwrap();
    let batches = record_one_card_creation(&backend, dir.path(), "Kan").await;

    let replay_store = fresh_destination(&backend, &path_for(&backend, dir.path(), "replay")).await;
    replay_onto(replay_store.as_ref(), &batches);

    let rows = replay_store.list_prefixes().unwrap();
    assert_eq!(
        rows.len(),
        1,
        "a mixed-case prefix must resolve to exactly one row, got {rows:?}"
    );
    assert_eq!(
        rows[0].name, "kan",
        "the row must be stored under its normalised name"
    );
}

async fn assert_replaying_twice_is_idempotent(backend: Backend) {
    let dir = tempdir().unwrap();
    let batches = record_one_card_creation(&backend, dir.path(), "KAN").await;

    let replay_store = fresh_destination(&backend, &path_for(&backend, dir.path(), "replay")).await;
    replay_onto(replay_store.as_ref(), &batches);
    let first = replay_store.get_prefix("kan").unwrap().unwrap();

    let create_card = batches
        .iter()
        .flat_map(|b| &b.commands)
        .find(|c| matches!(c, Command::Card(CardCommand::Create(_))))
        .expect("batches must contain the CreateCard command");
    let cmd_ctx = CommandContext {
        store: replay_store.as_ref(),
    };
    create_card.execute(&cmd_ctx).unwrap();
    let second = replay_store.get_prefix("kan").unwrap().unwrap();

    assert_eq!(
        first, second,
        "re-executing the same CreateCard command must leave the prefix row unchanged"
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

#[tokio::test(flavor = "multi_thread")]
async fn test_json_replay_never_lowers_an_existing_counter() -> KanbanResult<()> {
    assert_replay_never_lowers_an_existing_counter(Backend::Json).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_replay_never_lowers_an_existing_counter() -> KanbanResult<()> {
    assert_replay_never_lowers_an_existing_counter(Backend::Sqlite).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_replay_stores_one_normalised_row_for_a_mixed_case_prefix() -> KanbanResult<()> {
    assert_replay_stores_one_normalised_row_for_a_mixed_case_prefix(Backend::Json).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_replay_stores_one_normalised_row_for_a_mixed_case_prefix() -> KanbanResult<()>
{
    assert_replay_stores_one_normalised_row_for_a_mixed_case_prefix(Backend::Sqlite).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_replaying_twice_is_idempotent() -> KanbanResult<()> {
    assert_replaying_twice_is_idempotent(Backend::Json).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_replaying_twice_is_idempotent() -> KanbanResult<()> {
    assert_replaying_twice_is_idempotent(Backend::Sqlite).await;
    Ok(())
}
