//! Card and sprint numbering must survive `export --board` / `import`.
//!
//! The prefix rows hold every number a workspace has handed out. An export that
//! omits them hands the destination the cards but not the counters, so the next
//! card minted re-uses a number that is already on a card in the same store.
//!
//! Import is a MERGE into a populated store, unlike migrate which writes a
//! fresh destination. That makes the direction of the merge load-bearing: a
//! destination already ahead of the import must not be rolled backwards, or the
//! collision simply happens from the other side.

use kanban_domain::{CreateCardOptions, KanbanOperations, KanbanResult, Prefix};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

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

/// A board with `n` cards, addressed by `prefix`. Returns the exported JSON.
fn seed_and_export(ctx: &mut KanbanContext, prefix: &str, n: u32) -> KanbanResult<String> {
    use kanban_domain::{BoardUpdate, FieldUpdate};

    let board = ctx.create_board("Source".into(), None)?;
    ctx.update_board(
        board.id,
        BoardUpdate {
            card_prefix: FieldUpdate::Set(prefix.to_string()),
            ..Default::default()
        },
    )?;
    let column = ctx.create_column(board.id, "TODO".into(), None)?;
    for i in 0..n {
        ctx.create_card(
            board.id,
            column.id,
            format!("card {i}"),
            CreateCardOptions::default(),
        )?;
    }
    ctx.export_board(Some(board.id))
}

fn counter_of(ctx: &KanbanContext, name: &str) -> u32 {
    ctx.backend()
        .get_prefix(name)
        .unwrap()
        .map_or(0, |p| p.card_counter)
}

/// The headline case, end to end: the number the destination hands out next
/// must not already be taken.
async fn assert_no_collision_after_round_trip(mut src: KanbanContext, mut dest: KanbanContext) {
    let exported = seed_and_export(&mut src, "TASK", 3).unwrap();

    dest.import_board(&exported).unwrap();

    let board = dest.list_boards().unwrap()[0].clone();
    let column = dest.list_columns(board.id).unwrap()[0].clone();
    let existing: Vec<u32> = dest
        .list_all_cards()
        .unwrap()
        .iter()
        .map(|c| c.card_number)
        .collect();

    let fresh = dest
        .create_card(
            board.id,
            column.id,
            "probe".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    assert!(
        !existing.contains(&fresh.card_number),
        "imported cards hold {existing:?}, and the next card minted {} — a duplicate",
        fresh.card_number
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_export_import_does_not_mint_a_colliding_card_number() {
    let dir = tempdir().unwrap();
    let src = open_json(&dir.path().join("src.json")).await;
    let dest = open_json(&dir.path().join("dest.json")).await;
    assert_no_collision_after_round_trip(src, dest).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_export_import_does_not_mint_a_colliding_card_number() {
    let dir = tempdir().unwrap();
    let src = open_sqlite(&dir.path().join("src.db")).await;
    let dest = open_sqlite(&dir.path().join("dest.db")).await;
    assert_no_collision_after_round_trip(src, dest).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_single_board_export_carries_the_counters_for_the_prefixes_it_uses() {
    let dir = tempdir().unwrap();
    let mut src = open_json(&dir.path().join("src.json")).await;

    let exported = seed_and_export(&mut src, "TASK", 3).unwrap();
    let value: serde_json::Value = serde_json::from_str(&exported).unwrap();

    let rows = value["prefixes"].as_array().expect("prefixes array");
    let task = rows
        .iter()
        .find(|p| p["name"] == "task")
        .expect("the namespace the exported cards are addressed by must be carried");
    assert_eq!(
        task["card_counter"], 3,
        "the counter must match what the board has handed out"
    );
}

/// Exporting one board must not disclose or transplant the numbering of
/// namespaces it does not address.
#[tokio::test(flavor = "multi_thread")]
async fn test_single_board_export_omits_prefixes_it_does_not_address() {
    use kanban_domain::{BoardUpdate, FieldUpdate};

    let dir = tempdir().unwrap();
    let mut src = open_json(&dir.path().join("src.json")).await;

    let other = src.create_board("Other".into(), None).unwrap();
    src.update_board(
        other.id,
        BoardUpdate {
            card_prefix: FieldUpdate::Set("OTHER".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let other_col = src.create_column(other.id, "TODO".into(), None).unwrap();
    src.create_card(
        other.id,
        other_col.id,
        "elsewhere".into(),
        CreateCardOptions::default(),
    )
    .unwrap();

    let exported = seed_and_export(&mut src, "TASK", 2).unwrap();
    let value: serde_json::Value = serde_json::from_str(&exported).unwrap();

    let names: Vec<String> = value["prefixes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect();
    assert!(
        !names.contains(&"other".to_string()),
        "an unrelated board's namespace leaked into a single-board export: {names:?}"
    );
}

/// Import merges into a populated store. A destination already ahead must keep
/// its own counter, or the numbers it hands out next collide with its own cards.
#[tokio::test(flavor = "multi_thread")]
async fn test_import_never_lowers_a_counter_the_destination_is_already_past() {
    let dir = tempdir().unwrap();
    let mut src = open_json(&dir.path().join("src.json")).await;
    let mut dest = open_json(&dir.path().join("dest.json")).await;

    let exported = seed_and_export(&mut src, "TASK", 2).unwrap();

    dest.backend()
        .upsert_prefix(Prefix {
            name: "task".into(),
            card_counter: 99,
            sprint_counter: 7,
        })
        .unwrap();

    dest.import_board(&exported).unwrap();

    assert_eq!(
        counter_of(&dest, "task"),
        99,
        "the destination was at 99 and the import carried 2; taking the import would \
         re-mint numbers the destination has already used"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_raises_a_counter_the_destination_is_behind() {
    let dir = tempdir().unwrap();
    let mut src = open_json(&dir.path().join("src.json")).await;
    let mut dest = open_json(&dir.path().join("dest.json")).await;

    let exported = seed_and_export(&mut src, "TASK", 5).unwrap();

    dest.backend()
        .upsert_prefix(Prefix {
            name: "task".into(),
            card_counter: 1,
            sprint_counter: 0,
        })
        .unwrap();

    dest.import_board(&exported).unwrap();

    assert_eq!(
        counter_of(&dest, "task"),
        5,
        "the import carried 5 and the destination was at 1; staying at 1 would \
         re-mint the imported cards' numbers"
    );
}

/// Files written by the current release carry no prefix rows at all. Importing
/// one must still not collide, which means deriving the counter from the
/// highest number actually present on the imported cards.
#[tokio::test(flavor = "multi_thread")]
async fn test_importing_a_prefixless_export_derives_the_counter_from_the_cards() {
    let dir = tempdir().unwrap();
    let mut src = open_json(&dir.path().join("src.json")).await;
    let mut dest = open_json(&dir.path().join("dest.json")).await;

    let exported = seed_and_export(&mut src, "TASK", 4).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&exported).unwrap();
    value["prefixes"] = serde_json::json!([]);
    let legacy = serde_json::to_string(&value).unwrap();

    dest.import_board(&legacy).unwrap();

    assert_eq!(
        counter_of(&dest, "task"),
        4,
        "an export with no counters must have them reconstructed from its cards"
    );
}
