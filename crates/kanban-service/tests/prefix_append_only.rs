//! A prefix row is a permanent record: the identifier it names has already
//! left the database (branch names, PR titles), so it cannot be reused for a
//! different namespace. Changing a board's card prefix must resolve the NEXT
//! allocation to a different row and leave the old row, counter and all,
//! untouched.
//!
//! `prefix_allocation.rs::test_batch_and_single_resolution_agree_after_a_board_rename`
//! already changes a board's prefix mid-test, but it only pins identifier
//! RESOLUTION (single vs batch agree on the new prefix). It never inspects a
//! prefix row or a counter, so it says nothing about whether the old row
//! survives -- that is what this file pins.

use kanban_domain::{BoardUpdate, CreateCardOptions, FieldUpdate, KanbanOperations, Prefix};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

async fn open_json(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

async fn open_sqlite(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap());
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

enum Backend {
    Json,
    Sqlite,
}

async fn open(backend: &Backend, path: &std::path::Path) -> KanbanContext {
    match backend {
        Backend::Json => open_json(path).await,
        Backend::Sqlite => open_sqlite(path).await,
    }
}

fn path_for(backend: &Backend, dir: &std::path::Path, stem: &str) -> std::path::PathBuf {
    match backend {
        Backend::Json => dir.join(format!("{stem}.json")),
        Backend::Sqlite => dir.join(format!("{stem}.sqlite")),
    }
}

fn row_of(ctx: &KanbanContext, name: &str) -> Option<Prefix> {
    ctx.backend().get_prefix(name).unwrap()
}

fn counter_of(ctx: &KanbanContext, name: &str) -> u32 {
    row_of(ctx, name).map_or(0, |p| p.card_counter)
}

async fn seed(ctx: &mut KanbanContext) -> (Uuid, Uuid) {
    let board = ctx
        .create_board("Renamed".into(), Some("OLD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    for i in 0..3 {
        ctx.create_card(
            board.id,
            column.id,
            format!("card {i}"),
            CreateCardOptions::default(),
        )
        .unwrap();
    }
    assert_eq!(
        counter_of(ctx, "old"),
        3,
        "precondition: seeding must hand out task 1..3 under the old namespace"
    );
    (board.id, column.id)
}

fn change_prefix(ctx: &mut KanbanContext, board_id: Uuid) {
    ctx.update_board(
        board_id,
        BoardUpdate {
            card_prefix: FieldUpdate::Set("NEW".into()),
            ..Default::default()
        },
    )
    .unwrap();
}

async fn assert_old_row_and_cards_intact(backend: Backend) {
    let dir = tempdir().unwrap();
    let path = path_for(&backend, dir.path(), "store");
    let mut ctx = open(&backend, &path).await;

    let (board_id, _) = seed(&mut ctx).await;
    change_prefix(&mut ctx, board_id);

    let old_row = row_of(&ctx, "old").expect("the old row must not disappear on a prefix change");
    assert_eq!(
        old_row.name, "old",
        "the old row was renamed rather than left behind"
    );
    assert_eq!(
        old_row.card_counter, 3,
        "the old row's counter must stay at what it had already handed out"
    );
    assert_eq!(
        counter_of(&ctx, "new"),
        0,
        "changing the board's prefix must not create or advance the new namespace on its own"
    );
    let cards = ctx.list_all_cards().unwrap();
    assert_eq!(cards.len(), 3, "the board's original cards must survive");
    assert!(
        cards.iter().all(|c| c.prefix == "OLD"),
        "an existing card's stamped prefix must not be rewritten by the board's rename"
    );

    ctx.save().await.unwrap();
    drop(ctx);
    let reopened = open(&backend, &path).await;

    let old_row = row_of(&reopened, "old")
        .expect("the old row must survive a save and reload, not just an in-session read");
    assert_eq!(
        old_row.name, "old",
        "the old row was renamed rather than left behind (after reload)"
    );
    assert_eq!(
        old_row.card_counter, 3,
        "the old row's counter must survive a save and reload"
    );
    assert_eq!(
        counter_of(&reopened, "new"),
        0,
        "the new namespace must still be untouched after reload"
    );
    let cards = reopened.list_all_cards().unwrap();
    assert_eq!(
        cards.len(),
        3,
        "the board's original cards must survive a save and reload"
    );
    assert!(
        cards.iter().all(|c| c.prefix == "OLD"),
        "a card's stamped prefix must survive a save and reload unchanged"
    );
}

async fn assert_new_row_starts_at_one(backend: Backend) {
    let dir = tempdir().unwrap();
    let path = path_for(&backend, dir.path(), "store");
    let mut ctx = open(&backend, &path).await;

    let (board_id, column_id) = seed(&mut ctx).await;
    change_prefix(&mut ctx, board_id);

    let fresh = ctx
        .create_card(
            board_id,
            column_id,
            "fourth".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    assert_eq!(
        fresh.prefix, "NEW",
        "the fourth card must resolve to the board's changed prefix"
    );
    assert_eq!(
        fresh.card_number, 1,
        "the new namespace must start counting from one"
    );
    assert_eq!(
        counter_of(&ctx, "old"),
        3,
        "the old namespace's counter must not advance for a card minted under the new one"
    );
    assert_eq!(
        counter_of(&ctx, "new"),
        1,
        "the new namespace's counter must reflect exactly the one card minted under it"
    );

    let originals: Vec<_> = ctx
        .list_all_cards()
        .unwrap()
        .into_iter()
        .filter(|c| c.id != fresh.id)
        .collect();
    assert_eq!(originals.len(), 3, "the three original cards must survive");
    let mut original_numbers: Vec<u32> = originals.iter().map(|c| c.card_number).collect();
    original_numbers.sort_unstable();
    assert_eq!(
        original_numbers,
        vec![1, 2, 3],
        "the fourth card must be NEW-1 alongside OLD-1..3, not a replacement for them"
    );
    assert!(
        originals.iter().all(|c| c.prefix == "OLD"),
        "the original three cards must keep their OLD prefix after the board's rename"
    );

    ctx.save().await.unwrap();
    drop(ctx);
    let reopened = open(&backend, &path).await;

    assert_eq!(
        counter_of(&reopened, "old"),
        3,
        "the old namespace's counter must survive a save and reload"
    );
    assert_eq!(
        counter_of(&reopened, "new"),
        1,
        "the new namespace's counter must survive a save and reload"
    );
    let cards = reopened.list_all_cards().unwrap();
    assert_eq!(
        cards.len(),
        4,
        "all four cards must survive a save and reload"
    );
    let fresh_reloaded = cards
        .iter()
        .find(|c| c.prefix == "NEW")
        .expect("the fourth card must still be addressed under the new namespace after reload");
    assert_eq!(fresh_reloaded.card_number, 1);
    let mut reloaded_old_numbers: Vec<u32> = cards
        .iter()
        .filter(|c| c.prefix == "OLD")
        .map(|c| c.card_number)
        .collect();
    reloaded_old_numbers.sort_unstable();
    assert_eq!(
        reloaded_old_numbers,
        vec![1, 2, 3],
        "the original three cards must keep their numbers and OLD prefix after reload"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_changing_a_board_prefix_leaves_the_old_row_and_its_cards_intact() {
    assert_old_row_and_cards_intact(Backend::Json).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_changing_a_board_prefix_leaves_the_old_row_and_its_cards_intact() {
    assert_old_row_and_cards_intact(Backend::Sqlite).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_allocation_after_a_prefix_change_starts_the_new_row_at_one() {
    assert_new_row_starts_at_one(Backend::Json).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_allocation_after_a_prefix_change_starts_the_new_row_at_one() {
    assert_new_row_starts_at_one(Backend::Sqlite).await;
}
